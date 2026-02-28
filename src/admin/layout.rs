// 后台管理共享布局工具

/// 插件侧边栏条目
#[derive(Clone)]
pub struct PluginSidebarEntry {
    pub label: String,
    pub href: String,
    pub icon: String,
}

/// RFC3339 时间格式化为 "YYYY-MM-DD HH:MM:SS"
pub fn format_datetime(s: &str) -> String {
    let s = &s[..19.min(s.len())];
    s.replace('T', " ")
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── 侧边栏数据 ──

fn is_active(current_path: &str, item_path: &str) -> bool {
    if item_path == "/admin" {
        current_path == "/admin"
    } else {
        current_path.starts_with(item_path)
    }
}

struct SidebarItem {
    label: &'static str,
    href: &'static str,
    icon: &'static str,
}

struct SidebarGroup {
    label: &'static str,
    items: &'static [SidebarItem],
}

const SIDEBAR_GROUPS: &[SidebarGroup] = &[
    SidebarGroup {
        label: "",
        items: &[SidebarItem {
            label: "仪表盘",
            href: "/admin",
            icon: "grid",
        }],
    },
    SidebarGroup {
        label: "内容",
        items: &[
            SidebarItem {
                label: "文章管理",
                href: "/admin/posts",
                icon: "file-text",
            },
            SidebarItem {
                label: "页面管理",
                href: "/admin/pages",
                icon: "file",
            },
            SidebarItem {
                label: "分类管理",
                href: "/admin/categories",
                icon: "folder",
            },
            SidebarItem {
                label: "标签管理",
                href: "/admin/tags",
                icon: "tag",
            },
            SidebarItem {
                label: "媒体库",
                href: "/admin/media",
                icon: "image",
            },
        ],
    },
    SidebarGroup {
        label: "系统",
        items: &[
            SidebarItem {
                label: "构建管理",
                href: "/admin/build",
                icon: "package",
            },
            SidebarItem {
                label: "主题设置",
                href: "/admin/theme",
                icon: "palette",
            },
            SidebarItem {
                label: "插件管理",
                href: "/admin/plugins",
                icon: "plug",
            },
        ],
    },
];

/// 为 minijinja 模板生成 sidebar_groups 数据
pub fn sidebar_groups_value(active_path: &str) -> Vec<minijinja::Value> {
    SIDEBAR_GROUPS
        .iter()
        .map(|group| {
            let items: Vec<minijinja::Value> = group
                .items
                .iter()
                .map(|item| {
                    minijinja::context! {
                        label => item.label,
                        href => item.href,
                        icon => item.icon,
                        active => is_active(active_path, item.href),
                    }
                })
                .collect();
            minijinja::context! {
                label => if group.label.is_empty() { None } else { Some(group.label) },
                items => items,
            }
        })
        .collect()
}

/// 为 minijinja 模板生成 plugin_sidebar_items 数据
pub fn plugin_sidebar_value(
    entries: &[PluginSidebarEntry],
    active_path: &str,
) -> Vec<minijinja::Value> {
    entries
        .iter()
        .map(|entry| {
            minijinja::context! {
                label => entry.label.as_str(),
                href => entry.href.as_str(),
                icon => entry.icon.as_str(),
                active => is_active(active_path, &entry.href),
            }
        })
        .collect()
}

// ── SVG 图标 ──

pub fn svg_icon(name: &str) -> &'static str {
    match name {
        "grid" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>"#,
        "file-text" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>"#,
        "file" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><polyline points="13 2 13 9 20 9"/></svg>"#,
        "image" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>"#,
        "package" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="16.5" y1="9.4" x2="7.5" y2="4.21"/><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>"#,
        "palette" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="2"/><circle cx="17.5" cy="10.5" r="2"/><circle cx="8.5" cy="7.5" r="2"/><circle cx="6.5" cy="12.5" r="2"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/></svg>"#,
        "plug" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a6 6 0 0 1-6 6 6 6 0 0 1-6-6V8z"/></svg>"#,
        "user" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>"#,
        "key" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>"#,
        "log-out" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>"#,
        "terminal" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>"#,
        "search" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>"#,
        "arrow-left" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>"#,
        "plus" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#,
        "upload" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 16 12 12 8 16"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.39 18.39A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.3"/></svg>"#,
        "folder" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>"#,
        "tag" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>"#,
        "external-link" => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>"#,
        _ => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/></svg>"#,
    }
}
