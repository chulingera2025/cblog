use axum::extract::State;
use axum::response::Json;
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let db_ok = sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .is_ok();

    let status = if db_ok { "ok" } else { "degraded" };

    Json(json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "database": if db_ok { "connected" } else { "error" },
    }))
}
