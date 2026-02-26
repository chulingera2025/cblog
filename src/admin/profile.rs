use axum::extract::State;
use axum::response::Html;

use crate::admin::layout::{admin_page, svg_icon, PageContext};
use crate::state::AppState;

pub async fn profile_page(State(state): State<AppState>) -> Html<String> {
    let ctx = PageContext {
        site_title: crate::admin::settings::get_site_title(&state).await,
        plugin_sidebar_items: state.plugin_admin_pages.clone(),
    };

    let body = format!(
        r#"<div class="page-header">
    <h1 class="page-title">个人资料</h1>
</div>
<div class="card">
    <div class="card-header">
        <span class="card-title">修改密码</span>
    </div>
    <div class="card-body">
        <form method="POST" action="/admin/profile/password" id="password-form">
            <div class="form-group">
                <label class="form-label">当前密码</label>
                <input type="password" name="old_password" class="form-input" required autocomplete="current-password">
            </div>
            <div class="form-group">
                <label class="form-label">新密码</label>
                <input type="password" name="new_password" class="form-input" required autocomplete="new-password" minlength="6">
                <div class="form-hint">密码长度至少 6 个字符</div>
            </div>
            <div class="form-group">
                <label class="form-label">确认新密码</label>
                <input type="password" name="confirm_password" class="form-input" required autocomplete="new-password">
            </div>
            <button type="submit" class="btn btn-primary">{icon} 更新密码</button>
        </form>
    </div>
</div>"#,
        icon = svg_icon("key"),
    );

    Html(admin_page("个人资料", "/admin/profile", &body, &ctx))
}
