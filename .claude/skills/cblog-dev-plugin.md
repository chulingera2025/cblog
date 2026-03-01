# cblog-dev-plugin

cblog 插件开发技能。当用户需要为 cblog 创建、修改或调试插件时使用此技能。

---

## 一、cblog 插件系统概述

cblog 的插件系统基于 **Lua 5.4 脚本引擎**（通过 `mlua` crate 嵌入），采用 **hook（钩子）驱动** 架构。插件以独立目录形式存在于项目根目录的 `plugins/` 下，每个插件包含一个 `plugin.toml` 元数据文件和一个 `main.lua` 入口脚本。

**核心设计理念：**
- 插件通过 Lua 脚本编写，热加载无需重新编译 Rust 代码
- 插件在构建流水线的各阶段通过 hook 介入
- 运行在沙箱化的 Lua VM 中，路径限制 + 危险 API 移除
- 插件配置通过数据库 KV 存储持久化
- 后台管理面板提供插件的启用/禁用/配置界面
- 插件可声明自定义的后台管理页面

**关键源码文件：**

| 文件 | 职责 |
|------|------|
| `src/plugin.rs` | 模块入口，导出 registry、scheduler、store |
| `src/plugin/registry.rs` | 插件元数据结构体、plugin.toml 解析、插件发现 |
| `src/plugin/scheduler.rs` | 插件加载顺序（拓扑排序）、冲突检测 |
| `src/plugin/store.rs` | 插件 KV 存储（数据库 CRUD） |
| `src/lua.rs` | Lua 模块入口，导出 hooks、runtime、sandbox |
| `src/lua/hooks.rs` | Hook 注册表（filter/action 两类 hook） |
| `src/lua/runtime.rs` | 插件引擎核心（Lua VM 创建、API 注册、插件加载） |
| `src/lua/sandbox.rs` | Lua 沙箱安全策略 |
| `src/admin/plugins.rs` | 后台插件管理 handler |
| `src/admin.rs` | 路由注册（含插件自定义后台页面路由） |
| `src/build/pipeline.rs` | 构建流水线（插件引擎初始化和 hook 调用点） |
| `src/config.rs` | 站点配置（PluginConfig 结构体） |
| `src/state.rs` | 应用状态（plugin_admin_pages 收集） |
| `src/check.rs` | 项目完整性检查（含插件检查逻辑） |

---

## 二、插件目录结构

### 2.1 标准目录结构

```
plugins/
  <plugin-name>/
    plugin.toml         # 必须，插件元数据配置
    main.lua            # 必须，插件入口脚本
    lib/                # 可选，Lua 模块库目录（自动加入 require 路径）
      utils.lua         # 自定义 Lua 模块
    admin/              # 可选，自定义后台页面模板
      <slug>.cbtml      # 对应 plugin.toml 中 [[admin.pages]] 的 slug
```

### 2.2 关键规则

- 插件目录名即插件标识名
- `plugin.toml` 和 `main.lua` 缺一不可
- `lib/` 目录下的 Lua 文件可以通过 `require("module_name")` 引入
- `admin/` 目录下的 `.cbtml` 模板文件用于自定义后台页面

---

## 三、plugin.toml 配置文件完整格式

```toml
[plugin]
name = "my-plugin"                     # 必填，插件名称（应与目录名一致）
version = "1.0.0"                      # 可选，版本号
author = "作者名"                       # 可选，作者
description = "插件功能描述"             # 可选，描述（显示在后台插件列表）
homepage = "https://github.com/..."    # 可选，主页 URL
min_cblog = "0.1.0"                    # 可选，最低 cblog 版本要求（加载时校验）

[capabilities]
reads = ["post.title", "post.content", "post.tags"]    # 声明读取哪些数据
writes = []                                             # 声明写入哪些数据
generates = ["public/search-index.json"]                # 声明生成哪些文件

[dependencies]
after = ["other-plugin"]               # 必须在这些插件之后加载
conflicts = ["bad-plugin"]             # 与这些插件冲突，不能同时启用

# 可选，声明后台管理页面（可以有多个）
[[admin.pages]]
label = "搜索统计"                      # 侧边栏显示名称
slug = "stats"                         # URL 路径段（/admin/ext/{plugin}/{slug}）
icon = "search"                        # 图标名称（对应 svg_icon 函数支持的图标名）

[[admin.pages]]
label = "高级设置"
slug = "settings"
icon = "settings"
```

