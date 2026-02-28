use axum::extract::State;
use axum::response::Html;
use minijinja::context;
use sqlx::Row;

use crate::admin::template::render_admin;
use crate::state::AppState;

pub async fn dashboard(State(state): State<AppState>) -> Html<String> {
    let (published_posts, draft_posts) = state.posts.count_by_status().await;
    let total_posts = published_posts + draft_posts;
    let total_pages = state.pages.count_active().await;
    let total_media = state.media.count().await;

    let recent_rows = state.posts.recent(5).await;

    let posts_ctx: Vec<_> = recent_rows
        .iter()
        .map(|row| {
            let id: &str = row.get("id");
            let title: &str = row.get("title");
            let status: &str = row.get("status");
            let updated_at: &str = row.get("updated_at");

            let (badge_class, status_label) = match status {
                "published" => ("badge-success", "已发布"),
                "draft" => ("badge-warning", "草稿"),
                other => ("badge-neutral", other),
            };
            context! {
                id => id,
                title => title,
                badge_class => badge_class,
                status_label => status_label,
                updated_at => crate::admin::layout::format_datetime(updated_at),
            }
        })
        .collect();

    let sidebar_groups = crate::admin::layout::sidebar_groups_value("/admin");
    let plugin_items = crate::admin::layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin");

    let ctx = context! {
        page_title => "仪表盘",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        site_url => crate::admin::settings::get_site_url(&state).await,
        total_posts => total_posts,
        published_posts => published_posts,
        total_pages => total_pages,
        total_media => total_media,
        recent_posts => posts_ctx,
    };

    let html = render_admin(&state.admin_env, "dashboard.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}
