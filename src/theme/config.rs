use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// theme.toml 完整结构
#[derive(Debug, Deserialize)]
pub struct ThemeToml {
    pub theme: ThemeMeta,
    #[serde(default)]
    pub config: Vec<ConfigField>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ThemeMeta {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    pub parent: Option<String>,
}

/// [[config]] 配置项定义，对应后台设置表单的一个字段
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigField {
    pub key: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub label: String,
    #[serde(default = "default_toml_value")]
    pub default: toml::Value,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub options: Vec<toml::Value>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub depends_on: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ResolvedTheme {
    pub name: String,
    pub meta: ThemeMeta,
    pub config_schema: Vec<ConfigField>,
    /// 继承链：从当前主题到根主题
    pub parent_chain: Vec<String>,
}

/// 加载单个主题的 theme.toml
pub fn load_theme_toml(project_root: &Path, theme_name: &str) -> Result<ThemeToml> {
    let theme_path = project_root
        .join("themes")
        .join(theme_name)
        .join("theme.toml");
    let content = std::fs::read_to_string(&theme_path)
        .with_context(|| format!("读取 theme.toml 失败: {}", theme_path.display()))?;
    let theme_toml: ThemeToml = toml::from_str(&content)
        .with_context(|| format!("解析 theme.toml 失败: {}", theme_path.display()))?;
    Ok(theme_toml)
}

/// 解析主题并处理子主题继承链
///
/// 子主题通过 parent 字段声明父主题，配置项从根主题向下合并，
/// 子主题可以覆盖父主题同 key 配置项的 default 值。
pub fn resolve_theme(project_root: &Path, theme_name: &str) -> Result<ResolvedTheme> {
    let mut chain = Vec::new();
    let mut current = theme_name.to_string();
    let mut all_tomls = Vec::new();

    loop {
        if chain.contains(&current) {
            anyhow::bail!("检测到主题循环继承: {:?} -> {}", chain, current);
        }
        chain.push(current.clone());
        let toml = load_theme_toml(project_root, &current)?;
        let parent = toml.theme.parent.clone();
        all_tomls.push(toml);
        match parent {
            Some(p) => current = p,
            None => break,
        }
    }

    // 从根主题（链尾）向子主题（链首）合并配置字段
    let mut merged_fields: Vec<ConfigField> = Vec::new();
    let mut seen_keys: HashMap<String, usize> = HashMap::new();

    for toml in all_tomls.iter().rev() {
        for field in &toml.config {
            if let Some(&idx) = seen_keys.get(&field.key) {
                merged_fields[idx] = field.clone();
            } else {
                seen_keys.insert(field.key.clone(), merged_fields.len());
                merged_fields.push(field.clone());
            }
        }
    }

    let leaf = all_tomls.into_iter().next().unwrap();

    Ok(ResolvedTheme {
        name: theme_name.to_string(),
        meta: leaf.theme,
        config_schema: merged_fields,
        parent_chain: chain,
    })
}

/// 从配置 schema 生成默认值映射
pub fn default_values(schema: &[ConfigField]) -> HashMap<String, serde_json::Value> {
    schema
        .iter()
        .map(|f| (f.key.clone(), toml_to_json(&f.default)))
        .collect()
}

/// 合并默认值和已保存的配置值，已保存的值优先
pub fn effective_values(
    schema: &[ConfigField],
    saved: &HashMap<String, serde_json::Value>,
) -> HashMap<String, serde_json::Value> {
    let mut values = default_values(schema);
    for (key, val) in saved {
        if values.contains_key(key) {
            values.insert(key.clone(), val.clone());
        }
    }
    values
}

/// 列出所有已安装的主题（存在 theme.toml 的目录）
pub fn list_themes(project_root: &Path) -> Result<Vec<String>> {
    let themes_dir = project_root.join("themes");
    if !themes_dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&themes_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let theme_toml = entry.path().join("theme.toml");
            if theme_toml.exists()
                && let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
        }
    }
    names.sort();
    Ok(names)
}

/// 从 select/font_select 配置项的 options 中提取 (value, label) 对
pub fn extract_option_pairs(options: &[toml::Value]) -> Vec<(String, String)> {
    options
        .iter()
        .filter_map(|opt| match opt {
            toml::Value::String(s) => Some((s.clone(), s.clone())),
            toml::Value::Table(tbl) => {
                let value = tbl.get("value")?.as_str()?.to_string();
                let label = tbl.get("label")?.as_str()?.to_string();
                Some((value, label))
            }
            _ => None,
        })
        .collect()
}

pub fn toml_to_json(val: &toml::Value) -> serde_json::Value {
    match val {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::json!(*i),
        toml::Value::Float(f) => serde_json::json!(*f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(tbl) => {
            let map: serde_json::Map<String, serde_json::Value> =
                tbl.iter().map(|(k, v)| (k.clone(), toml_to_json(v))).collect();
            serde_json::Value::Object(map)
        }
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
    }
}

fn default_toml_value() -> toml::Value {
    toml::Value::String(String::new())
}

pub fn build_scss_overrides(values: &HashMap<String, serde_json::Value>) -> String {
    let mut overrides = String::new();
    for (key, val) in values {
        let scss_var = key.replace('_', "-");
        match val {
            serde_json::Value::String(s) => {
                overrides.push_str(&format!("${scss_var}: {s};\n"));
            }
            serde_json::Value::Number(n) => {
                overrides.push_str(&format!("${scss_var}: {n};\n"));
            }
            serde_json::Value::Bool(b) => {
                overrides.push_str(&format!("${scss_var}: {b};\n"));
            }
            _ => {}
        }
    }
    overrides
}
