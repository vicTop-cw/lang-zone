// Lang-Zong 编译器 — parser/mod.rs
// 语法分析模块入口：子模块声明 + Re-export + Token::to_string 实现

mod parser;
mod stmt;
mod expr;
mod helpers;

// Re-export 所有公开类型（保持 crate::parser::* 兼容性）
pub use crate::ast::*;
pub use parser::Parser;
pub use helpers::{is_expr_start, validate_fstring};
pub use expr::ParserExprExt;

// Token::to_string 实现（放在解析器模块中以便访问 Token 类型）
use crate::lexer::Token;
impl Token {
    pub fn to_string(&self) -> String {
        match self {
            Token::Ident(s) => s.clone(),
            Token::MagicMethod(s) => s.clone(),
            Token::StrLit(s) => s.clone(),
            Token::IntLit(n) => n.to_string(),
            Token::FloatLit(f) => f.to_string(),
            Token::None_ => "None".to_string(),
            Token::Some_ => "Some".to_string(),
            Token::Ok_ => "Ok".to_string(),
            Token::Err_ => "Err".to_string(),
            Token::True => "True".to_string(),
            Token::False => "False".to_string(),
            Token::Self_ => "Self".to_string(),
            Token::Eof => "EOF".to_string(),
            _ => format!("{:?}", self),
        }
    }
}
