use axum::extract::State;
use axum::response::Html;

use crate::admin::layout::{admin_page, html_escape, PageContext};
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

    let mut post_rows = String::new();
    for p in &recent_posts {
        let (badge_class, status_label) = match p.status.as_str() {
            "published" => ("badge-success", "已发布"),
            "draft" => ("badge-warning", "草稿"),
            _ => ("badge-neutral", p.status.as_str()),
        };
        post_rows.push_str(&format!(
            r#"<tr>
                <td><a href="/admin/posts/{id}">{title}</a></td>
                <td><span class="badge {badge_class}">{status_label}</span></td>
                <td>{updated_at}</td>
            </tr>"#,
            id = p.id,
            title = html_escape(&p.title),
            badge_class = badge_class,
            status_label = status_label,
            updated_at = crate::admin::layout::format_datetime(&p.updated_at),
        ));
    }

    let body = format!(
        r#"<div class="page-header">
            <h1 class="page-title">仪表盘</h1>
        </div>
        <div class="stat-grid">
            <div class="stat-card">
                <div class="stat-value">{total_posts}</div>
                <div class="stat-label">文章总数</div>
            </div>
            <div class="stat-card">
                <div class="stat-value">{published_posts}</div>
                <div class="stat-label">已发布文章</div>
            </div>
            <div class="stat-card">
                <div class="stat-value">{total_pages}</div>
                <div class="stat-label">页面总数</div>
            </div>
            <div class="stat-card">
                <div class="stat-value">{total_media}</div>
                <div class="stat-label">媒体文件</div>
            </div>
        </div>
        <div class="card">
            <div class="card-header">
                <h2 class="card-title">最近文章</h2>
            </div>
            <table>
                <thead><tr><th>标题</th><th>状态</th><th>更新时间</th></tr></thead>
                <tbody>{post_rows}</tbody>
            </table>
        </div>"#,
        total_posts = total_posts,
        published_posts = published_posts,
        total_pages = total_pages,
        total_media = total_media,
        post_rows = post_rows,
    );

    let ctx = PageContext {
        site_title: state.config.site.title.clone(),
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    Html(admin_page("仪表盘", "/admin", &body, &ctx))
}