### 3.1 核心结构体

| 结构体 | 说明 |
|--------|------|
| `PluginToml` | plugin.toml 完整结构 |
| `PluginMeta` | `[plugin]` 段元信息 |
| `PluginCapabilities` | `[capabilities]` 段能力声明 |
| `PluginDependencies` | `[dependencies]` 段依赖关系 |
| `PluginAdmin` | `[admin]` 段后台页面声明 |
| `PluginAdminPage` | 单个后台页面声明 |
| `PluginInfo` | 运行时插件信息 |
| `PluginEngine` | 插件引擎（Lua VM + HookRegistry） |
| `HookRegistry` | Hook 注册表（filters + actions） |

### 3.2 capabilities 说明

`capabilities` 目前仅为**声明式**，系统不强制校验。但它是良好实践，帮助用户理解插件的行为范围：
- `reads`: 插件读取哪些数据（如 `post.title`, `post.content`）
- `writes`: 插件修改哪些数据
- `generates`: 插件生成哪些文件（如 `public/search-index.json`）

---

## 四、启用插件

在 `cblog.toml` 的 `[plugins]` 段配置：

```toml
[plugins]
enabled = ["search", "seo-optimizer", "syntax-highlight"]
```

也可在后台管理面板通过 `/admin/plugins` 页面启用/禁用（直接编辑 `cblog.toml`）。

---

## 五、插件加载流程

### 5.1 完整加载流程

```
1. 检查 config.plugins.enabled 是否为空
2. scheduler::resolve_load_order() 获取排序后的插件列表
   → 冲突检测（conflicts）
   → 拓扑排序（Kahn 算法，根据 after 依赖）
   → 同层按字母序保证确定性
3. PluginEngine::new() 初始化
   → 创建 Lua 5.4 VM
   → 应用沙箱（移除危险 API、限制文件路径）
   → 注册 cblog.* 全局 API
4. engine.load_plugins(&ordered) 逐个加载
   → 检查 plugin.toml 存在
   → 加载 PluginInfo（元数据）
   → 如果有 lib/ 目录，加入 Lua require 搜索路径
   → setup_plugin_api(name) 创建 plugin.filter/action/config API
   → 执行 main.lua 脚本
   → collect_pending_hooks() 收集注册的 hook 到 Rust HookRegistry
```

### 5.2 依赖解析

`resolve_load_order()`（`src/plugin/scheduler.rs`）：
1. 只保留 `after` 中同样在 enabled 列表中的插件
2. 遍历所有 `conflicts` 声明，若目标也被启用则报错
3. Kahn 算法拓扑排序，同层按字母序
4. 排序结果数量不等于启用插件数量时报 "循环依赖"

---

## 六、Hook 系统

### 6.1 两种 Hook 类型

**Filter hook**（数据转换型）：
- 接收一个值，经处理后返回修改过的值
- 多个 handler 按优先级串联执行，数据流经每个 handler
- 低优先级数字先执行

**Action hook**（动作型）：
- 接收上下文数据，执行操作但不修改返回值
- 多个 handler 按优先级依次执行

### 6.2 注册 Hook

在 `main.lua` 中通过 `plugin.filter()` 和 `plugin.action()` 注册：

```lua
-- 注册 filter hook
plugin.filter("hook_name", priority, function(data)
    -- 处理数据，必须返回修改后的值
    return modified_data
end)

-- 注册 action hook
plugin.action("hook_name", priority, function(ctx)
    -- 执行操作，不需要返回值
end)
```

**优先级（priority）**：整数值，越小越先执行。建议范围 1-100，默认使用 10。

### 6.3 注册机制内部流程

