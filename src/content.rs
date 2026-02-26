pub mod excerpt;
pub mod frontmatter;
pub mod markdown;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use ulid::Ulid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PostStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Debug)]
pub struct MarkdownContent {
    pub raw: String,
    html: OnceLock<String>,
}

impl MarkdownContent {
    pub fn new(raw: String) -> Self {
        Self {
            raw,
            html: OnceLock::new(),
        }
    }

    pub fn html(&self) -> &str {
        self.html.get_or_init(|| markdown::render_markdown(&self.raw))
    }

    pub fn set_html(&self, html: String) {
        let _ = self.html.set(html);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Post {
    pub id: Ulid,
    pub slug: String,
    pub title: String,
    pub content: MarkdownContent,
    pub status: PostStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub cover_image: Option<String>,
    pub excerpt: Option<String>,
    pub author: Option<String>,
    pub template: Option<String>,
    pub layout: Option<String>,
    pub reading_time: u32,
    pub word_count: u32,
    pub toc: Option<String>,
    pub meta: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Page {
    pub id: Ulid,
    pub slug: String,
    pub title: String,
    pub content: MarkdownContent,
    pub status: PostStatus,
    pub template: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 供模板渲染用的文章简要引用
#[derive(Debug, Clone, Serialize)]
pub struct PostRef {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub url: String,
    pub excerpt: Option<String>,
    pub cover_image: Option<String>,
    pub created_at: String,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub reading_time: u32,
}

/// 分类索引
#[derive(Debug, Default)]
pub struct TaxonomyIndex {
    pub tags: HashMap<String, Vec<PostRef>>,
    pub categories: HashMap<String, Vec<PostRef>>,
    pub archives: std::collections::BTreeMap<(i32, u32), Vec<PostRef>>,
}
