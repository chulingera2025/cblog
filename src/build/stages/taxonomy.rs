use crate::config::SiteConfig;
use crate::content::{Post, PostRef, TaxonomyIndex};
use chrono::Datelike;
use std::collections::HashMap;

/// 构建标签、分类和时间归档索引
pub fn build_taxonomy(posts: &[Post], _config: &SiteConfig) -> TaxonomyIndex {
    let mut tags: HashMap<String, Vec<PostRef>> = HashMap::new();
    let mut categories: HashMap<String, Vec<PostRef>> = HashMap::new();
    let mut archives: std::collections::BTreeMap<(i32, u32), Vec<PostRef>> =
        std::collections::BTreeMap::new();

    for post in posts {
        let post_ref = post_to_ref(post);

        for tag in &post.tags {
            tags.entry(tag.clone())
                .or_default()
                .push(post_ref.clone());
        }

        if let Some(cat) = &post.category {
            categories
                .entry(cat.clone())
                .or_default()
                .push(post_ref.clone());
        }

        let year = post.created_at.year();
        let month = post.created_at.month();
        archives
            .entry((year, month))
            .or_default()
            .push(post_ref);
    }

    TaxonomyIndex {
        tags,
        categories,
        archives,
    }
}

fn post_to_ref(post: &Post) -> PostRef {
    PostRef {
        id: post.id.to_string(),
        slug: post.slug.clone(),
        title: post.title.clone(),
        url: format!("/posts/{}/", post.slug),
        excerpt: post.excerpt.clone(),
        cover_image: post.cover_image.clone(),
        created_at: post.created_at.to_rfc3339(),
        tags: post.tags.clone(),
        category: post.category.clone(),
        reading_time: post.reading_time,
    }
}
