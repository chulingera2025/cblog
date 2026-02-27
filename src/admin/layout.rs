// 后台管理共享布局 — Stripe Dashboard 风格

/// 插件侧边栏条目
#[derive(Clone)]
pub struct PluginSidebarEntry {
    pub plugin_name: String,
    pub label: String,
    pub href: String,
    pub icon: String,
}

/// 页面渲染上下文
pub struct PageContext {
    pub site_title: String,
    pub plugin_sidebar_items: Vec<PluginSidebarEntry>,
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

pub fn admin_page(title: &str, active_path: &str, body: &str, ctx: &PageContext) -> String {
    admin_page_inner(title, active_path, body, "", ctx)
}

pub fn admin_page_with_script(
    title: &str,
    active_path: &str,
    body: &str,
    script: &str,
    ctx: &PageContext,
) -> String {
    admin_page_inner(title, active_path, body, script, ctx)
}

fn admin_page_inner(
    title: &str,
    active_path: &str,
    body: &str,
    script: &str,
    ctx: &PageContext,
) -> String {
    let sidebar = render_sidebar(active_path, ctx);
    let script_tag = if script.is_empty() {
        String::new()
    } else {
        format!("<script>{script}</script>")
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} - {site_title} 管理后台</title>
<style>{CSS}</style>
</head>
<body>
<div class="admin-layout">
{sidebar}
<main class="admin-main">
<div class="admin-content">
{body}
</div>
</main>
</div>
<div id="toast-container" class="toast-container"></div>
<script>{TOAST_SCRIPT}</script>
<script>{CONFIRM_SCRIPT}</script>
{script_tag}
</body>
</html>"#,
        title = html_escape(title),
        site_title = html_escape(&ctx.site_title),
        CSS = CSS,
        sidebar = sidebar,
        body = body,
        TOAST_SCRIPT = TOAST_SCRIPT,
        CONFIRM_SCRIPT = CONFIRM_SCRIPT,
        script_tag = script_tag,
    )
}

// ── 侧边栏渲染 ──

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

fn render_sidebar(active_path: &str, ctx: &PageContext) -> String {
    let mut html = String::from(r#"<aside class="admin-sidebar">"#);

    // 品牌标识
    html.push_str(r#"<div class="sidebar-brand">"#);
    html.push_str(svg_icon("terminal"));
    html.push_str("<span>cblog</span></div>");

    // 内置分组
    for group in SIDEBAR_GROUPS {
        html.push_str(r#"<div class="sidebar-group">"#);
        if !group.label.is_empty() {
            html.push_str(&format!(
                r#"<div class="sidebar-group-label">{}</div>"#,
                group.label
            ));
        }
        for item in group.items {
            let active_class = if is_active(active_path, item.href) {
                " active"
            } else {
                ""
            };
            html.push_str(&format!(
                r#"<a href="{href}" class="sidebar-item{active_class}">{icon}<span>{label}</span></a>"#,
                href = item.href,
                active_class = active_class,
                icon = svg_icon(item.icon),
                label = item.label,
            ));
        }
        html.push_str("</div>");
    }

    // 插件扩展分组
    if !ctx.plugin_sidebar_items.is_empty() {
        html.push_str(r#"<div class="sidebar-group">"#);
        html.push_str(r#"<div class="sidebar-group-label">插件扩展</div>"#);
        for entry in &ctx.plugin_sidebar_items {
            let active_class = if is_active(active_path, &entry.href) {
                " active"
            } else {
                ""
            };
            html.push_str(&format!(
                r#"<a href="{href}" class="sidebar-item{active_class}">{icon}<span>{label}</span></a>"#,
                href = html_escape(&entry.href),
                active_class = active_class,
                icon = svg_icon(&entry.icon),
                label = html_escape(&entry.label),
            ));
        }
        html.push_str("</div>");
    }

    // 底部固定区域
    html.push_str(r#"<div class="sidebar-footer">"#);
    let profile_active = if is_active(active_path, "/admin/profile") {
        " active"
    } else {
        ""
    };
    html.push_str(&format!(
        r#"<a href="/admin/profile" class="sidebar-item{profile_active}">{icon}<span>个人资料</span></a>"#,
        profile_active = profile_active,
        icon = svg_icon("user"),
    ));
    html.push_str(&format!(
        r#"<form method="POST" action="/admin/logout" class="sidebar-logout-form">
<button type="submit" class="sidebar-item">{icon}<span>登出</span></button>
</form>"#,
        icon = svg_icon("log-out"),
    ));
    html.push_str("</div>");

    html.push_str("</aside>");
    html
}

// ── 模板 sidebar 数据构建 ──

/// 将内置侧边栏分组数据转为 minijinja Value，用于模板渲染
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
                label => if group.label.is_empty() { "" } else { group.label },
                items => items,
            }
        })
        .collect()
}

/// 将插件侧边栏条目转为 minijinja Value
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
        _ => r#"<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/></svg>"#,
    }
}

