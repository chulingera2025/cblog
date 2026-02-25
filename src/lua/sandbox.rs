use anyhow::{Context, Result};
use mlua::Lua;
use std::path::{Path, PathBuf};

pub fn apply(lua: &Lua, project_root: &Path) -> Result<()> {
    let globals = lua.globals();

    let os: mlua::Table = globals
        .get("os")
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    os.set("execute", mlua::Value::Nil)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    os.set("exit", mlua::Value::Nil)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let io: mlua::Table = globals
        .get("io")
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    io.set("popen", mlua::Value::Nil)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let root = project_root
        .canonicalize()
        .context("项目根目录 canonicalize 失败")?;

    let root_c = root.clone();
    let safe_open = lua
        .create_function(move |lua, (path, mode): (String, Option<String>)| {
            let full = resolve_path(&root_c, &path)?;
            let mode = mode.unwrap_or_else(|| "r".to_string());

            let io: mlua::Table = lua.globals().get("_io_original")?;
            let open_fn: mlua::Function = io.get("open")?;
            open_fn.call::<mlua::Value>((full.to_string_lossy().to_string(), mode))
        })
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let original_io = lua
        .create_table()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let original_open: mlua::Function = io
        .get("open")
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    original_io
        .set("open", original_open)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    globals
        .set("_io_original", original_io)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    io.set("open", safe_open)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(())
}

/// 解析相对路径并验证不越界项目根目录
pub fn resolve_path(project_root: &Path, relative: &str) -> Result<PathBuf, mlua::Error> {
    let path = Path::new(relative);

    if path.is_absolute() {
        return Err(mlua::Error::external(format!(
            "不允许绝对路径: {}",
            relative
        )));
    }

    let full = project_root.join(path);

    let canonical = if full.exists() {
        full.canonicalize().map_err(|e| {
            mlua::Error::external(format!("路径解析失败: {} - {}", relative, e))
        })?
    } else {
        let mut resolved = project_root.to_path_buf();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    resolved.pop();
                }
                std::path::Component::Normal(c) => {
                    resolved.push(c);
                }
                std::path::Component::CurDir => {}
                _ => {
                    return Err(mlua::Error::external(format!(
                        "路径包含非法组件: {}",
                        relative
                    )));
                }
            }
        }
        resolved
    };

    if !canonical.starts_with(project_root) {
        return Err(mlua::Error::external(format!(
            "路径越界: {} 不在项目根目录内",
            relative
        )));
    }

    Ok(canonical)
}
