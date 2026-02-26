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

    #[derive(sqlx::FromRow)]
    struct BuildRow {
        status: String,
        started_at: String,
        finished_at: Option<String>,
    }

    let last_build: Option<BuildRow> = sqlx::query_as::<_, BuildRow>(
        "SELECT status, started_at, finished_at FROM build_history ORDER BY started_at DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

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
            updated_at = &p.updated_at[..10.min(p.updated_at.len())],
        ));
    }

    let build_section = match &last_build {
        Some(b) => {
            let (badge_class, label) = match b.status.as_str() {
                "success" => ("badge-success", "成功"),
                "failed" => ("badge-danger", "失败"),
                _ => ("badge-warning", "进行中"),
            };
            let finished = b.finished_at.as_deref().unwrap_or("-");
            format!(
                r#"<div class="card" style="margin-bottom:24px;">
                    <div class="card-header">
                        <h2 class="card-title">最近构建</h2>
                        <div>
                            <a href="/admin/build" class="btn btn-secondary btn-sm">构建历史</a>
                            <form method="POST" action="/admin/build" style="display:inline;">
                                <button type="submit" class="btn btn-success btn-sm">触发构建</button>
                            </form>
                        </div>
                    </div>
                    <div class="card-body">
                        <p>状态：<span class="badge {badge_class}">{label}</span></p>
                        <p>开始时间：{started_at}</p>
                        <p>完成时间：{finished}</p>
                    </div>
                </div>"#,
                badge_class = badge_class,
                label = label,
                started_at = html_escape(&b.started_at),
                finished = html_escape(finished),
            )
        }
        None => {
            r#"<div class="card" style="margin-bottom:24px;">
                    <div class="card-header">
                        <h2 class="card-title">最近构建</h2>
                    </div>
                    <div class="card-body">
                        <p class="empty-state">暂无构建记录</p>
                        <form method="POST" action="/admin/build">
                            <button type="submit" class="btn btn-success">触发构建</button>
                        </form>
                    </div>
                </div>"#.to_string()
        }
    };

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
        {build_section}
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
        build_section = build_section,
        post_rows = post_rows,
    );

    let ctx = PageContext {
        site_title: state.config.site.title.clone(),
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    Html(admin_page("仪表盘", "/admin", &body, &ctx))
}
