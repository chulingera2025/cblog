use crate::cbtml::error::CbtmlError;
use crate::cbtml::lexer::{AttrValue, Token, TokenKind};
use anyhow::Result;

/// cbtml AST 节点
#[derive(Debug, Clone)]
pub enum Node {
    /// 文档根节点
    Document {
        extends: Option<String>,
        children: Vec<Node>,
    },
    /// HTML 元素
    Element {
        tag: String,
        classes: Vec<String>,
        id: Option<String>,
        attributes: Vec<(String, String)>,
        children: Vec<Node>,
        self_closing: bool,
    },
    /// 文本内容
    Text(String),
    /// {{ expr }} 转义输出
    Expression(String),
    /// raw expr 不转义输出
    Raw(String),
    /// if / else if / else 分支
    Conditional {
        condition: String,
        then_branch: Vec<Node>,
        else_if_branches: Vec<(String, Vec<Node>)>,
        else_branch: Option<Vec<Node>>,
    },
    /// for item in collection
    ForLoop {
        var: String,
        collection: String,
        body: Vec<Node>,
    },
    /// slot name
    Slot {
        name: String,
        children: Vec<Node>,
    },
    /// include path
    Include(String),
    /// <style> 原生块
    Style(String),
    /// <script> 原生块
    Script(String),
    /// {# 注释 #} - 编译期移除
    #[allow(dead_code)]
    Comment(String),
    /// hook 调用
    Hook { name: String, data: String },
}

/// 自闭合 void 元素
const VOID_ELEMENTS: &[&str] = &[
    "meta", "link", "input", "br", "hr", "img", "source", "area", "base", "col", "embed",
    "track", "wbr",
];

pub fn is_void_element(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
}

/// 将 Token 序列解析为 AST
pub fn parse(tokens: Vec<Token>, file_name: &str) -> Result<Node> {
    let mut extends = None;
    let mut pos = 0;

    // 检查首行是否为 extends 指令
    if let Some(token) = tokens.first()
        && let TokenKind::Extends(parent) = &token.kind {
            extends = Some(parent.clone());
            pos = 1;
        }

    let children = parse_children(&tokens, &mut pos, 0, file_name)?;

    Ok(Node::Document { extends, children })
}

/// 递归解析同一缩进层级的子节点序列
fn parse_children(
    tokens: &[Token],
    pos: &mut usize,
    expected_indent: usize,
    file_name: &str,
) -> Result<Vec<Node>> {
    let mut children = Vec::new();

    while *pos < tokens.len() {
        let token = &tokens[*pos];

        // 缩进小于期望层级，返回到父级
        if token.indent < expected_indent {
            break;
        }

        // 跳过缩进更深的 token（不应该在这里出现，但作为安全措施）
        if token.indent > expected_indent {
            break;
        }

        match &token.kind {
            // end / else / else if 是控制结构的边界，由上层处理
            TokenKind::End | TokenKind::Else | TokenKind::ElseIf(_) => break,

            TokenKind::Comment(text) => {
                children.push(Node::Comment(text.clone()));
                *pos += 1;
            }

            TokenKind::Text(text) => {
                children.push(Node::Text(text.clone()));
                *pos += 1;
            }

            TokenKind::Expression(expr) => {
                children.push(Node::Expression(expr.clone()));
                *pos += 1;
            }

            TokenKind::Raw(expr) => {
                children.push(Node::Raw(expr.clone()));
                *pos += 1;
            }

            TokenKind::Include(path) => {
                children.push(Node::Include(path.clone()));
                *pos += 1;
            }

            TokenKind::Hook { name, data } => {
                children.push(Node::Hook {
                    name: name.clone(),
                    data: data.clone(),
                });
                *pos += 1;
            }

            TokenKind::If(_) => {
                let node = parse_conditional(tokens, pos, file_name)?;
                children.push(node);
            }

            TokenKind::For { var, collection } => {
                let var = var.clone();
                let collection = collection.clone();
                let current_indent = token.indent;
                let for_line = token.line;
                let for_col = token.col;
                *pos += 1;
                let body = parse_block_body(tokens, pos, current_indent, file_name)?;

                // 期望遇到 end
                if *pos < tokens.len() {
                    if let TokenKind::End = &tokens[*pos].kind {
                        *pos += 1;
                    }
                } else {
                    return Err(CbtmlError::syntax(
                        file_name,
                        for_line,
                        for_col,
                        "for 块缺少对应的 'end' 标签",
                    ).into());
                }

                children.push(Node::ForLoop {
                    var,
                    collection,
                    body,
                });
            }

            TokenKind::Slot(name) => {
                let name = name.clone();
                let current_indent = token.indent;
                *pos += 1;
                // slot 的子节点由更深缩进决定
                let slot_children = parse_indented_children(tokens, pos, current_indent, file_name)?;
                children.push(Node::Slot {
                    name,
                    children: slot_children,
                });
            }

            TokenKind::Element {
                tag,
                classes,
                id,
                attributes,
                inline_text,
            } => {
                let tag = tag.clone();
                let classes = classes.clone();
                let id = id.clone();
                let inline_text = inline_text.clone();
                let self_closing = is_void_element(&tag);
                let current_indent = token.indent;

                // 转换属性值
                let attrs: Vec<(String, String)> = attributes
                    .iter()
                    .map(|(k, v)| {
                        let val = match v {
                            AttrValue::Static(s) => s.clone(),
                            AttrValue::Dynamic(expr) => format!("{{{{ {} }}}}", expr),
                        };
                        (k.clone(), val)
                    })
                    .collect();

                *pos += 1;

                let mut elem_children = Vec::new();

                // 内联文本作为第一个子节点
                if let Some(text) = inline_text {
                    // 内联文本中可能包含 {{ expr }}，需要拆分
                    parse_inline_text(&text, &mut elem_children);
                }

                // void 元素不收集子节点
                if !self_closing {
                    let indented =
                        parse_indented_children(tokens, pos, current_indent, file_name)?;
                    elem_children.extend(indented);
                }

                children.push(Node::Element {
                    tag,
                    classes,
                    id,
                    attributes: attrs,
                    children: elem_children,
                    self_closing,
                });
            }

            TokenKind::StyleBlock => {
                *pos += 1;
                let content = collect_raw_content(tokens, pos);
                children.push(Node::Style(content));
            }

            TokenKind::ScriptBlock => {
                *pos += 1;
                let content = collect_raw_content(tokens, pos);
                children.push(Node::Script(content));
            }

            TokenKind::Extends(_) => {
                return Err(CbtmlError::syntax(
                    file_name,
                    token.line,
                    token.col,
                    "extends 指令只能出现在文件首行",
                ).into());
            }

            TokenKind::RawContent(_) => {
                // 正常不应该在这里遇到，由 StyleBlock/ScriptBlock 处理消费
                *pos += 1;
            }
        }
    }

    Ok(children)
}

