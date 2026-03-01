# cblog-dev-theme

cblog 主题开发技能。当用户需要为 cblog 创建、修改或调试主题时使用此技能。

---

## 一、cblog 主题系统概述

cblog 是一个 **Rust + Lua 博客引擎**，采用"后台动态，前台全静态 SSG"架构。主题系统负责控制前台静态站点的外观和行为，同时也支持覆盖后台管理面板的模板。

**核心技术栈：**

| 技术 | 用途 |
|------|------|
| CBTML | cblog 自研模板 DSL（编译为 MiniJinja） |
| MiniJinja | 模板渲染运行时 |
| SCSS (grass) | 样式编译 |
| rayon | 并行页面渲染 |

**关键源码文件：**

| 文件 | 职责 |
|------|------|
| `src/theme/config.rs` | 主题配置核心（ThemeToml、ConfigField、ResolvedTheme 等结构体） |
| `src/cbtml.rs` | CBTML 模块入口，暴露 `compile()` 函数 |
| `src/cbtml/lexer.rs` | CBTML 词法分析器 |
| `src/cbtml/parser.rs` | CBTML 语法解析器（Token → AST） |
| `src/cbtml/codegen.rs` | CBTML 代码生成（AST → MiniJinja 模板字符串） |
| `src/cbtml/filters.rs` | 内置过滤器注册 |
| `src/cbtml/error.rs` | 错误类型与格式化 |
| `src/build/stages/render.rs` | 前台模板编译与渲染 |
| `src/build/stages/generate.rs` | 页面生成（构建模板上下文数据） |
| `src/build/stages/assets.rs` | SCSS/CSS/JS 资源处理 |
| `src/admin/theme.rs` | 后台主题设置/切换 handler |
| `src/admin/template.rs` | 后台模板渲染环境 |
| `src/repository/build.rs` | 主题配置数据库操作 |

---

## 二、主题目录结构

一个完整的 cblog 主题目录结构如下：

```
themes/<theme-name>/
  theme.toml                          -- 主题配置文件（必须）
  templates/                          -- 模板目录
    base.cbtml                        -- 基础布局（必须）
    index.cbtml                       -- 首页/文章列表页（必须）
    post.cbtml                        -- 文章详情页（必须）
    page.cbtml                        -- 独立页面
    tag.cbtml                         -- 标签归档页
    category.cbtml                    -- 分类归档页
    archive.cbtml                     -- 时间归档页
    404.cbtml                         -- 404 页面
    partials/                         -- 可复用片段
      nav.cbtml                       -- 导航栏
      footer.cbtml                    -- 页脚
      post-card.cbtml                 -- 文章卡片
      pagination.cbtml                -- 分页组件
    admin/                            -- 后台管理模板覆盖（可选）
      base.cbtml                      -- 后台基础布局
      theme.cbtml                     -- 主题设置页
      ... (其他后台页面)
  assets/                             -- 静态资源
    scss/                             -- SCSS 源文件
      main.scss                       -- SCSS 入口（编译为 main.css）
      _variables.scss                 -- SCSS 变量定义
    css/                              -- CSS 文件（直接复制到 public/assets/）
    js/                               -- JavaScript 文件（直接复制到 public/assets/）
      main.js                         -- 前台 JS
    admin/                            -- 后台静态资源（可选覆盖）
      admin.css                       -- 后台 CSS
      editor.js                       -- 后台编辑器 JS
```

**关键规则：**
- 主题**必须**有 `theme.toml` 文件，否则不会被识别
- 模板文件使用 `.cbtml` 扩展名
- 最小可用主题只需要：`theme.toml` + `templates/base.cbtml` + `templates/index.cbtml` + `templates/post.cbtml`
- SCSS 入口文件必须命名为 `main.scss`，编译输出为 `public/assets/main.css`
- `_variables.scss` 中的变量使用 `!default` 标记，允许被主题配置覆盖

---

## 三、theme.toml 配置文件格式

### 3.1 完整格式

