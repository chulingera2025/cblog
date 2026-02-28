use anyhow::Result;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

#[derive(Clone)]
pub struct BuildRepository {
    db: SqlitePool,
}

/// 构建记录插入参数
pub struct BuildHistoryParams<'a> {
    pub id: &'a str,
    pub trigger: &'a str,
    pub status: &'a str,
    pub duration_ms: Option<i64>,
    pub error: Option<&'a str>,
    pub started_at: &'a str,
    pub finished_at: &'a str,
    pub total_pages: Option<i64>,
    pub rebuilt: Option<i64>,
    pub cached: Option<i64>,
}

impl BuildRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list_history(&self, limit: i32) -> Vec<sqlx::sqlite::SqliteRow> {
        sqlx::query(
            "SELECT trigger, status, duration_ms, error, started_at, finished_at \
             FROM build_history ORDER BY started_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default()
    }

    pub async fn insert_history(&self, p: &BuildHistoryParams<'_>) -> Result<()> {
        sqlx::query(
            "INSERT INTO build_history (id, trigger, status, duration_ms, error, started_at, finished_at, total_pages, rebuilt, cached) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(p.id)
        .bind(p.trigger)
        .bind(p.status)
        .bind(p.duration_ms)
        .bind(p.error)
        .bind(p.started_at)
        .bind(p.finished_at)
        .bind(p.total_pages)
        .bind(p.rebuilt)
        .bind(p.cached)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// 加载主题配置
    pub async fn load_theme_config(&self, theme_name: &str) -> HashMap<String, serde_json::Value> {
        sqlx::query("SELECT config FROM theme_config WHERE theme_name = ?")
            .bind(theme_name)
            .fetch_optional(&self.db)
            .await
            .ok()
            .flatten()
            .and_then(|row| {
                let json_str: String = row.get("config");
                serde_json::from_str(&json_str).ok()
            })
            .unwrap_or_default()
    }

    /// 保存主题配置
    pub async fn save_theme_config(&self, theme_name: &str, config_json: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO theme_config (theme_name, config) VALUES (?, ?) \
             ON CONFLICT(theme_name) DO UPDATE SET config = excluded.config",
        )
        .bind(theme_name)
        .bind(config_json)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
