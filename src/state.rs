use crate::admin::layout::PluginSidebarEntry;
use crate::admin::settings::SiteSettings;
use crate::build::events::BuildEvent;
use crate::config::SiteConfig;
use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
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
    /// 构建防抖计数器：用于判断是否有更新的构建请求
    pub build_request_counter: Arc<AtomicU64>,
    /// 构建互斥锁：确保任何时刻只有一个构建在执行
    pub build_mutex: Arc<tokio::sync::Mutex<()>>,
    /// 站点设置（从数据库加载，可动态修改）
    pub site_settings: Arc<tokio::sync::RwLock<SiteSettings>>,
    /// 安装状态缓存，避免每次请求查数据库
    pub installed: Arc<AtomicBool>,
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

        // 加载站点设置，DB 中没有的字段 fallback 到 config
        let mut site_settings = SiteSettings::load(&pool).await.unwrap_or_default();
        if site_settings.site_title.is_empty() {
            site_settings.site_title = config.site.title.clone();
        }
        if site_settings.site_url.is_empty() {
            site_settings.site_url = config.site.url.clone();
        }
        if site_settings.site_subtitle.is_empty() {
            site_settings.site_subtitle = config.site.subtitle.clone();
        }

        // 检查安装状态：users 表是否有记录
        let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));
        let installed = user_count.0 > 0;

        Ok(Self {
            db: pool,
            config: Arc::new(config),
            project_root,
            build_events,
            login_limiter: Arc::new(std::sync::Mutex::new(HashMap::new())),
            plugin_admin_pages,
            build_request_counter: Arc::new(AtomicU64::new(0)),
            build_mutex: Arc::new(tokio::sync::Mutex::new(())),
            site_settings: Arc::new(tokio::sync::RwLock::new(site_settings)),
            installed: Arc::new(AtomicBool::new(installed)),
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
