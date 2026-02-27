use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub mod auth;
pub mod build;
pub mod cleanup;
pub mod dashboard;
pub mod health;
pub mod layout;
pub mod media;
pub mod pages;
pub mod plugins;
pub mod posts;
pub mod profile;
pub mod template;
pub mod theme;

pub fn router(state: AppState) -> Router {
    // 无需认证的路由
    let public_routes = Router::new()
        .route("/admin/login", get(auth::login_page).post(auth::login_submit))
        .route("/health", get(health::health_check));

    // 需要认证的路由
    let protected_routes = Router::new()
        // 仪表盘
        .route("/admin", get(dashboard::dashboard))
        // 登出
        .route("/admin/logout", post(auth::logout))
        // 个人资料
        .route("/admin/profile", get(profile::profile_page))
        .route("/admin/profile/password", post(auth::change_password))
        // 文章管理
        .route("/admin/posts", get(posts::list_posts).post(posts::create_post))
        .route("/admin/posts/new", get(posts::new_post_page))
        .route("/admin/posts/{id}", get(posts::edit_post_page).post(posts::update_post))
        .route("/admin/posts/{id}/delete", post(posts::delete_post))
        .route("/admin/posts/{id}/publish", post(posts::publish_post))
        .route("/admin/posts/{id}/unpublish", post(posts::unpublish_post))
        // 页面管理
        .route("/admin/pages", get(pages::list_pages).post(pages::create_page))
        .route("/admin/pages/new", get(pages::new_page_page))
        .route("/admin/pages/{id}", get(pages::edit_page_page).post(pages::update_page))
        .route("/admin/pages/{id}/delete", post(pages::delete_page))
        // 媒体管理
        .route("/admin/media", get(media::list_media))
        .route("/admin/media/upload", get(media::upload_page).post(media::upload_media))
        .route("/admin/media/{id}/delete", post(media::delete_media))
        .route("/admin/api/media", get(media::api_media_list))
        // 构建管理
        .route("/admin/build/ws", get(build::build_status_ws))
        .route("/admin/build", get(build::build_history).post(build::trigger_build))
        // 插件管理
        .route("/admin/plugins", get(plugins::list_plugins))
        .route("/admin/plugins/toggle", post(plugins::toggle_plugin))
        .route("/admin/plugins/{name}", get(plugins::plugin_detail))
        .route("/admin/plugins/{name}/config", post(plugins::save_plugin_config))
        // 主题管理
        .route("/admin/theme", get(theme::theme_settings).post(theme::save_theme_settings))
        .route("/admin/theme/switch", post(theme::switch_theme))
        // 插件自定义后台页面
        .route("/admin/ext/{plugin}/{slug}", get(plugin_admin_page))
        // 废弃路由重定向
        .route("/admin/posts/{id}/edit", get(redirect_post_edit))
        .route("/admin/pages/{id}/edit", get(redirect_page_edit))
        .route("/admin/password", post(auth::change_password))
        // 应用认证中间件
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    // 静态文件服务（媒体文件）
    let media_service = tower_http::services::ServeDir::new(
        state.project_root.join(&state.config.media.upload_dir),
    );

    // 后台静态资源（CSS 等）
    let admin_static = tower_http::services::ServeDir::new(
        state.project_root.join("admin/static"),
    );

    // 静态站点服务（build 输出目录作为 fallback）
    let static_site = tower_http::services::ServeDir::new(
        state.project_root.join(&state.config.build.output_dir),
    )
    .append_index_html_on_directories(true);

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .nest_service("/media", media_service)
        .nest_service("/admin/static", admin_static)
        .fallback_service(static_site)
        .with_state(state)
}

/// 旧路由 /admin/posts/{id}/edit → 301 重定向到 /admin/posts/{id}
async fn redirect_post_edit(Path(id): Path<String>) -> Redirect {
    Redirect::permanent(&format!("/admin/posts/{id}"))
}

/// 旧路由 /admin/pages/{id}/edit → 301 重定向到 /admin/pages/{id}
async fn redirect_page_edit(Path(id): Path<String>) -> Redirect {
    Redirect::permanent(&format!("/admin/pages/{id}"))
}

/// 插件自定义后台页面：加载 CBTML 模板 → 编译 → MiniJinja 渲染 → 包裹在 admin 布局中
async fn plugin_admin_page(
    State(state): State<AppState>,
    Path((plugin_name, slug)): Path<(String, String)>,
) -> Response {
    let ctx = layout::PageContext {
        site_title: state.config.site.title.clone(),
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    let active_path = format!("/admin/ext/{plugin_name}/{slug}");

    // 加载 CBTML 模板
    let template_path = state
        .project_root
        .join("plugins")
        .join(&plugin_name)
        .join("admin")
        .join(format!("{slug}.cbtml"));

    let source = match std::fs::read_to_string(&template_path) {
        Ok(s) => s,
        Err(_) => {
            let body = format!(
                r#"<div class="empty-state"><p>插件页面模板不存在：{}</p></div>"#,
                layout::html_escape(&template_path.display().to_string()),
            );
            return Html(layout::admin_page(
                "页面未找到",
                &active_path,
                &body,
                &ctx,
            ))
            .into_response();
        }
    };

    // 编译 CBTML → MiniJinja 模板
    let file_label = format!("plugins/{plugin_name}/admin/{slug}.cbtml");
    let compiled = match crate::cbtml::compile(&source, &file_label) {
        Ok(t) => t,
        Err(e) => {
            let body = format!(
                r#"<div class="alert alert-error">CBTML 编译失败：{}</div>"#,
                layout::html_escape(&e.to_string()),
            );
            return Html(layout::admin_page("编译错误", &active_path, &body, &ctx))
                .into_response();
        }
    };

    // 构建 MiniJinja 渲染环境
    let mut env = minijinja::Environment::new();
    crate::cbtml::filters::register_filters(&mut env, &state.config.site.url);
    env.add_template("page", &compiled).ok();

    // 构建渲染上下文
    let plugin_config = crate::plugin::store::PluginStore::get_all(&state.db, &plugin_name)
        .await
        .unwrap_or_default();

    let render_ctx = minijinja::context! {
        plugin_name => &plugin_name,
        plugin_config => &plugin_config,
        site => minijinja::context! {
            title => &state.config.site.title,
            url => &state.config.site.url,
            description => &state.config.site.description,
        },
    };

    let rendered = match env.get_template("page") {
        Ok(tmpl) => match tmpl.render(&render_ctx) {
            Ok(html) => html,
            Err(e) => {
                let body = format!(
                    r#"<div class="alert alert-error">模板渲染失败：{}</div>"#,
                    layout::html_escape(&e.to_string()),
                );
                return Html(layout::admin_page("渲染错误", &active_path, &body, &ctx))
                    .into_response();
            }
        },
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "模板加载失败").into_response();
        }
    };

    Html(layout::admin_page(
        &format!("{plugin_name} - {slug}"),
        &active_path,
        &rendered,
        &ctx,
    ))
    .into_response()
}
