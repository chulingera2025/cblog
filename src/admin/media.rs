use axum::extract::{Multipart, Path, Query, State};
use axum::response::{Html, IntoResponse, Json, Redirect};
use serde::{Deserialize, Serialize};
use std::path;

use crate::admin::layout::{admin_page, html_escape};
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

const EXTRA_STYLE: &str = r#"
    .container { max-width:1200px; }
    input[type=file] { margin-bottom:12px; }
    .media-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(200px,1fr)); gap:16px; }
    .media-card { background:#fff; border-radius:6px; overflow:hidden; box-shadow:0 1px 3px rgba(0,0,0,0.1); }
    .media-card img { width:100%; height:160px; object-fit:cover; display:block; background:#eee; }
    .media-card .file-icon { width:100%; height:160px; display:flex; align-items:center; justify-content:center; background:#e8e8e8; font-size:48px; color:#999; }
    .media-card .info { padding:10px; }
    .media-card .info .filename { font-size:13px; font-weight:500; word-break:break-all; margin-bottom:4px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
    .media-card .info .meta { font-size:11px; color:#888; margin-bottom:6px; }
    .media-card .info .actions { display:flex; gap:6px; align-items:center; }
    .media-card .info .actions a,
    .media-card .info .actions button { font-size:12px; padding:2px 8px; }
    .alert { padding:10px 16px; border-radius:4px; margin-bottom:16px; }
    .alert-error { background:#fce4e4; color:#c0392b; border:1px solid #e74c3c; }
"#;

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

    let mut cards = String::new();
    for item in &rows {
        let is_image = item.mime_type.starts_with("image/");
        let preview = if is_image {
            let src = item.thumb_url.as_deref().unwrap_or(&item.url);
            format!(
                r#"<img src="{}" alt="{}" loading="lazy">"#,
                html_escape(src),
                html_escape(&item.original_name)
            )
        } else {
            r#"<div class="file-icon">&#128196;</div>"#.to_string()
        };

        let size_display = upload::format_size(item.size_bytes as usize);
        let date = &item.uploaded_at[..10.min(item.uploaded_at.len())];

        cards.push_str(&format!(
            r#"<div class="media-card">
                {preview}
                <div class="info">
                    <div class="filename" title="{original_name}">{original_name}</div>
                    <div class="meta">{size} &middot; {date}</div>
                    <div class="actions">
                        <a href="{url}" target="_blank" class="btn btn-secondary">查看</a>
                        <form method="POST" action="/admin/media/{id}/delete" style="display:inline;" onsubmit="return confirm('确定删除此文件？')">
                            <button type="submit" class="btn btn-danger">删除</button>
                        </form>
                    </div>
                </div>
            </div>"#,
            preview = preview,
            original_name = html_escape(&item.original_name),
            size = size_display,
            date = date,
            url = html_escape(&item.url),
            id = item.id,
        ));
    }

    let pagination = {
        let mut p = String::new();
        if page > 1 {
            p.push_str(&format!(
                r#"<a href="/admin/media?page={}" class="btn btn-secondary">上一页</a>"#,
                page - 1
            ));
        }
        if rows.len() as u32 == per_page {
            p.push_str(&format!(
                r#"<a href="/admin/media?page={}" class="btn btn-secondary">下一页</a>"#,
                page + 1
            ));
        }
        p
    };

    let body = format!(
        r#"<div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;">
                <h1>媒体库</h1>
                <a href="/admin/media/upload" class="btn btn-primary">上传文件</a>
            </div>
            <div class="media-grid">{cards}</div>
            <div style="margin-top:16px;display:flex;gap:8px;">
                {pagination}
            </div>
        </div>"#,
        cards = cards,
        pagination = pagination,
    );

    Html(admin_page("媒体库", EXTRA_STYLE, &body))
}

pub async fn upload_page() -> Html<String> {
    let body = r#"<div class="container" style="max-width:600px;">
            <h1>上传文件</h1>
            <form method="POST" action="/admin/media/upload" enctype="multipart/form-data">
                <div style="margin-bottom:12px;">
                    <label>选择文件</label>
                    <input type="file" name="file" required accept="image/*">
                </div>
                <div style="margin-top:16px;">
                    <button type="submit" class="btn btn-primary">上传</button>
                    <a href="/admin/media" class="btn btn-secondary" style="margin-left:8px;">返回媒体库</a>
                </div>
            </form>
        </div>"#;
    Html(admin_page("上传文件", EXTRA_STYLE, body))
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
                return upload_error_page(&format!("读取文件失败：{e}"));
            }
        };

        let config = &state.config.media;

        if let Err(e) = upload::validate_upload(&data, &content_type, config) {
            return upload_error_page(&e.to_string());
        }

        let processed = match process::process_image(&data, config) {
            Ok(p) => p,
            Err(e) => {
                return upload_error_page(&format!("图片处理失败：{e}"));
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
            return upload_error_page(&format!("写入文件失败：{e}"));
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

    upload_error_page("未找到上传文件")
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

fn upload_error_page(message: &str) -> axum::response::Response {
    let body = format!(
        r#"<div class="container" style="max-width:600px;">
            <h1>上传失败</h1>
            <div class="alert alert-error">{msg}</div>
            <a href="/admin/media/upload" class="btn btn-primary">重新上传</a>
            <a href="/admin/media" class="btn btn-secondary" style="margin-left:8px;">返回媒体库</a>
        </div>"#,
        msg = html_escape(message),
    );
    Html(admin_page("上传失败", EXTRA_STYLE, &body)).into_response()
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