// ── CSS 设计系统 ──

const CSS: &str = r#"
/* ── Design Tokens ── */
:root {
    --c-brand: #635BFF;
    --c-brand-hover: #5851db;
    --c-text-primary: #0A2540;
    --c-text-body: #3C4257;
    --c-text-secondary: #697386;
    --c-bg-page: #F6F9FC;
    --c-bg-card: #FFFFFF;
    --c-bg-sidebar: #0A2540;
    --c-border: #E3E8EE;
    --c-success: #30B566;
    --c-warning: #E5A54B;
    --c-danger: #DF1B41;
    --c-info: #635BFF;
    --sidebar-width: 240px;
    --radius: 6px;
    --radius-lg: 8px;
    --shadow-sm: 0 1px 2px rgba(0,0,0,.05);
    --shadow: 0 1px 3px rgba(0,0,0,.08), 0 1px 2px rgba(0,0,0,.04);
    --transition: 150ms ease;
}

/* ── Reset ── */
*, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
    background: var(--c-bg-page);
    color: var(--c-text-body);
    font-size: 14px;
    line-height: 1.5;
    -webkit-font-smoothing: antialiased;
}

/* ── Layout ── */
.admin-layout {
    display: flex;
    min-height: 100vh;
}

.admin-sidebar {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    width: var(--sidebar-width);
    background: var(--c-bg-sidebar);
    color: #C1C9D2;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    z-index: 100;
}

.admin-main {
    margin-left: var(--sidebar-width);
    flex: 1;
    min-width: 0;
}

.admin-content {
    max-width: 1200px;
    margin: 0 auto;
    padding: 32px 40px;
}

/* ── Sidebar ── */
.sidebar-brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px 20px 16px;
    font-size: 18px;
    font-weight: 700;
    color: #FFFFFF;
    letter-spacing: -0.3px;
}
.sidebar-brand .icon {
    width: 20px;
    height: 20px;
}

.sidebar-group {
    padding: 4px 12px;
}

.sidebar-group-label {
    padding: 8px 12px 4px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #5E6B7A;
}

.sidebar-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 12px;
    border-radius: var(--radius);
    color: #C1C9D2;
    text-decoration: none;
    font-size: 13px;
    font-weight: 500;
    transition: background var(--transition), color var(--transition);
    cursor: pointer;
}
.sidebar-item:hover {
    background: rgba(255,255,255,.08);
    color: #FFFFFF;
    text-decoration: none;
}
.sidebar-item.active {
    background: rgba(255,255,255,.12);
    color: #FFFFFF;
}
.sidebar-item .icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    opacity: 0.7;
}
.sidebar-item.active .icon,
.sidebar-item:hover .icon {
    opacity: 1;
}

.sidebar-footer {
    margin-top: auto;
    padding: 12px;
    border-top: 1px solid rgba(255,255,255,.08);
}

.sidebar-logout-form {
    margin: 0;
}
.sidebar-logout-form .sidebar-item {
    width: 100%;
    background: none;
    border: none;
    font: inherit;
    text-align: left;
}

.sidebar-badge {
    margin-left: auto;
    background: var(--c-brand);
    color: #fff;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 10px;
    font-weight: 600;
}

/* ── Page Header ── */
.page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 24px;
}

.page-title {
    font-size: 24px;
    font-weight: 600;
    color: var(--c-text-primary);
    line-height: 1.3;
}

.page-back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--c-text-secondary);
    text-decoration: none;
    margin-bottom: 16px;
    transition: color var(--transition);
}
.page-back:hover {
    color: var(--c-brand);
    text-decoration: none;
}
.page-back .icon {
    width: 14px;
    height: 14px;
}

/* ── Card ── */
.card {
    background: var(--c-bg-card);
    border: 1px solid var(--c-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
}

.card-header {
    padding: 16px 20px;
    border-bottom: 1px solid var(--c-border);
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.card-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--c-text-primary);
}

.card-body {
    padding: 20px;
}

/* ── Stat Grid ── */
.stat-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 16px;
    margin-bottom: 24px;
}