```toml
[theme]
name = "my-theme"             # 必填，主题名称
version = "1.0.0"             # 版本号
author = "作者名"              # 作者
description = "主题描述"       # 描述
homepage = "https://..."       # 主页 URL
parent = "aurora"              # 可选，父主题名称（子主题继承）

# 配置项以 [[config]] TOML 数组定义，每项一个配置字段
[[config]]
key     = "primary_color"      # 唯一标识（也作为 SCSS 变量名，_ 转为 -）
type    = "color"              # 字段类型
label   = "主色调"             # 后台显示标签
default = "#6366f1"            # 默认值
group   = "外观"               # 分组名（后台按组显示）

[[config]]
key     = "posts_per_page"
type    = "number"
label   = "每页文章数"
default = 10
group   = "布局"
min     = 1                    # number 类型的最小值
max     = 50                   # number 类型的最大值

[[config]]
key     = "show_reading_time"
type    = "boolean"
label   = "显示阅读时间"
default = true
group   = "功能"

[[config]]
key     = "dark_mode"
type    = "select"
label   = "暗黑模式"
default = "auto"
group   = "外观"
options = [
  { value = "auto", label = "跟随系统" },
  { value = "light", label = "浅色" },
  { value = "dark", label = "深色" }
]

[[config]]
key     = "font_body"
type    = "font_select"
label   = "正文字体"
default = "system-ui"
group   = "外观"
options = ["system-ui", "Georgia", "Merriweather", "Lora"]

[[config]]
key     = "footer_text"
type    = "textarea"
label   = "页脚文本"
default = "Powered by cblog"
group   = "高级"

[[config]]
key     = "custom_css"
type    = "code"
label   = "自定义 CSS"
default = ""
group   = "高级"
language = "css"               # code 类型的语言标识

[[config]]
key     = "custom_head_html"
type    = "richtext"
label   = "自定义 Head HTML"
default = ""
group   = "高级"

[[config]]
key     = "logo_url"
type    = "image"
label   = "Logo 图片"
default = ""
group   = "外观"

[[config]]
key     = "sidebar_enabled"
type    = "boolean"
label   = "启用侧边栏"
default = true
group   = "布局"

[[config]]
key     = "sidebar_widgets"
type    = "select"
label   = "侧边栏组件"
default = "tags"
group   = "布局"
depends_on = "sidebar_enabled"  # 当 sidebar_enabled 为 true 时才显示
options = [
  { value = "tags", label = "标签云" },
  { value = "recent", label = "最近文章" },
  { value = "both", label = "全部显示" }
]

[[config]]
key     = "description"
type    = "text"
label   = "自定义描述"
default = ""
group   = "外观"
description = "在这里输入自定义描述文字"  # 配置项的说明文字
```

### 3.2 支持的字段类型（field_type）

| 类型 | HTML 表单控件 | 说明 |
|------|--------------|------|
| `color` | `<input type="color">` | 颜色选择器 |
| `number` | `<input type="number">` | 数字输入，支持 `min`/`max` |
| `boolean` | `<input type="checkbox">` | 开关 |
| `select` | `<select>` | 下拉选择 |
| `font_select` | `<select>` | 字体选择（同 select） |
| `text` | `<input type="text">` | 文本输入（默认） |
| `textarea` | `<textarea>` | 多行文本 |
| `richtext` | `<textarea>` | 富文本（支持 HTML） |
| `code` | `<textarea class="code">` | 代码编辑器，支持 `language` 属性 |
| `image` | `<input type="text">` | 图片 URL |

### 3.3 高级特性

- **`depends_on`**: 字段可声明依赖另一个 boolean 字段，后台通过 JS 动态显示/隐藏
- **`options`**: select/font_select 支持两种格式：
  - 简单字符串：`options = ["system-ui", "Georgia"]`
  - 键值对：`options = [{ value = "auto", label = "跟随系统" }]`
- **`description`**: 可选，配置项的说明文字

### 3.4 配置值的存储

- 数据库表 `theme_config`，每个主题一行，`config` 字段存储所有配置的 JSON 对象
- 保存配置时自动触发站点重建

---

## 四、CBTML 模板语法完整参考

