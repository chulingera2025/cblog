use crate::config::AuthConfig;
use crate::state::AppState;
use anyhow::{Context, Result};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::extract::{Form, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ── 数据结构 ──

#[derive(Clone)]
#[allow(dead_code)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    exp: usize,
    jti: String,
}

// ── 密码工具 ──

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("密码哈希失败: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("解析密码哈希失败: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

// ── JWT 工具 ──

fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: u64 = num_str.parse().context("无效的时间数值")?;
    let secs = match unit {
        "d" => num * 86400,
        "h" => num * 3600,
        "m" => num * 60,
        "s" => num,
        _ => anyhow::bail!("不支持的时间单位: {unit}"),
    };
    Ok(Duration::from_secs(secs))
}

/// 返回 (token, jti)
fn create_jwt(user_id: &str, username: &str, config: &AuthConfig) -> Result<(String, String)> {
    let duration = parse_duration(&config.jwt_expires_in)?;
    let exp = chrono::Utc::now().timestamp() as usize + duration.as_secs() as usize;
    let jti = ulid::Ulid::new().to_string();

    let claims = Claims {
        sub: user_id.to_owned(),
        username: username.to_owned(),
        exp,
        jti: jti.clone(),
    };

    let token = jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .context("JWT 编码失败")?;

    Ok((token, jti))
}

fn decode_jwt(token: &str, config: &AuthConfig) -> Result<Claims> {
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .context("JWT 解码失败")?;
    Ok(data.claims)
}

fn build_cookie(name: &str, value: &str, max_age_secs: i64) -> String {
    format!(
        "{name}={value}; HttpOnly; SameSite=Strict; Path=/admin; Max-Age={max_age_secs}"
    )
}

// ── 路由处理 ──

pub async fn login_page() -> Html<String> {
    Html(LOGIN_HTML.to_owned())
}

pub async fn login_submit(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    // 提取客户端 IP：优先 x-forwarded-for，回退到 x-real-ip
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_owned())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned());

    // 速率限制检查：60 秒窗口内最多 5 次尝试
    {
        let mut limiter = state.login_limiter.lock().unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        let window = Duration::from_secs(60);

        let attempts = limiter.entry(client_ip.clone()).or_default();
        attempts.retain(|t| now.duration_since(*t) < window);

        if attempts.len() >= 5 {
            return (StatusCode::TOO_MANY_REQUESTS, "登录请求过于频繁，请稍后再试").into_response();
        }

        attempts.push(now);
    }

    let result = try_login(&state, &form).await;
    match result {
        Ok((token, _jti)) => {
            let duration = parse_duration(&state.config.auth.jwt_expires_in).unwrap_or(Duration::from_secs(7 * 86400));
            let cookie = build_cookie(
                &state.config.auth.session_name,
                &token,
                duration.as_secs() as i64,
            );
            let mut resp = Redirect::to("/admin").into_response();
            resp.headers_mut()
                .insert(SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
            resp
        }
        Err(_) => Redirect::to("/admin/login?error=1").into_response(),
    }
}

async fn try_login(state: &AppState, form: &LoginForm) -> Result<(String, String)> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT id, password_hash FROM users WHERE username = ?",
    )
    .bind(&form.username)
    .fetch_optional(&state.db)
    .await
    .context("查询用户失败")?
    .context("用户不存在")?;

    let (user_id, password_hash) = row;

    if !verify_password(&form.password, &password_hash)? {
        anyhow::bail!("密码错误");
    }

    // 更新最后登录时间
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&user_id)
        .execute(&state.db)
        .await;

    create_jwt(&user_id, &form.username, &state.config.auth)
}

pub async fn logout(State(state): State<AppState>, req: Request<axum::body::Body>) -> Response {
    let cookie_name = &state.config.auth.session_name;

    if let Some(token) = extract_token_from_request(&req, cookie_name)
        && let Ok(claims) = decode_jwt(&token, &state.config.auth) {
            let _ = sqlx::query("INSERT OR IGNORE INTO revoked_tokens (jti, expires_at) VALUES (?, ?)")
                .bind(&claims.jti)
                .bind(chrono::DateTime::from_timestamp(claims.exp as i64, 0)
                    .unwrap_or_default()
                    .to_rfc3339())
                .execute(&state.db)
                .await;
        }

    let clear_cookie = build_cookie(cookie_name, "", 0);
    let mut resp = Redirect::to("/admin/login").into_response();
    resp.headers_mut()
        .insert(SET_COOKIE, HeaderValue::from_str(&clear_cookie).unwrap());
    resp
}

