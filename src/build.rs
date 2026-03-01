pub mod events;
pub mod graph;
pub mod incremental;
pub mod pipeline;
pub mod stages;

use crate::admin::settings::SiteSettings;
use crate::build::stages::load::DbPost;
use crate::config::SiteConfig;
use anyhow::Result;
use incremental::BuildStats;
use std::collections::HashMap;
use std::path::Path;

/// 构建运行参数
pub struct BuildParams {
    pub clean: bool,
    pub force: bool,
    pub plugin_configs: HashMap<String, HashMap<String, serde_json::Value>>,
    pub theme_saved_config: HashMap<String, serde_json::Value>,
    pub db_posts: Vec<DbPost>,
    pub site_settings: SiteSettings,
}

pub fn run(
    project_root: &Path,
    config: &SiteConfig,
    params: BuildParams,
) -> Result<BuildStats> {
    let output_dir = project_root.join(&config.build.output_dir);

    if params.clean {
        if output_dir.exists() {
            std::fs::remove_dir_all(&output_dir)?;
            tracing::info!("已清除输出目录：{}", output_dir.display());
        }
        let cache_dir = project_root.join(&config.build.cache_dir);
        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)?;
            tracing::info!("已清除缓存目录：{}", cache_dir.display());
        }
    }

    std::fs::create_dir_all(&output_dir)?;

    // clean 模式下缓存已被清除，等同于 force
    let force = params.force || params.clean;
    let stats = pipeline::execute(
        project_root,
        config,
        params.plugin_configs,
        params.theme_saved_config,
        params.db_posts,
        params.site_settings,
        force,
    )?;

    Ok(stats)
}
