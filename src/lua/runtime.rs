use anyhow::{Context, Result};
use mlua::{Lua, LuaOptions, LuaSerdeExt, StdLib};
use std::path::Path;
use std::path::PathBuf;

use crate::config::SiteConfig;
use crate::lua::hooks::HookRegistry;
use crate::lua::sandbox;
use crate::plugin::registry::PluginInfo;

/// 插件引擎：持有 Lua VM、Hook 注册表和已加载的插件信息
pub struct PluginEngine {
    pub lua: Lua,
    pub hooks: HookRegistry,
    pub project_root: PathBuf,
    pub plugins: Vec<PluginInfo>,
    pub plugin_configs: std::collections::HashMap<String, std::collections::HashMap<String, serde_json::Value>>,
}

impl PluginEngine {
    pub fn new(
        project_root: &Path,
        config: &SiteConfig,
        plugin_configs: std::collections::HashMap<String, std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<Self> {
        let lua = Lua::new_with(StdLib::ALL, LuaOptions::default())
            .map_err(|e| anyhow::anyhow!("Lua VM 初始化失败: {e}"))?;

        sandbox::apply(&lua, project_root)?;

        let hooks = HookRegistry::new();

        let engine = Self {
            lua,
            hooks,
            project_root: project_root.to_path_buf(),
            plugins: Vec::new(),
            plugin_configs,
        };

        engine.register_core_api(config)?;

        Ok(engine)
    }

    fn register_core_api(&self, config: &SiteConfig) -> Result<()> {
        let lua = &self.lua;
        let globals = lua.globals();

        let cblog = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("创建 cblog table 失败: {e}"))?;

        // cblog.version()
        let version = concat!(env!("CARGO_PKG_VERSION"), "-", env!("CBLOG_GIT_COMMIT")).to_string();
        cblog
            .set(
                "version",
                lua.create_function(move |_, ()| Ok(version.clone()))
                    .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.slugify(text)
        cblog
            .set(
                "slugify",
                lua.create_function(|_, text: String| {
                    let slug: String = text
                        .to_lowercase()
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '-' })
                        .collect::<String>()
                        .split('-')
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("-");
                    Ok(slug)
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.json(table) -> string
        cblog
            .set(
                "json",
                lua.create_function(|lua, val: mlua::Value| {
                    let json_val: serde_json::Value = lua.from_value(val)?;
                    serde_json::to_string(&json_val)
                        .map_err(|e| mlua::Error::external(format!("JSON 序列化失败: {e}")))
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.iso_date(date_str)
        cblog
            .set(
                "iso_date",
                lua.create_function(|_, date_str: String| Ok(date_str))
                    .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.site()
        let site_title = config.site.title.clone();
        let site_url = config.site.url.clone();
        let site_lang = config.site.language.clone();
        let site_desc = config.site.description.clone();
        let author_name = config.site.author.name.clone();
        let author_email = config.site.author.email.clone();
        cblog
            .set(
                "site",
                lua.create_function(move |lua, ()| {
                    let site = lua.create_table()?;
                    site.set("title", site_title.clone())?;
                    site.set("url", site_url.clone())?;
                    site.set("language", site_lang.clone())?;
                    site.set("description", site_desc.clone())?;
                    let author = lua.create_table()?;
                    author.set("name", author_name.clone())?;
                    author.set("email", author_email.clone())?;
                    site.set("author", author)?;
                    Ok(site)
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.strip_html(html) — 去除 HTML 标签
        cblog
            .set(
                "strip_html",
                lua.create_function(|_, html: String| {
                    let mut result = String::with_capacity(html.len());
                    let mut inside_tag = false;
                    for ch in html.chars() {
                        match ch {
                            '<' => inside_tag = true,
                            '>' => inside_tag = false,
                            _ if !inside_tag => result.push(ch),
                            _ => {}
                        }
                    }
                    Ok(result)
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.highlight(code, language) — syntect 代码高亮
        cblog
            .set(
                "highlight",
                lua.create_function(|_, (code, lang): (String, String)| {
                    use syntect::html::{ClassStyle, ClassedHTMLGenerator};
                    use syntect::parsing::SyntaxSet;
                    use syntect::util::LinesWithEndings;

                    let ss = SyntaxSet::load_defaults_newlines();
                    let syntax = match ss.find_syntax_by_token(&lang) {
                        Some(s) => s,
                        None => return Ok(code),
                    };
                    let mut generator = ClassedHTMLGenerator::new_with_class_style(
                        syntax,
                        &ss,
                        ClassStyle::Spaced,
                    );
                    for line in LinesWithEndings::from(&code) {
                        if generator
                            .parse_html_for_line_which_includes_newline(line)
                            .is_err()
                        {
                            return Ok(code);
                        }
                    }
                    Ok(generator.finalize())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.version_lt(v1, v2) — 语义版本比较 v1 < v2
        cblog
            .set(
                "version_lt",
                lua.create_function(|_, (v1, v2): (String, String)| {
                    let parse = |v: &str| -> Vec<u64> {
                        v.split('.')
                            .map(|s| s.parse::<u64>().unwrap_or(0))
                            .collect()
                    };
                    let a = parse(&v1);
                    let b = parse(&v2);
                    let len = a.len().max(b.len());
                    for i in 0..len {
                        let sa = a.get(i).copied().unwrap_or(0);
                        let sb = b.get(i).copied().unwrap_or(0);
                        if sa < sb {
                            return Ok(true);
                        }
                        if sa > sb {
                            return Ok(false);
                        }
                    }
                    Ok(false)
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.log.*
        let log = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        log.set(
            "info",
            lua.create_function(|_, msg: String| {
                tracing::info!("[plugin] {}", msg);
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        log.set(
            "warn",
            lua.create_function(|_, msg: String| {
                tracing::warn!("[plugin] {}", msg);
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        log.set(
            "error",
            lua.create_function(|_, msg: String| {
                tracing::error!("[plugin] {}", msg);
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        log.set(
            "debug",
            lua.create_function(|_, msg: String| {
                tracing::debug!("[plugin] {}", msg);
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        cblog
            .set("log", log)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.files.*
        let root = self.project_root.clone();
        let files = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "read",
                lua.create_function(move |_, path: String| {
                    let full = sandbox::resolve_path(&root_c, &path)?;
                    std::fs::read_to_string(&full)
                        .map_err(|e| mlua::Error::external(format!("读取文件失败: {e}")))
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "write",
                lua.create_function(move |_, (path, content): (String, String)| {
                    let full = sandbox::resolve_path(&root_c, &path)?;
                    if let Some(parent) = full.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| mlua::Error::external(format!("创建目录失败: {e}")))?;
                    }
                    std::fs::write(&full, content)
                        .map_err(|e| mlua::Error::external(format!("写入文件失败: {e}")))
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "exists",
                lua.create_function(move |_, path: String| {
                    let full = sandbox::resolve_path(&root_c, &path)?;
                    Ok(full.exists())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "remove",
                lua.create_function(move |_, path: String| {
                    let full = sandbox::resolve_path(&root_c, &path)?;
                    if full.exists() {
                        std::fs::remove_file(&full)
                            .map_err(|e| mlua::Error::external(format!("删除文件失败: {e}")))?;
                    }
                    Ok(())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "mkdir",
                lua.create_function(move |_, path: String| {
                    let full = sandbox::resolve_path(&root_c, &path)?;
                    std::fs::create_dir_all(&full)
                        .map_err(|e| mlua::Error::external(format!("创建目录失败: {e}")))
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "list",
                lua.create_function(move |lua, path: String| {
                    let full = sandbox::resolve_path(&root_c, &path)?;
                    let entries = std::fs::read_dir(&full)
                        .map_err(|e| mlua::Error::external(format!("读取目录失败: {e}")))?;
                    let tbl = lua.create_table()?;
                    let mut idx = 1;
                    for entry in entries {
                        let entry = entry.map_err(|e| {
                            mlua::Error::external(format!("遍历目录条目失败: {e}"))
                        })?;
                        if let Some(name) = entry.file_name().to_str() {
                            tbl.set(idx, name.to_string())?;
                            idx += 1;
                        }
                    }
                    Ok(tbl)
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "copy",
                lua.create_function(move |_, (src, dst): (String, String)| {
                    let src_full = sandbox::resolve_path(&root_c, &src)?;
                    let dst_full = sandbox::resolve_path(&root_c, &dst)?;
                    if let Some(parent) = dst_full.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| mlua::Error::external(format!("创建目录失败: {e}")))?;
                    }
                    std::fs::copy(&src_full, &dst_full)
                        .map_err(|e| mlua::Error::external(format!("复制文件失败: {e}")))?;
                    Ok(())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_c = root.clone();
        files
            .set(
                "append",
                lua.create_function(move |_, (path, content): (String, String)| {
                    let full = sandbox::resolve_path(&root_c, &path)?;
                    use std::io::Write;
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&full)
                        .map_err(|e| mlua::Error::external(format!("打开文件失败: {e}")))?;
                    file.write_all(content.as_bytes())
                        .map_err(|e| mlua::Error::external(format!("追加写入失败: {e}")))?;
                    Ok(())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        cblog
            .set("files", files)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        globals
            .set("cblog", cblog)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(())
    }

    /// 加载所有已激活的插件
    pub fn load_plugins(&mut self, enabled_plugins: &[String]) -> Result<()> {
        let plugins_dir = self.project_root.join("plugins");
        if !plugins_dir.exists() {
            return Ok(());
        }

        for name in enabled_plugins {
            let plugin_dir = plugins_dir.join(name);
            let toml_path = plugin_dir.join("plugin.toml");
            if !toml_path.exists() {
                tracing::warn!("插件 {} 的 plugin.toml 不存在，跳过", name);
                continue;
            }

            let info = crate::plugin::registry::load_plugin_info(&toml_path)
                .with_context(|| format!("加载插件 {} 元数据失败", name))?;

            // 设置 require 搜索路径
            let lib_dir = plugin_dir.join("lib");
            if lib_dir.exists() {
                let path_str = format!("{}/?.lua", lib_dir.display());
                let package: mlua::Table = self
                    .lua
                    .globals()
                    .get("package")
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                let current_path: String =
                    package.get("path").map_err(|e| anyhow::anyhow!("{e}"))?;
                package
                    .set("path", format!("{};{}", path_str, current_path))
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            }

            // 创建 _pending_hooks 收集表
            self.setup_plugin_api(name)?;

            // 执行 main.lua
            let main_lua = plugin_dir.join("main.lua");
            if main_lua.exists() {
                let code = std::fs::read_to_string(&main_lua)
                    .with_context(|| format!("读取 {} main.lua 失败", name))?;
                self.lua
                    .load(&code)
                    .set_name(format!("plugins/{}/main.lua", name))
                    .exec()
                    .map_err(|e| {
                        anyhow::anyhow!("执行插件 {} 的 main.lua 失败: {}", name, e)
                    })?;
            }

            // 收集 pending hooks 注册到 HookRegistry
            self.collect_pending_hooks()?;

            tracing::info!("已加载插件: {} v{}", info.name, info.version);
            self.plugins.push(info);
        }

        Ok(())
    }

    /// 为插件创建 plugin.filter/action API，将注册暂存到 Lua table
    fn setup_plugin_api(&self, plugin_name: &str) -> Result<()> {
        let lua = &self.lua;
        let globals = lua.globals();

        // 创建收集表
        let pending = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let filters = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let actions = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        pending
            .set("filters", filters)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        pending
            .set("actions", actions)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        globals
            .set("_pending_hooks", pending)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let plugin_table = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // plugin.filter(hook_name, priority, handler)
        plugin_table
            .set(
                "filter",
                lua.create_function(|lua, (hook, priority, func): (String, i32, mlua::Function)| {
                    let pending: mlua::Table = lua.globals().get("_pending_hooks")?;
                    let filters: mlua::Table = pending.get("filters")?;
                    let entry = lua.create_table()?;
                    entry.set("hook", hook)?;
                    entry.set("priority", priority)?;
                    entry.set("func", func)?;
                    filters.push(entry)?;
                    Ok(())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // plugin.action(hook_name, priority, handler)
        plugin_table
            .set(
                "action",
                lua.create_function(|lua, (hook, priority, func): (String, i32, mlua::Function)| {
                    let pending: mlua::Table = lua.globals().get("_pending_hooks")?;
                    let actions: mlua::Table = pending.get("actions")?;
                    let entry = lua.create_table()?;
                    entry.set("hook", hook)?;
                    entry.set("priority", priority)?;
                    entry.set("func", func)?;
                    actions.push(entry)?;
                    Ok(())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // plugin.config() — 返回预加载的插件配置
        let config_value = if let Some(cfg) = self.plugin_configs.get(plugin_name) {
            lua.to_value(cfg).unwrap_or(mlua::Value::Nil)
        } else {
            mlua::Value::Nil
        };
        let config_key = lua.create_registry_value(config_value)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        plugin_table
            .set(
                "config",
                lua.create_function(move |lua, ()| {
                    let val: mlua::Value = lua.registry_value(&config_key)?;
                    match val {
                        mlua::Value::Nil => lua.create_table().map(mlua::Value::Table),
                        other => Ok(other),
                    }
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        globals
            .set("plugin", plugin_table)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(())
    }

    /// 从 Lua _pending_hooks 表收集 hook 注册到 Rust HookRegistry
    fn collect_pending_hooks(&mut self) -> Result<()> {
        let lua = &self.lua;
        let globals = lua.globals();

        let pending: mlua::Table = globals
            .get("_pending_hooks")
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // 收集 filters
        let filters: mlua::Table = pending
            .get("filters")
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        for pair in filters.sequence_values::<mlua::Table>() {
            let entry = pair.map_err(|e| anyhow::anyhow!("{e}"))?;
            let hook: String = entry.get("hook").map_err(|e| anyhow::anyhow!("{e}"))?;
            let priority: i32 = entry.get("priority").map_err(|e| anyhow::anyhow!("{e}"))?;
            let func: mlua::Function = entry.get("func").map_err(|e| anyhow::anyhow!("{e}"))?;
            let key = lua
                .create_registry_value(func)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            self.hooks.add_filter(&hook, priority, key);
        }

        // 收集 actions
        let actions: mlua::Table = pending
            .get("actions")
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        for pair in actions.sequence_values::<mlua::Table>() {
            let entry = pair.map_err(|e| anyhow::anyhow!("{e}"))?;
            let hook: String = entry.get("hook").map_err(|e| anyhow::anyhow!("{e}"))?;
            let priority: i32 = entry.get("priority").map_err(|e| anyhow::anyhow!("{e}"))?;
            let func: mlua::Function = entry.get("func").map_err(|e| anyhow::anyhow!("{e}"))?;
            let key = lua
                .create_registry_value(func)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            self.hooks.add_action(&hook, priority, key);
        }

        // 清空 pending
        globals
            .set("_pending_hooks", mlua::Value::Nil)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(())
    }
}
