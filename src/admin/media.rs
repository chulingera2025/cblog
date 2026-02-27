use axum::extract::{Multipart, Path, Query, State};
use axum::response::{Html, IntoResponse, Json, Redirect};
use minijinja::context;
use serde::{Deserialize, Serialize};
use std::path;

use crate::admin::template::{build_admin_context, render_admin};
use crate::media::{process, upload};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<u32>,
}

#[derive(Serialize, sqlx::FromRow)]
struct MediaItem {
    id: String,
    filename: String,
    original_name: String,
    mime_type: String,
    size_bytes: i64,
    width: Option<i64>,
    height: Option<i64>,
    url: String,
    thumb_url: Option<String>,
    uploaded_at: String,
}

pub async fn list_media(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page: u32 = 24;
    let offset = (page - 1) * per_page;

    let rows: Vec<MediaItem> = sqlx::query_as::<_, MediaItem>(
        r#"SELECT id, filename, original_name, mime_type, size_bytes,
                  width, height, url, thumb_url, uploaded_at
           FROM media
           ORDER BY uploaded_at DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let has_next = rows.len() as u32 == per_page;
    let has_prev = page > 1;

    let media_items: Vec<minijinja::Value> = rows
        .iter()
        .map(|item| {
            let is_image = item.mime_type.starts_with("image/");
            let thumb_url = if is_image {
                item.thumb_url.as_deref().unwrap_or(&item.url).to_string()
            } else {
                String::new()
            };
            let size_display = upload::format_size(item.size_bytes as usize);
            let date = crate::admin::layout::format_datetime(&item.uploaded_at);
            context! {
                id => &item.id,
                original_name => &item.original_name,
                is_image => is_image,
                thumb_url => thumb_url,
                url => &item.url,
                size_display => size_display,
                date => date,
            }
        })
        .collect();

    let ctx = context! {
        media_items => media_items,
        has_prev => has_prev,
        has_next => has_next,
        prev_page => if has_prev { page - 1 } else { 1 },
        next_page => page + 1,
        ..build_admin_context(
            "媒体库",
            "/admin/media",
            &crate::admin::settings::get_site_title(&state).await,
            &crate::admin::settings::get_site_url(&state).await,
            &state.plugin_admin_pages,
        )
    };

    match render_admin(&state.admin_env, "media/list.cbtml", ctx) {
        Ok(html) => Html(html),
        Err(e) => Html(format!("模板渲染错误: {e:#}")),
    }
}

pub async fn upload_page(State(state): State<AppState>) -> Html<String> {
    let ctx = build_admin_context(
        "上传文件",
        "/admin/media",
        &crate::admin::settings::get_site_title(&state).await,
        &crate::admin::settings::get_site_url(&state).await,
        &state.plugin_admin_pages,
    );

    match render_admin(&state.admin_env, "media/upload.cbtml", ctx) {
        Ok(html) => Html(html),
        Err(e) => Html(format!("模板渲染错误: {e:#}")),
    }
}

pub async fn upload_media(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }

        let file_name = field.file_name().unwrap_or("upload").to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                return render_upload_error(&state, &format!("读取文件失败：{e}"));
            }
        };

        let config = &state.config.media;

        if let Err(e) = upload::validate_upload(&data, &content_type, config) {
            return render_upload_error(&state, &e.to_string());
        }

        let processed = match process::process_image(&data, config) {
            Ok(p) => p,
            Err(e) => {
                return render_upload_error(&state, &format!("图片处理失败：{e}"));
            }
        };

        // 如果转码为 WebP 则更新扩展名
        let final_name = if processed.mime_type == "image/webp" && !file_name.ends_with(".webp") {
            let stem = path::Path::new(&file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("upload");
            format!("{stem}.webp")
        } else {
            file_name.clone()
        };

        let (relative_path, url) = upload::generate_storage_path(&final_name);
        let upload_dir = &config.upload_dir;

        // 写入 {project_root}/{upload_dir}/{relative_path}
        let media_path = state.project_root.join(upload_dir).join(&relative_path);
        if let Some(parent) = media_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        if let Err(e) = tokio::fs::write(&media_path, &processed.data).await {
            return render_upload_error(&state, &format!("写入文件失败：{e}"));
        }

        // 同时写入 public 目录以供静态访问
        let public_path = state.project_root.join("public/media").join(&relative_path);
        if let Some(parent) = public_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&public_path, &processed.data).await.ok();

        // 写入缩略图
        let thumb_url = if let Some(ref thumb_data) = processed.thumbnail {
            let thumb_relative = thumb_relative_path(&relative_path);

            let thumb_media = state.project_root.join(upload_dir).join(&thumb_relative);
            if let Some(parent) = thumb_media.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            tokio::fs::write(&thumb_media, thumb_data).await.ok();

            let thumb_public = state.project_root.join("public/media").join(&thumb_relative);
            if let Some(parent) = thumb_public.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            tokio::fs::write(&thumb_public, thumb_data).await.ok();

            Some(format!("/media/{thumb_relative}"))
        } else {
            None
        };

        // 插入数据库
        let id = ulid::Ulid::new().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let filename = path::Path::new(&relative_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        sqlx::query(
            "INSERT INTO media (id, filename, original_name, mime_type, size_bytes, width, height, url, thumb_url, uploaded_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&filename)
        .bind(&file_name)
        .bind(&processed.mime_type)
        .bind(processed.data.len() as i64)
        .bind(processed.width as i64)
        .bind(processed.height as i64)
        .bind(&url)
        .bind(&thumb_url)
        .bind(&now)
        .execute(&state.db)
        .await
        .ok();

        return Redirect::to("/admin/media").into_response();
    }

    render_upload_error(&state, "未找到上传文件")
}

