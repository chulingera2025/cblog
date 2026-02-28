use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct CategoryRepository {
    db: SqlitePool,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

impl CategoryRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list_with_counts(
        &self,
        per_page: i32,
        offset: i32,
    ) -> Vec<sqlx::sqlite::SqliteRow> {
        sqlx::query(
            "SELECT c.id, c.name, c.slug, c.description, c.parent_id, c.created_at, \
             (SELECT COUNT(*) FROM post_categories pc WHERE pc.category_id = c.id) AS post_count, \
             p.name AS parent_name \
             FROM categories c \
             LEFT JOIN categories p ON c.parent_id = p.id \
             ORDER BY c.created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }

    pub async fn get_by_id(&self, id: &str) -> Option<Category> {
        sqlx::query_as::<_, Category>(
            "SELECT id, name, slug, description, parent_id, created_at FROM categories WHERE id = ?",
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
        parent_id: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO categories (id, name, slug, description, parent_id, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(slug)
        .bind(description)
        .bind(parent_id)
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
        parent_id: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE categories SET name = ?, slug = ?, description = ?, parent_id = ? WHERE id = ?",
        )
        .bind(name)
        .bind(slug)
        .bind(description)
        .bind(parent_id)
        .bind(id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn list_all(&self) -> Vec<Category> {
        sqlx::query_as::<_, Category>(
            "SELECT id, name, slug, description, parent_id, created_at FROM categories ORDER BY name",
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }
}
