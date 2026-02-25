use crate::build::stages::generate::RenderPage;
use crate::cbtml;
use crate::config::SiteConfig;
use anyhow::Result;
use minijinja::Environment;
use std::collections::HashMap;
use std::path::Path;

/// 渲染所有页面到 public/ 目录
pub fn render_pages(
    project_root: &Path,
    config: &SiteConfig,
    pages: &[RenderPage],
) -> Result<()> {
    let output_dir = project_root.join(&config.build.output_dir);
    let theme_dir = project_root
        .join("themes")
        .join(&config.theme.active)
        .join("templates");

    // 预编译所有 cbtml 模板为 MiniJinja 模板字符串
    let compiled_templates = compile_all_templates(&theme_dir)?;

    let mut env = Environment::new();
    cbtml::filters::register_filters(&mut env);

    // 使用 loader 加载编译后的模板
    let templates = compiled_templates.clone();
    env.set_loader(move |name| {
        Ok(templates.get(name).cloned())
    });

    let site_ctx = serde_json::json!({
        "title": config.site.title,
        "subtitle": config.site.subtitle,
        "description": config.site.description,
        "url": config.site.url,
        "language": config.site.language,
        "author": {
            "name": config.site.author.name,
            "email": config.site.author.email,
            "avatar": config.site.author.avatar,
            "bio": config.site.author.bio,
        },
    });

    for page in pages {
        let template_name = format!("{}.cbtml", page.template);
        let tmpl = match env.get_template(&template_name) {
            Ok(t) => t,
            Err(_) => {
                tracing::warn!("模板 {} 不存在，跳过页面 {}", template_name, page.url);
                continue;
            }
        };

        let mut ctx = page.context.as_object().cloned().unwrap_or_default();
        ctx.insert("site".into(), site_ctx.clone());
        let ctx_value = minijinja::Value::from_serialize(&ctx);

        let html = match tmpl.render(ctx_value) {
            Ok(html) => html,
            Err(e) => {
                tracing::error!("渲染页面 {} 失败：{}", page.url, e);
                continue;
            }
        };

        let file_path = if page.url.ends_with('/') {
            output_dir
                .join(page.url.trim_start_matches('/'))
                .join("index.html")
        } else {
            output_dir.join(page.url.trim_start_matches('/'))
        };

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, html)?;
        tracing::debug!("已写入：{}", file_path.display());
    }

    tracing::info!("渲染完成，共 {} 个页面", pages.len());
    Ok(())
}

/// 递归扫描主题模板目录，将所有 .cbtml 文件编译为 MiniJinja 模板字符串
fn compile_all_templates(theme_dir: &Path) -> Result<HashMap<String, String>> {
    let mut templates = HashMap::new();
    if !theme_dir.exists() {
        tracing::warn!("主题模板目录不存在：{}", theme_dir.display());
        return Ok(templates);
    }
    collect_templates(theme_dir, theme_dir, &mut templates)?;
    Ok(templates)
}

fn collect_templates(
    base_dir: &Path,
    current_dir: &Path,
    templates: &mut HashMap<String, String>,
) -> Result<()> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_templates(base_dir, &path, templates)?;
        } else if path.extension().is_some_and(|ext| ext == "cbtml") {
            let rel_path = path.strip_prefix(base_dir)?;
            let template_name = rel_path.to_string_lossy().to_string();

            let source = std::fs::read_to_string(&path)?;
            let compiled = cbtml::compile(&source, &template_name)?;

            templates.insert(template_name, compiled);
        }
    }
    Ok(())
}
