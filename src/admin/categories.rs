use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use minijinja::context;
use serde::Deserialize;
use sqlx::Row;

use crate::admin::layout;
use crate::admin::template::render_admin;
use crate::repository::category::Category;
use crate::state::AppState;

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

pub async fn list_categories(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page: i32 = 20;
    let offset = (page as i32 - 1) * per_page;

    let rows = state.categories.list_with_counts(per_page, offset).await;

    let has_next = rows.len() as i32 == per_page;

    let categories: Vec<minijinja::Value> = rows
        .iter()
        .map(|row| {
            let id: &str = row.get("id");
            let name: &str = row.get("name");
            let slug: &str = row.get("slug");
            let description: &str = row.get("description");
            let parent_name: Option<&str> = row.get("parent_name");
            let post_count: i32 = row.get("post_count");
            let created_at: &str = row.get("created_at");

            context! {
                id => id,
                name => name,
                slug => slug,
                description => description,
                parent_name => parent_name.unwrap_or("-"),
                post_count => post_count,
                created_at => layout::format_datetime(created_at),
            }
        })
        .collect();

    let total_pages = if has_next { page + 1 } else { page };
    let active_path = "/admin/categories";
    let sidebar_groups = layout::sidebar_groups_value(active_path);
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let ctx = context! {
        page_title => "分类管理",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        mode => "list",
        categories => categories,
        current_page => page,
        total_pages => total_pages,
        base_url => "/admin/categories",
    };

    let html = render_admin(&state.admin_env, "categories.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}

pub async fn new_category_page(State(state): State<AppState>) -> Html<String> {
    let parent_options = build_parent_options(&state, None, None).await;

    let active_path = "/admin/categories";
    let sidebar_groups = layout::sidebar_groups_value(active_path);
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let ctx = context! {
        page_title => "新建分类",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        back_url => "/admin/categories",
        mode => "new",
        parent_options => parent_options,
    };

    let html = render_admin(&state.admin_env, "categories.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    if let Err(e) = state.categories.create(&id, &form.name, &slug, description, parent_id).await {
        tracing::error!("创建分类失败：{e}");
        return Redirect::to("/admin/categories/new?toast_msg=创建失败，名称或slug可能已存在&toast_type=error")
            .into_response();
    }

    state.call_hook("after_category_create", &serde_json::json!({
        "id": id, "name": form.name, "slug": slug
    })).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:create_category").await;
    });

    Redirect::to("/admin/categories?toast_msg=分类已创建&toast_type=success").into_response()
}

pub async fn edit_category_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Html<String> {
    let active_path = "/admin/categories";
    let sidebar_groups = layout::sidebar_groups_value(active_path);
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, active_path);

    let Some(cat) = state.categories.get_by_id(&id).await else {
        let ctx = context! {
            page_title => "分类不存在",
            site_title => crate::admin::settings::get_site_title(&state).await,
            sidebar_groups => sidebar_groups,
            plugin_sidebar_items => plugin_items,
            profile_active => false,
            mode => "not_found",
        };
        return Html(
            render_admin(&state.admin_env, "categories.cbtml", ctx)
                .unwrap_or_else(|e| format!("模板渲染失败: {e}")),
        );
    };

    let parent_options = build_parent_options(&state, Some(&cat.id), cat.parent_id.as_deref()).await;

    let cat_ctx = context! {
        id => &cat.id,
        name => &cat.name,
        slug => &cat.slug,
        description => &cat.description,
    };

    let ctx = context! {
        page_title => "编辑分类",
        site_title => crate::admin::settings::get_site_title(&state).await,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        back_url => "/admin/categories",
        mode => "edit",
        category => cat_ctx,
        parent_options => parent_options,
    };

    let html = render_admin(&state.admin_env, "categories.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
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

    if let Err(e) = state.categories.update(&id, &form.name, &slug, description, parent_id).await {
        tracing::error!("更新分类失败：{e}");
        return Redirect::to(&format!(
            "/admin/categories/{id}?toast_msg=更新失败&toast_type=error"
        ))
        .into_response();
    }

    state.call_hook("after_category_update", &serde_json::json!({
        "id": id, "name": form.name, "slug": slug
    })).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:update_category").await;
    });

    Redirect::to("/admin/categories?toast_msg=分类已更新&toast_type=success").into_response()
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = state.categories.delete(&id).await;

    state.call_hook("after_category_delete", &serde_json::json!({
        "id": id
    })).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:delete_category").await;
    });

    Redirect::to("/admin/categories?toast_msg=分类已删除&toast_type=success")
}

pub async fn api_list_categories(State(state): State<AppState>) -> Response {
    let cats = state.categories.list_all().await;
    axum::Json(cats).into_response()
}

/// 构建父分类选项列表，排除自身（编辑时），标记当前选中项
async fn build_parent_options(
    state: &AppState,
    exclude_id: Option<&str>,
    selected_parent_id: Option<&str>,
) -> Vec<minijinja::Value> {
    let cats: Vec<Category> = state.categories.list_all().await;

    cats.iter()
        .filter(|cat| exclude_id != Some(cat.id.as_str()))
        .map(|cat| {
            let selected = selected_parent_id == Some(cat.id.as_str());
            context! {
                id => &cat.id,
                name => &cat.name,
                selected => selected,
            }
        })
        .collect()
}
