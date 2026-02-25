---
title: "cbtml 模板语言设计思路"
date: 2024-03-10
tags: ["模板", "设计"]
category: "设计"
---

## 什么是 cbtml

cbtml 是 cblog 专属的声明式模板语言。它的核心理念是**缩进即结构**，消除传统 HTML 模板中大量的闭合标签噪音。

## 设计目标

cbtml 追求以下目标：

1. **简洁** - 缩进表示嵌套关系，无需闭合标签
2. **安全** - 默认 HTML 转义，防止 XSS
3. **强类型** - 编译期检查模板变量
4. **高性能** - 编译为原生 MiniJinja 模板

### 语法示例

cbtml 使用类似 CSS 选择器的语法来声明 HTML 元素：

```
html [lang="zh-CN"]
  head
    meta [charset="UTF-8"]
    title {{ page.title }}
  body
    header.site-header
      h1.site-title {{ site.title }}
    main.content
      for post in posts
        article.post-card
          h2.post-title
            a [href=post.url] {{ post.title }}
```

这种语法大幅减少了模板文件的视觉噪音，让开发者能专注于页面结构和数据绑定。
