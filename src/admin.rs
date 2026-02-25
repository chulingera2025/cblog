use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub mod auth;
pub mod build;
pub mod dashboard;
pub mod media;
pub mod pages;
pub mod posts;

pub fn router(state: AppState) -> Router {
    // 无需认证的路由
    let public_routes = Router::new()
        .route("/admin/login", get(auth::login_page).post(auth::login_submit));

    // 需要认证的路由
    let protected_routes = Router::new()
        // 仪表盘
        .route("/admin", get(dashboard::dashboard))
        // 登出
        .route("/admin/logout", post(auth::logout))
        // 修改密码
        .route("/admin/password", post(auth::change_password))
        // 文章管理
        .route("/admin/posts", get(posts::list_posts).post(posts::create_post))
        .route("/admin/posts/new", get(posts::new_post_page))
        .route("/admin/posts/{id}/edit", get(posts::edit_post_page))
        .route("/admin/posts/{id}", post(posts::update_post))
        .route("/admin/posts/{id}/delete", post(posts::delete_post))
        .route("/admin/posts/{id}/publish", post(posts::publish_post))
        .route("/admin/posts/{id}/unpublish", post(posts::unpublish_post))
        // 页面管理
        .route("/admin/pages", get(pages::list_pages).post(pages::create_page))
        .route("/admin/pages/new", get(pages::new_page_page))
        .route("/admin/pages/{id}/edit", get(pages::edit_page_page))
        .route("/admin/pages/{id}", post(pages::update_page))
        .route("/admin/pages/{id}/delete", post(pages::delete_page))
        // 媒体管理
        .route("/admin/media", get(media::list_media))
        .route("/admin/media/upload", get(media::upload_page).post(media::upload_media))
        .route("/admin/media/{id}/delete", post(media::delete_media))
        .route("/admin/api/media", get(media::api_media_list))
        // 构建管理
        .route("/admin/build", get(build::build_history).post(build::trigger_build))
        // 应用认证中间件
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    // 静态文件服务（媒体文件）
    let media_service = tower_http::services::ServeDir::new(
        state.project_root.join(&state.config.media.upload_dir),
    );

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .nest_service("/media", media_service)
        .with_state(state)
}
