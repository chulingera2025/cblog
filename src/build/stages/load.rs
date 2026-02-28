use crate::config::SiteConfig;
use crate::content::excerpt;
use crate::content::markdown;
use crate::content::{MarkdownContent, Post, PostStatus};
use chrono::DateTime;
use std::collections::HashMap;
use std::path::Path;
use ulid::Ulid;

/// 数据库文章的简化结构，用于从异步上下文传递给同步构建函数
#[derive(Clone)]
pub struct DbPost {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub meta: serde_json::Value,
}

/// 从预取的数据库文章构建 Post 列表
pub fn load_posts_from_db(db_posts: Vec<DbPost>, config: &SiteConfig) -> Vec<Post> {
    let mut posts = Vec::new();

    for db_post in db_posts {
        if db_post.status != "published" {
            continue;
        }

        let id = db_post.id.parse::<Ulid>().unwrap_or_else(|_| Ulid::new());

        let created_at = DateTime::parse_from_rfc3339(&db_post.created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let updated_at = DateTime::parse_from_rfc3339(&db_post.updated_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(created_at);

        let tags: Vec<String> = db_post.meta["tags"]
            .as_str()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let category = db_post.meta["category"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let cover_image = db_post.meta["cover_image"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let meta_excerpt = db_post.meta["excerpt"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let html_content = &db_post.content;

        let word_count = markdown::count_words_html(html_content);
        let reading_time = markdown::reading_time(word_count);
        let toc = markdown::extract_toc_from_html(html_content);
        let auto_excerpt = excerpt::extract_excerpt(html_content, config.build.excerpt_length);

        let md_content = MarkdownContent::new(String::new());
        md_content.set_html(db_post.content);

        let post = Post {
            id,
            slug: db_post.slug,
            title: db_post.title,
            content: md_content,
            status: PostStatus::Published,
            created_at,
            updated_at,
            tags,
            category,
            cover_image,
            excerpt: Some(meta_excerpt.unwrap_or(auto_excerpt)),
            author: None,
            template: None,
            layout: None,
            reading_time,
            word_count,
            toc,
            meta: HashMap::new(),
        };

        posts.push(post);
    }

    // 按创建时间降序排列
    posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    posts
}

/// 同步从数据库预取发布状态的文章（用于 CLI build 命令等无 async runtime 的场景）
pub fn fetch_db_posts_sync(db_path: &Path) -> Vec<DbPost> {
    if !db_path.exists() {
        return Vec::new();
    }
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Vec::new();
    };
    rt.block_on(async {
        let db_url = format!("sqlite:{}?mode=ro", db_path.display());
        let Ok(pool) = sqlx::SqlitePool::connect(&db_url).await else {
            return Vec::new();
        };
        let rows = sqlx::query(
            "SELECT id, slug, title, content, status, created_at, updated_at, meta FROM posts WHERE status = 'published'"
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        use sqlx::Row;
        rows.into_iter()
            .map(|row| {
                let meta_str: String = row.get("meta");
                let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();
                DbPost {
                    id: row.get("id"),
                    slug: row.get("slug"),
                    title: row.get("title"),
                    content: row.get("content"),
                    status: row.get("status"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    meta,
                }
            })
            .collect()
    })
}
