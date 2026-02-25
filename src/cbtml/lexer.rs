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
pub fn tokenize(source: &str, _file_name: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    // TODO!!! 实现完整词法分析器
    let _ = source;
    Ok(tokens)
}
