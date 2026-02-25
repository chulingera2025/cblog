pub mod pipeline;
pub mod stages;

use crate::config::SiteConfig;
use anyhow::Result;
use std::path::Path;

pub fn run(project_root: &Path, config: &SiteConfig, clean: bool) -> Result<()> {
    let output_dir = project_root.join(&config.build.output_dir);

    if clean {
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

    pipeline::execute(project_root, config)?;

    Ok(())
}
