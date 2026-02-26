use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use sqlx::Row;
use std::sync::Arc;

use crate::admin::layout::{admin_page_with_script, format_datetime, html_escape, PageContext};
use crate::build::events::BuildEvent;
use crate::state::AppState;

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
            "success" => r#"<span class="badge badge-success">成功</span>"#,
            "failed" => r#"<span class="badge badge-danger">失败</span>"#,
            _ => r#"<span class="badge badge-warning">进行中</span>"#,
        };
        let duration = row
            .duration_ms
            .map(|d| format!("{d}ms"))
            .unwrap_or_else(|| "-".to_string());
        let finished = row.finished_at.as_deref().unwrap_or("-");
        let error_html = row
            .error
            .as_deref()
            .map(|e| format!(r#"<span class="badge badge-danger" title="{}">{}</span>"#, html_escape(e), html_escape(&e.chars().take(80).collect::<String>())))
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
            started_at = format_datetime(&row.started_at),
            trigger = html_escape(&row.trigger),
            badge = badge,
            duration = duration,
            finished = if finished == "-" { "-".to_string() } else { format_datetime(finished) },
            error_html = error_html,
        ));
    }

    let body = format!(
        r#"<div class="page-header">
            <h1 class="page-title">构建管理</h1>
            <div class="actions">
                <div id="build-status"></div>
                <button type="button" id="trigger-build-btn" class="btn btn-success">触发构建</button>
            </div>
        </div>
        <div class="table-wrapper">
            <table>
                <thead><tr><th>开始时间</th><th>触发方式</th><th>状态</th><th>耗时</th><th>完成时间</th><th>错误</th></tr></thead>
                <tbody id="build-tbody">{table_rows}</tbody>
            </table>
        </div>"#,
        table_rows = table_rows,
    );

    let script = r#"
        (function() {
            var protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
            var ws = new WebSocket(protocol + '//' + location.host + '/admin/build/ws');
            var statusEl = document.getElementById('build-status');
            var btn = document.getElementById('trigger-build-btn');
            var tbody = document.getElementById('build-tbody');

            function pad(n) { return n < 10 ? '0' + n : '' + n; }
            function fmtTime(d) {
                return d.getFullYear() + '-' + pad(d.getMonth()+1) + '-' + pad(d.getDate())
                    + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds());
            }
            function escHtml(s) {
                var d = document.createElement('div');
                d.textContent = s;
                return d.innerHTML;
            }

            btn.addEventListener('click', function() {
                btn.disabled = true;
                btn.textContent = '构建中...';
                fetch('/admin/build', { method: 'POST' }).catch(function() {
                    btn.disabled = false;
                    btn.textContent = '触发构建';
                    showToast('触发构建请求失败', 'error');
                });
            });

            ws.onmessage = function(e) {
                var event = JSON.parse(e.data);
                if (event.type === 'Started') {
                    statusEl.innerHTML = '<span class="badge badge-warning">构建中...</span>';
                    btn.disabled = true;
                    btn.textContent = '构建中...';
                } else if (event.type === 'Finished') {
                    statusEl.innerHTML = '<span class="badge badge-success">完成: '
                        + event.total_pages + ' 页, '
                        + event.rebuilt + ' 重建, '
                        + event.cached + ' 缓存, '
                        + event.total_ms + 'ms</span>';
                    btn.disabled = false;
                    btn.textContent = '触发构建';
                    var now = fmtTime(new Date());
                    var tr = document.createElement('tr');
                    tr.innerHTML = '<td>' + now + '</td>'
                        + '<td>manual</td>'
                        + '<td><span class="badge badge-success">成功</span></td>'
                        + '<td>' + event.total_ms + 'ms</td>'
                        + '<td>' + now + '</td>'
                        + '<td></td>';
                    tbody.insertBefore(tr, tbody.firstChild);
                } else if (event.type === 'Failed') {
                    statusEl.innerHTML = '<span class="badge badge-danger">失败: ' + escHtml(event.error) + '</span>';
                    btn.disabled = false;
                    btn.textContent = '触发构建';
                    var now = fmtTime(new Date());
                    var errMsg = (event.error || '').substring(0, 80);
                    var tr = document.createElement('tr');
                    tr.innerHTML = '<td>' + now + '</td>'
                        + '<td>manual</td>'
                        + '<td><span class="badge badge-danger">失败</span></td>'
                        + '<td>-</td>'
                        + '<td>' + now + '</td>'
                        + '<td><span class="badge badge-danger">' + escHtml(errMsg) + '</span></td>';
                    tbody.insertBefore(tr, tbody.firstChild);
                }
            };
        })();
    "#;

    let ctx = PageContext {
        site_title: state.config.site.title.clone(),
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    Html(admin_page_with_script("构建管理", "/admin/build", &body, script, &ctx))
}

/// 异步触发构建，立即返回 202，构建在后台执行
pub async fn trigger_build(State(state): State<AppState>) -> StatusCode {
    let _ = state.build_events.send(BuildEvent::Started {
        trigger: "manual".to_string(),
    });

    let config = Arc::clone(&state.config);

    // 预取插件配置（需要 async）
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

    // 后台执行构建，不阻塞响应
    let project_root = state.project_root.clone();
    let db = state.db.clone();
    let build_events = state.build_events.clone();

    tokio::task::spawn(async move {
        let started_at = chrono::Utc::now().to_rfc3339();

        let build_root = project_root.clone();
        let build_config = Arc::clone(&config);
        let result = tokio::task::spawn_blocking(move || {
            crate::build::run(&build_root, &build_config, false, plugin_configs, theme_saved_config)
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

        match stats {
            Some(ref s) => {
                let _ = build_events.send(BuildEvent::Finished {
                    total_ms: duration_ms.unwrap_or(0) as u64,
                    total_pages: s.total_pages,
                    rebuilt: s.rebuilt,
                    cached: s.cached,
                });
            }
            None => {
                let _ = build_events.send(BuildEvent::Failed {
                    error: error.clone().unwrap_or_default(),
                });
            }
        }

        let id = ulid::Ulid::new().to_string();
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
        .execute(&db)
        .await;
    });

    StatusCode::ACCEPTED
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
