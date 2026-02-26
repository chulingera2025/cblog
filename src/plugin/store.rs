use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// 插件 KV 存储操作（基于 plugin_store 表）
pub struct PluginStore;

#[allow(dead_code)]
impl PluginStore {
    /// 获取指定插件的某个 key
    pub async fn get(db: &SqlitePool, plugin_name: &str, key: &str) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query("SELECT value FROM plugin_store WHERE plugin_name = ? AND key = ?")
            .bind(plugin_name)
            .bind(key)
            .fetch_optional(db)
            .await?;

        match row {
            Some(row) => {
                let value_str: String = sqlx::Row::get(&row, "value");
                let value: serde_json::Value = serde_json::from_str(&value_str)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// 设置指定插件的某个 key（UPSERT）
    pub async fn set(
        db: &SqlitePool,
        plugin_name: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let value_str = serde_json::to_string(value)?;

        sqlx::query(
            "INSERT INTO plugin_store (plugin_name, key, value, updated_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT(plugin_name, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(plugin_name)
        .bind(key)
        .bind(&value_str)
        .bind(&now)
        .execute(db)
        .await?;

        Ok(())
    }

    /// 删除指定插件的某个 key
    pub async fn delete(db: &SqlitePool, plugin_name: &str, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM plugin_store WHERE plugin_name = ? AND key = ?")
            .bind(plugin_name)
            .bind(key)
            .execute(db)
            .await?;
        Ok(())
    }

    /// 列出指定插件的所有 key
    pub async fn keys(db: &SqlitePool, plugin_name: &str) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT key FROM plugin_store WHERE plugin_name = ? ORDER BY key")
            .bind(plugin_name)
            .fetch_all(db)
            .await?;

        Ok(rows.iter().map(|r| sqlx::Row::get(r, "key")).collect())
    }

    /// 获取指定插件的所有 KV 对
    pub async fn get_all(
        db: &SqlitePool,
        plugin_name: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let rows =
            sqlx::query("SELECT key, value FROM plugin_store WHERE plugin_name = ? ORDER BY key")
                .bind(plugin_name)
                .fetch_all(db)
                .await?;

        let mut map = HashMap::new();
        for row in &rows {
            let key: String = sqlx::Row::get(row, "key");
            let value_str: String = sqlx::Row::get(row, "value");
            if let Ok(value) = serde_json::from_str(&value_str) {
                map.insert(key, value);
            }
        }
        Ok(map)
    }
}
