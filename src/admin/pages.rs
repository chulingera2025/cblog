use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, Redirect};
use minijinja::context;
use serde::Deserialize;
use sqlx::Row;

use crate::admin::layout;
use crate::admin::template::render_admin;
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

    let has_next = rows.len() as i32 == per_page;

    let pages_ctx: Vec<_> = rows
        .iter()
        .map(|row| {
            let id: &str = row.get("id");
            let title: &str = row.get("title");
            let slug: &str = row.get("slug");
            let status: &str = row.get("status");
            let template: Option<&str> = row.get("template");
            let updated_at: &str = row.get("updated_at");

            let (badge_class, status_label) = if status == "published" {
                ("badge-success", "已发布")
            } else {
                ("badge-warning", "草稿")
            };

            context! {
                id => id,
                title => title,
                slug => slug,
                badge_class => badge_class,
                status_label => status_label,
                template => template.unwrap_or("default"),
                updated_at => layout::format_datetime(updated_at),
            }
        })
        .collect();

    let total_pages = if has_next { page + 1 } else { page };

    let sidebar_groups = layout::sidebar_groups_value("/admin/pages");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/pages");

    let ctx = context! {
        page_title => "页面管理",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        pages => pages_ctx,
        current_page => page,
        total_pages => total_pages,
        base_url => "/admin/pages",
    };

    let html = render_admin(&state.admin_env, "pages/list.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}

pub async fn new_page_page(State(state): State<AppState>) -> Html<String> {
    let sidebar_groups = layout::sidebar_groups_value("/admin/pages");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/pages");

    let ctx = context! {
        page_title => "新建页面",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        wide_content => true,
        is_edit => false,
        editor_initial_content => "",
    };

    let html = render_admin(&state.admin_env, "pages/form.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    if status == "published" {
        let state_clone = state.clone();
        tokio::spawn(async move {
            crate::admin::build::spawn_build(&state_clone, "auto:create_page").await;
        });
    }

    Redirect::to(&format!("/admin/pages/{id}"))
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

    let sidebar_groups = layout::sidebar_groups_value("/admin/pages");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/pages");

    let ctx = context! {
        page_title => "编辑页面",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        wide_content => true,
        is_edit => true,
        page_id => pg_id,
        page_title => pg_title,
        page_slug => pg_slug,
        page_status => pg_status,
        page_template => pg_template.unwrap_or(""),
        editor_initial_content => pg_content,
    };

    let html = render_admin(&state.admin_env, "pages/form.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:update_page").await;
    });

    Redirect::to(&format!("/admin/pages/{id}"))
}

pub async fn delete_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = sqlx::query("DELETE FROM pages WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:delete_page").await;
    });

    Redirect::to("/admin/pages")
}
