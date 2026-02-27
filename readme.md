# cblog

Rust + Lua 驱动的现代静态博客引擎，内置后台管理面板、主题系统、插件与 Lua Hook 扩展，开箱即用、可深度定制。

---

## 功能概览

- **静态站点生成**：  
  - 从 Markdown 内容与配置生成纯静态 HTML，用于部署到任意静态托管（GitHub Pages、Vercel、COS 等）。  
- **后台管理面板（Admin）**：  
  - 基于 `axum` 的管理后台，支持文章 / 页面 / 媒体 / 主题 / 插件 / 构建管理。  
- **Aurora 默认主题**：  
  - 简洁优雅，支持颜色、字体、深色模式、阅读时间、目录等配置，所有配置可在后台实时调整。  
- **可配置构建管线**：  
  - 多阶段构建（加载 → 渲染 → 生成 → 归档 → 资产处理 → 最终输出），支持增量构建与缓存。  
- **插件系统**：  
  - 插件可注册后台页面（`/admin/ext/{plugin}/{slug}`），配合 CBTML 模板与 Lua 实现扩展逻辑。  
- **模板语言 CBTML**：  
  - 自研模板语言，编译为 MiniJinja 模板（`src/cbtml/*`），支持过滤器与代码生成。  
- **内容模型**：  
  - 文章 / 页面统一为 Markdown 内容 + 元信息（标签、分类、封面、阅读时间、目录等）。  
- **媒体与图片处理**：  
  - 上传媒体到 `media` 目录，可自动生成 WebP、缩略图，限制文件类型与大小。  
- **配置与多环境支持**：  
  - TOML 配置 `cblog.toml`，包含站点信息、路由、构建、Feed、Sitemap、认证、媒体、插件等。  

---

## 架构概览

- **CLI 入口**：`src/main.rs`  
  - 命令：`build` / `serve` / `check`（默认 `serve`）。  
  - 根据项目根目录加载 `cblog.toml`，设置日志级别并调度后续逻辑。  
- **配置加载**：`src/config.rs`  
  - 解析 `cblog.toml` 到 `SiteConfig`。  
  - 提供完善的默认值（语言、时区、输出目录、分页规则、Feed/Sitemap、媒体、JWT 等）。  
- **初始化逻辑**：`src/init.rs`  
  - 当项目根目录缺少 `cblog.toml` 时自动初始化：  
    - 创建 `content/posts` / `content/pages` / `themes/aurora` / `plugins` / `media` 等目录。  
    - 写入默认 `cblog.toml` 与 Aurora 主题的模板 / 资源文件。  
- **应用状态与数据库**：`src/state.rs`  
  - 使用 SQLite (`cblog.db`) 存储用户、内容、构建历史、插件配置、主题配置等。  
  - 通过 `sqlx::migrate!("./migrations")` 自动执行数据库迁移。  
- **构建管线**：`src/build.rs` 与 `src/build/*`  
  - 主要流程：  
    - `stages::load`：加载数据库中的内容记录。  
    - `stages::render`：渲染 Markdown 与 CBTML 模板。  
    - `stages::generate`：生成页面、列表、归档、RSS/Atom、Sitemap 等。  
    - `stages::assets`：拷贝 / 处理静态资源。  
    - `stages::finalize`：写入输出目录。  
  - 支持 `--clean` 清理输出与缓存。  
- **后台管理**：`src/admin.rs` 与 `src/admin/*`  
  - 路由基于 `axum`：  
    - 登录 / 登出 / 健康检查。  
    - 文章 / 页面 / 媒体 / 构建 / 主题 / 插件管理。  
    - 插件自定义后台页面（通过 CBTML + MiniJinja 渲染）。  
  - 使用会话 + JWT 做登录态管理与速率限制。  
  - 首次启动时自动创建默认管理员账号 `admin/admin`。  
- **内容与 Markdown**：`src/content.rs`  
  - `Post` / `Page` / `PostRef` / `TaxonomyIndex` 等数据结构。  
  - 通过 `MarkdownContent` 延迟渲染 Markdown 为 HTML。  
