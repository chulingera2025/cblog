use crate::admin::auth::hash_password;
use crate::admin::settings::SiteSettings;
use crate::state::AppState;
use axum::extract::{Form, State};
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::sync::atomic::Ordering;

/// 检查是否已安装（users 表有记录）
pub async fn is_installed(state: &AppState) -> bool {
    state.installed.load(Ordering::Relaxed)
}

/// 安装检测中间件
/// 未安装时：非安装/健康检查路径重定向到 /install
/// 已安装时：/install 路径重定向到 /admin
pub async fn install_check_middleware(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_owned();
    let installed = is_installed(&state).await;

    if !installed {
        // 未安装，允许安装和健康检查路径通过
        if path.starts_with("/install") || path == "/health" {
            return next.run(req).await;
        }
        return Redirect::to("/install").into_response();
    }

    // 已安装，阻止再次访问安装页面
    if path.starts_with("/install") {
        return Redirect::to("/admin").into_response();
    }

    next.run(req).await
}

// ── 安装页面 ──

pub async fn install_page() -> Html<String> {
    Html(INSTALL_HTML.to_owned())
}

#[derive(Deserialize)]
pub struct InstallForm {
    pub site_title: String,
    pub site_subtitle: String,
    pub site_url: String,
    pub admin_email: String,
}

pub async fn install_submit(
    State(state): State<AppState>,
    Form(form): Form<InstallForm>,
) -> Response {
    // 双重检查：已安装则拒绝
    if is_installed(&state).await {
        return Redirect::to("/admin").into_response();
    }

    let settings = SiteSettings {
        site_title: form.site_title,
        site_subtitle: form.site_subtitle,
        site_url: form.site_url,
        admin_email: form.admin_email,
    };

    // 在事务中保存所有设置项，保证原子性
    let save_result: Result<(), sqlx::Error> = async {
        let mut tx = state.db.begin().await?;

        let pairs = [
            ("site_title", &settings.site_title),
            ("site_subtitle", &settings.site_subtitle),
            ("site_url", &settings.site_url),
            ("admin_email", &settings.admin_email),
        ];

        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO site_settings (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(value)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
    .await;

    if let Err(e) = save_result {
        tracing::error!("保存站点设置失败：{e}");
        return Redirect::to("/install?error=save_failed").into_response();
    }

    // 更新内存缓存
    *state.site_settings.write().await = settings;

    Redirect::to("/install/register").into_response()
}

// ── 注册页面 ──

pub async fn register_page(State(state): State<AppState>) -> Response {
    // 已有用户时不允许访问注册页
    if is_installed(&state).await {
        return Redirect::to("/admin").into_response();
    }
    Html(REGISTER_HTML.to_owned()).into_response()
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
}

pub async fn register_submit(
    State(state): State<AppState>,
    Form(form): Form<RegisterForm>,
) -> Response {
    if is_installed(&state).await {
        return Redirect::to("/admin").into_response();
    }

    if form.password != form.confirm_password {
        return Redirect::to("/install/register?error=password_mismatch").into_response();
    }

    if form.password.len() < 6 {
        return Redirect::to("/install/register?error=password_short").into_response();
    }

    if form.username.trim().is_empty() {
        return Redirect::to("/install/register?error=username_empty").into_response();
    }

    let password_hash = match hash_password(&form.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("密码哈希失败：{e}");
            return Redirect::to("/install/register?error=internal").into_response();
        }
    };

    let id = ulid::Ulid::new().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    match sqlx::query(
        "INSERT INTO users (id, username, password_hash, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(form.username.trim())
    .bind(&password_hash)
    .bind(&now)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            // 标记为已安装
            state.installed.store(true, Ordering::Relaxed);
            Redirect::to("/admin/login").into_response()
        }
        Err(e) => {
            tracing::error!("创建管理员账号失败：{e}");
            Redirect::to("/install/register?error=internal").into_response()
        }
    }
}

