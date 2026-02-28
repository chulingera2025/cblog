use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct PageRepository {
    db: SqlitePool,
}

impl PageRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        page: u32,
        per_page: i32,
        status: Option<&str>,
    ) -> Vec<sqlx::sqlite::SqliteRow> {
        let offset = (page as i32 - 1) * per_page;

        match status {
            Some(status) => {
                sqlx::query(
                    "SELECT id, title, slug, status, template, updated_at FROM pages \
                     WHERE status = ? ORDER BY updated_at DESC LIMIT ? OFFSET ?",
                )
                .bind(status)
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.db)
                .await
                .unwrap_or_default()
            }
            None => {
                sqlx::query(
                    "SELECT id, title, slug, status, template, updated_at FROM pages \
                     WHERE status != 'archived' ORDER BY updated_at DESC LIMIT ? OFFSET ?",
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
            "SELECT id, slug, title, content, status, template FROM pages WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
    }

    pub async fn create(
        &self,
        id: &str,
        slug: &str,
        title: &str,
        content: &str,
        status: &str,
        template: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO pages (id, slug, title, content, status, template, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(slug)
        .bind(title)
        .bind(content)
        .bind(status)
        .bind(template)
        .bind(&now)
        .bind(&now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn update(
        &self,
        id: &str,
        slug: &str,
        title: &str,
        content: &str,
        status: &str,
        template: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE pages SET title = ?, slug = ?, content = ?, status = ?, template = ?, updated_at = ? WHERE id = ?",
        )
        .bind(title)
        .bind(slug)
        .bind(content)
        .bind(status)
        .bind(template)
        .bind(&now)
        .bind(id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE pages SET status = 'archived', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// 仪表盘统计
    pub async fn count_active(&self) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pages WHERE status != 'archived'")
            .fetch_one(&self.db)
            .await
            .unwrap_or(0)
    }
}
