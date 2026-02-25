use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, Redirect};
use serde::Deserialize;
use sqlx::Row;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct PostForm {
    pub title: String,
    pub slug: Option<String>,
    pub content: String,
    pub status: Option<String>,
    pub tags: Option<String>,
    pub category: Option<String>,
    pub cover_image: Option<String>,
    pub excerpt: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub status: Option<String>,
    pub search: Option<String>,
}

fn generate_slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn admin_nav() -> String {
    r#"<nav style="background:#1a1a2e;padding:12px 24px;display:flex;gap:24px;align-items:center;">
        <a href="/admin" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">仪表盘</a>
        <a href="/admin/posts" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">文章</a>
        <a href="/admin/pages" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">页面</a>
        <a href="/admin/media" style="color:#e0e0e0;text-decoration:none;font-weight:bold;">媒体</a>
    </nav>"#
        .to_string()
}

fn page_style() -> &'static str {
    r#"<style>
        * { margin:0; padding:0; box-sizing:border-box; }
        body { font-family:system-ui,-apple-system,sans-serif; background:#f5f5f5; color:#333; }
        .container { max-width:1000px; margin:24px auto; padding:0 16px; }
        h1 { margin-bottom:16px; }
        table { width:100%; border-collapse:collapse; background:#fff; border-radius:4px; overflow:hidden; box-shadow:0 1px 3px rgba(0,0,0,0.1); }
        th,td { padding:10px 14px; text-align:left; border-bottom:1px solid #eee; }
        th { background:#f8f8f8; font-weight:600; }
        a { color:#4a6cf7; text-decoration:none; }
        a:hover { text-decoration:underline; }
        .btn { display:inline-block; padding:6px 14px; border-radius:4px; border:none; cursor:pointer; font-size:14px; text-decoration:none; }
        .btn-primary { background:#4a6cf7; color:#fff; }
        .btn-danger { background:#e74c3c; color:#fff; }
        .btn-secondary { background:#6c757d; color:#fff; }
        .btn-success { background:#27ae60; color:#fff; }
        label { display:block; margin-bottom:4px; font-weight:500; }
        input[type=text], textarea, select { width:100%; padding:8px 10px; border:1px solid #ccc; border-radius:4px; font-size:14px; margin-bottom:12px; }
        textarea { min-height:300px; font-family:monospace; }
        .form-row { margin-bottom:8px; }
        .status-badge { padding:2px 8px; border-radius:10px; font-size:12px; }
        .status-draft { background:#ffeaa7; color:#6c5b00; }
        .status-published { background:#a8e6cf; color:#1b5e20; }
        .status-archived { background:#ddd; color:#555; }
        .actions form { display:inline; }
        .filter-bar { margin-bottom:16px; display:flex; gap:12px; align-items:center; }
    </style>"#
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub async fn list_posts(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page: i32 = 20;
    let offset = (page as i32 - 1) * per_page;

    let rows = match (params.status.as_deref(), params.search.as_deref()) {
        (Some(status), Some(search)) => {
            let pattern = format!("%{search}%");
            sqlx::query(
                "SELECT id, title, status, created_at, updated_at FROM posts \
                 WHERE status != 'archived' AND status = ? AND title LIKE ? \
                 ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(&pattern)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
        }
        (Some(status), None) => {
            sqlx::query(
                "SELECT id, title, status, created_at, updated_at FROM posts \
                 WHERE status != 'archived' AND status = ? \
                 ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
        }
        (None, Some(search)) => {
            let pattern = format!("%{search}%");
            sqlx::query(
                "SELECT id, title, status, created_at, updated_at FROM posts \
                 WHERE status != 'archived' AND title LIKE ? \
                 ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(&pattern)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
        }
        (None, None) => {
            sqlx::query(
                "SELECT id, title, status, created_at, updated_at FROM posts \
                 WHERE status != 'archived' \
                 ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(per_page)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
        }
    };

    let mut table_rows = String::new();
    for row in &rows {
        let id: &str = row.get("id");
        let title: &str = row.get("title");
        let status: &str = row.get("status");
        let created_at: &str = row.get("created_at");
        let updated_at: &str = row.get("updated_at");

        let badge_class = match status {
            "published" => "status-published",
            "draft" => "status-draft",
            _ => "status-archived",
        };
        let status_label = match status {
            "published" => "已发布",
            "draft" => "草稿",
            other => other,
        };
        table_rows.push_str(&format!(
            r#"<tr>
                <td><a href="/admin/posts/{id}/edit">{title}</a></td>
                <td><span class="status-badge {badge_class}">{status_label}</span></td>
                <td>{created_at}</td>
                <td>{updated_at}</td>
                <td class="actions">
                    <a href="/admin/posts/{id}/edit" class="btn btn-secondary" style="padding:2px 8px;font-size:12px;">编辑</a>
                    <form method="POST" action="/admin/posts/{id}/delete" style="display:inline;" onsubmit="return confirm('确定删除？')">
                        <button type="submit" class="btn btn-danger" style="padding:2px 8px;font-size:12px;">删除</button>
                    </form>
                </td>
            </tr>"#,
            id = html_escape(id),
            title = html_escape(title),
            badge_class = badge_class,
            status_label = status_label,
            created_at = &created_at[..10.min(created_at.len())],
            updated_at = &updated_at[..10.min(updated_at.len())],
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>文章管理</title>{style}</head>
        <body>{nav}
        <div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;">
                <h1>文章管理</h1>
                <a href="/admin/posts/new" class="btn btn-primary">新建文章</a>
            </div>
            <div class="filter-bar">
                <form method="GET" action="/admin/posts" style="display:flex;gap:8px;align-items:center;">
                    <select name="status" style="width:auto;margin-bottom:0;">
                        <option value="">全部状态</option>
                        <option value="draft" {sel_draft}>草稿</option>
                        <option value="published" {sel_pub}>已发布</option>
                    </select>
                    <input type="text" name="search" placeholder="搜索标题..." value="{search_val}" style="width:200px;margin-bottom:0;">
                    <button type="submit" class="btn btn-primary" style="padding:6px 12px;">筛选</button>
                </form>
            </div>
            <table>
                <thead><tr><th>标题</th><th>状态</th><th>创建时间</th><th>更新时间</th><th>操作</th></tr></thead>
                <tbody>{table_rows}</tbody>
            </table>
            <div style="margin-top:16px;display:flex;gap:8px;">
                {pagination}
            </div>
        </div></body></html>"#,
        style = page_style(),
        nav = admin_nav(),
        table_rows = table_rows,
        sel_draft = if params.status.as_deref() == Some("draft") { "selected" } else { "" },
        sel_pub = if params.status.as_deref() == Some("published") { "selected" } else { "" },
        search_val = html_escape(params.search.as_deref().unwrap_or("")),
        pagination = {
            let mut p = String::new();
            if page > 1 {
                p.push_str(&format!(
                    r#"<a href="/admin/posts?page={}" class="btn btn-secondary">上一页</a>"#,
                    page - 1
                ));
            }
            if rows.len() as i32 == per_page {
                p.push_str(&format!(
                    r#"<a href="/admin/posts?page={}" class="btn btn-secondary">下一页</a>"#,
                    page + 1
                ));
            }
            p
        },
    );

    Html(html)
}

pub async fn new_post_page() -> Html<String> {
    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>新建文章</title>{style}</head>
        <body>{nav}
        <div class="container">
            <h1>新建文章</h1>
            <form method="POST" action="/admin/posts">
                <div class="form-row">
                    <label>标题</label>
                    <input type="text" name="title" required>
                </div>
                <div class="form-row">
                    <label>Slug（留空自动生成）</label>
                    <input type="text" name="slug">
                </div>
                <div class="form-row">
                    <label>内容</label>
                    <textarea name="content"></textarea>
                </div>
                <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;">
                    <div class="form-row">
                        <label>状态</label>
                        <select name="status">
                            <option value="draft">草稿</option>
                            <option value="published">已发布</option>
                        </select>
                    </div>
                    <div class="form-row">
                        <label>分类</label>
                        <input type="text" name="category">
                    </div>
                    <div class="form-row">
                        <label>标签（逗号分隔）</label>
                        <input type="text" name="tags">
                    </div>
                    <div class="form-row">
                        <label>封面图 URL</label>
                        <input type="text" name="cover_image">
                    </div>
                </div>
                <div class="form-row">
                    <label>摘要</label>
                    <input type="text" name="excerpt">
                </div>
                <div style="margin-top:16px;">
                    <button type="submit" class="btn btn-primary">创建文章</button>
                    <a href="/admin/posts" class="btn btn-secondary" style="margin-left:8px;">取消</a>
                </div>
            </form>
        </div></body></html>"#,
        style = page_style(),
        nav = admin_nav(),
    );
    Html(html)
}

pub async fn create_post(
    State(state): State<AppState>,
    Form(form): Form<PostForm>,
) -> Redirect {
    let id = ulid::Ulid::new().to_string();
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.title),
    };
    let status = form.status.as_deref().unwrap_or("draft");
    let now = chrono::Utc::now().to_rfc3339();

    let meta = serde_json::json!({
        "tags": form.tags.as_deref().unwrap_or(""),
        "category": form.category.as_deref().unwrap_or(""),
        "cover_image": form.cover_image.as_deref().unwrap_or(""),
        "excerpt": form.excerpt.as_deref().unwrap_or(""),
    })
    .to_string();

    let _ = sqlx::query(
        "INSERT INTO posts (id, slug, title, content, status, created_at, updated_at, meta) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&slug)
    .bind(&form.title)
    .bind(&form.content)
    .bind(status)
    .bind(&now)
    .bind(&now)
    .bind(&meta)
    .execute(&state.db)
    .await;

    Redirect::to(&format!("/admin/posts/{id}/edit"))
}

pub async fn edit_post_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Html<String> {
    let row = sqlx::query(
        "SELECT id, slug, title, content, status, meta FROM posts WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some(post) = row else {
        return Html("<h1>文章不存在</h1>".to_string());
    };

    let post_id: &str = post.get("id");
    let post_slug: &str = post.get("slug");
    let post_title: &str = post.get("title");
    let post_content: &str = post.get("content");
    let post_status: &str = post.get("status");
    let post_meta: &str = post.get("meta");

    let meta: serde_json::Value = serde_json::from_str(post_meta).unwrap_or_default();
    let tags = meta["tags"].as_str().unwrap_or("");
    let category = meta["category"].as_str().unwrap_or("");
    let cover_image = meta["cover_image"].as_str().unwrap_or("");
    let excerpt = meta["excerpt"].as_str().unwrap_or("");

    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>编辑文章</title>{style}</head>
        <body>{nav}
        <div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;">
                <h1>编辑文章</h1>
                <div style="display:flex;gap:8px;">
                    {publish_btn}
                    <form method="POST" action="/admin/posts/{id}/delete" onsubmit="return confirm('确定删除？')">
                        <button type="submit" class="btn btn-danger">删除</button>
                    </form>
                </div>
            </div>
            <form method="POST" action="/admin/posts/{id}">
                <div class="form-row">
                    <label>标题</label>
                    <input type="text" name="title" value="{title}" required>
                </div>
                <div class="form-row">
                    <label>Slug</label>
                    <input type="text" name="slug" value="{slug}">
                </div>
                <div class="form-row">
                    <label>内容</label>
                    <textarea name="content">{content}</textarea>
                </div>
                <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;">
                    <div class="form-row">
                        <label>状态</label>
                        <select name="status">
                            <option value="draft" {sel_draft}>草稿</option>
                            <option value="published" {sel_pub}>已发布</option>
                        </select>
                    </div>
                    <div class="form-row">
                        <label>分类</label>
                        <input type="text" name="category" value="{category}">
                    </div>
                    <div class="form-row">
                        <label>标签（逗号分隔）</label>
                        <input type="text" name="tags" value="{tags}">
                    </div>
                    <div class="form-row">
                        <label>封面图 URL</label>
                        <input type="text" name="cover_image" value="{cover_image}">
                    </div>
                </div>
                <div class="form-row">
                    <label>摘要</label>
                    <input type="text" name="excerpt" value="{excerpt}">
                </div>
                <div style="margin-top:16px;">
                    <button type="submit" class="btn btn-primary">保存修改</button>
                    <a href="/admin/posts" class="btn btn-secondary" style="margin-left:8px;">返回列表</a>
                </div>
            </form>
        </div></body></html>"#,
        style = page_style(),
        nav = admin_nav(),
        id = html_escape(post_id),
        title = html_escape(post_title),
        slug = html_escape(post_slug),
        content = html_escape(post_content),
        sel_draft = if post_status == "draft" { "selected" } else { "" },
        sel_pub = if post_status == "published" { "selected" } else { "" },
        tags = html_escape(tags),
        category = html_escape(category),
        cover_image = html_escape(cover_image),
        excerpt = html_escape(excerpt),
        publish_btn = if post_status == "draft" {
            format!(
                r#"<form method="POST" action="/admin/posts/{}/publish"><button type="submit" class="btn btn-success">发布</button></form>"#,
                html_escape(post_id)
            )
        } else {
            format!(
                r#"<form method="POST" action="/admin/posts/{}/unpublish"><button type="submit" class="btn btn-secondary">取消发布</button></form>"#,
                html_escape(post_id)
            )
        },
    );

    Html(html)
}

pub async fn update_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<PostForm>,
) -> Redirect {
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.title),
    };
    let status = form.status.as_deref().unwrap_or("draft");
    let now = chrono::Utc::now().to_rfc3339();

    let meta = serde_json::json!({
        "tags": form.tags.as_deref().unwrap_or(""),
        "category": form.category.as_deref().unwrap_or(""),
        "cover_image": form.cover_image.as_deref().unwrap_or(""),
        "excerpt": form.excerpt.as_deref().unwrap_or(""),
    })
    .to_string();

    let _ = sqlx::query(
        "UPDATE posts SET title = ?, slug = ?, content = ?, status = ?, meta = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&form.title)
    .bind(&slug)
    .bind(&form.content)
    .bind(status)
    .bind(&meta)
    .bind(&now)
    .bind(&id)
    .execute(&state.db)
    .await;

    Redirect::to(&format!("/admin/posts/{id}/edit"))
}

pub async fn delete_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE posts SET status = 'archived', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to("/admin/posts")
}

pub async fn publish_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE posts SET status = 'published', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to(&format!("/admin/posts/{id}/edit"))
}

pub async fn unpublish_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE posts SET status = 'draft', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to(&format!("/admin/posts/{id}/edit"))
}