/// 解析 if / else if / else / end 条件结构
fn parse_conditional(tokens: &[Token], pos: &mut usize, file_name: &str) -> Result<Node> {
    let condition = match &tokens[*pos].kind {
        TokenKind::If(cond) => cond.clone(),
        _ => unreachable!(),
    };
    let if_indent = tokens[*pos].indent;
    let if_line = tokens[*pos].line;
    let if_col = tokens[*pos].col;
    *pos += 1;

    let then_branch = parse_block_body(tokens, pos, if_indent, file_name)?;
    let mut else_if_branches = Vec::new();
    let mut else_branch = None;
    let mut found_end = false;

    // 处理 else if / else
    while *pos < tokens.len() && tokens[*pos].indent == if_indent {
        match &tokens[*pos].kind {
            TokenKind::ElseIf(cond) => {
                let cond = cond.clone();
                *pos += 1;
                let branch = parse_block_body(tokens, pos, if_indent, file_name)?;
                else_if_branches.push((cond, branch));
            }
            TokenKind::Else => {
                *pos += 1;
                let branch = parse_block_body(tokens, pos, if_indent, file_name)?;
                else_branch = Some(branch);
            }
            TokenKind::End => {
                *pos += 1;
                found_end = true;
                break;
            }
            _ => break,
        }
    }

    if !found_end {
        return Err(CbtmlError::syntax(
            file_name,
            if_line,
            if_col,
            "if 块缺少对应的 'end' 标签",
        ).into());
    }

    Ok(Node::Conditional {
        condition,
        then_branch,
        else_if_branches,
        else_branch,
    })
}

/// 解析控制块（if/for）内部的子节点，直到遇到同级的 end/else/else if
fn parse_block_body(
    tokens: &[Token],
    pos: &mut usize,
    block_indent: usize,
    file_name: &str,
) -> Result<Vec<Node>> {
    let mut children = Vec::new();

    while *pos < tokens.len() {
        let token = &tokens[*pos];

        // 回到控制块同级，是 end/else/else if 的边界
        if token.indent == block_indent {
            match &token.kind {
                TokenKind::End | TokenKind::Else | TokenKind::ElseIf(_) => break,
                _ => {}
            }
        }

        // 缩进比控制块浅，跳出
        if token.indent < block_indent {
            break;
        }

        // 缩进比控制块深的内容属于这个块
        let saved_pos = *pos;
        let target_indent = token.indent;
        let inner = parse_children(tokens, pos, target_indent, file_name)?;
        children.extend(inner);

        // 防止死循环：如果 parse_children 没推进 pos，手动跳过
        if *pos == saved_pos {
            *pos += 1;
        }
    }

    Ok(children)
}

/// 解析缩进更深的子节点（用于元素和 slot）
fn parse_indented_children(
    tokens: &[Token],
    pos: &mut usize,
    parent_indent: usize,
    file_name: &str,
) -> Result<Vec<Node>> {
    if *pos >= tokens.len() || tokens[*pos].indent <= parent_indent {
        return Ok(Vec::new());
    }

    let child_indent = tokens[*pos].indent;
    parse_children(tokens, pos, child_indent, file_name)
}

/// 收集 RawContent token，拼接为原始文本
fn collect_raw_content(tokens: &[Token], pos: &mut usize) -> String {
    let mut lines = Vec::new();
    while *pos < tokens.len() {
        if let TokenKind::RawContent(content) = &tokens[*pos].kind {
            lines.push(content.clone());
            *pos += 1;
        } else {
            break;
        }
    }
    lines.join("\n")
}

/// 解析内联文本中的 {{ expr }} 表达式，拆分为 Text 和 Expression 节点
fn parse_inline_text(text: &str, children: &mut Vec<Node>) {
    let mut remaining = text;

    while let Some(start) = remaining.find("{{") {
        if start > 0 {
            children.push(Node::Text(remaining[..start].to_string()));
        }
        remaining = &remaining[start + 2..];

        if let Some(end) = remaining.find("}}") {
            let expr = remaining[..end].trim().to_string();
            children.push(Node::Expression(expr));
            remaining = &remaining[end + 2..];
        } else {
            // 没有匹配的 }}，当作普通文本
            children.push(Node::Text(format!("{{{{{}", remaining)));
            return;
        }
    }

    if !remaining.is_empty() {
        children.push(Node::Text(remaining.to_string()));
    }
}
