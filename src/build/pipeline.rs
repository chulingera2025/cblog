use crate::build::graph::DepGraph;
use crate::build::incremental::{BuildStats, HashCache};
use crate::build::stages;
use crate::build::stages::load::DbPost;
use crate::config::SiteConfig;
use crate::admin::settings::SiteSettings;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// 构建管道上下文，聚合构建所需的全部参数
struct BuildContext<'a> {
    project_root: &'a Path,
    config: &'a SiteConfig,
    plugin_configs: &'a HashMap<String, HashMap<String, serde_json::Value>>,
    theme_saved_config: &'a HashMap<String, serde_json::Value>,
    site_settings: &'a SiteSettings,
    hash_cache: &'a mut HashCache,
    dep_graph: &'a mut DepGraph,
}

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

/// 计算单篇文章的内容哈希（基于 slug + content + updated_at + meta 字段）
fn compute_post_hash(db_post: &DbPost) -> String {
    let fingerprint = format!(
        "{}|{}|{}|{}|{}",
        db_post.slug, db_post.title, db_post.content, db_post.updated_at, db_post.meta
    );
    HashCache::hash_bytes(fingerprint.as_bytes())
}

/// 执行构建管道，支持增量构建
///
/// `force` 为 true 时跳过增量判断，执行全量重建
pub fn execute(
    project_root: &Path,
    config: &SiteConfig,
    plugin_configs: HashMap<String, HashMap<String, serde_json::Value>>,
    theme_saved_config: HashMap<String, serde_json::Value>,
    db_posts: Vec<DbPost>,
    site_settings: SiteSettings,
    force: bool,
) -> Result<BuildStats> {
    tracing::info!("开始构建...");
    let start = std::time::Instant::now();

    let cache_dir = project_root.join(&config.build.cache_dir);
    let mut hash_cache = HashCache::load(&cache_dir);
    let mut dep_graph = DepGraph::load(&cache_dir);

    // 判断是否需要全量重建
    let force_full = force || should_full_rebuild(&hash_cache, project_root, config);

    if force_full {
        if force {
            tracing::info!("已指定强制全量重建");
        }
    } else {
        tracing::info!("尝试增量构建");
    }

    let mut bctx = BuildContext {
        project_root,
        config,
        plugin_configs: &plugin_configs,
        theme_saved_config: &theme_saved_config,
        site_settings: &site_settings,
        hash_cache: &mut hash_cache,
        dep_graph: &mut dep_graph,
    };

    // 尝试增量构建，失败时自动回退到全量
    let result = if force_full {
        full_build(&mut bctx, db_posts)
    } else {
        match incremental_build(&mut bctx, &db_posts) {
            Ok(stats) => Ok(stats),
            Err(e) => {
                tracing::warn!("增量构建失败，回退到全量重建：{e}");
                full_build(&mut bctx, db_posts)
            }
        }
    };

    // 构建完成后更新配置文件和模板哈希
    let config_path = project_root.join("cblog.toml");
    if config_path.exists()
        && let Ok(hash) = HashCache::compute_hash(&config_path)
    {
        hash_cache.update("cblog.toml".to_owned(), hash);
    }

    let themes_dir = project_root.join("themes");
    hash_cache.update_templates(&themes_dir, &config.theme.active);

    if let Err(e) = hash_cache.save() {
        tracing::warn!("保存哈希缓存失败：{}", e);
    }
    if let Err(e) = dep_graph.save() {
        tracing::warn!("保存依赖图失败：{}", e);
    }

    let duration = start.elapsed();
    if let Ok(ref stats) = result {
        tracing::info!(
            "构建完成，耗时 {:.2}s（共 {} 页，重建 {}，缓存 {}）",
            duration.as_secs_f64(),
            stats.total_pages,
            stats.rebuilt,
            stats.cached,
        );
    }

    result
}

/// 判断是否需要全量重建
fn should_full_rebuild(hash_cache: &HashCache, project_root: &Path, config: &SiteConfig) -> bool {
    match hash_cache.config_changed(project_root) {
        Ok(true) => {
            tracing::info!("cblog.toml 已变更，将执行全量重建");
            return true;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!("检测配置变更失败，默认全量重建：{e}");
            return true;
        }
    }

    let theme_toml = project_root
        .join("themes")
        .join(&config.theme.active)
        .join("theme.toml");
    if theme_toml.exists() {
        match HashCache::compute_hash(&theme_toml) {
            Ok(hash) => {
                if hash_cache.has_changed("theme.toml", &hash) {
                    tracing::info!("theme.toml 已变更，将执行全量重建");
                    return true;
                }
            }
            Err(e) => {
                tracing::warn!("检测 theme.toml 变更失败：{e}");
                return true;
            }
        }
    }

    let output_dir = project_root.join(&config.build.output_dir);
    if !output_dir.exists() {
        tracing::info!("输出目录不存在，将执行全量重建");
        return true;
    }

    false
}

