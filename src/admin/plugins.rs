use axum::extract::{Form, Path, State};
use axum::response::{Html, Redirect};
use std::collections::HashMap;

use crate::admin::layout::{admin_page, html_escape, svg_icon, PageContext};
use crate::plugin::registry::{list_available_plugins, load_plugin_info};
use crate::plugin::store::PluginStore;
use crate::state::AppState;

pub async fn list_plugins(State(state): State<AppState>) -> Html<String> {
    let available = list_available_plugins(&state.project_root).unwrap_or_default();
    let enabled = &state.config.plugins.enabled;

    let mut cards_html = String::new();

    for name in &available {
        let toml_path = state
            .project_root
            .join("plugins")
            .join(name)
            .join("plugin.toml");

        let (version, description) = match load_plugin_info(&toml_path) {
            Ok(info) => (info.version, info.description),
            Err(_) => (String::new(), String::new()),
        };

        let is_enabled = enabled.contains(name);

        let badge = if is_enabled {
            r#"<span class="badge badge-success">已启用</span>"#
        } else {
            r#"<span class="badge badge-neutral">未启用</span>"#
        };

        let toggle_label = if is_enabled { "禁用" } else { "启用" };
        let toggle_btn_class = if is_enabled {
            "btn btn-danger btn-sm"
        } else {
            "btn btn-primary btn-sm"
        };

        let settings_link = if is_enabled {
            format!(
                r#"<a href="/admin/plugins/{name}" class="btn btn-secondary btn-sm">设置</a>"#,
                name = html_escape(name),
            )
        } else {
            String::new()
        };

        let version_html = if version.is_empty() {
            String::new()
        } else {
            format!(
                r#" <span class="badge badge-neutral">v{}</span>"#,
                html_escape(&version)
            )
        };

        let desc_html = if description.is_empty() {
            String::new()
        } else {
            format!(
                r#"<p class="form-hint">{}</p>"#,
                html_escape(&description)
            )
        };

        cards_html.push_str(&format!(
            r#"<div class="card" style="margin-bottom:12px;">
                <div class="card-body" style="display:flex;justify-content:space-between;align-items:center;">
                    <div>
                        <div><strong>{name}</strong>{version_html} {badge}</div>
                        {desc_html}
                    </div>
                    <div class="actions">
                        <form method="POST" action="/admin/plugins/toggle">
                            <input type="hidden" name="plugin_name" value="{name_escaped}">
                            <button type="submit" class="{toggle_btn_class}">{toggle_label}</button>
                        </form>
                        {settings_link}
                    </div>
                </div>
            </div>"#,
            name = html_escape(name),
            version_html = version_html,
            badge = badge,
            desc_html = desc_html,
            name_escaped = html_escape(name),
            toggle_btn_class = toggle_btn_class,
            toggle_label = toggle_label,
            settings_link = settings_link,
        ));
    }

    if available.is_empty() {
        cards_html =
            r#"<div class="empty-state"><p>暂无可用插件。将插件放入 plugins/ 目录即可。</p></div>"#
                .to_string();
    }

    let body = format!(
        r#"<div class="page-header">
            <h1 class="page-title">插件管理</h1>
        </div>
        <p class="form-hint" style="margin-bottom:20px;">管理已安装的插件，启用或禁用插件功能。</p>
        {cards_html}"#,
        cards_html = cards_html,
    );

    let ctx = PageContext {
        site_title: crate::admin::settings::get_site_title(&state).await,
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    Html(admin_page("插件管理", "/admin/plugins", &body, &ctx))
}

/// POST /admin/plugins/toggle
pub async fn toggle_plugin(
    State(state): State<AppState>,
    Form(form): Form<HashMap<String, String>>,
) -> Redirect {
    let Some(plugin_name) = form.get("plugin_name") else {
        return Redirect::to("/admin/plugins");
    };

    let config_path = state.project_root.join("cblog.toml");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return Redirect::to("/admin/plugins");
    };

    let mut current_enabled: Vec<String> = state.config.plugins.enabled.clone();
    if let Some(pos) = current_enabled.iter().position(|n| n == plugin_name) {
        current_enabled.remove(pos);
    } else {
        current_enabled.push(plugin_name.clone());
    }

    let enabled_str = if current_enabled.is_empty() {
        "enabled = []".to_string()
    } else {
        let items: Vec<String> = current_enabled
            .iter()
            .map(|n| format!(r#""{}""#, n))
            .collect();
        format!("enabled = [{}]", items.join(", "))
    };

    let mut in_plugins_section = false;
    let mut replaced = false;
    let new_content: String = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_plugins_section = trimmed == "[plugins]";
            }
            if in_plugins_section && !replaced && trimmed.starts_with("enabled")
                && trimmed.contains('=') {
                    replaced = true;
                    return enabled_str.clone();
                }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    let final_content = if replaced {
        if content.ends_with('\n') && !new_content.ends_with('\n') {
            new_content + "\n"
        } else {
            new_content
        }
    } else {
        let mut c = new_content;
        if !c.ends_with('\n') {
            c.push('\n');
        }
        if !content.contains("[plugins]") {
            c.push_str("\n[plugins]\n");
        }
        c.push_str(&enabled_str);
        c.push('\n');
        c
    };

    let _ = std::fs::write(&config_path, final_content);

    Redirect::to("/admin/plugins")
}

/// GET /admin/plugins/{name}
pub async fn plugin_detail(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Html<String> {
    let ctx = PageContext {
        site_title: crate::admin::settings::get_site_title(&state).await,
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    let toml_path = state
        .project_root
        .join("plugins")
        .join(&name)
        .join("plugin.toml");

    let info = match load_plugin_info(&toml_path) {
        Ok(info) => info,
        Err(e) => {
            let body = format!(
                r#"<a href="/admin/plugins" class="page-back">{icon} 返回插件列表</a>
                <div class="page-header"><h1 class="page-title">插件详情</h1></div>
                <div class="alert alert-error">加载插件信息失败：{err}</div>"#,
                icon = svg_icon("arrow-left"),
                err = html_escape(&e.to_string()),
            );
            return Html(admin_page("插件详情", "/admin/plugins", &body, &ctx));
        }
    };

    let is_enabled = state.config.plugins.enabled.contains(&name);

    let version_html = if info.version.is_empty() {
        String::new()
    } else {
        format!(
            r#" <span class="badge badge-neutral">v{}</span>"#,
            html_escape(&info.version)
        )
    };

    let status_badge = if is_enabled {
        r#"<span class="badge badge-success">已启用</span>"#
    } else {
        r#"<span class="badge badge-neutral">未启用</span>"#
    };

    fn render_cap_list(label: &str, items: &[String]) -> String {
        if items.is_empty() {
            return format!(
                r#"<div class="form-group"><strong>{}</strong> <span class="form-hint">无</span></div>"#,
                label
            );
        }
        let tags: String = items
            .iter()
            .map(|item| {
                format!(
                    r#"<span class="badge badge-info">{}</span>"#,
                    item.replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;")
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            r#"<div class="form-group"><strong>{}</strong><div>{}</div></div>"#,
            label, tags
        )
    }

    let caps_html = format!(
        "{}{}{}",
        render_cap_list("Reads:", &info.capabilities.reads),
        render_cap_list("Writes:", &info.capabilities.writes),
        render_cap_list("Generates:", &info.capabilities.generates),
    );

    let deps_html = format!(
        "{}{}",
        render_cap_list("After:", &info.dependencies.after),
        render_cap_list("Conflicts:", &info.dependencies.conflicts),
    );

    let store_data = PluginStore::get_all(&state.db, &name).await.unwrap_or_default();

    let config_html = if store_data.is_empty() {
        r#"<p class="form-hint">此插件暂无配置数据。</p>"#.to_string()
    } else {
        let mut form_fields = String::new();
        let mut sorted_keys: Vec<&String> = store_data.keys().collect();
        sorted_keys.sort();

        for key in &sorted_keys {
            let value = &store_data[*key];
            let val_str = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            form_fields.push_str(&format!(
                r#"<div class="form-group">
                    <label class="form-label">{key}</label>
                    <input type="text" class="form-input" name="{key}" value="{val}">
                </div>"#,
                key = html_escape(key),
                val = html_escape(&val_str),
            ));
        }

        format!(
            r#"<form method="POST" action="/admin/plugins/{name}/config">
                {form_fields}
                <button type="submit" class="btn btn-primary">保存配置</button>
            </form>"#,
            name = html_escape(&name),
            form_fields = form_fields,
        )
    };

    let body = format!(
        r#"<a href="/admin/plugins" class="page-back">{back_icon} 返回插件列表</a>
        <div class="page-header">
            <h1 class="page-title">{name}{version_html} {status_badge}</h1>
        </div>
        <p class="form-hint" style="margin-bottom:20px;">{desc}</p>

        <div class="card" style="margin-bottom:16px;">
            <div class="card-header"><span class="card-title">能力声明</span></div>
            <div class="card-body">{caps_html}</div>
        </div>

        <div class="card" style="margin-bottom:16px;">
            <div class="card-header"><span class="card-title">依赖关系</span></div>
            <div class="card-body">{deps_html}</div>
        </div>

        <div class="card">
            <div class="card-header"><span class="card-title">插件配置</span></div>
            <div class="card-body">{config_html}</div>
        </div>"#,
        back_icon = svg_icon("arrow-left"),
        name = html_escape(&name),
        version_html = version_html,
        status_badge = status_badge,
        desc = html_escape(&info.description),
        caps_html = caps_html,
        deps_html = deps_html,
        config_html = config_html,
    );

    Html(admin_page("插件详情", "/admin/plugins", &body, &ctx))
}

/// POST /admin/plugins/{name}/config
pub async fn save_plugin_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Form(form): Form<HashMap<String, String>>,
) -> Redirect {
    for (key, value) in &form {
        let json_value = serde_json::Value::String(value.clone());
        let _ = PluginStore::set(&state.db, &name, key, &json_value).await;
    }

    Redirect::to(&format!("/admin/plugins/{}", name))
}
