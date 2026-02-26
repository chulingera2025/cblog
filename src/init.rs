use anyhow::Result;
use std::fs;
use std::path::Path;

// 嵌入默认 cblog.toml
const DEFAULT_CONFIG: &str = r#"[site]
title = "My Blog"
subtitle = ""
description = ""
url = "https://example.com"
language = "zh-CN"
timezone = "Asia/Shanghai"

[site.author]
name = ""
email = ""

[build]
output_dir = "public"

[theme]
active = "aurora"
"#;

// 嵌入 aurora 主题所有文件
const THEME_TOML: &str = include_str!("../themes/aurora/theme.toml");

const TPL_BASE: &str = include_str!("../themes/aurora/templates/base.cbtml");
const TPL_INDEX: &str = include_str!("../themes/aurora/templates/index.cbtml");
const TPL_POST: &str = include_str!("../themes/aurora/templates/post.cbtml");
const TPL_PAGE: &str = include_str!("../themes/aurora/templates/page.cbtml");
const TPL_404: &str = include_str!("../themes/aurora/templates/404.cbtml");
const TPL_ARCHIVE: &str = include_str!("../themes/aurora/templates/archive.cbtml");
const TPL_CATEGORY: &str = include_str!("../themes/aurora/templates/category.cbtml");
const TPL_TAG: &str = include_str!("../themes/aurora/templates/tag.cbtml");

const TPL_NAV: &str = include_str!("../themes/aurora/templates/partials/nav.cbtml");
const TPL_FOOTER: &str = include_str!("../themes/aurora/templates/partials/footer.cbtml");
const TPL_POST_CARD: &str = include_str!("../themes/aurora/templates/partials/post-card.cbtml");
const TPL_PAGINATION: &str = include_str!("../themes/aurora/templates/partials/pagination.cbtml");

const SCSS_VARIABLES: &str = include_str!("../themes/aurora/assets/scss/_variables.scss");
const SCSS_MAIN: &str = include_str!("../themes/aurora/assets/scss/main.scss");

const JS_MAIN: &str = include_str!("../themes/aurora/assets/js/main.js");

/// 检测项目是否已初始化，未初始化则自动创建骨架。
/// 返回 `true` 表示执行了初始化，`false` 表示已存在。
pub fn ensure_initialized(root: &Path) -> Result<bool> {
    if root.join("cblog.toml").exists() {
        return Ok(false);
    }

    // 创建目录结构
    let dirs = [
        "content/posts",
        "content/pages",
        "themes/aurora/templates/partials",
        "themes/aurora/assets/scss",
        "themes/aurora/assets/js",
        "plugins",
        "media",
    ];
    for dir in &dirs {
        fs::create_dir_all(root.join(dir))?;
    }

    // 写入 cblog.toml
    fs::write(root.join("cblog.toml"), DEFAULT_CONFIG)?;

    // 写入 aurora 主题
    let theme_files: &[(&str, &str)] = &[
        ("themes/aurora/theme.toml", THEME_TOML),
        ("themes/aurora/templates/base.cbtml", TPL_BASE),
        ("themes/aurora/templates/index.cbtml", TPL_INDEX),
        ("themes/aurora/templates/post.cbtml", TPL_POST),
        ("themes/aurora/templates/page.cbtml", TPL_PAGE),
        ("themes/aurora/templates/404.cbtml", TPL_404),
        ("themes/aurora/templates/archive.cbtml", TPL_ARCHIVE),
        ("themes/aurora/templates/category.cbtml", TPL_CATEGORY),
        ("themes/aurora/templates/tag.cbtml", TPL_TAG),
        ("themes/aurora/templates/partials/nav.cbtml", TPL_NAV),
        ("themes/aurora/templates/partials/footer.cbtml", TPL_FOOTER),
        (
            "themes/aurora/templates/partials/post-card.cbtml",
            TPL_POST_CARD,
        ),
        (
            "themes/aurora/templates/partials/pagination.cbtml",
            TPL_PAGINATION,
        ),
        ("themes/aurora/assets/scss/_variables.scss", SCSS_VARIABLES),
        ("themes/aurora/assets/scss/main.scss", SCSS_MAIN),
        ("themes/aurora/assets/js/main.js", JS_MAIN),
    ];

    for (path, content) in theme_files {
        fs::write(root.join(path), content)?;
    }

    Ok(true)
}
