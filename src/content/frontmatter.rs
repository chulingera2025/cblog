use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct RawFrontMatter {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub date: Option<String>,
    pub updated: Option<String>,
    pub tags: Option<Vec<String>>,
    pub category: Option<String>,
    pub draft: Option<bool>,
    pub cover_image: Option<String>,
    pub excerpt: Option<String>,
    pub author: Option<String>,
    pub template: Option<String>,
    pub layout: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

pub struct ParsedContent {
    pub front_matter: RawFrontMatter,
    pub body: String,
}

/// 解析 Markdown 文件，分离 Front Matter 和正文
pub fn parse_file(path: &Path) -> Result<ParsedContent> {
    let content = std::fs::read_to_string(path)?;
    parse_content(&content)
}

/// 解析内容字符串，分离 Front Matter 和正文
pub fn parse_content(content: &str) -> Result<ParsedContent> {
    let matter = gray_matter::Matter::<gray_matter::engine::YAML>::new();
    let result = matter.parse_with_struct::<RawFrontMatter>(content);

    match result {
        Some(parsed) => Ok(ParsedContent {
            front_matter: parsed.data,
            body: parsed.content,
        }),
        None => {
            // 没有 Front Matter 或解析失败，直接当正文处理
            let parsed = matter.parse(content);
            Ok(ParsedContent {
                front_matter: RawFrontMatter::default(),
                body: parsed.content,
            })
        }
    }
}

/// 从文件名推导 slug（去除日期前缀和扩展名）
pub fn slug_from_filename(filename: &str) -> String {
    let name = filename
        .trim_end_matches(".md")
        .trim_end_matches(".markdown");
    // 去掉 YYYY-MM-DD- 日期前缀（验证数字格式）
    let slug = if name.len() > 11 {
        let bytes = name.as_bytes();
        let has_date_prefix = bytes[0..4].iter().all(|b| b.is_ascii_digit())
            && bytes[4] == b'-'
            && bytes[5..7].iter().all(|b| b.is_ascii_digit())
            && bytes[7] == b'-'
            && bytes[8..10].iter().all(|b| b.is_ascii_digit())
            && bytes[10] == b'-';
        if has_date_prefix { &name[11..] } else { name }
    } else {
        name
    };
    slug.to_string()
}

/// 解析日期字符串为 DateTime<Utc>
pub fn parse_date(date_str: &str) -> Result<DateTime<Utc>> {
    let s = date_str.trim();

    // RFC 3339: 2024-01-15T10:30:00+08:00
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.to_utc());
    }
    // ISO 8601 带时间不带时区: 2024-01-15T10:30:00
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    // 纯日期: 2024-01-15
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    // 斜线格式: 2024/01/15
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y/%m/%d") {
        let dt = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    anyhow::bail!("无法解析日期：{}", s)
}
