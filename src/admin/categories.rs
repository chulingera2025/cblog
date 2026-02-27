use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::admin::layout::{admin_page, format_datetime, html_escape, svg_icon, PageContext};
use crate::state::AppState;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CategoryForm {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<String>,
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

pub async fn list_categories(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let site_title = crate::admin::settings::get_site_title(&state).await;
    let ctx = make_ctx(&state, site_title);

    let page = params.page.unwrap_or(1).max(1);
    let per_page: i32 = 20;
    let offset = (page as i32 - 1) * per_page;

    let rows = sqlx::query(
        "SELECT c.id, c.name, c.slug, c.description, c.parent_id, c.created_at, \
         (SELECT COUNT(*) FROM post_categories pc WHERE pc.category_id = c.id) AS post_count, \
         p.name AS parent_name \
         FROM categories c \
         LEFT JOIN categories p ON c.parent_id = p.id \
         ORDER BY c.created_at DESC LIMIT ? OFFSET ?",
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
        let parent_name: Option<&str> = row.get("parent_name");
        let post_count: i32 = row.get("post_count");
        let created_at: &str = row.get("created_at");

        table_rows.push_str(&format!(
            r#"<tr>
                <td><a href="/admin/categories/{id}">{name}</a></td>
                <td>{slug}</td>
                <td>{description}</td>
                <td>{parent}</td>
                <td>{post_count}</td>
                <td>{created_at}</td>
                <td class="actions">
                    <a href="/admin/categories/{id}" class="btn btn-secondary btn-sm">编辑</a>
                    <form method="POST" action="/admin/categories/{id}/delete" style="display:inline;" onsubmit="confirmAction('删除分类', '确定要删除该分类吗？删除后文章关联将被移除。', this); return false;">
                        <button type="submit" class="btn btn-danger btn-sm">删除</button>
                    </form>
                </td>
            </tr>"#,
            id = html_escape(id),
            name = html_escape(name),
            slug = html_escape(slug),
            description = html_escape(description),
            parent = html_escape(parent_name.unwrap_or("-")),
            post_count = post_count,
            created_at = format_datetime(created_at),
        ));
    }

    let pagination = {
        let mut p = String::new();
        if page > 1 {
            p.push_str(&format!(
                r#"<a href="/admin/categories?page={}" class="btn btn-secondary btn-sm">上一页</a>"#,
                page - 1
            ));
        }
        if rows.len() as i32 == per_page {
            p.push_str(&format!(
                r#"<a href="/admin/categories?page={}" class="btn btn-secondary btn-sm">下一页</a>"#,
                page + 1
            ));
        }
        p
    };

    let body = format!(
        r#"<div class="page-header">
    <h1 class="page-title">分类管理</h1>
    <a href="/admin/categories/new" class="btn btn-primary">{icon_plus} 新建分类</a>
</div>
<div class="table-wrapper">
    <table>
        <thead><tr><th>名称</th><th>Slug</th><th>描述</th><th>父分类</th><th>文章数</th><th>创建时间</th><th>操作</th></tr></thead>
        <tbody>{table_rows}</tbody>
    </table>
</div>
<div class="pagination">{pagination}</div>"#,
        icon_plus = svg_icon("plus"),
        table_rows = table_rows,
        pagination = pagination,
    );

    Html(admin_page("分类管理", "/admin/categories", &body, &ctx))
}

pub async fn new_category_page(State(state): State<AppState>) -> Html<String> {
    let site_title = crate::admin::settings::get_site_title(&state).await;
    let ctx = make_ctx(&state, site_title);

    let parent_options = build_parent_options(&state.db, None).await;

    let body = format!(
        r#"<a href="/admin/categories" class="page-back">{icon_back} 返回分类列表</a>
<div class="page-header">
    <h1 class="page-title">新建分类</h1>
</div>
<div class="card">
    <div class="card-body">
        <form method="POST" action="/admin/categories">
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
                <label class="form-label">父分类</label>
                <select name="parent_id" class="form-select">
                    <option value="">无</option>
                    {parent_options}
                </select>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-primary">创建分类</button>
                <a href="/admin/categories" class="btn btn-secondary">取消</a>
            </div>
        </form>
    </div>
</div>"#,
        icon_back = svg_icon("arrow-left"),
        parent_options = parent_options,
    );

    Html(admin_page("新建分类", "/admin/categories", &body, &ctx))
}

pub async fn create_category(
    State(state): State<AppState>,
    Form(form): Form<CategoryForm>,
) -> Response {
    let id = ulid::Ulid::new().to_string();
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.name),
    };
    let description = form.description.as_deref().unwrap_or("");
    let parent_id = form.parent_id.as_deref().filter(|s| !s.is_empty());
    let now = chrono::Utc::now().to_rfc3339();

    if let Err(e) = sqlx::query(
        "INSERT INTO categories (id, name, slug, description, parent_id, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&form.name)
    .bind(&slug)
    .bind(description)
    .bind(parent_id)
    .bind(&now)
    .execute(&state.db)
    .await
    {
        tracing::error!("创建分类失败：{e}");
        return Redirect::to("/admin/categories/new?toast_msg=创建失败，名称或slug可能已存在&toast_type=error")
            .into_response();
    }

    Redirect::to("/admin/categories?toast_msg=分类已创建&toast_type=success").into_response()
}

