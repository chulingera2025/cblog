use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::plugin::registry::load_plugin_info;

/// 解析插件加载顺序：检测冲突、拓扑排序（基于 after 依赖）
pub fn resolve_load_order(project_root: &Path, enabled: &[String]) -> Result<Vec<String>> {
    if enabled.is_empty() {
        return Ok(Vec::new());
    }

    let plugins_dir = project_root.join("plugins");
    let enabled_set: HashSet<&str> = enabled.iter().map(|s| s.as_str()).collect();

    // 加载所有启用插件的元数据
    let mut after_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut conflicts_map: HashMap<String, Vec<String>> = HashMap::new();

    for name in enabled {
        let toml_path = plugins_dir.join(name).join("plugin.toml");
        let info = load_plugin_info(&toml_path)
            .with_context(|| format!("加载插件 {name} 的 plugin.toml 失败"))?;

        // 只保留同样被启用的 after 依赖
        let deps: Vec<String> = info
            .dependencies
            .after
            .into_iter()
            .filter(|d| enabled_set.contains(d.as_str()))
            .collect();
        after_map.insert(name.clone(), deps);
        conflicts_map.insert(name.clone(), info.dependencies.conflicts);
    }

    // 冲突检测：如果 A 声明与 B 冲突，且 B 也被启用，则报错
    for (name, conflicts) in &conflicts_map {
        for c in conflicts {
            if enabled_set.contains(c.as_str()) {
                bail!("插件 {name} 与 {c} 存在冲突，不能同时启用");
            }
        }
    }

    // Kahn 算法拓扑排序
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for name in enabled {
        in_degree.entry(name.as_str()).or_insert(0);
        adj.entry(name.as_str()).or_default();
    }

    for (name, deps) in &after_map {
        for dep in deps {
            // dep 应该在 name 之前加载，即 dep -> name 的边
            adj.entry(dep.as_str()).or_default().push(name.as_str());
            *in_degree.entry(name.as_str()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&str> = VecDeque::new();
    for (name, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(name);
        }
    }

    // 稳定排序：同层按字母序处理
    let mut result = Vec::with_capacity(enabled.len());
    while !queue.is_empty() {
        let mut batch: Vec<&str> = queue.drain(..).collect();
        batch.sort();
        for node in batch {
            result.push(node.to_string());
            if let Some(neighbors) = adj.get(node) {
                for &next in neighbors {
                    let deg = in_degree.get_mut(next).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    if result.len() != enabled.len() {
        bail!("插件依赖存在循环：无法确定加载顺序");
    }

    Ok(result)
}
