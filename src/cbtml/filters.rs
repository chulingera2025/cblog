use minijinja::{Environment, Value};
use sha2::{Digest, Sha256};

/// 向 MiniJinja 环境注册所有内置过滤器
pub fn register_filters(env: &mut Environment) {
    env.add_filter("date", filter_date);
    env.add_filter("iso", filter_iso);
    env.add_filter("slugify", filter_slugify);
    env.add_filter("truncate", filter_truncate);
    env.add_filter("wordcount", filter_wordcount);
    env.add_filter("reading_time", filter_reading_time);
    env.add_filter("reading_time_label", filter_reading_time_label);
    env.add_filter("tag_url", filter_tag_url);
    env.add_filter("category_url", filter_category_url);
    env.add_filter("abs_url", filter_abs_url);
    env.add_filter("json", filter_json);
    env.add_filter("active_class", filter_active_class);
    env.add_filter("md5", filter_md5);
    env.add_filter("upper", filter_upper);
    env.add_filter("lower", filter_lower);
    env.add_filter("capitalize", filter_capitalize);
}

fn filter_date(value: Value, format: Option<String>) -> Result<String, minijinja::Error> {
    // TODO!!! 实现日期格式化，根据 cblog.toml date_format 或自定义 format
    let s = value.to_string();
    Ok(if let Some(_fmt) = format { s } else { s })
}

fn filter_iso(value: Value) -> Result<String, minijinja::Error> {
    Ok(value.to_string())
}

pub fn filter_slugify(value: String) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn filter_truncate(value: String, length: Option<usize>) -> String {
    let len = length.unwrap_or(160);
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= len {
        value
    } else {
        let mut s: String = chars[..len].iter().collect();
        s.push('\u{2026}');
        s
    }
}

fn filter_wordcount(value: String) -> u32 {
    crate::content::markdown::count_words(&value)
}

fn filter_reading_time(value: Value) -> u32 {
    let wc = value.to_string().parse::<u32>().unwrap_or(0);
    (wc / 200).max(1)
}

fn filter_reading_time_label(minutes: u32) -> String {
    if minutes < 1 {
        "不足 1 分钟阅读".into()
    } else if minutes == 1 {
        "约 1 分钟阅读".into()
    } else {
        format!("约 {} 分钟阅读", minutes)
    }
}

fn filter_tag_url(tag: String) -> String {
    format!("/tags/{}/", filter_slugify(tag))
}

fn filter_category_url(category: String) -> String {
    format!("/category/{}/", filter_slugify(category))
}

fn filter_abs_url(path: String) -> String {
    // TODO!!! 需要 site.url 上下文拼接绝对 URL
    path
}

fn filter_json(value: Value) -> Result<String, minijinja::Error> {
    Ok(value.to_string())
}

fn filter_active_class(value: Value) -> String {
    if value.is_true() { "active".into() } else { String::new() }
}

fn filter_md5(value: String) -> String {
    let hash = Sha256::digest(value.as_bytes());
    format!("{:x}", hash)
}

fn filter_upper(value: String) -> String {
    value.to_uppercase()
}

fn filter_lower(value: String) -> String {
    value.to_lowercase()
}

fn filter_capitalize(value: String) -> String {
    let mut chars = value.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = first.to_uppercase().to_string();
            result.extend(chars);
            result
        }
    }
}
