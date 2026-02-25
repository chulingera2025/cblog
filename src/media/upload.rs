use crate::config::MediaConfig;
use anyhow::{Result, bail};

pub fn parse_max_size(size_str: &str) -> usize {
    let s = size_str.trim().to_uppercase();

    let (num_part, multiplier) = if let Some(n) = s.strip_suffix("GB") {
        (n, 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("KB") {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix('B') {
        (n, 1)
    } else {
        (s.as_str(), 1)
    };

    num_part.trim().parse::<usize>().unwrap_or(0) * multiplier
}

pub fn validate_upload(data: &[u8], mime_type: &str, config: &MediaConfig) -> Result<()> {
    let max_size = parse_max_size(&config.max_file_size);
    if data.len() > max_size {
        bail!(
            "文件大小 {} 超出限制 {}",
            format_size(data.len()),
            config.max_file_size
        );
    }

    if !config.allowed_types.iter().any(|t| t == mime_type) {
        bail!("不支持的文件类型：{}", mime_type);
    }

    Ok(())
}

/// 生成存储的相对路径和 URL
/// 返回 (相对于 upload_dir 的路径, 公开 URL)
pub fn generate_storage_path(original_name: &str) -> (String, String) {
    let now = chrono::Utc::now();
    let year = now.format("%Y");
    let month = now.format("%m");
    let id = ulid::Ulid::new();

    let ext = std::path::Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");

    // 相对于 upload_dir 的路径
    let relative = format!("{year}/{month}/{id}.{ext}");
    let url = format!("/media/{relative}");

    (relative, url)
}

pub fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}
