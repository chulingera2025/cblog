use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct MediaRepository {
    db: SqlitePool,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct MediaItem {
    pub id: String,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub url: String,
    pub thumb_url: Option<String>,
    pub uploaded_at: String,
}

/// 媒体插入参数
pub struct MediaInsertParams<'a> {
    pub id: &'a str,
    pub filename: &'a str,
    pub original_name: &'a str,
    pub mime_type: &'a str,
    pub size_bytes: i64,
    pub width: i64,
    pub height: i64,
    pub url: &'a str,
    pub thumb_url: Option<&'a str>,
}

impl MediaRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list(&self, per_page: u32, offset: u32) -> Vec<MediaItem> {
        sqlx::query_as::<_, MediaItem>(
            "SELECT id, filename, original_name, mime_type, size_bytes, \
                    width, height, url, thumb_url, uploaded_at \
             FROM media ORDER BY uploaded_at DESC LIMIT ? OFFSET ?",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }

    pub async fn count(&self) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM media")
            .fetch_one(&self.db)
            .await
            .unwrap_or(0)
    }

    pub async fn insert(&self, p: &MediaInsertParams<'_>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO media (id, filename, original_name, mime_type, size_bytes, width, height, url, thumb_url, uploaded_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(p.id)
        .bind(p.filename)
        .bind(p.original_name)
        .bind(p.mime_type)
        .bind(p.size_bytes)
        .bind(p.width)
        .bind(p.height)
        .bind(p.url)
        .bind(p.thumb_url)
        .bind(&now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn get_urls(&self, id: &str) -> Option<(String, Option<String>)> {
        sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT url, thumb_url FROM media WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM media WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