CBTML 是 cblog 自研的模板 DSL，编译流程为：
```
CBTML 源码 → Lexer(词法分析) → Parser(语法解析) → AST → Codegen(代码生成) → MiniJinja 模板字符串
```

### 4.1 HTML 元素声明

语法：`tag.class1.class2#id [attr="val"] [attr2={{ expr }}] 内联文本`

```
div.post-card
  h2.title
    a [href="{{ post.url }}"] {{ post.title }}
```

编译为：
```html
<div class="post-card"><h2 class="title"><a href="{{ post.url }}">{{ post.title }}</a></h2></div>
```

**缩进规则：**
- 使用 **2 空格缩进** 表示父子关系（严格 2 空格为 1 级）
- 缩进更深的行自动成为上一行元素的子节点

**属性语法：**
- 静态属性：`[href="/path"]`
- 动态属性：`[href="{{ post.url }}"]`
- 布尔属性：`[checked]`（无值）
- 多属性块：`[attr1="val1"] [attr2="val2"]`
- 支持单引号和双引号

**自闭合（Void）元素** 自动识别，不生成闭合标签：
`meta`, `link`, `input`, `br`, `hr`, `img`, `source`, `area`, `base`, `col`, `embed`, `track`, `wbr`

### 4.2 变量输出

**转义输出（防 XSS）：**
```
{{ expr }}
```

可以独立一行使用，也可以嵌入元素的内联文本：
```
h1 {{ post.title }}
span 共 {{ pagination.total_posts }} 篇
```

**不转义输出（原始 HTML）：**
```
raw expr
```
编译为 `{{ expr | safe }}`。用于输出 HTML 内容如文章正文、TOC、自定义 HTML。

### 4.3 条件指令

```
if condition
  ...
else if another_condition
  ...
else
  ...
end
```

支持复合条件表达式：
```
if pagination and pagination.total_pages > 1
if config.show_reading_time
if post.cover_image
```

### 4.4 循环指令

```
for item in collection
  ...
end
```

示例：
```
for post in posts
  include partials/post-card
end

for tag in post.tags
  a.tag [href="{{ tag | tag_url }}"] {{ tag }}
end
```

### 4.5 模板继承（extends + slot）

**父模板** 定义插槽：
```
slot content
  默认内容（可选）
```

**子模板** 首行声明继承并填充插槽：
```
extends base
slot content
  div.post-list
    for post in posts
      include partials/post-card
    end
```

**跨主题继承** 使用 `theme:template` 语法：
```
extends aurora:base
```
编译为 `{% extends "aurora/base.cbtml" %}`。

**继承规则：**
- `extends` 指令只能出现在文件第一行
- 子模板用 `slot name` 覆盖父模板中同名 slot 的默认内容
- 支持多级继承

### 4.6 模板包含

```
include partials/nav
```
编译为 `{% include "partials/nav.cbtml" %}`。自动追加 `.cbtml` 扩展名。

### 4.7 注释

```
{# 这是一条注释 #}
```

支持多行：
```
{#
  多行注释
  编译时完全移除
#}
```

### 4.8 原生块（style / script）

`style` 和 `script` 是特殊标签，内部内容不经 CBTML 解析，直接作为原始文本输出：

```
style
  body { margin: 0; }
  .container { max-width: 1200px; }

script
  console.log('hello');
```

带属性的 script 标签：
```
script [src="/assets/main.js"] [defer]
```

### 4.9 Hook 调用

```
hook("hook_name")
hook("hook_name", data)
```
编译为 `{{ hook("hook_name") }}`。用于插件系统在模板中注入内容。

### 4.10 纯文本行

使用 `|` 前缀表示纯文本（不作为标签解析）：
```
| 这是纯文本内容
```

不匹配任何指令的行也被当作纯文本直接输出。

### 4.11 指令编译对照表

