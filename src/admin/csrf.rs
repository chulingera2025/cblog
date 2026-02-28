use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::body::Body;
use axum::http::{HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

const CSRF_COOKIE_NAME: &str = "csrf_token";
const CSRF_HEADER: &str = "X-CSRF-Token";
const TOKEN_BYTES: usize = 32;

/// 生成 32 字节随机 hex token
pub fn generate_csrf_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// CSRF 保护中间件
///
/// GET/HEAD/OPTIONS：若无 csrf cookie 则生成并设置
/// POST/PUT/DELETE：验证 cookie 中的 token 与请求中的 token 一致
///
/// token 来源优先级：
/// 1. X-CSRF-Token header（JSON API、fetch 调用）
/// 2. URL query 参数 _csrf_token（WebSocket 等场景）
/// 3. application/x-www-form-urlencoded body 中的 _csrf_token 字段
pub async fn csrf_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let cookie_token = extract_csrf_cookie(&req);

    if is_safe_method(&method) {
        let mut resp = next.run(req).await;
        if cookie_token.is_none() {
            let token = generate_csrf_token();
            set_csrf_cookie(&mut resp, &token);
        }
        return resp;
    }

    let Some(ref expected) = cookie_token else {
        return (StatusCode::FORBIDDEN, "CSRF token 缺失").into_response();
    };

    // 优先从 header 获取
    if let Some(ref token) = extract_csrf_from_header(&req) {
        if constant_time_eq(token, expected) {
            let mut resp = next.run(req).await;
            let new_token = generate_csrf_token();
            set_csrf_cookie(&mut resp, &new_token);
            return resp;
        }
        return (StatusCode::FORBIDDEN, "CSRF token 验证失败").into_response();
    }

    // 从 query 参数获取
    if let Some(ref token) = extract_csrf_from_query(&req) {
        if constant_time_eq(token, expected) {
            let mut resp = next.run(req).await;
            let new_token = generate_csrf_token();
            set_csrf_cookie(&mut resp, &new_token);
            return resp;
        }
        return (StatusCode::FORBIDDEN, "CSRF token 验证失败").into_response();
    }

    // 对 multipart 请求跳过 body 解析（文件上传场景，应通过 header 传递 token）
    let content_type = req
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.starts_with("multipart/") {
        return (StatusCode::FORBIDDEN, "CSRF token 验证失败，multipart 请求请通过 X-CSRF-Token header 传递").into_response();
    }

    // 对 application/x-www-form-urlencoded，缓冲 body 提取 token 后重建请求
    if content_type.starts_with("application/x-www-form-urlencoded") {
        let (parts, body) = req.into_parts();
        let body_bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
            Ok(b) => b,
            Err(_) => return (StatusCode::BAD_REQUEST, "请求体读取失败").into_response(),
        };

        let submitted = extract_csrf_from_form_body(&body_bytes);
        if let Some(ref token) = submitted
            && constant_time_eq(token, expected)
        {
            let req = Request::from_parts(parts, Body::from(body_bytes));
            let mut resp = next.run(req).await;
            let new_token = generate_csrf_token();
            set_csrf_cookie(&mut resp, &new_token);
            return resp;
        }
        return (StatusCode::FORBIDDEN, "CSRF token 验证失败").into_response();
    }

    // JSON 或其他 content-type 的 POST，依赖 header
    (StatusCode::FORBIDDEN, "CSRF token 验证失败").into_response()
}

fn is_safe_method(method: &Method) -> bool {
    matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
}

fn extract_csrf_cookie(req: &Request<Body>) -> Option<String> {
    let header = req.headers().get(axum::http::header::COOKIE)?;
    let header_str = header.to_str().ok()?;
    for pair in header_str.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix(CSRF_COOKIE_NAME) {
            let value = value.strip_prefix('=')?;
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

fn extract_csrf_from_header(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
}

fn extract_csrf_from_query(req: &Request<Body>) -> Option<String> {
    let query = req.uri().query()?;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("_csrf_token=")
            && !value.is_empty()
        {
            return Some(value.to_owned());
        }
    }
    None
}

/// 从 URL-encoded form body 提取 _csrf_token 字段
fn extract_csrf_from_form_body(body: &[u8]) -> Option<String> {
    let body_str = std::str::from_utf8(body).ok()?;
    for pair in body_str.split('&') {
        if let Some(value) = pair.strip_prefix("_csrf_token=")
            && !value.is_empty()
        {
            let decoded = url_decode(value);
            if !decoded.is_empty() {
                return Some(decoded);
            }
        }
    }
    None
}

/// 简易 URL 解码（仅处理 hex 编码）
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(|c| (c as char).to_digit(16));
            let lo = chars.next().and_then(|c| (c as char).to_digit(16));
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h * 16 + l) as u8 as char);
            }
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

fn set_csrf_cookie(resp: &mut Response, token: &str) {
    let cookie = format!(
        "{CSRF_COOKIE_NAME}={token}; SameSite=Strict; Path=/; Max-Age=86400"
    );
    if let Ok(val) = HeaderValue::from_str(&cookie) {
        resp.headers_mut()
            .append(axum::http::header::SET_COOKIE, val);
    }
}

/// 常量时间比较，防止计时攻击
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
