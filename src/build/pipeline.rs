use crate::build::stages;
use crate::config::SiteConfig;
use anyhow::Result;
use std::path::Path;

/// 执行完整构建管道
pub fn execute(project_root: &Path, config: &SiteConfig) -> Result<()> {
    tracing::info!("开始构建...");
    let start = std::time::Instant::now();

    // 初始化插件引擎（仅在有启用插件时）
    let engine = if !config.plugins.enabled.is_empty() {
        let ordered = crate::plugin::scheduler::resolve_load_order(
            project_root,
            &config.plugins.enabled,
        )?;
        let mut eng = crate::lua::runtime::PluginEngine::new(project_root, config)?;
        eng.load_plugins(&ordered)?;
        Some(eng)
    } else {
        None
    };

    let ctx = serde_json::json!({
        "project_root": project_root.to_string_lossy(),
    });

    // 阶段 1: content.load - 加载内容文件
    let posts = stages::load::load_posts(project_root, config)?;
    tracing::info!("加载了 {} 篇文章", posts.len());

    if let Some(ref eng) = engine {
        eng.hooks.call_action(&eng.lua, "after_load", &ctx)?;
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
    tracing::info!("生成了 {} 个页面", pages.len());

    // 阶段 5: page.render - 渲染所有页面
    stages::render::render_pages(project_root, config, &pages)?;

    if let Some(ref eng) = engine {
        eng.hooks.call_action(&eng.lua, "after_render", &ctx)?;
    }

    // 阶段 6: asset.process - 编译 SCSS、复制主题资源
    stages::assets::process_assets(project_root, config)?;

    if let Some(ref eng) = engine {
        eng.hooks.call_action(&eng.lua, "after_assets", &ctx)?;
    }

    // 阶段 7: build.finalize - 收尾工作
    stages::finalize::finalize(project_root, config, &posts)?;

    if let Some(ref eng) = engine {
        eng.hooks.call_action(&eng.lua, "after_finalize", &ctx)?;
    }

    let duration = start.elapsed();
    tracing::info!("构建完成，耗时 {:.2}s", duration.as_secs_f64());

    Ok(())
}
