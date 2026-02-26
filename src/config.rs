use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SiteConfig {
    pub site: SiteInfo,
    pub build: BuildConfig,
    pub theme: ThemeRef,
    #[serde(default)]
    pub routes: RouteConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub feed: FeedConfig,
    #[serde(default)]
    pub sitemap: SitemapConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub media: MediaConfig,
    #[serde(default)]
    pub plugins: PluginConfig,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SiteInfo {
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub author: AuthorInfo,
}

#[derive(Debug, Default, Deserialize)]
pub struct AuthorInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub bio: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BuildConfig {
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
    #[serde(default = "default_posts_per_page")]
    pub posts_per_page: usize,
    #[serde(default = "default_date_format")]
    pub date_format: String,
    #[serde(default = "default_excerpt_length")]
    pub excerpt_length: usize,
    #[serde(default = "default_true")]
    pub parallel: bool,
}

#[derive(Debug, Deserialize)]
pub struct ThemeRef {
    pub active: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RouteConfig {
    #[serde(default = "default_post_url")]
    pub post_url: String,
    #[serde(default = "default_tag_url")]
    pub tag_url: String,
    #[serde(default = "default_category_url")]
    pub category_url: String,
    #[serde(default = "default_archive_url")]
    pub archive_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Deserialize)]
pub struct FeedConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_feed_formats")]
    pub format: Vec<String>,
    #[serde(default = "default_feed_count")]
    pub post_count: usize,
    #[serde(default)]
    pub full_content: bool,
}

#[derive(Debug, Deserialize)]
pub struct SitemapConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_change_freq")]
    pub change_freq: String,
    #[serde(default = "default_priority")]
    pub priority: f32,
}

impl SiteConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join("cblog.toml");
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| anyhow::anyhow!("读取 cblog.toml 失败：{}", e))?;
        let config: SiteConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("解析 cblog.toml 失败：{}", e))?;
        Ok(config)
    }
}

// 默认值函数
fn default_language() -> String { "zh-CN".into() }
fn default_timezone() -> String { "Asia/Shanghai".into() }
fn default_output_dir() -> String { "public".into() }
fn default_cache_dir() -> String { ".cblog-cache".into() }
fn default_posts_per_page() -> usize { 10 }
fn default_date_format() -> String { "Y年m月d日".into() }
fn default_excerpt_length() -> usize { 160 }
fn default_true() -> bool { true }
fn default_post_url() -> String { "/posts/{slug}/".into() }
fn default_tag_url() -> String { "/tags/{slug}/".into() }
fn default_category_url() -> String { "/category/{slug}/".into() }
fn default_archive_url() -> String { "/archive/{year}/{month}/".into() }
fn default_host() -> String { "127.0.0.1".into() }
fn default_port() -> u16 { 3000 }
fn default_log_level() -> String { "info".into() }
fn default_feed_formats() -> Vec<String> { vec!["rss".into(), "atom".into()] }
fn default_feed_count() -> usize { 20 }
fn default_change_freq() -> String { "weekly".into() }
fn default_priority() -> f32 { 0.8 }
fn default_jwt_secret() -> String { "CHANGE_ME_IN_PRODUCTION".into() }
fn default_jwt_expires_in() -> String { "7d".into() }
fn default_session_name() -> String { "cblog_session".into() }
fn default_upload_dir() -> String { "media".into() }
fn default_max_file_size() -> String { "10MB".into() }
fn default_allowed_types() -> Vec<String> {
    vec![
        "image/jpeg".into(),
        "image/png".into(),
        "image/gif".into(),
        "image/webp".into(),
    ]
}
fn default_webp_quality() -> u8 { 85 }
fn default_thumb_width() -> u32 { 400 }

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            post_url: default_post_url(),
            tag_url: default_tag_url(),
            category_url: default_category_url(),
            archive_url: default_archive_url(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
        }
    }
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: default_feed_formats(),
            post_count: default_feed_count(),
            full_content: false,
        }
    }
}

impl Default for SitemapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            change_freq: default_change_freq(),
            priority: default_priority(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expires_in")]
    pub jwt_expires_in: String,
    #[serde(default = "default_session_name")]
    pub session_name: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            jwt_expires_in: default_jwt_expires_in(),
            session_name: default_session_name(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct MediaConfig {
    #[serde(default = "default_upload_dir")]
    pub upload_dir: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: String,
    #[serde(default = "default_allowed_types")]
    pub allowed_types: Vec<String>,
    #[serde(default = "default_true")]
    pub auto_webp: bool,
    #[serde(default = "default_webp_quality")]
    pub webp_quality: u8,
    #[serde(default = "default_true")]
    pub generate_thumb: bool,
    #[serde(default = "default_thumb_width")]
    pub thumb_width: u32,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            upload_dir: default_upload_dir(),
            max_file_size: default_max_file_size(),
            allowed_types: default_allowed_types(),
            auto_webp: true,
            webp_quality: default_webp_quality(),
            generate_thumb: true,
            thumb_width: default_thumb_width(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PluginConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: Vec::new(),
        }
    }
}
