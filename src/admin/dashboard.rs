use axum::extract::State;
use axum::response::Html;

use crate::admin::layout::{admin_page, html_escape};
use crate::state::AppState;

const EXTRA_STYLE: &str = r#"
    .stat-grid { display:grid; grid-template-columns:repeat(4,1fr); gap:16px; margin-bottom:24px; }
    .stat-card { background:#fff; padding:20px; border-radius:8px; box-shadow:0 1px 3px rgba(0,0,0,0.1); text-align:center; }
    .stat-card .number { font-size:32px; font-weight:bold; color:#4a6cf7; }
    .stat-card .label { font-size:14px; color:#666; margin-top:4px; }
"#;

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
        let badge_class = match p.status.as_str() {
            "published" => "status-success",
            "draft" => "status-running",
            _ => "",
        };
        let status_label = match p.status.as_str() {
            "published" => "已发布",
            "draft" => "草稿",
            _ => &p.status,
        };
        post_rows.push_str(&format!(
            r#"<tr>
                <td><a href="/admin/posts/{id}/edit">{title}</a></td>
                <td><span class="status-badge {badge_class}">{status_label}</span></td>
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
                "success" => ("status-success", "成功"),
                "failed" => ("status-failed", "失败"),
                _ => ("status-running", "进行中"),
            };
            let finished = b.finished_at.as_deref().unwrap_or("-");
            format!(
                r#"<div style="background:#fff;padding:16px;border-radius:8px;box-shadow:0 1px 3px rgba(0,0,0,0.1);margin-bottom:24px;">
                    <h2 style="margin-bottom:12px;">最近构建</h2>
                    <p>状态：<span class="status-badge {badge_class}">{label}</span></p>
                    <p style="margin-top:8px;">开始时间：{started_at}</p>
                    <p>完成时间：{finished}</p>
                    <div style="margin-top:12px;">
                        <a href="/admin/build" class="btn btn-primary" style="margin-right:8px;">构建历史</a>
                        <form method="POST" action="/admin/build" style="display:inline;">
                            <button type="submit" class="btn btn-success">触发构建</button>
                        </form>
                    </div>
                </div>"#,
                badge_class = badge_class,
                label = label,
                started_at = html_escape(&b.started_at),
                finished = html_escape(finished),
            )
        }
        None => {
            r#"<div style="background:#fff;padding:16px;border-radius:8px;box-shadow:0 1px 3px rgba(0,0,0,0.1);margin-bottom:24px;">
                    <h2 style="margin-bottom:12px;">最近构建</h2>
                    <p style="color:#999;">暂无构建记录</p>
                    <div style="margin-top:12px;">
                        <form method="POST" action="/admin/build" style="display:inline;">
                            <button type="submit" class="btn btn-success">触发构建</button>
                        </form>
                    </div>
                </div>"#.to_string()
        }
    };

    let body = format!(
        r#"<div class="container">
            <h1>仪表盘</h1>
            <div class="stat-grid">
                <div class="stat-card">
                    <div class="number">{total_posts}</div>
                    <div class="label">文章总数</div>
                </div>
                <div class="stat-card">
                    <div class="number">{published_posts}</div>
                    <div class="label">已发布文章</div>
                </div>
                <div class="stat-card">
                    <div class="number">{total_pages}</div>
                    <div class="label">页面总数</div>
                </div>
                <div class="stat-card">
                    <div class="number">{total_media}</div>
                    <div class="label">媒体文件</div>
                </div>
            </div>
            {build_section}
            <div style="background:#fff;padding:16px;border-radius:8px;box-shadow:0 1px 3px rgba(0,0,0,0.1);">
                <h2 style="margin-bottom:12px;">最近文章</h2>
                <table>
                    <thead><tr><th>标题</th><th>状态</th><th>更新时间</th></tr></thead>
                    <tbody>{post_rows}</tbody>
                </table>
            </div>
        </div>"#,
        total_posts = total_posts,
        published_posts = published_posts,
        total_pages = total_pages,
        total_media = total_media,
        build_section = build_section,
        post_rows = post_rows,
    );

    Html(admin_page("仪表盘", EXTRA_STYLE, &body))
}