| CBTML 指令 | 编译结果 |
|-----------|---------|
| `tag.class#id [attr="val"] text` | `<tag class="class" id="id" attr="val">text</tag>` |
| `{{ expr }}` | `{{ expr }}` |
| `raw expr` | `{{ expr \| safe }}` |
| `if cond` ... `end` | `{% if cond %}...{% endif %}` |
| `else if cond` | `{% elif cond %}` |
| `else` | `{% else %}` |
| `for x in y` ... `end` | `{% for x in y %}...{% endfor %}` |
| `extends base` | `{% extends "base.cbtml" %}` |
| `extends theme:base` | `{% extends "theme/base.cbtml" %}` |
| `slot name` | `{% block name %}...{% endblock name %}` |
| `include path` | `{% include "path.cbtml" %}` |
| `hook("name", data)` | `{{ hook("name", data) }}` |
| `style` / `script` | `<style>...</style>` / `<script>...</script>` |
| `{# comment #}` | （完全移除） |

---

## 五、内置过滤器

定义在 `src/cbtml/filters.rs`，通过 `register_filters()` 注册到 MiniJinja Environment。

| 过滤器 | 参数 | 说明 | 示例 |
|--------|------|------|------|
| `date` | `format?` | 日期格式化，默认 `%Y年%m月%d日` | `{{ post.created_at \| date }}` |
| `iso` | 无 | RFC 3339 格式日期 | `{{ post.created_at \| iso }}` |
| `slugify` | 无 | URL 友好化（小写+连字符） | `{{ title \| slugify }}` |
| `truncate` | `length?` | 截断文本，默认 160 字符，超出 `...` | `{{ post.content \| truncate(100) }}` |
| `wordcount` | 无 | 字数统计 | `{{ post.content \| wordcount }}` |
| `reading_time` | 无 | 阅读时间（分钟），200 字/分钟 | `{{ post.content \| reading_time }}` |
| `reading_time_label` | 无 | 中文阅读标签，如"约 3 分钟阅读" | `{{ post.reading_time \| reading_time_label }}` |
| `tag_url` | 无 | 生成标签 URL `/tags/{slug}/` | `{{ tag \| tag_url }}` |
| `category_url` | 无 | 生成分类 URL `/category/{slug}/` | `{{ cat \| category_url }}` |
| `json` | 无 | JSON 序列化 | `{{ data \| json }}` |
| `active_class` | 无 | 真值→`"active"`，假值→`""` | `[class="{{ is_home \| active_class }}"]` |
| `md5` | 无 | MD5 哈希（十六进制） | `{{ email \| md5 }}` |
| `upper` | 无 | 转大写 | `{{ text \| upper }}` |
| `lower` | 无 | 转小写 | `{{ text \| lower }}` |
| `capitalize` | 无 | 首字母大写 | `{{ text \| capitalize }}` |
| `abs_url` | 无 | 拼接站点 URL 生成绝对路径 | `{{ "/about/" \| abs_url }}` |
| `default` | `value` | MiniJinja 内置，默认值 | `{{ config.x \| default("fallback") }}` |
| `safe` | 无 | MiniJinja 内置，不转义（raw 自动使用） | `{{ html \| safe }}` |

**后台专用过滤器**（`src/admin/template.rs` 额外注册）：
- `format_datetime`: RFC3339 → `YYYY-MM-DD HH:MM:SS`

**后台专用全局函数：**
- `svg_icon("name")`: 返回对应名称的 SVG 图标 HTML

---

## 六、模板上下文变量

### 6.1 全局变量（所有前台页面共享）

注入位置：`src/build/stages/render.rs`

**`site` 对象：**
```
site.title          -- 站点标题
site.subtitle       -- 站点副标题
site.description    -- 站点描述
site.url            -- 站点 URL（如 https://example.com）
site.language       -- 站点语言（如 zh-CN）
site.author.name    -- 作者名称
site.author.email   -- 作者邮箱
site.author.avatar  -- 作者头像 URL
site.author.bio     -- 作者简介
```

**`config` 对象（主题配置值）：**
```
config.*            -- theme.toml 中定义的所有配置项
                       例如 config.primary_color, config.dark_mode, config.show_reading_time
```

### 6.2 首页/列表页（index.cbtml）

