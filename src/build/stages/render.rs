use crate::build::stages::generate::RenderPage;
use crate::cbtml;
use crate::config::SiteConfig;
use anyhow::Result;
use minijinja::Environment;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// 渲染所有页面到 public/ 目录
pub fn render_pages(
    project_root: &Path,
    config: &SiteConfig,
    pages: &[RenderPage],
    theme_config: &HashMap<String, serde_json::Value>,
) -> Result<()> {
    let output_dir = project_root.join(&config.build.output_dir);
    let themes_dir = project_root.join("themes");
    let active_theme = &config.theme.active;

    // 预编译所有 cbtml 模板为 MiniJinja 模板字符串
    let compiled_templates = compile_all_templates(&themes_dir, active_theme)?;

    let mut env = Environment::new();
    cbtml::filters::register_filters(&mut env, &config.site.url);

    // 将编译后的模板逐个添加到环境中
    for (name, source) in &compiled_templates {
        env.add_template_owned(name.clone(), source.clone())?;
    }

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

    let rendered_count = AtomicUsize::new(0);
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());

    pages.par_iter().for_each(|page| {
        let template_name = format!("{}.cbtml", page.template);
        let tmpl = match env.get_template(&template_name) {
            Ok(t) => t,
            Err(_) => {
                tracing::warn!("模板 {} 不存在，跳过页面 {}", template_name, page.url);
                return;
            }
        };

        let mut ctx = page.context.as_object().cloned().unwrap_or_default();
        ctx.insert("site".into(), site_ctx.clone());
        let config_obj: serde_json::Map<String, serde_json::Value> =
            theme_config.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        ctx.insert("config".into(), serde_json::Value::Object(config_obj));
        let ctx_value = minijinja::Value::from_serialize(&ctx);

        let html = match tmpl.render(ctx_value) {
            Ok(html) => html,
            Err(e) => {
                let msg = format!(
                    "渲染页面 {} 失败（模板：{}）：\n{}",
                    page.url, template_name, e
                );
                tracing::error!("{}", msg);
                errors.lock().unwrap().push(msg);
                return;
            }
        };

        let file_path = if page.url.ends_with('/') {
            output_dir
                .join(page.url.trim_start_matches('/'))
                .join("index.html")
        } else {
            output_dir.join(page.url.trim_start_matches('/'))
        };

        if let Some(parent) = file_path.parent()
            && let Err(e) = std::fs::create_dir_all(parent) {
                errors
                    .lock()
                    .unwrap()
                    .push(format!("创建目录失败 {}: {}", parent.display(), e));
                return;
            }
        if let Err(e) = std::fs::write(&file_path, html) {
            errors
                .lock()
                .unwrap()
                .push(format!("写入文件失败 {}: {}", file_path.display(), e));
            return;
        }

        rendered_count.fetch_add(1, Ordering::Relaxed);
        tracing::debug!("已写入：{}", file_path.display());
    });

    let errs = errors.into_inner().unwrap();
    if !errs.is_empty() {
        tracing::warn!("渲染过程中有 {} 个错误", errs.len());
    }

    tracing::info!(
        "渲染完成，共 {} 个页面（成功 {}）",
        pages.len(),
        rendered_count.load(Ordering::Relaxed)
    );
    Ok(())
}

/// 编译所有主题的模板。当前主题的模板以 `name.cbtml` 注册，
/// 所有主题的模板额外以 `theme_name/name.cbtml` 注册以支持跨主题继承。
fn compile_all_templates(
    themes_dir: &Path,
    active_theme: &str,
) -> Result<HashMap<String, String>> {
    let mut templates = HashMap::new();

    if !themes_dir.exists() {
        tracing::warn!("主题目录不存在：{}", themes_dir.display());
        return Ok(templates);
    }

    for entry in std::fs::read_dir(themes_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let theme_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let template_dir = path.join("templates");
        if !template_dir.exists() {
            continue;
        }

        let mut theme_templates = HashMap::new();
        collect_templates(&template_dir, &template_dir, &mut theme_templates)?;

        for (rel_name, compiled) in theme_templates {
            // 以 theme_name/template.cbtml 格式注册，供跨主题继承使用
            let namespaced = format!("{}/{}", theme_name, rel_name);
            templates.insert(namespaced, compiled.clone());

            // 当前活跃主题的模板同时以短名注册，作为默认模板
            if theme_name == active_theme {
                templates.insert(rel_name, compiled);
            }
        }
    }

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
            let compiled = cbtml::compile(&source, &template_name)
                .map_err(|e| e.context(format!("编译模板 {} 失败", template_name)))?;

            templates.insert(template_name, compiled);
        }
    }
    Ok(())
}
