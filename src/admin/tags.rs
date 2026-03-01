use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use minijinja::context;
use serde::Deserialize;
use sqlx::Row;

use crate::admin::layout;
use crate::admin::template::render_admin;
use crate::state::AppState;

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

pub async fn list_tags(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page: i32 = 20;
    let offset = (page as i32 - 1) * per_page;

    let rows = state.tags.list_with_counts(per_page, offset).await;

    let has_next = rows.len() as i32 == per_page;

    let tags: Vec<minijinja::Value> = rows
        .iter()
        .map(|row| {
            let id: &str = row.get("id");
            let name: &str = row.get("name");
            let slug: &str = row.get("slug");
            let description: &str = row.get("description");
            let post_count: i32 = row.get("post_count");
            let created_at: &str = row.get("created_at");

            context! {
                id => id,
                name => name,
                slug => slug,
                description => description,
                post_count => post_count,
                created_at => layout::format_datetime(created_at),
            }
        })
        .collect();

    let total_pages = if has_next { page + 1 } else { page };
    let active_path = "/admin/tags";
    let sidebar_groups = layout::sidebar_groups_value(active_path);
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let ctx = context! {
        page_title => "标签管理",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        mode => "list",
        tags => tags,
        current_page => page,
        total_pages => total_pages,
        base_url => "/admin/tags",
    };

    let html = render_admin(&state.admin_env, "tags.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}

pub async fn new_tag_page(State(state): State<AppState>) -> Html<String> {
    let active_path = "/admin/tags";
    let sidebar_groups = layout::sidebar_groups_value(active_path);
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let ctx = context! {
        page_title => "新建标签",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        back_url => "/admin/tags",
        mode => "new",
    };

    let html = render_admin(&state.admin_env, "tags.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    if let Err(e) = state.tags.create(&id, &form.name, &slug, description).await {
        tracing::error!("创建标签失败：{e}");
        return Redirect::to(
            "/admin/tags/new?toast_msg=创建失败，名称或slug可能已存在&toast_type=error",
        )
        .into_response();
    }

    state.call_hook("after_tag_create", &serde_json::json!({
        "id": id, "name": form.name, "slug": slug
    })).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:create_tag").await;
    });

    Redirect::to("/admin/tags?toast_msg=标签已创建&toast_type=success").into_response()
}

pub async fn edit_tag_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Html<String> {
    let active_path = "/admin/tags";
    let sidebar_groups = layout::sidebar_groups_value(active_path);
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let Some(tag) = state.tags.get_by_id(&id).await else {
        let ctx = context! {
            page_title => "标签不存在",
            site_title => crate::admin::settings::get_site_title(&state).await,
            sidebar_groups => sidebar_groups,
            plugin_sidebar_items => plugin_items,
            profile_active => false,
            mode => "not_found",
        };
        return Html(
            render_admin(&state.admin_env, "tags.cbtml", ctx)
                .unwrap_or_else(|e| format!("模板渲染失败: {e}")),
        );
    };

    let tag_ctx = context! {
        id => &tag.id,
        name => &tag.name,
        slug => &tag.slug,
        description => &tag.description,
    };

    let ctx = context! {
        page_title => "编辑标签",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        back_url => "/admin/tags",
        mode => "edit",
        tag => tag_ctx,
    };

    let html = render_admin(&state.admin_env, "tags.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    if let Err(e) = state.tags.update(&id, &form.name, &slug, description).await {
        tracing::error!("更新标签失败：{e}");
        return Redirect::to(&format!(
            "/admin/tags/{id}?toast_msg=更新失败&toast_type=error"
        ))
        .into_response();
    }

    state.call_hook("after_tag_update", &serde_json::json!({
        "id": id, "name": form.name, "slug": slug
    })).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:update_tag").await;
    });

    Redirect::to("/admin/tags?toast_msg=标签已更新&toast_type=success").into_response()
}

pub async fn delete_tag(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = state.tags.delete(&id).await;

    state.call_hook("after_tag_delete", &serde_json::json!({
        "id": id
    })).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:delete_tag").await;
    });

    Redirect::to("/admin/tags?toast_msg=标签已删除&toast_type=success")
}

pub async fn api_list_tags(State(state): State<AppState>) -> Response {
    let tags = state.tags.list_all().await;
    axum::Json(tags).into_response()
}
