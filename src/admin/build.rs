use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use minijinja::context;
use sqlx::Row;
use std::sync::Arc;

use crate::admin::layout::{format_datetime, html_escape};
use crate::admin::template::{build_admin_context, render_admin};
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

    let builds: Vec<minijinja::Value> = rows
        .iter()
        .map(|row| {
            let duration = row
                .duration_ms
                .map(|d| format!("{d}ms"))
                .unwrap_or_else(|| "-".to_string());
            let finished = row
                .finished_at
                .as_deref()
                .map(|f| if f == "-" { "-".to_string() } else { format_datetime(f) })
                .unwrap_or_else(|| "-".to_string());
            let error_full = row.error.as_deref().unwrap_or("");
            let error_short: String = error_full.chars().take(80).collect();
            context! {
                started_at => format_datetime(&row.started_at),
                trigger => html_escape(&row.trigger),
                status => &row.status,
                duration => duration,
                finished_at => finished,
                error => if error_short.is_empty() { None } else { Some(error_short) },
                error_full => html_escape(error_full),
            }
        })
        .collect();

    let base = build_admin_context(
        "构建管理",
        "/admin/build",
        &state.config.site.title,
        &state.plugin_admin_pages,
    );

    let ctx = context! {
        builds => builds,
        ..base
    };

    match render_admin(&state.admin_env, "build.cbtml", ctx) {
        Ok(html) => Html(html),
        Err(e) => Html(format!("模板渲染错误: {e:#}")),
    }
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

    // 预取发布状态的文章
    use crate::build::stages::load::DbPost;

    let db_posts: Vec<DbPost> = sqlx::query(
        "SELECT id, slug, title, content, status, created_at, updated_at, meta FROM posts WHERE status = 'published'"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| {
        let meta_str: String = row.get("meta");
        let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();
        DbPost {
            id: row.get("id"),
            slug: row.get("slug"),
            title: row.get("title"),
            content: row.get("content"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            meta,
        }
    })
    .collect();

    // 后台执行构建，不阻塞响应
    let project_root = state.project_root.clone();
    let db = state.db.clone();
    let build_events = state.build_events.clone();

    tokio::task::spawn(async move {
        let started_at = chrono::Utc::now().to_rfc3339();

        let build_root = project_root.clone();
        let build_config = Arc::clone(&config);
        let result = tokio::task::spawn_blocking(move || {
            crate::build::run(&build_root, &build_config, false, plugin_configs, theme_saved_config, db_posts)
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
