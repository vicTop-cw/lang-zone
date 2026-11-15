// Lang-Zone / Lang-Zong 编译器 — parser/pattern.rs
// Pattern 解析（P2 move-only：自 expr.rs 原样搬出，方法体逐字节未改）
//
// 拆分依据（实测耦合面）：parse_pattern 只调用 Parser 的游标原语
// advance / check / expect / peek / peek_n 与自身递归，
// 不调用任何表达式解析方法 ⇒ 与 expr.rs 无互递归。
// 反向依赖只有 5 处调用（expr.rs×2、stmt.rs×5），均按固有方法解析，
// 不需要任何 trait 导入。

use super::parser::Parser;
use crate::ast::Pattern;
use crate::lexer::Token;

impl Parser {
    /// 解析 pattern（match arm / guard let 等）
    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, String> {
        // ref/mut 模式修饰符：`ref mut c` → &mut 绑定（case Some(ref mut c)，
        // 臂体内 c = c + 1 修改引用指向的值）。ref 单独用 → & 只读绑定。
        let mut ref_kind: Option<bool> = None; // Some(true)=mut, Some(false)=ref
        while self.check(&Token::Ref) || self.check(&Token::Mut) {
            if self.check(&Token::Mut) {
                ref_kind = Some(true);
            } else {
                ref_kind = Some(false);
            }
            self.advance();
        }
        let first = self.advance();
        match first {
            // 负数模式: case -999 =>
            Token::Minus => match self.advance() {
                Token::IntLit(n) => Ok(Pattern::Int(-n)),
                t => Err(format!(
                    "Expected integer after minus in pattern, got {:?}",
                    t
                )),
            },
            Token::IntLit(n) => {
                // 范围模式: n..m 或 n..=m
                if self.check(&Token::DotDot) {
                    self.advance(); // ..
                    match self.advance() {
                        Token::IntLit(end) => Ok(Pattern::Range {
                            start: n,
                            end,
                            inclusive: false,
                        }),
                        t => Err(format!("Expected end of range pattern, got {:?}", t)),
                    }
                } else if self.check(&Token::DotDotEq) {
                    self.advance(); // ..=
                    match self.advance() {
                        Token::IntLit(end) => Ok(Pattern::Range {
                            start: n,
                            end,
                            inclusive: true,
                        }),
                        t => Err(format!("Expected end of range pattern, got {:?}", t)),
                    }
                } else {
                    Ok(Pattern::Int(n))
                }
            }
            Token::Int128Lit(n) => Ok(Pattern::Int128(n)),
            Token::BigIntLit(s) => Ok(Pattern::BigInt(s.clone())),
            Token::ComplexLit(re, im) => Ok(Pattern::Complex(re, im)),
            Token::FloatLit(f) => Ok(Pattern::Str(f.to_string())),
            Token::StrLit(s) => Ok(Pattern::Str(s)),
            Token::True => Ok(Pattern::Bool(true)),
            Token::False => Ok(Pattern::Bool(false)),
            Token::Underscore => Ok(Pattern::Wildcard),
            // match 兜底分支：`else => ...` 等价 `_ => ...`（通配）
            Token::Else => Ok(Pattern::Wildcard),
            // `..` / `..rest` 剩余绑定（元组模式 (a, ..) / 列表模式 [a, ..rest]，03d §2.8 方案 B）
            Token::DotDot | Token::DotDotDot => {
                // first 已由顶层 advance() 消费（first == DotDot），此处仅检查 rest 绑定名
                let rest_name = if matches!(self.peek(), Token::Ident(_)) {
                    Some(self.advance().to_string())
                } else {
                    None
                };
                Ok(Pattern::Rest(rest_name))
            }
            Token::Ident(n) => {
                let mut name = n;
                // 处理点路径模式: Shape.Circle(x: _, y: _, radius: r)
                while self.check(&Token::Dot) {
                    self.advance(); // .
                    let seg = match self.advance() {
                        Token::Ident(s) => s,
                        t => return Err(format!("Expected path segment in pattern, got {:?}", t)),
                    };
                    name = format!("{}.{}", name, seg);
                }
                // 元组变体: Some(x, y) 或 Shape.Circle(radius: r)
                let patterns = if self.check(&Token::LParen) {
                    self.advance();
                    let mut pats = Vec::new();
                    while !self.check(&Token::RParen) {
                        // 处理关键字参数风格模式: name: pattern
                        if matches!(self.peek(), Token::Ident(_)) && self.peek_n(1) == &Token::Colon
                        {
                            self.advance(); // field name
                            self.advance(); // :
                            pats.push(self.parse_pattern()?);
                        } else {
                            pats.push(self.parse_pattern()?);
                        }
                        if self.check(&Token::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(Token::RParen)?;
                    if pats.is_empty() {
                        Vec::new()
                    } else {
                        pats
                    }
                } else {
                    Vec::new()
                };

                // 点路径模式（如 Color.Red）即使无括号参数也是变体
                if !patterns.is_empty() || name.contains('.') {
                    Ok(Pattern::Variant(name, patterns))
                } else if ref_kind == Some(true) {
                    Ok(Pattern::RefMutIdent(name))
                } else {
                    Ok(Pattern::Ident(name))
                }
            }
            Token::LParen => {
                let mut patterns = Vec::new();
                // 跟踪是否出现逗号：`(a)` 是分组（解包为单模式），`(a,)` 是单元素
                // 元组模式（保留 Tuple），`(a, b)` 是多元素元组（03d §2.8 方案 B：
                // `case (a,) => a` / `case (a, ..) => a` 匹配 type-pack 异质元组）
                let mut has_comma = false;
                while !self.check(&Token::RParen) {
                    patterns.push(self.parse_pattern()?);
                    if self.check(&Token::Comma) {
                        self.advance();
                        has_comma = true;
                    }
                }
                self.expect(Token::RParen)?;
                if patterns.len() == 1 && !has_comma {
                    Ok(patterns.into_iter().next().unwrap())
                } else {
                    Ok(Pattern::Tuple(patterns))
                }
            }
            Token::LBrack => {
                // 列表模式: [a, b, c] 或 [first, ..rest]
                let mut patterns = Vec::new();
                while !self.check(&Token::RBrack) {
                    if self.check(&Token::DotDot) {
                        self.advance(); // ..
                                        // .. 后面可能有 rest 绑定名: ..rest
                        let rest_pat = if let Token::Ident(_) = self.peek() {
                            let name = self.advance().to_string();
                            Pattern::Rest(Some(name))
                        } else {
                            Pattern::Rest(None)
                        };
                        patterns.push(rest_pat);
                    } else {
                        patterns.push(self.parse_pattern()?);
                    }
                    if self.check(&Token::Comma) {
                        self.advance();
                    }
                }
                self.expect(Token::RBrack)?;
                Ok(Pattern::List(patterns))
            }
            Token::LBrace => {
                // 字典模式: {"port": p} — 键值对匹配（未列出的键忽略）
                let mut entries = Vec::new();
                while !self.check(&Token::RBrace) {
                    // 键：字符串字面量或标识符
                    let key = match self.advance() {
                        Token::StrLit(s) => s,
                        Token::Ident(s) => s,
                        t => {
                            return Err(format!("Expected string key in dict pattern, got {:?}", t))
                        }
                    };
                    self.expect(Token::Colon)?; // :
                    let pat = self.parse_pattern()?;
                    entries.push((key, pat));
                    if self.check(&Token::Comma) {
                        self.advance();
                    }
                }
                self.expect(Token::RBrace)?;
                Ok(Pattern::Dict(entries))
            }
            _ => Err(format!("Unexpected token in pattern: {:?}", first)),
        }
    }
}
