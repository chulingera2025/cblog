use axum::extract::State;
use axum::response::Html;
use minijinja::context;

use crate::admin::template::render_admin;
use crate::state::AppState;

pub async fn dashboard(State(state): State<AppState>) -> Html<String> {
    #[derive(sqlx::FromRow)]
    struct CountRow {
        count: i64,
    }

    let total_posts = sqlx::query_as::<_, CountRow>(
        "SELECT COUNT(*) as count FROM posts WHERE status != 'archived'",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.count)
    .unwrap_or(0);

    let published_posts = sqlx::query_as::<_, CountRow>(
        "SELECT COUNT(*) as count FROM posts WHERE status = 'published'",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.count)
    .unwrap_or(0);

    let total_pages =
        sqlx::query_as::<_, CountRow>("SELECT COUNT(*) as count FROM pages")
            .fetch_one(&state.db)
            .await
            .map(|r| r.count)
            .unwrap_or(0);

    let total_media =
        sqlx::query_as::<_, CountRow>("SELECT COUNT(*) as count FROM media")
            .fetch_one(&state.db)
            .await
            .map(|r| r.count)
            .unwrap_or(0);

    #[derive(sqlx::FromRow)]
    struct RecentPost {
        id: String,
        title: String,
        status: String,
        updated_at: String,
    }

    let recent_posts: Vec<RecentPost> = sqlx::query_as::<_, RecentPost>(
        r#"SELECT id, title, status, updated_at FROM posts
           WHERE status != 'archived'
           ORDER BY updated_at DESC LIMIT 5"#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // 构建模板需要的文章列表，预处理状态标签和 badge 样式类名
    let posts_ctx: Vec<_> = recent_posts
        .iter()
        .map(|p| {
            let (badge_class, status_label) = match p.status.as_str() {
                "published" => ("badge-success", "已发布"),
                "draft" => ("badge-warning", "草稿"),
                _ => ("badge-neutral", p.status.as_str()),
            };
            context! {
                id => p.id,
                title => p.title,
                badge_class => badge_class,
                status_label => status_label,
                updated_at => crate::admin::layout::format_datetime(&p.updated_at),
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