```
posts                       -- 当前页的文章列表（数组，每项结构见下方 post_to_ctx）

pagination.current          -- 当前页码
pagination.total_pages      -- 总页数
pagination.total_posts      -- 总文章数
pagination.prev             -- 上一页 URL（null 表示无）— 页码更小的方向
pagination.next             -- 下一页 URL（null 表示无）— 页码更大的方向

page.title                  -- 页面 SEO 标题
page.description            -- 页面描述
page.url                    -- 当前页面 URL
page.type                   -- "index"
```

### 6.3 文章详情页（post.cbtml）

```
post.id                     -- 文章 ID (ULID)
post.slug                   -- URL slug
post.title                  -- 文章标题
post.content                -- HTML 正文
post.excerpt                -- 摘要
post.cover_image            -- 封面图 URL
post.created_at             -- 创建时间 (RFC3339)
post.updated_at             -- 更新时间 (RFC3339)
post.tags                   -- 标签列表（字符串数组）
post.category               -- 分类名称
post.author                 -- 作者
post.reading_time           -- 阅读时间（分钟）
post.word_count             -- 字数
post.toc                    -- 目录 HTML
post.url                    -- 文章 URL

prev_post                   -- 更旧的一篇（时间上更早，可能为 null）
next_post                   -- 更新的一篇（时间上更近，可能为 null）

page.title                  -- 页面 SEO 标题（= 文章标题）
page.description            -- 页面描述（= excerpt）
page.url                    -- 当前页面 URL
page.type                   -- "post"
```

### 6.4 标签归档页（tag.cbtml）

```
tag                         -- 标签名称
posts                       -- 该标签下的文章列表
page.title                  -- "标签：{tag}"
page.type                   -- "tag"
```

### 6.5 分类归档页（category.cbtml）

```
category                    -- 分类名称
posts                       -- 该分类下的文章列表
page.title                  -- "分类：{category}"
page.type                   -- "category"
```

### 6.6 时间归档页（archive.cbtml）

```
year                        -- 年份（整数）
month                       -- 月份（整数）
posts                       -- 该月的文章列表
page.title                  -- "{year}年{month}月"
page.type                   -- "archive"
```

### 6.7 文章对象结构（post_to_ctx 生成）

列表页 `posts` 数组中每个元素、`post` 单篇文章、`prev_post`/`next_post` 均使用相同结构：
```
post.id                     -- ULID 字符串
post.slug                   -- URL slug
post.title                  -- 文章标题
post.url                    -- "/posts/{slug}/"
post.content                -- 完整 HTML
post.excerpt                -- 摘要文本
post.cover_image            -- 封面图 URL
post.created_at             -- RFC3339 时间
post.updated_at             -- RFC3339 时间
post.tags                   -- 标签数组
post.category               -- 分类名
post.author                 -- 作者
post.reading_time           -- 阅读分钟数
post.word_count             -- 字数
post.toc                    -- 目录 HTML
```

### 6.8 后台模板上下文

基础上下文（`src/admin/template.rs` 的 `build_admin_context()`）：
```
page_title                  -- 页面标题
site_title                  -- 站点标题
site_url                    -- 站点 URL
sidebar_groups              -- 侧边栏菜单组数组
  [].label                  -- 分组标签
  [].items[].label          -- 菜单项标签
  [].items[].href           -- 菜单项链接
  [].items[].icon           -- 图标名称
  [].items[].active         -- 是否当前激活
plugin_sidebar_items        -- 插件注册的侧边栏项目
profile_active              -- 个人设置页是否激活
wide_content                -- 是否使用宽内容布局
```

---

## 七、静态资源管理

### 7.1 SCSS 编译流程

处理位置：`src/build/stages/assets.rs`

1. 从 `theme.toml` 的 `[[config]]` 提取配置 schema
2. 合并默认值和数据库已保存值（`effective_values()`）
3. 调用 `build_scss_overrides()` 生成 SCSS 变量覆盖
   - 配置键名中的下划线 `_` 转为连字符 `-`
   - 例如 `primary_color` → `$primary-color: #6366f1;`
4. 将 `@use "variables" as *;` 替换为 `_variables.scss` 的完整内容（内联）
5. 将覆盖变量拼接到源码前面（覆盖 `!default` 值）
6. 使用 `grass` 库编译 SCSS → CSS
7. 输出到 `public/assets/main.css`

