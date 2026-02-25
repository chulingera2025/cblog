use crate::cbtml::lexer::Token;
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
    Comment(String),
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
pub fn parse(tokens: Vec<Token>, _file_name: &str) -> Result<Node> {
    // TODO!!! 实现基于缩进的语法分析器
    let _ = tokens;
    Ok(Node::Document {
        extends: None,
        children: vec![],
    })
}
