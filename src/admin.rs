use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::middleware;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

/// 内嵌的默认后台 CSS
const EMBEDDED_ADMIN_CSS: &str = include_str!("../themes/aurora/assets/admin/admin.css");
/// 内嵌的默认后台 JS
const EMBEDDED_EDITOR_JS: &str = include_str!("../themes/aurora/assets/admin/editor.js");

pub mod auth;
pub mod build;
pub mod categories;
pub mod cleanup;
pub mod csrf;
pub mod dashboard;
pub mod health;
pub mod install;
pub mod layout;
pub mod media;
pub mod pages;
pub mod plugins;
pub mod posts;
pub mod profile;
pub mod settings;
pub mod tags;
pub mod template;
pub mod theme;

pub fn router(state: AppState) -> Router {
    // 安装路由（不受认证和安装检测中间件限制）
    let install_routes = Router::new()
        .route("/install", get(install::install_page).post(install::install_submit))
        .route(
            "/install/register",
            get(install::register_page).post(install::register_submit),
        );

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
        .route("/admin/posts/autosave", post(posts::autosave_create))
        .route("/admin/posts/{id}", get(posts::edit_post_page).post(posts::update_post))
        .route("/admin/posts/{id}/delete", post(posts::delete_post))
        .route("/admin/posts/{id}/publish", post(posts::publish_post))
        .route("/admin/posts/{id}/unpublish", post(posts::unpublish_post))
        .route("/admin/posts/{id}/autosave", post(posts::autosave_update))
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
        .route("/admin/api/media/upload", post(media::api_upload_media))
        // 分类管理
        .route("/admin/categories", get(categories::list_categories).post(categories::create_category))
        .route("/admin/categories/new", get(categories::new_category_page))
        .route("/admin/categories/{id}", get(categories::edit_category_page).post(categories::update_category))
        .route("/admin/categories/{id}/delete", post(categories::delete_category))
        .route("/admin/api/categories", get(categories::api_list_categories))
        // 标签管理
        .route("/admin/tags", get(tags::list_tags).post(tags::create_tag))
        .route("/admin/tags/new", get(tags::new_tag_page))
        .route("/admin/tags/{id}", get(tags::edit_tag_page).post(tags::update_tag))
        .route("/admin/tags/{id}/delete", post(tags::delete_tag))
        .route("/admin/api/tags", get(tags::api_list_tags))
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
        // 常规设置
        .route("/admin/settings", get(settings::settings_page).post(settings::save_settings))
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

    // 静态站点服务（build 输出目录作为 fallback）
    let static_site = tower_http::services::ServeDir::new(
        state.project_root.join(&state.config.build.output_dir),
    )
    .append_index_html_on_directories(true);

    // 后台静态资源路由（内嵌 + 主题目录覆盖）
    let admin_static_routes = Router::new()
        .route("/admin/static/admin.css", get(serve_admin_css))
        .route("/admin/static/editor.js", get(serve_editor_js));

    Router::new()
        .merge(install_routes)
        .merge(public_routes)
        .merge(protected_routes)
        .merge(admin_static_routes)
        .nest_service("/media", media_service)
        .fallback_service(static_site)
        // CSRF 保护中间件（在安装检测之前，确保所有表单都受保护）
        .layer(middleware::from_fn(csrf::csrf_middleware))
        // 安装检测中间件应用于所有路由
        .layer(middleware::from_fn_with_state(
            state.clone(),
            install::install_check_middleware,
        ))
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
    let active_path = format!("/admin/ext/{plugin_name}/{slug}");
    let sidebar_groups = layout::sidebar_groups_value(&active_path);
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, &active_path);

    let page_title = format!("{plugin_name} - {slug}");

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
            let ctx = minijinja::context! {
                page_title => &page_title,
                site_title => settings::get_site_title(&state).await,
                sidebar_groups => sidebar_groups,
                plugin_sidebar_items => plugin_items,
                profile_active => false,
                empty_message => format!("插件页面模板不存在：{}", template_path.display()),
            };
            return Html(
                template::render_admin(&state.admin_env, "plugin-page.cbtml", ctx)
                    .unwrap_or_else(|e| format!("模板渲染失败: {e}")),
            )
            .into_response();
        }
    };

    // 编译 CBTML → MiniJinja 模板
    let file_label = format!("plugins/{plugin_name}/admin/{slug}.cbtml");
    let compiled = match crate::cbtml::compile(&source, &file_label) {
        Ok(t) => t,
        Err(e) => {
            let ctx = minijinja::context! {
                page_title => &page_title,
                site_title => settings::get_site_title(&state).await,
                sidebar_groups => sidebar_groups,
                plugin_sidebar_items => plugin_items,
                profile_active => false,
                error_message => format!("CBTML 编译失败：{}", e),
            };
            return Html(
                template::render_admin(&state.admin_env, "plugin-page.cbtml", ctx)
                    .unwrap_or_else(|e| format!("模板渲染失败: {e}")),
            )
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
                let ctx = minijinja::context! {
                    page_title => &page_title,
                    site_title => settings::get_site_title(&state).await,
                    sidebar_groups => sidebar_groups,
                    plugin_sidebar_items => plugin_items,
                    profile_active => false,
                    error_message => format!("模板渲染失败：{}", e),
                };
                return Html(
                    template::render_admin(&state.admin_env, "plugin-page.cbtml", ctx)
                        .unwrap_or_else(|e| format!("模板渲染失败: {e}")),
                )
                .into_response();
            }
        },
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "模板加载失败").into_response();
        }
    };

    let ctx = minijinja::context! {
        page_title => &page_title,
        site_title => settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        plugin_content => rendered,
    };

    Html(
        template::render_admin(&state.admin_env, "plugin-page.cbtml", ctx)
            .unwrap_or_else(|e| format!("模板渲染失败: {e}")),
    )
    .into_response()
}

/// 提供后台 CSS：优先从主题目录加载，否则返回内嵌默认版本
async fn serve_admin_css(State(state): State<AppState>) -> Response {
    let theme_path = state
        .project_root
        .join("themes")
        .join(&state.config.theme.active)
        .join("assets/admin/admin.css");

    let content = if theme_path.exists() {
        std::fs::read_to_string(&theme_path).unwrap_or_else(|_| EMBEDDED_ADMIN_CSS.to_string())
    } else {
        EMBEDDED_ADMIN_CSS.to_string()
    };

    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], content).into_response()
}

/// 提供后台 JS：优先从主题目录加载，否则返回内嵌默认版本
async fn serve_editor_js(State(state): State<AppState>) -> Response {
    let theme_path = state
        .project_root
        .join("themes")
        .join(&state.config.theme.active)
        .join("assets/admin/editor.js");

    let content = if theme_path.exists() {
        std::fs::read_to_string(&theme_path).unwrap_or_else(|_| EMBEDDED_EDITOR_JS.to_string())
    } else {
        EMBEDDED_EDITOR_JS.to_string()
    };

    ([(header::CONTENT_TYPE, "application/javascript; charset=utf-8")], content).into_response()
}