pub async fn edit_category_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Html<String> {
    let site_title = crate::admin::settings::get_site_title(&state).await;
    let ctx = make_ctx(&state, site_title);

    let row = sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description, parent_id, created_at FROM categories WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some(cat) = row else {
        return Html(admin_page(
            "分类不存在",
            "/admin/categories",
            r#"<div class="empty-state"><p>该分类不存在</p></div>"#,
            &ctx,
        ));
    };

    let parent_options = build_parent_options(&state.db, Some(&cat.id)).await;

    let body = format!(
        r#"<a href="/admin/categories" class="page-back">{icon_back} 返回分类列表</a>
<div class="page-header">
    <h1 class="page-title">编辑分类</h1>
</div>
<div class="card">
    <div class="card-body">
        <form method="POST" action="/admin/categories/{id}">
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
                <label class="form-label">父分类</label>
                <select name="parent_id" class="form-select">
                    <option value="">无</option>
                    {parent_options}
                </select>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-primary">保存修改</button>
                <a href="/admin/categories" class="btn btn-secondary">取消</a>
            </div>
        </form>
    </div>
</div>"#,
        icon_back = svg_icon("arrow-left"),
        id = html_escape(&cat.id),
        name = html_escape(&cat.name),
        slug = html_escape(&cat.slug),
        description = html_escape(&cat.description),
        parent_options = parent_options,
    );

    Html(admin_page("编辑分类", "/admin/categories", &body, &ctx))
}

pub async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<CategoryForm>,
) -> Response {
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => generate_slug(&form.name),
    };
    let description = form.description.as_deref().unwrap_or("");
    let parent_id = form.parent_id.as_deref().filter(|s| !s.is_empty());

    if let Err(e) = sqlx::query(
        "UPDATE categories SET name = ?, slug = ?, description = ?, parent_id = ? WHERE id = ?",
    )
    .bind(&form.name)
    .bind(&slug)
    .bind(description)
    .bind(parent_id)
    .bind(&id)
    .execute(&state.db)
    .await
    {
        tracing::error!("更新分类失败：{e}");
        return Redirect::to(&format!(
            "/admin/categories/{id}?toast_msg=更新失败&toast_type=error"
        ))
        .into_response();
    }

    Redirect::to("/admin/categories?toast_msg=分类已更新&toast_type=success").into_response()
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to("/admin/categories?toast_msg=分类已删除&toast_type=success")
}

pub async fn api_list_categories(State(state): State<AppState>) -> Response {
    let cats = sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description, parent_id, created_at FROM categories ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    axum::Json(cats).into_response()
}

/// 构建父分类下拉选项 HTML，排除自身（编辑时）
async fn build_parent_options(db: &sqlx::SqlitePool, exclude_id: Option<&str>) -> String {
    let cats: Vec<Category> = sqlx::query_as(
        "SELECT id, name, slug, description, parent_id, created_at FROM categories ORDER BY name",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut html = String::new();
    for cat in &cats {
        if exclude_id == Some(cat.id.as_str()) {
            continue;
        }
        html.push_str(&format!(
            r#"<option value="{id}">{name}</option>"#,
            id = html_escape(&cat.id),
            name = html_escape(&cat.name),
        ));
    }
    html
}
