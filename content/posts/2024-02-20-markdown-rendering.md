---
title: "Markdown 渲染与 Front Matter 解析"
date: 2024-02-20
tags: ["Rust", "Markdown"]
category: "技术"
---

## Front Matter

cblog 使用 YAML 格式的 Front Matter 来定义文章的元数据。每篇文章以 `---` 分隔的 YAML 块开头：

```yaml
---
title: "文章标题"
date: 2024-02-20
tags: ["Rust", "Markdown"]
category: "技术"
---
```

支持的字段包括 title、date、tags、category、excerpt、draft 等。

## Markdown 扩展

cblog 基于 pulldown-cmark 实现 Markdown 渲染，支持以下扩展语法：

- **表格**：GitHub 风格的表格
- **脚注**：文末注释引用
- **删除线**：`~~文本~~` 形式
- **任务列表**：`- [x]` 和 `- [ ]` 复选框

### 表格示例

| 功能 | 状态 |
|------|------|
| 基本 Markdown | 完成 |
| Front Matter | 完成 |
| TOC 生成 | 完成 |
| 代码高亮 | 计划中 |

## 目录生成

系统会自动从 Markdown 中提取 h2-h4 级别的标题，生成带锚点链接的目录（TOC）。标题文本会被 slugify 处理后作为 HTML id 属性。
