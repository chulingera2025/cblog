use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::admin::layout::{admin_page, format_datetime, html_escape, svg_icon, PageContext};
use crate::state::AppState;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct TagForm {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<u32>,
}

fn generate_slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn make_ctx(state: &AppState, site_title: String) -> PageContext {
    PageContext {
        site_title,
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    }
}

pub async fn list_tags(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let site_title = crate::admin::settings::get_site_title(&state).await;
    let ctx = make_ctx(&state, site_title);

    let page = params.page.unwrap_or(1).max(1);
    let per_page: i32 = 20;
    let offset = (page as i32 - 1) * per_page;

    let rows = sqlx::query(
        "SELECT t.id, t.name, t.slug, t.description, t.created_at, \
         (SELECT COUNT(*) FROM post_tags pt WHERE pt.tag_id = t.id) AS post_count \
         FROM tags t \
         ORDER BY t.created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut table_rows = String::new();
    for row in &rows {
        let id: &str = row.get("id");
        let name: &str = row.get("name");
        let slug: &str = row.get("slug");
        let description: &str = row.get("description");
        let post_count: i32 = row.get("post_count");
        let created_at: &str = row.get("created_at");

        table_rows.push_str(&format!(
            r#"<tr>
                <td><a href="/admin/tags/{id}">{name}</a></td>
                <td>{slug}</td>
                <td>{description}</td>
                <td>{post_count}</td>
                <td>{created_at}</td>
                <td class="actions">
                    <a href="/admin/tags/{id}" class="btn btn-secondary btn-sm">编辑</a>
                    <form method="POST" action="/admin/tags/{id}/delete" style="display:inline;" onsubmit="confirmAction('删除标签', '确定要删除该标签吗？删除后文章关联将被移除。', this); return false;">
                        <button type="submit" class="btn btn-danger btn-sm">删除</button>
                    </form>
                </td>
            </tr>"#,
            id = html_escape(id),
            name = html_escape(name),
            slug = html_escape(slug),
            description = html_escape(description),
            post_count = post_count,
            created_at = format_datetime(created_at),
        ));
    }

    let pagination = {
        let mut p = String::new();
        if page > 1 {
            p.push_str(&format!(
                r#"<a href="/admin/tags?page={}" class="btn btn-secondary btn-sm">上一页</a>"#,
                page - 1
            ));
        }
        if rows.len() as i32 == per_page {
            p.push_str(&format!(
                r#"<a href="/admin/tags?page={}" class="btn btn-secondary btn-sm">下一页</a>"#,
                page + 1
            ));
        }
        p
    };

    let body = format!(
        r#"<div class="page-header">
    <h1 class="page-title">标签管理</h1>
    <a href="/admin/tags/new" class="btn btn-primary">{icon_plus} 新建标签</a>
</div>
<div class="table-wrapper">
    <table>
        <thead><tr><th>名称</th><th>Slug</th><th>描述</th><th>文章数</th><th>创建时间</th><th>操作</th></tr></thead>
        <tbody>{table_rows}</tbody>
    </table>
</div>
<div class="pagination">{pagination}</div>"#,
        icon_plus = svg_icon("plus"),
        table_rows = table_rows,
        pagination = pagination,
    );

    Html(admin_page("标签管理", "/admin/tags", &body, &ctx))
}

pub async fn new_tag_page(State(state): State<AppState>) -> Html<String> {
    let site_title = crate::admin::settings::get_site_title(&state).await;
    let ctx = make_ctx(&state, site_title);

    let body = format!(
        r#"<a href="/admin/tags" class="page-back">{icon_back} 返回标签列表</a>
<div class="page-header">
    <h1 class="page-title">新建标签</h1>
</div>
<div class="card">
    <div class="card-body">
        <form method="POST" action="/admin/tags">
            <div class="form-group">
                <label class="form-label">名称</label>
                <input type="text" name="name" class="form-input" required>
            </div>
            <div class="form-group">
                <label class="form-label">Slug（留空自动生成）</label>
                <input type="text" name="slug" class="form-input">
            </div>
            <div class="form-group">
                <label class="form-label">描述</label>
                <textarea name="description" class="form-textarea" rows="3"></textarea>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-primary">创建标签</button>
                <a href="/admin/tags" class="btn btn-secondary">取消</a>
            </div>
        </form>
    </div>
</div>"#,
        icon_back = svg_icon("arrow-left"),
    );

    Html(admin_page("新建标签", "/admin/tags", &body, &ctx))
}

pub async fn create_tag(
    State(state): State<AppState>,
    Form(form): Form<TagForm>,
) -> Response {
    let id = ulid::Ulid::new().to_string();
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.name),
    };
    let description = form.description.as_deref().unwrap_or("");
    let now = chrono::Utc::now().to_rfc3339();

    if let Err(e) = sqlx::query(
        "INSERT INTO tags (id, name, slug, description, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&form.name)
    .bind(&slug)
    .bind(description)
    .bind(&now)
    .execute(&state.db)
    .await
    {
        tracing::error!("创建标签失败：{e}");
        return Redirect::to(
            "/admin/tags/new?toast_msg=创建失败，名称或slug可能已存在&toast_type=error",
        )
        .into_response();
    }

    Redirect::to("/admin/tags?toast_msg=标签已创建&toast_type=success").into_response()
}