1. `setup_plugin_api()` 创建 `_pending_hooks` 临时 Lua 表和 `plugin.filter`/`plugin.action` 函数
2. `main.lua` 执行时，调用 `plugin.filter/action` 将 hook 信息暂存到 `_pending_hooks`
3. 脚本执行完毕后，`collect_pending_hooks()` 将暂存的 hook 注册到 Rust 端 `HookRegistry`
4. 清空 `_pending_hooks` 表

### 6.4 构建管道中的 Hook 调用点

| Hook 名称 | 调用时机 | 传入的上下文数据 |
|-----------|---------|----------------|
| `after_load` | 文章从数据库加载完毕后 | `{ project_root, posts }` |
| `after_taxonomy` | 分类索引构建完毕后 | `{ project_root, output_dir, tag_count, category_count, archive_count }` |
| `after_render` | 页面渲染完毕后 | `{ project_root, output_dir }` |
| `after_assets` | 静态资源处理完毕后 | `{ project_root, output_dir }` |
| `after_finalize` | 构建最终阶段完成后 | `{ project_root, output_dir, posts, site_url }` |

**重要**：当前构建管道中所有 hook 均以 **action** 类型调用（`call_action`）。

### 6.5 Hook 上下文数据详情

**`after_load` 上下文：**
```lua
ctx.project_root    -- 项目根目录路径
ctx.posts           -- 文章数组，每个元素包含：
  post.id           -- 文章 ID (ULID)
  post.slug         -- URL slug
  post.title        -- 标题
  post.url          -- URL ("/posts/{slug}/")
  post.content      -- HTML 正文
  post.tags         -- 标签数组
  post.category     -- 分类名
  post.excerpt      -- 摘要
  post.created_at   -- 创建时间
  post.updated_at   -- 更新时间
  post.toc          -- 目录 HTML
  post.cover_image  -- 封面图 URL
  post.author       -- 作者
  post.reading_time -- 阅读时间（分钟）
  post.word_count   -- 字数
```

**`after_finalize` 上下文：**
```lua
ctx.project_root    -- 项目根目录路径
ctx.output_dir      -- 输出目录路径（通常是 "public"）
ctx.posts           -- 同 after_load 的 posts
ctx.site_url        -- 站点 URL
```

**`after_taxonomy` 上下文：**
```lua
ctx.project_root    -- 项目根目录路径
ctx.output_dir      -- 输出目录路径（通常是 "public"）
ctx.tag_count       -- 标签数量
ctx.category_count  -- 分类数量
ctx.archive_count   -- 月份归档数量
```

**`after_render` / `after_assets` 上下文：**
```lua
ctx.project_root    -- 项目根目录路径
ctx.output_dir      -- 输出目录路径（通常是 "public"）
```

---

## 七、Lua API 完整参考

### 7.1 `cblog` 全局对象

| API | 签名 | 说明 |
|-----|------|------|
| `cblog.version()` | `() -> string` | 返回 cblog 版本号（含 git commit） |
| `cblog.slugify(text)` | `string -> string` | 文本转 URL-safe slug |
| `cblog.json(table)` | `table -> string` | Lua table 序列化为 JSON 字符串 |
| `cblog.iso_date(date_str)` | `string -> string` | 日期格式化为 ISO 格式（支持 RFC3339、YYYY-MM-DD HH:MM:SS、YYYY-MM-DD，解析失败返回原值） |
| `cblog.site()` | `() -> table` | 返回站点信息 |
| `cblog.strip_html(html)` | `string -> string` | 去除 HTML 标签，提取纯文本 |
| `cblog.highlight(code, lang)` | `(string, string) -> string` | 基于 syntect 的代码语法高亮 |
| `cblog.version_lt(v1, v2)` | `(string, string) -> bool` | 语义版本比较 v1 < v2 |

**`cblog.site()` 返回值结构：**
```lua
{
  title = "站点标题",
  url = "https://example.com",
  language = "zh-CN",
  description = "站点描述",
  author = {
    name = "作者名",
    email = "邮箱"
  }
}
```

### 7.2 `cblog.log` 日志 API

