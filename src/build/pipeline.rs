use crate::build::graph::DepGraph;
use crate::build::incremental::{BuildStats, HashCache};
use crate::build::stages;
use crate::build::stages::load::DbPost;
use crate::config::SiteConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

fn serialize_posts(posts: &[crate::content::Post]) -> serde_json::Value {
    serde_json::json!(posts
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id.to_string(),
                "slug": &p.slug,
                "title": &p.title,
                "url": format!("/posts/{}/", p.slug),
                "content": p.content.html(),
                "tags": &p.tags,
                "category": &p.category,
                "excerpt": &p.excerpt,
                "created_at": p.created_at.to_rfc3339(),
                "updated_at": p.updated_at.to_rfc3339(),
                "toc": &p.toc,
                "cover_image": &p.cover_image,
                "author": &p.author,
                "reading_time": p.reading_time,
                "word_count": p.word_count,
            })
        })
        .collect::<Vec<_>>())
}

/// 执行完整构建管道
pub fn execute(
    project_root: &Path,
    config: &SiteConfig,
    plugin_configs: HashMap<String, HashMap<String, serde_json::Value>>,
    theme_saved_config: HashMap<String, serde_json::Value>,
    db_posts: Vec<DbPost>,
) -> Result<BuildStats> {
    tracing::info!("开始构建...");
    let start = std::time::Instant::now();

    let cache_dir = project_root.join(&config.build.cache_dir);
    let mut hash_cache = HashCache::load(&cache_dir);
    let mut dep_graph = DepGraph::load(&cache_dir);

    // 配置文件变更时记录日志（后续迭代用于决定全量重建）
    match hash_cache.config_changed(project_root) {
        Ok(true) => tracing::info!("cblog.toml 已变更，将执行全量重建"),
        Ok(false) => tracing::debug!("cblog.toml 未变更"),
        Err(e) => tracing::warn!("检测配置变更失败：{}", e),
    }

    // 初始化插件引擎（仅在有启用插件时）
    let engine = if !config.plugins.enabled.is_empty() {
        let ordered = crate::plugin::scheduler::resolve_load_order(
            project_root,
            &config.plugins.enabled,
        )?;
        let mut eng = crate::lua::runtime::PluginEngine::new(project_root, config, plugin_configs)?;
        eng.load_plugins(&ordered)?;
        Some(eng)
    } else {
        None
    };

    let ctx = serde_json::json!({
        "project_root": project_root.to_string_lossy(),
    });

    // 阶段 1: content.load - 从数据库加载内容
    let posts = stages::load::load_posts_from_db(db_posts, config);
    tracing::info!("从数据库加载了 {} 篇文章", posts.len());

    if let Some(ref eng) = engine {
        let load_ctx = serde_json::json!({
            "project_root": project_root.to_string_lossy(),
            "posts": serialize_posts(&posts),
        });
        eng.hooks.call_action(&eng.lua, "after_load", &load_ctx)?;
    }

    // 阶段 2: content.parse - 已在加载阶段完成（Front Matter + Markdown）

    // 阶段 3: taxonomy.build - 构建分类索引
    let taxonomy = stages::taxonomy::build_taxonomy(&posts, config);
    tracing::info!(
        "分类索引：{} 个标签，{} 个分类，{} 个月份归档",
        taxonomy.tags.len(),
        taxonomy.categories.len(),
        taxonomy.archives.len()
    );

    if let Some(ref eng) = engine {
        eng.hooks.call_action(&eng.lua, "after_taxonomy", &ctx)?;
    }

    // 阶段 4: page.generate - 生成页面列表
    let pages = stages::generate::generate_pages(&posts, &taxonomy, config);
    let total_pages = pages.len();
    tracing::info!("生成了 {} 个页面", total_pages);

    // 重建依赖图
    dep_graph.clear();
    for page in &pages {
        dep_graph.add_dependency(&page.template, &page.url);
    }

    // 阶段 5: page.render - 渲染所有页面
    stages::render::render_pages(project_root, config, &pages, &theme_saved_config)?;

    if let Some(ref eng) = engine {
        eng.hooks.call_action(&eng.lua, "after_render", &ctx)?;
    }

    // 阶段 6: asset.process - 编译 SCSS、复制主题资源
    stages::assets::process_assets(project_root, config, &theme_saved_config)?;

    if let Some(ref eng) = engine {
        eng.hooks.call_action(&eng.lua, "after_assets", &ctx)?;
    }

    // 阶段 7: build.finalize - 收尾工作
    stages::finalize::finalize(project_root, config, &posts)?;

    if let Some(ref eng) = engine {
        let finalize_ctx = serde_json::json!({
            "project_root": project_root.to_string_lossy(),
            "output_dir": project_root.join(&config.build.output_dir).to_string_lossy(),
            "posts": serialize_posts(&posts),
            "site_url": &config.site.url,
        });
        eng.hooks
            .call_action(&eng.lua, "after_finalize", &finalize_ctx)?;
    }

    // 更新配置文件哈希
    let config_path = project_root.join("cblog.toml");
    if config_path.exists()
        && let Ok(hash) = HashCache::compute_hash(&config_path) {
            hash_cache.update("cblog.toml".to_owned(), hash);
        }

    // 持久化缓存
    if let Err(e) = hash_cache.save() {
        tracing::warn!("保存哈希缓存失败：{}", e);
    }
    if let Err(e) = dep_graph.save() {
        tracing::warn!("保存依赖图失败：{}", e);
    }

    let duration = start.elapsed();
    tracing::info!("构建完成，耗时 {:.2}s", duration.as_secs_f64());

    Ok(BuildStats {
        total_pages,
        rebuilt: total_pages,
        cached: 0,
    })
}
