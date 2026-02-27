use anyhow::Result;
use axum::extract::{Form, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::Path;

use crate::admin::layout::{admin_page, html_escape, PageContext};
use crate::state::AppState;

#[derive(Clone, Default, serde::Serialize)]
pub struct SiteSettings {
    pub site_title: String,
    pub site_subtitle: String,
    pub site_url: String,
    pub admin_email: String,
}

impl SiteSettings {
    pub async fn load(db: &SqlitePool) -> Result<Self> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM site_settings")
                .fetch_all(db)
                .await?;

        let mut settings = Self::default();
        for (key, value) in rows {
            match key.as_str() {
                "site_title" => settings.site_title = value,
                "site_subtitle" => settings.site_subtitle = value,
                "site_url" => settings.site_url = value,
                "admin_email" => settings.admin_email = value,
                _ => {}
            }
        }
        Ok(settings)
    }

    pub async fn save(&self, db: &SqlitePool) -> Result<()> {
        let pairs = [
            ("site_title", &self.site_title),
            ("site_subtitle", &self.site_subtitle),
            ("site_url", &self.site_url),
            ("admin_email", &self.admin_email),
        ];

        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO site_settings (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(value)
            .execute(db)
            .await?;
        }
        Ok(())
    }

    /// 同步加载站点设置（用于 CLI build 等无 async runtime 的场景）
    pub fn load_sync(db_path: &Path) -> Self {
        if !db_path.exists() {
            return Self::default();
        }
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return Self::default();
        };
        rt.block_on(async {
            let db_url = format!("sqlite:{}?mode=ro", db_path.display());
            let Ok(pool) = sqlx::SqlitePool::connect(&db_url).await else {
                return Self::default();
            };
            Self::load(&pool).await.unwrap_or_default()
        })
    }
}

/// 从 AppState 获取站点标题，优先使用 DB 设置，fallback 到 config
pub async fn get_site_title(state: &AppState) -> String {
    let settings = state.site_settings.read().await;
    if settings.site_title.is_empty() {
        state.config.site.title.clone()
    } else {
        settings.site_title.clone()
    }
}

/// 从 AppState 获取站点 URL，优先使用 DB 设置，fallback 到 config
pub async fn get_site_url(state: &AppState) -> String {
    let settings = state.site_settings.read().await;
    if settings.site_url.is_empty() {
        state.config.site.url.clone()
    } else {
        settings.site_url.clone()
    }
}

// ── 设置页面 ──

pub async fn settings_page(State(state): State<AppState>) -> Html<String> {
    let settings = state.site_settings.read().await;

    let body = format!(
        r#"<div class="page-header">
    <h1 class="page-title">常规设置</h1>
</div>
<div class="card">
    <div class="card-body">
        <form method="POST" action="/admin/settings">
            <div class="form-group">
                <label class="form-label">站点标题</label>
                <input type="text" name="site_title" class="form-input" value="{site_title}" required>
            </div>
            <div class="form-group">
                <label class="form-label">副标题</label>
                <input type="text" name="site_subtitle" class="form-input" value="{site_subtitle}">
            </div>
            <div class="form-group">
                <label class="form-label">站点 URL</label>
                <input type="url" name="site_url" class="form-input" value="{site_url}" required>
                <div class="form-hint">站点的完整访问地址，用于生成绝对链接</div>
            </div>
            <div class="form-group">
                <label class="form-label">管理员邮箱</label>
                <input type="email" name="admin_email" class="form-input" value="{admin_email}">
            </div>
            <button type="submit" class="btn btn-primary">保存设置</button>
        </form>
    </div>
</div>"#,
        site_title = html_escape(&settings.site_title),
        site_subtitle = html_escape(&settings.site_subtitle),
        site_url = html_escape(&settings.site_url),
        admin_email = html_escape(&settings.admin_email),
    );

    let ctx = PageContext {
        site_title: get_site_title(&state).await,
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    Html(admin_page("常规设置", "/admin/settings", &body, &ctx))
}

#[derive(Deserialize)]
pub struct SaveSettingsForm {
    pub site_title: String,
    pub site_subtitle: String,
    pub site_url: String,
    pub admin_email: String,
}

pub async fn save_settings(
    State(state): State<AppState>,
    Form(form): Form<SaveSettingsForm>,
) -> Response {
    let settings = SiteSettings {
        site_title: form.site_title,
        site_subtitle: form.site_subtitle,
        site_url: form.site_url,
        admin_email: form.admin_email,
    };

    if let Err(e) = settings.save(&state.db).await {
        tracing::error!("保存站点设置失败：{e}");
        return Redirect::to("/admin/settings?toast_msg=保存失败&toast_type=error").into_response();
    }

    // 更新内存缓存
    *state.site_settings.write().await = settings;

    Redirect::to("/admin/settings?toast_msg=设置已保存&toast_type=success").into_response()
}