| API | 说明 |
|-----|------|
| `cblog.log.info(msg)` | INFO 级别日志，前缀 `[plugin]` |
| `cblog.log.warn(msg)` | WARN 级别日志 |
| `cblog.log.error(msg)` | ERROR 级别日志 |
| `cblog.log.debug(msg)` | DEBUG 级别日志 |

### 7.3 `cblog.files` 文件操作 API

所有路径相对于项目根目录，经过沙箱路径验证。

| API | 签名 | 说明 |
|-----|------|------|
| `cblog.files.read(path)` | `string -> string` | 读取文件内容 |
| `cblog.files.write(path, content)` | `(string, string) -> nil` | 写入文件（自动创建父目录） |
| `cblog.files.exists(path)` | `string -> bool` | 判断文件/目录是否存在 |
| `cblog.files.remove(path)` | `string -> nil` | 删除文件 |
| `cblog.files.mkdir(path)` | `string -> nil` | 递归创建目录 |
| `cblog.files.list(path)` | `string -> table` | 列出目录下的文件/子目录名 |
| `cblog.files.copy(src, dst)` | `(string, string) -> nil` | 复制文件 |
| `cblog.files.append(path, content)` | `(string, string) -> nil` | 追加写入文件 |

**路径安全规则：**
- 禁止绝对路径
- 相对路径拼接到项目根目录
- 自动消除 `..` 等路径遍历
- 最终路径必须位于项目根目录内

### 7.4 `plugin` 全局对象

| API | 签名 | 说明 |
|-----|------|------|
| `plugin.filter(hook_name, priority, handler)` | `(string, int, function) -> nil` | 注册 filter hook |
| `plugin.action(hook_name, priority, handler)` | `(string, int, function) -> nil` | 注册 action hook |
| `plugin.config()` | `() -> table` | 获取当前插件的配置（预加载的 KV 对） |

---

## 八、沙箱安全机制

### 8.1 危险 API 移除

| 被移除的 API | 原因 |
|-------------|------|
| `os.execute` | 禁止执行系统命令 |
| `os.exit` | 禁止退出进程 |
| `io.popen` | 禁止管道操作 |

### 8.2 文件路径沙箱

- `io.open` 被替换为安全版本，所有路径经过 `resolve_path()` 验证
- `resolve_path()` 规则：
  - 禁止绝对路径
  - 相对路径拼接到项目根目录
  - 通过 `canonicalize()` 消除 `..` 路径遍历
  - 最终路径必须位于项目根目录内，否则报错
  - `cblog.files.*` 所有操作也使用相同的 `resolve_path()`

---

## 九、插件配置系统

### 9.1 数据库存储

插件配置存储在 `plugin_store` 表：
```sql
CREATE TABLE IF NOT EXISTS plugin_store (
    plugin_name TEXT NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,     -- JSON 字符串
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (plugin_name, key)
);
```

### 9.2 Rust 端操作（PluginStore）

| 方法 | 说明 |
|------|------|
| `get(db, plugin_name, key)` | 获取单个配置值 |
| `set(db, plugin_name, key, value)` | 设置配置值（UPSERT） |
| `delete(db, plugin_name, key)` | 删除配置 |
| `keys(db, plugin_name)` | 列出所有 key |
| `get_all(db, plugin_name)` | 获取所有 KV 对 |

### 9.3 配置加载流程

1. **CLI `build` 命令**：`load_all_configs_sync()` 同步加载所有启用插件的配置
2. **serve 模式**：`spawn_build()` 异步预取每个启用插件的配置
3. **Lua 端访问**：`plugin.config()` 获取当前插件的配置 table，如无配置则返回空 table

### 9.4 后台配置管理

- **查看配置**：`GET /admin/plugins/{name}` — 显示所有 KV 对，每个 key 一个文本输入框
- **保存配置**：`POST /admin/plugins/{name}/config` — 表单提交，所有字段以 JSON string 值保存

---

## 十、插件路由

### 10.1 固定后台路由

