use crate::config::SiteConfig;
use regex::Regex;
use std::sync::LazyLock;

static IMG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<img([^>]*?)(/?>)").unwrap());

/// 对渲染后的 HTML 进行后处理（写入磁盘前）
pub fn apply(html: String, config: &SiteConfig) -> String {
    let mut html = html;

    if config.features.image_optimize.enabled {
        html = add_lazy_loading(html);
    }

    html
}

/// 为没有 loading 属性的 <img> 标签添加 loading="lazy"
fn add_lazy_loading(html: String) -> String {
    IMG_RE
        .replace_all(&html, |caps: &regex::Captures| {
            let attrs = &caps[1];
            let close = &caps[2];
            if attrs.contains("loading=") {
                format!("<img{attrs}{close}")
            } else {
                format!("<img loading=\"lazy\"{attrs}{close}")
            }
        })
        .into_owned()
}
