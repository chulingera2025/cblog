use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// 构建统计信息
#[derive(Debug, Default, Clone)]
pub struct BuildStats {
    pub total_pages: usize,
    pub rebuilt: usize,
    pub cached: usize,
}

/// 内容哈希缓存：追踪文件 SHA-256 哈希，支持增量构建
pub struct HashCache {
    hashes: HashMap<String, String>,
    cache_path: PathBuf,
}

impl HashCache {
    /// 从缓存文件加载，不存在则返回空表
    pub fn load(cache_dir: &Path) -> Self {
        let cache_path = cache_dir.join("hashes.json");
        let hashes = std::fs::read_to_string(&cache_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { hashes, cache_path }
    }

    /// 持久化当前哈希表到缓存文件
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.hashes)?;
        std::fs::write(&self.cache_path, json)?;
        Ok(())
    }

    /// 计算文件的 SHA-256 哈希（十六进制）
    pub fn compute_hash(path: &Path) -> Result<String> {
        let data = std::fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// 计算任意字节数据的 SHA-256 哈希（十六进制）
    pub fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// 判断文件是否发生变更（哈希不同或不在缓存中）
    pub fn has_changed(&self, relative_path: &str, current_hash: &str) -> bool {
        match self.hashes.get(relative_path) {
            Some(cached) => cached != current_hash,
            None => true,
        }
    }

    /// 更新某个文件的哈希记录
    pub fn update(&mut self, relative_path: String, hash: String) {
        self.hashes.insert(relative_path, hash);
    }

    /// 检查项目配置文件 cblog.toml 是否发生变更
    pub fn config_changed(&self, project_root: &Path) -> Result<bool> {
        let config_path = project_root.join("cblog.toml");
        if !config_path.exists() {
            return Ok(false);
        }
        let hash = Self::compute_hash(&config_path)?;
        Ok(self.has_changed("cblog.toml", &hash))
    }

    /// 检查主题模板目录下哪些模板发生了变更，返回变更模板名称集合
    pub fn changed_templates(&self, themes_dir: &Path, active_theme: &str) -> HashSet<String> {
        let mut changed = HashSet::new();
        let template_dir = themes_dir.join(active_theme).join("templates");
        if !template_dir.exists() {
            return changed;
        }
        Self::scan_templates(&template_dir, &template_dir, self, &mut changed);
        changed
    }

    fn scan_templates(
        base_dir: &Path,
        current_dir: &Path,
        cache: &HashCache,
        changed: &mut HashSet<String>,
    ) {
        let Ok(entries) = std::fs::read_dir(current_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::scan_templates(base_dir, &path, cache, changed);
            } else if path.extension().is_some_and(|ext| ext == "cbtml") {
                let Ok(rel) = path.strip_prefix(base_dir) else {
                    continue;
                };
                let template_name = rel.to_string_lossy().to_string();
                let cache_key = format!("template:{template_name}");
                if let Ok(hash) = Self::compute_hash(&path)
                    && cache.has_changed(&cache_key, &hash)
                {
                    changed.insert(template_name);
                }
            }
        }
    }

    /// 批量更新模板哈希
    pub fn update_templates(&mut self, themes_dir: &Path, active_theme: &str) {
        let template_dir = themes_dir.join(active_theme).join("templates");
        if !template_dir.exists() {
            return;
        }
        Self::update_templates_recursive(&template_dir, &template_dir, self);
    }

    /// 获取缓存中所有以 "post:" 开头的键
    pub fn cached_post_keys(&self) -> Vec<String> {
        self.hashes
            .keys()
            .filter(|k| k.starts_with("post:"))
            .cloned()
            .collect()
    }

    fn update_templates_recursive(base_dir: &Path, current_dir: &Path, cache: &mut HashCache) {
        let Ok(entries) = std::fs::read_dir(current_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::update_templates_recursive(base_dir, &path, cache);
            } else if path.extension().is_some_and(|ext| ext == "cbtml") {
                let Ok(rel) = path.strip_prefix(base_dir) else {
                    continue;
                };
                let template_name = rel.to_string_lossy().to_string();
                let cache_key = format!("template:{template_name}");
                if let Ok(hash) = Self::compute_hash(&path) {
                    cache.update(cache_key, hash);
                }
            }
        }
    }
}