| 方法 | 路径 | Handler | 说明 |
|------|------|---------|------|
| GET | `/admin/plugins` | `plugins::list_plugins` | 插件列表页 |
| POST | `/admin/plugins/toggle` | `plugins::toggle_plugin` | 启用/禁用切换 |
| GET | `/admin/plugins/{name}` | `plugins::plugin_detail` | 插件详情/配置页 |
| POST | `/admin/plugins/{name}/config` | `plugins::save_plugin_config` | 保存插件配置 |
| GET | `/admin/ext/{plugin}/{slug}` | `plugin_admin_page` | 插件自定义后台页面 |

### 10.2 插件自定义后台页面

在 `plugin.toml` 中声明：
```toml
[[admin.pages]]
label = "搜索统计"
slug = "stats"
icon = "search"
```

效果：
1. 后台侧边栏"插件扩展"分组下显示链接
2. URL 为 `/admin/ext/{plugin_name}/{slug}`
3. 模板文件路径为 `plugins/{plugin_name}/admin/{slug}.cbtml`

**渲染流程（`src/admin.rs`）：**
1. 读取 CBTML 模板文件
2. 使用 `cbtml::compile()` 编译为 MiniJinja 模板
3. 创建临时 MiniJinja 环境，注册过滤器
4. 构建渲染上下文，包含 `plugin_name`、`plugin_config`、`site` 信息
5. 渲染插件模板得到 HTML 片段
6. 包裹在 `plugin-page.cbtml` 布局中显示

**自定义后台页面模板可用变量：**
```
plugin_name                 -- 插件名称
plugin_config               -- 插件配置（HashMap 结构）
site.title                  -- 站点标题
site.url                    -- 站点 URL
site.description            -- 站点描述
```

---

## 十一、错误处理和日志

### 11.1 Rust 端

- 所有核心函数返回 `anyhow::Result`
- `plugin.toml` 解析失败：`with_context()` 提供文件路径信息
- Lua VM 初始化/脚本执行失败：转为 `anyhow::anyhow!` 错误
- Hook 执行失败：详细错误信息包含 hook 名称和优先级
- 插件冲突/循环依赖：使用 `bail!` 中止

### 11.2 Lua 端

通过 `cblog.log.*` 输出日志，前缀 `[plugin]`：
```lua
cblog.log.info("插件已初始化")
cblog.log.warn("配置缺失，使用默认值")
cblog.log.error("处理失败: " .. err)
cblog.log.debug("调试信息: " .. tostring(data))
```

### 11.3 完整性检查（`cblog check`）

`check_plugins()` 检查内容：
- 每个启用插件的目录是否存在
- `plugin.toml` 是否存在且可解析
- `main.lua` 是否存在

---

## 十二、现有内置插件详解

### 12.1 hello-world — 示例插件

**plugin.toml：**
```toml
[plugin]
name = "hello-world"
version = "0.1.0"
description = "示例插件：在构建日志中打印问候信息"
```

**main.lua：**
```lua
plugin.action("after_load", 10, function(ctx)
    cblog.log.info("Hello from hello-world plugin!")
    cblog.log.info("共加载 " .. #ctx.posts .. " 篇文章")
end)

plugin.action("after_render", 10, function(ctx)
    cblog.log.debug("hello-world: 输出目录 = " .. (ctx.output_dir or "unknown"))
end)
```

### 12.2 search — 搜索索引生成

**plugin.toml：**
```toml
[plugin]
name = "search"
version = "0.1.0"
description = "生成搜索索引文件"

[capabilities]
reads = ["post.title", "post.content", "post.tags"]
generates = ["public/search-index.json"]
```

**main.lua 核心逻辑：**
```lua
plugin.action("after_finalize", 10, function(ctx)
    local entries = {}
    for _, post in ipairs(ctx.posts) do
        local plain = cblog.strip_html(post.content)
        if #plain > 500 then
            plain = plain:sub(1, 500)
        end
        table.insert(entries, {
            id = post.id,
            title = post.title,
            url = post.url,
            content = plain,
            tags = post.tags,
            date = post.created_at
        })
    end
    local json = cblog.json(entries)
    cblog.files.write(ctx.output_dir .. "/search-index.json", json)
    cblog.log.info("搜索索引已生成，共 " .. #entries .. " 篇文章")
end)
```