- **模板语言 CBTML**：`src/cbtml.rs` 与子模块  
  - 词法分析 → 语法分析 → 代码生成 → MiniJinja 模板。  
  - 提供自定义过滤器与错误处理。  
- **主题系统**：`src/theme/config.rs` + `themes/aurora/*`  
  - 主题使用 `theme.toml` 描述元信息与配置 Schema（`[[config]]`）。  
  - 支持主题继承与配置合并；可将配置转换为 SCSS 变量写入覆盖文件，实现可视化换肤。  
- **Lua 扩展**：`src/lua/*`  
  - 提供 Lua 沙箱运行环境与 Hook 机制，可在构建 / 渲染周期中注入自定义逻辑。  
- **插件系统**：`src/plugin/*`  
  - 解析 `plugin.toml`，加载插件元信息与后台页面配置。  
  - 通过 SQLite 存储和读取插件配置。  

---

## 快速开始

### 1. 环境要求

- **Rust 工具链**：建议使用最新 stable 版本（可通过 `rustup` 安装）。  
- **SQLite3**：无需额外服务端，仅使用文件数据库。  

### 2. 构建二进制

在仓库根目录执行：

```bash
cargo build --release
```

生成的可执行文件通常位于：

```bash
target/release/cblog
```

你也可以通过 `cargo install --path .` 将其安装到全局 `cargo bin` 目录：

```bash
cargo install --path .
```

---

### 3. 初始化一个新站点

在你想创建博客的目录下执行（假设使用当前目录）：  

```bash
cblog serve --root .
```

首次运行时，如果没有 `cblog.toml`，程序会：

- 写入默认 `cblog.toml`。  
- 创建目录：`content/posts`、`content/pages`、`themes/aurora/*`、`plugins`、`media`。  
- 准备 Aurora 默认主题模板与资源文件。  

此后你可以直接访问后台管理页面开始写作与配置主题。

---

### 4. 启动后台管理面板

默认监听地址与端口由 `cblog.toml` 中的 `[server]` 配置控制（默认 `127.0.0.1:3000`）：

```bash
cblog serve --root .
```

可选参数：

- `--host`：覆盖配置中的监听地址，例如 `0.0.0.0`。  
- `--port`：覆盖配置中的端口，例如 `8080`。  

启动后访问：

```text
http://127.0.0.1:3000/admin
```

默认管理员账号：

- 用户名：`admin`  
- 密码：`admin`  

**强烈建议** 登录后立即前往个人资料页面修改密码。

---

### 5. 写作与内容组织

- **文章（posts）**：  
  - 通过后台 “文章管理” 页面新建 / 编辑 / 发布。  
  - 文章正文使用 Markdown，支持标题、列表、代码块等常见语法。  
  - 可在编辑界面设置封面、标签、分类、自定义元信息等。  
- **页面（pages）**：  
  - 用于“关于我”、“友链”等独立页面。  
  - 也使用 Markdown，支持选择不同模板。  
- **草稿 / 发布 / 归档**：  
  - 通过 `status` 管理文章生命周期，在构建时只输出已发布内容。  

如果你更偏向 Git 驱动的工作流，也可以把后台当作管理界面，在 CI 中调用 `cblog build` 生成静态文件。

---

### 6. 静态构建与部署

手动触发构建：

```bash
cblog build --root . --clean
```

参数说明：

- `--root`：项目根目录（默认 `.`）。  
- `--clean`：构建前清空输出目录与缓存目录。  

构建完成后，静态站点会输出到 `cblog.toml` 中 `[build]` 的 `output_dir`（默认 `public`）：  

```text
public/
  index.html
  posts/...
  tags/...
  category/...
  archive/...
  assets/...
```

你可以将 `public/` 目录直接部署到任意静态托管服务。

---

## 配置说明（cblog.toml）

项目根目录下的 `cblog.toml` 是全局配置文件，初始化后大致形如：

