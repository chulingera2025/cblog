use crate::config::SiteConfig;
use crate::theme::config::{build_scss_overrides, default_values, resolve_theme};
use anyhow::{Context, Result};
use std::path::Path;

pub fn process_assets(project_root: &Path, config: &SiteConfig) -> Result<()> {
    let active = &config.theme.active;
    let theme_dir = project_root.join("themes").join(active);
    let output_dir = project_root.join(&config.build.output_dir);
    let assets_out = output_dir.join("assets");

    std::fs::create_dir_all(&assets_out)?;

    compile_scss(project_root, &theme_dir, &assets_out, active)?;
    copy_css(&theme_dir, &assets_out)?;
    copy_js(&theme_dir, &assets_out)?;
    copy_media(project_root, &output_dir)?;

    Ok(())
}

fn compile_scss(
    project_root: &Path,
    theme_dir: &Path,
    assets_out: &Path,
    theme_name: &str,
) -> Result<()> {
    let scss_dir = theme_dir.join("assets").join("scss");
    let main_scss = scss_dir.join("main.scss");

    if !main_scss.exists() {
        tracing::debug!("主题无 main.scss，跳过 SCSS 编译");
        return Ok(());
    }

    // 从主题配置 schema 提取默认值，生成 SCSS 变量覆盖
    let resolved = resolve_theme(project_root, theme_name)?;
    let values = default_values(&resolved.config_schema);
    let overrides = build_scss_overrides(&values);

    let source = std::fs::read_to_string(&main_scss)
        .with_context(|| format!("读取 main.scss 失败: {}", main_scss.display()))?;

    // 将覆盖变量前置到源码中，确保优先级高于 !default 声明
    let input = format!("{overrides}\n{source}");

    let options = grass::Options::default().load_path(&scss_dir);
    let css = grass::from_string(input, &options)
        .map_err(|e| anyhow::anyhow!("SCSS 编译失败: {e}"))?;

    std::fs::write(assets_out.join("main.css"), css)?;
    tracing::info!("已编译 main.scss → main.css");

    Ok(())
}

fn copy_css(theme_dir: &Path, assets_out: &Path) -> Result<()> {
    let css_dir = theme_dir.join("assets").join("css");
    if !css_dir.exists() {
        return Ok(());
    }
    copy_files_with_ext(&css_dir, assets_out, "css")
}

fn copy_js(theme_dir: &Path, assets_out: &Path) -> Result<()> {
    let js_dir = theme_dir.join("assets").join("js");
    if !js_dir.exists() {
        return Ok(());
    }
    copy_files_with_ext(&js_dir, assets_out, "js")
}

fn copy_files_with_ext(src_dir: &Path, dest_dir: &Path, ext: &str) -> Result<()> {
    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == ext) {
            if let Some(name) = path.file_name() {
                std::fs::copy(&path, dest_dir.join(name))?;
                tracing::debug!("已复制资源: {}", path.display());
            }
        }
    }
    Ok(())
}

fn copy_media(project_root: &Path, output_dir: &Path) -> Result<()> {
    let media_src = project_root.join("media");
    if !media_src.exists() {
        return Ok(());
    }
    let media_dest = output_dir.join("media");
    copy_dir_recursive(&media_src, &media_dest)?;
    tracing::info!("已复制 media 目录");
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}