/// 全量重建
fn full_build(bctx: &mut BuildContext<'_>, db_posts: Vec<DbPost>) -> Result<BuildStats> {
    for db_post in &db_posts {
        let post_key = format!("post:{}", db_post.slug);
        let post_hash = compute_post_hash(db_post);
        bctx.hash_cache.update(post_key, post_hash);
    }

    let theme_toml = bctx.project_root
        .join("themes")
        .join(&bctx.config.theme.active)
        .join("theme.toml");
    if theme_toml.exists()
        && let Ok(hash) = HashCache::compute_hash(&theme_toml)
    {
        bctx.hash_cache.update("theme.toml".to_owned(), hash);
    }

    run_pipeline(bctx, db_posts, None)
}

/// 增量构建：仅重建变更的页面
fn incremental_build(
    bctx: &mut BuildContext<'_>,
    db_posts: &[DbPost],
) -> Result<BuildStats> {
    let themes_dir = bctx.project_root.join("themes");

    let changed_templates = bctx.hash_cache.changed_templates(&themes_dir, &bctx.config.theme.active);

    let mut changed_post_slugs: HashSet<String> = HashSet::new();
    let mut any_post_changed = false;

    for db_post in db_posts {
        let post_key = format!("post:{}", db_post.slug);
        let post_hash = compute_post_hash(db_post);
        if bctx.hash_cache.has_changed(&post_key, &post_hash) {
            changed_post_slugs.insert(db_post.slug.clone());
            any_post_changed = true;
        }
        bctx.hash_cache.update(post_key, post_hash);
    }

    // 检查文章数量变化（新增或删除文章）
    let current_slugs: HashSet<String> = db_posts.iter().map(|p| p.slug.clone()).collect();
    let cached_post_keys: Vec<String> = bctx.hash_cache.cached_post_keys();
    for key in &cached_post_keys {
        let slug = key.strip_prefix("post:").unwrap_or(key);
        if !current_slugs.contains(slug) {
            any_post_changed = true;
            break;
        }
    }

    if !any_post_changed && changed_templates.is_empty() {
        tracing::info!("无内容变更，跳过构建");
        let posts = stages::load::load_posts_from_db(db_posts.to_vec(), bctx.config);
        let taxonomy = stages::taxonomy::build_taxonomy(&posts, bctx.config);
        let pages = stages::generate::generate_pages(&posts, &taxonomy, bctx.config);
        return Ok(BuildStats {
            total_pages: pages.len(),
            rebuilt: 0,
            cached: pages.len(),
        });
    }

    let mut urls_to_rebuild: HashSet<String> = HashSet::new();

    if any_post_changed {
        for slug in &changed_post_slugs {
            urls_to_rebuild.insert(format!("/posts/{slug}/"));
        }
    }

    // 模板变更 → 通过 DepGraph 找到使用该模板的所有页面
    for template_name in &changed_templates {
        let tpl_key = template_name.strip_suffix(".cbtml").unwrap_or(template_name);
        for url in bctx.dep_graph.get_affected_pages(tpl_key) {
            urls_to_rebuild.insert(url.to_owned());
        }
    }

    // 文章内容变更的连锁影响太广（列表排序、分页、标签统计等），全量渲染
    let rebuild_filter = if any_post_changed {
        None
    } else {
        Some(urls_to_rebuild)
    };

    run_pipeline(bctx, db_posts.to_vec(), rebuild_filter)
}