### 7.2 SCSS 变量与主题配置联动

在 `_variables.scss` 中用 `!default` 声明变量：
```scss
$primary-color: #6366f1 !default;
$font-body: system-ui !default;
$border-radius: 8px !default;
```

`theme.toml` 中 key 为 `primary_color` 的配置项会自动生成 `$primary-color: <用户设置值>;`，覆盖 `!default` 默认值。

### 7.3 CSS/JS 直接复制

- `themes/<name>/assets/css/*.css` → `public/assets/`
- `themes/<name>/assets/js/*.js` → `public/assets/`

### 7.4 后台静态资源覆盖

后台 CSS 和 JS 有双重回退机制（`src/admin.rs`）：
- 先检查主题目录 `themes/<active>/assets/admin/admin.css`
- 不存在则使用编译内嵌的默认版本（`include_str!`）
- 路由：`/admin/static/admin.css` 和 `/admin/static/editor.js`

---

## 八、子主题继承

### 8.1 声明父主题

在 `theme.toml` 的 `[theme]` 段设置 `parent`：
```toml
[theme]
name = "my-child-theme"
parent = "aurora"
```

### 8.2 继承机制

`resolve_theme()` 处理继承链：
- 检测循环继承
- 从根主题（链尾）向子主题（链首）合并配置字段
- 子主题可覆盖父主题同 key 配置项的 default 值

### 8.3 跨主题模板继承

```
extends aurora:base
```
编译为 `{% extends "aurora/base.cbtml" %}`，引用命名空间模板。

模板命名空间系统（`src/build/stages/render.rs`）：
- 当前活跃主题模板以短名注册（如 `base.cbtml`）
- 所有主题模板同时以命名空间全名注册（如 `aurora/base.cbtml`）

---

## 九、主题加载与切换

### 9.1 加载流程

1. **启动时**（`AppState::new()`）：从 `cblog.toml` 读取 `[theme] active = "aurora"` 确定活跃主题
2. **构建时**（`pipeline::execute()`）：
   - `compile_all_templates()` 编译所有主题的模板
   - 当前主题以短名注册，所有主题以命名空间注册
   - 加载主题配置并注入到模板上下文的 `config` 变量

### 9.2 切换机制

后台 handler `switch_theme()`（`src/admin/theme.rs`）：
1. 验证目标主题的 `theme.toml` 存在
2. 直接修改 `cblog.toml` 文件中的 `active = "xxx"` 行
3. 触发后台异步构建

### 9.3 配置保存

后台 handler `save_theme_settings()`：
1. 解析表单数据
2. 根据 schema 的 field_type 进行类型转换（boolean/number/string）
3. 序列化为 JSON 存入 `theme_config` 表
4. 自动触发构建

---

## 十、Hook 系统（与插件交互）

### 10.1 模板中使用 Hook

```
hook("head_extra")
hook("after_post_content", post)
```

### 10.2 构建管道 Hook 调用点

| Hook 名称 | 触发时机 | 上下文数据 |
|-----------|---------|-----------|
| `after_load` | 文章加载后 | `{ project_root, posts }` |
| `after_taxonomy` | 分类索引构建后 | `{ project_root, output_dir, tag_count, category_count, archive_count }` |
| `after_render` | 页面渲染后 | `{ project_root, output_dir }` |
| `after_assets` | 资源处理后 | `{ project_root, output_dir }` |
| `after_finalize` | 构建完成后 | `{ project_root, output_dir, posts, site_url }` |

---

## 十一、前台 URL 结构

cblog 是 SSG，构建时生成静态 HTML 写入 `public/` 目录：

| 页面类型 | URL 格式 | 使用的模板 |
|---------|---------|-----------|
| 首页 | `/` | `index.cbtml` |
| 分页 | `/page/{n}/` | `index.cbtml` |
| 文章 | `/posts/{slug}/` | `post.cbtml`（或文章 meta 中自定义 template） |
| 标签 | `/tags/{slug}/` | `tag.cbtml` |
| 分类 | `/category/{slug}/` | `category.cbtml` |
| 归档 | `/archive/{year}/{month}/` | `archive.cbtml` |

