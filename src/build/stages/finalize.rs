use crate::config::SiteConfig;
use crate::content::Post;
use anyhow::Result;
use std::path::Path;

/// 构建收尾：生成 sitemap.xml、feed.xml 等
pub fn finalize(project_root: &Path, config: &SiteConfig, posts: &[Post]) -> Result<()> {
    let output_dir = project_root.join(&config.build.output_dir);

    if config.sitemap.enabled {
        generate_sitemap(&output_dir, config, posts)?;
    }

    if config.feed.enabled {
        generate_feed(&output_dir, config, posts)?;
    }

    Ok(())
}

fn generate_sitemap(output_dir: &Path, config: &SiteConfig, posts: &[Post]) -> Result<()> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    // 首页
    xml.push_str(&format!(
        "  <url>\n    <loc>{}/</loc>\n    <changefreq>{}</changefreq>\n    <priority>1.0</priority>\n  </url>\n",
        config.site.url, config.sitemap.change_freq
    ));

    // 文章页
    for post in posts {
        xml.push_str(&format!(
            "  <url>\n    <loc>{}/posts/{}/</loc>\n    <lastmod>{}</lastmod>\n    <changefreq>{}</changefreq>\n    <priority>{}</priority>\n  </url>\n",
            config.site.url,
            post.slug,
            post.updated_at.format("%Y-%m-%d"),
            config.sitemap.change_freq,
            config.sitemap.priority
        ));
    }

    xml.push_str("</urlset>\n");
    std::fs::write(output_dir.join("sitemap.xml"), xml)?;
    tracing::info!("已生成 sitemap.xml");
    Ok(())
}

fn generate_feed(output_dir: &Path, config: &SiteConfig, posts: &[Post]) -> Result<()> {
    let count = config.feed.post_count.min(posts.len());
    let feed_posts = &posts[..count];

    for format in &config.feed.format {
        match format.as_str() {
            "rss" => generate_rss(output_dir, config, feed_posts)?,
            "atom" => generate_atom(output_dir, config, feed_posts)?,
            _ => tracing::warn!("未知的 feed 格式：{}", format),
        }
    }

    Ok(())
}

fn generate_rss(output_dir: &Path, config: &SiteConfig, posts: &[Post]) -> Result<()> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n");
    xml.push_str("  <channel>\n");
    xml.push_str(&format!("    <title>{}</title>\n", xml_escape(&config.site.title)));
    xml.push_str(&format!("    <link>{}</link>\n", config.site.url));
    xml.push_str(&format!("    <description>{}</description>\n", xml_escape(&config.site.description)));
    xml.push_str(&format!(
        "    <atom:link href=\"{}/feed.xml\" rel=\"self\" type=\"application/rss+xml\" />\n",
        config.site.url
    ));

    for post in posts {
        xml.push_str("    <item>\n");
        xml.push_str(&format!("      <title>{}</title>\n", xml_escape(&post.title)));
        xml.push_str(&format!("      <link>{}/posts/{}/</link>\n", config.site.url, post.slug));
        xml.push_str(&format!(
            "      <guid isPermaLink=\"true\">{}/posts/{}/</guid>\n",
            config.site.url, post.slug
        ));
        xml.push_str(&format!(
            "      <pubDate>{}</pubDate>\n",
            post.created_at.format("%a, %d %b %Y %H:%M:%S %z")
        ));
        if let Some(excerpt) = &post.excerpt {
            xml.push_str(&format!("      <description>{}</description>\n", xml_escape(excerpt)));
        }
        xml.push_str("    </item>\n");
    }

    xml.push_str("  </channel>\n</rss>\n");
    std::fs::write(output_dir.join("feed.xml"), xml)?;
    tracing::info!("已生成 RSS feed");
    Ok(())
}

fn generate_atom(output_dir: &Path, config: &SiteConfig, posts: &[Post]) -> Result<()> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
    xml.push_str(&format!("  <title>{}</title>\n", xml_escape(&config.site.title)));
    xml.push_str(&format!("  <link href=\"{}\" />\n", config.site.url));
    xml.push_str(&format!(
        "  <link href=\"{}/atom.xml\" rel=\"self\" />\n",
        config.site.url
    ));
    xml.push_str(&format!("  <id>{}/</id>\n", config.site.url));

    if let Some(post) = posts.first() {
        xml.push_str(&format!(
            "  <updated>{}</updated>\n",
            post.updated_at.to_rfc3339()
        ));
    }

    for post in posts {
        xml.push_str("  <entry>\n");
        xml.push_str(&format!("    <title>{}</title>\n", xml_escape(&post.title)));
        xml.push_str(&format!(
            "    <link href=\"{}/posts/{}/\" />\n",
            config.site.url, post.slug
        ));
        xml.push_str(&format!(
            "    <id>{}/posts/{}/</id>\n",
            config.site.url, post.slug
        ));
        xml.push_str(&format!(
            "    <updated>{}</updated>\n",
            post.updated_at.to_rfc3339()
        ));
        if let Some(excerpt) = &post.excerpt {
            xml.push_str(&format!(
                "    <summary>{}</summary>\n",
                xml_escape(excerpt)
            ));
        }
        xml.push_str("  </entry>\n");
    }

    xml.push_str("</feed>\n");
    std::fs::write(output_dir.join("atom.xml"), xml)?;
    tracing::info!("已生成 Atom feed");
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
