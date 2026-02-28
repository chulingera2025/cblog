use anyhow::Result;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

/// 文章写入参数
pub struct PostWriteParams<'a> {
    pub id: &'a str,
    pub slug: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub status: &'a str,
    pub meta: &'a str,
    pub tags_str: &'a str,
    pub category_str: &'a str,
}

/// 自动保存参数（不含 status）
pub struct PostAutosaveParams<'a> {
    pub id: &'a str,
    pub slug: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub meta: &'a str,
    pub tags_str: &'a str,
    pub category_str: &'a str,
}

#[derive(Clone)]
pub struct PostRepository {
    db: SqlitePool,
}

impl PostRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        page: u32,
        per_page: i32,
        status: Option<&str>,
        search: Option<&str>,
    ) -> Vec<sqlx::sqlite::SqliteRow> {
        let offset = (page as i32 - 1) * per_page;

        match (status, search) {
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
                .fetch_all(&self.db)
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
                .fetch_all(&self.db)
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
                .fetch_all(&self.db)
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
                .fetch_all(&self.db)
                .await
                .unwrap_or_default()
            }
        }
    }

    pub async fn get_by_id(&self, id: &str) -> Option<sqlx::sqlite::SqliteRow> {
        sqlx::query(
            "SELECT id, slug, title, content, status, meta FROM posts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
    }

    pub async fn create(&self, p: &PostWriteParams<'_>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        let mut tx = self.db.begin().await?;

        sqlx::query(
            "INSERT INTO posts (id, slug, title, content, status, created_at, updated_at, meta) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(p.id)
        .bind(p.slug)
        .bind(p.title)
        .bind(p.content)
        .bind(p.status)
        .bind(&now)
        .bind(&now)
        .bind(p.meta)
        .execute(&mut *tx)
        .await?;

        sync_post_taxonomy(&mut tx, p.id, p.tags_str, p.category_str).await;

        tx.commit().await?;
        Ok(())
    }

    pub async fn update(&self, p: &PostWriteParams<'_>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        let mut tx = self.db.begin().await?;

        sqlx::query(
            "UPDATE posts SET title = ?, slug = ?, content = ?, status = ?, meta = ?, updated_at = ? WHERE id = ?",
        )
        .bind(p.title)
        .bind(p.slug)
        .bind(p.content)
        .bind(p.status)
        .bind(p.meta)
        .bind(&now)
        .bind(p.id)
        .execute(&mut *tx)
        .await?;

        sync_post_taxonomy(&mut tx, p.id, p.tags_str, p.category_str).await;

        tx.commit().await?;
        Ok(())
    }

    /// 自动保存创建（草稿状态）
    pub async fn autosave_create(&self, p: &PostAutosaveParams<'_>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        let mut tx = self.db.begin().await?;

        sqlx::query(
            "INSERT INTO posts (id, slug, title, content, status, created_at, updated_at, meta) VALUES (?, ?, ?, ?, 'draft', ?, ?, ?)",
        )
        .bind(p.id)
        .bind(p.slug)
        .bind(p.title)
        .bind(p.content)
        .bind(&now)
        .bind(&now)
        .bind(p.meta)
        .execute(&mut *tx)
        .await?;

        sync_post_taxonomy(&mut tx, p.id, p.tags_str, p.category_str).await;

        tx.commit().await?;
        Ok(())
    }

    /// 自动保存更新（不改变 status）
    pub async fn autosave_update(&self, p: &PostAutosaveParams<'_>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        let mut tx = self.db.begin().await?;

        sqlx::query(
            "UPDATE posts SET title = ?, slug = ?, content = ?, meta = ?, updated_at = ? WHERE id = ?",
        )
        .bind(p.title)
        .bind(p.slug)
        .bind(p.content)
        .bind(p.meta)
        .bind(&now)
        .bind(p.id)
        .execute(&mut *tx)
        .await?;

        sync_post_taxonomy(&mut tx, p.id, p.tags_str, p.category_str).await;

        tx.commit().await?;
        Ok(())
    }

    /// 软删除（归档）
    pub async fn delete(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE posts SET status = 'archived', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn publish(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE posts SET status = 'published', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn unpublish(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE posts SET status = 'draft', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// 仪表盘统计：已发布/草稿文章数
    pub async fn count_by_status(&self) -> (i64, i64) {
        #[derive(sqlx::FromRow)]
        struct Counts {
            published: i64,
            draft: i64,
        }

        let counts = sqlx::query_as::<_, Counts>(
            "SELECT \
                (SELECT COUNT(*) FROM posts WHERE status = 'published') as published, \
                (SELECT COUNT(*) FROM posts WHERE status = 'draft') as draft",
        )
        .fetch_one(&self.db)
        .await
        .unwrap_or(Counts {
            published: 0,
            draft: 0,
        });

        (counts.published, counts.draft)
    }

    /// 仪表盘：最近更新的文章
    pub async fn recent(&self, limit: i32) -> Vec<sqlx::sqlite::SqliteRow> {
        sqlx::query(
            "SELECT id, title, status, updated_at FROM posts \
             WHERE status != 'archived' \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }

    /// 构建时获取已发布文章
    pub async fn fetch_published(&self) -> Vec<sqlx::sqlite::SqliteRow> {
        sqlx::query(
            "SELECT id, slug, title, content, status, created_at, updated_at, meta FROM posts WHERE status = 'published'",
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }
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

/// 同步文章的分类和标签关联表
async fn sync_post_taxonomy(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    post_id: &str,
    tags_str: &str,
    category_str: &str,
) {
    let tags: Vec<&str> = tags_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // -- 同步标签 --
    let _ = sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(&mut **tx)
        .await;

    if !tags.is_empty() {
        let now = chrono::Utc::now().to_rfc3339();

        let tag_data: Vec<(String, String, String)> = tags
            .iter()
            .map(|name| {
                let id = ulid::Ulid::new().to_string();
                let slug = generate_slug(name);
                (id, name.to_string(), slug)
            })
            .collect();

        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("INSERT OR IGNORE INTO tags (id, name, slug, description, created_at) ");
        qb.push_values(&tag_data, |mut b, (id, name, slug)| {
            b.push_bind(id)
                .push_bind(name)
                .push_bind(slug)
                .push_bind("")
                .push_bind(&now);
        });
        let _ = qb.build().execute(&mut **tx).await;

        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT id, name FROM tags WHERE name IN (");
        let mut separated = qb.separated(", ");
        for tag_name in &tags {
            separated.push_bind(*tag_name);
        }
        separated.push_unseparated(")");
        let tag_rows = qb.build().fetch_all(&mut **tx).await.unwrap_or_default();

        if !tag_rows.is_empty() {
            let mut qb: QueryBuilder<Sqlite> =
                QueryBuilder::new("INSERT OR IGNORE INTO post_tags (post_id, tag_id) ");
            qb.push_values(&tag_rows, |mut b, row| {
                let tag_id: &str = row.get("id");
                b.push_bind(post_id).push_bind(tag_id);
            });
            let _ = qb.build().execute(&mut **tx).await;
        }
    }

    // -- 同步分类 --
    let _ = sqlx::query("DELETE FROM post_categories WHERE post_id = ?")
        .bind(post_id)
        .execute(&mut **tx)
        .await;

    let category = category_str.trim();
    if !category.is_empty() {
        let slug = generate_slug(category);
        let now = chrono::Utc::now().to_rfc3339();
        let cat_id = ulid::Ulid::new().to_string();

        let _ = sqlx::query(
            "INSERT OR IGNORE INTO categories (id, name, slug, description, created_at) VALUES (?, ?, ?, '', ?)",
        )
        .bind(&cat_id)
        .bind(category)
        .bind(&slug)
        .bind(&now)
        .execute(&mut **tx)
        .await;

        let row: Option<(String,)> =
            sqlx::query_as("SELECT id FROM categories WHERE name = ?")
                .bind(category)
                .fetch_optional(&mut **tx)
                .await
                .ok()
                .flatten();

        if let Some((cid,)) = row {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO post_categories (post_id, category_id) VALUES (?, ?)",
            )
            .bind(post_id)
            .bind(&cid)
            .execute(&mut **tx)
            .await;
        }
    }
}
