// Lang-Zong 编译器 — parser/helpers.rs
// 解析器辅助函数

use crate::lexer::{Token, Lexer};
use super::parser::Parser;
use super::expr::ParserExprExt;

/// 判断 token 是否可能开启一个新表达式（用于 ^ 后缀 move 与中缀 XOR 消歧）
pub fn is_expr_start(tok: &Token) -> bool {
    matches!(tok,
        Token::IntLit(_) | Token::FloatLit(_) | Token::StrLit(_)
        | Token::FStrLit(_) | Token::RawStrLit(_)
        | Token::True | Token::False | Token::Ident(_)
        | Token::LParen | Token::LBrack | Token::LBrace
        | Token::Minus | Token::Exclamation | Token::Not
    )
}

/// 预校验 f-string 插值：每个 `{expr}` 重新词法化 + 解析，失败则返回 parse error。
/// 与 codegen `gen_fstring` 的提取逻辑保持一致，确保畸形插值在解析期被拒绝
/// （不再泄漏原始文本到 rustc）。`{{` / `}}` 为转义大括号，跳过。
pub fn validate_fstring(s: &str) -> Result<(), String> {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                continue;
            }
            let mut expr = String::new();
            while let Some(ec) = chars.next() {
                if ec == '}' { break; }
                expr.push(ec);
            }
            let expr_str = expr.trim();
            let parsed = {
                let mut lexer = Lexer::new(expr_str);
                let toks = lexer.tokenize();
                let mut parser = Parser::new(toks);
                parser.parse_expr()
            };
            if let Err(e) = parsed {
                return Err(format!("f-string 插值 '{{{}}}' 无效: {}", expr_str, e));
            }
        } else if c == '}' && chars.peek() == Some(&'}') {
            chars.next();
        }
    }
    Ok(())
}
