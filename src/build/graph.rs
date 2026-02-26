use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 模板依赖图：记录每个模板被哪些页面使用
pub struct DepGraph {
    deps: HashMap<String, Vec<String>>,
    cache_path: PathBuf,
}

impl DepGraph {
    /// 从缓存文件加载，不存在则返回空图
    pub fn load(cache_dir: &Path) -> Self {
        let cache_path = cache_dir.join("deps.json");
        let deps = std::fs::read_to_string(&cache_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { deps, cache_path }
    }

    /// 持久化依赖图到缓存文件
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.deps)?;
        std::fs::write(&self.cache_path, json)?;
        Ok(())
    }

    /// 记录页面使用了某个模板
    pub fn add_dependency(&mut self, template: &str, page_url: &str) {
        self.deps
            .entry(template.to_owned())
            .or_default()
            .push(page_url.to_owned());
    }

    /// 获取使用了某个模板的所有页面 URL
    pub fn get_affected_pages(&self, template: &str) -> Vec<&str> {
        self.deps
            .get(template)
            .map(|urls| urls.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// 清空依赖图（每次构建前重建）
    pub fn clear(&mut self) {
        self.deps.clear();
    }
}
