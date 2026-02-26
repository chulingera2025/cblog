use mlua::{LuaSerdeExt, RegistryKey};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;

/// Hook 注册表：管理 filter 和 action 两类 hook
pub struct HookRegistry {
    filters: HashMap<String, Vec<(i32, RegistryKey)>>,
    actions: HashMap<String, Vec<(i32, RegistryKey)>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            filters: HashMap::new(),
            actions: HashMap::new(),
        }
    }

    pub fn add_filter(&mut self, hook: &str, priority: i32, key: RegistryKey) {
        self.filters
            .entry(hook.to_string())
            .or_default()
            .push((priority, key));
    }

    pub fn add_action(&mut self, hook: &str, priority: i32, key: RegistryKey) {
        self.actions
            .entry(hook.to_string())
            .or_default()
            .push((priority, key));
    }

    /// 执行 filter hook：数据依次流经所有按优先级排序的处理器
    #[allow(dead_code)]
    pub fn apply_filter<T>(
        &self,
        lua: &mlua::Lua,
        hook: &str,
        value: T,
    ) -> anyhow::Result<T>
    where
        T: Serialize + DeserializeOwned,
    {
        let handlers = match self.filters.get(hook) {
            Some(h) => h,
            None => return Ok(value),
        };

        let mut sorted: Vec<_> = handlers.iter().collect();
        sorted.sort_by_key(|(p, _)| *p);

        let mut current = value;
        for (priority, key) in sorted {
            let func: mlua::Function = lua
                .registry_value(key)
                .map_err(|e| anyhow::anyhow!("获取 filter '{}' handler 失败: {}", hook, e))?;
            let lua_val = lua
                .to_value(&current)
                .map_err(|e| anyhow::anyhow!("序列化 filter '{}' 输入失败: {}", hook, e))?;
            let result = func.call::<mlua::Value>(lua_val).map_err(|e| {
                anyhow::anyhow!("filter '{}' 执行失败 (priority={}): {}", hook, priority, e)
            })?;
            current = lua.from_value(result).map_err(|e| {
                anyhow::anyhow!(
                    "filter '{}' 返回值反序列化失败 (priority={}): {}",
                    hook,
                    priority,
                    e
                )
            })?;
        }

        Ok(current)
    }

    /// 执行 action hook：按优先级排序调用所有处理器，忽略返回值
    pub fn call_action<T>(
        &self,
        lua: &mlua::Lua,
        hook: &str,
        ctx: &T,
    ) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let handlers = match self.actions.get(hook) {
            Some(h) => h,
            None => return Ok(()),
        };

        let mut sorted: Vec<_> = handlers.iter().collect();
        sorted.sort_by_key(|(p, _)| *p);

        let lua_ctx = lua
            .to_value(ctx)
            .map_err(|e| anyhow::anyhow!("序列化 action '{}' 上下文失败: {}", hook, e))?;
        for (priority, key) in sorted {
            let func: mlua::Function = lua
                .registry_value(key)
                .map_err(|e| anyhow::anyhow!("获取 action '{}' handler 失败: {}", hook, e))?;
            func.call::<()>(lua_ctx.clone()).map_err(|e| {
                anyhow::anyhow!("action '{}' 执行失败 (priority={}): {}", hook, priority, e)
            })?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn has_handlers(&self, hook: &str) -> bool {
        self.filters
            .get(hook)
            .is_some_and(|h| !h.is_empty())
            || self
                .actions
                .get(hook)
                .is_some_and(|h| !h.is_empty())
    }
}
