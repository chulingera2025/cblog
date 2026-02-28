use axum::extract::State;
use axum::response::Html;
use minijinja::context;

use crate::admin::template::render_admin;
use crate::state::AppState;

pub async fn dashboard(State(state): State<AppState>) -> Html<String> {
    #[derive(sqlx::FromRow)]
    struct DashboardCounts {
        published_posts: i64,
        draft_posts: i64,
        total_pages: i64,
        total_media: i64,
    }

    let counts = sqlx::query_as::<_, DashboardCounts>(
        r#"SELECT
            (SELECT COUNT(*) FROM posts WHERE status = 'published') as published_posts,
            (SELECT COUNT(*) FROM posts WHERE status = 'draft') as draft_posts,
            (SELECT COUNT(*) FROM pages WHERE status != 'archived') as total_pages,
            (SELECT COUNT(*) FROM media) as total_media"#,
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(DashboardCounts {
        published_posts: 0,
        draft_posts: 0,
        total_pages: 0,
        total_media: 0,
    });

    let total_posts = counts.published_posts + counts.draft_posts;

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
        published_posts => counts.published_posts,
        total_pages => counts.total_pages,
        total_media => counts.total_media,
        recent_posts => posts_ctx,
    };

    let html = render_admin(&state.admin_env, "dashboard.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}
