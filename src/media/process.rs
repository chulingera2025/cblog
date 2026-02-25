use anyhow::{Context, Result};
use image::ImageFormat;
use std::io::Cursor;

use crate::config::MediaConfig;

pub struct ProcessedImage {
    pub data: Vec<u8>,
    pub thumbnail: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
}

pub fn process_image(input: &[u8], config: &MediaConfig) -> Result<ProcessedImage> {
    let img = image::load_from_memory(input).context("无法解码图片")?;

    let width = img.width();
    let height = img.height();

    let format = image::guess_format(input).ok();
    let is_webp = matches!(format, Some(ImageFormat::WebP));

    let (data, mime_type) = if config.auto_webp && !is_webp {
        match encode_webp(&img) {
            Ok(webp_data) => (webp_data, "image/webp".to_string()),
            Err(_) => (input.to_vec(), format_to_mime(format)),
        }
    } else {
        (input.to_vec(), format_to_mime(format))
    };

    let thumbnail = if config.generate_thumb {
        let thumb = img.thumbnail(config.thumb_width, u32::MAX);
        if mime_type == "image/webp" {
            encode_webp(&thumb).ok()
        } else {
            encode_as_format(&thumb, format).ok()
        }
    } else {
        None
    };

    Ok(ProcessedImage {
        data,
        thumbnail,
        width,
        height,
        mime_type,
    })
}

fn encode_webp(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut buf);
    img.write_with_encoder(encoder)
        .context("WebP 编码失败")?;
    Ok(buf.into_inner())
}

fn encode_as_format(img: &image::DynamicImage, format: Option<ImageFormat>) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    let fmt = format.unwrap_or(ImageFormat::Png);
    img.write_to(&mut buf, fmt).context("图片编码失败")?;
    Ok(buf.into_inner())
}

fn format_to_mime(format: Option<ImageFormat>) -> String {
    match format {
        Some(ImageFormat::Jpeg) => "image/jpeg",
        Some(ImageFormat::Png) => "image/png",
        Some(ImageFormat::Gif) => "image/gif",
        Some(ImageFormat::WebP) => "image/webp",
        _ => "application/octet-stream",
    }
    .to_string()
}
