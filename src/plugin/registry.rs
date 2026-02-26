use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// plugin.toml 完整结构
#[derive(Debug, Deserialize)]
pub struct PluginToml {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub capabilities: PluginCapabilities,
    #[serde(default)]
    pub dependencies: PluginDependencies,
}

/// [plugin] 元数据
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PluginMeta {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub min_cblog: Option<String>,
}

/// [capabilities] 能力声明
#[derive(Debug, Default, Deserialize)]
pub struct PluginCapabilities {
    #[serde(default)]
    pub reads: Vec<String>,
    #[serde(default)]
    pub writes: Vec<String>,
    #[serde(default)]
    pub generates: Vec<String>,
}

/// [dependencies] 依赖关系
#[derive(Debug, Default, Deserialize)]
pub struct PluginDependencies {
    #[serde(default)]
    pub after: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
}

/// 已加载插件的运行时信息
#[derive(Debug)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: PluginCapabilities,
    pub dependencies: PluginDependencies,
}

/// 从 plugin.toml 加载插件元数据
pub fn load_plugin_info(path: &Path) -> Result<PluginInfo> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("读取 plugin.toml 失败: {}", path.display()))?;
    let toml: PluginToml = toml::from_str(&content)
        .with_context(|| format!("解析 plugin.toml 失败: {}", path.display()))?;

    Ok(PluginInfo {
        name: toml.plugin.name,
        version: toml.plugin.version,
        description: toml.plugin.description,
        capabilities: toml.capabilities,
        dependencies: toml.dependencies,
    })
}

/// 列出 plugins/ 目录下所有包含 plugin.toml 的插件名
pub fn list_available_plugins(project_root: &Path) -> Result<Vec<String>> {
    let plugins_dir = project_root.join("plugins");
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&plugins_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let toml_path = entry.path().join("plugin.toml");
            if toml_path.exists()
                && let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
        }
    }
    names.sort();
    Ok(names)
}
