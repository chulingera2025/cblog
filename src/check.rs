use anyhow::Result;
use std::path::Path;

pub struct CheckResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// 执行项目完整性检查，依次验证配置、主题、插件和内容目录
pub fn run(project_root: &Path) -> Result<CheckResult> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    check_config(project_root, &mut errors, &mut warnings);
    check_theme(project_root, &mut errors, &mut warnings);
    check_plugins(project_root, &mut errors, &mut warnings);
    check_content(project_root, &mut errors, &mut warnings);

    Ok(CheckResult { errors, warnings })
}

fn check_config(root: &Path, errors: &mut Vec<String>, _warnings: &mut Vec<String>) {
    let config_path = root.join("cblog.toml");
    if !config_path.exists() {
        errors.push("缺少 cblog.toml 配置文件".to_string());
        return;
    }
    if let Err(e) = crate::config::SiteConfig::load(root) {
        errors.push(format!("cblog.toml 解析失败: {e}"));
    }
}

fn check_theme(root: &Path, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    // 需要先成功加载配置才能获取当前激活的主题名
    let config = match crate::config::SiteConfig::load(root) {
        Ok(cfg) => cfg,
        Err(_) => return,
    };

    let active = &config.theme.active;
    let theme_dir = root.join("themes").join(active);

    if !theme_dir.exists() {
        errors.push(format!("主题目录 themes/{active}/ 不存在"));
        return;
    }

    let theme_toml = theme_dir.join("theme.toml");
    if !theme_toml.exists() {
        errors.push(format!("主题 {active} 缺少 theme.toml"));
    }

    let templates_dir = theme_dir.join("templates");
    if !templates_dir.exists() {
        errors.push(format!("主题 {active} 缺少 templates/ 目录"));
    } else {
        // base.html 或 base.cbtml 至少存在一个
        let has_base = templates_dir.join("base.html").exists()
            || templates_dir.join("base.cbtml").exists();
        if !has_base {
            errors.push(format!(
                "主题 {active} 缺少必需模板 base.html 或 base.cbtml"
            ));
        }
    }

    let assets_dir = theme_dir.join("assets");
    if !assets_dir.exists() {
        warnings.push(format!("主题 {active} 缺少 assets/ 目录"));
    }
}

fn check_plugins(root: &Path, errors: &mut Vec<String>, _warnings: &mut Vec<String>) {
    let config = match crate::config::SiteConfig::load(root) {
        Ok(cfg) => cfg,
        Err(_) => return,
    };

    for name in &config.plugins.enabled {
        let plugin_dir = root.join("plugins").join(name);
        if !plugin_dir.exists() {
            errors.push(format!("插件目录 plugins/{name}/ 不存在"));
            continue;
        }

        let toml_path = plugin_dir.join("plugin.toml");
        if !toml_path.exists() {
            errors.push(format!("插件 {name} 缺少 plugin.toml"));
        } else if let Err(e) = crate::plugin::registry::load_plugin_info(&toml_path) {
            errors.push(format!("插件 {name} 的 plugin.toml 解析失败: {e}"));
        }

        let main_lua = plugin_dir.join("main.lua");
        if !main_lua.exists() {
            errors.push(format!("插件 {name} 缺少 main.lua"));
        }
    }
}

fn check_content(root: &Path, _errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    let posts_dir = root.join("content").join("posts");
    if !posts_dir.exists() {
        warnings.push("content/posts/ 目录不存在".to_string());
    }

    let pages_dir = root.join("content").join("pages");
    if !pages_dir.exists() {
        warnings.push("content/pages/ 目录不存在".to_string());
    }
}