.stat-card {
    background: var(--c-bg-card);
    border: 1px solid var(--c-border);
    border-radius: var(--radius-lg);
    padding: 20px;
    box-shadow: var(--shadow-sm);
}

.stat-value {
    font-size: 28px;
    font-weight: 600;
    color: var(--c-text-primary);
    line-height: 1.2;
}

.stat-label {
    font-size: 13px;
    color: var(--c-text-secondary);
    margin-top: 4px;
}

/* ── Table ── */
.table-wrapper {
    background: var(--c-bg-card);
    border: 1px solid var(--c-border);
    border-radius: var(--radius-lg);
    overflow: hidden;
    box-shadow: var(--shadow-sm);
}

table {
    width: 100%;
    border-collapse: collapse;
}

th {
    padding: 10px 16px;
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--c-text-secondary);
    background: #FAFBFC;
    border-bottom: 1px solid var(--c-border);
}

td {
    padding: 12px 16px;
    border-bottom: 1px solid #F1F5F9;
    font-size: 14px;
    color: var(--c-text-body);
    vertical-align: middle;
}

tr:last-child td {
    border-bottom: none;
}

tr:hover td {
    background: #FAFBFC;
}

td a {
    color: var(--c-brand);
    text-decoration: none;
    font-weight: 500;
}
td a:hover {
    text-decoration: underline;
}

td .actions {
    display: flex;
    gap: 6px;
    align-items: center;
}
td .actions form {
    display: inline;
}

/* ── Form ── */
.form-group {
    margin-bottom: 16px;
}

.form-label {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: var(--c-text-primary);
    margin-bottom: 6px;
}

.form-hint {
    font-size: 12px;
    color: var(--c-text-secondary);
    margin-top: 4px;
}

.form-input,
.form-select,
.form-textarea {
    width: 100%;
    padding: 8px 12px;
    border: 1px solid var(--c-border);
    border-radius: var(--radius);
    font-size: 14px;
    color: var(--c-text-body);
    background: var(--c-bg-card);
    transition: border-color var(--transition), box-shadow var(--transition);
    outline: none;
    font-family: inherit;
}

.form-input:focus,
.form-select:focus,
.form-textarea:focus {
    border-color: var(--c-brand);
    box-shadow: 0 0 0 3px rgba(99,91,255,.12);
}

.form-textarea {
    min-height: 120px;
    resize: vertical;
}

textarea.code {
    font-family: 'SF Mono', 'Fira Code', 'Fira Mono', Menlo, Consolas, monospace;
    font-size: 13px;
    min-height: 200px;
}

.form-row {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 16px;
}

.form-check {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
}

.form-check input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--c-brand);
}

/* ── Button ── */
.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: var(--radius);
    border: none;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    font-family: inherit;
    text-decoration: none;
    transition: background var(--transition), box-shadow var(--transition);
    line-height: 1.4;
    white-space: nowrap;
}
.btn .icon {
    width: 14px;
    height: 14px;
}

.btn-primary {
    background: var(--c-brand);
    color: #FFFFFF;
}
.btn-primary:hover {
    background: var(--c-brand-hover);
    text-decoration: none;
}

.btn-secondary {
    background: #FFFFFF;
    color: var(--c-text-body);
    border: 1px solid var(--c-border);
}
.btn-secondary:hover {
    background: #F6F9FC;
    text-decoration: none;
}

.btn-danger {
    background: var(--c-danger);
    color: #FFFFFF;
}
.btn-danger:hover {
    background: #C41636;
    text-decoration: none;
}

.btn-success {
    background: var(--c-success);
    color: #FFFFFF;
}
.btn-success:hover {
    background: #28A05A;
    text-decoration: none;
}

.btn-ghost {
    background: transparent;
    color: var(--c-text-secondary);
    padding: 6px 10px;
}
.btn-ghost:hover {
    background: rgba(0,0,0,.04);
    color: var(--c-text-body);
    text-decoration: none;
}

.btn-sm {
    padding: 4px 10px;
    font-size: 12px;
}

/* ── Badge ── */
.badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 12px;
    font-weight: 500;
    line-height: 1.5;
}

.badge-success {
    background: #E8F5ED;
    color: #1A7F37;
}

.badge-warning {
    background: #FFF3E0;
    color: #9A6700;
}

.badge-danger {
    background: #FFEEF0;
    color: #C41636;
}

.badge-info {
    background: #EEF0FF;
    color: #4A46B8;
}

