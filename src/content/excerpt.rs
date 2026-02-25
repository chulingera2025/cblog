/// 从 HTML 内容中提取纯文本摘要
pub fn extract_excerpt(html: &str, max_chars: usize) -> String {
    let plain = strip_html_tags(html);
    let plain = plain.trim();

    let chars: Vec<char> = plain.chars().collect();
    if chars.len() <= max_chars {
        return plain.to_string();
    }

    let mut end = max_chars;
    // 尝试在句号、问号、感叹号处截断
    for i in (max_chars.saturating_sub(30)..max_chars).rev() {
        if i < chars.len() && matches!(chars[i], '。' | '？' | '！' | '.' | '?' | '!') {
            end = i + 1;
            break;
        }
    }

    let mut excerpt: String = chars[..end].iter().collect();
    if end < chars.len() {
        excerpt.push('…');
    }
    excerpt
}

/// 去除 HTML 标签，保留纯文本
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_entity = false;
    let mut entity = String::new();

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '&' if !in_tag => {
                in_entity = true;
                entity.clear();
                entity.push(ch);
            }
            ';' if in_entity => {
                in_entity = false;
                entity.push(ch);
                // 将常见 HTML 实体转为文本
                match entity.as_str() {
                    "&amp;" => result.push('&'),
                    "&lt;" => result.push('<'),
                    "&gt;" => result.push('>'),
                    "&quot;" => result.push('"'),
                    "&#39;" | "&apos;" => result.push('\''),
                    "&nbsp;" => result.push(' '),
                    _ => result.push_str(&entity),
                }
            }
            _ if in_entity => entity.push(ch),
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    // 压缩连续空白
    let mut compressed = String::with_capacity(result.len());
    let mut last_was_space = false;
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                compressed.push(' ');
                last_was_space = true;
            }
        } else {
            compressed.push(ch);
            last_was_space = false;
        }
    }

    compressed
}
