use axum::extract::State;
use axum::response::Html;
use minijinja::context;

use crate::admin::template::render_admin;
use crate::state::AppState;

pub async fn profile_page(State(state): State<AppState>) -> Html<String> {
    let sidebar_groups = crate::admin::layout::sidebar_groups_value("/admin/profile");
    let plugin_items =
        crate::admin::layout::plugin_sidebar_value(&state.plugin_admin_pages, "/admin/profile");

    let ctx = context! {
        page_title => "个人资料",
        site_title => &state.config.site.title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => plugin_items,
        profile_active => true,
    };

    let html = render_admin(&state.admin_env, "profile.cbtml", ctx)
        .unwrap_or_else(|e| format!("模板渲染失败: {e}"));

    Html(html)
}