.badge-neutral {
    background: #F1F5F9;
    color: var(--c-text-secondary);
}

/* ── Pagination ── */
.pagination {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 20px;
}

.pagination .btn {
    min-width: 36px;
}

/* ── Filter Bar ── */
.filter-bar {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 16px;
    flex-wrap: wrap;
}

.filter-bar .form-input,
.filter-bar .form-select {
    width: auto;
    min-width: 140px;
}

/* ── Media Grid ── */
.media-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 16px;
}

.media-card {
    background: var(--c-bg-card);
    border: 1px solid var(--c-border);
    border-radius: var(--radius-lg);
    overflow: hidden;
    box-shadow: var(--shadow-sm);
    transition: box-shadow var(--transition);
}
.media-card:hover {
    box-shadow: var(--shadow);
}

.media-card img {
    width: 100%;
    height: 160px;
    object-fit: cover;
    display: block;
    background: #F1F5F9;
}

.media-card .file-icon {
    width: 100%;
    height: 160px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #F1F5F9;
    font-size: 40px;
    color: var(--c-text-secondary);
}

.media-card .info {
    padding: 12px;
}

.media-card .info .filename {
    font-size: 13px;
    font-weight: 500;
    color: var(--c-text-primary);
    word-break: break-all;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-bottom: 4px;
}

.media-card .info .meta {
    font-size: 11px;
    color: var(--c-text-secondary);
    margin-bottom: 8px;
}

.media-card .info .actions {
    display: flex;
    gap: 6px;
}

/* ── Empty State ── */
.empty-state {
    text-align: center;
    padding: 48px 20px;
    color: var(--c-text-secondary);
}

.empty-state p {
    margin-top: 8px;
    font-size: 14px;
}

/* ── Toast ── */
.toast-container {
    position: fixed;
    top: 20px;
    right: 20px;
    z-index: 10000;
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.toast {
    padding: 12px 20px;
    border-radius: var(--radius);
    color: #fff;
    font-size: 14px;
    font-weight: 500;
    box-shadow: 0 4px 12px rgba(0,0,0,.15);
    opacity: 0;
    transform: translateX(40px);
    animation: toast-in 300ms ease forwards;
    max-width: 400px;
}

.toast-success { background: var(--c-success); }
.toast-error { background: var(--c-danger); }
.toast-info { background: var(--c-brand); }

@keyframes toast-in {
    to { opacity: 1; transform: translateX(0); }
}
@keyframes toast-out {
    to { opacity: 0; transform: translateX(40px); }
}

/* ── Modal ── */
.modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(10,37,64,.4);
    z-index: 9000;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: fade-in 150ms ease;
}

.modal {
    background: var(--c-bg-card);
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 60px rgba(0,0,0,.2);
    padding: 24px;
    width: 100%;
    max-width: 420px;
    animation: scale-in 200ms ease;
}

.modal-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--c-text-primary);
    margin-bottom: 12px;
}

.modal-body {
    font-size: 14px;
    color: var(--c-text-body);
    margin-bottom: 20px;
}

.modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
}

@keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
}
@keyframes scale-in {
    from { opacity: 0; transform: scale(.95); }
    to { opacity: 1; transform: scale(1); }
}

/* ── Icon ── */
.icon {
    width: 16px;
    height: 16px;
    vertical-align: middle;
    flex-shrink: 0;
}

/* ── Links ── */
a {
    color: var(--c-brand);
    text-decoration: none;
    transition: color var(--transition);
}
a:hover {
    color: var(--c-brand-hover);
}

/* ── Alert ── */
.alert {
    padding: 12px 16px;
    border-radius: var(--radius);
    margin-bottom: 16px;
    font-size: 14px;
}
.alert-error {
    background: #FFEEF0;
    color: var(--c-danger);
    border: 1px solid #FFD1D6;
}
.alert-success {
    background: #E8F5ED;
    color: #1A7F37;
    border: 1px solid #C6ECD2;
}

/* ── Responsive ── */
@media (max-width: 768px) {
    .admin-sidebar { width: 200px; }
    .admin-main { margin-left: 200px; }
    .admin-content { padding: 20px 16px; }
    .stat-grid { grid-template-columns: repeat(2, 1fr); }
    .form-row { grid-template-columns: 1fr; }
}

/* ── Editor ── */
.editor-wrap {
    border: 1px solid var(--c-border);
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--c-bg-card);
}

.editor-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 8px;
    background: #FAFBFC;
    border-bottom: 1px solid var(--c-border);
    position: sticky;
    top: 0;
    z-index: 10;
}

