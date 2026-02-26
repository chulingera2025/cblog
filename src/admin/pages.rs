use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, Redirect};
use serde::Deserialize;
use sqlx::Row;

use crate::admin::layout::{admin_page, html_escape};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct PageForm {
    pub title: String,
    pub slug: Option<String>,
    pub content: String,
    pub status: Option<String>,
    pub template: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub status: Option<String>,
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

const EXTRA_STYLE: &str = r#"
    textarea { min-height:300px; font-family:monospace; }
"#;

pub async fn list_pages(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page: i32 = 20;
    let offset = (page as i32 - 1) * per_page;

    let rows = match params.status.as_deref() {
        Some(status) => {
            sqlx::query(
                "SELECT id, title, slug, status, template, updated_at FROM pages \
                 WHERE status = ? ORDER BY updated_at DESC LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
        }
        None => {
            sqlx::query(
                "SELECT id, title, slug, status, template, updated_at FROM pages \
                 ORDER BY updated_at DESC LIMIT ? OFFSET ?",
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
        let slug: &str = row.get("slug");
        let status: &str = row.get("status");
        let template: Option<&str> = row.get("template");
        let updated_at: &str = row.get("updated_at");

        let badge_class = if status == "published" {
            "status-published"
        } else {
            "status-draft"
        };
        let status_label = if status == "published" {
            "已发布"
        } else {
            "草稿"
        };
        let tpl = template.unwrap_or("default");

        table_rows.push_str(&format!(
            r#"<tr>
                <td><a href="/admin/pages/{id}/edit">{title}</a></td>
                <td>{slug}</td>
                <td><span class="status-badge {badge_class}">{status_label}</span></td>
                <td>{tpl}</td>
                <td>{updated_at}</td>
                <td class="actions">
                    <a href="/admin/pages/{id}/edit" class="btn btn-secondary" style="padding:2px 8px;font-size:12px;">编辑</a>
                    <form method="POST" action="/admin/pages/{id}/delete" style="display:inline;" onsubmit="return confirm('确定删除？')">
                        <button type="submit" class="btn btn-danger" style="padding:2px 8px;font-size:12px;">删除</button>
                    </form>
                </td>
            </tr>"#,
            id = html_escape(id),
            title = html_escape(title),
            slug = html_escape(slug),
            badge_class = badge_class,
            status_label = status_label,
            tpl = html_escape(tpl),
            updated_at = &updated_at[..10.min(updated_at.len())],
        ));
    }

    let body = format!(
        r#"<div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;">
                <h1>页面管理</h1>
                <a href="/admin/pages/new" class="btn btn-primary">新建页面</a>
            </div>
            <table>
                <thead><tr><th>标题</th><th>Slug</th><th>状态</th><th>模板</th><th>更新时间</th><th>操作</th></tr></thead>
                <tbody>{table_rows}</tbody>
            </table>
            <div style="margin-top:16px;display:flex;gap:8px;">
                {pagination}
            </div>
        </div>"#,
        table_rows = table_rows,
        pagination = {
            let mut p = String::new();
            if page > 1 {
                p.push_str(&format!(
                    r#"<a href="/admin/pages?page={}" class="btn btn-secondary">上一页</a>"#,
                    page - 1
                ));
            }
            if rows.len() as i32 == per_page {
                p.push_str(&format!(
                    r#"<a href="/admin/pages?page={}" class="btn btn-secondary">下一页</a>"#,
                    page + 1
                ));
            }
            p
        },
    );

    Html(admin_page("页面管理", EXTRA_STYLE, &body))
}

pub async fn new_page_page() -> Html<String> {
    let body = r#"<div class="container">
            <h1>新建页面</h1>
            <form method="POST" action="/admin/pages">
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
                        <label>模板</label>
                        <input type="text" name="template" placeholder="default">
                    </div>
                </div>
                <div style="margin-top:16px;">
                    <button type="submit" class="btn btn-primary">创建页面</button>
                    <a href="/admin/pages" class="btn btn-secondary" style="margin-left:8px;">取消</a>
                </div>
            </form>
        </div>"#;
    Html(admin_page("新建页面", EXTRA_STYLE, body))
}

pub async fn create_page(
    State(state): State<AppState>,
    Form(form): Form<PageForm>,
) -> Redirect {
    let id = ulid::Ulid::new().to_string();
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.title),
    };
    let status = form.status.as_deref().unwrap_or("draft");
    let template = form.template.as_deref().filter(|s| !s.trim().is_empty());
    let now = chrono::Utc::now().to_rfc3339();

    let _ = sqlx::query(
        "INSERT INTO pages (id, slug, title, content, status, template, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&slug)
    .bind(&form.title)
    .bind(&form.content)
    .bind(status)
    .bind(template)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await;

    Redirect::to(&format!("/admin/pages/{id}/edit"))
}

pub async fn edit_page_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Html<String> {
    let row = sqlx::query(
        "SELECT id, slug, title, content, status, template FROM pages WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some(pg) = row else {
        return Html("<h1>页面不存在</h1>".to_string());
    };

    let pg_id: &str = pg.get("id");
    let pg_slug: &str = pg.get("slug");
    let pg_title: &str = pg.get("title");
    let pg_content: &str = pg.get("content");
    let pg_status: &str = pg.get("status");
    let pg_template: Option<&str> = pg.get("template");

    let body = format!(
        r#"<div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;">
                <h1>编辑页面</h1>
                <form method="POST" action="/admin/pages/{id}/delete" onsubmit="return confirm('确定删除？')">
                    <button type="submit" class="btn btn-danger">删除</button>
                </form>
            </div>
            <form method="POST" action="/admin/pages/{id}">
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
                        <label>模板</label>
                        <input type="text" name="template" value="{template}">
                    </div>
                </div>
                <div style="margin-top:16px;">
                    <button type="submit" class="btn btn-primary">保存修改</button>
                    <a href="/admin/pages" class="btn btn-secondary" style="margin-left:8px;">返回列表</a>
                </div>
            </form>
        </div>"#,
        id = html_escape(pg_id),
        title = html_escape(pg_title),
        slug = html_escape(pg_slug),
        content = html_escape(pg_content),
        sel_draft = if pg_status == "draft" { "selected" } else { "" },
        sel_pub = if pg_status == "published" { "selected" } else { "" },
        template = html_escape(pg_template.unwrap_or("")),
    );

    Html(admin_page("编辑页面", EXTRA_STYLE, &body))
}

pub async fn update_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<PageForm>,
) -> Redirect {
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.title),
    };
    let status = form.status.as_deref().unwrap_or("draft");
    let template = form.template.as_deref().filter(|s| !s.trim().is_empty());
    let now = chrono::Utc::now().to_rfc3339();

    let _ = sqlx::query(
        "UPDATE pages SET title = ?, slug = ?, content = ?, status = ?, template = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&form.title)
    .bind(&slug)
    .bind(&form.content)
    .bind(status)
    .bind(template)
    .bind(&now)
    .bind(&id)
    .execute(&state.db)
    .await;

    Redirect::to(&format!("/admin/pages/{id}/edit"))
}

pub async fn delete_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = sqlx::query("DELETE FROM pages WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to("/admin/pages")
}
