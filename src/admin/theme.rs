use axum::extract::{Form, State};
use axum::response::{Html, Redirect};
use serde::Deserialize;
use sqlx::Row;
use std::collections::HashMap;

use crate::admin::layout::{admin_page, admin_page_with_script, html_escape, PageContext};
use crate::state::AppState;
use crate::theme::config::{self, ConfigField};

#[derive(Deserialize)]
pub struct SwitchForm {
    pub theme_name: String,
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
        Some(d) if !d.is_empty() => format!(r#"<div class="form-hint">{}</div>"#, html_escape(d)),
        _ => String::new(),
    };

    let input_html = match field.field_type.as_str() {
        "color" => {
            format!(r#"<input type="color" class="form-input" name="{key}" value="{val_str}">"#)
        }
        "boolean" => {
            let checked = match value {
                serde_json::Value::Bool(true) => " checked",
                serde_json::Value::String(s) if s == "true" => " checked",
                _ => "",
            };
            format!(
                r#"<label class="form-check">
                    <input type="checkbox" name="{key}" value="true"{checked}>
                    {label}
                </label>"#
            )
        }
        "number" => {
            let min_attr = field.min.map(|m| format!(r#" min="{m}""#)).unwrap_or_default();
            let max_attr = field.max.map(|m| format!(r#" max="{m}""#)).unwrap_or_default();
            format!(r#"<input type="number" class="form-input" name="{key}" value="{val_str}"{min_attr}{max_attr}>"#)
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
            format!(r#"<select class="form-select" name="{key}">{opts}</select>"#)
        }
        "textarea" => {
            format!(r#"<textarea class="form-textarea" name="{key}">{val_str}</textarea>"#)
        }
        "richtext" => {
            format!(
                r#"<textarea class="form-textarea" name="{key}" placeholder="支持 HTML">{val_str}</textarea>"#
            )
        }
        "code" => {
            let lang = field.language.as_deref().unwrap_or("");
            format!(
                r#"<textarea class="form-textarea code" name="{key}" placeholder="{lang}">{val_str}</textarea>"#,
                lang = html_escape(lang),
            )
        }
        "image" => {
            format!(r#"<input type="text" class="form-input" name="{key}" value="{val_str}" placeholder="图片 URL">"#)
        }
        _ => {
            format!(r#"<input type="text" class="form-input" name="{key}" value="{val_str}">"#)
        }
    };

    if field.field_type == "boolean" {
        format!(
            r#"<div class="form-group"{depends_attr}>{input_html}{desc_html}</div>"#
        )
    } else {
        format!(
            r#"<div class="form-group"{depends_attr}><label class="form-label">{label}</label>{input_html}{desc_html}</div>"#
        )
    }
}

pub async fn theme_settings(State(state): State<AppState>) -> Html<String> {
    let active_theme = &state.config.theme.active;

    let ctx = PageContext {
        site_title: crate::admin::settings::get_site_title(&state).await,
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    let resolved = match config::resolve_theme(&state.project_root, active_theme) {
        Ok(r) => r,
        Err(e) => {
            let body = format!(
                r#"<div class="page-header"><h1 class="page-title">主题设置</h1></div>
                <div class="alert alert-error">加载主题配置失败：{err}</div>"#,
                err = html_escape(&e.to_string()),
            );
            return Html(admin_page("主题设置", "/admin/theme", &body, &ctx));
        }
    };

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

    let mut form_html = String::new();
    for (group_name, fields) in &groups {
        form_html.push_str(&format!(
            r#"<div class="card" style="margin-bottom:16px;">
            <div class="card-header"><span class="card-title">{}</span></div>
            <div class="card-body">"#,
            html_escape(group_name)
        ));
        for field in fields {
            let val = values
                .get(&field.key)
                .cloned()
                .unwrap_or(serde_json::Value::String(String::new()));
            form_html.push_str(&render_field(field, &val));
        }
        form_html.push_str("</div></div>");
    }

    let all_themes = config::list_themes(&state.project_root).unwrap_or_default();
    let mut theme_list_html = String::new();
    for theme_name in &all_themes {
        let is_active = theme_name == active_theme;

        let meta_html = match config::load_theme_toml(&state.project_root, theme_name) {
            Ok(toml) => {
                let ver = if toml.theme.version.is_empty() {
                    String::new()
                } else {
                    format!(
                        r#" <span class="badge badge-neutral">v{}</span>"#,
                        html_escape(&toml.theme.version)
                    )
                };
                let desc = if toml.theme.description.is_empty() {
                    String::new()
                } else {
                    format!(
                        r#"<p class="form-hint">{}</p>"#,
                        html_escape(&toml.theme.description)
                    )
                };
                format!(
                    r#"<div><strong>{name}</strong>{ver}{desc}</div>"#,
                    name = html_escape(theme_name),
                )
            }
            Err(_) => format!(
                r#"<div><strong>{}</strong></div>"#,
                html_escape(theme_name),
            ),
        };

        let action_html = if is_active {
            r#"<span class="badge badge-success">当前主题</span>"#.to_string()
        } else {
            format!(
                r#"<form method="POST" action="/admin/theme/switch">
                    <input type="hidden" name="theme_name" value="{name}">
                    <button type="submit" class="btn btn-primary btn-sm">切换</button>
                </form>"#,
                name = html_escape(theme_name),
            )
        };

        theme_list_html.push_str(&format!(
            r#"<div class="card" style="margin-bottom:12px;">
                <div class="card-body" style="display:flex;justify-content:space-between;align-items:center;">
                    {meta_html}{action_html}
                </div>
            </div>"#,
        ));
    }

    let version_badge = if resolved.meta.version.is_empty() {
        String::new()
    } else {
        format!(
            r#" <span class="badge badge-info">v{}</span>"#,
            html_escape(&resolved.meta.version)
        )
    };

    let body = format!(
        r#"<div class="page-header">
            <h1 class="page-title">主题设置</h1>
        </div>
        <p class="form-hint" style="margin-bottom:20px;">当前主题：<strong>{theme_name}</strong>{version_badge}</p>

        <form method="POST" action="/admin/theme">
            {form_html}
            <button type="submit" class="btn btn-primary">保存配置</button>
        </form>

        <h2 class="page-title" style="font-size:18px;margin-top:32px;margin-bottom:12px;">已安装主题</h2>
        {theme_list_html}"#,
        theme_name = html_escape(active_theme),
        version_badge = version_badge,
        form_html = form_html,
        theme_list_html = theme_list_html,
    );

    let depends_script = r#"
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
    "#;

    Html(admin_page_with_script("主题设置", "/admin/theme", &body, depends_script, &ctx))
}

pub async fn save_theme_settings(
    State(state): State<AppState>,
    Form(form): Form<HashMap<String, String>>,
) -> Redirect {
    let active_theme = &state.config.theme.active;

    let schema = config::resolve_theme(&state.project_root, active_theme)
        .map(|r| r.config_schema)
        .unwrap_or_default();

    let mut values = serde_json::Map::new();

    for field in &schema {
        if field.field_type == "boolean" {
            let checked = form.contains_key(&field.key);
            values.insert(field.key.clone(), serde_json::Value::Bool(checked));
        } else if let Some(v) = form.get(&field.key) {
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

        let final_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
            new_content + "\n"
        } else {
            new_content
        };

        let _ = std::fs::write(&config_path, final_content);
    }

    Redirect::to("/admin/theme")
}
