use crate::config::SiteConfig;
use crate::content::{Post, TaxonomyIndex};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderPage {
    pub url: String,
    pub template: String,
    pub context: serde_json::Value,
}

/// 根据文章和分类索引生成所有需要渲染的页面
pub fn generate_pages(
    posts: &[Post],
    taxonomy: &TaxonomyIndex,
    config: &SiteConfig,
) -> Vec<RenderPage> {
    let mut pages = Vec::new();

    // 文章页
    for (i, post) in posts.iter().enumerate() {
        let prev = if i + 1 < posts.len() { Some(post_to_ctx(&posts[i + 1])) } else { None };
        let next = if i > 0 { Some(post_to_ctx(&posts[i - 1])) } else { None };

        let template = post.template.clone().unwrap_or_else(|| "post".into());
        let context = serde_json::json!({
            "post": post_to_ctx(post),
            "prev_post": prev,
            "next_post": next,
            "page": {
                "title": post.title,
                "url": format!("/posts/{}/", post.slug),
                "type": "post",
            },
        });

        pages.push(RenderPage {
            url: format!("/posts/{}/", post.slug),
            template,
            context,
        });
    }

    // 首页 + 分页
    let per_page = config.build.posts_per_page;
    let total_pages = ((posts.len() as f64) / per_page as f64).ceil() as usize;
    let total_pages = total_pages.max(1);

    for page_num in 1..=total_pages {
        let start = (page_num - 1) * per_page;
        let end = (start + per_page).min(posts.len());
        let page_posts: Vec<_> = posts[start..end].iter().map(post_to_ctx).collect();

        let url = if page_num == 1 {
            "/".into()
        } else {
            format!("/page/{}/", page_num)
        };

        let pagination = serde_json::json!({
            "current": page_num,
            "total_pages": total_pages,
            "total_posts": posts.len(),
            "prev": if page_num > 1 { Some(if page_num == 2 { "/".into() } else { format!("/page/{}/", page_num - 1) }) } else { None::<String> },
            "next": if page_num < total_pages { Some(format!("/page/{}/", page_num + 1)) } else { None::<String> },
        });

        pages.push(RenderPage {
            url,
            template: "index".into(),
            context: serde_json::json!({
                "posts": page_posts,
                "pagination": pagination,
                "page": {
                    "title": if page_num == 1 { config.site.title.clone() } else { format!("第 {} 页", page_num) },
                    "url": if page_num == 1 { "/".into() } else { format!("/page/{}/", page_num) },
                    "type": "index",
                },
            }),
        });
    }

    // 标签归档页
    for (tag, tag_posts) in &taxonomy.tags {
        let slug = crate::cbtml::filters::filter_slugify(tag.clone());
        pages.push(RenderPage {
            url: format!("/tags/{}/", slug),
            template: "tag".into(),
            context: serde_json::json!({
                "tag": tag,
                "posts": tag_posts,
                "page": {
                    "title": format!("标签：{}", tag),
                    "url": format!("/tags/{}/", slug),
                    "type": "tag",
                },
            }),
        });
    }

    // 分类归档页
    for (cat, cat_posts) in &taxonomy.categories {
        let slug = crate::cbtml::filters::filter_slugify(cat.clone());
        pages.push(RenderPage {
            url: format!("/category/{}/", slug),
            template: "category".into(),
            context: serde_json::json!({
                "category": cat,
                "posts": cat_posts,
                "page": {
                    "title": format!("分类：{}", cat),
                    "url": format!("/category/{}/", slug),
                    "type": "category",
                },
            }),
        });
    }

    // 时间归档页
    for ((year, month), archive_posts) in &taxonomy.archives {
        pages.push(RenderPage {
            url: format!("/archive/{}/{:02}/", year, month),
            template: "archive".into(),
            context: serde_json::json!({
                "year": year,
                "month": month,
                "posts": archive_posts,
                "page": {
                    "title": format!("{}年{}月", year, month),
                    "url": format!("/archive/{}/{:02}/", year, month),
                    "type": "archive",
                },
            }),
        });
    }

    pages
}

fn post_to_ctx(post: &Post) -> serde_json::Value {
    serde_json::json!({
        "id": post.id.to_string(),
        "slug": post.slug,
        "title": post.title,
        "content": post.content.html(),
        "excerpt": post.excerpt,
        "cover_image": post.cover_image,
        "created_at": post.created_at.to_rfc3339(),
        "updated_at": post.updated_at.to_rfc3339(),
        "tags": post.tags,
        "category": post.category,
        "author": post.author,
        "reading_time": post.reading_time,
        "word_count": post.word_count,
        "toc": post.toc,
        "url": format!("/posts/{}/", post.slug),
    })
}
