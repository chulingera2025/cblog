use axum::extract::{Form, Path, State};
use axum::response::{Html, Redirect};
use minijinja::context;
use std::collections::HashMap;

use crate::admin::template::render_admin;
use crate::plugin::registry::{list_available_plugins, load_plugin_info, load_plugin_toml};
use crate::plugin::store::PluginStore;
use crate::state::AppState;

pub async fn list_plugins(State(state): State<AppState>) -> Html<String> {
    let available = list_available_plugins(&state.project_root).unwrap_or_default();
    let enabled = state.enabled_plugins.read().await;

    let plugins: Vec<minijinja::Value> = available
        .iter()
        .map(|name| {
            let toml_path = state
                .project_root
                .join("plugins")
                .join(name)
                .join("plugin.toml");

            let (version, description) = match load_plugin_info(&toml_path) {
                Ok(info) => {
                    let ver = if info.version.is_empty() {
                        None
                    } else {
                        Some(info.version)
                    };
                    let desc = if info.description.is_empty() {
                        None
                    } else {
                        Some(info.description)
                    };
                    (ver, desc)
                }
                Err(_) => (None, None),
            };

            let is_enabled = enabled.contains(name);

            let admin_pages: Vec<minijinja::Value> = load_plugin_toml(&toml_path)
                .map(|toml| {
                    toml.admin
                        .pages
                        .iter()
                        .map(|p| {
                            context! {
                                label => &p.label,
                                href => format!("/admin/ext/{}/{}", name, p.slug),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            context! {
                name => name,
                version => version,
                description => description,
                is_enabled => is_enabled,
                admin_pages => admin_pages,
            }
        })
        .collect();

    let active_path = "/admin/plugins";
    let sidebar_groups = crate::admin::layout::sidebar_groups_value(active_path);
    let plugin_items =
        crate::admin::layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let ctx = context! {
        page_title => "插件管理",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        plugins => plugins,
    };

    let html = render_admin(&state.admin_env, "plugins/list.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    let mut current_enabled: Vec<String> = state.enabled_plugins.read().await.clone();
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
                && trimmed.contains('=')
            {
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

    // 同步更新内存中的启用列表
    let is_enabled = current_enabled.contains(plugin_name);
    *state.enabled_plugins.write().await = current_enabled;

    state.call_hook("after_plugin_toggle", &serde_json::json!({
        "plugin_name": plugin_name,
        "enabled": is_enabled
    })).await;
    state.reload_runtime_plugins().await;

    Redirect::to("/admin/plugins")
}

/// GET /admin/plugins/{name}
pub async fn plugin_detail(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Html<String> {
    let active_path = "/admin/plugins";
    let sidebar_groups = crate::admin::layout::sidebar_groups_value(active_path);
    let plugin_items =
        crate::admin::layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let toml_path = state
        .project_root
        .join("plugins")
        .join(&name)
        .join("plugin.toml");

    let info = match load_plugin_info(&toml_path) {
        Ok(info) => info,
        Err(e) => {
            let ctx = context! {
                page_title => "插件详情",
                site_title => &state.config.site.title,
                sidebar_groups => sidebar_groups,
                plugin_sidebar_items => plugin_items,
                profile_active => false,
                plugin_name => &name,
                error_message => format!("加载插件信息失败：{}", e),
            };
            let html = render_admin(&state.admin_env, "plugins/detail.cbtml", ctx)
                .unwrap_or_else(|e| format!("模板渲染失败: {e}"));
            return Html(html);
        }
    };

    // 加载 admin pages 声明
    let admin_pages: Vec<minijinja::Value> = load_plugin_toml(&toml_path)
        .map(|toml| {
            toml.admin
                .pages
                .iter()
                .map(|p| {
                    context! {
                        label => &p.label,
                        href => format!("/admin/ext/{}/{}", name, p.slug),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let is_enabled = state.enabled_plugins.read().await.contains(&name);

    let plugin_version = if info.version.is_empty() {
        None
    } else {
        Some(&info.version)
    };

    let store_data = PluginStore::get_all(&state.db, &name).await.unwrap_or_default();

    let config_fields: Vec<minijinja::Value> = {
        let mut sorted_keys: Vec<&String> = store_data.keys().collect();
        sorted_keys.sort();
        sorted_keys
            .iter()
            .map(|key| {
                let value = &store_data[*key];
                let val_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                context! {
                    key => key.as_str(),
                    value => val_str,
                }
            })
            .collect()
    };

    let ctx = context! {
        page_title => "插件详情",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        plugin_name => &name,
        plugin_version => plugin_version,
        plugin_description => &info.description,
        is_enabled => is_enabled,
        cap_reads => &info.capabilities.reads,
        cap_writes => &info.capabilities.writes,
        cap_generates => &info.capabilities.generates,
        dep_after => &info.dependencies.after,
        dep_conflicts => &info.dependencies.conflicts,
        admin_pages => admin_pages,
        config_fields => config_fields,
    };

    let html = render_admin(&state.admin_env, "plugins/detail.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}

/// POST /admin/plugins/{name}/config
pub async fn save_plugin_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: axum::http::HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Redirect {
    for (key, value) in &form {
        if key == "_csrf_token" {
            continue;
        }
        let json_value = serde_json::Value::String(value.clone());
        let _ = PluginStore::set(&state.db, &name, key, &json_value).await;
    }

    state.call_hook("after_plugin_config_save", &serde_json::json!({
        "plugin_name": name
    })).await;
    state.reload_runtime_plugins().await;

    // 优先返回来源页（如插件自定义设置页面），否则跳转到插件详情页
    if let Some(referer) = headers.get(axum::http::header::REFERER).and_then(|v| v.to_str().ok())
        && let Ok(uri) = referer.parse::<axum::http::Uri>()
    {
        let path = uri.path();
        if path.starts_with("/admin/") {
            return Redirect::to(path);
        }
    }

    Redirect::to(&format!("/admin/plugins/{}", name))
}
