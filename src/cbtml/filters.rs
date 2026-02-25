use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use md5::{Digest as Md5Digest, Md5};
use minijinja::{Environment, Value};

/// 向 MiniJinja 环境注册所有内置过滤器
pub fn register_filters(env: &mut Environment, site_url: &str) {
    env.add_filter("date", filter_date);
    env.add_filter("iso", filter_iso);
    env.add_filter("slugify", filter_slugify);
    env.add_filter("truncate", filter_truncate);
    env.add_filter("wordcount", filter_wordcount);
    env.add_filter("reading_time", filter_reading_time);
    env.add_filter("reading_time_label", filter_reading_time_label);
    env.add_filter("tag_url", filter_tag_url);
    env.add_filter("category_url", filter_category_url);
    env.add_filter("json", filter_json);
    env.add_filter("active_class", filter_active_class);
    env.add_filter("md5", filter_md5);
    env.add_filter("upper", filter_upper);
    env.add_filter("lower", filter_lower);
    env.add_filter("capitalize", filter_capitalize);

    let url = site_url.trim_end_matches('/').to_owned();
    env.add_filter("abs_url", move |path: String| -> String {
        let path = path.trim_start_matches('/');
        format!("{}/{}", url, path)
    });
}

/// 尝试从多种常见格式中解析日期字符串
fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    // RFC 3339 / ISO 8601 带时区
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.to_utc());
    }
    // ISO 8601 无时区（带时间）
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc());
    }
    // 仅日期
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc());
    }
    None
}

fn filter_date(value: Value, format: Option<String>) -> Result<String, minijinja::Error> {
    let s = value.to_string();
    let dt = parse_datetime(&s).ok_or_else(|| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("无法解析日期: {}", s),
        )
    })?;
    let fmt = format.as_deref().unwrap_or("%Y年%m月%d日");
    Ok(dt.format(fmt).to_string())
}

fn filter_iso(value: Value) -> Result<String, minijinja::Error> {
    let s = value.to_string();
    let dt = parse_datetime(&s).ok_or_else(|| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("无法解析日期: {}", s),
        )
    })?;
    Ok(dt.to_rfc3339())
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

fn filter_json(value: Value) -> Result<String, minijinja::Error> {
    let serialized = serde_json::to_string(&value).map_err(|e| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("JSON 序列化失败: {}", e),
        )
    })?;
    Ok(serialized)
}

fn filter_active_class(value: Value) -> String {
    if value.is_true() { "active".into() } else { String::new() }
}

fn filter_md5(value: String) -> String {
    let hash = Md5::digest(value.as_bytes());
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
