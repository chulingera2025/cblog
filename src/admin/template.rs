use crate::admin::layout::PluginSidebarEntry;
use crate::cbtml;
use anyhow::{Context, Result};
use minijinja::{Environment, Value, context};
use std::collections::HashMap;
use std::path::Path;

/// 构建后台专用 MiniJinja 渲染环境
///
/// 从 admin/templates/ 读取 cbtml 模板并编译，注册后台所需的过滤器和全局函数
pub fn build_admin_env(project_root: &Path, site_url: &str) -> Result<Environment<'static>> {
    let mut env = Environment::new();

    // 复用前台已有的过滤器
    cbtml::filters::register_filters(&mut env, site_url);

    // 后台专用过滤器
    env.add_filter("format_datetime", filter_format_datetime);

    // 全局函数：svg_icon，在模板中以 {{ svg_icon("posts") }} 调用
    env.add_function("svg_icon", fn_svg_icon);

    // 从 admin/templates/ 加载并编译 cbtml 模板
    let templates_dir = project_root.join("admin/templates");
    if templates_dir.exists() {
        let compiled = compile_admin_templates(&templates_dir)?;
        for (name, source) in compiled {
            env.add_template_owned(name.clone(), source)
                .with_context(|| format!("注册后台模板 {} 失败", name))?;
        }
    }

    Ok(env)
}

/// 渲染后台模板
pub fn render_admin(env: &Environment, name: &str, ctx: Value) -> Result<String> {
    let tmpl = env
        .get_template(name)
        .with_context(|| format!("后台模板 {} 不存在", name))?;
    let html = tmpl
        .render(ctx)
        .with_context(|| format!("渲染后台模板 {} 失败", name))?;
    Ok(html)
}

/// 侧边栏定义
struct SidebarItemDef {
    label: &'static str,
    href: &'static str,
    icon: &'static str,
}

struct SidebarGroupDef {
    label: &'static str,
    items: &'static [SidebarItemDef],
}

const SIDEBAR_GROUPS: &[SidebarGroupDef] = &[
    SidebarGroupDef {
        label: "",
        items: &[SidebarItemDef {
            label: "仪表盘",
            href: "/admin",
            icon: "grid",
        }],
    },
    SidebarGroupDef {
        label: "内容",
        items: &[
            SidebarItemDef {
                label: "文章管理",
                href: "/admin/posts",
                icon: "file-text",
            },
            SidebarItemDef {
                label: "页面管理",
                href: "/admin/pages",
                icon: "file",
            },
            SidebarItemDef {
                label: "媒体库",
                href: "/admin/media",
                icon: "image",
            },
        ],
    },
    SidebarGroupDef {
        label: "系统",
        items: &[
            SidebarItemDef {
                label: "构建管理",
                href: "/admin/build",
                icon: "package",
            },
            SidebarItemDef {
                label: "主题设置",
                href: "/admin/theme",
                icon: "palette",
            },
            SidebarItemDef {
                label: "插件管理",
                href: "/admin/plugins",
                icon: "plug",
            },
        ],
    },
];

fn is_active(current_path: &str, item_path: &str) -> bool {
    if item_path == "/admin" {
        current_path == "/admin"
    } else {
        current_path.starts_with(item_path)
    }
}

/// 构建后台模板渲染所需的基础 context（sidebar、page_title 等）
///
/// 各页面在此基础上通过 context! 扩展页面特有的变量
pub fn build_admin_context(
    page_title: &str,
    active_path: &str,
    site_title: &str,
    plugin_sidebar_items: &[PluginSidebarEntry],
) -> Value {
    let sidebar_groups: Vec<Value> = SIDEBAR_GROUPS
        .iter()
        .map(|group| {
            let items: Vec<Value> = group
                .items
                .iter()
                .map(|item| {
                    context! {
                        label => item.label,
                        href => item.href,
                        icon => item.icon,
                        active => is_active(active_path, item.href),
                    }
                })
                .collect();
            context! {
                label => if group.label.is_empty() { None } else { Some(group.label) },
                items => items,
            }
        })
        .collect();

    let plugin_items: Vec<Value> = plugin_sidebar_items
        .iter()
        .map(|entry| {
            context! {
                label => &entry.label,
                href => &entry.href,
                icon => &entry.icon,
                active => is_active(active_path, &entry.href),
            }
        })
        .collect();

    context! {
        page_title => page_title,
        site_title => site_title,
        sidebar_groups => sidebar_groups,
        plugin_sidebar_items => if plugin_items.is_empty() { None } else { Some(plugin_items) },
        profile_active => is_active(active_path, "/admin/profile"),
    }
}

/// 递归编译 admin/templates/ 下的所有 .cbtml 文件
fn compile_admin_templates(templates_dir: &Path) -> Result<HashMap<String, String>> {
    let mut templates = HashMap::new();
    collect_and_compile(templates_dir, templates_dir, &mut templates)?;
    Ok(templates)
}

fn collect_and_compile(
    base_dir: &Path,
    current_dir: &Path,
    templates: &mut HashMap<String, String>,
) -> Result<()> {
    for entry in std::fs::read_dir(current_dir)
        .with_context(|| format!("读取目录 {} 失败", current_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_and_compile(base_dir, &path, templates)?;
        } else if path.extension().is_some_and(|ext| ext == "cbtml") {
            let rel_path = path.strip_prefix(base_dir)?;
            let template_name = rel_path.to_string_lossy().to_string();

            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("读取模板文件 {} 失败", path.display()))?;
            let compiled = cbtml::compile(&source, &template_name)
                .with_context(|| format!("编译后台模板 {} 失败", template_name))?;

            templates.insert(template_name, compiled);
        }
    }
    Ok(())
}

/// RFC3339 时间格式化为 "YYYY-MM-DD HH:MM:SS"
fn filter_format_datetime(value: String) -> String {
    let s = &value[..19.min(value.len())];
    s.replace('T', " ")
}

/// svg_icon 全局函数，返回对应图标的 SVG HTML
fn fn_svg_icon(name: String) -> String {
    crate::admin::layout::svg_icon(&name).to_string()
}
