// Lang-Zong 编译器 — codegen/helpers.rs
// 独立辅助函数 (escape_str, gen_fstring, extract_return, gen_decorator_attr, out_push_attr)

use crate::parser::*;
use crate::lexer::Lexer;
use crate::parser::ParserExprExt;
use super::CodeGen;
use super::expr::CodeGenExprExt;

/// 转义字符串中的特殊字符
pub(super) fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"', "\\\"")
     .replace('\n', "\\n")
     .replace('\t', "\\t")
     .replace('\r', "\\r")
}

/// 生成 f-string 转译为 Rust format! 宏
/// f"hello {name}" → format!("hello {}", name)
pub(super) fn gen_fstring(cg: &CodeGen, s: &str) -> String {
    let mut fmt_str = String::new();
    let mut args = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                fmt_str.push('{');
                chars.next();
            } else {
                let mut expr = String::new();
                while let Some(ec) = chars.next() {
                    if ec == '}' { break; }
                    expr.push(ec);
                }
                let expr_str = expr.trim().to_string();
                let parsed = {
                    let mut lexer = Lexer::new(&expr_str);
                    let toks = lexer.tokenize();
                    let mut parser = Parser::new(toks);
                    parser.parse_expr()
                };
                match parsed {
                    Ok(e) => args.push(cg.gen_expr(&e)),
                    Err(_) => args.push(expr_str),
                }
                fmt_str.push_str("{}");
            }
        } else if c == '}' && chars.peek() == Some(&'}') {
            fmt_str.push('}');
            chars.next();
        } else if c == '"' {
            fmt_str.push_str("\\\"");
        } else if c == '\\' {
            fmt_str.push_str("\\\\");
        } else {
            fmt_str.push(c);
        }
    }

    if args.is_empty() {
        format!("\"{}\".to_string()", fmt_str)
    } else {
        format!("format!(\"{}\", {})", fmt_str, args.join(", "))
    }
}

/// 从函数体中提取返回值表达式
pub(super) fn extract_return(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.starts_with("return ") && trimmed.ends_with(';') {
        let expr = &trimmed[7..trimmed.len()-1];
        return Some(expr.to_string());
    }
    if trimmed == "return;" {
        return Some("()".to_string());
    }
    if !trimmed.is_empty() && !trimmed.contains('\n') && !trimmed.ends_with(';') {
        return Some(trimmed.to_string());
    }
    if !trimmed.is_empty() && !trimmed.contains('\n') && trimmed.ends_with(';') {
        return Some(trimmed[..trimmed.len()-1].to_string());
    }
    None
}

/// 装饰器 → Rust attribute 字符串
pub(super) fn gen_decorator_attr(d: &Decorator) -> String {
    match d.name.as_str() {
        "simd" => return "#[inline(always)] // @simd: autovectorize hint\n".to_string(),
        "parallel" => return "// @parallel: rayon par_iter candidate\n".to_string(),
        _ => {}
    }
    let args: Vec<String> = d.args.iter()
        .map(|a| match a {
            Expr::Ident(n) => n.clone(),
            Expr::StrLit(s) => format!("\"{}\"", s),
            _ => format!("{:?}", a),
        })
        .collect();
    if args.is_empty() {
        format!("#[{}]\n", d.name)
    } else {
        format!("#[{}({})]\n", d.name, args.join(", "))
    }
}

/// placeholder
pub(super) fn out_push_attr(_out: &mut String, _d: &Decorator) {
}
