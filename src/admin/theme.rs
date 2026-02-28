use axum::extract::{Form, State};
use axum::response::{Html, Redirect};
use minijinja::context;
use serde::Deserialize;
use sqlx::Row;
use std::collections::HashMap;

use crate::admin::template::render_admin;
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

fn is_checked(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::Bool(true))
        || matches!(value, serde_json::Value::String(s) if s == "true")
}

/// 将 ConfigField 转换为模板可消费的 minijinja::Value
fn field_to_ctx(field: &ConfigField, value: &serde_json::Value) -> minijinja::Value {
    let val_str = json_value_to_string(value);

    let options: Vec<minijinja::Value> = if field.field_type == "select"
        || field.field_type == "font_select"
    {
        config::extract_option_pairs(&field.options)
            .into_iter()
            .map(|(v, l)| {
                let selected = json_value_to_string(value) == v;
                context! {
                    value => v,
                    label => l,
                    selected => selected,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    context! {
        key => &field.key,
        label => &field.label,
        field_type => &field.field_type,
        value => val_str,
        checked => is_checked(value),
        depends_on => &field.depends_on,
        description => &field.description,
        min_attr => field.min.map(|m| m.to_string()),
        max_attr => field.max.map(|m| m.to_string()),
        language => field.language.as_deref().unwrap_or(""),
        options => options,
    }
}

pub async fn theme_settings(State(state): State<AppState>) -> Html<String> {
    let active_theme = &state.config.theme.active;
    let active_path = "/admin/theme";

    let sidebar_groups = crate::admin::layout::sidebar_groups_value(active_path);
    let plugin_items =
        crate::admin::layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let resolved = match config::resolve_theme(&state.project_root, active_theme) {
        Ok(r) => r,
        Err(e) => {
            let ctx = context! {
                page_title => "主题设置",
                site_title => &state.config.site.title,
                sidebar_groups => sidebar_groups,
                plugin_sidebar_items => plugin_items,
                profile_active => false,
                error_message => format!("加载主题配置失败：{}", e),
            };
            let html = render_admin(&state.admin_env, "theme.cbtml", ctx)
                .unwrap_or_else(|e| format!("模板渲染失败: {e}"));
            return Html(html);
        }
    };

    let saved: HashMap<String, serde_json::Value> =
        sqlx::query("SELECT config FROM theme_config WHERE theme_name = ?")
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

    // 按组分组字段
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

    let config_groups: Vec<minijinja::Value> = groups
        .iter()
        .map(|(group_name, fields)| {
            let field_values: Vec<minijinja::Value> = fields
                .iter()
                .map(|field| {
                    let val = values
                        .get(&field.key)
                        .cloned()
                        .unwrap_or(serde_json::Value::String(String::new()));
                    field_to_ctx(field, &val)
                })
                .collect();
            context! {
                name => group_name,
                fields => field_values,
            }
        })
        .collect();

    // 构建已安装主题列表
    let all_themes_list = config::list_themes(&state.project_root).unwrap_or_default();
    let all_themes: Vec<minijinja::Value> = all_themes_list
        .iter()
        .map(|theme_name| {
            let is_active = theme_name == active_theme;
            let (version, description) =
                match config::load_theme_toml(&state.project_root, theme_name) {
                    Ok(toml) => {
                        let ver = if toml.theme.version.is_empty() {
                            None
                        } else {
                            Some(toml.theme.version)
                        };
                        let desc = if toml.theme.description.is_empty() {
                            None
                        } else {
                            Some(toml.theme.description)
                        };
                        (ver, desc)
                    }
                    Err(_) => (None, None),
                };
            context! {
                name => theme_name,
                is_active => is_active,
                version => version,
                description => description,
            }
        })
        .collect();

    let theme_version = if resolved.meta.version.is_empty() {
        None
    } else {
        Some(&resolved.meta.version)
    };

    let ctx = context! {
        page_title => "主题设置",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        current_theme => active_theme,
        theme_version => theme_version,
        config_groups => config_groups,
        all_themes => all_themes,
    };

    let html = render_admin(&state.admin_env, "theme.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:save_theme").await;
    });

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

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:switch_theme").await;
    });

    Redirect::to("/admin/theme")
}