pub async fn edit_tag_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Html<String> {
    let site_title = crate::admin::settings::get_site_title(&state).await;
    let ctx = make_ctx(&state, site_title);

    let row = sqlx::query_as::<_, Tag>(
        "SELECT id, name, slug, description, created_at FROM tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some(tag) = row else {
        return Html(admin_page(
            "标签不存在",
            "/admin/tags",
            r#"<div class="empty-state"><p>该标签不存在</p></div>"#,
            &ctx,
        ));
    };

    let body = format!(
        r#"<a href="/admin/tags" class="page-back">{icon_back} 返回标签列表</a>
<div class="page-header">
    <h1 class="page-title">编辑标签</h1>
</div>
<div class="card">
    <div class="card-body">
        <form method="POST" action="/admin/tags/{id}">
            <div class="form-group">
                <label class="form-label">名称</label>
                <input type="text" name="name" value="{name}" class="form-input" required>
            </div>
            <div class="form-group">
                <label class="form-label">Slug</label>
                <input type="text" name="slug" value="{slug}" class="form-input">
            </div>
            <div class="form-group">
                <label class="form-label">描述</label>
                <textarea name="description" class="form-textarea" rows="3">{description}</textarea>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-primary">保存修改</button>
                <a href="/admin/tags" class="btn btn-secondary">取消</a>
            </div>
        </form>
    </div>
</div>"#,
        icon_back = svg_icon("arrow-left"),
        id = html_escape(&tag.id),
        name = html_escape(&tag.name),
        slug = html_escape(&tag.slug),
        description = html_escape(&tag.description),
    );

    Html(admin_page("编辑标签", "/admin/tags", &body, &ctx))
}

pub async fn update_tag(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<TagForm>,
) -> Response {
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.name),
    };
    let description = form.description.as_deref().unwrap_or("");

    if let Err(e) = sqlx::query(
        "UPDATE tags SET name = ?, slug = ?, description = ? WHERE id = ?",
    )
    .bind(&form.name)
    .bind(&slug)
    .bind(description)
    .bind(&id)
    .execute(&state.db)
    .await
    {
        tracing::error!("更新标签失败：{e}");
        return Redirect::to(&format!(
            "/admin/tags/{id}?toast_msg=更新失败&toast_type=error"
        ))
        .into_response();
    }

    Redirect::to("/admin/tags?toast_msg=标签已更新&toast_type=success").into_response()
}

pub async fn delete_tag(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to("/admin/tags?toast_msg=标签已删除&toast_type=success")
}

pub async fn api_list_tags(State(state): State<AppState>) -> Response {
    let tags = sqlx::query_as::<_, Tag>(
        "SELECT id, name, slug, description, created_at FROM tags ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    axum::Json(tags).into_response()
}
