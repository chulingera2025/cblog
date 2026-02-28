use crate::admin::layout::PluginSidebarEntry;
use crate::admin::settings::SiteSettings;
use crate::build::events::BuildEvent;
use crate::config::SiteConfig;
use anyhow::Result;
use minijinja::Environment;
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
    /// 后台模板渲染环境
    pub admin_env: Arc<Environment<'static>>,
    /// 实际使用的 JWT 密钥（优先配置文件，其次数据库持久化自动生成）
    pub jwt_secret: Arc<String>,
    /// 站点是否通过 HTTPS 提供服务（根据 site_url 判断）
    pub is_https: bool,
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

        // 将 posts.meta 中的 tags/category 迁移到关联表（幂等操作）
        migrate_post_taxonomy(&pool).await;

        // 解析 JWT secret：配置文件显式设置 > 数据库持久化 > 自动生成
        let jwt_secret = resolve_jwt_secret(&config.auth.jwt_secret, &pool).await?;

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

        // 构建后台模板渲染环境（从当前激活主题加载后台模板）
        let admin_env = crate::admin::template::build_admin_env(
            &project_root,
            &config.theme.active,
            &config.site.url,
        )?;

        let is_https = config.site.url.starts_with("https://");

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
            admin_env: Arc::new(admin_env),
            jwt_secret: Arc::new(jwt_secret),
            is_https,
        })
    }
}

const DEFAULT_JWT_SECRET: &str = "CHANGE_ME_IN_PRODUCTION";

/// 配置文件显式设置 > 数据库持久化 > 自动生成新密钥
async fn resolve_jwt_secret(config_secret: &str, db: &SqlitePool) -> Result<String> {
    if config_secret != DEFAULT_JWT_SECRET && !config_secret.is_empty() {
        return Ok(config_secret.to_owned());
    }

    tracing::warn!("JWT secret 未配置或为默认值，将使用自动生成的安全密钥");

    // 尝试从数据库读取已持久化的密钥
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT value FROM site_settings WHERE key = 'jwt_secret'")
            .fetch_optional(db)
            .await?;

    if let Some((secret,)) = existing {
        return Ok(secret);
    }

    // 生成新密钥并持久化到数据库
    let secret = generate_random_secret();
    sqlx::query(
        "INSERT INTO site_settings (key, value) VALUES ('jwt_secret', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&secret)
    .execute(db)
    .await?;

    tracing::info!("已自动生成 JWT secret 并持久化到数据库");
    Ok(secret)
}

fn generate_random_secret() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};

    let mut bytes = [0u8; 64];
    OsRng.fill_bytes(&mut bytes);
    // 转为 hex 字符串（128 字符）
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn collect_plugin_admin_pages(project_root: &std::path::Path, config: &SiteConfig) -> Vec<PluginSidebarEntry> {
    use crate::plugin::registry::load_plugin_toml;

    let mut pages = Vec::new();
    for name in &config.plugins.enabled {
        let toml_path = project_root.join("plugins").join(name).join("plugin.toml");
        if let Ok(toml) = load_plugin_toml(&toml_path) {
            for page in &toml.admin.pages {
                pages.push(PluginSidebarEntry {
                    label: page.label.clone(),
                    href: format!("/admin/ext/{}/{}", name, page.slug),
                    icon: page.icon.clone(),
                });
            }
        }
    }
    pages
}

/// 从 posts.meta JSON 中的 tags/category 迁移到独立关联表
/// 仅对尚未在关联表中存在的文章做迁移，已有关联的跳过
/// 整个迁移在单个事务中完成，确保数据一致性
async fn migrate_post_taxonomy(pool: &SqlitePool) {
    use sqlx::Row;

    let rows = sqlx::query("SELECT id, meta FROM posts")
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    if rows.is_empty() {
        return;
    }

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("迁移事务开启失败：{e}");
            return;
        }
    };

    for row in rows {
        let post_id: String = row.get("id");
        let meta_str: String = row.get("meta");
        let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();

        // 迁移标签
        let tags_str = meta["tags"].as_str().unwrap_or("");
        let tags: Vec<&str> = tags_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        for tag_name in tags {
            let slug = generate_slug(tag_name);
            let tag_id = ensure_tag(&mut tx, tag_name, &slug).await;
            if let Some(tid) = tag_id {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)",
                )
                .bind(&post_id)
                .bind(&tid)
                .execute(&mut *tx)
                .await;
            }
        }

        // 迁移分类
        let category_str = meta["category"].as_str().unwrap_or("").trim();
        if !category_str.is_empty() {
            let slug = generate_slug(category_str);
            let cat_id = ensure_category(&mut tx, category_str, &slug).await;
            if let Some(cid) = cat_id {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO post_categories (post_id, category_id) VALUES (?, ?)",
                )
                .bind(&post_id)
                .bind(&cid)
                .execute(&mut *tx)
                .await;
            }
        }
    }

    if let Err(e) = tx.commit().await {
        tracing::error!("迁移事务提交失败：{e}");
    }
}

async fn ensure_tag(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, name: &str, slug: &str) -> Option<String> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM tags WHERE name = ?")
            .bind(name)
            .fetch_optional(&mut **tx)
            .await
            .ok()?;

    if let Some((id,)) = existing {
        return Some(id);
    }

    let id = ulid::Ulid::new().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO tags (id, name, slug, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(slug)
        .bind(&now)
        .execute(&mut **tx)
        .await
        .ok()?;
    Some(id)
}

async fn ensure_category(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, name: &str, slug: &str) -> Option<String> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM categories WHERE name = ?")
            .bind(name)
            .fetch_optional(&mut **tx)
            .await
            .ok()?;

    if let Some((id,)) = existing {
        return Some(id);
    }

    let id = ulid::Ulid::new().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO categories (id, name, slug, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(slug)
        .bind(&now)
        .execute(&mut **tx)
        .await
        .ok()?;
    Some(id)
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
