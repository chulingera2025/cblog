use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, Redirect};
use minijinja::context;
use serde::Deserialize;
use sqlx::Row;

use crate::admin::layout;
use crate::admin::template::render_admin;
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

    // 分页：简单的上一页/下一页，需要计算 total_pages 来兼容 pagination partial
    // 由于原始实现没有 COUNT 查询，这里用 has_next 模拟总页数
    let total_pages = if has_next { page + 1 } else { page };

    let sidebar_groups = layout::sidebar_groups_value("/admin/posts");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/posts");

    let ctx = context! {
        page_title => "文章管理",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
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

pub async fn new_post_page(State(state): State<AppState>) -> Html<String> {
    let sidebar_groups = layout::sidebar_groups_value("/admin/posts");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/posts");

    let ctx = context! {
        page_title => "新建文章",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        is_edit => false,
        editor_initial_content => "",
    };

    let html = render_admin(&state.admin_env, "posts/form.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

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

    sync_post_taxonomy(
        &state.db,
        &id,
        form.tags.as_deref().unwrap_or(""),
        form.category.as_deref().unwrap_or(""),
    )
    .await;

    if status == "published" {
        let state_clone = state.clone();
        tokio::spawn(async move {
            crate::admin::build::spawn_build(&state_clone, "auto:create_post").await;
        });
    }

    Redirect::to(&format!("/admin/posts/{id}"))
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

    let sidebar_groups = layout::sidebar_groups_value("/admin/posts");
    let plugin_items = layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/posts");

    let ctx = context! {
        page_title => "编辑文章",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => false,
        is_edit => true,
        post_id => post_id,
        post_title => post_title,
        post_slug => post_slug,
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

    sync_post_taxonomy(
        &state.db,
        &id,
        form.tags.as_deref().unwrap_or(""),
        form.category.as_deref().unwrap_or(""),
    )
    .await;

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
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE posts SET status = 'archived', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await;

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
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE posts SET status = 'published', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await;

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
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE posts SET status = 'draft', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await;

    let state_clone = state.clone();
    tokio::spawn(async move {
        crate::admin::build::spawn_build(&state_clone, "auto:unpublish_post").await;
    });

    Redirect::to(&format!("/admin/posts/{id}"))
}

/// 同步文章的分类和标签关联表
/// 根据表单提交的 tags（逗号分隔）和 category 字符串，
/// 清空旧关联并重建，对不存在的标签/分类自动创建
async fn sync_post_taxonomy(
    db: &sqlx::SqlitePool,
    post_id: &str,
    tags_str: &str,
    category_str: &str,
) {
    // 同步标签
    let _ = sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(db)
        .await;

    let tags: Vec<&str> = tags_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    for tag_name in tags {
        let slug = generate_slug(tag_name);
        if let Some(tag_id) = get_or_create_tag(db, tag_name, &slug).await {
            let _ = sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
                .bind(post_id)
                .bind(&tag_id)
                .execute(db)
                .await;
        }
    }

    // 同步分类
    let _ = sqlx::query("DELETE FROM post_categories WHERE post_id = ?")
        .bind(post_id)
        .execute(db)
        .await;

    let category = category_str.trim();
    if !category.is_empty() {
        let slug = generate_slug(category);
        if let Some(cat_id) = get_or_create_category(db, category, &slug).await {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO post_categories (post_id, category_id) VALUES (?, ?)",
            )
            .bind(post_id)
            .bind(&cat_id)
            .execute(db)
            .await;
        }
    }
}

async fn get_or_create_tag(db: &sqlx::SqlitePool, name: &str, slug: &str) -> Option<String> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM tags WHERE name = ?")
            .bind(name)
            .fetch_optional(db)
            .await
            .ok()?;

    if let Some((id,)) = existing {
        return Some(id);
    }

    let id = ulid::Ulid::new().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO tags (id, name, slug, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(slug)
        .bind(&now)
        .execute(db)
        .await
        .ok()?;
    Some(id)
}

async fn get_or_create_category(db: &sqlx::SqlitePool, name: &str, slug: &str) -> Option<String> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM categories WHERE name = ?")
            .bind(name)
            .fetch_optional(db)
            .await
            .ok()?;

    if let Some((id,)) = existing {
        return Some(id);
    }

    let id = ulid::Ulid::new().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO categories (id, name, slug, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(slug)
        .bind(&now)
        .execute(db)
        .await
        .ok()?;
    Some(id)
}