```toml
[site]
title = "My Blog"
subtitle = ""
description = ""
url = "https://example.com"
language = "zh-CN"
timezone = "Asia/Shanghai"

[site.author]
name = ""
email = ""

[build]
output_dir = "public"

[theme]
active = "aurora"
```

更多字段在 `src/config.rs` 中定义，这里只列出常用小结：

- **`[site]` 与 `[site.author]`**：  
  - 站点标题、副标题、描述、URL、语言、时区、作者信息。  
- **`[build]`**：  
  - `output_dir`：静态输出目录。  
  - `cache_dir`：构建缓存目录（默认 `.cblog-cache`）。  
  - `posts_per_page`：首页与列表分页大小。  
  - `date_format`：日期展示格式。  
  - `excerpt_length`：自动摘要长度。  
- **`[routes]`**：  
  - 自定义文章 / 标签 / 分类 / 归档的 URL 模式。  
- **`[server]`**：  
  - `host` / `port` / `log_level`。  
- **`[feed]` / `[sitemap]`**：  
  - 是否启用 RSS/Atom 与 Sitemap、更新频率与权重。  
- **`[auth]`**：  
  - JWT 密钥与过期时间、Session 名称（生产环境请务必修改默认密钥）。  
- **`[media]`**：  
  - 上传目录、最大文件大小、允许 MIME 类型、是否自动生成 WebP 与缩略图等。  
- **`[plugins]`**：  
  - `enabled = ["foo", "bar"]` 用于启用插件。  

---

## 主题系统

Aurora 默认主题位于：

```text
themes/aurora/
  theme.toml
  templates/*.cbtml
  templates/partials/*.cbtml
  assets/scss/*.scss
  assets/js/main.js
```

`theme.toml` 示例（节选）：

```toml
[theme]
name        = "Aurora"
version     = "1.0.0"
author      = "cblog"
description = "cblog 默认主题，简洁优雅"
license     = "MIT"

[[config]]
key     = "primary_color"
type    = "color"
label   = "主色调"
default = "#6366f1"
group   = "外观"
```

- **`[theme]`**：主题元信息，用于展示在后台与主题管理。  
- **`[[config]]`**：配置 Schema，用于自动生成后台表单：  
  - `key`：配置键名，对应 SCSS 变量等。  
  - `type`：字段类型，如 `color` / `font_select` / `select` / `number` / `boolean` / `code` / `richtext` 等。  
  - `label`：后台显示名称。  
  - `default`：默认值，可被用户配置覆盖。  
  - `group`：分组（外观 / 布局 / 功能 / 高级等）。  
  - `options` / `min` / `max` / `description`：用于渲染选择器和校验范围。  

主题配置值会通过 `build_scss_overrides` 转成 SCSS 变量，从而影响最终主题样式。

你可以在 Aurora 的基础上复制目录创建自定义主题，并在 `cblog.toml` 中修改 `[theme].active` 指向新的主题名。

---

## 插件与 Lua 扩展（简要）

- **插件目录**：  
  - 插件位于项目根目录 `plugins/` 下，每个插件一个子目录，包含 `plugin.toml`、Lua 脚本、CBTML 模板等。  
- **后台页面**：  
  - 插件可以通过 `plugin.toml` 声明后台侧边栏项，最终渲染成 `/admin/ext/{plugin}/{slug}` 路由。  
  - 后台页面使用 CBTML 模板，运行时由 `minijinja` 渲染。  
- **Lua Hook**：  
  - 在构建过程的关键阶段，Lua 代码可以通过 Hook 介入，实现自定义数据加工与内容生成逻辑。  

具体插件与 Lua API 请参考后续文档或源码（`src/plugin/*`、`src/lua/*`）。

---

## CLI 使用速查

```bash
# 启动后台管理 + 静态站点服务（默认命令）
cblog serve --root . [--host 0.0.0.0] [--port 3000]

# 构建静态站点
cblog build --root . [--clean]

# 检查配置与内容完整性
cblog check --root .
```

- **`serve`**：  
  - 初始化项目（如未初始化），然后启动后台服务与静态文件服务。  