.toolbar-group {
    display: flex;
    gap: 2px;
    align-items: center;
}

.toolbar-divider {
    width: 1px;
    height: 24px;
    background: var(--c-border);
    margin: 0 4px;
}

.editor-toolbar button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    color: var(--c-text-secondary);
    cursor: pointer;
    font-size: 14px;
    font-weight: 600;
    transition: background var(--transition), color var(--transition);
}

.editor-toolbar button:hover {
    background: var(--c-border);
    color: var(--c-text-primary);
}

.editor-toolbar button.active {
    background: var(--c-brand);
    color: #fff;
}

.editor-toolbar select {
    padding: 4px 8px;
    border: 1px solid var(--c-border);
    border-radius: var(--radius);
    font-size: 13px;
    color: var(--c-text-body);
    background: var(--c-bg-card);
    cursor: pointer;
    outline: none;
}

.editor-content .ProseMirror {
    min-height: 400px;
    padding: 20px;
    outline: none;
    font-size: 15px;
    line-height: 1.7;
    color: var(--c-text-body);
}

.editor-content .ProseMirror:focus {
    outline: none;
}

.editor-content .ProseMirror p.is-editor-empty:first-child::before {
    content: attr(data-placeholder);
    float: left;
    color: var(--c-text-secondary);
    pointer-events: none;
    height: 0;
}

.editor-content .ProseMirror h1 { font-size: 2em; font-weight: 700; margin: 1em 0 0.5em; color: var(--c-text-primary); }
.editor-content .ProseMirror h2 { font-size: 1.5em; font-weight: 600; margin: 0.8em 0 0.4em; color: var(--c-text-primary); }
.editor-content .ProseMirror h3 { font-size: 1.25em; font-weight: 600; margin: 0.6em 0 0.3em; color: var(--c-text-primary); }

.editor-content .ProseMirror ul,
.editor-content .ProseMirror ol {
    padding-left: 1.5em;
    margin: 0.5em 0;
}

.editor-content .ProseMirror blockquote {
    border-left: 3px solid var(--c-brand);
    padding-left: 1em;
    margin: 1em 0;
    color: var(--c-text-secondary);
}

.editor-content .ProseMirror pre {
    background: #1E293B;
    color: #E2E8F0;
    border-radius: var(--radius);
    padding: 16px;
    margin: 1em 0;
    overflow-x: auto;
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 13px;
}

.editor-content .ProseMirror code {
    background: #F1F5F9;
    padding: 2px 6px;
    border-radius: 3px;
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 0.9em;
}

.editor-content .ProseMirror pre code {
    background: none;
    padding: 0;
    border-radius: 0;
    color: inherit;
}

.editor-content .ProseMirror img {
    max-width: 100%;
    height: auto;
    border-radius: var(--radius);
    margin: 1em 0;
}

.editor-content .ProseMirror hr {
    border: none;
    border-top: 2px solid var(--c-border);
    margin: 2em 0;
}

.editor-content .ProseMirror table {
    border-collapse: collapse;
    width: 100%;
    margin: 1em 0;
}

.editor-content .ProseMirror table td,
.editor-content .ProseMirror table th {
    border: 1px solid var(--c-border);
    padding: 8px 12px;
    text-align: left;
}

.editor-content .ProseMirror table th {
    background: #FAFBFC;
    font-weight: 600;
}

.editor-content .ProseMirror ul[data-type="taskList"] {
    list-style: none;
    padding-left: 0;
}

.editor-content .ProseMirror ul[data-type="taskList"] li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
}

.editor-content .ProseMirror ul[data-type="taskList"] li label {
    margin-top: 2px;
}

.editor-content .ProseMirror a {
    color: var(--c-brand);
    text-decoration: underline;
}

.media-picker-modal {
    max-width: 720px;
}

.media-picker-modal .modal-body {
    max-height: 400px;
    overflow-y: auto;
}

.media-picker-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 12px;
}

.media-picker-item {
    cursor: pointer;
    border: 2px solid transparent;
    border-radius: var(--radius);
    overflow: hidden;
    transition: border-color var(--transition);
}

.media-picker-item:hover {
    border-color: var(--c-brand);
}

.media-picker-item img {
    width: 100%;
    height: 80px;
    object-fit: cover;
    display: block;
}

