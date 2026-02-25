use crate::config::SiteConfig;
use crate::content::frontmatter;
use crate::content::markdown;
use crate::content::excerpt;
use crate::content::{MarkdownContent, Post, PostStatus};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;
use ulid::Ulid;

/// 加载 content/posts/ 下的所有 Markdown 文章
pub fn load_posts(project_root: &Path, config: &SiteConfig) -> Result<Vec<Post>> {
    let posts_dir = project_root.join("content/posts");
    if !posts_dir.exists() {
        tracing::warn!("文章目录不存在：{}", posts_dir.display());
        return Ok(Vec::new());
    }

    let mut posts = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&posts_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "md")
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        match load_single_post(&entry.path(), config) {
            Ok(post) => {
                if post.status == PostStatus::Draft {
                    tracing::debug!("跳过草稿：{}", post.slug);
                    continue;
                }
                posts.push(post);
            }
            Err(e) => {
                tracing::error!("加载文章失败 {}: {}", entry.path().display(), e);
            }
        }
    }

    // 按创建时间降序排列
    posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(posts)
}

fn load_single_post(path: &Path, config: &SiteConfig) -> Result<Post> {
    let parsed = frontmatter::parse_file(path)?;
    let fm = parsed.front_matter;

    let filename = path.file_name().unwrap().to_string_lossy();
    let slug = fm.slug.unwrap_or_else(|| frontmatter::slug_from_filename(&filename));
    let title = fm.title.unwrap_or_else(|| slug.clone());

    let now = Utc::now();
    let created_at = fm.date
        .as_deref()
        .map(frontmatter::parse_date)
        .transpose()?
        .unwrap_or(now);
    let updated_at = fm.updated
        .as_deref()
        .map(frontmatter::parse_date)
        .transpose()?
        .unwrap_or(created_at);

    let status = if fm.draft.unwrap_or(false) {
        PostStatus::Draft
    } else {
        PostStatus::Published
    };

    let md_content = MarkdownContent::new(parsed.body);
    let html = md_content.html().to_string();

    let word_count = markdown::count_words(&html);
    let reading_time = markdown::reading_time(word_count);
    let toc = markdown::extract_toc(&md_content.raw);
    let auto_excerpt = excerpt::extract_excerpt(&html, config.build.excerpt_length);

    Ok(Post {
        id: Ulid::new(),
        slug,
        title,
        content: md_content,
        status,
        created_at,
        updated_at,
        tags: fm.tags.unwrap_or_default(),
        category: fm.category,
        cover_image: fm.cover_image,
        excerpt: Some(fm.excerpt.unwrap_or(auto_excerpt)),
        author: fm.author,
        template: fm.template,
        layout: fm.layout,
        reading_time,
        word_count,
        toc,
        meta: fm.extra.into_iter().map(|(k, v)| (k, v)).collect(),
    })
}
