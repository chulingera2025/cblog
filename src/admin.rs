use crate::state::AppState;
use axum::Router;

pub mod auth;
pub mod posts;
pub mod pages;
pub mod media;
pub mod dashboard;
pub mod build;

pub fn router(_state: AppState) -> Router {
    // TODO!!! 路由将在 Task 5 中组装
    Router::new()
}
