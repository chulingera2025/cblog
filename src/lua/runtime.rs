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
                lua.create_function(|_, date_str: String| {
                    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime};

                    // RFC 3339 (如 "2024-01-15T10:30:00+08:00")
                    if let Ok(dt) = DateTime::<FixedOffset>::parse_from_rfc3339(&date_str) {
                        return Ok(dt.to_rfc3339());
                    }
                    // "YYYY-MM-DD HH:MM:SS"
                    if let Ok(ndt) = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S") {
                        return Ok(ndt.format("%Y-%m-%dT%H:%M:%S").to_string());
                    }
                    // "YYYY-MM-DD"
                    if let Ok(nd) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                        return Ok(nd.format("%Y-%m-%dT00:00:00").to_string());
                    }
                    // 解析失败返回原字符串
                    Ok(date_str)
                })
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
                    Ok(crate::plugin::registry::version_lt(&v1, &v2))
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

        // cblog.http.* — 网络请求能力
        let http = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        http.set(
            "get",
            lua.create_function(|lua, (url, opts): (String, Option<mlua::Table>)| {
                let headers = extract_headers_from_opts(lua, opts.as_ref())?;
                let resp = execute_http_request("GET", &url, headers, None, None)?;
                resp.to_lua_table(lua)
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        http.set(
            "post",
            lua.create_function(|lua, (url, opts): (String, Option<mlua::Table>)| {
                let headers = extract_headers_from_opts(lua, opts.as_ref())?;
                let body = opts
                    .as_ref()
                    .and_then(|t| t.get::<String>("body").ok());
                let resp = execute_http_request("POST", &url, headers, body, None)?;
                resp.to_lua_table(lua)
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        http.set(
            "put",
            lua.create_function(|lua, (url, opts): (String, Option<mlua::Table>)| {
                let headers = extract_headers_from_opts(lua, opts.as_ref())?;
                let body = opts
                    .as_ref()
                    .and_then(|t| t.get::<String>("body").ok());
                let resp = execute_http_request("PUT", &url, headers, body, None)?;
                resp.to_lua_table(lua)
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        http.set(
            "delete",
            lua.create_function(|lua, (url, opts): (String, Option<mlua::Table>)| {
                let headers = extract_headers_from_opts(lua, opts.as_ref())?;
                let resp = execute_http_request("DELETE", &url, headers, None, None)?;
                resp.to_lua_table(lua)
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        let root_http = self.project_root.clone();
        http.set(
            "put_file",
            lua.create_function(move |lua, (url, filepath, opts): (String, String, Option<mlua::Table>)| {
                let headers = extract_headers_from_opts(lua, opts.as_ref())?;
                let full = sandbox::resolve_path(&root_http, &filepath)?;
                let resp = execute_http_request("PUT", &url, headers, None, Some(&full))?;
                resp.to_lua_table(lua)
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        cblog
            .set("http", http)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // cblog.s3.* — AWS Signature V4 签名辅助
        let s3 = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        s3.set(
            "sign_headers",
            lua.create_function(
                |lua,
                 (method, url, region, access_key, secret_key, headers, payload_hash): (
                    String,
                    String,
                    String,
                    String,
                    String,
                    Option<mlua::Table>,
                    Option<String>,
                )| {
                    let extra_headers = if let Some(h) = headers.as_ref() {
                        let mut v = Vec::new();
                        for pair in h.pairs::<String, String>() {
                            let (k, val) = pair?;
                            v.push((k, val));
                        }
                        v
                    } else {
                        Vec::new()
                    };

                    let payload = payload_hash.as_deref().unwrap_or("UNSIGNED-PAYLOAD");
                    let signed = compute_s3_signature(
                        &method,
                        &url,
                        &region,
                        &access_key,
                        &secret_key,
                        &extra_headers,
                        payload,
                    )
                    .map_err(mlua::Error::external)?;

                    let tbl = lua.create_table()?;
                    for (k, v) in &signed {
                        tbl.set(k.as_str(), v.as_str())?;
                    }
                    Ok(tbl)
                },
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        cblog
            .set("s3", s3)
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

            // min_cblog 版本校验
            if let Some(ref min_ver) = info.min_cblog {
                let current_ver = env!("CARGO_PKG_VERSION");
                if crate::plugin::registry::version_lt(current_ver, min_ver) {
                    anyhow::bail!(
                        "插件 {} 要求 cblog >= {}，当前版本 {}",
                        name,
                        min_ver,
                        current_ver
                    );
                }
            }

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

/// HTTP 响应的中间表示，用于转换为 Lua table
struct HttpResponse {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
}

impl HttpResponse {
    fn to_lua_table(&self, lua: &Lua) -> Result<mlua::Table, mlua::Error> {
        let tbl = lua.create_table()?;
        tbl.set("status", self.status)?;
        tbl.set("body", self.body.clone())?;
        let hdrs = lua.create_table()?;
        for (k, v) in &self.headers {
            hdrs.set(k.as_str(), v.as_str())?;
        }
        tbl.set("headers", hdrs)?;
        Ok(tbl)
    }
}

/// 从 Lua opts table 中提取 headers 字段
fn extract_headers_from_opts(
    _lua: &Lua,
    opts: Option<&mlua::Table>,
) -> Result<Vec<(String, String)>, mlua::Error> {
    let Some(opts) = opts else {
        return Ok(Vec::new());
    };
    let Ok(headers_tbl) = opts.get::<mlua::Table>("headers") else {
        return Ok(Vec::new());
    };
    let mut headers = Vec::new();
    for pair in headers_tbl.pairs::<String, String>() {
        let (k, v) = pair?;
        headers.push((k, v));
    }
    Ok(headers)
}

/// 在同步上下文中执行 HTTP 请求
/// Lua 运行时是同步的，需要桥接到 tokio 异步运行时
fn execute_http_request(
    method: &str,
    url: &str,
    headers: Vec<(String, String)>,
    body: Option<String>,
    file_path: Option<&std::path::Path>,
) -> Result<HttpResponse, mlua::Error> {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::time::Duration;

    let exec = || async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

        let mut header_map = HeaderMap::new();
        for (k, v) in &headers {
            let name = HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| format!("无效的 header name '{k}': {e}"))?;
            let value = HeaderValue::from_str(v)
                .map_err(|e| format!("无效的 header value '{v}': {e}"))?;
            header_map.insert(name, value);
        }

        let mut req = match method {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            _ => return Err(format!("不支持的 HTTP 方法: {method}")),
        };

        req = req.headers(header_map);

        if let Some(fp) = file_path {
            let data = std::fs::read(fp)
                .map_err(|e| format!("读取文件失败 {}: {e}", fp.display()))?;
            req = req.body(data);
        } else if let Some(b) = body {
            req = req.body(b);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("HTTP 请求失败: {e}"))?;

        let status = resp.status().as_u16();
        let resp_headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let resp_body = resp
            .text()
            .await
            .map_err(|e| format!("读取响应体失败: {e}"))?;

        Ok(HttpResponse {
            status,
            body: resp_body,
            headers: resp_headers,
        })
    };

    // 尝试使用已有 tokio runtime，否则创建新的
    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(exec()))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| mlua::Error::external(format!("创建 tokio runtime 失败: {e}")))?
            .block_on(exec())
    };

    result.map_err(mlua::Error::external)
}

/// AWS Signature V4 签名计算
/// 返回签名后需要附加到请求的 headers（包含 Authorization, x-amz-date, x-amz-content-sha256, host）
fn compute_s3_signature(
    method: &str,
    url: &str,
    region: &str,
    access_key: &str,
    secret_key: &str,
    extra_headers: &[(String, String)],
    payload_hash: &str,
) -> Result<Vec<(String, String)>, String> {
    use hmac::{Hmac, Mac};
    use sha2::{Digest, Sha256};

    let parsed = reqwest::Url::parse(url).map_err(|e| format!("URL 解析失败: {e}"))?;
    let host = parsed
        .host_str()
        .ok_or("URL 中缺少 host")?
        .to_string();
    let path = parsed.path().to_string();
    let query = parsed.query().unwrap_or("").to_string();

    let now = chrono::Utc::now();
    let date_stamp = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

    // 构建所有需要签名的 headers
    let mut sign_headers: Vec<(String, String)> = vec![
        ("host".to_string(), host.clone()),
        ("x-amz-content-sha256".to_string(), payload_hash.to_string()),
        ("x-amz-date".to_string(), amz_date.clone()),
    ];
    for (k, v) in extra_headers {
        let lower_k = k.to_lowercase();
        if lower_k != "host" && lower_k != "x-amz-content-sha256" && lower_k != "x-amz-date" {
            sign_headers.push((lower_k, v.clone()));
        }
    }
    sign_headers.sort_by(|a, b| a.0.cmp(&b.0));

    let signed_headers_str: String = sign_headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_headers: String = sign_headers
        .iter()
        .map(|(k, v)| format!("{}:{}\n", k, v.trim()))
        .collect();

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, path, query, canonical_headers, signed_headers_str, payload_hash
    );

    let scope = format!("{}/{}/s3/aws4_request", date_stamp, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{:x}",
        amz_date,
        scope,
        Sha256::digest(canonical_request.as_bytes())
    );

    type HmacSha256 = Hmac<Sha256>;

    let signing_key = {
        let k_date = HmacSha256::new_from_slice(format!("AWS4{}", secret_key).as_bytes())
            .map_err(|e| format!("HMAC 初始化失败: {e}"))?
            .chain_update(date_stamp.as_bytes())
            .finalize()
            .into_bytes();
        let k_region = HmacSha256::new_from_slice(&k_date)
            .map_err(|e| format!("HMAC 初始化失败: {e}"))?
            .chain_update(region.as_bytes())
            .finalize()
            .into_bytes();
        let k_service = HmacSha256::new_from_slice(&k_region)
            .map_err(|e| format!("HMAC 初始化失败: {e}"))?
            .chain_update(b"s3")
            .finalize()
            .into_bytes();
        HmacSha256::new_from_slice(&k_service)
            .map_err(|e| format!("HMAC 初始化失败: {e}"))?
            .chain_update(b"aws4_request")
            .finalize()
            .into_bytes()
    };

    let signature = HmacSha256::new_from_slice(&signing_key)
        .map_err(|e| format!("HMAC 初始化失败: {e}"))?
        .chain_update(string_to_sign.as_bytes())
        .finalize()
        .into_bytes();
    let signature_hex: String = signature.iter().map(|b| format!("{b:02x}")).collect();

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        access_key, scope, signed_headers_str, signature_hex
    );

    let mut result = vec![
        ("Authorization".to_string(), authorization),
        ("x-amz-date".to_string(), amz_date),
        ("x-amz-content-sha256".to_string(), payload_hash.to_string()),
        ("host".to_string(), host),
    ];
    for (k, v) in extra_headers {
        let lower_k = k.to_lowercase();
        if lower_k != "host" && lower_k != "x-amz-content-sha256" && lower_k != "x-amz-date" {
            result.push((k.clone(), v.clone()));
        }
    }

    Ok(result)
}