/// 执行完整管道，可选地仅渲染指定 URL 的页面
fn run_pipeline(
    bctx: &mut BuildContext<'_>,
    db_posts: Vec<DbPost>,
    rebuild_urls: Option<HashSet<String>>,
) -> Result<BuildStats> {
    let project_root = bctx.project_root;
    let config = bctx.config;

    // 初始化插件引擎
    const BUILTIN_FEATURES: &[&str] = &["image-optimize", "syntax-highlight", "toc", "search"];
    for plugin_name in &config.plugins.enabled {
        if BUILTIN_FEATURES.contains(&plugin_name.as_str()) {
            tracing::warn!(
                "插件 '{plugin_name}' 已内置为核心功能，请从 [plugins] enabled 中移除。\
                可通过 [features.{section}] enabled = false 禁用。",
                section = plugin_name.replace('-', "_")
            );
        }
    }

    let engine = if !config.plugins.enabled.is_empty() {
        let ordered = crate::plugin::scheduler::resolve_load_order(
            project_root,
            &config.plugins.enabled,
        )?;
        let mut eng = crate::lua::runtime::PluginEngine::new(project_root, config, bctx.plugin_configs.clone())?;
        eng.load_plugins(&ordered)?;
        Some(eng)
    } else {
        None
    };

    // Lua sandbox 只允许相对路径，传给插件的 output_dir 使用配置中的相对路径
    let output_dir_str = config.build.output_dir.clone();

    // 阶段 1: content.load
    let posts = stages::load::load_posts_from_db(db_posts, config);
    tracing::info!("从数据库加载了 {} 篇文章", posts.len());

    if let Some(ref eng) = engine {
        let load_ctx = serde_json::json!({
            "project_root": project_root.to_string_lossy(),
            "posts": serialize_posts(&posts),
        });
        eng.hooks.call_action(&eng.lua, "after_load", &load_ctx)?;
    }

    // 阶段 3: taxonomy.build
    let taxonomy = stages::taxonomy::build_taxonomy(&posts, config);
    tracing::info!(
        "分类索引：{} 个标签，{} 个分类，{} 个月份归档",
        taxonomy.tags.len(),
        taxonomy.categories.len(),
        taxonomy.archives.len()
    );

    if let Some(ref eng) = engine {
        let taxonomy_ctx = serde_json::json!({
            "project_root": project_root.to_string_lossy(),
            "output_dir": &output_dir_str,
            "tag_count": taxonomy.tags.len(),
            "category_count": taxonomy.categories.len(),
            "archive_count": taxonomy.archives.len(),
        });
        eng.hooks.call_action(&eng.lua, "after_taxonomy", &taxonomy_ctx)?;
    }

    // 阶段 4: page.generate
    let pages = stages::generate::generate_pages(&posts, &taxonomy, config);
    let total_pages = pages.len();
    tracing::info!("生成了 {} 个页面", total_pages);

    // 重建依赖图
    bctx.dep_graph.clear();
    for page in &pages {
        bctx.dep_graph.add_dependency(&page.template, &page.url);
    }

    // 阶段 5: page.render - 根据 rebuild_urls 过滤
    let (pages_to_render, cached) = match rebuild_urls {
        Some(ref urls) => {
            let filtered: Vec<_> = pages.iter().filter(|p| urls.contains(&p.url)).collect();
            let rebuilt = filtered.len();
            let cached = total_pages - rebuilt;
            tracing::info!(
                "增量渲染：{} 个页面需重建，{} 个页面缓存跳过",
                rebuilt, cached
            );
            (filtered, cached)
        }
        None => {
            let all: Vec<_> = pages.iter().collect();
            (all, 0)
        }
    };

    let rebuilt = pages_to_render.len();
    stages::render::render_pages(project_root, config, &pages_to_render, bctx.theme_saved_config, bctx.site_settings)?;

    if let Some(ref eng) = engine {
        let render_ctx = serde_json::json!({
            "project_root": project_root.to_string_lossy(),
            "output_dir": &output_dir_str,
        });
        eng.hooks.call_action(&eng.lua, "after_render", &render_ctx)?;
    }

    // 阶段 6: asset.process
    stages::assets::process_assets(project_root, config, bctx.theme_saved_config)?;

    if let Some(ref eng) = engine {
        let assets_ctx = serde_json::json!({
            "project_root": project_root.to_string_lossy(),
            "output_dir": &output_dir_str,
        });
        eng.hooks.call_action(&eng.lua, "after_assets", &assets_ctx)?;
    }

    // 阶段 7: build.finalize
    stages::finalize::finalize(project_root, config, &posts)?;

    if let Some(ref eng) = engine {
        let finalize_ctx = serde_json::json!({
            "project_root": project_root.to_string_lossy(),
            "output_dir": &output_dir_str,
            "posts": serialize_posts(&posts),
            "site_url": &config.site.url,
        });
        eng.hooks
            .call_action(&eng.lua, "after_finalize", &finalize_ctx)?;
    }

    Ok(BuildStats {
        total_pages,
        rebuilt,
        cached,
    })
}
