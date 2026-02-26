use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CbtmlError {
    #[error("{}", format_error("编译错误", file, *line, *col, context, message, hint))]
    CompileError {
        file: String,
        line: usize,
        col: usize,
        message: String,
        context: String,
        hint: Option<String>,
    },

    #[error("{}", format_error("语法错误", file, *line, *col, context, message, hint))]
    SyntaxError {
        file: String,
        line: usize,
        col: usize,
        message: String,
        context: String,
        hint: Option<String>,
    },
}

fn format_error(
    kind: &str,
    file: &str,
    line: usize,
    col: usize,
    context: &str,
    message: &str,
    hint: &Option<String>,
) -> String {
    let mut out = format!("cbtml {kind}\n  → {file}:{line}:{col}\n\n");
    if !context.is_empty() {
        out.push_str(context);
    }
    out.push_str(&format!("  错误：{message}"));
    if let Some(h) = hint {
        out.push_str(&format!("\n  提示：{h}"));
    }
    out
}

impl CbtmlError {
    pub fn compile(file: &str, line: usize, col: usize, message: impl Into<String>, source: &str) -> Self {
        let context = build_error_context(source, line);
        Self::CompileError {
            file: file.to_string(),
            line,
            col,
            message: message.into(),
            context,
            hint: None,
        }
    }

    #[allow(dead_code)]
    pub fn compile_with_hint(
        file: &str,
        line: usize,
        col: usize,
        message: impl Into<String>,
        hint: impl fmt::Display,
        source: &str,
    ) -> Self {
        let context = build_error_context(source, line);
        Self::CompileError {
            file: file.to_string(),
            line,
            col,
            message: message.into(),
            context,
            hint: Some(hint.to_string()),
        }
    }

    /// 在有源码的上下文中创建语法错误（如 lexer），包含上下文行
    pub fn syntax_with_source(file: &str, line: usize, col: usize, message: impl Into<String>, source: &str) -> Self {
        let context = build_error_context(source, line);
        Self::SyntaxError {
            file: file.to_string(),
            line,
            col,
            message: message.into(),
            context,
            hint: None,
        }
    }

    /// 在无源码的上下文中创建语法错误（如 parser），仅标注行列号
    pub fn syntax(file: &str, line: usize, col: usize, message: impl Into<String>) -> Self {
        Self::SyntaxError {
            file: file.to_string(),
            line,
            col,
            message: message.into(),
            context: String::new(),
            hint: None,
        }
    }
}

fn build_error_context(source: &str, error_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = error_line.saturating_sub(3);
    let end = (error_line + 2).min(lines.len());

    let mut ctx = String::new();
    let width = format!("{}", end).len();
    for i in start..end {
        let marker = if i + 1 == error_line { ">" } else { " " };
        ctx.push_str(&format!(
            "  {} {:>width$} | {}\n",
            marker,
            i + 1,
            lines.get(i).unwrap_or(&""),
            width = width,
        ));
    }
    ctx
}
