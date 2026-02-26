use axum::extract::{Form, Path, State};
use axum::response::{Html, Redirect};
use std::collections::HashMap;

use crate::plugin::registry::{list_available_plugins, load_plugin_info};
use crate::plugin::store::PluginStore;
use crate::state::AppState;

fn admin_nav() -> String {
    r#"<nav style="background:#1a1a2e;padding:12px 24px;display:flex;gap:24px;align-items:center;">
        <a href="/admin" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">仪表盘</a>
        <a href="/admin/posts" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">文章</a>
        <a href="/admin/pages" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">页面</a>
        <a href="/admin/media" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">媒体</a>
        <a href="/admin/theme" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">主题</a>
        <a href="/admin/plugins" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">插件</a>
    </nav>"#
        .to_string()
}

fn page_style() -> &'static str {
    r#"<style>
        * { margin:0; padding:0; box-sizing:border-box; }
        body { font-family:system-ui,-apple-system,sans-serif; background:#f5f5f5; color:#333; }
        .container { max-width:1000px; margin:24px auto; padding:0 16px; }
        h1 { margin-bottom:16px; }
        h2 { margin-top:24px; margin-bottom:12px; }
        a { color:#4a6cf7; text-decoration:none; }
        a:hover { text-decoration:underline; }
        .btn { display:inline-block; padding:6px 14px; border-radius:4px; border:none; cursor:pointer; font-size:14px; text-decoration:none; }
        .btn-primary { background:#4a6cf7; color:#fff; }
        .btn-danger { background:#e74c3c; color:#fff; }
        .btn-secondary { background:#6c757d; color:#fff; }
        .plugin-card { background:#fff; border:1px solid #ddd; border-radius:6px; padding:16px; margin-bottom:12px; display:flex; justify-content:space-between; align-items:center; }
        .plugin-card.enabled { border-color:#4a6cf7; border-width:2px; }
        .plugin-name { font-weight:600; font-size:16px; }
        .plugin-version { color:#888; font-size:13px; margin-left:8px; }
        .plugin-desc { color:#666; font-size:13px; margin-top:4px; }
        .badge { display:inline-block; padding:2px 8px; border-radius:10px; font-size:12px; }
        .badge-enabled { background:#e8f5e9; color:#2e7d32; }
        .badge-disabled { background:#fafafa; color:#999; }
        .actions { display:flex; gap:8px; align-items:center; }
        label { display:block; margin-bottom:4px; font-weight:500; }
        input[type=text], textarea {
            width:100%; padding:8px 10px; border:1px solid #ccc; border-radius:4px; font-size:14px; margin-bottom:12px;
        }
        textarea { min-height:80px; }
        .form-row { margin-bottom:8px; }
        .detail-section { background:#fff; border:1px solid #ddd; border-radius:6px; padding:16px; margin-bottom:16px; }
        .cap-list { display:flex; gap:8px; flex-wrap:wrap; margin-top:4px; }
        .cap-tag { background:#eef; padding:2px 10px; border-radius:12px; font-size:13px; color:#4a6cf7; }
        table { width:100%; border-collapse:collapse; background:#fff; border-radius:4px; overflow:hidden; box-shadow:0 1px 3px rgba(0,0,0,0.1); }
        th,td { padding:10px 14px; text-align:left; border-bottom:1px solid #eee; }
        th { background:#f8f8f8; font-weight:600; }
    </style>"#
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// GET /admin/plugins — 插件列表页面
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
        let card_class = if is_enabled {
            "plugin-card enabled"
        } else {
            "plugin-card"
        };

        let badge = if is_enabled {
            r#"<span class="badge badge-enabled">已启用</span>"#
        } else {
            r#"<span class="badge badge-disabled">未启用</span>"#
        };

        let toggle_label = if is_enabled { "禁用" } else { "启用" };
        let toggle_btn_class = if is_enabled {
            "btn btn-danger"
        } else {
            "btn btn-primary"
        };

        let settings_link = if is_enabled {
            format!(
                r#"<a href="/admin/plugins/{name}" class="btn btn-secondary">设置</a>"#,
                name = html_escape(name),
            )
        } else {
            String::new()
        };

        let version_html = if version.is_empty() {
            String::new()
        } else {
            format!(
                r#"<span class="plugin-version">v{}</span>"#,
                html_escape(&version)
            )
        };

        let desc_html = if description.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="plugin-desc">{}</div>"#,
                html_escape(&description)
            )
        };

        cards_html.push_str(&format!(
            r#"<div class="{card_class}">
                <div>
                    <div><span class="plugin-name">{name}</span>{version_html} {badge}</div>
                    {desc_html}
                </div>
                <div class="actions">
                    <form method="POST" action="/admin/plugins/toggle" style="margin:0;">
                        <input type="hidden" name="plugin_name" value="{name_escaped}">
                        <button type="submit" class="{toggle_btn_class}">{toggle_label}</button>
                    </form>
                    {settings_link}
                </div>
            </div>"#,
            card_class = card_class,
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
        cards_html = r#"<p style="color:#888;">暂无可用插件。将插件放入 plugins/ 目录即可。</p>"#
            .to_string();
    }

    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>插件管理 - cblog</title>{style}</head>
        <body>{nav}
        <div class="container">
            <h1>插件管理</h1>
            <p style="margin-bottom:20px;color:#666;">管理已安装的插件，启用或禁用插件功能。</p>
            {cards_html}
        </div>
        </body></html>"#,
        style = page_style(),
        nav = admin_nav(),
        cards_html = cards_html,
    );

    Html(html)
}

/// POST /admin/plugins/toggle — 切换插件启用状态
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

    // 构建新的 enabled 列表字符串
    let enabled_str = if current_enabled.is_empty() {
        "enabled = []".to_string()
    } else {
        let items: Vec<String> = current_enabled
            .iter()
            .map(|n| format!(r#""{}""#, n))
            .collect();
        format!("enabled = [{}]", items.join(", "))
    };

    // 行级替换 [plugins] 段中的 enabled 字段
    let mut in_plugins_section = false;
    let mut replaced = false;
    let new_content: String = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_plugins_section = trimmed == "[plugins]";
            }
            if in_plugins_section && !replaced && trimmed.starts_with("enabled") {
                if trimmed.contains('=') {
                    replaced = true;
                    return enabled_str.clone();
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 如果文件中没有 [plugins] 段或 enabled 字段，追加到文件末尾
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

/// GET /admin/plugins/{name} — 插件详情页
pub async fn plugin_detail(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Html<String> {
    let toml_path = state
        .project_root
        .join("plugins")
        .join(&name)
        .join("plugin.toml");

    let info = match load_plugin_info(&toml_path) {
        Ok(info) => info,
        Err(e) => {
            return Html(format!(
                r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>插件详情</title>{style}</head>
                <body>{nav}<div class="container"><h1>插件详情</h1>
                <p style="color:#e74c3c;">加载插件信息失败：{err}</p>
                <a href="/admin/plugins">返回插件列表</a></div></body></html>"#,
                style = page_style(),
                nav = admin_nav(),
                err = html_escape(&e.to_string()),
            ));
        }
    };

    let is_enabled = state.config.plugins.enabled.contains(&name);

    // 基本信息
    let version_html = if info.version.is_empty() {
        String::new()
    } else {
        format!(
            r#"<span class="plugin-version">v{}</span>"#,
            html_escape(&info.version)
        )
    };

    let status_badge = if is_enabled {
        r#"<span class="badge badge-enabled">已启用</span>"#
    } else {
        r#"<span class="badge badge-disabled">未启用</span>"#
    };

    // Capabilities
    fn render_cap_list(label: &str, items: &[String]) -> String {
        if items.is_empty() {
            return format!(
                r#"<div style="margin-bottom:8px;"><strong>{}</strong> <span style="color:#999;">无</span></div>"#,
                label
            );
        }
        let tags: String = items
            .iter()
            .map(|item| {
                format!(
                    r#"<span class="cap-tag">{}</span>"#,
                    item.replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;")
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<div style="margin-bottom:8px;"><strong>{}</strong><div class="cap-list">{}</div></div>"#,
            label, tags
        )
    }

    let caps_html = format!(
        "{}{}{}",
        render_cap_list("Reads:", &info.capabilities.reads),
        render_cap_list("Writes:", &info.capabilities.writes),
        render_cap_list("Generates:", &info.capabilities.generates),
    );

    // Dependencies
    let deps_html = format!(
        "{}{}",
        render_cap_list("After:", &info.dependencies.after),
        render_cap_list("Conflicts:", &info.dependencies.conflicts),
    );

    // 从 PluginStore 读取配置
    let store_data = PluginStore::get_all(&state.db, &name).await.unwrap_or_default();

    let config_html = if store_data.is_empty() {
        r#"<p style="color:#888;">此插件暂无配置数据。</p>"#.to_string()
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
                r#"<div class="form-row">
                    <label>{key}</label>
                    <input type="text" name="{key}" value="{val}">
                </div>"#,
                key = html_escape(key),
                val = html_escape(&val_str),
            ));
        }

        format!(
            r#"<form method="POST" action="/admin/plugins/{name}/config">
                {form_fields}
                <div style="margin-top:12px;">
                    <button type="submit" class="btn btn-primary">保存配置</button>
                </div>
            </form>"#,
            name = html_escape(&name),
            form_fields = form_fields,
        )
    };

    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>{title} - 插件详情</title>{style}</head>
        <body>{nav}
        <div class="container">
            <p style="margin-bottom:12px;"><a href="/admin/plugins">&larr; 返回插件列表</a></p>
            <h1><span class="plugin-name">{name}</span>{version_html} {status_badge}</h1>
            <p style="color:#666;margin-bottom:20px;">{desc}</p>

            <div class="detail-section">
                <h2>能力声明</h2>
                {caps_html}
            </div>

            <div class="detail-section">
                <h2>依赖关系</h2>
                {deps_html}
            </div>

            <div class="detail-section">
                <h2>插件配置</h2>
                {config_html}
            </div>
        </div>
        </body></html>"#,
        title = html_escape(&name),
        style = page_style(),
        nav = admin_nav(),
        name = html_escape(&name),
        version_html = version_html,
        status_badge = status_badge,
        desc = html_escape(&info.description),
        caps_html = caps_html,
        deps_html = deps_html,
        config_html = config_html,
    );

    Html(html)
}

/// POST /admin/plugins/{name}/config — 保存插件配置
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
