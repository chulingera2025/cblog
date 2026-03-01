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
    #[serde(default)]
    pub admin: PluginAdmin,
}

/// [admin] 后台管理页面声明
#[derive(Debug, Default, Deserialize)]
pub struct PluginAdmin {
    #[serde(default)]
    pub pages: Vec<PluginAdminPage>,
}

/// [[admin.pages]] 单个后台页面
#[derive(Debug, Deserialize)]
pub struct PluginAdminPage {
    pub label: String,
    pub slug: String,
    #[serde(default)]
    pub icon: String,
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
    pub min_cblog: Option<String>,
    pub capabilities: PluginCapabilities,
    pub dependencies: PluginDependencies,
}

/// 从 plugin.toml 加载完整的 PluginToml 结构
pub fn load_plugin_toml(path: &Path) -> Result<PluginToml> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("读取 plugin.toml 失败: {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("解析 plugin.toml 失败: {}", path.display()))
}

/// 从 plugin.toml 加载插件元数据
pub fn load_plugin_info(path: &Path) -> Result<PluginInfo> {
    let toml = load_plugin_toml(path)?;

    Ok(PluginInfo {
        name: toml.plugin.name,
        version: toml.plugin.version,
        description: toml.plugin.description,
        min_cblog: toml.plugin.min_cblog,
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

/// 语义版本比较：v1 < v2 返回 true
pub fn version_lt(v1: &str, v2: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|s| s.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let a = parse(v1);
    let b = parse(v2);
    let len = a.len().max(b.len());
    for i in 0..len {
        let sa = a.get(i).copied().unwrap_or(0);
        let sb = b.get(i).copied().unwrap_or(0);
        if sa < sb {
            return true;
        }
        if sa > sb {
            return false;
        }
    }
    false
}