### 12.3 seo-optimizer — SEO 文件生成

```lua
plugin.action("after_finalize", 20, function(ctx)
    local site_url = ctx.site_url or ""
    local robots = "User-agent: *\nAllow: /\nDisallow: /admin/\n\n"
    robots = robots .. "Sitemap: " .. site_url .. "/sitemap.xml\n"
    cblog.files.write(ctx.output_dir .. "/robots.txt", robots)
    cblog.log.info("robots.txt 已生成")
end)
```

### 12.4 image-optimize — 图片懒加载

```lua
plugin.action("after_render", 10, function(ctx)
    local output = ctx.output_dir or "public"
    local count = 0
    -- 遍历 HTML 文件，为 <img> 标签添加 loading="lazy"
    local items = cblog.files.list(output)
    for _, name in ipairs(items) do
        local path = output .. "/" .. name
        if name:match("%.html$") then
            local html = cblog.files.read(path)
            local modified = html:gsub('<img([^>]-)>', function(attrs)
                if not attrs:match('loading=') then
                    count = count + 1
                    return '<img loading="lazy"' .. attrs .. '>'
                end
                return '<img' .. attrs .. '>'
            end)
            if modified ~= html then
                cblog.files.write(path, modified)
            end
        end
    end
    cblog.log.info("图片懒加载：已处理 " .. count .. " 个标签")
end)
```

### 12.5 toc — 目录平滑滚动

```lua
plugin.action("after_render", 20, function(ctx)
    local output = ctx.output_dir or "public"
    -- 为包含 toc-list class 的 HTML 注入平滑滚动 CSS
    local css = '<style>html{scroll-behavior:smooth}.toc-list a{...}</style>'
    -- 在 </head> 前注入
end)
```

### 12.6 syntax-highlight — 代码高亮样式

```lua
plugin.action("after_render", 15, function(ctx)
    local output = ctx.output_dir or "public"
    -- 为包含 code-highlight class 的 HTML 注入 One Dark 主题 CSS
    -- 在 </head> 前注入
end)
```

---

## 十三、完整插件开发示例

### 13.1 最小插件

**plugins/my-plugin/plugin.toml：**
```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
author = "Developer"
description = "一个最小化的 cblog 插件"
```

**plugins/my-plugin/main.lua：**
```lua
plugin.action("after_load", 10, function(ctx)
    cblog.log.info("my-plugin: 共加载 " .. #ctx.posts .. " 篇文章")
end)
```

### 13.2 带配置的插件

**plugins/word-counter/plugin.toml：**
```toml
[plugin]
name = "word-counter"
version = "1.0.0"
description = "统计所有文章的总字数并生成报告"

[capabilities]
reads = ["post.content", "post.word_count"]
generates = ["public/stats.json"]
```

**plugins/word-counter/main.lua：**
```lua
plugin.action("after_finalize", 10, function(ctx)
    local config = plugin.config()
    local min_words = tonumber(config.min_words) or 0

    local total_words = 0
    local post_count = 0
    for _, post in ipairs(ctx.posts) do
        if post.word_count >= min_words then
            total_words = total_words + post.word_count
            post_count = post_count + 1
        end
    end

    local stats = {
        total_words = total_words,
        total_posts = post_count,
        avg_words = post_count > 0 and math.floor(total_words / post_count) or 0
    }

    cblog.files.write(ctx.output_dir .. "/stats.json", cblog.json(stats))
    cblog.log.info("字数统计：总计 " .. total_words .. " 字，" .. post_count .. " 篇文章")
end)
```

### 13.3 带自定义后台页面的插件

**plugins/analytics/plugin.toml：**
```toml
[plugin]
name = "analytics"
version = "1.0.0"
description = "站点统计分析"

[capabilities]
reads = ["post.title", "post.word_count", "post.created_at"]

[[admin.pages]]
label = "统计面板"
slug = "dashboard"
icon = "chart"
```

