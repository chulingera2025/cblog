# cblog 项目设计文档

> 一个基于 Rust + Lua 的博客引擎：后台动态（`/admin`），前台全静态 SSG。  
> 目标是达到 WordPress 级别的主题/插件拓展性，同时前台零运行时开销。  
> 模板使用 cblog 专属的声明式语言 **cbtml**，编译为 HTML 输出。

---

## 目录

1. [项目概览](#1-项目概览)
2. [整体架构](#2-整体架构)
3. [技术选型](#3-技术选型)
4. [目录结构](#4-目录结构)
5. [站点配置（cblog.toml）](#5-站点配置-cblogtoml)
6. [cbtml 模板语言](#6-cbtml-模板语言)
7. [内容模型](#7-内容模型)
8. [构建管道](#8-构建管道)
9. [主题系统](#9-主题系统)
10. [插件系统](#10-插件系统)
11. [后台管理 /admin](#11-后台管理-admin)
12. [认证与安全](#12-认证与安全)
13. [媒体文件管理](#13-媒体文件管理)
14. [Lua 运行时与 API](#14-lua-运行时与-api)
15. [前台静态能力边界](#15-前台静态能力边界)
16. [数据库设计](#16-数据库设计)
17. [错误处理与日志](#17-错误处理与日志)
18. [部署](#18-部署)
19. [备份与恢复](#19-备份与恢复)
20. [开发路线图](#20-开发路线图)

---

## 1. 项目概览

### 1.1 核心思路

WordPress 的根本问题是**每次读请求都要重走全套 PHP 执行流程**，而博客内容变更频率极低。cblog 把这个问题翻转：

- **写操作**（发文、改配置）在 `/admin` 动态处理，写完触发增量构建
- **读操作**（所有访客页面）全部是构建期生成的静态 HTML，由 Nginx/CDN 直接伺服，零应用层参与
- **拓展性**不牺牲——Lua 脚本在构建期实现与 WordPress 等价的 Hook 能力，`/admin` 动态路由让主题和插件都能注册自己的后台页面

### 1.2 设计原则

| 原则 | 说明 |
|------|------|
| 构建期优先 | 能在构建期确定的事绝不留到运行时 |
| 拓展点显式化 | 所有 Hook 点有明确阶段、类型和优先级，不是全局隐式状态 |
| 插件/主题隔离 | 各自独立的 KV 存储、受限 IO、显式能力声明 |
| 主题零代码配置 | theme.toml 声明配置 schema，后台自动生成表单，主题作者不写后台代码 |
| 增量构建 | 基于内容哈希和模板依赖图，只重建真正受影响的页面 |
| 模板简洁 | cbtml 声明式语法，缩进即结构，无闭合标签噪音 |
| 单二进制部署 | 一个 `cblog` 可执行文件，SQLite 数据库，无外部服务依赖 |

### 1.3 适用场景

cblog 针对以下场景设计，不是通用 CMS：

- 个人博客、技术博客、团队博客
- 内容更新频率低（发文 → 触发构建 → 访客访问静态文件）
- 需要高度定制主题和功能，但不想接受 WordPress 的性能开销
- 希望内容用 Markdown 管理，同时有图形化后台

不适合：电商、需要用户注册/登录的社区、内容秒级更新的场景。

---

## 2. 整体架构

### 2.1 系统架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                       浏览器 / 管理员                             │
└────────────────┬────────────────────────┬────────────────────────┘
                 │ /admin/*               │ /* (所有其他路径)
                 ▼                        ▼
┌───────────────────────┐    ┌──────────────────────────────────────┐
│    Axum 动态服务       │    │       Nginx 伺服静态文件              │
│  （持续运行进程）       │    │                                      │
│  · 文章/页面 CRUD      │    │   public/                            │
│  · 主题配置管理        │    │   ├── index.html                     │
│  · 插件/主题后台页面   │    │   ├── posts/hello-world/index.html   │
│  · 媒体上传           │    │   ├── tags/rust/index.html           │
│  · 构建触发 & 状态 WS  │    │   ├── archive/2024/index.html        │
│  · JWT 认证           │    │   ├── assets/（主题/插件资源）         │
│  · CSRF 防护          │    │   ├── media/（上传的图片等）           │
└──────────┬────────────┘    │   ├── sitemap.xml                    │
           │ 内容变更后       │   ├── feed.xml                       │
           │ 发送构建任务     │   └── search-index.json              │
           ▼                 └──────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                   Build Pipeline（Rust Core）                     │
│                                                                  │
│  content.load → content.parse → content.transform               │
│  → taxonomy.build → page.generate → page.render                 │
│  → asset.process → build.finalize                               │
│                                                                  │
│  · SHA-256 内容哈希增量构建                                       │
│  · 模板依赖图（模板变更自动找到所有受影响页面）                    │
│  · rayon 并行渲染（CPU 核心数并发）                               │
│  · 构建进度通过 channel 实时广播到 WebSocket                      │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│                cbtml 编译器 + Lua 运行时（mlua）                  │
│                                                                  │
│  cbtml → Lexer → AST → CodeGen → MiniJinja 求值 → HTML          │
│                                                                  │
│  Lua 环境：全标准库开放，仅禁 os.execute / io.popen / os.exit    │
│  IO 路径限制在 cblog 项目根目录（cblog.toml 所在位置）            │
│  Rust ↔ Lua：serde_json 透明序列化，零手写绑定                   │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 请求生命周期

**访客读取文章：**
```
访客 → Nginx → public/posts/my-post/index.html（直接返回，0 应用层参与）
```

**管理员发布文章：**
```
管理员 POST /admin/posts
  → JWT 验证
  → 写入 SQLite
  → 发送 BuildTask::PostChanged(id) 到构建队列
  → 返回 200（不等待构建完成）

构建队列（异步）：
  → 分析变更范围（哪些页面需要重建）
  → 并行重建受影响页面
  → 写入 public/ 目录
  → 通过 WebSocket 广播构建完成事件
```

**管理员修改主题配置：**
```
管理员 POST /admin/theme
  → 校验配置值（按 theme.toml schema）
  → 写入 theme_config 表
  → 发送 BuildTask::FullRebuild 到构建队列
  → 重建全部页面（配置变更影响所有页面）
```

---

## 3. 技术选型

### 3.1 依赖总览

| 层次 | Crate / 工具 | 版本约束 | 理由 |
|------|-------------|---------|------|
| Web 框架 | `axum` | `^0.7` | 异步、Tower 中间件生态、类型安全路由 |
| 异步运行时 | `tokio` | `^1` | full features，驱动 Axum 和异步 IO |
| 序列化 | `serde` + `serde_json` | `^1` | 全栈数据流转 |
| 数据库 | `sqlx` | `^0.7` | 编译期 SQL 检查，SQLite driver |
| Lua 嵌入 | `mlua` | `^0.9` | features = ["lua54", "vendored", "serialize"] |
| Markdown | `pulldown-cmark` | `^0.11` | 纯 Rust，AST 可遍历，速度快 |
| Front Matter | `gray_matter` | `^0.2` | YAML/TOML front matter 解析 |
| 模板引擎（内部） | `minijinja` | `^1` | cbtml 编译目标，不直接暴露给主题作者 |
| 并行 | `rayon` | `^1` | 页面并行构建 |
| ID 生成 | `ulid` | `^1` | 时间排序、URL 安全 |
| 时间处理 | `chrono` | `^0.4` | 日期格式化、时区 |
| 哈希 | `sha2` | `^0.10` | 内容哈希，增量构建 |
| 配置解析 | `toml` | `^0.8` | cblog.toml / theme.toml / plugin.toml |
| 认证 | `jsonwebtoken` | `^9` | JWT 签发与验证 |
| 密码哈希 | `argon2` | `^0.5` | 管理员密码存储 |
| CSRF | `tower-sessions` | `^0.12` | CSRF token 管理 |
| 日志 | `tracing` + `tracing-subscriber` | `^0.1` | 结构化日志 |
| 错误处理 | `anyhow` | `^1` | 应用层错误链 |
| 文件监听 | `notify` | `^6` | 开发模式下监听文件变更 |
| 图片处理 | `image` | `^0.25` | 上传图片压缩/转码 WebP |
| 资源打包 | `grass` | `^0.13` | SCSS → CSS 编译（主题资源管道） |

### 3.2 为什么不用现有模板引擎

| 方案 | 问题 |
|------|------|
| MiniJinja / Tera | HTML 噪音重，大量闭合标签，主题可读性差 |
| Pug / Slim / Haml | 语法理想，但 Rust 无成熟实现，需引入 Node.js |
| Maud（Rust 宏） | 编译期类型安全，但主题必须用 Rust 写，热重载不可能 |
| Askama | 编译期模板，不支持运行时动态加载，无法热切换主题 |

cbtml 的定位：**Pug 的缩进结构语法 × Jinja2 的逻辑表达能力**，纯 Rust 实现，运行时动态加载，支持主题热切换。

### 3.3 为什么选 Lua 而不是其他脚本语言

| 语言 | 嵌入成本 | 路径沙箱 | 性能 | 生态 | 结论 |
|------|---------|---------|------|------|------|
| Lua 5.4（mlua） | 极低 | 可精确控制 | 极高 | 成熟 | ✅ 选用 |
| JavaScript（QuickJS） | 中 | 弱 | 高 | 极强 | 体积大，与 Rust 类型系统整合差 |
| Python | 高 | 极弱 | 低 | 极强 | 嵌入复杂，沙箱几乎做不到 |
| Rhai（纯 Rust） | 极低 | 极强 | 中 | 几乎无 | 无现成插件生态，语法陌生 |
| WebAssembly | 中 | 极强 | 极高 | 中 | 插件开发门槛高，工具链复杂 |

---

## 4. 目录结构

```
cblog/                               ← 项目根目录（cblog.toml 所在处）
│
├── cblog.toml                       # 站点全局配置
├── Cargo.toml
├── Cargo.lock
│
├── content/                         # 用户内容（Markdown 原始文件）
│   ├── posts/                       # 博客文章
│   │   ├── 2024-01-15-hello-world.md
│   │   └── 2024-03-20-rust-tips.md
│   └── pages/                       # 独立页面（不参与分页/归档）
│       ├── about.md
│       └── projects.md
│
├── media/                           # 上传的媒体文件（同步到 public/media/）
│   ├── 2024/
│   │   └── 01/
│   │       └── cover.webp
│   └── ...
│
├── themes/                          # 主题目录
│   └── aurora/                      # 主题名
│       ├── theme.toml               # 主题元数据 + 配置 schema
│       ├── hooks.lua                # 主题级 Lua Hook 与后台页面注册
│       ├── templates/               # cbtml 模板文件
│       │   ├── base.cbtml           # 基础布局
│       │   ├── index.cbtml          # 首页
│       │   ├── post.cbtml           # 文章页
│       │   ├── page.cbtml           # 独立页面
│       │   ├── archive.cbtml        # 归档页（按年/月）
│       │   ├── category.cbtml       # 分类归档页
│       │   ├── tag.cbtml            # 标签归档页
│       │   ├── 404.cbtml            # 404 页面
│       │   └── partials/            # 可复用片段
│       │       ├── head.cbtml       # <head> 内容
│       │       ├── nav.cbtml        # 导航栏
│       │       ├── footer.cbtml     # 页脚
│       │       ├── post-card.cbtml  # 文章卡片
│       │       └── pagination.cbtml # 分页组件
│       ├── assets/                  # 主题前端资源
│       │   ├── scss/                # SCSS 源文件（构建时编译为 CSS）
│       │   │   ├── main.scss
│       │   │   └── _variables.scss
│       │   ├── css/                 # 纯 CSS（直接复制）
│       │   └── js/                  # JavaScript
│       └── admin/                   # 主题后台页面模板（cbtml）
│           ├── menus.cbtml
│           └── typography.cbtml
│
├── plugins/                         # 插件目录
│   ├── seo-optimizer/
│   │   ├── plugin.toml
│   │   ├── main.lua
│   │   ├── lib/
│   │   │   └── analyzer.lua
│   │   └── admin/
│   │       ├── settings.cbtml
│   │       └── editor-panel.cbtml
│   └── comments/
│       ├── plugin.toml
│       ├── main.lua
│       ├── data.db                  # 插件独立 SQLite 数据库
│       └── admin/
│           ├── list.cbtml
│           ├── pending.cbtml
│           └── settings.cbtml
│
├── public/                          # 构建输出（不提交 Git）
│   ├── index.html
│   ├── posts/
│   ├── tags/
│   ├── archive/
│   ├── assets/                      # 主题/插件静态资源
│   ├── media/                       # 上传的媒体文件
│   ├── sitemap.xml
│   ├── feed.xml
│   ├── search-index.json
│   └── robots.txt
│
├── .cblog-cache/                    # 构建缓存（不提交 Git）
│   ├── hashes.json                  # 文件内容哈希表
│   └── deps.json                    # 模板依赖图
│
└── src/
    ├── main.rs                      # 程序入口，初始化 Tokio + Axum
    ├── config.rs                    # cblog.toml 解析与验证
    ├── state.rs                     # AppState：共享状态（DB、构建队列等）
    │
    ├── admin/                       # /admin 动态路由
    │   ├── mod.rs
    │   ├── auth.rs                  # 登录、JWT、CSRF
    │   ├── posts.rs                 # 文章 CRUD
    │   ├── pages.rs                 # 独立页面 CRUD
    │   ├── media.rs                 # 媒体上传/管理
    │   ├── theme.rs                 # 主题配置 & 切换
    │   ├── plugins.rs               # 插件列表 & 激活/停用
    │   └── build.rs                 # 构建触发 & WebSocket 状态
    │
    ├── build/                       # 构建管道
    │   ├── mod.rs
    │   ├── pipeline.rs              # 管道编排与阶段驱动
    │   ├── stages/
    │   │   ├── load.rs              # content.load
    │   │   ├── parse.rs             # content.parse
    │   │   ├── transform.rs         # content.transform
    │   │   ├── taxonomy.rs          # taxonomy.build
    │   │   ├── generate.rs          # page.generate
    │   │   ├── render.rs            # page.render
    │   │   ├── assets.rs            # asset.process（含 SCSS 编译）
    │   │   └── finalize.rs          # build.finalize
    │   ├── incremental.rs           # 哈希缓存 & 脏页检测
    │   ├── graph.rs                 # 模板依赖图
    │   └── scheduler.rs             # 构建任务队列（channel）
    │
    ├── cbtml/                       # cbtml 编译器
    │   ├── mod.rs
    │   ├── lexer.rs                 # 缩进感知词法分析
    │   ├── parser.rs                # 语法分析 → DOM AST
    │   ├── codegen.rs               # AST → MiniJinja 模板字符串
    │   ├── filters.rs               # 内置过滤器注册
    │   └── error.rs                 # 编译错误（行号 + 列号 + 上下文）
    │
    ├── content/                     # 内容解析
    │   ├── mod.rs
    │   ├── markdown.rs              # Markdown → HTML（含 TOC 提取）
    │   ├── frontmatter.rs           # YAML Front Matter 解析
    │   └── excerpt.rs               # 自动摘要截取
    │
    ├── theme/                       # 主题系统
    │   ├── mod.rs
    │   ├── loader.rs                # 主题加载、父子继承链解析
    │   ├── config.rs                # theme.toml schema 解析与验证
    │   └── assets.rs                # SCSS 编译、资源复制
    │
    ├── plugin/                      # 插件系统
    │   ├── mod.rs
    │   ├── registry.rs              # 插件注册与生命周期
    │   ├── scheduler.rs             # 能力声明解析、拓扑排序、并行调度
    │   └── store.rs                 # 插件 KV 存储（plugin_store 表）
    │
    ├── lua/                         # Lua 运行时
    │   ├── mod.rs
    │   ├── runtime.rs               # Lua VM 初始化、生命周期管理
    │   ├── sandbox.rs               # 受限 io 模块、危险函数置 nil
    │   ├── hooks.rs                 # Hook 注册表与执行引擎
    │   └── api/                     # cblog.* Lua API 注册
    │       ├── mod.rs
    │       ├── db.rs                # cblog.db.*
    │       ├── files.rs             # cblog.files.*
    │       ├── store.rs             # cblog.store.*
    │       ├── site.rs              # cblog.site.*
    │       └── http.rs              # cblog.http.*（admin handler 期间）
    │
    └── media/                       # 媒体处理
        ├── mod.rs
        ├── upload.rs                # 上传接收、格式验证
        └── process.rs               # 图片压缩、WebP 转码、缩略图
```

---

## 5. 站点配置（cblog.toml）

`cblog.toml` 是整个站点的全局配置，位于项目根目录。

```toml
# ── 站点基本信息 ──────────────────────────────────────────────────
[site]
title       = "My Blog"
subtitle    = "写点有意思的东西"
description = "个人技术博客，主要写 Rust 和系统编程"
url         = "https://example.com"     # 生产环境完整 URL，生成 sitemap 用
language    = "zh-CN"
timezone    = "Asia/Shanghai"

# 作者信息（多作者时可在文章 Front Matter 覆盖）
[site.author]
name   = "张三"
email  = "me@example.com"
avatar = "/media/avatar.webp"
bio    = "Rustacean，喜欢折腾"

# ── 构建配置 ──────────────────────────────────────────────────────
[build]
output_dir      = "public"             # 静态文件输出目录，相对于项目根
cache_dir       = ".cblog-cache"       # 构建缓存目录
posts_per_page  = 10                   # 全局分页，可被主题配置覆盖
date_format     = "Y年m月d日"          # 全局日期格式，可被模板过滤器覆盖
excerpt_length  = 160                  # 自动摘要截取字符数
parallel        = true                 # 是否并行构建（false 用于调试）

# ── 激活主题 ──────────────────────────────────────────────────────
[theme]
active = "aurora"                      # themes/ 目录下的主题名

# ── 激活的插件列表（按此顺序加载，插件内部声明的 after 会覆盖此顺序） ──
[plugins]
enabled = [
    "seo-optimizer",
    "comments",
    "search",
    "syntax-highlight",
]

# ── 路由配置 ──────────────────────────────────────────────────────
[routes]
post_url    = "/posts/{slug}/"         # 文章 URL 模式
tag_url     = "/tags/{slug}/"          # 标签归档 URL
category_url = "/category/{slug}/"    # 分类归档 URL
archive_url = "/archive/{year}/{month}/" # 时间归档 URL

# ── 后台服务配置 ───────────────────────────────────────────────────
[server]
host        = "127.0.0.1"             # 监听地址（生产建议 127.0.0.1，前置 Nginx）
port        = 3000
log_level   = "info"                  # trace | debug | info | warn | error

# ── 认证配置 ──────────────────────────────────────────────────────
[auth]
# JWT 密钥，生产环境必须修改为随机强密码，不要提交到 Git
jwt_secret      = "CHANGE_ME_IN_PRODUCTION"
jwt_expires_in  = "7d"               # token 有效期：7d / 24h / 30m
session_name    = "cblog_session"

# ── 媒体配置 ──────────────────────────────────────────────────────
[media]
upload_dir      = "media"             # 相对于项目根
max_file_size   = "10MB"
allowed_types   = ["image/jpeg", "image/png", "image/gif", "image/webp"]
auto_webp       = true                # 自动将上传图片转码为 WebP
webp_quality    = 85
generate_thumb  = true
thumb_width     = 400

# ── Feed 配置 ─────────────────────────────────────────────────────
[feed]
enabled     = true
format      = ["rss", "atom"]         # 可同时生成两种格式
post_count  = 20                      # Feed 中包含最新 N 篇文章
full_content = false                  # false = 只包含摘要

# ── Sitemap 配置 ──────────────────────────────────────────────────
[sitemap]
enabled    = true
change_freq = "weekly"
priority    = 0.8
```

---

## 6. cbtml 模板语言

cbtml（cblog template language）是 cblog 的专属模板语言。**缩进定义层级，无闭合标签**，逻辑语法简洁，编译为标准 HTML 输出。

### 6.1 完整语法规则

```
规则一：缩进即层级
  每层缩进 2 空格（全文统一，不支持混用 Tab 和空格）
  父节点关闭标签由缩进层级自动推断，无需手写

规则二：元素语法
  tag                          →  <tag></tag>
  tag.class                    →  <tag class="class">
  tag#id                       →  <tag id="id">
  tag.c1.c2#id                 →  <tag class="c1 c2" id="id">
  tag [attr="val"]             →  <tag attr="val">
  tag [attr={{ expr }}]        →  动态属性（{{ }} 内为表达式）
  tag 文字内容                 →  <tag>文字内容</tag>（单行文本内容）
  void 元素（自动识别）：
    meta / link / input / br / hr / img / source
                               →  自动输出自闭合 <tag />

规则三：文本与输出
  {{ expr }}                   →  HTML 转义输出（默认安全）
  raw expr                     →  不转义，直接输出原始 HTML（用于可信内容）

规则四：逻辑控制
  if expr
    ...
  else if expr
    ...
  else
    ...
  end

  for item in collection
    ...
  end
  （循环内可用 loop.index（1起）、loop.index0（0起）、
    loop.first、loop.last、loop.length）

  {# 注释内容 #}               →  编译期移除，不输出到 HTML

规则五：模板组合
  extends parent_template      →  继承，必须在文件第一行（不含注释）
  slot name                    →  定义具名插槽（父模板中）/ 填充插槽（子模板中）
  include partials/component   →  内联展开另一个 cbtml 文件（路径相对于模板根）
  extends theme_name:template  →  跨主题继承（子主题用，如 extends aurora:post）

规则六：原生块（块内内容原样输出，不做元素解析）
  style                        →  输出 <style> 块，内容为 CSS
  script                       →  输出 <script> 块，内容为 JS

规则七：cblog 专属指令
  hook("hook_name", data)      →  调用 Lua Hook，返回值列表，配合 for 使用
  {{ site.* }}                 →  站点全局数据（标题、URL、导航等）
  {{ theme_config.* }}         →  当前主题配置值
  {{ page.* }}                 →  当前页面元数据
  {{ post.* }}                 →  当前文章数据（文章模板内）
  {{ pagination.* }}           →  分页数据（列表模板内）
```

### 6.2 base.cbtml（基础布局）

```cbtml
{# themes/aurora/templates/base.cbtml #}

html [data-theme="{{ theme_config.dark_mode }}"] [lang="{{ site.language }}"]
  head
    meta [charset="utf-8"]
    meta [name="viewport"] [content="width=device-width, initial-scale=1"]

    {# SEO 基础 meta #}
    title {{ page.title }} - {{ site.title }}
    meta [name="description"] [content="{{ page.description | default(site.description) }}"]
    link [rel="canonical"] [href="{{ site.url }}{{ page.url }}"]

    {# 主题变量注入为 CSS 自定义属性 #}
    style
      :root {
        --color-primary:      {{ theme_config.primary_color }};
        --color-primary-hover:{{ theme_config.primary_hover }};
        --color-primary-light:{{ theme_config.primary_light }};
        --font-body:          {{ theme_config.font_body }}, sans-serif;
        --font-code:          {{ theme_config.font_code }}, monospace;
      }
      {{ theme_config.custom_css }}

    link [rel="stylesheet"] [href="/assets/main.css"]
    link [rel="alternate"] [type="application/rss+xml"]
         [title="{{ site.title }} RSS"] [href="/feed.xml"]

    {# 外部字体链接（由 hooks.lua 根据字体配置动态注入） #}
    for link in ctx.head_links
      raw link
    end

    {# 插件/主题向 <head> 注入的内容（统计代码、Open Graph 等） #}
    for item in hook("head_items", ctx)
      raw item
    end

    {# 主题高级配置：自定义 head 注入 #}
    raw theme_config.custom_head_html

  body [class="layout-{{ layout }}"]

    {# 管理员登录时显示快捷编辑工具栏 #}
    if ctx.is_admin_preview
      div.admin-bar
        span 预览模式
        if post
          a [href="/admin/posts/{{ post.id }}/edit"] 编辑此文章
        end

    include partials/nav

    main.site-main
      slot content

    if theme_config.show_sidebar
      aside.sidebar [class="sidebar-{{ theme_config.sidebar_position }}"]
        for widget in hook("sidebar_widgets", ctx)
          raw widget
        end
    end

    include partials/footer

    {# 插件注入的全局脚本 #}
    for script in ctx.global_scripts
      raw script
    end
```

### 6.3 post.cbtml（文章页）

```cbtml
{# themes/aurora/templates/post.cbtml #}
extends base

slot content
  article.post [itemscope] [itemtype="https://schema.org/BlogPosting"]

    {# 封面图 #}
    if post.cover_image
      div.post-cover
        img [src="{{ post.cover_image }}"] [alt="{{ post.title }}"]
             [itemprop="image"]
    end

    header.post-header
      {# 面包屑 #}
      nav.breadcrumb
        a [href="/"] 首页
        span ›
        if post.category
          a [href="{{ post.category | category_url }}"] {{ post.category }}
          span ›
        end
        span {{ post.title }}

      h1.post-title [itemprop="headline"] {{ post.title }}

      div.post-meta
        time.post-date [datetime="{{ post.created_at | iso }}"] [itemprop="datePublished"]
          {{ post.created_at | date }}
        if post.updated_at != post.created_at
          span.post-updated
            span 更新于
            time [datetime="{{ post.updated_at | iso }}"] {{ post.updated_at | date }}
        end
        if theme_config.show_reading_time
          span.reading-time · {{ post.reading_time | reading_time_label }}
        end
        if post.author
          span.post-author [itemprop="author"]
            raw " · "
            span {{ post.author }}
        end

      if post.tags
        div.post-tags
          for tag in post.tags
            a.tag [href="{{ tag | tag_url }}"] # {{ tag }}
          end
      end

    {# 插件注入区（SEO meta、版权声明等） #}
    for item in hook("post_meta_items", post)
      raw item
    end

    {# 目录（超过阈值才显示） #}
    if post.toc and post.toc | length >= theme_config.toc_min_headings
      nav.toc
        h3.toc-title 目录
        raw post.toc
    end

    {# 正文 #}
    div.post-content [itemprop="articleBody"]
      raw post.content

    {# 文章底部：可被子主题覆盖 #}
    slot post_footer
      div.post-footer
        if post.tags
          div.post-tags-footer
            span 标签：
            for tag in post.tags
              a.tag [href="{{ tag | tag_url }}"] {{ tag }}
            end
        end

        {# 插件注入：分享按钮、打赏等 #}
        for item in hook("post_footer_items", post)
          raw item
        end

        nav.post-nav
          if prev_post
            a.post-nav-prev [href="{{ prev_post.url }}"]
              span.nav-label ← 上一篇
              span.nav-title {{ prev_post.title }}
          end
          if next_post
            a.post-nav-next [href="{{ next_post.url }}"]
              span.nav-label 下一篇 →
              span.nav-title {{ next_post.title }}
          end

    {# 插件注入区（评论区等） #}
    for item in hook("post_bottom_items", post)
      raw item
    end
```

### 6.4 index.cbtml（首页）

```cbtml
{# themes/aurora/templates/index.cbtml #}
extends base

slot content
  div.home-page

    {# 插件可注入首页顶部横幅 #}
    for item in hook("home_top_items", ctx)
      raw item
    end

    div.post-list
      if posts | length == 0
        div.empty-state 还没有文章，去后台写第一篇吧。
      else
        for post in posts
          include partials/post-card
        end
      end

    include partials/pagination
```

### 6.5 partials/post-card.cbtml

```cbtml
{# themes/aurora/templates/partials/post-card.cbtml #}
article.post-card
  if post.cover_image
    a.post-card-cover-link [href="{{ post.url }}"]
      img.post-card-cover [src="{{ post.cover_image }}"] [alt="{{ post.title }}"]
  end

  div.post-card-body
    if post.category
      a.post-card-category [href="{{ post.category | category_url }}"]
        {{ post.category }}
    end

    h2.post-card-title
      a [href="{{ post.url }}"] {{ post.title }}

    div.post-card-meta
      time {{ post.created_at | date }}
      if theme_config.show_reading_time
        span · {{ post.reading_time | reading_time_label }}
      end

    p.post-card-excerpt {{ post.excerpt }}

    if post.tags
      div.post-card-tags
        for tag in post.tags
          a.tag [href="{{ tag | tag_url }}"] {{ tag }}
        end
    end
```

### 6.6 partials/pagination.cbtml

```cbtml
{# themes/aurora/templates/partials/pagination.cbtml #}
if pagination and pagination.total_pages > 1
  nav.pagination [aria-label="文章分页"]
    if pagination.prev
      a.btn.btn-prev [href="{{ pagination.prev }}"] [rel="prev"]
        span ← 较新文章
    else
      span.btn.btn-prev.disabled ← 较新文章
    end

    div.pagination-info
      for i in pagination.page_range
        if i == pagination.current
          span.page-current {{ i }}
        else
          a.page-num [href="{{ pagination.url_for(i) }}"] {{ i }}
        end
      end

    if pagination.next
      a.btn.btn-next [href="{{ pagination.next }}"] [rel="next"]
        span 较旧文章 →
    else
      span.btn.btn-next.disabled 较旧文章 →
    end
end
```

### 6.7 内置过滤器完整列表

| 过滤器 | 说明 | 示例 |
|--------|------|------|
| `date` | 按 cblog.toml date_format 格式化 | `{{ post.created_at \| date }}` |
| `date("fmt")` | 自定义格式（strftime） | `{{ post.created_at \| date("%Y-%m-%d") }}` |
| `iso` | ISO 8601 格式（用于 datetime 属性） | `{{ post.created_at \| iso }}` |
| `slugify` | 转换为 URL 安全 slug | `{{ tag \| slugify }}` |
| `tag_url` | 生成标签归档 URL | `{{ tag \| tag_url }}` |
| `category_url` | 生成分类归档 URL | `{{ post.category \| category_url }}` |
| `truncate(n)` | 截取前 n 个字符，末尾加省略号 | `{{ post.excerpt \| truncate(120) }}` |
| `wordcount` | 统计字数 | `{{ post.content \| wordcount }}` |
| `reading_time` | 估算阅读分钟数（按 200 字/分钟） | `{{ post.content \| reading_time }}` |
| `reading_time_label` | 格式化阅读时间为可读文本 | `{{ post.reading_time \| reading_time_label }}` |
| `escape` | HTML 转义（{{ }} 默认行为） | `{{ content \| escape }}` |
| `safe` | 不转义直接输出（信任来源） | `{{ post.content \| safe }}` |
| `upper` / `lower` | 大小写转换 | `{{ tag \| upper }}` |
| `capitalize` | 首字母大写 | `{{ post.category \| capitalize }}` |
| `length` | 获取字符串或数组长度 | `{{ post.tags \| length }}` |
| `default("val")` | 值为空/nil 时使用默认值 | `{{ post.excerpt \| default("暂无摘要") }}` |
| `active_class` | truthy 时输出 `"active"`，否则空字符串 | `{{ item.active \| active_class }}` |
| `json` | 序列化为 JSON 字符串 | `{{ data \| json }}` |
| `md5` | 计算 MD5（用于 Gravatar 等） | `{{ author.email \| md5 }}` |
| `abs_url` | 转换相对 URL 为绝对 URL | `{{ post.url \| abs_url }}` |

### 6.8 cbtml 编译器实现思路

cbtml 编译分两层，避免重复实现表达式引擎：

```
第一层（cbtml 编译器负责）：结构 → 中间表示
  cbtml 源码
    → Lexer：按行扫描，识别缩进层级、tag 声明、指令、文本内容
    → Parser：根据缩进构建 DOM 树 AST
              （节点类型：Element | Text | If | For | Slot | Include | Raw | Style | Script）
    → CodeGen：DOM 树 → Jinja2 格式模板字符串
              （把 cbtml 的 if/for/slot/include 翻译为 {% %} 指令）

第二层（MiniJinja 负责）：表达式求值
  Jinja2 格式字符串 + 渲染上下文（来自 BuildContext）
    → MiniJinja 执行 if/for 逻辑、变量替换、过滤器调用
    → 最终 HTML 字符串

cbtml 用户（主题/插件作者）只看到 cbtml 语法。
MiniJinja 是内部实现细节，不暴露给外部。
编译错误优先显示 cbtml 的行列号，而非 MiniJinja 的内部行号。
```

**错误格式示例：**
```
cbtml 编译错误
  → themes/aurora/templates/post.cbtml:42:5
  
  40 |     div.post-meta
  41 |       time {{ post.created_at | date }}
  42 |       if theme_config.show_reading_time
  43 |         span.reading-time · {{ post.reading_time | unknown_filter }}
  44 |       end
  
  错误：未知过滤器 `unknown_filter`
  提示：内置过滤器列表见文档，或在 hooks.lua 中用 theme.register_filter() 注册自定义过滤器
```

---

## 7. 内容模型

### 7.1 文章 Front Matter 完整参考

```yaml
---
# ── 必填 ──────────────────────────────────────────────────────────
title: "Rust 异步编程入门"

# ── 可选：路由 ────────────────────────────────────────────────────
slug: "rust-async-intro"           # 不填则从文件名生成（去掉日期前缀和扩展名）

# ── 可选：时间 ────────────────────────────────────────────────────
date: 2024-01-15T10:00:00+08:00    # 发布时间，不填则取文件 mtime
updated: 2024-01-16T08:00:00+08:00 # 最后更新时间，不填则同 date

# ── 可选：分类 ────────────────────────────────────────────────────
tags: ["Rust", "异步", "并发"]
category: "技术"                   # 单个分类

# ── 可选：状态 ────────────────────────────────────────────────────
draft: false                       # true = 不输出到 public/
# status: published                # published | draft | archived（与 draft 二选一）

# ── 可选：展示 ────────────────────────────────────────────────────
cover_image: "/media/2024/01/cover.webp"
excerpt: "手写摘要，不填则自动截取正文前 N 字"
author: "李四"                     # 多作者场景下覆盖全局 author

# ── 可选：模板 ────────────────────────────────────────────────────
template: "post-wide"              # 不填则用主题默认的 post.cbtml
layout: "no-sidebar"               # 传入模板的 layout 变量，主题按需使用

# ── 可选：SEO（通常由 seo-optimizer 插件自动生成，也可手动覆盖） ──
seo_title: ""                      # 自定义 <title>，不填用文章标题
seo_description: ""                # 自定义 meta description
og_image: ""                       # Open Graph 图片，不填用 cover_image
no_index: false                    # true = meta robots noindex

# ── 可选：插件扩展字段（任意 key，插件在 content.transform 阶段读取） ──
series: "Rust 系列"
series_order: 1
---
```

### 7.2 Rust 内容类型定义

```rust
// 文章
pub struct Post {
    pub id:           Ulid,
    pub slug:         String,
    pub title:        String,
    pub content:      MarkdownContent,
    pub status:       PostStatus,
    pub created_at:   DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
    pub tags:         Vec<String>,
    pub category:     Option<String>,
    pub cover_image:  Option<String>,
    pub excerpt:      Option<String>,
    pub author:       Option<String>,
    pub template:     Option<String>,
    pub layout:       Option<String>,
    pub reading_time: u32,            // 分钟，构建时计算
    pub word_count:   u32,            // 字数，构建时计算
    pub toc:          Option<String>, // 目录 HTML，构建时提取
    pub meta:         HashMap<String, serde_json::Value>,  // Front Matter 扩展字段
}

// 懒解析：Markdown 只在需要渲染时才解析 AST
pub struct MarkdownContent {
    pub raw:  String,
    pub html: OnceLock<String>,
    pub ast:  OnceLock<mdast::Node>,
}

pub enum PostStatus { Draft, Published, Archived }

// 标签/分类聚合（taxonomy.build 阶段生成）
pub struct TaxonomyIndex {
    pub tags:       HashMap<String, Vec<PostRef>>,
    pub categories: HashMap<String, Vec<PostRef>>,
    pub archives:   BTreeMap<(i32, u32), Vec<PostRef>>,  // (年, 月) → 文章列表
}

// 页面（by generate 阶段生成）
pub struct Page {
    pub url:       String,         // 输出路径，如 /posts/my-post/
    pub template:  String,         // 使用的模板名
    pub context:   serde_json::Value,  // 传入模板的数据
    pub source:    PageSource,     // 来源：Post | TaxonomyPage | CustomPage
}

// 构建上下文（只读，线程安全共享）
pub struct BuildContext {
    pub posts:        Vec<Post>,
    pub pages:        Vec<Page>,
    pub taxonomy:     TaxonomyIndex,
    pub config:       SiteConfig,
    pub theme_config: HashMap<String, serde_json::Value>,
    pub site:         SiteData,    // 注入模板的 site.* 变量
}
```

### 7.3 模板渲染上下文变量

每个模板渲染时可访问的上下文变量：

```
site.*
  site.title          站点标题
  site.subtitle       站点副标题
  site.description    站点描述
  site.url            站点根 URL（如 https://example.com）
  site.language       语言代码（zh-CN）
  site.author         作者信息对象
  site.nav            导航菜单（由主题 hooks.lua 填充）

page.*
  page.title          当前页面标题
  page.url            当前页面路径（如 /posts/my-post/）
  page.description    当前页面描述
  page.type           页面类型（post / index / tag / archive 等）

post.*（仅文章页）
  post.id             文章 ULID
  post.title          标题
  post.slug           URL slug
  post.content        渲染后的 HTML 内容
  post.excerpt        摘要
  post.cover_image    封面图 URL
  post.created_at     发布时间（DateTime 对象）
  post.updated_at     更新时间
  post.tags           标签数组
  post.category       分类名
  post.author         作者名
  post.reading_time   阅读时间（分钟数）
  post.word_count     字数
  post.toc            目录 HTML（若有）
  post.meta.*         Front Matter 扩展字段

posts（列表页）
  posts               文章数组，每项为 post 对象

pagination（列表页）
  pagination.current      当前页码（1起）
  pagination.total_pages  总页数
  pagination.total_posts  总文章数
  pagination.prev         上一页 URL（第1页时为 nil）
  pagination.next         下一页 URL（最后一页时为 nil）
  pagination.page_range   页码数组（用于生成页码导航）
  pagination.url_for(n)   生成第 n 页的 URL

prev_post / next_post（文章页）
  同 post 对象，相邻文章的简要信息

theme_config.*
  主题 theme.toml 中声明的所有配置项当前值

ctx.*
  ctx.head_links          <head> 中的外部链接（link 标签）
  ctx.global_scripts      全局注入的 script 标签
  ctx.is_admin_preview    是否处于管理员预览模式
```

---

## 8. 构建管道

### 8.1 管道阶段与 Hook 点

```
┌─────────────────────────────────────────────────────────────┐
│  [content.load]                                             │
│    - 扫描 content/posts/ 和 content/pages/ 目录             │
│    - 从 SQLite 加载 meta 覆盖数据                           │
│    - 过滤 draft 文章（生产构建时跳过）                       │
│    hook: after_load(posts, pages)                           │
├─────────────────────────────────────────────────────────────┤
│  [content.parse]                                            │
│    - 解析 YAML Front Matter                                 │
│    - 解析 Markdown → AST                                    │
│    - 提取 TOC（标题树）                                      │
│    - 计算字数和阅读时间                                      │
│    - 自动截取 excerpt（若未手动设置）                         │
│    hook: after_parse(post)   ← 每篇文章单独调用             │
├─────────────────────────────────────────────────────────────┤
│  [content.transform]  ★ 插件最常用的阶段                    │
│    - Markdown AST → HTML（含代码高亮、图片懒加载等）          │
│    - 运行所有注册了此阶段的插件 filter                       │
│    hook: after_transform(post)                              │
├─────────────────────────────────────────────────────────────┤
│  [taxonomy.build]                                           │
│    - 构建标签索引：tag → [post, ...]                        │
│    - 构建分类索引：category → [post, ...]                   │
│    - 构建时间归档索引：(年, 月) → [post, ...]               │
│    hook: after_taxonomy(taxonomy_index)                     │
├─────────────────────────────────────────────────────────────┤
│  [page.generate]                                            │
│    - 为每篇文章生成页面记录                                  │
│    - 为每个标签/分类生成归档页（含分页）                     │
│    - 为时间归档生成页面                                      │
│    - 生成首页（含分页）                                      │
│    hook: after_generate(pages)   ← 插件可在此注入额外页面   │
├─────────────────────────────────────────────────────────────┤
│  [page.render]                                              │
│    - 对每个 Page：编译 cbtml → HTML                         │
│    - rayon 并行执行（CPU 核心数）                            │
│    - 增量：跳过哈希未变更的页面                              │
│    hook: after_render(page, html)                           │
├─────────────────────────────────────────────────────────────┤
│  [asset.process]                                            │
│    - 编译主题 SCSS → CSS                                    │
│    - 复制主题 assets/ → public/assets/                      │
│    - 复制插件 assets/ → public/assets/                      │
│    - 复制 media/ → public/media/                            │
│    hook: after_assets(ctx)                                  │
├─────────────────────────────────────────────────────────────┤
│  [build.finalize]                                           │
│    - 生成 sitemap.xml                                       │
│    - 生成 RSS/Atom feed.xml                                 │
│    - 生成客户端搜索索引（若插件开启）                         │
│    - 清理已删除文章的旧 HTML 文件                            │
│    hook: after_finalize(ctx)                                │
└─────────────────────────────────────────────────────────────┘
```

### 8.2 增量构建机制

增量构建基于两套数据结构：

**内容哈希表**（`.cblog-cache/hashes.json`）：
```json
{
  "content/posts/2024-01-hello.md": "a3f5b2...",
  "themes/aurora/templates/post.cbtml": "d8c1e0...",
  "themes/aurora/assets/scss/main.scss": "7b3a91...",
  "cblog.toml": "2f8d44..."
}
```

**模板依赖图**（`.cblog-cache/deps.json`）：
```json
{
  "themes/aurora/templates/post.cbtml": [
    "/posts/hello-world/",
    "/posts/rust-tips/"
  ],
  "themes/aurora/templates/partials/post-card.cbtml": [
    "/",
    "/page/2/",
    "/archive/2024/01/"
  ]
}
```

**脏页判定规则：**
```rust
fn is_dirty(page: &Page, cache: &BuildCache, changed_files: &[PathBuf]) -> bool {
    // 规则 1：页面对应的源文件（Markdown）内容哈希变了
    if let Some(source) = page.source_file() {
        if cache.hash_changed(source) { return true; }
    }
    // 规则 2：页面使用的模板文件内容哈希变了
    for tmpl in page.used_templates() {
        if cache.hash_changed(tmpl) { return true; }
    }
    // 规则 3：全局配置或主题配置变了 → 触发全量重建（调用方处理）
    false
}
```

### 8.3 构建任务队列

构建任务通过 Tokio channel 实现，Axum handler 写入任务，独立的构建 worker 消费：

```rust
pub enum BuildTask {
    PostChanged(Ulid),           // 单篇文章变更，增量构建
    PageChanged(Ulid),           // 独立页面变更，增量构建
    ThemeConfigChanged,          // 主题配置变更，全量重建
    PluginConfigChanged(String), // 插件配置变更，全量重建
    MediaChanged,                // 媒体变更，只处理 asset.process 阶段
    FullRebuild,                 // 强制全量重建
}

// 构建进度事件，通过 broadcast channel 推送到 WebSocket
pub enum BuildEvent {
    Started { task: BuildTask, total_pages: usize },
    StageBegin { stage: &'static str },
    PageRendered { url: String, from_cache: bool },
    StageEnd { stage: &'static str, duration_ms: u64 },
    Finished { total_ms: u64, rebuilt: usize, cached: usize },
    Failed { stage: &'static str, error: String },
}
```

### 8.4 主题资源管道（asset.process）

```
themes/aurora/assets/
├── scss/
│   ├── main.scss        → 编译为 CSS（grass crate）
│   └── _variables.scss  → @import 由 main.scss 引用，不单独输出
├── css/
│   └── print.css        → 直接复制，不编译
└── js/
    └── main.js          → 直接复制（不做 bundle，保持简单）

输出到：
public/assets/
├── main.css             （含 source map，开发模式）
├── print.css
└── main.js
```

SCSS 变量注入（把主题配置值作为 SCSS 变量传入）：

```rust
// asset.process 阶段在编译 SCSS 前，动态生成变量覆盖字符串
fn build_scss_overrides(theme_config: &HashMap<String, Value>) -> String {
    let mut overrides = String::new();
    for (key, val) in theme_config {
        if let Some(s) = val.as_str() {
            // 把 theme_config.primary_color → $primary-color: #6366f1;
            overrides.push_str(&format!(
                "${}: {};\n",
                key.replace('_', "-"),
                s
            ));
        }
    }
    overrides
}
// 然后将 overrides 字符串前置到 main.scss 内容后编译
```

---

## 9. 主题系统

### 9.1 theme.toml 配置 Schema

```toml
[theme]
name        = "Aurora"
version     = "1.2.0"
author      = "your-name"
description = "一个简洁的博客主题"
homepage    = "https://github.com/yourname/aurora"
# 声明父主题（子主题时填写）
# parent = "another-theme"

# ── 配置项声明 ────────────────────────────────────────────────────
# 每个 [[config]] 对应后台主题设置页的一个表单字段
# 后台读取此文件自动生成表单，主题作者无需写任何后台代码

[[config]]
key        = "primary_color"
type       = "color"
label      = "主色调"
default    = "#6366f1"
group      = "外观"

[[config]]
key        = "font_body"
type       = "font_select"
label      = "正文字体"
options    = ["system-ui", "Georgia", "Noto Serif SC", "LXGW WenKai"]
default    = "system-ui"
group      = "外观"

[[config]]
key        = "font_code"
type       = "font_select"
label      = "代码字体"
options    = ["monospace", "JetBrains Mono", "Fira Code"]
default    = "monospace"
group      = "外观"

[[config]]
key        = "dark_mode"
type       = "select"
label      = "深色模式"
options    = [
    { value = "auto",  label = "跟随系统" },
    { value = "light", label = "始终浅色" },
    { value = "dark",  label = "始终深色" },
]
default    = "auto"
group      = "外观"

[[config]]
key        = "posts_per_page"
type       = "number"
label      = "每页文章数"
default    = 10
min        = 1
max        = 50
group      = "布局"

[[config]]
key        = "show_sidebar"
type       = "boolean"
label      = "显示侧边栏"
default    = true
group      = "布局"

[[config]]
key        = "sidebar_position"
type       = "select"
label      = "侧边栏位置"
options    = [
    { value = "right", label = "右侧" },
    { value = "left",  label = "左侧" },
]
default    = "right"
depends_on = "show_sidebar"    # 仅在 show_sidebar=true 时显示此字段
group      = "布局"

[[config]]
key        = "show_reading_time"
type       = "boolean"
label      = "显示阅读时间"
default    = true
group      = "功能"

[[config]]
key        = "toc_min_headings"
type       = "number"
label      = "目录显示阈值（超过 N 个标题才显示）"
default    = 3
min        = 0
max        = 20
group      = "功能"

[[config]]
key        = "footer_text"
type       = "richtext"
label      = "页脚文字"
default    = "Powered by cblog"
group      = "功能"

[[config]]
key        = "custom_head_html"
type       = "code"
language   = "html"
label      = "Head 注入"
description = "注入到 <head> 末尾，适合放统计代码（如 Umami、Google Analytics）"
default    = ""
group      = "高级"

[[config]]
key        = "custom_css"
type       = "code"
language   = "css"
label      = "自定义 CSS"
description = "追加到主题 CSS 后面，优先级最高"
default    = ""
group      = "高级"
```

**config.type 枚举说明：**

| type | 后台控件 | 值类型 |
|------|---------|-------|
| `color` | 颜色选择器 | `#rrggbb` 字符串 |
| `font_select` | 字体下拉 + 预览 | 字符串 |
| `select` | 下拉选择，options 为字符串或 `{value, label}` 数组 | 字符串 |
| `boolean` | 开关 Toggle | `true` / `false` |
| `number` | 数字输入框，支持 `min` / `max` | 整数 |
| `text` | 单行文本 | 字符串 |
| `textarea` | 多行文本 | 字符串 |
| `richtext` | 富文本（支持 HTML） | HTML 字符串 |
| `code` | 代码编辑器，`language` 指定语法高亮语言 | 字符串 |
| `image` | 图片选择器（从媒体库选择） | URL 字符串 |

### 9.2 主题后台页面

主题通过 `hooks.lua` 注册自己的后台页面，自动出现在侧边栏"外观"分组下：

```lua
-- themes/aurora/hooks.lua

-- 注册主题后台页面
theme.register_admin_page({
    slug     = "aurora/menus",
    title    = "菜单管理",
    icon     = "menu",
    group    = "appearance",
})

theme.register_admin_page({
    slug     = "aurora/typography",
    title    = "排版预览",
    icon     = "type",
    group    = "appearance",
})

-- 菜单管理 GET
theme.on_admin_get("aurora/menus", function(req, store)
    local menus = store:get("menus") or {
        { label = "首页", url = "/" },
        { label = "归档", url = "/archive/" },
        { label = "关于", url = "/about/" },
    }
    return { view = "admin/menus.cbtml", data = { menus = menus } }
end)

-- 菜单管理 POST（保存）
theme.on_admin_post("aurora/menus", function(req, store)
    local labels = req.form["label[]"] or {}
    local urls   = req.form["url[]"]   or {}
    local menus  = {}
    for i = 1, #labels do
        if labels[i] ~= "" and urls[i] ~= "" then
            table.insert(menus, { label = labels[i], url = urls[i] })
        end
    end
    store:set("menus", menus)
    theme.trigger_rebuild()
    return { redirect = "/admin/theme/aurora/menus", flash = "菜单已保存" }
end)

-- 排版预览：只读
theme.on_admin_get("aurora/typography", function(req, store)
    return { view = "admin/typography.cbtml", data = {} }
end)
```

### 9.3 主题 Lua Hook（hooks.lua）

```lua
-- themes/aurora/hooks.lua

-- ── 声明主题暴露给插件的 Hook 点 ─────────────────────────────────
theme.expose_hook("head_items",        { returns = "string[]" })
theme.expose_hook("post_meta_items",   { returns = "string[]" })
theme.expose_hook("post_footer_items", { returns = "string[]" })
theme.expose_hook("post_bottom_items", { returns = "string[]" })
theme.expose_hook("sidebar_widgets",   { returns = "Widget[]" })
theme.expose_hook("home_top_items",    { returns = "string[]" })

-- ── 构建前 Hook：处理配置联动逻辑 ────────────────────────────────
theme.hook("pre_render", function(ctx)
    local cfg = ctx.theme_config

    -- 根据字体选择注入外部 CDN 链接
    local font_cdns = {
        ["Noto Serif SC"] = "https://fonts.googleapis.com/css2?family=Noto+Serif+SC:wght@400;700&display=swap",
        ["LXGW WenKai"]   = "https://cdn.jsdelivr.net/npm/lxgw-wenkai-webfont/style.css",
        ["JetBrains Mono"]= "https://fonts.googleapis.com/css2?family=JetBrains+Mono&display=swap",
        ["Fira Code"]     = "https://fonts.googleapis.com/css2?family=Fira+Code&display=swap",
    }
    if font_cdns[cfg.font_body] then
        table.insert(ctx.head_links, string.format(
            '<link rel="stylesheet" href="%s">', font_cdns[cfg.font_body]
        ))
    end
    if font_cdns[cfg.font_code] then
        table.insert(ctx.head_links, string.format(
            '<link rel="stylesheet" href="%s">', font_cdns[cfg.font_code]
        ))
    end

    -- 从 store 读取菜单数据，注入到所有模板的 site.nav
    local menus = theme.store():get("menus") or {}
    ctx.site.nav = menus

    -- 自动派生色彩变量（供 SCSS 和模板使用）
    ctx.theme_config.primary_hover = color.darken(cfg.primary_color, 0.1)
    ctx.theme_config.primary_light = color.lighten(cfg.primary_color, 0.9)

    return ctx
end)

-- ── 注册自定义过滤器 ──────────────────────────────────────────────
theme.register_filter("reading_time_label", function(minutes)
    if minutes < 1 then return "不足 1 分钟阅读"
    elseif minutes == 1 then return "约 1 分钟阅读"
    else return string.format("约 %d 分钟阅读", minutes)
    end
end)

theme.register_filter("tag_url", function(tag)
    return "/tags/" .. cblog.slugify(tag) .. "/"
end)

theme.register_filter("category_url", function(category)
    return "/category/" .. cblog.slugify(category) .. "/"
end)
```

### 9.4 子主题

```cbtml
{# themes/my-child/templates/post.cbtml #}
{# 继承父主题 aurora 的 post 模板，只覆盖 post_footer slot #}
extends aurora:post

slot post_footer
  div.custom-footer
    p.license
      本文采用
      a [href="https://creativecommons.org/licenses/by/4.0/"] CC BY 4.0
      协议授权，转载请注明出处。
    if post.tags
      div.footer-tags
        span 标签：
        for tag in post.tags
          a.tag [href="{{ tag | tag_url }}"] {{ tag }}
        end
    end
```

```toml
# themes/my-child/theme.toml
[theme]
name   = "My Child Theme"
parent = "aurora"
# 子主题只需声明想覆盖的配置默认值，其余继承父主题
[[config]]
key     = "primary_color"
default = "#e74c3c"
```

---

## 10. 插件系统

### 10.1 plugin.toml 元数据

```toml
[plugin]
name        = "seo-optimizer"
version     = "1.2.0"
author      = "your-name"
description = "自动生成 SEO meta 标签、Open Graph、JSON-LD 结构化数据"
homepage    = "https://github.com/yourname/cblog-seo"
min_cblog   = "0.5.0"              # 要求的最低 cblog 版本

# 显式声明能力（用于拓扑排序和并行调度）
[capabilities]
reads     = ["post.meta", "post.content", "config.site_url"]
writes    = ["post.meta.og_tags", "post.meta.json_ld", "post.meta.canonical"]
generates = ["public/robots.txt"]

# 与其他插件的关系
[dependencies]
after     = ["syntax-highlight"]   # 在代码高亮插件之后运行
conflicts = ["basic-seo"]          # 不能与这些插件同时激活

[hooks]
filters = ["post.meta"]
actions = ["build.finalize"]
```

### 10.2 插件 main.lua 完整示例

```lua
-- plugins/seo-optimizer/main.lua
local plugin  = require("plugin")
local analyzer = require("lib.analyzer")

-- ── 构建期 Filter Hook ────────────────────────────────────────────

-- 修改文章 meta 数据（在 content.transform 之后执行）
plugin.filter("post.meta", 10, function(meta, post)
    local cfg  = plugin.config()
    local site = cblog.site()

    -- Open Graph
    meta.og_title       = meta.seo_title or post.title
    meta.og_description = meta.seo_description or post.excerpt or ""
    meta.og_image       = meta.og_image or post.cover_image
                          or cfg.default_og_image or ""
    meta.og_url         = site.url .. post.url
    meta.og_type        = "article"

    -- Canonical
    meta.canonical      = site.url .. post.url

    -- JSON-LD 结构化数据
    meta.json_ld = cblog.json({
        ["@context"]        = "https://schema.org",
        ["@type"]           = "BlogPosting",
        headline            = post.title,
        description         = meta.og_description,
        image               = meta.og_image,
        datePublished       = cblog.iso_date(post.created_at),
        dateModified        = cblog.iso_date(post.updated_at),
        author              = {
            ["@type"] = "Person",
            name      = post.author or site.author.name,
        },
        publisher           = {
            ["@type"] = "Organization",
            name      = site.title,
        },
    })

    -- SEO 分析得分（存入 meta，编辑器侧边栏面板读取）
    meta.seo_score = analyzer.score(post, cfg)

    return meta
end)

-- ── 构建期 Action Hook ────────────────────────────────────────────

plugin.action("build.finalize", 100, function(ctx)
    local cfg  = plugin.config()
    local site = cblog.site()

    -- 生成 robots.txt
    local robots = "User-agent: *\n"
    robots = robots .. "Allow: /\n"
    if cfg.disallow_paths then
        for _, path in ipairs(cfg.disallow_paths) do
            robots = robots .. "Disallow: " .. path .. "\n"
        end
    end
    robots = robots .. "\nSitemap: " .. site.url .. "/sitemap.xml\n"
    cblog.files.write("public/robots.txt", robots)
end)

-- ── 后台页面：SEO 设置 ────────────────────────────────────────────

plugin.register_admin_page({
    slug  = "seo-optimizer",
    title = "SEO 设置",
    icon  = "search",
    group = "plugins",
})

plugin.on_admin_get("seo-optimizer", function(req, store)
    local config = store:get("config") or {
        auto_description = true,
        default_og_image = "",
        disallow_paths   = {},
    }
    return { view = "admin/settings.cbtml", data = { config = config } }
end)

plugin.on_admin_post("seo-optimizer", function(req, store)
    local data = req.form

    -- 校验
    if data.default_og_image ~= ""
       and not string.match(data.default_og_image, "^https?://") then
        return {
            view  = "admin/settings.cbtml",
            data  = { config = data, error = "OG 图片必须是完整 URL（https://...）" },
        }
    end

    -- 解析多行 disallow 输入
    local disallow = {}
    for line in string.gmatch(data.disallow_raw or "", "[^\n]+") do
        local path = string.match(line, "^%s*(.-)%s*$")
        if path ~= "" then table.insert(disallow, path) end
    end
    data.disallow_paths = disallow

    store:set("config", data)
    plugin.trigger_rebuild()

    return { redirect = "/admin/plugins/seo-optimizer", flash = "SEO 设置已保存" }
end)

-- ── 注入文章编辑器侧边栏面板 ─────────────────────────────────────

plugin.inject_editor_panel({
    target   = "post_editor",
    panel_id = "seo-panel",
    title    = "SEO 分析",
    position = "sidebar",
    render   = function(post, store)
        local cfg      = store:get("config") or {}
        local analysis = analyzer.analyze(post, cfg)
        return {
            view = "admin/editor-panel.cbtml",
            data = { analysis = analysis, post = post },
        }
    end,
})
```

### 10.3 插件后台模板（cbtml）

```cbtml
{# plugins/seo-optimizer/admin/settings.cbtml #}
extends admin:base

slot content
  div.settings-page
    h2 SEO 全局设置

    if error
      div.alert.alert-error {{ error }}
    end
    if flash
      div.alert.alert-success {{ flash }}
    end

    form [method="POST"]

      div.field
        label 自动生成 Meta Description
        div.field-control
          input [type="checkbox"] [name="auto_description"] [id="auto_description"]
                [checked="{{ config.auto_description }}"]
          label.checkbox-label [for="auto_description"]
            从文章正文截取前 160 字作为 meta description

      div.field
        label 默认 OG 图片
        div.field-control
          input.input [type="url"] [name="default_og_image"]
                [value="{{ config.default_og_image }}"]
                [placeholder="https://example.com/og-default.jpg"]
          p.field-hint 当文章没有封面图时使用此图片作为 Open Graph 图片

      div.field
        label Robots.txt Disallow 路径
        div.field-control
          textarea.textarea [name="disallow_raw"]
                            [placeholder="/private/\n/draft/"]
            {{ config.disallow_paths | join("\n") }}
          p.field-hint 每行一个路径，留空表示全部允许

      div.form-actions
        button.btn.btn-primary [type="submit"] 保存并重新构建
        a.btn.btn-secondary [href="/admin/plugins/seo-optimizer"] 取消
```

### 10.4 插件多子页面（评论插件）

```lua
-- plugins/comments/main.lua
local plugin = require("plugin")

-- 建立插件独立数据库表
plugin.on_install(function(db)
    db:exec([[
        CREATE TABLE IF NOT EXISTS comments (
            id         TEXT PRIMARY KEY,
            post_slug  TEXT NOT NULL,
            author     TEXT NOT NULL,
            email      TEXT NOT NULL,
            content    TEXT NOT NULL,
            status     TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL
        )
    ]])
end)

-- 注册多个后台页面，形成子菜单
plugin.register_admin_page({
    slug     = "comments",
    title    = "评论管理",
    icon     = "comment",
    group    = "content",
    children = {
        { slug = "comments/all",      title = "所有评论" },
        { slug = "comments/pending",  title = "待审核",
          badge = function()
              return plugin.get_db():count(
                  "SELECT COUNT(*) FROM comments WHERE status='pending'"
              )
          end
        },
        { slug = "comments/settings", title = "评论设置" },
    },
})

-- 待审核列表
plugin.on_admin_get("comments/pending", function(req, store)
    local db      = plugin.get_db()
    local page    = tonumber(req.query.page) or 1
    local limit   = 20
    local offset  = (page - 1) * limit
    local pending = db:query(
        "SELECT * FROM comments WHERE status='pending' ORDER BY created_at DESC LIMIT ? OFFSET ?",
        limit, offset
    )
    local total = db:count("SELECT COUNT(*) FROM comments WHERE status='pending'")
    return {
        view = "admin/pending.cbtml",
        data = {
            comments   = pending,
            pagination = { current = page, total = math.ceil(total / limit) }
        }
    }
end)

plugin.on_admin_post("comments/pending", function(req, store)
    local db     = plugin.get_db()
    local ids    = req.form["ids[]"] or {}
    local action = req.form.action

    if #ids == 0 then
        return { redirect = "/admin/plugins/comments/pending", flash = "请选择评论" }
    end

    if action == "approve" then
        for _, id in ipairs(ids) do
            db:exec("UPDATE comments SET status='approved' WHERE id=?", id)
        end
        -- 审核通过的评论涉及的文章需要重建
        local slugs = db:query(
            "SELECT DISTINCT post_slug FROM comments WHERE id IN (?)", ids
        )
        plugin.trigger_rebuild({ scope = "posts", slugs = slugs })
    elseif action == "reject" then
        for _, id in ipairs(ids) do
            db:exec("UPDATE comments SET status='rejected' WHERE id=?", id)
        end
    elseif action == "delete" then
        for _, id in ipairs(ids) do
            db:exec("DELETE FROM comments WHERE id=?", id)
        end
    end

    return { redirect = "/admin/plugins/comments/pending", flash = "操作成功" }
end)

-- 在每篇文章底部注入评论区
plugin.filter("post_bottom_items", 10, function(items, post)
    local cfg      = plugin.config()
    local db       = plugin.get_db()
    local comments = db:query(
        "SELECT * FROM comments WHERE post_slug=? AND status='approved' ORDER BY created_at ASC",
        post.slug
    )
    local html = cblog.render_template("plugins/comments/comment-section.cbtml", {
        post     = post,
        comments = comments,
        config   = cfg,
    })
    table.insert(items, html)
    return items
end)
```

### 10.5 插件间通信（事件总线）

```lua
-- plugins/toc-generator/main.lua
plugin.filter("content.transform", 10, function(post)
    if not post.toc then
        post.toc = generate_toc(post.content)
    end
    -- 广播事件，感兴趣的插件可以监听
    plugin.emit("toc.generated", { post_id = post.id, toc = post.toc })
    return post
end)

-- plugins/reading-progress/main.lua
-- 仅当 toc 插件激活时才工作（优雅降级）
plugin.on("toc.generated", function(event)
    cblog.files.append(
        "public/assets/reading-progress.js",
        string.format("initProgress('%s');\n", event.post_id)
    )
end)
```

### 10.6 插件生命周期

```lua
-- 安装时执行（首次激活插件）
plugin.on_install(function(db)
    -- 建表、设置初始数据等
end)

-- 卸载时执行（停用并删除插件数据）
plugin.on_uninstall(function(db)
    -- 清理数据库表、删除生成的文件等
    db:exec("DROP TABLE IF EXISTS comments")
    cblog.files.remove_dir("public/comment-assets/")
end)

-- 升级时执行（版本号变化时）
plugin.on_upgrade(function(db, from_version, to_version)
    if cblog.version_lt(from_version, "1.1.0") then
        -- 1.0.x → 1.1.0 的数据库迁移
        db:exec("ALTER TABLE comments ADD COLUMN ip TEXT")
    end
end)

-- 每次构建开始前执行
plugin.on_build_start(function(ctx)
    -- 可用于清理上次构建的中间文件等
end)
```

---

## 11. 后台管理 /admin

### 11.1 路由结构

```
/admin
├── GET  /                           # 仪表盘
├── GET  /login                      # 登录页
├── POST /login                      # 登录提交
├── POST /logout                     # 退出登录
│
├── GET  /posts                      # 文章列表（支持搜索、筛选、分页）
├── GET  /posts/new                  # 新建文章
├── POST /posts                      # 创建文章
├── GET  /posts/:id/edit             # 编辑文章
├── PUT  /posts/:id                  # 更新文章
├── DELETE /posts/:id                # 删除文章（软删除 → archived）
├── POST /posts/:id/publish          # 发布草稿
├── POST /posts/:id/unpublish        # 下线文章
│
├── GET  /pages                      # 独立页面列表
├── GET  /pages/new
├── POST /pages
├── GET  /pages/:id/edit
├── PUT  /pages/:id
├── DELETE /pages/:id
│
├── GET  /media                      # 媒体库
├── POST /media/upload               # 上传文件（multipart/form-data）
├── DELETE /media/:id                # 删除媒体文件
│
├── GET  /theme                      # 主题配置（theme.toml 自动生成表单）
├── POST /theme                      # 保存主题配置
├── GET  /theme/switch               # 主题列表
├── POST /theme/switch               # 切换主题
├── GET  /theme/:slug                # 主题注册的后台页面（动态挂载）
├── POST /theme/:slug                # 主题后台页面表单提交
│
├── GET  /plugins                    # 插件列表
├── POST /plugins/:name/enable       # 激活插件
├── POST /plugins/:name/disable      # 停用插件
├── GET  /plugins/:slug              # 插件注册的后台页面（动态挂载）
├── POST /plugins/:slug              # 插件后台页面表单提交
│
├── POST /build/trigger              # 手动触发全量构建
├── GET  /build/status               # WebSocket：实时构建进度推送
├── GET  /build/history              # 构建历史列表
│
├── GET  /settings                   # 全局设置（cblog.toml 中可修改的部分）
├── POST /settings                   # 保存全局设置
├── POST /settings/change-password   # 修改管理员密码
│
└── GET  /api/media                  # JSON API：获取媒体库列表（编辑器用）
```

### 11.2 后台侧边栏结构

```
CBLOG 管理后台
│
├── 🏠  仪表盘
│     文章总数 · 草稿数 · 本月访问（若有统计插件）
│     最近构建状态
│
├── 内容
│   ├── 所有文章
│   ├── 新建文章
│   ├── 页面管理
│   └── 媒体库
│
├── 外观
│   ├── 主题设置        ← theme.toml schema 自动生成表单
│   ├── 切换主题
│   │
│   └── [主题动态注入]   ← 当前激活主题注册的页面
│       ├── 菜单管理     （aurora 主题示例）
│       └── 排版预览
│
├── 插件
│   ├── 已安装插件
│   └── [插件动态注入]   ← 各插件注册的页面
│       ├── SEO 设置
│       ├── 评论管理
│       │   ├── 所有评论
│       │   └── 待审核 (3)  ← badge 由插件动态计算
│       └── 访问统计
│
└── 系统
    ├── 全局设置
    ├── 构建历史
    └── 修改密码
```

### 11.3 文章编辑器

编辑器页面 `/admin/posts/:id/edit` 的布局：

```
┌──────────────────────────────────┬────────────────────────┐
│  标题输入框（大）                 │  侧边栏                  │
│                                  │  ┌──────────────────┐  │
│  Markdown 编辑区                 │  │ 发布设置          │  │
│  （CodeMirror / Monaco）         │  │  状态：[草稿 ▼]  │  │
│                                  │  │  发布时间：[日期]  │  │
│  ─────────────────────────────── │  │  [保存草稿] [发布] │  │
│  实时预览（可切换显示/隐藏）       │  └──────────────────┘  │
│                                  │  ┌──────────────────┐  │
│                                  │  │ 文章元数据        │  │
│                                  │  │  slug、分类、标签  │  │
│                                  │  │  封面图、摘要      │  │
│                                  │  └──────────────────┘  │
│                                  │                          │
│                                  │  [插件注入的侧边栏面板]   │
│                                  │  ┌──────────────────┐  │
│                                  │  │ SEO 分析         │  │
│                                  │  │  得分：82/100    │  │
│                                  │  │  ✓ 标题长度适中  │  │
│                                  │  │  ✗ 缺少 meta 描述 │  │
│                                  │  └──────────────────┘  │
└──────────────────────────────────┴────────────────────────┘
```

编辑器工具栏提供：加粗、斜体、代码块、引用、有序/无序列表、插入图片（打开媒体库选择器）、插入链接、全屏模式。

### 11.4 构建状态 WebSocket

前端订阅构建进度，实时显示在仪表盘和编辑器保存按钮旁：

```javascript
// 后台 JS
const ws = new WebSocket('/admin/build/status');
ws.onmessage = (e) => {
    const event = JSON.parse(e.data);
    switch (event.type) {
        case 'started':
            showBuildProgress(0, event.total_pages);
            break;
        case 'page_rendered':
            incrementProgress(event.url, event.from_cache);
            break;
        case 'finished':
            showBuildSuccess(event.rebuilt, event.total_ms);
            break;
        case 'failed':
            showBuildError(event.stage, event.error);
            break;
    }
};
```

### 11.5 Axum 路由组装

```rust
pub async fn build_admin_router(
    plugins: Arc<PluginRegistry>,
    theme:   Arc<ActiveTheme>,
) -> Router {
    let mut router = Router::new()
        // 认证
        .route("/admin/login",            get(login_page).post(login_submit))
        .route("/admin/logout",           post(logout))
        // 文章
        .route("/admin/posts",            get(list_posts).post(create_post))
        .route("/admin/posts/new",        get(new_post_page))
        .route("/admin/posts/:id/edit",   get(edit_post_page))
        .route("/admin/posts/:id",        put(update_post).delete(delete_post))
        .route("/admin/posts/:id/publish",  post(publish_post))
        .route("/admin/posts/:id/unpublish",post(unpublish_post))
        // 媒体
        .route("/admin/media",            get(media_library))
        .route("/admin/media/upload",     post(upload_media))
        .route("/admin/media/:id",        delete(delete_media))
        .route("/admin/api/media",        get(api_media_list))
        // 主题
        .route("/admin/theme",            get(theme_settings).post(save_theme_settings))
        .route("/admin/theme/switch",     get(theme_list).post(switch_theme))
        // 插件
        .route("/admin/plugins",          get(plugin_list))
        .route("/admin/plugins/:name/enable",  post(enable_plugin))
        .route("/admin/plugins/:name/disable", post(disable_plugin))
        // 构建
        .route("/admin/build/trigger",    post(trigger_build))
        .route("/admin/build/status",     get(build_status_ws))
        .route("/admin/build/history",    get(build_history))
        // 系统设置
        .route("/admin/settings",         get(settings_page).post(save_settings))
        .route("/admin/settings/change-password", post(change_password));

    // 动态挂载主题后台页面（归入 /admin/theme/:slug）
    for page in theme.admin_pages() {
        let path = format!("/admin/theme/{}", page.slug);
        let get_h  = page.get_handler.clone();
        let post_h = page.post_handler.clone();
        router = router.route(&path,
            get(move |req, s| call_lua_handler(get_h.clone(), req, s))
            .post(move |req, s| call_lua_handler(post_h.clone(), req, s))
        );
    }

    // 动态挂载插件后台页面（归入 /admin/plugins/:slug）
    for plugin in plugins.all() {
        for page in plugin.admin_pages() {
            let path = format!("/admin/plugins/{}", page.slug);
            let get_h  = page.get_handler.clone();
            let post_h = page.post_handler.clone();
            router = router.route(&path,
                get(move |req, s| call_lua_handler(get_h.clone(), req, s))
                .post(move |req, s| call_lua_handler(post_h.clone(), req, s))
            );
        }
    }

    // 登录页不需要认证，其余所有 admin 路由需要 JWT 验证
    router
        .route_layer(middleware::from_fn(require_auth))
        .layer(CsrfLayer::new())
}
```

---

## 12. 认证与安全

### 12.1 认证流程

cblog 后台仅支持单管理员账号（博客场景不需要多用户系统）。

```
登录流程：
  POST /admin/login { username, password }
    → 从 users 表查询账号
    → argon2 验证密码哈希
    → 验证通过 → 签发 JWT（有效期见 cblog.toml auth.jwt_expires_in）
    → 设置 HttpOnly Cookie（名称见 auth.session_name）
    → 重定向到 /admin

后续请求：
  请求携带 Cookie
    → middleware::require_auth 提取 Cookie
    → 验证 JWT 签名和有效期
    → 注入 AuthUser 到 Extension
    → 继续处理

Token 刷新：
  每次请求时检查 JWT 剩余有效期
  若剩余时间 < 总有效期的 1/3，自动续期（重新签发并更新 Cookie）
```

### 12.2 安全措施

**密码存储：**
```rust
// 使用 argon2id，参数：m=19456, t=2, p=1（OWASP 推荐的最低安全参数）
let hash = Argon2::default()
    .hash_password(password.as_bytes(), &salt)?
    .to_string();
```

**CSRF 防护：**
所有非幂等操作（POST/PUT/DELETE）需要携带 CSRF token。token 通过 `tower-sessions` 管理，模板中通过 `{{ ctx.csrf_token }}` 注入表单隐藏字段。

**HTTP 安全 Headers（通过 Axum Tower Layer）：**
```
Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; ...
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
```

**请求速率限制：**
登录接口实施速率限制（同 IP 每分钟最多 10 次），防止暴力破解：
```rust
.route_layer(RateLimitLayer::new(10, Duration::from_secs(60)))
```

**会话管理：**
- JWT 存在 HttpOnly + SameSite=Strict + Secure Cookie 中，不暴露给 JS
- 登出时在服务端维护一个撤销 token 的黑名单（存入 SQLite），有效期内的 token 可立即失效

### 12.3 管理后台与公开 API 的路由隔离

Nginx 配置确保 `/admin` 路径只能从本地访问（或通过 IP 白名单）：
```nginx
location /admin {
    # 可选：限制只允许特定 IP 访问后台
    # allow 1.2.3.4;
    # deny all;
    proxy_pass http://127.0.0.1:3000;
}
```

---

## 13. 媒体文件管理

### 13.1 上传流程

```
POST /admin/media/upload (multipart/form-data, 需要认证)
  → 检查文件大小（< cblog.toml media.max_file_size）
  → 检查 MIME 类型（在 allowed_types 白名单内）
  → 检查文件头魔数（防止伪造 Content-Type）
  → 生成存储路径：media/{year}/{month}/{ulid}.{ext}
  → 若 auto_webp=true 且是图片：转码为 WebP
  → 若 generate_thumb=true：生成缩略图
  → 写入磁盘 media/ 目录
  → 插入 media 数据库表
  → 触发 BuildTask::MediaChanged（复制到 public/media/）
  → 返回 { url, width, height, size }
```

### 13.2 图片处理（image crate）

```rust
pub async fn process_image(
    input:  &[u8],
    config: &MediaConfig,
) -> Result<ProcessedImage> {
    let img = image::load_from_memory(input)?;

    let webp_data = if config.auto_webp {
        // 转码为 WebP
        let encoder = webp::Encoder::from_image(&img)?;
        Some(encoder.encode(config.webp_quality as f32).to_vec())
    } else {
        None
    };

    let thumbnail = if config.generate_thumb {
        let thumb = img.thumbnail(config.thumb_width, u32::MAX);
        let mut buf = Vec::new();
        thumb.write_to(&mut Cursor::new(&mut buf), ImageFormat::WebP)?;
        Some(buf)
    } else {
        None
    };

    Ok(ProcessedImage {
        original:  webp_data.clone().unwrap_or_else(|| input.to_vec()),
        thumbnail: thumbnail,
        width:     img.width(),
        height:    img.height(),
    })
}
```

### 13.3 媒体库界面

媒体库 `/admin/media` 提供：
- 网格视图 / 列表视图切换
- 按类型筛选（图片 / 其他）
- 按月份筛选
- 搜索文件名
- 点击查看详情（URL、尺寸、大小、上传时间）
- 复制 URL / Markdown 语法
- 在文章编辑器中通过工具栏按钮打开媒体库选择器（iframe 弹窗），选择后自动插入 Markdown 语法

---

## 14. Lua 运行时与 API

### 14.1 沙箱设计

插件可以访问 **cblog 项目根目录**（`cblog.toml` 所在目录）下的所有文件，完整开放 Lua 标准库，**只移除系统命令执行能力**。

```
cblog/               ← 项目根，插件 io 操作的边界
├── cblog.toml
├── content/
├── themes/
├── plugins/
├── media/
└── public/
```

**开放的能力：**

| 能力 | 说明 |
|------|------|
| `io.*`（路径受限） | 完整文件读写，路径限制在项目根目录内 |
| `os.time / os.date / os.clock / os.difftime` | 时间函数 |
| `require / package` | 加载插件自身 lib/ 目录下的 Lua 模块 |
| `table / string / math / utf8` | 全部标准库 |
| `coroutine` | 协程 |
| `debug`（只读） | 调试信息 |
| `cblog.*` | cblog 注入的完整 API（见下节） |

**禁止的能力：**

| 能力 | 原因 |
|------|------|
| `os.execute(cmd)` | 执行 shell 命令 |
| `io.popen(cmd)` | 执行命令并读取输出 |
| `os.exit()` | 终止整个进程 |
| 项目根目录之外的路径 | 防止读取系统文件或写入任意位置 |
| 绝对路径（`/etc/...`） | 同上 |

```rust
let lua = Lua::new_with(LuaStdLib::ALL, LuaOptions::default())?;

let os: Table = lua.globals().get("os")?;
os.set("execute", LuaNil)?;
os.set("exit",    LuaNil)?;

// 替换 io 为路径受限版本
lua.globals().raw_set("io", build_safe_io(&lua, &project_root)?)?;

fn build_safe_io(lua: &Lua, project_root: &Path) -> LuaResult<Table> {
    let root = project_root.canonicalize()?;
    let io_table = lua.create_table()?;

    let root_c = root.clone();
    io_table.set("open", lua.create_function(move |lua, (path, mode): (String, Option<String>)| {
        // 拒绝绝对路径
        if Path::new(&path).is_absolute() {
            return Err(LuaError::RuntimeError(
                format!("不允许绝对路径：{}", path)
            ));
        }
        let full = root_c.join(&path);
        let canonical = full.canonicalize().unwrap_or(full);
        if !canonical.starts_with(&root_c) {
            return Err(LuaError::RuntimeError(
                format!("路径越界：{} 不在 cblog 项目根目录内", path)
            ));
        }
        let mode = mode.unwrap_or_else(|| "r".to_string());
        lua_file_open(lua, canonical, &mode)
    })?)?;

    // io.lines、io.read、io.write 同样经过路径检查
    // io.popen 不提供
    Ok(io_table)
}
```

### 14.2 cblog.* Lua API 完整参考

cblog 向所有 Lua 环境注入 `cblog` 全局对象，提供以下 API：

#### cblog.site

```lua
local site = cblog.site()
-- 返回站点配置对象：
-- site.title, site.url, site.language,
-- site.author.name, site.author.email 等
```

#### cblog.files

```lua
-- 写文件（相对于项目根）
cblog.files.write("public/my-file.txt", "内容")

-- 追加文件
cblog.files.append("public/my-file.txt", "追加内容")

-- 读文件
local content = cblog.files.read("content/posts/my-post.md")

-- 检查文件是否存在
local exists = cblog.files.exists("public/my-file.txt")

-- 删除文件
cblog.files.remove("public/my-file.txt")

-- 删除目录（递归）
cblog.files.remove_dir("public/my-plugin-assets/")

-- 创建目录
cblog.files.mkdir("public/my-dir/")

-- 列出目录内容
local files = cblog.files.list("content/posts/")
-- 返回：{ { name="2024-01.md", is_dir=false }, ... }

-- 复制文件
cblog.files.copy("plugins/my-plugin/assets/style.css", "public/assets/my-plugin.css")
```

#### cblog.store（插件/主题 KV 存储）

```lua
-- 存储值（JSON 序列化）
cblog.store.set("config", { key = "value" })
cblog.store.set("counter", 42)

-- 读取值
local config = cblog.store.get("config")  -- 返回 nil 若不存在

-- 删除
cblog.store.delete("counter")

-- 列出所有 key
local keys = cblog.store.keys()
```

#### cblog.db（插件独立数据库）

```lua
local db = cblog.db()  -- 返回插件的 SQLite 数据库连接

-- 执行 DDL/DML（无返回值）
db:exec("CREATE TABLE IF NOT EXISTS items (id TEXT PRIMARY KEY, val TEXT)")
db:exec("INSERT INTO items VALUES (?, ?)", "id1", "value1")

-- 查询多行
local rows = db:query("SELECT * FROM items WHERE val LIKE ?", "%foo%")
-- 返回：{ { id="id1", val="value1" }, ... }

-- 查询单行
local row = db:query_one("SELECT * FROM items WHERE id=?", "id1")

-- 统计查询
local count = db:count("SELECT COUNT(*) FROM items")

-- 事务
db:transaction(function()
    db:exec("INSERT INTO items VALUES (?, ?)", "id2", "val2")
    db:exec("UPDATE items SET val=? WHERE id=?", "new", "id1")
end)
```

#### cblog.render_template

```lua
-- 渲染一个 cbtml 模板为 HTML 字符串（用于插件生成 HTML 片段）
local html = cblog.render_template("plugins/comments/comment-section.cbtml", {
    post     = post,
    comments = comments,
    config   = cfg,
})
```

#### cblog.slugify / cblog.json / cblog.iso_date

```lua
local slug = cblog.slugify("Hello World! 你好")
-- → "hello-world-ni-hao"

local json_str = cblog.json({ key = "value", arr = {1, 2, 3} })
-- → '{"arr":[1,2,3],"key":"value"}'

local iso = cblog.iso_date(post.created_at)
-- → "2024-01-15T10:00:00+08:00"
```

#### cblog.log

```lua
cblog.log.debug("调试信息：%s", some_value)
cblog.log.info("插件初始化完成")
cblog.log.warn("配置项 %s 已废弃", key)
cblog.log.error("处理文章 %s 时出错：%s", post.slug, err)
-- 日志输出到 tracing subscriber，遵循 cblog.toml server.log_level 设置
```

#### cblog.http（仅 admin handler 期间可用）

```lua
-- 只在 plugin.on_admin_get / plugin.on_admin_post 回调中可用
-- 用于插件需要在后台页面中请求外部服务的场景

local resp = cblog.http.get("https://api.example.com/data", {
    headers = { ["Authorization"] = "Bearer " .. token }
})
-- resp.status, resp.body, resp.headers

local resp = cblog.http.post("https://api.example.com/submit", {
    body    = cblog.json(payload),
    headers = { ["Content-Type"] = "application/json" }
})
```

#### cblog.version_lt / cblog.version

```lua
local ver = cblog.version()  -- 返回当前 cblog 版本字符串，如 "0.5.1"

if cblog.version_lt(from_version, "1.1.0") then
    -- 执行升级迁移
end
```

### 14.3 Rust ↔ Lua 数据桥接

```rust
// Filter Hook：数据流经所有注册的处理器，每个处理器返回修改后的值
pub fn apply_filter<T>(&self, hook: &str, value: T) -> Result<T>
where
    T: Serialize + DeserializeOwned,
{
    let handlers = self.hook_registry.get_sorted(hook);
    let mut current = value;
    for (priority, handler) in handlers {
        // serde_json → mlua::Value（自动类型映射）
        let lua_val = self.lua.to_value(&current)
            .with_context(|| format!("序列化 filter '{}' 输入失败（priority={}）", hook, priority))?;

        let result = handler.call::<_, LuaValue>(lua_val)
            .with_context(|| format!("执行 filter '{}' 失败（priority={}）", hook, priority))?;

        current = self.lua.from_value(result)
            .with_context(|| format!("反序列化 filter '{}' 输出失败（priority={}）", hook, priority))?;
    }
    Ok(current)
}

// Action Hook：只传递数据，不关心返回值，用于副作用
pub fn call_action(&self, hook: &str, ctx: &BuildContext) -> Result<()> {
    let handlers = self.hook_registry.get_sorted(hook);
    let lua_ctx = self.lua.to_value(ctx)?;
    for (_priority, handler) in handlers {
        handler.call::<_, ()>(lua_ctx.clone())?;
    }
    Ok(())
}
```

### 14.4 插件独立存储

插件 KV 存储统一存在 SQLite 的 `plugin_store` 表，key 由框架自动加上插件名前缀防止冲突。主题使用相同机制，前缀为 `theme:{theme_name}`：

```
plugin_store 表：
┌───────────────────────┬──────────────────┬──────────────────────────┐
│  plugin_name          │  key             │  value (JSON)            │
├───────────────────────┼──────────────────┼──────────────────────────┤
│ seo-optimizer         │ config           │ {"auto_desc":true,...}   │
│ comments              │ config           │ {"moderation":true}      │
│ comments              │ spam_keywords    │ ["casino","poker"]       │
│ theme:aurora          │ menus            │ [{"label":"首页",...}]   │
└───────────────────────┴──────────────────┴──────────────────────────┘
```

---

## 15. 前台静态能力边界

前台访问的全部是静态文件，以下功能通过混合方案处理：

| 功能 | 方案 | 说明 |
|------|------|------|
| 全文搜索 | 构建期生成 JSON 索引 + 客户端 MiniSearch.js | 完全离线，无服务端参与 |
| 评论 | 插件生成 HTML（静态展示） + 提交到 Cloudflare Workers | 展示静态，写入走边缘函数 |
| 评论（简单方案） | Giscus（GitHub Discussions）嵌入 | 插件注入 script 标签 |
| 表单（联系我等） | Cloudflare Workers / Netlify Functions | 插件注入 endpoint URL |
| 访问统计 | Umami / Plausible 自托管 | 插件注入统计脚本，隐私友好 |
| RSS/Atom | 构建期生成 feed.xml | 纯静态 |
| Sitemap | 构建期生成 sitemap.xml | 纯静态 |
| 搜索（高级） | Algolia DocSearch / Meilisearch | 插件注入 API 配置 |
| 图片懒加载 | 构建期在 img 标签加 `loading="lazy"` | 纯静态，浏览器原生 |
| 代码高亮 | 构建期 syntect 生成带 class 的 HTML | 纯静态，零客户端 JS |

**搜索插件示例（构建期生成索引）：**

```lua
-- plugins/search/main.lua
plugin.action("build.finalize", 10, function(ctx)
    local index = {}
    for _, post in ipairs(ctx.posts) do
        table.insert(index, {
            id      = post.id,
            title   = post.title,
            url     = post.url,
            content = string.sub(cblog.strip_html(post.content), 1, 500),
            tags    = post.tags,
            date    = cblog.iso_date(post.created_at),
        })
    end

    cblog.files.write("public/search-index.json", cblog.json(index))

    -- 注入客户端 JS（MiniSearch）
    table.insert(ctx.global_scripts,
        '<script src="/assets/search.js" defer></script>'
    )
end)
```

---

## 16. 数据库设计

SQLite 单文件数据库，文件路径为 `{project_root}/cblog.db`。

### 16.1 核心表结构

```sql
-- ── 管理员账号 ────────────────────────────────────────────────────
CREATE TABLE users (
    id            TEXT PRIMARY KEY,        -- ULID
    username      TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,           -- argon2id hash
    created_at    TEXT NOT NULL,
    last_login_at TEXT
);

-- ── 文章 ──────────────────────────────────────────────────────────
CREATE TABLE posts (
    id         TEXT PRIMARY KEY,           -- ULID
    slug       TEXT UNIQUE NOT NULL,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,              -- 原始 Markdown
    status     TEXT NOT NULL DEFAULT 'draft',  -- draft|published|archived
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    meta       TEXT NOT NULL DEFAULT '{}'  -- JSON：Front Matter 扩展字段 + 插件写入的数据
);

CREATE INDEX idx_posts_status     ON posts(status);
CREATE INDEX idx_posts_created_at ON posts(created_at DESC);

-- ── 独立页面 ──────────────────────────────────────────────────────
CREATE TABLE pages (
    id         TEXT PRIMARY KEY,
    slug       TEXT UNIQUE NOT NULL,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'draft',
    template   TEXT,                       -- 自定义模板名，NULL 用默认
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- ── 媒体文件 ──────────────────────────────────────────────────────
CREATE TABLE media (
    id          TEXT PRIMARY KEY,          -- ULID
    filename    TEXT NOT NULL,             -- 存储文件名（含路径，相对于 media/）
    original_name TEXT NOT NULL,           -- 上传时的原始文件名
    mime_type   TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    width       INTEGER,                   -- 图片宽度，NULL 表示非图片
    height      INTEGER,
    url         TEXT NOT NULL,             -- 访问 URL（/media/2024/01/xxx.webp）
    thumb_url   TEXT,                      -- 缩略图 URL
    uploaded_at TEXT NOT NULL
);

CREATE INDEX idx_media_uploaded_at ON media(uploaded_at DESC);

-- ── 主题配置 ──────────────────────────────────────────────────────
CREATE TABLE theme_config (
    theme_name TEXT PRIMARY KEY,
    config     TEXT NOT NULL DEFAULT '{}'  -- JSON，key-value 对应 theme.toml schema
);

-- ── 插件/主题 KV 存储 ─────────────────────────────────────────────
CREATE TABLE plugin_store (
    plugin_name TEXT NOT NULL,             -- 插件名或 "theme:{name}"
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,             -- JSON
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (plugin_name, key)
);

-- ── 撤销的 JWT（用于即时登出） ────────────────────────────────────
CREATE TABLE revoked_tokens (
    jti        TEXT PRIMARY KEY,           -- JWT ID（jti claim）
    expires_at TEXT NOT NULL               -- token 过期时间，过期后可清理
);

-- ── 构建历史 ──────────────────────────────────────────────────────
CREATE TABLE build_history (
    id          TEXT PRIMARY KEY,
    trigger     TEXT NOT NULL,             -- manual|post_changed|theme_config|...
    status      TEXT NOT NULL,             -- running|success|failed
    total_pages INTEGER,
    rebuilt     INTEGER,
    cached      INTEGER,
    duration_ms INTEGER,
    error       TEXT,                      -- 失败时的错误信息
    started_at  TEXT NOT NULL,
    finished_at TEXT
);

CREATE INDEX idx_build_history_started ON build_history(started_at DESC);
```

### 16.2 数据库迁移

使用 sqlx 内置的 `migrate!` 宏管理迁移脚本，按序执行：

```
src/migrations/
├── 0001_init.sql           # 初始建表
├── 0002_add_media.sql      # 新增 media 表
├── 0003_add_build_history.sql
└── 0004_add_revoked_tokens.sql
```

程序启动时自动执行未应用的迁移：

```rust
sqlx::migrate!("src/migrations")
    .run(&pool)
    .await
    .expect("数据库迁移失败");
```

### 16.3 插件独立数据库

需要复杂关系型存储的插件（如评论插件）使用独立 SQLite 文件，路径为 `plugins/{name}/data.db`。插件通过 `cblog.db()` API 访问，框架在插件加载时建立连接，卸载时关闭并可选清理。

---

## 17. 错误处理与日志

### 17.1 错误分类与处理策略

| 错误类型 | 发生位置 | 处理策略 |
|---------|---------|---------|
| 构建期 Lua 插件报错 | content.transform 等阶段 | 记录错误，跳过该文章，继续构建其他页面 |
| cbtml 编译错误 | page.render 阶段 | 输出错误页（含行号/列号），标记该页面构建失败 |
| 数据库错误 | Admin API | 返回 500，记录详细错误日志 |
| 文件 IO 错误 | asset.process 等 | 记录错误，尽可能继续其他操作 |
| Lua 路径越界 | 所有 Lua IO 操作 | 立即返回 Lua RuntimeError，不中断整体构建 |
| JWT 验证失败 | Admin 请求 | 返回 401，重定向到登录页 |
| CSRF 校验失败 | Admin 表单提交 | 返回 403 |
| 媒体上传文件类型不合法 | /admin/media/upload | 返回 400 + 具体原因 |

### 17.2 tracing 日志配置

```rust
// 结构化日志，支持 JSON 输出（生产）和人类可读输出（开发）
tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::new(&config.server.log_level)
    )
    .with(if is_production {
        tracing_subscriber::fmt::layer().json().boxed()
    } else {
        tracing_subscriber::fmt::layer().pretty().boxed()
    })
    .init();
```

关键日志事件（均使用结构化字段便于检索）：

```rust
// 构建完成
tracing::info!(
    rebuilt  = rebuilt_count,
    cached   = cached_count,
    duration = duration_ms,
    "构建完成"
);

// 插件错误
tracing::error!(
    plugin = plugin_name,
    hook   = hook_name,
    post   = post_slug,
    error  = %err,
    "插件 Hook 执行失败"
);

// 管理员操作
tracing::info!(
    user   = username,
    action = "publish_post",
    post   = post_id,
    "文章已发布"
);
```

### 17.3 构建失败处理

构建失败时，不删除 `public/` 目录中的旧文件，保持上一次成功构建的结果继续对外服务。失败信息通过 WebSocket 推送到后台，并记录到 `build_history` 表。

---

## 18. 部署

### 18.1 目录结构（生产服务器）

```
/opt/cblog/
├── cblog                    # 编译好的二进制文件
├── cblog.toml               # 站点配置（含 JWT secret，权限 600）
├── cblog.db                 # SQLite 数据库
├── content/                 # 内容文件（建议用 Git 管理）
├── themes/
├── plugins/
├── media/
├── public/                  # 静态文件输出目录
└── .cblog-cache/            # 构建缓存
```

### 18.2 Nginx 配置

```nginx
server {
    listen 80;
    listen 443 ssl http2;
    server_name example.com;

    # SSL 配置（Let's Encrypt）
    ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;

    # HTTP → HTTPS 重定向
    if ($scheme != "https") {
        return 301 https://$host$request_uri;
    }

    root /opt/cblog/public;
    index index.html;

    # ── 静态文件伺服（前台，直接返回） ────────────────────────────
    location / {
        try_files $uri $uri/ $uri/index.html =404;

        # 静态资源长缓存
        location ~* \.(css|js|webp|jpg|png|gif|woff2|ico)$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }

        # HTML 文件不缓存（或短缓存），确保构建后访客能拿到新内容
        location ~* \.html$ {
            expires 5m;
            add_header Cache-Control "public, must-revalidate";
        }
    }

    # ── 动态后台（代理到 Axum） ────────────────────────────────────
    location /admin {
        proxy_pass         http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;

        # WebSocket 支持（构建状态推送）
        proxy_set_header   Upgrade    $http_upgrade;
        proxy_set_header   Connection "upgrade";
        proxy_read_timeout 3600s;

        # 媒体上传：允许较大 body
        client_max_body_size 20M;
    }

    # ── Gzip 压缩 ─────────────────────────────────────────────────
    gzip on;
    gzip_types text/html text/css application/javascript application/json
               application/xml image/svg+xml;
    gzip_min_length 1024;

    # ── 自定义 404 页面 ───────────────────────────────────────────
    error_page 404 /404.html;
}
```

### 18.3 systemd Service

```ini
# /etc/systemd/system/cblog.service

[Unit]
Description=cblog Blog Engine
After=network.target

[Service]
Type=simple
User=cblog
Group=cblog
WorkingDirectory=/opt/cblog
ExecStart=/opt/cblog/cblog serve
Restart=on-failure
RestartSec=5s

# 环境变量（敏感配置也可通过此处注入，而不写死在 cblog.toml）
# Environment="CBLOG_JWT_SECRET=your_secret_here"

# 资源限制
LimitNOFILE=65536

# 安全加固（可选）
NoNewPrivileges=yes
ProtectSystem=strict
ReadWritePaths=/opt/cblog
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
```

```bash
# 部署步骤
sudo useradd -r -s /bin/false cblog
sudo mkdir -p /opt/cblog
sudo chown cblog:cblog /opt/cblog
sudo chmod 700 /opt/cblog

# 上传二进制和配置
sudo cp cblog /opt/cblog/
sudo cp cblog.toml /opt/cblog/
sudo chmod 600 /opt/cblog/cblog.toml  # 保护 JWT secret

sudo systemctl daemon-reload
sudo systemctl enable --now cblog
sudo systemctl status cblog
```

### 18.4 CLI 命令

```bash
# 启动后台服务（同时提供 /admin 和 /build 能力）
cblog serve

# 全量构建（不启动服务，适合 CI/CD 部署）
cblog build

# 增量构建（仅重建变更内容）
cblog build --incremental

# 强制全量重建（清除缓存）
cblog build --clean

# 开发模式（监听文件变更，自动重建 + 浏览器热重载）
cblog dev

# 初始化新项目
cblog init my-blog

# 创建管理员账号（首次部署时使用）
cblog user create --username admin --password your_password

# 修改密码
cblog user passwd admin

# 检查主题/插件是否有语法错误
cblog check

# 查看构建统计
cblog stats
```

### 18.5 Docker 部署（可选）

```dockerfile
# Dockerfile
FROM rust:1.78 AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/cblog
COPY --from=builder /build/target/release/cblog .
EXPOSE 3000
CMD ["./cblog", "serve"]
```

```yaml
# docker-compose.yml
services:
  cblog:
    build: .
    ports:
      - "127.0.0.1:3000:3000"
    volumes:
      - ./cblog.toml:/opt/cblog/cblog.toml:ro
      - ./content:/opt/cblog/content
      - ./themes:/opt/cblog/themes
      - ./plugins:/opt/cblog/plugins
      - ./media:/opt/cblog/media
      - cblog_data:/opt/cblog/cblog.db
      - cblog_public:/opt/cblog/public

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/conf.d/default.conf:ro
      - cblog_public:/usr/share/nginx/html:ro
      - ./ssl:/etc/ssl/certs:ro

volumes:
  cblog_data:
  cblog_public:
```

---

## 19. 备份与恢复

### 19.1 需要备份的数据

| 数据 | 位置 | 备份频率建议 |
|------|------|------------|
| 数据库 | `cblog.db` | 每日 |
| 内容文件 | `content/` | 每次变更（建议用 Git） |
| 媒体文件 | `media/` | 每日或每次上传后 |
| 配置文件 | `cblog.toml` | 每次变更 |
| 插件数据 | `plugins/*/data.db` | 每日 |

`public/` 目录不需要备份，可从数据和内容重新构建生成。

### 19.2 备份脚本

```bash
#!/bin/bash
# /opt/cblog/scripts/backup.sh

BACKUP_DIR="/backup/cblog"
DATE=$(date +%Y%m%d_%H%M%S)
DEST="$BACKUP_DIR/$DATE"

mkdir -p "$DEST"

# SQLite 数据库（使用 .backup 命令保证一致性）
sqlite3 /opt/cblog/cblog.db ".backup '$DEST/cblog.db'"

# 插件数据库
for db in /opt/cblog/plugins/*/data.db; do
    plugin_name=$(basename $(dirname $db))
    sqlite3 "$db" ".backup '$DEST/plugin_${plugin_name}.db'"
done

# 媒体文件
rsync -a /opt/cblog/media/ "$DEST/media/"

# 配置文件
cp /opt/cblog/cblog.toml "$DEST/"

# 清理 30 天前的备份
find "$BACKUP_DIR" -maxdepth 1 -type d -mtime +30 -exec rm -rf {} \;

echo "备份完成：$DEST"
```

```bash
# crontab 每日凌晨 3 点执行
0 3 * * * /opt/cblog/scripts/backup.sh >> /var/log/cblog-backup.log 2>&1
```

### 19.3 恢复流程

```bash
# 1. 停止服务
sudo systemctl stop cblog

# 2. 恢复数据库
cp /backup/cblog/20240115_030000/cblog.db /opt/cblog/cblog.db

# 3. 恢复媒体文件
rsync -a /backup/cblog/20240115_030000/media/ /opt/cblog/media/

# 4. 启动服务
sudo systemctl start cblog

# 5. 触发全量重建（public/ 目录从数据重建）
/opt/cblog/cblog build --clean
```

---

## 20. 开发路线图

### Phase 1 — 基础 SSG（MVP）
- [ ] pulldown-cmark Markdown 解析，Front Matter 提取
- [ ] cbtml 编译器核心：Lexer → Parser → DOM AST → CodeGen
- [ ] MiniJinja 作为表达式求值后端
- [ ] 基础构建管道：load → parse → render → output
- [ ] 全量静态文件输出
- [ ] 内置过滤器：date、slugify、truncate、safe、default、length

### Phase 2 — cbtml 完善
- [ ] extends / slot 继承系统
- [ ] include 片段引入
- [ ] 跨主题继承（`extends theme_name:template`）
- [ ] 剩余内置过滤器：reading_time、wordcount、abs_url、json 等
- [ ] 自定义过滤器注册 API
- [ ] 友好编译错误（行列号 + 上下文 + 修复建议）
- [ ] cbtml 语法高亮（VS Code 插件）

### Phase 3 — 后台管理
- [ ] Axum Web 服务启动
- [ ] SQLite + sqlx 集成，数据库迁移系统
- [ ] 文章/页面 CRUD API
- [ ] 后台 UI（cbtml 模板，简洁实用）
- [ ] JWT 认证，argon2 密码哈希
- [ ] CSRF 防护
- [ ] 文章编辑器（CodeMirror + Markdown 预览）
- [ ] 媒体上传（图片压缩、WebP 转码）

### Phase 4 — 主题系统
- [ ] theme.toml schema 解析与验证
- [ ] 主题配置后台页面（表单自动生成）
- [ ] 子主题继承解析
- [ ] SCSS 编译（grass）
- [ ] 资源管道（CSS/JS 复制）
- [ ] 主题热切换
- [ ] 主题后台页面注册与挂载

### Phase 5 — 插件系统
- [ ] mlua 集成，Lua VM 初始化
- [ ] IO 路径限制沙箱
- [ ] cblog.* Lua API 完整实现
- [ ] Hook 注册与执行引擎（filter/action，优先级排序）
- [ ] 插件能力声明解析
- [ ] 拓扑排序 + 并行调度
- [ ] 冲突检测（前置启动时报告，不等到构建时发现）
- [ ] 插件 KV 存储
- [ ] 插件独立 SQLite 数据库
- [ ] 插件后台页面动态路由挂载
- [ ] 编辑器侧边栏面板注入
- [ ] 插件生命周期（install / uninstall / upgrade）

### Phase 6 — 增量构建与性能
- [ ] SHA-256 内容哈希缓存（持久化到 .cblog-cache/）
- [ ] 模板依赖图（构建 + 持久化）
- [ ] 脏页检测算法
- [ ] rayon 并行页面渲染
- [ ] 构建任务队列（tokio channel）
- [ ] WebSocket 实时构建进度推送
- [ ] 构建历史记录

### Phase 7 — 内置插件
- [ ] `sitemap`：生成 sitemap.xml（支持 lastmod、changefreq、priority）
- [ ] `feed`：生成 RSS 2.0 + Atom 1.0
- [ ] `search`：客户端搜索索引（MiniSearch 格式）
- [ ] `seo`：自动 Open Graph、JSON-LD、canonical
- [ ] `syntax-highlight`：syntect 代码高亮（构建期，零客户端 JS）
- [ ] `toc`：文章目录生成（支持多级、平滑滚动高亮）
- [ ] `image-optimize`：图片懒加载属性、WebP src-set 生成

### Phase 8 — 生产加固
- [ ] 速率限制（登录接口）
- [ ] JWT 撤销（即时登出）
- [ ] 完善日志（tracing，结构化 JSON 输出）
- [ ] 健康检查接口（`/health`）
- [ ] `cblog check` 命令（静态分析主题和插件）
- [ ] Docker 镜像和 Compose 配置
- [ ] 完整文档网站

---

*cblog — 构建期做复杂的事，运行时只伺服静态文件。*