// ── 认证中间件 ──

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let cookie_name = &state.config.auth.session_name;
    let config = &state.config.auth;

    let token = match extract_token_from_request(&req, cookie_name) {
        Some(t) => t,
        None => return redirect_to_login(),
    };

    let claims = match decode_jwt(&token, config) {
        Ok(c) => c,
        Err(_) => return redirect_to_login(),
    };

    // 检查是否已被撤销
    let revoked: bool = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = ?)",
    )
    .bind(&claims.jti)
    .fetch_one(&state.db)
    .await
    .unwrap_or(true);

    if revoked {
        return redirect_to_login();
    }

    req.extensions_mut().insert(AuthUser {
        id: claims.sub.clone(),
        username: claims.username.clone(),
    });

    let mut resp = next.run(req).await;

    // 自动续期：剩余时间不足总有效期的 1/3 时签发新 token
    if let Ok(total_duration) = parse_duration(&config.jwt_expires_in) {
        let now = chrono::Utc::now().timestamp() as usize;
        let remaining = claims.exp.saturating_sub(now);
        let threshold = total_duration.as_secs() as usize / 3;

        if remaining < threshold
            && let Ok((new_token, _)) = create_jwt(&claims.sub, &claims.username, config) {
                let cookie = build_cookie(
                    cookie_name,
                    &new_token,
                    total_duration.as_secs() as i64,
                );
                if let Ok(val) = HeaderValue::from_str(&cookie) {
                    resp.headers_mut().insert(SET_COOKIE, val);
                }
            }
    }

    resp
}

// ── 修改密码 ──

pub async fn change_password(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Form(form): Form<ChangePasswordForm>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT password_hash FROM users WHERE id = ?",
    )
    .bind(&user.id)
    .fetch_optional(&state.db)
    .await;

    let password_hash = match row {
        Ok(Some((hash,))) => hash,
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, "用户查询失败").into_response(),
    };

    match verify_password(&form.old_password, &password_hash) {
        Ok(true) => {}
        Ok(false) => return (StatusCode::BAD_REQUEST, "原密码错误").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "密码验证失败").into_response(),
    }

    let new_hash = match hash_password(&form.new_password) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "密码哈希失败").into_response(),
    };

    match sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(&user.id)
        .execute(&state.db)
        .await
    {
        Ok(_) => Redirect::to("/admin/profile?toast_msg=密码已更新&toast_type=success").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "更新密码失败").into_response(),
    }
}

// ── 辅助函数 ──

fn extract_token_from_request<B>(req: &Request<B>, cookie_name: &str) -> Option<String> {
    let header = req.headers().get(axum::http::header::COOKIE)?;
    let header_str = header.to_str().ok()?;
    for pair in header_str.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix(cookie_name) {
            let value = value.strip_prefix('=')?;
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

fn redirect_to_login() -> Response {
    Redirect::to("/admin/login").into_response()
}

// ── 登录页 HTML ──

const LOGIN_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>登录 - cblog</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;background:#F6F9FC;display:flex;align-items:center;justify-content:center;min-height:100vh}
.login-container{width:100%;max-width:400px;padding:0 1rem}
.login-card{background:#fff;border-radius:8px;box-shadow:0 2px 4px rgba(0,0,0,.07),0 4px 12px rgba(0,0,0,.05);padding:2.5rem 2rem}
.login-brand{font-size:1.6rem;font-weight:700;color:#0A2540;text-align:center;margin-bottom:.25rem}
.login-subtitle{font-size:.95rem;color:#697386;text-align:center;margin-bottom:1.75rem}
.error-msg{background:#FFF0F2;color:#DF1B41;font-size:.85rem;text-align:center;padding:.6rem .8rem;border-radius:6px;margin-bottom:1.25rem}
.form-group{margin-bottom:1.25rem}
.form-group label{display:block;font-size:.875rem;font-weight:500;color:#3C4257;margin-bottom:.4rem}
.form-group input{width:100%;padding:.65rem .75rem;border:1px solid #E0E6EB;border-radius:6px;font-size:.95rem;color:#1A1F36;outline:none;transition:border .15s,box-shadow .15s}
.form-group input:focus{border-color:#635BFF;box-shadow:0 0 0 3px rgba(99,91,255,.12)}
button[type=submit]{width:100%;padding:.7rem;background:#635BFF;color:#fff;border:none;border-radius:6px;font-size:.95rem;font-weight:600;cursor:pointer;transition:background .15s;margin-top:.25rem}
button[type=submit]:hover{background:#5851db}
</style>
</head>
<body>
<div class="login-container">
    <div class="login-card">
        <div class="login-brand">cblog</div>
        <p class="login-subtitle">登录管理后台</p>
        <script>if(location.search.includes('error=1'))document.write('<div class="error-msg">用户名或密码错误</div>')</script>
        <form method="post" action="/admin/login">
            <div class="form-group">
                <label for="username">用户名</label>
                <input type="text" id="username" name="username" required autofocus>
            </div>
            <div class="form-group">
                <label for="password">密码</label>
                <input type="password" id="password" name="password" required>
            </div>
            <button type="submit">登录</button>
        </form>
    </div>
</div>
</body>
</html>"#;
