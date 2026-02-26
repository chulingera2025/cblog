use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{Html, Redirect};
use sqlx::Row;
use std::sync::Arc;

use crate::admin::layout::{admin_page_with_script, html_escape};
use crate::build::events::BuildEvent;
use crate::state::AppState;

const EXTRA_STYLE: &str = r#"
    .error-text { color:#c62828; font-size:12px; }
"#;

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

    let body = format!(
        r#"<div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;">
                <h1>构建历史</h1>
                <div style="display:flex;gap:12px;align-items:center;">
                    <div id="build-status"></div>
                    <form method="POST" action="/admin/build">
                        <button type="submit" class="btn btn-success">触发构建</button>
                    </form>
                </div>
            </div>
            <table>
                <thead><tr><th>开始时间</th><th>触发方式</th><th>状态</th><th>耗时</th><th>完成时间</th><th>错误</th></tr></thead>
                <tbody>{table_rows}</tbody>
            </table>
        </div>"#,
        table_rows = table_rows,
    );

    let script = r#"
        (function() {
            var protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
            var ws = new WebSocket(protocol + '//' + location.host + '/admin/build/ws');
            var el = document.getElementById('build-status');
            ws.onmessage = function(e) {
                var event = JSON.parse(e.data);
                if (event.type === 'Started') {
                    el.innerHTML = '<span class="status-badge status-running">构建中...</span>';
                } else if (event.type === 'Finished') {
                    el.innerHTML = '<span class="status-badge status-success">完成: '
                        + event.total_pages + '页, '
                        + event.rebuilt + '重建, '
                        + event.cached + '缓存, '
                        + event.total_ms + 'ms</span>';
                    setTimeout(function() { location.reload(); }, 1500);
                } else if (event.type === 'Failed') {
                    el.innerHTML = '<span class="status-badge status-failed">失败: ' + event.error + '</span>';
                    setTimeout(function() { location.reload(); }, 1500);
                }
            };
        })();
    "#;

    Html(admin_page_with_script("构建历史", EXTRA_STYLE, &body, script))
}

pub async fn trigger_build(State(state): State<AppState>) -> Redirect {
    let id = ulid::Ulid::new().to_string();
    let started_at = chrono::Utc::now().to_rfc3339();

    let _ = state.build_events.send(BuildEvent::Started {
        trigger: "manual".to_string(),
    });

    let project_root = state.project_root.clone();
    let config = Arc::clone(&state.config);

    // 预取插件配置
    let mut plugin_configs = std::collections::HashMap::new();
    for name in &config.plugins.enabled {
        if let Ok(cfg) = crate::plugin::store::PluginStore::get_all(&state.db, name).await
            && !cfg.is_empty()
        {
            plugin_configs.insert(name.clone(), cfg);
        }
    }

    // 预取主题配置
    let theme_saved_config: std::collections::HashMap<String, serde_json::Value> =
        sqlx::query("SELECT config FROM theme_config WHERE theme_name = ?")
            .bind(&config.theme.active)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .and_then(|row| {
                let json_str: String = row.get("config");
                serde_json::from_str(&json_str).ok()
            })
            .unwrap_or_default();

    let result = tokio::task::spawn_blocking(move || {
        crate::build::run(&project_root, &config, false, plugin_configs, theme_saved_config)
    })
    .await;

    let finished_at = chrono::Utc::now().to_rfc3339();
    let start_time = chrono::DateTime::parse_from_rfc3339(&started_at).ok();
    let duration_ms = start_time.map(|s| {
        (chrono::Utc::now() - s.with_timezone(&chrono::Utc)).num_milliseconds()
    });

    let (status, error, stats) = match &result {
        Ok(Ok(stats)) => ("success", None, Some(stats.clone())),
        Ok(Err(e)) => ("failed", Some(format!("{e:#}")), None),
        Err(e) => ("failed", Some(format!("任务执行异常: {e}")), None),
    };

    // 广播构建结果事件
    match stats {
        Some(ref s) => {
            let _ = state.build_events.send(BuildEvent::Finished {
                total_ms: duration_ms.unwrap_or(0) as u64,
                total_pages: s.total_pages,
                rebuilt: s.rebuilt,
                cached: s.cached,
            });
        }
        None => {
            let _ = state.build_events.send(BuildEvent::Failed {
                error: error.clone().unwrap_or_default(),
            });
        }
    }

    let total_pages = stats.as_ref().map(|s| s.total_pages as i64);
    let rebuilt = stats.as_ref().map(|s| s.rebuilt as i64);
    let cached = stats.as_ref().map(|s| s.cached as i64);

    let _ = sqlx::query(
        "INSERT INTO build_history (id, trigger, status, duration_ms, error, started_at, finished_at, total_pages, rebuilt, cached) VALUES (?, 'manual', ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(status)
    .bind(duration_ms)
    .bind(error.as_deref())
    .bind(&started_at)
    .bind(&finished_at)
    .bind(total_pages)
    .bind(rebuilt)
    .bind(cached)
    .execute(&state.db)
    .await;

    Redirect::to("/admin/build")
}

pub async fn build_status_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut rx = state.build_events.subscribe();
    while let Ok(event) = rx.recv().await {
        let json = serde_json::to_string(&event).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