**plugins/analytics/main.lua：**
```lua
plugin.action("after_finalize", 10, function(ctx)
    -- 统计数据并存储为 JSON
    local stats = {
        total_posts = #ctx.posts,
        total_words = 0
    }
    for _, post in ipairs(ctx.posts) do
        stats.total_words = stats.total_words + (post.word_count or 0)
    end
    cblog.files.write(ctx.output_dir .. "/analytics.json", cblog.json(stats))
end)
```

**plugins/analytics/admin/dashboard.cbtml：**
```
div.analytics-dashboard
  h2 站点统计
  div.stats-grid
    div.stat-card
      h3 插件名称
      p {{ plugin_name }}
    div.stat-card
      h3 配置项数
      p {{ plugin_config | length }}

  if plugin_config.api_key
    div.config-info
      p API Key 已配置
  else
    div.config-warning
      p 请在插件设置中配置 API Key
  end

  style
    .analytics-dashboard { padding: 20px; }
    .stats-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; }
    .stat-card { background: var(--card-bg); padding: 16px; border-radius: 8px; }
```

### 13.4 使用 lib/ 目录的插件

**plugins/my-plugin/lib/utils.lua：**
```lua
local M = {}

function M.truncate_text(text, max_len)
    if #text <= max_len then
        return text
    end
    return text:sub(1, max_len) .. "..."
end

function M.count_words(text)
    local count = 0
    for _ in text:gmatch("%S+") do
        count = count + 1
    end
    return count
end

return M
```

**plugins/my-plugin/main.lua：**
```lua
local utils = require("utils")

plugin.action("after_load", 10, function(ctx)
    for _, post in ipairs(ctx.posts) do
        local plain = cblog.strip_html(post.content)
        local preview = utils.truncate_text(plain, 200)
        cblog.log.debug("文章 " .. post.title .. ": " .. preview)
    end
end)
```

### 13.5 HTML 注入模式（常见模式）

许多插件需要在生成的 HTML 中注入 CSS/JS，标准做法是在 `after_render` hook 中处理：

```lua
plugin.action("after_render", 15, function(ctx)
    local output = ctx.output_dir or "public"

    -- 要注入的 CSS
    local css = [[<style>
        .my-feature { color: red; }
    </style>]]

    -- 要注入的 JS
    local js = [[<script>
        console.log('my plugin loaded');
    </script>]]

    -- 遍历所有 HTML 文件
    local items = cblog.files.list(output)
    for _, name in ipairs(items) do
        if name:match("%.html$") then
            local path = output .. "/" .. name
            local html = cblog.files.read(path)

            -- 检查是否需要注入（例如只对包含特定 class 的页面注入）
            if html:match('class="my%-target"') then
                -- 在 </head> 前注入 CSS
                html = html:gsub("</head>", css .. "\n</head>")
                -- 在 </body> 前注入 JS
                html = html:gsub("</body>", js .. "\n</body>")
                cblog.files.write(path, html)
            end
        end
    end
end)
```

**注意**：当前内置插件只遍历一级子目录。如需递归遍历，可以自行编写递归函数。

### 13.6 文件生成模式（常见模式）

在 `after_finalize` hook 中生成额外文件到输出目录：

```lua
plugin.action("after_finalize", 10, function(ctx)
    -- 生成 JSON 数据文件
    local data = { posts = {} }
    for _, post in ipairs(ctx.posts) do
        table.insert(data.posts, {
            title = post.title,
            url = post.url,
            date = post.created_at
        })
    end
    cblog.files.write(ctx.output_dir .. "/api/posts.json", cblog.json(data))

    -- 生成文本文件
    cblog.files.write(ctx.output_dir .. "/manifest.txt", "cblog site manifest\n")

    cblog.log.info("额外文件已生成")
end)
```

---

## 十四、后台管理功能

### 14.1 插件列表页（`GET /admin/plugins`）

- 扫描 `plugins/` 目录下所有可用插件
- 显示每个插件的名称、版本、描述、启用状态
- 提供启用/禁用按钮和"设置"链接

### 14.2 插件启用/禁用（`POST /admin/plugins/toggle`）

