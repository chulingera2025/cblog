use crate::build::events::BuildEvent;
use crate::config::SiteConfig;
use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<SiteConfig>,
    pub project_root: PathBuf,
    pub build_events: broadcast::Sender<BuildEvent>,
    /// 登录速率限制：IP -> 登录尝试时间戳列表
    pub login_limiter: Arc<std::sync::Mutex<HashMap<String, Vec<Instant>>>>,
}

impl AppState {
    pub async fn new(project_root: PathBuf, config: SiteConfig) -> Result<Self> {
        let db_path = project_root.join("cblog.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePool::connect(&db_url).await?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("数据库迁移失败：{}", e))?;

        let (build_events, _) = broadcast::channel::<BuildEvent>(64);

        Ok(Self {
            db: pool,
            config: Arc::new(config),
            project_root,
            build_events,
            login_limiter: Arc::new(std::sync::Mutex::new(HashMap::new())),
        })
    }
}
