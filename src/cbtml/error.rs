use thiserror::Error;

#[derive(Debug, Error)]
pub enum CbtmlError {
    #[error("cbtml 编译错误\n  → {file}:{line}:{col}\n\n{context}\n\n  错误：{message}")]
    CompileError {
        file: String,
        line: usize,
        col: usize,
        message: String,
        context: String,
    },

    #[error("cbtml 语法错误\n  → {file}:{line}:{col}\n\n  错误：{message}")]
    SyntaxError {
        file: String,
        line: usize,
        col: usize,
        message: String,
    },
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
        }
    }

    pub fn syntax(file: &str, line: usize, col: usize, message: impl Into<String>) -> Self {
        Self::SyntaxError {
            file: file.to_string(),
            line,
            col,
            message: message.into(),
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
