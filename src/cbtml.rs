pub mod codegen;
pub mod error;
pub mod filters;
pub mod lexer;
pub mod parser;

use anyhow::Result;

/// 编译 cbtml 源码为 MiniJinja 模板字符串
pub fn compile(source: &str, file_name: &str) -> Result<String> {
    let tokens = lexer::tokenize(source, file_name)?;
    let ast = parser::parse(tokens, file_name)?;
    let output = codegen::generate(&ast)?;
    Ok(output)
}
