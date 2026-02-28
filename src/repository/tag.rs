use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct TagRepository {
    db: SqlitePool,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub created_at: String,
}

impl TagRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list_with_counts(
        &self,
        per_page: i32,
        offset: i32,
    ) -> Vec<sqlx::sqlite::SqliteRow> {
        sqlx::query(
            "SELECT t.id, t.name, t.slug, t.description, t.created_at, \
             (SELECT COUNT(*) FROM post_tags pt WHERE pt.tag_id = t.id) AS post_count \
             FROM tags t \
             ORDER BY t.created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }

    pub async fn get_by_id(&self, id: &str) -> Option<Tag> {
        sqlx::query_as::<_, Tag>(
            "SELECT id, name, slug, description, created_at FROM tags WHERE id = ?",
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
        name: &str,
        slug: &str,
        description: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO tags (id, name, slug, description, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(slug)
        .bind(description)
        .bind(&now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn update(
        &self,
        id: &str,
        name: &str,
        slug: &str,
        description: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE tags SET name = ?, slug = ?, description = ? WHERE id = ?",
        )
        .bind(name)
        .bind(slug)
        .bind(description)
        .bind(id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM tags WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn list_all(&self) -> Vec<Tag> {
        sqlx::query_as::<_, Tag>(
            "SELECT id, name, slug, description, created_at FROM tags ORDER BY name",
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }
}