.media-picker-item .name {
    font-size: 11px;
    padding: 4px;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
"#;

// ── Toast JS ──

const TOAST_SCRIPT: &str = r#"
function showToast(msg, type) {
    type = type || 'info';
    var container = document.getElementById('toast-container');
    var el = document.createElement('div');
    el.className = 'toast toast-' + type;
    el.textContent = msg;
    container.appendChild(el);
    setTimeout(function() {
        el.style.animation = 'toast-out 300ms ease forwards';
        setTimeout(function() { el.remove(); }, 300);
    }, 3000);
}
(function() {
    var params = new URLSearchParams(window.location.search);
    var msg = params.get('toast_msg');
    var type = params.get('toast_type') || 'success';
    if (msg) {
        showToast(decodeURIComponent(msg), type);
        var url = new URL(window.location);
        url.searchParams.delete('toast_msg');
        url.searchParams.delete('toast_type');
        window.history.replaceState({}, '', url);
    }
})();
"#;

// ── 模态确认框 JS ──

const CONFIRM_SCRIPT: &str = r#"
function confirmAction(title, message, formEl) {
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
        '<div class="modal">' +
            '<div class="modal-title">' + title + '</div>' +
            '<div class="modal-body">' + message + '</div>' +
            '<div class="modal-actions">' +
                '<button class="btn btn-secondary" id="modal-cancel">取消</button>' +
                '<button class="btn btn-danger" id="modal-confirm">确认</button>' +
            '</div>' +
        '</div>';
    document.body.appendChild(backdrop);
    document.getElementById('modal-cancel').onclick = function() { backdrop.remove(); };
    document.getElementById('modal-confirm').onclick = function() {
        backdrop.remove();
        formEl.submit();
    };
    backdrop.onclick = function(e) { if (e.target === backdrop) backdrop.remove(); };
}
"#;

const EDITOR_SCRIPT: &str = r#"
import { Editor } from 'https://esm.sh/@tiptap/core@2'
import StarterKit from 'https://esm.sh/@tiptap/starter-kit@2'
import Link from 'https://esm.sh/@tiptap/extension-link@2'
import Image from 'https://esm.sh/@tiptap/extension-image@2'
import Table from 'https://esm.sh/@tiptap/extension-table@2'
import TableRow from 'https://esm.sh/@tiptap/extension-table-row@2'
import TableCell from 'https://esm.sh/@tiptap/extension-table-cell@2'
import TableHeader from 'https://esm.sh/@tiptap/extension-table-header@2'
import Underline from 'https://esm.sh/@tiptap/extension-underline@2'
import TextAlign from 'https://esm.sh/@tiptap/extension-text-align@2'
import Placeholder from 'https://esm.sh/@tiptap/extension-placeholder@2'
import TextStyle from 'https://esm.sh/@tiptap/extension-text-style@2'
import Color from 'https://esm.sh/@tiptap/extension-color@2'
import Highlight from 'https://esm.sh/@tiptap/extension-highlight@2'
import TaskList from 'https://esm.sh/@tiptap/extension-task-list@2'
import TaskItem from 'https://esm.sh/@tiptap/extension-task-item@2'

const contentDataEl = document.getElementById('editor-content-data');
const initialContent = contentDataEl ? JSON.parse(contentDataEl.textContent) : '';

const editor = new Editor({
    element: document.getElementById('editor'),
    extensions: [
        StarterKit,
        Link.configure({ openOnClick: false }),
        Image,
        Table.configure({ resizable: true }),
        TableRow,
        TableCell,
        TableHeader,
        Underline,
        TextAlign.configure({ types: ['heading', 'paragraph'] }),
        Placeholder.configure({ placeholder: '开始编写内容...' }),
        TextStyle,
        Color,
        Highlight.configure({ multicolor: true }),
        TaskList,
        TaskItem.configure({ nested: true }),
    ],
    content: initialContent,
    onUpdate({ editor }) {
        document.getElementById('content-input').value = editor.getHTML();
    },
});

// 初始化时同步一次
document.getElementById('content-input').value = editor.getHTML();

// 表单提交时确保最新内容
document.querySelectorAll('form').forEach(form => {
    form.addEventListener('submit', () => {
        const input = document.getElementById('content-input');
        if (input) input.value = editor.getHTML();
    });
});

// 工具栏按钮绑定
const toolbar = document.getElementById('editor-toolbar');
if (toolbar) {
    toolbar.querySelectorAll('button[data-cmd]').forEach(btn => {
        btn.addEventListener('click', () => {
            const cmd = btn.dataset.cmd;
            switch (cmd) {
                case 'bold': editor.chain().focus().toggleBold().run(); break;
                case 'italic': editor.chain().focus().toggleItalic().run(); break;
                case 'underline': editor.chain().focus().toggleUnderline().run(); break;
                case 'strike': editor.chain().focus().toggleStrike().run(); break;
                case 'code': editor.chain().focus().toggleCode().run(); break;
                case 'bulletList': editor.chain().focus().toggleBulletList().run(); break;
                case 'orderedList': editor.chain().focus().toggleOrderedList().run(); break;
                case 'taskList': editor.chain().focus().toggleTaskList().run(); break;
                case 'blockquote': editor.chain().focus().toggleBlockquote().run(); break;
                case 'codeBlock': editor.chain().focus().toggleCodeBlock().run(); break;
                case 'hr': editor.chain().focus().setHorizontalRule().run(); break;
                case 'alignLeft': editor.chain().focus().setTextAlign('left').run(); break;
                case 'alignCenter': editor.chain().focus().setTextAlign('center').run(); break;
                case 'alignRight': editor.chain().focus().setTextAlign('right').run(); break;
                case 'undo': editor.chain().focus().undo().run(); break;
                case 'redo': editor.chain().focus().redo().run(); break;
                case 'link': {
                    const url = prompt('输入链接 URL:');
                    if (url) editor.chain().focus().setLink({ href: url }).run();
                    break;
                }
                case 'image': {
                    openMediaPicker(editor);
                    break;
                }
                case 'table': {
                    editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
                    break;
                }
            }
        });
    });

    // 标题选择器
    const headingSelect = document.getElementById('tb-heading');
    if (headingSelect) {
        headingSelect.addEventListener('change', () => {
            const val = headingSelect.value;
            if (val === 'p') {
                editor.chain().focus().setParagraph().run();
            } else {
                editor.chain().focus().toggleHeading({ level: parseInt(val) }).run();
            }
        });
    }

    // 更新工具栏激活状态
    editor.on('selectionUpdate', () => updateToolbar());
    editor.on('update', () => updateToolbar());

    function updateToolbar() {
        toolbar.querySelectorAll('button[data-cmd]').forEach(btn => {
            const cmd = btn.dataset.cmd;
            let isActive = false;
            switch (cmd) {
                case 'bold': isActive = editor.isActive('bold'); break;
                case 'italic': isActive = editor.isActive('italic'); break;
                case 'underline': isActive = editor.isActive('underline'); break;
                case 'strike': isActive = editor.isActive('strike'); break;
                case 'code': isActive = editor.isActive('code'); break;
                case 'bulletList': isActive = editor.isActive('bulletList'); break;
                case 'orderedList': isActive = editor.isActive('orderedList'); break;
                case 'taskList': isActive = editor.isActive('taskList'); break;
                case 'blockquote': isActive = editor.isActive('blockquote'); break;
                case 'codeBlock': isActive = editor.isActive('codeBlock'); break;
                case 'alignLeft': isActive = editor.isActive({ textAlign: 'left' }); break;
                case 'alignCenter': isActive = editor.isActive({ textAlign: 'center' }); break;
                case 'alignRight': isActive = editor.isActive({ textAlign: 'right' }); break;
            }
            btn.classList.toggle('active', isActive);
        });

        // 更新标题选择器
        const headingSelect = document.getElementById('tb-heading');
        if (headingSelect) {
            if (editor.isActive('heading', { level: 1 })) headingSelect.value = '1';
            else if (editor.isActive('heading', { level: 2 })) headingSelect.value = '2';
            else if (editor.isActive('heading', { level: 3 })) headingSelect.value = '3';
            else headingSelect.value = 'p';
        }
    }
}

// 媒体选择器
function openMediaPicker(editor) {
    const backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
        '<div class="modal media-picker-modal">' +
            '<div class="modal-title">选择媒体</div>' +
            '<div class="modal-body"><div class="media-picker-grid" id="media-picker-grid">加载中...</div></div>' +
            '<div class="modal-actions">' +
                '<button class="btn btn-secondary" id="media-picker-cancel">取消</button>' +
                '<div style="flex:1"></div>' +
                '<input type="text" id="media-picker-url" placeholder="或输入图片 URL..." class="form-input" style="width:240px;margin-right:8px;">' +
                '<button class="btn btn-primary" id="media-picker-insert-url">插入 URL</button>' +
            '</div>' +
        '</div>';
    document.body.appendChild(backdrop);

    document.getElementById('media-picker-cancel').onclick = () => backdrop.remove();
    backdrop.onclick = (e) => { if (e.target === backdrop) backdrop.remove(); };

    document.getElementById('media-picker-insert-url').onclick = () => {
        const url = document.getElementById('media-picker-url').value.trim();
        if (url) {
            editor.chain().focus().setImage({ src: url }).run();
            backdrop.remove();
        }
    };

    fetch('/admin/api/media')
        .then(r => r.json())
        .then(items => {
            const grid = document.getElementById('media-picker-grid');
            if (!items || items.length === 0) {
                grid.innerHTML = '<p style="text-align:center;color:var(--c-text-secondary);">暂无媒体文件</p>';
                return;
            }
            grid.innerHTML = '';
            items.forEach(item => {
                if (!item.url) return;
                const isImage = /\.(jpg|jpeg|png|gif|webp|svg)$/i.test(item.url);
                if (!isImage) return;
                const el = document.createElement('div');
                el.className = 'media-picker-item';
                el.innerHTML = '<img src="' + item.url + '" alt="' + (item.filename || '') + '">' +
                    '<div class="name">' + (item.filename || '') + '</div>';
                el.onclick = () => {
                    editor.chain().focus().setImage({ src: item.url }).run();
                    backdrop.remove();
                };
                grid.appendChild(el);
            });
        })
        .catch(() => {
            document.getElementById('media-picker-grid').innerHTML =
                '<p style="text-align:center;color:var(--c-danger);">加载媒体失败</p>';
        });
}
"#;

pub fn editor_toolbar() -> &'static str {
    r#"<div class="editor-toolbar" id="editor-toolbar">
    <div class="toolbar-group">
        <select id="tb-heading">
            <option value="p">正文</option>
            <option value="1">标题 1</option>
            <option value="2">标题 2</option>
            <option value="3">标题 3</option>
        </select>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
        <button type="button" data-cmd="bold" title="粗体"><b>B</b></button>
        <button type="button" data-cmd="italic" title="斜体"><i>I</i></button>
        <button type="button" data-cmd="underline" title="下划线"><u>U</u></button>
        <button type="button" data-cmd="strike" title="删除线"><s>S</s></button>
        <button type="button" data-cmd="code" title="行内代码">&lt;/&gt;</button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
        <button type="button" data-cmd="bulletList" title="无序列表">•</button>
        <button type="button" data-cmd="orderedList" title="有序列表">1.</button>
        <button type="button" data-cmd="taskList" title="任务列表">☑</button>
        <button type="button" data-cmd="blockquote" title="引用">❝</button>
        <button type="button" data-cmd="codeBlock" title="代码块">{ }</button>
        <button type="button" data-cmd="hr" title="分隔线">—</button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
        <button type="button" data-cmd="link" title="链接">🔗</button>
        <button type="button" data-cmd="image" title="图片">🖼</button>
        <button type="button" data-cmd="table" title="表格">☰</button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
        <button type="button" data-cmd="alignLeft" title="左对齐">⫷</button>
        <button type="button" data-cmd="alignCenter" title="居中">≡</button>
        <button type="button" data-cmd="alignRight" title="右对齐">⫸</button>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-group">
        <button type="button" data-cmd="undo" title="撤销">↩</button>
        <button type="button" data-cmd="redo" title="重做">↪</button>
    </div>
</div>"#
}

pub fn admin_editor_page(
    title: &str,
    active_path: &str,
    body: &str,
    initial_content: &str,
    ctx: &PageContext,
) -> String {
    let sidebar = render_sidebar(active_path, ctx);

    let content_json = serde_json::to_string(initial_content).unwrap_or_else(|_| "\"\"".to_string());

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} - {site_title} 管理后台</title>
<style>{CSS}</style>
</head>
<body>
<div class="admin-layout">
{sidebar}
<main class="admin-main">
<div class="admin-content">
{body}
</div>
</main>
</div>
<div id="toast-container" class="toast-container"></div>
<script>{TOAST_SCRIPT}</script>
<script>{CONFIRM_SCRIPT}</script>
<script type="application/json" id="editor-content-data">{content_json}</script>
<script type="module">{EDITOR_SCRIPT}</script>
</body>
</html>"#,
        title = html_escape(title),
        site_title = html_escape(&ctx.site_title),
        CSS = CSS,
        sidebar = sidebar,
        body = body,
        TOAST_SCRIPT = TOAST_SCRIPT,
        CONFIRM_SCRIPT = CONFIRM_SCRIPT,
        content_json = content_json,
        EDITOR_SCRIPT = EDITOR_SCRIPT,
    )
}