- **`build`**：  
  - 可选 `--clean` 清除输出与缓存目录后再构建。  
- **`check`**：  
  - 扫描并输出警告与错误，适合作为 CI 中的预检步骤。  

---

## 开发与贡献

- **主要依赖技术栈**：  
  - 语言：Rust  
  - Web 框架：`axum`  
  - 异步运行时：`tokio`  
  - 数据库层：`sqlx` + SQLite  
  - 模板引擎：`MiniJinja` + 自定义 CBTML 编译器  
  - CLI：`clap`  
  - 日志：`tracing` / `tracing-subscriber`  
  - 密码哈希：`argon2`  
- **数据库迁移**：  
  - 迁移文件位于 `./migrations`，由 `sqlx::migrate!` 在第一次访问时自动执行。  

欢迎通过 Issue / PR 参与改进，或基于本项目开发自己的主题与插件。

---

## 许可证

请查看仓库根目录下的 `LICENSE` 文件（若存在），其中包含本项目的版权与许可条款。Aurora 默认主题在 `theme.toml` 中声明使用 MIT License。

---

## English (Brief Overview)

### What is cblog?

`cblog` is a modern static blog engine written in Rust with Lua scripting, featuring:

- A built-in admin dashboard for managing posts, pages, media, themes and plugins.  
- A fully static site generator suitable for any static hosting provider.  
- A flexible theme system (Aurora by default) with a declarative `theme.toml` schema.  
- A custom CBTML templating language compiled to MiniJinja.  
- Plugin and Lua hooks for advanced customization.  

---

### Architecture Highlights

- **CLI entry** (`src/main.rs`): commands `serve`, `build`, `check`.  
- **Configuration** (`src/config.rs`): loads `cblog.toml` into a strongly typed `SiteConfig`.  
- **Initialization** (`src/init.rs`): creates a project skeleton and the Aurora theme on first run.  
- **App state & DB** (`src/state.rs`): uses SQLite plus `sqlx` migrations.  
- **Build pipeline** (`src/build/*`): multi-stage static site generation with optional cleaning and caching.  
- **Admin panel** (`src/admin/*`): `axum`-based web UI at `/admin` with authentication and rate limiting.  
- **Content model** (`src/content.rs`): Markdown-backed posts/pages with taxonomy and reading stats.  
- **Templating** (`src/cbtml/*`): CBTML compiler to MiniJinja templates and custom filters.  
- **Themes** (`src/theme/config.rs`, `themes/aurora/*`): TOML-defined theme metadata and config schema.  
- **Lua & plugins** (`src/lua/*`, `src/plugin/*`): sandboxed Lua runtime and plugin registry/store.  

---

### Quick Start

1. **Build the binary**:

```bash
cargo build --release
```

2. **Initialize and run a site** (in your project directory):

```bash
cblog serve --root .
```

On first run this will:

- Create a default `cblog.toml`.  
- Scaffold content directories (`content/posts`, `content/pages`, etc.).  
- Install the bundled Aurora theme.  

3. **Admin login**:

- Visit `http://127.0.0.1:3000/admin`.  
- Default credentials: `admin` / `admin` (please change the password immediately).  

4. **Static build**:

```bash
cblog build --root . --clean
```

The generated static site is written to the `public` directory by default.

---

### Configuration & Themes

- Global configuration lives in `cblog.toml` and includes site metadata, routing, build options, feeds, sitemap, auth, media and enabled plugins.  
- Themes live under `themes/<name>/` with a `theme.toml` describing metadata and a list of `[[config]]` fields that power the admin theme settings UI.  
- Aurora ships as the default theme and can be used as a reference for building your own themes.  

---

### Development Notes

- Tech stack: Rust, `axum`, `tokio`, `sqlx` + SQLite, MiniJinja, CBTML, `clap`, `tracing`, `argon2`.  
- Database migrations are run automatically via `sqlx::migrate!("./migrations")`.  
- Contributions and feedback are welcome via issues and pull requests.  

