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