- 读取 `cblog.toml` 文件
- 切换指定插件的启用状态
- 直接修改 `[plugins]` 段的 `enabled` 行
- 如果文件中没有 `[plugins]` 段则自动添加

### 14.3 插件详情页（`GET /admin/plugins/{name}`）

- 显示插件完整元信息（名称、版本、描述）
- 显示能力声明（Reads/Writes/Generates）
- 显示依赖关系（After/Conflicts）
- 显示并编辑插件配置（KV 对文本输入框）

### 14.4 插件自定义后台页面

- URL：`/admin/ext/{plugin}/{slug}`
- 模板：`plugins/{plugin}/admin/{slug}.cbtml`
- 在后台侧边栏"插件扩展"分组中显示
- 渲染上下文包含 `plugin_name`、`plugin_config`、`site`

---

## 十五、CBTML 模板语法（插件后台页面使用）

插件的自定义后台页面模板使用 CBTML 语法，与主题模板语法完全一致。参见 cblog-dev-theme skill 中的 CBTML 语法部分。

要点：
- 2 空格缩进表示层级
- `tag.class#id [attr="val"] 内联文本` 声明元素
- `{{ expr }}` 输出变量
- `raw expr` 不转义输出
- `if`/`else if`/`else`/`end` 条件
- `for x in y`/`end` 循环
- `style`/`script` 原生块
- 插件后台页面会被包裹在 `plugin-page.cbtml` 布局中

---

## 十六、开发建议

1. **从 hello-world 开始**：复制 `plugins/hello-world/` 作为起点修改
2. **使用正确的 hook**：
   - 需要读取文章数据 → `after_load`
   - 需要修改生成的 HTML → `after_render`
   - 需要生成额外文件 → `after_finalize`
   - 需要处理资源 → `after_assets`
3. **注意优先级**：多个插件可能注册同一个 hook，合理设置 priority 避免冲突
4. **善用 `cblog.log`**：开发调试时多用 `cblog.log.debug()`
5. **路径安全**：所有文件操作使用相对路径，不要尝试访问项目目录外的文件
6. **声明 capabilities**：虽然不强制校验，但良好的 capabilities 声明帮助用户理解插件行为
7. **配置通过后台管理**：通过 `/admin/plugins/{name}` 页面设置 KV 对，Lua 中用 `plugin.config()` 读取
8. **测试**：使用 `cblog check` 验证插件配置完整性，使用 `cblog build` 验证插件运行
9. **遍历深度**：`cblog.files.list()` 只列出一级内容，需要递归时自行编写递归函数
10. **Lua 中无异步**：所有 Lua 脚本同步执行，不支持异步操作

---

## 十七、当前插件系统的限制

1. **仅构建期 hook**：当前所有 hook 仅在构建流水线中触发，没有请求级 hook
2. **无异步支持**：Lua 脚本同步执行，不支持 HTTP 请求等异步操作
3. **配置编辑简单**：后台配置页只提供文本输入框，不支持复杂类型
4. **无远程安装**：只有启用/禁用，不支持远程安装或自动卸载
5. **filter hook 未在管道中使用**：构建管道只调用了 `call_action`，`apply_filter` 和 `has_handlers` 已实现但尚无调用点。插件应使用 `plugin.action()` 注册 hook，使用 `plugin.filter()` 注册的 handler 不会在构建管道中被触发
6. **capabilities 仅声明式**：reads/writes/generates 不做强制校验

---

## 十八、关键依赖版本

| 依赖 | 版本 | 用途 |
|------|------|------|
| `mlua` | 0.11 | Lua 5.4 运行时 (features: lua54, serde, vendored) |
| `toml` | 0.8 | 解析 plugin.toml |
| `serde_json` | 1 | 插件配置序列化 |
| `sqlx` | 0.8 | plugin_store 数据库操作 |
| `minijinja` | 2 | 插件后台页面模板渲染 |
| `syntect` | 5 | cblog.highlight() 代码高亮 |
| `anyhow` | 1 | 错误处理 |
| `tracing` | 0.1 | 日志输出 |
