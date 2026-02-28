use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SettingsRepository {
    db: SqlitePool,
}

#[allow(dead_code)]
impl SettingsRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn load_all(&self) -> Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM site_settings")
                .fetch_all(&self.db)
                .await?;
        Ok(rows)
    }

    pub async fn save_pair(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO site_settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// 在事务中保存多个设置项
    pub async fn save_pairs_tx(&self, pairs: &[(&str, &str)]) -> Result<(), sqlx::Error> {
        let mut tx = self.db.begin().await?;

        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO site_settings (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(*key)
            .bind(*value)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
