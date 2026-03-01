use crate::config::SiteConfig;
use regex::Regex;
use std::sync::LazyLock;

static IMG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<img([^>]*?)(/?>)").unwrap());

const SYNTAX_HIGHLIGHT_CSS: &str = r#"<style>
.code-highlight { background: #282c34; color: #abb2bf; padding: 16px; border-radius: 6px; overflow-x: auto; }
.code-highlight code { background: none; padding: 0; }
.code-highlight .source { color: #abb2bf; }
.code-highlight .comment { color: #5c6370; font-style: italic; }
.code-highlight .string { color: #98c379; }
.code-highlight .constant { color: #d19a66; }
.code-highlight .keyword { color: #c678dd; }
.code-highlight .storage { color: #c678dd; }
.code-highlight .entity { color: #61afef; }
.code-highlight .variable { color: #e06c75; }
.code-highlight .support { color: #56b6c2; }
.code-highlight .punctuation { color: #abb2bf; }
.code-highlight .meta { color: #abb2bf; }
</style>"#;

const TOC_CSS: &str = r#"<style>
html { scroll-behavior: smooth; }
.toc-list { list-style: none; padding-left: 0; }
.toc-list li { margin: 4px 0; }
.toc-list a { color: #4a6cf7; text-decoration: none; }
.toc-list a:hover { text-decoration: underline; }
</style>"#;

/// 对渲染后的 HTML 进行后处理（写入磁盘前）
pub fn apply(html: String, config: &SiteConfig) -> String {
    let mut html = html;

    if config.features.image_optimize.enabled {
        html = add_lazy_loading(html);
    }

    // CSS 注入：syntax-highlight + toc，合并为一次 </head> 替换
    let mut head_inject = String::new();
    if config.features.syntax_highlight.enabled && html.contains("code-highlight") {
        head_inject.push_str(SYNTAX_HIGHLIGHT_CSS);
    }
    if config.features.toc.enabled && html.contains("toc-list") {
        head_inject.push_str(TOC_CSS);
    }

    if !head_inject.is_empty() {
        head_inject.push_str("</head>");
        html = html.replacen("</head>", &head_inject, 1);
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