// ── 安装页 HTML ──

const INSTALL_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>安装 - cblog</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;background:#F6F9FC;display:flex;align-items:center;justify-content:center;min-height:100vh}
.install-container{width:100%;max-width:480px;padding:0 1rem}
.install-card{background:#fff;border-radius:8px;box-shadow:0 2px 4px rgba(0,0,0,.07),0 4px 12px rgba(0,0,0,.05);padding:2.5rem 2rem}
.install-brand{font-size:1.6rem;font-weight:700;color:#0A2540;text-align:center;margin-bottom:.25rem}
.install-subtitle{font-size:.95rem;color:#697386;text-align:center;margin-bottom:1.75rem}
.install-step{font-size:.8rem;color:#635BFF;font-weight:600;text-align:center;margin-bottom:.5rem;letter-spacing:.5px;text-transform:uppercase}
.error-msg{background:#FFF0F2;color:#DF1B41;font-size:.85rem;text-align:center;padding:.6rem .8rem;border-radius:6px;margin-bottom:1.25rem}
.form-group{margin-bottom:1.25rem}
.form-group label{display:block;font-size:.875rem;font-weight:500;color:#3C4257;margin-bottom:.4rem}
.form-group input,.form-group select{width:100%;padding:.65rem .75rem;border:1px solid #E0E6EB;border-radius:6px;font-size:.95rem;color:#1A1F36;outline:none;transition:border .15s,box-shadow .15s}
.form-group input:focus,.form-group select:focus{border-color:#635BFF;box-shadow:0 0 0 3px rgba(99,91,255,.12)}
.form-group .hint{font-size:.8rem;color:#697386;margin-top:.3rem}
.form-group select{background:#fff;cursor:pointer}
button[type=submit]{width:100%;padding:.7rem;background:#635BFF;color:#fff;border:none;border-radius:6px;font-size:.95rem;font-weight:600;cursor:pointer;transition:background .15s;margin-top:.25rem}
button[type=submit]:hover{background:#5851db}
</style>
</head>
<body>
<div class="install-container">
    <div class="install-card">
        <div class="install-brand">cblog</div>
        <div class="install-step">步骤 1 / 2</div>
        <p class="install-subtitle">站点基本设置</p>
        <script>
        (function(){
            var p=new URLSearchParams(location.search);
            if(p.get('error')==='save_failed')document.write('<div class="error-msg">保存设置失败，请重试</div>');
        })();
        </script>
        <form method="post" action="/install">
            <div class="form-group">
                <label for="site_title">站点标题</label>
                <input type="text" id="site_title" name="site_title" required autofocus placeholder="我的博客">
            </div>
            <div class="form-group">
                <label for="site_subtitle">副标题</label>
                <input type="text" id="site_subtitle" name="site_subtitle" placeholder="一句话描述你的站点">
            </div>
            <div class="form-group">
                <label for="site_url">站点 URL</label>
                <input type="url" id="site_url" name="site_url" required placeholder="https://example.com">
                <div class="hint">站点的完整访问地址</div>
            </div>
            <div class="form-group">
                <label for="admin_email">管理员邮箱</label>
                <input type="email" id="admin_email" name="admin_email" placeholder="admin@example.com">
            </div>
            <div class="form-group">
                <label for="db_type">数据库类型</label>
                <select id="db_type" name="db_type" disabled>
                    <option value="sqlite" selected>SQLite</option>
                    <!-- TODO!!! MySQL 支持待实现 -->
                    <option value="mysql" disabled>MySQL（即将支持）</option>
                </select>
                <div class="hint">当前仅支持 SQLite</div>
            </div>
            <button type="submit">下一步 →</button>
        </form>
    </div>
</div>
<script>
(function(){
    var match=document.cookie.match(/(?:^|;\s*)csrf_token=([^;]+)/);
    if(match){document.querySelectorAll('form[method="post"]').forEach(function(f){
        var i=document.createElement('input');i.type='hidden';i.name='_csrf_token';i.value=match[1];f.appendChild(i);
    });}
})();
</script>
</body>
</html>"#;

// ── 注册页 HTML ──

const REGISTER_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>创建管理员 - cblog</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;background:#F6F9FC;display:flex;align-items:center;justify-content:center;min-height:100vh}
.install-container{width:100%;max-width:480px;padding:0 1rem}
.install-card{background:#fff;border-radius:8px;box-shadow:0 2px 4px rgba(0,0,0,.07),0 4px 12px rgba(0,0,0,.05);padding:2.5rem 2rem}
.install-brand{font-size:1.6rem;font-weight:700;color:#0A2540;text-align:center;margin-bottom:.25rem}
.install-subtitle{font-size:.95rem;color:#697386;text-align:center;margin-bottom:1.75rem}
.install-step{font-size:.8rem;color:#635BFF;font-weight:600;text-align:center;margin-bottom:.5rem;letter-spacing:.5px;text-transform:uppercase}
.error-msg{background:#FFF0F2;color:#DF1B41;font-size:.85rem;text-align:center;padding:.6rem .8rem;border-radius:6px;margin-bottom:1.25rem}
.form-group{margin-bottom:1.25rem}
.form-group label{display:block;font-size:.875rem;font-weight:500;color:#3C4257;margin-bottom:.4rem}
.form-group input{width:100%;padding:.65rem .75rem;border:1px solid #E0E6EB;border-radius:6px;font-size:.95rem;color:#1A1F36;outline:none;transition:border .15s,box-shadow .15s}
.form-group input:focus{border-color:#635BFF;box-shadow:0 0 0 3px rgba(99,91,255,.12)}
.form-group .hint{font-size:.8rem;color:#697386;margin-top:.3rem}
button[type=submit]{width:100%;padding:.7rem;background:#635BFF;color:#fff;border:none;border-radius:6px;font-size:.95rem;font-weight:600;cursor:pointer;transition:background .15s;margin-top:.25rem}
button[type=submit]:hover{background:#5851db}
</style>
</head>
<body>
<div class="install-container">
    <div class="install-card">
        <div class="install-brand">cblog</div>
        <div class="install-step">步骤 2 / 2</div>
        <p class="install-subtitle">创建管理员账号</p>
        <script>
        (function(){
            var p=new URLSearchParams(location.search);
            var err=p.get('error');
            if(err==='password_mismatch')document.write('<div class="error-msg">两次输入的密码不一致</div>');
            else if(err==='password_short')document.write('<div class="error-msg">密码长度至少 6 个字符</div>');
            else if(err==='username_empty')document.write('<div class="error-msg">用户名不能为空</div>');
            else if(err==='internal')document.write('<div class="error-msg">系统错误，请重试</div>');
        })();
        </script>
        <form method="post" action="/install/register">
            <div class="form-group">
                <label for="username">用户名</label>
                <input type="text" id="username" name="username" required autofocus placeholder="admin">
            </div>
            <div class="form-group">
                <label for="password">密码</label>
                <input type="password" id="password" name="password" required minlength="6">
                <div class="hint">密码长度至少 6 个字符</div>
            </div>
            <div class="form-group">
                <label for="confirm_password">确认密码</label>
                <input type="password" id="confirm_password" name="confirm_password" required>
            </div>
            <button type="submit">完成安装</button>
        </form>
    </div>
</div>
<script>
(function(){
    var match=document.cookie.match(/(?:^|;\s*)csrf_token=([^;]+)/);
    if(match){document.querySelectorAll('form[method="post"]').forEach(function(f){
        var i=document.createElement('input');i.type='hidden';i.name='_csrf_token';i.value=match[1];f.appendChild(i);
    });}
})();
</script>
</body>
</html>"#;
