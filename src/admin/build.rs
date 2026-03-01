use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use minijinja::context;
use sqlx::Row;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::admin::layout::{format_datetime, html_escape};
use crate::admin::template::{build_admin_context, render_admin};
use crate::build::events::BuildEvent;
use crate::state::AppState;

pub async fn build_history(State(state): State<AppState>) -> Html<String> {
    let rows = state.builds.list_history(30).await;

    let builds: Vec<minijinja::Value> = rows
        .iter()
        .map(|row| {
            let trigger: &str = row.get("trigger");
            let status: &str = row.get("status");
            let duration_ms: Option<i64> = row.get("duration_ms");
            let error: Option<&str> = row.get("error");
            let started_at: &str = row.get("started_at");
            let finished_at: Option<&str> = row.get("finished_at");

            let duration = duration_ms
                .map(|d| format!("{d}ms"))
                .unwrap_or_else(|| "-".to_string());
            let finished = finished_at
                .map(|f| if f == "-" { "-".to_string() } else { format_datetime(f) })
                .unwrap_or_else(|| "-".to_string());
            let error_full = error.unwrap_or("");
            let error_short: String = error_full.chars().take(80).collect();
            context! {
                started_at => format_datetime(started_at),
                trigger => html_escape(trigger),
                status => status,
                duration => duration,
                finished_at => finished,
                error => if error_short.is_empty() { None } else { Some(error_short) },
                error_full => html_escape(error_full),
            }
        })
        .collect();

    let ctx = context! {
        builds => builds,
        ..build_admin_context(
            "构建管理",
            "/admin/build",
            &crate::admin::settings::get_site_title(&state).await,
            &crate::admin::settings::get_site_url(&state).await,
            &state.plugin_admin_pages,
        )
    };

    match render_admin(&state.admin_env, "build.cbtml", ctx) {
        Ok(html) => Html(html),
        Err(e) => Html(format!("模板渲染错误: {e:#}")),
    }
}

/// 核心构建逻辑：防抖 + 互斥锁 + 预取数据 + 执行构建 + 记录历史
/// 非 manual 触发会应用 2 秒防抖，manual 触发直接执行
pub async fn spawn_build(state: &AppState, trigger: &str) {
    let my_id = state.build_request_counter.fetch_add(1, Ordering::SeqCst) + 1;

    // 非手动触发时应用 2 秒防抖
    if trigger != "manual" {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let current = state.build_request_counter.load(Ordering::SeqCst);
        if current != my_id {
            return;
        }
    }

    // 获取构建互斥锁，确保同一时刻只有一个构建在执行
    let _lock = state.build_mutex.lock().await;

    let trigger_str = trigger.to_string();

    let _ = state.build_events.send(BuildEvent::Started {
        trigger: trigger_str.clone(),
    });

    // 从文件重新加载配置，确保使用最新的插件启用状态
    let config = match crate::config::SiteConfig::load(&state.project_root) {
        Ok(c) => Arc::new(c),
        Err(e) => {
            tracing::warn!("重新加载配置失败，使用缓存配置：{e}");
            Arc::clone(&state.config)
        }
    };

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
    let theme_saved_config = state.builds.load_theme_config(&config.theme.active).await;

    // 预取发布状态的文章
    use crate::build::stages::load::DbPost;

    let published_rows = state.posts.fetch_published().await;
    let db_posts: Vec<DbPost> = published_rows
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

    let project_root = state.project_root.clone();
    let build_events = state.build_events.clone();
    let site_settings = state.site_settings.read().await.clone();
    let builds_repo = state.builds.clone();

    let started_at = chrono::Utc::now().to_rfc3339();

    let build_root = project_root.clone();
    let build_config = Arc::clone(&config);
    let result = tokio::task::spawn_blocking(move || {
        crate::build::run(&build_root, &build_config, crate::build::BuildParams {
            clean: false,
            force: false,
            plugin_configs,
            theme_saved_config,
            db_posts,
            site_settings,
        })
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

    let _ = builds_repo.insert_history(&crate::repository::build::BuildHistoryParams {
        id: &id,
        trigger: &trigger_str,
        status,
        duration_ms,
        error: error.as_deref(),
        started_at: &started_at,
        finished_at: &finished_at,
        total_pages,
        rebuilt,
        cached,
    }).await;
}

/// 异步触发构建，立即返回 202，构建在后台执行
pub async fn trigger_build(State(state): State<AppState>) -> StatusCode {
    let state_clone = state.clone();
    tokio::spawn(async move {
        spawn_build(&state_clone, "manual").await;
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
