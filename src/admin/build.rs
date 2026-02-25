use axum::extract::State;
use axum::response::{Html, Redirect};
use std::sync::Arc;

use crate::state::AppState;

fn admin_nav() -> String {
    r#"<nav style="background:#1a1a2e;padding:12px 24px;display:flex;gap:24px;align-items:center;">
        <a href="/admin" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">仪表盘</a>
        <a href="/admin/posts" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">文章</a>
        <a href="/admin/pages" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">页面</a>
        <a href="/admin/media" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">媒体</a>
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
        .btn-success { background:#27ae60; color:#fff; }
        .status-badge { padding:2px 8px; border-radius:10px; font-size:12px; }
        .status-success { background:#a8e6cf; color:#1b5e20; }
        .status-failed { background:#ffcdd2; color:#b71c1c; }
        .status-running { background:#ffeaa7; color:#6c5b00; }
        .error-text { color:#c62828; font-size:12px; }
    </style>"#
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub async fn build_history(State(state): State<AppState>) -> Html<String> {
    #[derive(sqlx::FromRow)]
    struct BuildRow {
        trigger: String,
        status: String,
        duration_ms: Option<i64>,
        error: Option<String>,
        started_at: String,
        finished_at: Option<String>,
    }

    let rows: Vec<BuildRow> = sqlx::query_as::<_, BuildRow>(
        "SELECT trigger, status, duration_ms, error, started_at, finished_at FROM build_history ORDER BY started_at DESC LIMIT 30",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut table_rows = String::new();
    for row in &rows {
        let badge = match row.status.as_str() {
            "success" => r#"<span class="status-badge status-success">成功</span>"#,
            "failed" => r#"<span class="status-badge status-failed">失败</span>"#,
            _ => r#"<span class="status-badge status-running">进行中</span>"#,
        };
        let duration = row
            .duration_ms
            .map(|d| format!("{d}ms"))
            .unwrap_or_else(|| "-".to_string());
        let finished = row.finished_at.as_deref().unwrap_or("-");
        let error_html = row
            .error
            .as_deref()
            .map(|e| format!(r#"<span class="error-text" title="{}">{}</span>"#, html_escape(e), html_escape(&e.chars().take(80).collect::<String>())))
            .unwrap_or_default();

        table_rows.push_str(&format!(
            r#"<tr>
                <td>{started_at}</td>
                <td>{trigger}</td>
                <td>{badge}</td>
                <td>{duration}</td>
                <td>{finished}</td>
                <td>{error_html}</td>
            </tr>"#,
            started_at = &row.started_at[..19.min(row.started_at.len())],
            trigger = html_escape(&row.trigger),
            badge = badge,
            duration = duration,
            finished = &finished[..19.min(finished.len())],
            error_html = error_html,
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>构建历史</title>{style}</head>
        <body>{nav}
        <div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;">
                <h1>构建历史</h1>
                <form method="POST" action="/admin/build">
                    <button type="submit" class="btn btn-success">触发构建</button>
                </form>
            </div>
            <table>
                <thead><tr><th>开始时间</th><th>触发方式</th><th>状态</th><th>耗时</th><th>完成时间</th><th>错误</th></tr></thead>
                <tbody>{table_rows}</tbody>
            </table>
        </div></body></html>"#,
        style = page_style(),
        nav = admin_nav(),
        table_rows = table_rows,
    );

    Html(html)
}

pub async fn trigger_build(State(state): State<AppState>) -> Redirect {
    let id = ulid::Ulid::new().to_string();
    let started_at = chrono::Utc::now().to_rfc3339();

    let project_root = state.project_root.clone();
    let config = Arc::clone(&state.config);

    let result = tokio::task::spawn_blocking(move || {
        crate::build::run(&project_root, &config, false)
    })
    .await;

    let finished_at = chrono::Utc::now().to_rfc3339();
    let start_time = chrono::DateTime::parse_from_rfc3339(&started_at).ok();
    let duration_ms = start_time.map(|s| {
        (chrono::Utc::now() - s.with_timezone(&chrono::Utc)).num_milliseconds()
    });

    let (status, error) = match result {
        Ok(Ok(())) => ("success", None),
        Ok(Err(e)) => ("failed", Some(format!("{e:#}"))),
        Err(e) => ("failed", Some(format!("任务执行异常: {e}"))),
    };

    let _ = sqlx::query(
        "INSERT INTO build_history (id, trigger, status, duration_ms, error, started_at, finished_at) VALUES (?, 'manual', ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(status)
    .bind(duration_ms)
    .bind(error.as_deref())
    .bind(&started_at)
    .bind(&finished_at)
    .execute(&state.db)
    .await;

    Redirect::to("/admin/build")
}