pub async fn delete_media(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Redirect {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT url, thumb_url FROM media WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((url, thumb_url)) = row {
        sqlx::query("DELETE FROM media WHERE id = ?")
            .bind(&id)
            .execute(&state.db)
            .await
            .ok();

        let upload_dir = &state.config.media.upload_dir;
        let relative = url.strip_prefix("/media/").unwrap_or(&url);

        let media_file = state.project_root.join(upload_dir).join(relative);
        tokio::fs::remove_file(&media_file).await.ok();

        let public_file = state.project_root.join("public/media").join(relative);
        tokio::fs::remove_file(&public_file).await.ok();

        if let Some(thumb) = thumb_url {
            let thumb_relative = thumb.strip_prefix("/media/").unwrap_or(&thumb);
            let thumb_media = state.project_root.join(upload_dir).join(thumb_relative);
            tokio::fs::remove_file(&thumb_media).await.ok();
            let thumb_public = state
                .project_root
                .join("public/media")
                .join(thumb_relative);
            tokio::fs::remove_file(&thumb_public).await.ok();
        }
    }

    Redirect::to("/admin/media")
}

/// JSON 格式的媒体上传接口，供编辑器拖拽/粘贴/选择上传使用
pub async fn api_upload_media(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }

        let file_name = field.file_name().unwrap_or("upload").to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": format!("读取文件失败：{e}") })),
                )
                    .into_response();
            }
        };

        let config = &state.config.media;

        if let Err(e) = upload::validate_upload(&data, &content_type, config) {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }

        let processed = match process::process_image(&data, config) {
            Ok(p) => p,
            Err(e) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": format!("图片处理失败：{e}") })),
                )
                    .into_response();
            }
        };

        let final_name = if processed.mime_type == "image/webp" && !file_name.ends_with(".webp") {
            let stem = path::Path::new(&file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("upload");
            format!("{stem}.webp")
        } else {
            file_name.clone()
        };

        let (relative_path, url) = upload::generate_storage_path(&final_name);
        let upload_dir = &config.upload_dir;

        let media_path = state.project_root.join(upload_dir).join(&relative_path);
        if let Some(parent) = media_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        if let Err(e) = tokio::fs::write(&media_path, &processed.data).await {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("写入文件失败：{e}") })),
            )
                .into_response();
        }

        let public_path = state.project_root.join("public/media").join(&relative_path);
        if let Some(parent) = public_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&public_path, &processed.data).await.ok();

        let thumb_url = if let Some(ref thumb_data) = processed.thumbnail {
            let thumb_relative = thumb_relative_path(&relative_path);

            let thumb_media = state.project_root.join(upload_dir).join(&thumb_relative);
            if let Some(parent) = thumb_media.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            tokio::fs::write(&thumb_media, thumb_data).await.ok();

            let thumb_public = state.project_root.join("public/media").join(&thumb_relative);
            if let Some(parent) = thumb_public.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            tokio::fs::write(&thumb_public, thumb_data).await.ok();

            Some(format!("/media/{thumb_relative}"))
        } else {
            None
        };

        let id = ulid::Ulid::new().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let filename = path::Path::new(&relative_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let _ = sqlx::query(
            "INSERT INTO media (id, filename, original_name, mime_type, size_bytes, width, height, url, thumb_url, uploaded_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&filename)
        .bind(&file_name)
        .bind(&processed.mime_type)
        .bind(processed.data.len() as i64)
        .bind(processed.width as i64)
        .bind(processed.height as i64)
        .bind(&url)
        .bind(&thumb_url)
        .bind(&now)
        .execute(&state.db)
        .await
        .ok();

        return Json(serde_json::json!({
            "url": url,
            "filename": filename,
            "id": id
        }))
        .into_response();
    }

    (
        axum::http::StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "未找到上传文件" })),
    )
        .into_response()
}

/// JSON 格式的媒体列表，供编辑器插入图片使用
pub async fn api_media_list(State(state): State<AppState>) -> impl IntoResponse {
    let items: Vec<MediaItem> = sqlx::query_as::<_, MediaItem>(
        "SELECT id, filename, original_name, mime_type, size_bytes,
                width, height, url, thumb_url, uploaded_at
         FROM media ORDER BY uploaded_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(items)
}

fn render_upload_error(state: &AppState, message: &str) -> axum::response::Response {
    let base = build_admin_context(
        "上传失败",
        "/admin/media",
        &state.config.site.title,
        &state.config.site.url,
        &state.plugin_admin_pages,
    );
    let ctx = context! {
        error_message => message,
        ..base
    };

    match render_admin(&state.admin_env, "media/error.cbtml", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => Html(format!("模板渲染错误: {e:#}")).into_response(),
    }
}

/// 缩略图路径：在文件名前加 thumb_ 前缀
fn thumb_relative_path(relative_path: &str) -> String {
    let p = path::Path::new(relative_path);
    let parent = p.parent().and_then(|p| p.to_str()).unwrap_or("");
    let filename = p
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("thumb.webp");

    if parent.is_empty() {
        format!("thumb_{filename}")
    } else {
        format!("{parent}/thumb_{filename}")
    }
}
