use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Json;
use minijinja::context;
use serde::Deserialize;
use sqlx::Row;

use crate::admin::layout;
use crate::admin::template::render_admin;
use crate::repository::post::{PostAutosaveParams, PostWriteParams};
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

pub async fn list_posts(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page: i32 = 20;

    let rows = state
        .posts
        .list(page, per_page, params.status.as_deref(), params.search.as_deref())
        .await;

    let has_next = rows.len() as i32 == per_page;

    let posts_ctx: Vec<_> = rows
        .iter()
        .map(|row| {
            let id: &str = row.get("id");
            let title: &str = row.get("title");
            let status: &str = row.get("status");
            let created_at: &str = row.get("created_at");
            let updated_at: &str = row.get("updated_at");

            let (badge_class, status_label) = match status {
                "published" => ("badge-success", "已发布"),
                "draft" => ("badge-warning", "草稿"),
                other => ("badge-neutral", other),
            };

            context! {
                id => id,
                title => title,
                badge_class => badge_class,
                status_label => status_label,
                created_at => layout::format_datetime(created_at),
                updated_at => layout::format_datetime(updated_at),
            }
        })
        .collect();

    let total_pages = if has_next { page + 1 } else { page };

    let sidebar_groups = layout::sidebar_groups_value("/admin/posts");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/posts");

    let ctx = context! {
        page_title => "文章管理",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        site_url => crate::admin::settings::get_site_url(&state).await,
        posts => posts_ctx,
        current_status => params.status.as_deref().unwrap_or(""),
        search_query => params.search.as_deref().unwrap_or(""),
        current_page => page,
        total_pages => total_pages,
        base_url => "/admin/posts",
    };

    let html = render_admin(&state.admin_env, "posts/list.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}

pub async fn new_post_page(State(state): State<AppState>) -> Response {
    let id = ulid::Ulid::new().to_string();
    let slug = format!("draft-{}", &id[..8].to_lowercase());

    if let Err(e) = state.posts.create(&PostWriteParams {
        id: &id, slug: &slug, title: "", content: "",
        status: "draft", meta: "{}",
        tags_str: "", category_str: "",
    }).await {
        tracing::error!("创建草稿失败：{e}");
        return Redirect::to("/admin/posts").into_response();
    }

    Redirect::to(&format!("/admin/posts/{id}")).into_response()
}

pub async fn edit_post_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Html<String> {
    let Some(post) = state.posts.get_by_id(&id).await else {
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

    // 草稿状态下隐藏自动生成的 draft-xxx slug
    let display_slug = if post_status == "draft" && post_slug.starts_with("draft-") {
        ""
    } else {
        post_slug
    };

    let sidebar_groups = layout::sidebar_groups_value("/admin/posts");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/posts");

    let ctx = context! {
        page_title => "编辑文章",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        wide_content => true,
        is_edit => true,
        post_id => post_id,
        post_title => post_title,
        post_slug => display_slug,
        post_status => post_status,
        post_tags => tags,
        post_category => category,
        post_cover_image => cover_image,
        post_excerpt => excerpt,
        editor_initial_content => post_content,
    };

    let html = render_admin(&state.admin_env, "posts/form.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}

pub async fn update_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<PostForm>,
) -> Redirect {
    let slug = match form.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            let generated = generate_slug(&form.title);
            if generated.is_empty() {
                state.posts.get_by_id(&id).await
                    .map(|row| row.get::<&str, _>("slug").to_string())
                    .unwrap_or(generated)
            } else {
                generated
            }
        }
    };
    let status = form.status.as_deref().unwrap_or("draft");

    let meta = serde_json::json!({
        "tags": form.tags.as_deref().unwrap_or(""),
        "category": form.category.as_deref().unwrap_or(""),
        "cover_image": form.cover_image.as_deref().unwrap_or(""),
        "excerpt": form.excerpt.as_deref().unwrap_or(""),
    })
    .to_string();

    if let Err(e) = state.posts.update(&PostWriteParams {
        id: &id, slug: &slug, title: &form.title, content: &form.content,
        status, meta: &meta,
        tags_str: form.tags.as_deref().unwrap_or(""),
        category_str: form.category.as_deref().unwrap_or(""),
    }).await {
        tracing::error!("更新文章失败：{e}");
        return Redirect::to(&format!("/admin/posts/{id}"));
    }

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:update_post").await;
    });

    Redirect::to(&format!("/admin/posts/{id}"))
}

pub async fn delete_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = state.posts.delete(&id).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:delete_post").await;
    });

    Redirect::to("/admin/posts")
}

pub async fn publish_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = state.posts.publish(&id).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:publish_post").await;
    });

    Redirect::to(&format!("/admin/posts/{id}"))
}

pub async fn unpublish_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let _ = state.posts.unpublish(&id).await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:unpublish_post").await;
    });

    Redirect::to(&format!("/admin/posts/{id}"))
}

#[derive(Deserialize)]
pub struct AutosaveBody {
    pub title: String,
    pub content: String,
    pub slug: Option<String>,
    pub tags: Option<String>,
    pub category: Option<String>,
    pub cover_image: Option<String>,
    pub excerpt: Option<String>,
}

pub async fn autosave_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AutosaveBody>,
) -> Response {
    let slug = match body.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            let generated = generate_slug(&body.title);
            if generated.is_empty() {
                state.posts.get_by_id(&id).await
                    .map(|row| row.get::<&str, _>("slug").to_string())
                    .unwrap_or(generated)
            } else {
                generated
            }
        }
    };

    let meta = serde_json::json!({
        "tags": body.tags.as_deref().unwrap_or(""),
        "category": body.category.as_deref().unwrap_or(""),
        "cover_image": body.cover_image.as_deref().unwrap_or(""),
        "excerpt": body.excerpt.as_deref().unwrap_or(""),
    })
    .to_string();

    match state.posts.autosave_update(&PostAutosaveParams {
        id: &id, slug: &slug, title: &body.title, content: &body.content, meta: &meta,
        tags_str: body.tags.as_deref().unwrap_or(""),
        category_str: body.category.as_deref().unwrap_or(""),
    }).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
