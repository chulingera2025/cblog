use anyhow::Result;

/// 解析 Markdown 为 HTML
pub fn render_markdown(source: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(source, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// 从 Markdown 内容提取 TOC（目录 HTML）
pub fn extract_toc(source: &str) -> Option<String> {
    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    let options = Options::empty();
    let parser = Parser::new_ext(source, options);

    let mut headings: Vec<(u8, String)> = Vec::new();
    let mut in_heading = false;
    let mut current_level: u8 = 0;
    let mut current_text = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                current_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                current_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                if current_level >= 2 && current_level <= 4 {
                    headings.push((current_level, current_text.clone()));
                }
            }
            Event::Text(text) if in_heading => {
                current_text.push_str(&text);
            }
            Event::Code(code) if in_heading => {
                current_text.push_str(&code);
            }
            _ => {}
        }
    }

    if headings.is_empty() {
        return None;
    }

    let mut toc = String::from("<ul class=\"toc-list\">\n");
    for (level, text) in &headings {
        let id = slugify_heading(text);
        let indent = "  ".repeat((*level as usize).saturating_sub(2));
        toc.push_str(&format!(
            "{}<li><a href=\"#{}\">{}</a></li>\n",
            indent, id, text
        ));
    }
    toc.push_str("</ul>");

    Some(toc)
}

fn slugify_heading(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// 统计字数（中文按字计算，英文按空格分词）
pub fn count_words(text: &str) -> u32 {
    let mut count: u32 = 0;
    let mut in_ascii_word = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if !in_ascii_word {
                count += 1;
                in_ascii_word = true;
            }
        } else if ch > '\u{4E00}' && ch < '\u{9FFF}' {
            // CJK 统一汉字
            count += 1;
            in_ascii_word = false;
        } else {
            in_ascii_word = false;
        }
    }

    count
}

/// 计算预估阅读时间（分钟）
pub fn reading_time(word_count: u32) -> u32 {
    (word_count / 200).max(1)
}
