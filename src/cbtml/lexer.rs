use crate::cbtml::error::CbtmlError;
use anyhow::Result;

/// cbtml Token 类型
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// HTML 元素：tag.class1.class2#id [attr="val"] 文本内容
    Element {
        tag: String,
        classes: Vec<String>,
        id: Option<String>,
        attributes: Vec<(String, AttrValue)>,
        inline_text: Option<String>,
    },
    /// 纯文本行
    Text(String),
    /// {{ expr }} 输出
    Expression(String),
    /// raw expr - 不转义输出
    Raw(String),
    /// if expr
    If(String),
    /// else if expr
    ElseIf(String),
    /// else
    Else,
    /// end
    End,
    /// for item in collection
    For { var: String, collection: String },
    /// extends parent_template
    Extends(String),
    /// slot name
    Slot(String),
    /// include path
    Include(String),
    /// style 原生块
    StyleBlock,
    /// script 原生块
    ScriptBlock,
    /// {# 注释 #}
    Comment(String),
    /// hook("name", data)
    Hook { name: String, data: String },
    /// 原生块内的内容行（style/script 内部）
    RawContent(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    Static(String),
    Dynamic(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub indent: usize,
    pub line: usize,
    pub col: usize,
}

/// 将 cbtml 源码词法分析为 Token 序列
pub fn tokenize(source: &str, file_name: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut line_idx = 0;

    while line_idx < lines.len() {
        let line = lines[line_idx];
        let trimmed = line.trim();

        // 跳过空行
        if trimmed.is_empty() {
            line_idx += 1;
            continue;
        }

        let indent_spaces = line.len() - line.trim_start().len();
        let indent = indent_spaces / 2;
        let line_num = line_idx + 1;

        // 注释 {# ... #}
        if let Some(after_open) = trimmed.strip_prefix("{#") {
            if let Some(end) = trimmed.find("#}") {
                let comment = trimmed[2..end].trim().to_string();
                tokens.push(Token {
                    kind: TokenKind::Comment(comment),
                    indent,
                    line: line_num,
                    col: indent_spaces + 1,
                });
                line_idx += 1;
                continue;
            }
            // 多行注释：收集直到 #}
            let comment_start_line = line_num;
            let mut comment = after_open.to_string();
            line_idx += 1;
            let mut found_end = false;
            while line_idx < lines.len() {
                let next = lines[line_idx];
                if let Some(end) = next.find("#}") {
                    comment.push('\n');
                    comment.push_str(next[..end].trim());
                    line_idx += 1;
                    found_end = true;
                    break;
                }
                comment.push('\n');
                comment.push_str(next.trim());
                line_idx += 1;
            }
            if !found_end {
                return Err(CbtmlError::compile(
                    file_name,
                    comment_start_line,
                    indent_spaces + 1,
                    "未闭合的多行注释，缺少 '#}'",
                    source,
                ).into());
            }
            tokens.push(Token {
                kind: TokenKind::Comment(comment.trim().to_string()),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            continue;
        }

        // 表达式 {{ expr }}
        if trimmed.starts_with("{{") && trimmed.ends_with("}}") {
            let expr = trimmed[2..trimmed.len() - 2].trim().to_string();
            tokens.push(Token {
                kind: TokenKind::Expression(expr),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // 独立行的 {{ 但没有 }} 闭合
        if trimmed.starts_with("{{") && !trimmed.contains("}}") {
            return Err(CbtmlError::compile(
                file_name,
                line_num,
                indent_spaces + 1,
                "未闭合的表达式，缺少 '}}'",
                source,
            ).into());
        }

        // extends 指令
        if let Some(rest) = trimmed.strip_prefix("extends ") {
            let parent = rest.trim().to_string();
            tokens.push(Token {
                kind: TokenKind::Extends(parent),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // if 指令
        if let Some(rest) = trimmed.strip_prefix("if ") {
            let expr = rest.trim().to_string();
            tokens.push(Token {
                kind: TokenKind::If(expr),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // else if 指令
        if let Some(rest) = trimmed.strip_prefix("else if ") {
            let expr = rest.trim().to_string();
            tokens.push(Token {
                kind: TokenKind::ElseIf(expr),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // else
        if trimmed == "else" {
            tokens.push(Token {
                kind: TokenKind::Else,
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // end
        if trimmed == "end" {
            tokens.push(Token {
                kind: TokenKind::End,
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // for var in collection
        if let Some(after_for) = trimmed.strip_prefix("for ") {
            let rest = after_for.trim();
            if let Some(in_pos) = rest.find(" in ") {
                let var = rest[..in_pos].trim().to_string();
                let collection = rest[in_pos + 4..].trim().to_string();
                tokens.push(Token {
                    kind: TokenKind::For { var, collection },
                    indent,
                    line: line_num,
                    col: indent_spaces + 1,
                });
            } else {
                return Err(CbtmlError::syntax_with_source(
                    file_name,
                    line_num,
                    indent_spaces + 1,
                    format!("for 语句缺少 'in' 关键字: {trimmed}"),
                    source,
                ).into());
            }
            line_idx += 1;
            continue;
        }

        // slot 指令
        if let Some(rest) = trimmed.strip_prefix("slot ") {
            let name = rest.trim().to_string();
            tokens.push(Token {
                kind: TokenKind::Slot(name),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // include 指令
        if let Some(rest) = trimmed.strip_prefix("include ") {
            let path = rest.trim().to_string();
            tokens.push(Token {
                kind: TokenKind::Include(path),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // raw expr
        if let Some(rest) = trimmed.strip_prefix("raw ") {
            let expr = rest.trim().to_string();
            tokens.push(Token {
                kind: TokenKind::Raw(expr),
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            continue;
        }

        // hook("name", data)
        if trimmed.starts_with("hook(")
            && let Some(token) = parse_hook(trimmed, indent, line_num, indent_spaces + 1) {
                tokens.push(token);
                line_idx += 1;
                continue;
            }

        // style 原生块：收集后续缩进更深的行作为内容
        if trimmed == "style" {
            tokens.push(Token {
                kind: TokenKind::StyleBlock,
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            // 收集 style 块内的原始内容
            while line_idx < lines.len() {
                let next_line = lines[line_idx];
                let next_trimmed = next_line.trim();
                if next_trimmed.is_empty() {
                    line_idx += 1;
                    continue;
                }
                let next_indent_spaces = next_line.len() - next_line.trim_start().len();
                if next_indent_spaces <= indent_spaces {
                    break;
                }
                tokens.push(Token {
                    kind: TokenKind::RawContent(next_trimmed.to_string()),
                    indent: next_indent_spaces / 2,
                    line: line_idx + 1,
                    col: next_indent_spaces + 1,
                });
                line_idx += 1;
            }
            continue;
        }

        // script 原生块
        if trimmed == "script" {
            tokens.push(Token {
                kind: TokenKind::ScriptBlock,
                indent,
                line: line_num,
                col: indent_spaces + 1,
            });
            line_idx += 1;
            while line_idx < lines.len() {
                let next_line = lines[line_idx];
                let next_trimmed = next_line.trim();
                if next_trimmed.is_empty() {
                    line_idx += 1;
                    continue;
                }
                let next_indent_spaces = next_line.len() - next_line.trim_start().len();
                if next_indent_spaces <= indent_spaces {
                    break;
                }
                tokens.push(Token {
                    kind: TokenKind::RawContent(next_trimmed.to_string()),
                    indent: next_indent_spaces / 2,
                    line: line_idx + 1,
                    col: next_indent_spaces + 1,
                });
                line_idx += 1;
            }
            continue;
        }

        // 元素声明：以合法 HTML 标签名开头
        if trimmed.starts_with(|c: char| c.is_ascii_alphabetic()) {
            // 检查未闭合的属性括号
            if trimmed.contains('[') && !trimmed.contains(']') {
                return Err(CbtmlError::compile(
                    file_name,
                    line_num,
                    indent_spaces + 1,
                    "未闭合的属性括号，缺少 ']'",
                    source,
                ).into());
            }
            if let Some(token) = parse_element(trimmed, indent, line_num, indent_spaces + 1) {
                tokens.push(token);
                line_idx += 1;
                continue;
            }
        }

        // 默认当作纯文本
        tokens.push(Token {
            kind: TokenKind::Text(trimmed.to_string()),
            indent,
            line: line_num,
            col: indent_spaces + 1,
        });
        line_idx += 1;
    }

    Ok(tokens)
}

/// 解析 hook("name", data) 调用
fn parse_hook(s: &str, indent: usize, line: usize, col: usize) -> Option<Token> {
    // hook("name", data) 或 hook("name")
    let inner = s.strip_prefix("hook(")?.strip_suffix(')')?;
    let inner = inner.trim();

    // 提取引号内的名称
    let name_start = inner.find('"')? + 1;
    let name_end = inner[name_start..].find('"')? + name_start;
    let name = inner[name_start..name_end].to_string();

    let rest = inner[name_end + 1..].trim();
    let data = if let Some(stripped) = rest.strip_prefix(',') {
        stripped.trim().to_string()
    } else {
        String::new()
    };

    Some(Token {
        kind: TokenKind::Hook { name, data },
        indent,
        line,
        col,
    })
}

/// 解析元素声明：tag.class1.class2#id [attr="val" attr2={{ expr }}] 内联文本
fn parse_element(s: &str, indent: usize, line: usize, col: usize) -> Option<Token> {
    let mut chars = s.chars().peekable();
    let mut tag = String::new();
    let mut classes = Vec::new();
    let mut id = None;
    let mut attributes = Vec::new();
    let mut inline_text = None;

    // 解析标签名
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            tag.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if tag.is_empty() {
        return None;
    }

    // 解析 .class 和 #id 选择器
    while let Some(&c) = chars.peek() {
        if c == '.' {
            chars.next();
            let mut class = String::new();
            while let Some(&c2) = chars.peek() {
                if c2.is_ascii_alphanumeric() || c2 == '-' || c2 == '_' {
                    class.push(c2);
                    chars.next();
                } else {
                    break;
                }
            }
            if !class.is_empty() {
                classes.push(class);
            }
        } else if c == '#' {
            chars.next();
            let mut id_str = String::new();
            while let Some(&c2) = chars.peek() {
                if c2.is_ascii_alphanumeric() || c2 == '-' || c2 == '_' {
                    id_str.push(c2);
                    chars.next();
                } else {
                    break;
                }
            }
            if !id_str.is_empty() {
                id = Some(id_str);
            }
        } else {
            break;
        }
    }

    // 跳过空格
    while let Some(&' ') = chars.peek() {
        chars.next();
    }

    // 解析属性块，支持多个 [attr="val"] [attr2="val2"] 形式
    while let Some(&'[') = chars.peek() {
        chars.next();
        loop {
            // 跳过空格
            while let Some(&' ') = chars.peek() {
                chars.next();
            }
            if let Some(&']') = chars.peek() {
                chars.next();
                break;
            }
            if chars.peek().is_none() {
                break;
            }

            // 属性名
            let mut attr_name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' {
                    attr_name.push(c);
                    chars.next();
                } else {
                    break;
                }
            }

            if attr_name.is_empty() {
                break;
            }

            // 跳过 =
            while let Some(&' ') = chars.peek() {
                chars.next();
            }
            if let Some(&'=') = chars.peek() {
                chars.next();
            } else {
                // boolean 属性
                attributes.push((attr_name, AttrValue::Static(String::new())));
                continue;
            }
            while let Some(&' ') = chars.peek() {
                chars.next();
            }

            // 属性值
            if let Some(&'{') = chars.peek() {
                // 动态属性 {{ expr }}
                chars.next();
                if let Some(&'{') = chars.peek() {
                    chars.next();
                    let mut expr = String::new();
                    let mut depth = 2;
                    while let Some(&c) = chars.peek() {
                        if c == '}' {
                            depth -= 1;
                            chars.next();
                            if depth == 0 {
                                break;
                            }
                        } else {
                            if c == '{' {
                                depth += 1;
                            }
                            expr.push(c);
                            chars.next();
                        }
                    }
                    attributes.push((attr_name, AttrValue::Dynamic(expr.trim().to_string())));
                }
            } else if let Some(&'"') = chars.peek() {
                chars.next();
                let mut val = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    val.push(c);
                    chars.next();
                }
                attributes.push((attr_name, AttrValue::Static(val)));
            } else if let Some(&'\'') = chars.peek() {
                chars.next();
                let mut val = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '\'' {
                        chars.next();
                        break;
                    }
                    val.push(c);
                    chars.next();
                }
                attributes.push((attr_name, AttrValue::Static(val)));
            }
        }

        // 跳过属性块之间的空格，以便检查下一个 [
        while let Some(&' ') = chars.peek() {
            chars.next();
        }
    }

    // 剩余部分为内联文本
    let remaining: String = chars.collect();
    let remaining = remaining.trim();
    if !remaining.is_empty() {
        inline_text = Some(remaining.to_string());
    }

    Some(Token {
        kind: TokenKind::Element {
            tag,
            classes,
            id,
            attributes,
            inline_text,
        },
        indent,
        line,
        col,
    })
}