文章支持自定义模板：如果文章 meta 中设置了 `template` 字段，将使用对应的自定义模板而非默认的 `post.cbtml`。

---

## 十二、后台模板覆盖

主题可以通过 `templates/admin/` 目录覆盖后台模板：

1. 内嵌默认模板（22 个）通过 `include_str!` 编译进二进制，确保后台始终可用
2. 如果 `themes/<active>/templates/admin/` 目录存在，其中的 `.cbtml` 文件会覆盖同名默认模板
3. 后台模板也使用 CBTML 语法，编译流程与前台一致

---

## 十三、完整主题开发示例

### 13.1 最小主题

**theme.toml：**
```toml
[theme]
name = "minimal"
version = "0.1.0"
author = "Developer"
description = "一个最小化的 cblog 主题"
homepage = ""
```

**templates/base.cbtml：**
```
html [lang="{{ site.language }}"]
  head
    meta [charset="utf-8"]
    meta [name="viewport"] [content="width=device-width, initial-scale=1"]
    title {{ page.title }} - {{ site.title }}
    link [rel="stylesheet"] [href="/assets/main.css"]
  body
    header
      h1
        a [href="/"] {{ site.title }}
    main
      slot content
    footer
      p {{ site.title }} &copy; 2026
```

**templates/index.cbtml：**
```
extends base
slot content
  div.post-list
    for post in posts
      article.post-item
        h2
          a [href="{{ post.url }}"] {{ post.title }}
        time [datetime="{{ post.created_at | iso }}"] {{ post.created_at | date }}
        if post.excerpt
          p.excerpt {{ post.excerpt }}
        end
    end
  if pagination and pagination.total_pages > 1
    nav.pagination
      if pagination.prev
        a [href="{{ pagination.prev }}"] 上一页
      end
      span 第 {{ pagination.current }} / {{ pagination.total_pages }} 页
      if pagination.next
        a [href="{{ pagination.next }}"] 下一页
      end
    end
  end
```

**templates/post.cbtml：**
```
extends base
slot content
  article.post
    h1.post-title {{ post.title }}
    div.post-meta
      time [datetime="{{ post.created_at | iso }}"] {{ post.created_at | date }}
      if post.category
        span.category
          a [href="{{ post.category | category_url }}"] {{ post.category }}
      end
      if post.tags
        div.tags
          for tag in post.tags
            a.tag [href="{{ tag | tag_url }}"] {{ tag }}
          end
        end
      end
    end
    if post.toc
      nav.toc
        h3 目录
        raw post.toc
      end
    end
    div.post-content
      raw post.content
    nav.post-nav
      if prev_post
        a.prev [href="{{ prev_post.url }}"] ← {{ prev_post.title }}
      end
      if next_post
        a.next [href="{{ next_post.url }}"] {{ next_post.title }} →
      end
    end
  end
```

### 13.2 带配置的完整主题

参考 `themes/aurora/` 目录下的所有文件，它包含：
- 10 个配置项、4 个分组（外观/布局/功能/高级）
- 8 个前台模板 + 4 个 partials
- 22 个后台模板覆盖
- SCSS 源文件 + JS 脚本
- 暗黑模式支持

---

## 十四、开发建议

1. **先理解 CBTML 缩进规则**：严格 2 空格，缩进决定 HTML 层级关系
2. **充分利用 `theme.toml` 配置**：通过配置项让用户自定义主题，而不是硬编码
3. **SCSS 变量与配置联动**：配置 key 的 `_` 自动转为 `-` 作为 SCSS 变量名
4. **使用 partials 复用组件**：`include partials/xxx` 引用可复用片段
5. **善用子主题继承**：不需要从头开发，可以继承已有主题修改部分模板
6. **测试构建**：开发过程中使用 `cblog build` 验证模板编译和渲染
7. **注意 void 元素**：`meta`、`link`、`img` 等自闭合标签不需要写子元素
8. **raw 输出 HTML**：文章正文 `post.content`、TOC `post.toc` 等 HTML 内容必须用 `raw` 输出
