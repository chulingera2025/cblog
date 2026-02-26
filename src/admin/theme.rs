use axum::extract::{Form, State};
use axum::response::{Html, Redirect};
use serde::Deserialize;
use sqlx::Row;
use std::collections::HashMap;

use crate::state::AppState;
use crate::theme::config::{self, ConfigField};

#[derive(Deserialize)]
pub struct SwitchForm {
    pub theme_name: String,
}

fn admin_nav() -> String {
    r#"<nav style="background:#1a1a2e;padding:12px 24px;display:flex;gap:24px;align-items:center;">
        <a href="/admin" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">仪表盘</a>
        <a href="/admin/posts" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">文章</a>
        <a href="/admin/pages" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">页面</a>
        <a href="/admin/media" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">媒体</a>
        <a href="/admin/theme" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">主题</a>
    </nav>"#
        .to_string()
}

fn page_style() -> &'static str {
    r#"<style>
        * { margin:0; padding:0; box-sizing:border-box; }
        body { font-family:system-ui,-apple-system,sans-serif; background:#f5f5f5; color:#333; }
        .container { max-width:1000px; margin:24px auto; padding:0 16px; }
        h1 { margin-bottom:16px; }
        table { width:100%; border-collapse:collapse; background:#fff; border-radius:4px; overflow:hidden; box-shadow:0 1px 3px rgba(0,0,0,0.1); }
        th,td { padding:10px 14px; text-align:left; border-bottom:1px solid #eee; }
        th { background:#f8f8f8; font-weight:600; }
        a { color:#4a6cf7; text-decoration:none; }
        a:hover { text-decoration:underline; }
        .btn { display:inline-block; padding:6px 14px; border-radius:4px; border:none; cursor:pointer; font-size:14px; text-decoration:none; }
        .btn-primary { background:#4a6cf7; color:#fff; }
        .btn-danger { background:#e74c3c; color:#fff; }
        .btn-secondary { background:#6c757d; color:#fff; }
        label { display:block; margin-bottom:4px; font-weight:500; }
        input[type=text], input[type=number], input[type=color], textarea, select {
            width:100%; padding:8px 10px; border:1px solid #ccc; border-radius:4px; font-size:14px; margin-bottom:12px;
        }
        textarea { min-height:120px; }
        .form-row { margin-bottom:8px; }
        fieldset { border:1px solid #ddd; border-radius:6px; padding:16px; margin-bottom:20px; background:#fff; }
        legend { font-weight:600; padding:0 8px; color:#4a6cf7; }
        .theme-card { background:#fff; border:1px solid #ddd; border-radius:6px; padding:16px; margin-bottom:12px; display:flex; justify-content:space-between; align-items:center; }
        .theme-card.active { border-color:#4a6cf7; border-width:2px; }
        .theme-name { font-weight:600; font-size:16px; }
        .theme-version { color:#888; font-size:13px; margin-left:8px; }
        .field-desc { font-size:12px; color:#888; margin-bottom:8px; margin-top:-8px; }
    </style>"#
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn json_value_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => val.to_string(),
    }
}

fn render_field(field: &ConfigField, value: &serde_json::Value) -> String {
    let key = html_escape(&field.key);
    let label = html_escape(&field.label);
    let val_str = html_escape(&json_value_to_string(value));

    let depends_attr = match &field.depends_on {
        Some(dep) => format!(r#" data-depends-on="{}""#, html_escape(dep)),
        None => String::new(),
    };

    let desc_html = match &field.description {
        Some(d) if !d.is_empty() => format!(r#"<div class="field-desc">{}</div>"#, html_escape(d)),
        _ => String::new(),
    };

    let input_html = match field.field_type.as_str() {
        "color" => {
            format!(r#"<input type="color" name="{key}" value="{val_str}">"#)
        }
        "boolean" => {
            let checked = match value {
                serde_json::Value::Bool(true) => " checked",
                serde_json::Value::String(s) if s == "true" => " checked",
                _ => "",
            };
            format!(
                r#"<label style="display:flex;align-items:center;gap:8px;font-weight:normal;cursor:pointer;">
                    <input type="checkbox" name="{key}" value="true"{checked} style="width:auto;margin:0;">
                    {label}
                </label>"#
            )
        }
        "number" => {
            let min_attr = field.min.map(|m| format!(r#" min="{m}""#)).unwrap_or_default();
            let max_attr = field.max.map(|m| format!(r#" max="{m}""#)).unwrap_or_default();
            format!(r#"<input type="number" name="{key}" value="{val_str}"{min_attr}{max_attr}>"#)
        }
        "select" | "font_select" => {
            let pairs = config::extract_option_pairs(&field.options);
            let mut opts = String::new();
            for (v, l) in &pairs {
                let ev = html_escape(v);
                let el = html_escape(l);
                let selected = if json_value_to_string(value) == *v { " selected" } else { "" };
                opts.push_str(&format!(r#"<option value="{ev}"{selected}>{el}</option>"#));
            }
            format!(r#"<select name="{key}">{opts}</select>"#)
        }
        "textarea" => {
            format!(r#"<textarea name="{key}">{val_str}</textarea>"#)
        }
        "richtext" => {
            format!(
                r#"<textarea name="{key}" placeholder="支持 HTML">{val_str}</textarea>"#
            )
        }
        "code" => {
            let lang = field.language.as_deref().unwrap_or("");
            format!(
                r#"<textarea name="{key}" style="font-family:monospace;" placeholder="{lang}">{val_str}</textarea>"#,
                lang = html_escape(lang),
            )
        }
        "image" => {
            format!(r#"<input type="text" name="{key}" value="{val_str}" placeholder="图片 URL">"#)
        }
        // text 及未知类型默认
        _ => {
            format!(r#"<input type="text" name="{key}" value="{val_str}">"#)
        }
    };

    // boolean 类型已自带 label
    if field.field_type == "boolean" {
        format!(
            r#"<div class="form-row"{depends_attr}>{input_html}{desc_html}</div>"#
        )
    } else {
        format!(
            r#"<div class="form-row"{depends_attr}><label>{label}</label>{input_html}{desc_html}</div>"#
        )
    }
}

pub async fn theme_settings(State(state): State<AppState>) -> Html<String> {
    let active_theme = &state.config.theme.active;

    // 解析主题配置 schema
    let resolved = match config::resolve_theme(&state.project_root, active_theme) {
        Ok(r) => r,
        Err(e) => {
            return Html(format!(
                r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>主题设置</title>{style}</head>
                <body>{nav}<div class="container"><h1>主题设置</h1>
                <p style="color:#e74c3c;">加载主题配置失败：{err}</p></div></body></html>"#,
                style = page_style(),
                nav = admin_nav(),
                err = html_escape(&e.to_string()),
            ));
        }
    };

    // 从数据库读取已保存的配置值
    let saved: HashMap<String, serde_json::Value> = sqlx::query(
        "SELECT config FROM theme_config WHERE theme_name = ?",
    )
    .bind(active_theme)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|row| {
        let json_str: String = row.get("config");
        serde_json::from_str(&json_str).ok()
    })
    .unwrap_or_default();

    let values = config::effective_values(&resolved.config_schema, &saved);

    // 按 group 分组
    let mut groups: Vec<(String, Vec<&ConfigField>)> = Vec::new();
    let mut group_index: HashMap<String, usize> = HashMap::new();

    for field in &resolved.config_schema {
        let group_name = if field.group.is_empty() {
            "通用".to_string()
        } else {
            field.group.clone()
        };
        if let Some(&idx) = group_index.get(&group_name) {
            groups[idx].1.push(field);
        } else {
            group_index.insert(group_name.clone(), groups.len());
            groups.push((group_name, vec![field]));
        }
    }

    // 生成分组表单
    let mut form_html = String::new();
    for (group_name, fields) in &groups {
        form_html.push_str(&format!(
            r#"<fieldset><legend>{}</legend>"#,
            html_escape(group_name)
        ));
        for field in fields {
            let val = values
                .get(&field.key)
                .cloned()
                .unwrap_or(serde_json::Value::String(String::new()));
            form_html.push_str(&render_field(field, &val));
        }
        form_html.push_str("</fieldset>");
    }

    // 列出所有主题
    let all_themes = config::list_themes(&state.project_root).unwrap_or_default();
    let mut theme_list_html = String::new();
    for theme_name in &all_themes {
        let is_active = theme_name == active_theme;
        let card_class = if is_active { "theme-card active" } else { "theme-card" };

        // 尝试加载主题元数据
        let meta_html = match config::load_theme_toml(&state.project_root, theme_name) {
            Ok(toml) => {
                let ver = if toml.theme.version.is_empty() {
                    String::new()
                } else {
                    format!(
                        r#"<span class="theme-version">v{}</span>"#,
                        html_escape(&toml.theme.version)
                    )
                };
                let desc = if toml.theme.description.is_empty() {
                    String::new()
                } else {
                    format!(
                        r#"<div style="color:#666;font-size:13px;margin-top:4px;">{}</div>"#,
                        html_escape(&toml.theme.description)
                    )
                };
                format!(
                    r#"<div><span class="theme-name">{name}</span>{ver}{desc}</div>"#,
                    name = html_escape(theme_name),
                )
            }
            Err(_) => format!(
                r#"<div><span class="theme-name">{}</span></div>"#,
                html_escape(theme_name),
            ),
        };

        let action_html = if is_active {
            r#"<span style="color:#4a6cf7;font-weight:600;">当前主题</span>"#.to_string()
        } else {
            format!(
                r#"<form method="POST" action="/admin/theme/switch" style="margin:0;">
                    <input type="hidden" name="theme_name" value="{name}">
                    <button type="submit" class="btn btn-primary">切换</button>
                </form>"#,
                name = html_escape(theme_name),
            )
        };

        theme_list_html.push_str(&format!(
            r#"<div class="{card_class}">{meta_html}{action_html}</div>"#
        ));
    }

    // depends_on 的 JS 实现
    let depends_js = r#"<script>
    document.addEventListener('DOMContentLoaded', function() {
        document.querySelectorAll('[data-depends-on]').forEach(function(el) {
            var depKey = el.getAttribute('data-depends-on');
            var checkbox = document.querySelector('input[name="' + depKey + '"][type="checkbox"]');
            if (!checkbox) return;
            function toggle() {
                el.style.display = checkbox.checked ? '' : 'none';
            }
            toggle();
            checkbox.addEventListener('change', toggle);
        });
    });
    </script>"#;

    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>主题设置</title>{style}</head>
        <body>{nav}
        <div class="container">
            <h1>主题设置</h1>
            <p style="margin-bottom:20px;color:#666;">当前主题：<strong>{theme_name}</strong>
            {version_badge}</p>

            <h2 style="margin-bottom:12px;">主题配置</h2>
            <form method="POST" action="/admin/theme">
                {form_html}
                <div style="margin-top:16px;">
                    <button type="submit" class="btn btn-primary">保存配置</button>
                </div>
            </form>

            <h2 style="margin-top:32px;margin-bottom:12px;">已安装主题</h2>
            {theme_list_html}
        </div>
        {depends_js}
        </body></html>"#,
        style = page_style(),
        nav = admin_nav(),
        theme_name = html_escape(active_theme),
        version_badge = if resolved.meta.version.is_empty() {
            String::new()
        } else {
            format!(
                r#"<span style="background:#eef;padding:2px 8px;border-radius:10px;font-size:12px;">v{}</span>"#,
                html_escape(&resolved.meta.version)
            )
        },
        form_html = form_html,
        theme_list_html = theme_list_html,
        depends_js = depends_js,
    );

    Html(html)
}

pub async fn save_theme_settings(
    State(state): State<AppState>,
    Form(form): Form<HashMap<String, String>>,
) -> Redirect {
    let active_theme = &state.config.theme.active;

    // 加载 schema 以识别 boolean 字段
    let schema = config::resolve_theme(&state.project_root, active_theme)
        .map(|r| r.config_schema)
        .unwrap_or_default();

    let mut values = serde_json::Map::new();

    for field in &schema {
        if field.field_type == "boolean" {
            let checked = form.contains_key(&field.key);
            values.insert(field.key.clone(), serde_json::Value::Bool(checked));
        } else if let Some(v) = form.get(&field.key) {
            // 数字类型尝试解析为数字
            if field.field_type == "number" {
                if let Ok(n) = v.parse::<i64>() {
                    values.insert(field.key.clone(), serde_json::json!(n));
                } else if let Ok(f) = v.parse::<f64>() {
                    values.insert(field.key.clone(), serde_json::json!(f));
                } else {
                    values.insert(
                        field.key.clone(),
                        serde_json::Value::String(v.clone()),
                    );
                }
            } else {
                values.insert(
                    field.key.clone(),
                    serde_json::Value::String(v.clone()),
                );
            }
        }
    }

    let json_str = serde_json::Value::Object(values).to_string();

    let _ = sqlx::query(
        "INSERT INTO theme_config (theme_name, config) VALUES (?, ?) \
         ON CONFLICT(theme_name) DO UPDATE SET config = excluded.config",
    )
    .bind(active_theme)
    .bind(&json_str)
    .execute(&state.db)
    .await;

    Redirect::to("/admin/theme")
}

pub async fn switch_theme(
    State(state): State<AppState>,
    Form(form): Form<SwitchForm>,
) -> Redirect {
    let theme_dir = state
        .project_root
        .join("themes")
        .join(&form.theme_name)
        .join("theme.toml");

    if !theme_dir.exists() {
        return Redirect::to("/admin/theme");
    }

    // 通过行替换修改 cblog.toml 中的 active 主题
    let config_path = state.project_root.join("cblog.toml");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        let mut in_theme_section = false;
        let mut replaced = false;
        let new_content: String = content
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_theme_section = trimmed == "[theme]";
                }
                if in_theme_section && !replaced && trimmed.starts_with("active")
                    && let Some(eq_pos) = trimmed.find('=') {
                        let _ = eq_pos;
                        replaced = true;
                        return format!(r#"active = "{}""#, form.theme_name);
                    }
                line.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // 保留原文件末尾的换行
        let final_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
            new_content + "\n"
        } else {
            new_content
        };

        let _ = std::fs::write(&config_path, final_content);
    }

    Redirect::to("/admin/theme")
}
