pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn admin_nav() -> String {
    r#"<nav style="background:#1a1a2e;padding:12px 24px;display:flex;gap:24px;align-items:center;">
        <a href="/admin" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">仪表盘</a>
        <a href="/admin/posts" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">文章</a>
        <a href="/admin/pages" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">页面</a>
        <a href="/admin/media" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">媒体</a>
        <a href="/admin/build" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">构建</a>
        <a href="/admin/theme" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">主题</a>
        <a href="/admin/plugins" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">插件</a>
        <div style="margin-left:auto;">
            <form method="POST" action="/admin/logout" style="margin:0;">
                <button type="submit" style="background:transparent;border:1px solid #e0e0e0;color:#e0e0e0;padding:4px 12px;border-radius:4px;cursor:pointer;font-size:13px;">登出</button>
            </form>
        </div>
    </nav>"#
        .to_string()
}

pub fn base_style() -> &'static str {
    r#"<style>
        * { margin:0; padding:0; box-sizing:border-box; }
        body { font-family:system-ui,-apple-system,sans-serif; background:#f5f5f5; color:#333; }
        .container { max-width:1000px; margin:24px auto; padding:0 16px; }
        h1 { margin-bottom:16px; }
        h2 { margin-top:24px; margin-bottom:12px; }
        table { width:100%; border-collapse:collapse; background:#fff; border-radius:4px; overflow:hidden; box-shadow:0 1px 3px rgba(0,0,0,0.1); }
        th,td { padding:10px 14px; text-align:left; border-bottom:1px solid #eee; }
        th { background:#f8f8f8; font-weight:600; }
        a { color:#4a6cf7; text-decoration:none; }
        a:hover { text-decoration:underline; }
        .btn { display:inline-block; padding:6px 14px; border-radius:4px; border:none; cursor:pointer; font-size:14px; text-decoration:none; }
        .btn-primary { background:#4a6cf7; color:#fff; }
        .btn-danger { background:#e74c3c; color:#fff; }
        .btn-secondary { background:#6c757d; color:#fff; }
        .btn-success { background:#27ae60; color:#fff; }
        label { display:block; margin-bottom:4px; font-weight:500; }
        input[type=text], input[type=number], input[type=color], textarea, select {
            width:100%; padding:8px 10px; border:1px solid #ccc; border-radius:4px; font-size:14px; margin-bottom:12px;
        }
        textarea { min-height:120px; }
        .form-row { margin-bottom:8px; }
        .status-badge { padding:2px 8px; border-radius:10px; font-size:12px; }
        .status-success { background:#a8e6cf; color:#1b5e20; }
        .status-failed { background:#ffcdd2; color:#b71c1c; }
        .status-running { background:#ffeaa7; color:#6c5b00; }
        .status-draft { background:#ffeaa7; color:#6c5b00; }
        .status-published { background:#a8e6cf; color:#1b5e20; }
        .status-archived { background:#ddd; color:#555; }
        .actions form { display:inline; }
    </style>"#
}

pub fn admin_page(title: &str, extra_style: &str, body: &str) -> String {
    let extra_style_wrapped = if extra_style.is_empty() {
        String::new()
    } else {
        format!("<style>{extra_style}</style>")
    };
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>{title}</title>{base_style}{extra_style_wrapped}</head>
        <body>{nav}{body}</body></html>"#,
        title = html_escape(title),
        base_style = base_style(),
        extra_style_wrapped = extra_style_wrapped,
        nav = admin_nav(),
        body = body,
    )
}

pub fn admin_page_with_script(
    title: &str,
    extra_style: &str,
    body: &str,
    script: &str,
) -> String {
    let extra_style_wrapped = if extra_style.is_empty() {
        String::new()
    } else {
        format!("<style>{extra_style}</style>")
    };
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>{title}</title>{base_style}{extra_style_wrapped}</head>
        <body>{nav}{body}<script>{script}</script></body></html>"#,
        title = html_escape(title),
        base_style = base_style(),
        extra_style_wrapped = extra_style_wrapped,
        nav = admin_nav(),
        body = body,
        script = script,
    )
}
