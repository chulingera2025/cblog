use crate::admin::layout::PluginSidebarEntry;
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
    /// 插件注册的后台侧边栏页面
    pub plugin_admin_pages: Vec<PluginSidebarEntry>,
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

        // 扫描已启用插件的 admin 页面声明
        let plugin_admin_pages = collect_plugin_admin_pages(&project_root, &config);

        Ok(Self {
            db: pool,
            config: Arc::new(config),
            project_root,
            build_events,
            login_limiter: Arc::new(std::sync::Mutex::new(HashMap::new())),
            plugin_admin_pages,
        })
    }
}

fn collect_plugin_admin_pages(project_root: &std::path::Path, config: &SiteConfig) -> Vec<PluginSidebarEntry> {
    use crate::plugin::registry::load_plugin_toml;

    let mut pages = Vec::new();
    for name in &config.plugins.enabled {
        let toml_path = project_root.join("plugins").join(name).join("plugin.toml");
        if let Ok(toml) = load_plugin_toml(&toml_path) {
            for page in &toml.admin.pages {
                pages.push(PluginSidebarEntry {
                    plugin_name: name.clone(),
                    label: page.label.clone(),
                    href: format!("/admin/ext/{}/{}", name, page.slug),
                    icon: page.icon.clone(),
                });
            }
        }
    }
    pages
}
