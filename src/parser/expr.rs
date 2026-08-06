// Lang-Zong 编译器 — parser/expr.rs
// 表达式解析（优先级递降）+ Pattern 解析（作为 Parser 的 trait 扩展）

use crate::lexer::Token;
use crate::types::Type;
use crate::ast::*;
use super::parser::Parser;
use super::helpers::validate_fstring;
use super::stmt::ParserStmtExt;

/// Parser 的表达式解析扩展 trait
pub trait ParserExprExt {
    fn parse_expr(&mut self) -> Result<Expr, String>;
    fn parse_or(&mut self) -> Result<Expr, String>;
    fn parse_and(&mut self) -> Result<Expr, String>;
    fn parse_not(&mut self) -> Result<Expr, String>;
    fn parse_comparison(&mut self) -> Result<Expr, String>;
    fn parse_in_is(&mut self) -> Result<Expr, String>;
    fn parse_pipe(&mut self) -> Result<Expr, String>;
    fn parse_null_coalesce(&mut self) -> Result<Expr, String>;
    fn parse_bit_or(&mut self) -> Result<Expr, String>;
    fn parse_bit_xor(&mut self) -> Result<Expr, String>;
    fn parse_bit_and(&mut self) -> Result<Expr, String>;
    fn parse_shift(&mut self) -> Result<Expr, String>;
    fn parse_additive(&mut self) -> Result<Expr, String>;
    fn parse_multiplicative(&mut self) -> Result<Expr, String>;
    fn parse_power(&mut self) -> Result<Expr, String>;
    fn parse_unary(&mut self) -> Result<Expr, String>;
    fn parse_postfix(&mut self) -> Result<Expr, String>;
    /// 从已有表达式继续解析 postfix（跨行方法链用）
    fn parse_postfix_chain(&mut self, expr: Expr) -> Result<Expr, String>;
    fn parse_primary(&mut self) -> Result<Expr, String>;
    fn parse_pattern(&mut self) -> Result<Pattern, String>;
    /// 解析 comprehension 的 iter 表达式：支持 range(..) 和 walrus(:=)，
    /// 但不消费 if（避免与 comprehension guard 冲突）
    fn parse_comprehension_iter(&mut self) -> Result<Expr, String>;
    /// 判断 `?` 后是否紧跟三元 true 分支（表达式开始）—— 用于区分
    /// 三元 `cond ? a : b` 与错误传播后缀 `expr?`
    fn is_ternary_after(&self) -> bool;
}

impl ParserExprExt for Parser {
    // ─── 表达式解析（优先级递降） ───

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let left = self.parse_or()?;
        // Ternary: a if cond else b (only if left is NOT an if/while/for expression)
        if !matches!(&left, Expr::If { .. } | Expr::Match { .. }) && self.check(&Token::If) {
            self.advance();
            let cond = self.parse_expr()?;
            self.skip_newlines();
            self.expect(Token::Else)?;
            let else_val = self.parse_expr()?;
            return Ok(Expr::If {
                cond: Box::new(cond),
                then_body: vec![Stmt::Expr(left)],
                elif_clauses: Vec::new(),
                else_body: Some(vec![Stmt::Expr(else_val)]),
            });
        }
        // Ternary (C 风格): cond ? a : b
        // `?` 作为三元中缀（其后跟表达式再跟 :）；若为行尾 Try 已由 postfix 消费
        if self.check(&Token::Question) {
            self.advance();
            let then_val = self.parse_expr()?;
            self.expect(Token::Colon)?;
            let else_val = self.parse_expr()?;
            return Ok(Expr::If {
                cond: Box::new(left),
                then_body: vec![Stmt::Expr(then_val)],
                elif_clauses: Vec::new(),
                else_body: Some(vec![Stmt::Expr(else_val)]),
            });
        }
        // Walrus: x := expr  (右结合，最低优先级)
        if self.check(&Token::ColonEq) {
            self.advance();
            let value = self.parse_expr()?;
            return Ok(Expr::Walrus { target: Box::new(left), value: Box::new(value) });
        }
        // Range: a..b or a..=b
        if self.check(&Token::DotDot) || self.check(&Token::DotDotEq) {
            let inclusive = self.check(&Token::DotDotEq);
            self.advance();
            let end = if self.check(&Token::Colon) || self.check(&Token::Newline)
                || self.check(&Token::Dedent) || self.check(&Token::Eof)
                || self.check(&Token::RParen) || self.check(&Token::RBrack) {
                None
            } else {
                Some(Box::new(self.parse_or()?))
            };
            return Ok(Expr::Range {
                start: Some(Box::new(left)),
                end,
                inclusive,
            });
        }
        Ok(left)
    }

    /// 解析 comprehension 的 iter 表达式：支持 range/walrus 但不消费 if
    fn parse_comprehension_iter(&mut self) -> Result<Expr, String> {
        let left = self.parse_or()?;
        // Walrus (不消费 if 三元)
        if self.check(&Token::ColonEq) {
            self.advance();
            let value = self.parse_expr()?;
            return Ok(Expr::Walrus { target: Box::new(left), value: Box::new(value) });
        }
        // Range
        if self.check(&Token::DotDot) || self.check(&Token::DotDotEq) {
            let inclusive = self.check(&Token::DotDotEq);
            self.advance();
            let end = if self.check(&Token::Colon) || self.check(&Token::Newline)
                || self.check(&Token::Dedent) || self.check(&Token::Eof)
                || self.check(&Token::RParen) || self.check(&Token::RBrack)
                || self.check(&Token::RBrace) || self.check(&Token::If) {
                None
            } else {
                Some(Box::new(self.parse_or()?))
            };
            return Ok(Expr::Range {
                start: Some(Box::new(left)),
                end,
                inclusive,
            });
        }
        Ok(left)
    }

    fn is_ternary_after(&self) -> bool {
        // `?` 后紧跟一个表达式开始（三元 true 分支）→ 是三元中缀
        // 注意：当前位置是 `?`，需检查其后的 token
        match self.peek_n(1) {
            Token::IntLit(_) | Token::FloatLit(_) | Token::StrLit(_) | Token::FStrLit(_)
            | Token::RawStrLit(_) | Token::TripleStrLit(_) | Token::True | Token::False
            | Token::Ident(_) | Token::MagicMethod(_) | Token::Underscore | Token::Self_
            | Token::LParen | Token::LBrack | Token::LBrace | Token::If
            | Token::Not | Token::Minus | Token::Plus | Token::Pipe | Token::BackPipe
            | Token::Try | Token::Async => true,
            _ => false,
        }
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.check(&Token::Or) || self.check(&Token::PipePipe) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::Or, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_not()?;
        while self.check(&Token::And) || self.check(&Token::AmpAmp) {
            self.advance();
            let right = self.parse_not()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::And, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, String> {
        if self.check(&Token::Not) {
            self.advance();
            let operand = self.parse_not()?;
            return Ok(Expr::Unary { op: UnaryOp::Not, operand: Box::new(operand) });
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_in_is()?;
        loop {
            let op = match self.peek() {
                Token::EqEq => BinOp::Eq,
                Token::NotEq => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Le => BinOp::Le,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_in_is()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_in_is(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_pipe()?;
        loop {
            match self.peek() {
                Token::In => {
                    self.advance();
                    let right = self.parse_pipe()?;
                    left = Expr::Binary { left: Box::new(left), op: BinOp::In, right: Box::new(right) };
                }
                Token::Not => {
                    self.advance();
                    // not in 运算符
                    self.expect(Token::In)?;
                    let right = self.parse_pipe()?;
                    left = Expr::Binary { left: Box::new(left), op: BinOp::NotIn, right: Box::new(right) };
                }
                Token::Is => {
                    self.advance();
                    let right = self.parse_pipe()?;
                    left = Expr::Binary { left: Box::new(left), op: BinOp::Is, right: Box::new(right) };
                }
                Token::As => {
                    self.advance();
                    // 解析 as 右侧的类型
                    let ty = self.parse_type()?;
                    let ty_name = ty.to_string();
                    left = Expr::Call { type_args: vec![],
                        func: Box::new(Expr::Ident("__as__".to_string())),
                        args: vec![left, Expr::Ident(ty_name)],
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_pipe(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_null_coalesce()?;
        while self.check(&Token::Pipe) {
            self.advance();
            // 右侧是函数调用: x |> f(args) 或 x |> (|y| expr)
            if self.check(&Token::LParen) {
                // 括号包裹的表达式: x |> (|y| expr)
                self.advance(); // consume (
                let expr = self.parse_expr()?;
                // 跳过可能的外层 Dedent（Lambda body 中的 match 块消费了 case Dedent）
                self.skip_newlines();
                while self.check(&Token::Dedent) {
                    self.advance();
                    self.skip_newlines();
                }
                self.expect(Token::RParen)?;
                // 将 receiver 作为参数调用括号内的函数
                left = Expr::Call {
                    type_args: vec![],
                    func: Box::new(expr),
                    args: vec![left],
                };
                continue;
            }
            let func_name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected function after |>, got {:?}", t)),
            };
            let args = if self.check(&Token::LParen) {
                self.advance();
                let mut args = Vec::new();
                while !self.check(&Token::RParen) {
                    args.push(self.parse_expr()?);
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RParen)?;
                args
            } else {
                vec![left.clone()]  // 无括号时，将 receiver 作为参数传入
            };
            left = Expr::Pipe { receiver: Box::new(left), func: func_name, args };
        }
        Ok(left)
    }

    fn parse_null_coalesce(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bit_or()?;
        while self.check(&Token::QuestionQuestion) {
            self.advance();
            let right = self.parse_bit_or()?;
            left = Expr::NullCoalesce { left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bit_xor()?;
        while self.check(&Token::Pipe_) {
            self.advance();
            let right = self.parse_bit_xor()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::BitOr, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, String> {
        // ^ 既作中缀 XOR，也作后缀 move（y^）；此处只处理中缀，后缀在 parse_postfix 消歧
        // CaretInfix（前置留白的 `a ^`）强制走中缀：缺右操作数时 parse_bit_and 报错（悬空 ^）。
        let mut left = self.parse_bit_and()?;
        while self.check(&Token::CaretOp) || self.check(&Token::CaretInfix) {
            self.advance();
            let right = self.parse_bit_and()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::BitXor, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_shift()?;
        while self.check(&Token::Amp) {
            self.advance();
            let right = self.parse_shift()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::BitAnd, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Token::Shl => BinOp::Shl,
                Token::Shr => BinOp::Shr,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_power()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_power()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        let left = self.parse_unary()?;
        if self.check(&Token::StarStar) {
            self.advance();
            let right = self.parse_power()?; // 右结合
            return Ok(Expr::Binary { left: Box::new(left), op: BinOp::Pow, right: Box::new(right) });
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary { op: UnaryOp::Neg, operand: Box::new(operand) })
            }
            Token::Plus => {
                self.advance();
                // 一元 + 不改变值，直接返回操作数
                self.parse_unary()
            }
            Token::Exclamation => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary { op: UnaryOp::BitNot, operand: Box::new(operand) })
            }
            Token::Not => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary { op: UnaryOp::Not, operand: Box::new(operand) })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let expr = self.parse_primary()?;
        self.parse_postfix_chain(expr)
    }

    /// 从已有表达式继续解析 postfix（跨行方法链用）
    fn parse_postfix_chain(&mut self, mut expr: Expr) -> Result<Expr, String> {
        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    // turbofish: parse.<int>("42")
                    if self.check(&Token::Lt) {
                        self.advance(); // <
                        let mut depth = 1;
                        while depth > 0 && !self.check(&Token::Eof) {
                            match self.peek() {
                                Token::Lt => depth += 1,
                                Token::Gt | Token::Shr => {
                                    depth -= 1;
                                    if depth == 0 { self.advance(); break; }
                                }
                                _ => {}
                            }
                            self.advance();
                        }
                        // Parse LParen args after >
                        if self.check(&Token::LParen) {
                            self.advance();
                            let mut args = Vec::new();
                            while !self.check(&Token::RParen) {
                                args.push(self.parse_expr()?);
                                if self.check(&Token::Comma) { self.advance(); }
                            }
                            self.expect(Token::RParen)?;
                            expr = Expr::Call { type_args: vec![], func: Box::new(expr), args };
                        }
                        continue;
                    }
                    let name = match self.advance() {
                        Token::Ident(n) => n,
                        Token::MagicMethod(n) => n,
                        Token::True => "True".to_string(),
                        Token::False => "False".to_string(),
                        t => return Err(format!("Expected field/method, got {:?}", t)),
                    };
                    if self.check(&Token::LParen) {
                        self.advance();
                        let mut args = Vec::new();
                        while !self.check(&Token::RParen) {
                        // 跳过参数间的换行和缩进
                        self.skip_newlines();
                        if self.check(&Token::Indent) { self.advance(); }
                        if self.check(&Token::Dedent) { break; }
                        // 关键字参数: name: value 或 name~ 语法糖
                        let arg = if let Token::Ident(_) = self.peek() {
                            if self.peek_n(1) == &Token::Tilde {
                                let name = self.advance().to_string();
                                self.advance(); // 消费 ~
                                Expr::KwArg { name: name.clone(), value: Box::new(Expr::Ident(name)) }
                            } else if self.peek_n(1) == &Token::Colon {
                                let name = self.advance().to_string();
                                self.advance(); // 消费 :
                                let v = self.parse_expr()?;
                                Expr::KwArg { name, value: Box::new(v) }
                            } else {
                                self.parse_expr()?
                            }
                        } else {
                            self.parse_expr()?
                        };
                            args.push(arg);
                            if self.check(&Token::Comma) { self.advance(); }
                        }
                        // Dedent 表示参数列表的缩进块结束，跳过它
                        if self.check(&Token::Dedent) { self.advance(); }
                        self.expect(Token::RParen)?;
                        expr = Expr::MethodCall { receiver: Box::new(expr), method: name, args };
                    } else {
                        expr = Expr::FieldAccess { receiver: Box::new(expr), field: name };
                    }
                }
                Token::PathSep => {
                    self.advance();
                    let seg = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected path segment, got {:?}", t)),
                    };
                    if self.check(&Token::LParen) {
                        self.advance();
                        let mut args = Vec::new();
                        while !self.check(&Token::RParen) {
                            self.skip_newlines();
                            if self.check(&Token::Indent) { self.advance(); }
                            if self.check(&Token::Dedent) { break; }
                            args.push(self.parse_expr()?);
                            if self.check(&Token::Comma) { self.advance(); }
                        }
                        if self.check(&Token::Dedent) { self.advance(); }
                        self.expect(Token::RParen)?;
                        expr = Expr::Call { type_args: vec![],
                            func: Box::new(Expr::PathAccess {
                                receiver: Box::new(expr),
                                segment: seg,
                            }),
                            args,
                        };
                    } else {
                        expr = Expr::PathAccess { receiver: Box::new(expr), segment: seg };
                    }
                }
                Token::Lt => {
                    // 泛型调用: func<Type>(args)
                    // 仅在当前表达式是 Ident（函数名）且后面跟着 > 和 ( 时处理
                    if !matches!(&expr, Expr::Ident(_)) {
                        break;
                    }
                    // peek ahead: 检查是否是泛型调用模式
                    // 模式: Ident < Type (,...)? > (
                    let mut is_generic_call = false;
                    let mut idx = 1usize;
                    loop {
                        match self.peek_n(idx) {
                            Token::Gt | Token::Shr => {
                                // 检查 > 后面是否是 (
                                let after_gt = if matches!(self.peek_n(idx), Token::Shr) { idx + 1 } else { idx + 1 };
                                if matches!(self.peek_n(after_gt), Token::LParen) {
                                    is_generic_call = true;
                                }
                                break;
                            }
                            Token::Comma => { idx += 1; }
                            Token::Ident(_) | Token::IntLit(_) | Token::StrLit(_) | Token::True | Token::False => { idx += 1; }
                            Token::LBrack => {
                                // 跳过 [Type] 如 List[int]
                                idx += 1;
                                let mut depth = 1;
                                while depth > 0 {
                                    match self.peek_n(idx) {
                                        Token::RBrack => { depth -= 1; idx += 1; }
                                        Token::Eof => break,
                                        _ => { idx += 1; }
                                    }
                                }
                            }
                            _ => break,
                        }
                    }
                    if !is_generic_call {
                        break;
                    }
                    // 确认是泛型调用，现在解析类型参数
                    let _ = self.advance(); // 消费 <
                    let mut type_args: Vec<Type> = Vec::new();
                    loop {
                        match self.peek() {
                            Token::Gt | Token::Shr | Token::Eof => break,
                            _ => {}
                        }
                        type_args.push(self.parse_type()?);
                        if self.check(&Token::Comma) {
                            let _ = self.advance();
                        } else {
                            break;
                        }
                    }
                    // 处理 >> 作为两个 > 的情况
                    if self.check(&Token::Shr) {
                        let _ = self.advance(); // 消费 >>（作为单个 >）
                    } else {
                        let _ = self.expect(Token::Gt)?;
                    }
                    // 解析参数
                    let _ = self.expect(Token::LParen)?;
                    let mut args = Vec::new();
                    while !self.check(&Token::RParen) {
                        args.push(self.parse_expr()?);
                        if self.check(&Token::Comma) { let _ = self.advance(); }
                    }
                    let _ = self.expect(Token::RParen)?;
                    let type_arg_names: Vec<String> = type_args.iter().map(|t| t.to_string()).collect();
                    expr = Expr::Call { type_args: type_arg_names, func: Box::new(expr), args };
                }
                Token::LParen => {
                    self.advance();
                    self.skip_newlines();
                    // 多行 struct 构造: Name(\n    field: value\n    ...)
                    let args = if self.check(&Token::Indent) {
                        self.advance();
                        let mut args = Vec::new();
                        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                            // 关键字参数: name: value 或 name~ 语法糖
                            let arg = if let Token::Ident(_) = self.peek() {
                                if self.peek_n(1) == &Token::Tilde {
                                    let name = self.advance().to_string();
                                    self.advance(); // ~
                                    Expr::KwArg { name: name.clone(), value: Box::new(Expr::Ident(name)) }
                                } else if self.peek_n(1) == &Token::Colon {
                                    let name = self.advance().to_string();
                                    self.advance(); // :
                                    let v = self.parse_expr()?;
                                    Expr::KwArg { name, value: Box::new(v) }
                                } else {
                                    self.parse_expr()?
                                }
                            } else {
                                self.parse_expr()?
                            };
                            args.push(arg);
                            if self.check(&Token::Comma) { self.advance(); }
                            self.skip_newlines();
                        }
                        self.expect(Token::Dedent)?;
                        // 消费闭合的 RParen（Dedent 后紧跟 )）
                        if self.check(&Token::RParen) { self.advance(); }
                        args
                    } else {
                        let mut args = Vec::new();
                        while !self.check(&Token::RParen) {
                            self.skip_newlines();
                            if self.check(&Token::Indent) { self.advance(); }
                            if self.check(&Token::Dedent) { break; }
                            // 关键字参数: name: value 或 name~ 语法糖
                            let arg = if let Token::Ident(_) = self.peek() {
                                if self.peek_n(1) == &Token::Tilde {
                                    let name = self.advance().to_string();
                                    self.advance(); // ~
                                    Expr::KwArg { name: name.clone(), value: Box::new(Expr::Ident(name)) }
                                } else if self.peek_n(1) == &Token::Colon {
                                    let name = self.advance().to_string();
                                    self.advance(); // :
                                    let v = self.parse_expr()?;
                                    Expr::KwArg { name, value: Box::new(v) }
                                } else {
                                    self.parse_expr()?
                                }
                            } else {
                                self.parse_expr()?
                            };
                            args.push(arg);
                            if self.check(&Token::Comma) { self.advance(); }
                        }
                        if self.check(&Token::Dedent) { self.advance(); }
                        self.expect(Token::RParen)?;
                        args
                    };
                    expr = Expr::Call { type_args: vec![], func: Box::new(expr), args };
                }
                Token::LBrace => {
                    // Struct ctor: Point{x: 10, y: 20} 或 Point{x~, y~}
                    self.advance();
                    let mut args = Vec::new();
                    while !self.check(&Token::RBrace) {
                        let name = match self.advance() {
                            Token::Ident(n) => n,
                            t => return Err(format!("Expected field name in struct ctor, got {:?}", t)),
                        };
                        if self.check(&Token::Tilde) {
                            self.advance(); // ~
                            args.push(Expr::KwArg { name: name.clone(), value: Box::new(Expr::Ident(name)) });
                        } else {
                            self.expect(Token::Colon)?;
                            let v = self.parse_expr()?;
                            args.push(Expr::KwArg { name, value: Box::new(v) });
                        }
                        if self.check(&Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RBrace)?;
                    expr = Expr::Call { type_args: vec![], func: Box::new(expr), args };
                }
                Token::LBrack => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBrack)?;
                    expr = Expr::Index { receiver: Box::new(expr), index: Box::new(index) };
                }
                Token::Question => {
                    // 区分三元 `cond ? a : b` 与错误传播 `expr?`：
                    // 若 `?` 后紧跟一个表达式开始（三元 true 分支），则交由
                    // parse_expr 处理三元；否则为 Try 错误传播后缀。
                    if self.is_ternary_after() {
                        break;
                    }
                    self.advance();
                    expr = Expr::Try(Box::new(expr));
                }
                Token::SafeNav => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected field name, got {:?}", t)),
                    };
                    expr = Expr::SafeNav { receiver: Box::new(expr), field };
                }
                // ^ 后缀 move（y^）：紧邻标识符的 CaretOp（无前置留白）
                Token::CaretOp => {
                    self.advance();
                    expr = Expr::Move(Box::new(expr));
                }
                // await 后缀：仅紧邻标识符/调用（无换行 / 无前置留白）
                Token::Await if !self.check(&Token::Newline) => {
                    self.advance();
                    expr = Expr::Await(Box::new(expr));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let tok = self.advance();
        match tok {
            // 跳过缩进 token（用于 Lambda body 跨行时外层的缩进）
            Token::Indent => self.parse_primary(),
            Token::IntLit(n) => Ok(Expr::IntLit(n)),
            Token::FloatLit(f) => Ok(Expr::FloatLit(f)),
            Token::StrLit(s) => Ok(Expr::StrLit(s)),
            Token::FStrLit(s) => {
                validate_fstring(&s)?;
                Ok(Expr::FStrLit(s))
            }
            Token::RawStrLit(s) => Ok(Expr::RawStrLit(s)),
            Token::True => Ok(Expr::BoolLit(true)),
            Token::False => Ok(Expr::BoolLit(false)),
            Token::From => Ok(Expr::Ident("from".to_string())),
            Token::Ident(name) => Ok(Expr::Ident(name)),
            Token::Underscore => Ok(Expr::Ident("_".to_string())),
            Token::Self_ => Ok(Expr::Ident("self".to_string())),
            Token::LParen => {
                // 空括号 () → 单元
                if self.check(&Token::RParen) {
                    self.advance();
                    return Ok(Expr::TupleLit(Vec::new()));
                }
                let first = self.parse_expr()?;
                // 单元素括号 (e) vs 元组 (e,)
                if self.check(&Token::Comma) {
                    let mut items = vec![first];
                    self.advance();
                    while !self.check(&Token::RParen) {
                        items.push(self.parse_expr()?);
                        if self.check(&Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::TupleLit(items))
                } else {
                    self.expect(Token::RParen)?;
                    Ok(Expr::Paren(Box::new(first))) // 保留括号分组信息
                }
            }
            Token::LBrack => {
                // [] 空列表
                if self.check(&Token::RBrack) {
                    self.advance();
                    return Ok(Expr::ListLit(Vec::new()));
                }
                let first = self.parse_expr()?;
                // 推导式: [output for var in iter if cond]
                // 支持多个 for 子句: [x * y for x in 0..N for y in 0..N if cond]
                if self.check(&Token::For) {
                    let mut clauses = Vec::new();
                    loop {
                        self.advance(); // for
                        let var = match self.advance() {
                            Token::Ident(n) => n,
                            t => return Err(format!("Expected var, got {:?}", t)),
                        };
                        self.expect(Token::In)?;
                        let iter = self.parse_comprehension_iter()?;
                        let cond = if self.check(&Token::If) {
                            self.advance();
                            Some(Box::new(self.parse_expr()?))
                        } else {
                            None
                        };
                        clauses.push((var, Box::new(iter), cond));
                        if !self.check(&Token::For) { break; }
                    }
                    self.expect(Token::RBrack)?;
                    return Ok(Expr::ListComprehension {
                        output: Box::new(first),
                        var: clauses[0].0.clone(),
                        iter: clauses[0].1.clone(),
                        cond: clauses[0].2.clone(),
                        extra_clauses: clauses[1..].to_vec(),
                    });
                }
                // 列表字面量
                let mut items = vec![first];
                if self.check(&Token::Comma) { self.advance(); }
                while !self.check(&Token::RBrack) {
                    items.push(self.parse_expr()?);
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RBrack)?;
                Ok(Expr::ListLit(items))
            }
            Token::LBrace => {
                // {} 空字典
                if self.check(&Token::RBrace) {
                    self.advance();
                    return Ok(Expr::DictLit(Vec::new()));
                }
                let first = self.parse_expr()?;
                // 字典推导: {k: v for x in iter}
                if self.check(&Token::Colon) {
                    self.advance();
                    let val = self.parse_expr()?;
                    if self.check(&Token::For) {
                        // Dict comprehension: {k: v for var in iter if cond}
                        self.advance(); // for
                        let var = match self.advance() {
                            Token::Ident(n) => n,
                            t => return Err(format!("Expected var in dict comprehension, got {:?}", t)),
                        };
                        self.expect(Token::In)?;
                    let iter = self.parse_comprehension_iter()?;
                    let cond = if self.check(&Token::If) {
                        self.advance();
                        Some(Box::new(self.parse_expr()?))
                    } else {
                        None
                    };
                    self.expect(Token::RBrace)?;
                    return Ok(Expr::DictComprehension {
                            key: Box::new(first),
                            value: Box::new(val),
                            var,
                            iter: Box::new(iter),
                            cond,
                        });
                    }
                    // 常规字典字面量 {k: v, k2: v2}
                    let mut entries = vec![(first, val)];
                    if self.check(&Token::Comma) { self.advance(); }
                    while !self.check(&Token::RBrace) {
                        let k = self.parse_expr()?;
                        self.expect(Token::Colon)?;
                        let v = self.parse_expr()?;
                        entries.push((k, v));
                        if self.check(&Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RBrace)?;
                    Ok(Expr::DictLit(entries))
                } else if self.check(&Token::For) {
                    // Set comprehension: {x for var in iter if cond}
                    self.advance(); // for
                    let var = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected var in set comprehension, got {:?}", t)),
                    };
                    self.expect(Token::In)?;
                let iter = self.parse_comprehension_iter()?;
                let cond = if self.check(&Token::If) {
                    self.advance();
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                self.expect(Token::RBrace)?;
                Ok(Expr::SetComprehension {
                        elem: Box::new(first),
                        var,
                        iter: Box::new(iter),
                        cond,
                    })
                } else {
                    // 集合字面量 {a, b, c}
                    let mut items = vec![first];
                    if self.check(&Token::Comma) { self.advance(); }
                    while !self.check(&Token::RBrace) {
                        items.push(self.parse_expr()?);
                        if self.check(&Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RBrace)?;
                    Ok(Expr::SetLit(items))
                }
            }
            Token::If => {
                let cond = self.parse_expr()?;
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let then_body = self.parse_block()?;
                self.expect(Token::Dedent)?;
                // elif / else
                let mut elif_clauses = Vec::new();
                let mut else_body = None;
                while self.check(&Token::Elif) {
                    self.advance();
                    let elif_cond = self.parse_expr()?;
                    self.expect(Token::Colon)?;
                    self.skip_newlines();
                    self.expect(Token::Indent)?;
                    let elif_body = self.parse_block()?;
                    self.expect(Token::Dedent)?;
                    elif_clauses.push((elif_cond, elif_body));
                }
                if self.check(&Token::Else) {
                    self.advance();
                    self.expect(Token::Colon)?;
                    self.skip_newlines();
                    if self.check(&Token::If) {
                        // else if 嵌套 → 转为 elif
                        self.advance();
                        let else_if_cond = self.parse_expr()?;
                        self.expect(Token::Colon)?;
                        self.skip_newlines();
                        self.expect(Token::Indent)?;
                        let else_if_body = self.parse_block()?;
                        self.expect(Token::Dedent)?;
                        elif_clauses.push((else_if_cond, else_if_body));
                    } else {
                        self.expect(Token::Indent)?;
                        else_body = Some(self.parse_block()?);
                        self.expect(Token::Dedent)?;
                    }
                }
                Ok(Expr::If { cond: Box::new(cond), then_body, elif_clauses, else_body })
            }
            Token::Block => {
                // block label: body — 命名块表达式
                self.advance();
                // 标签名可以是任意标识符或关键字（如 test, match 等）
                let label = self.advance().to_string();
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let body = self.parse_block()?;
                self.expect(Token::Dedent)?;
                Ok(Expr::BlockExpr(body))
            }
            Token::Match => {
                let expr = self.parse_expr()?;
                if !self.check(&Token::Colon) {
                    eprintln!("[DEBUG Match FAIL] after expr, next={:?}", self.peek());
                }
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let mut arms = Vec::new();
                while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                    self.skip_newlines();
                    if self.check(&Token::Dedent) || self.check(&Token::Eof) { break; }

                    // 可选 case 关键字
                    if self.check(&Token::Case) { self.advance(); }

                    // 模式支持 | 分隔多模式
                    let mut patterns = Vec::new();
                    loop {
                        // 尝试解析模式；如果失败则可能是缺少 Delim 导致，跳出
                        match self.parse_pattern() {
                            Ok(p) => patterns.push(p),
                            Err(e) => return Err(format!("{} (in match arm pattern)", e)),
                        }
                        if self.check(&Token::Pipe_) || self.check(&Token::PipePipe) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let guard = if self.check(&Token::If) {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    // 接受 : 或 => 作为 arm 分隔符（所有模式共享同一个 body）
                    if self.check(&Token::FatArrow) {
                        self.advance();
                    } else {
                        self.expect(Token::Colon)?;
                    }
                    self.skip_newlines();
                    let body = if self.check(&Token::Indent) {
                        self.advance();
                        let b = self.parse_block()?;
                        self.expect(Token::Dedent)?;
                        b
                    } else {
                        vec![self.parse_stmt()?]
                    };
                    // 对于多模式，每个模式复制一份 arm（简化处理）
                    for pat in &patterns {
                        arms.push(MatchArm { pattern: pat.clone(), guard: guard.clone(), body: body.clone() });
                    }
                    self.skip_newlines();
                }
                self.expect(Token::Dedent)?;
                Ok(Expr::Match { expr: Box::new(expr), arms })
            }
            Token::PipePipe => {
                // 空参数闭包: || expr
                // 跳过换行，支持跨行表达式
                self.skip_newlines();
                // 支持可选的返回类型注解: || -> int = body
                if self.check(&Token::Arrow) {
                    self.advance(); // consume ->
                    self.parse_type()?; // skip return type annotation
                }
                // 支持可选的 = 分隔符: || -> int = body
                if self.check(&Token::Eq) {
                    self.advance(); // consume =
                }
                self.skip_newlines();
                if self.check(&Token::Indent) {
                    self.advance(); // skip Indent
                }
                let body = self.parse_expr()?;
                Ok(Expr::Closure { params: Vec::new(), body: Box::new(body) })
            }
            Token::Pipe_ => {
                // 闭包: |x, y| x + y  或  |x: int, y: int| -> int = x + y
                let mut params = Vec::new();
                while !self.check(&Token::Pipe_) {
                    match self.advance() {
                        Token::Ident(n) => {
                            params.push(n);
                            // 支持可选的类型注解: |x: int|
                            if self.check(&Token::Colon) {
                                self.advance(); // consume :
                                self.parse_type()?; // skip type annotation
                            }
                        }
                        t => return Err(format!("Expected param, got {:?}", t)),
                    }
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.advance(); // consume |
                // 支持可选的返回类型注解: |x| -> int = ...
                if self.check(&Token::Arrow) {
                    self.advance(); // consume ->
                    self.parse_type()?; // skip return type annotation
                }
                // 支持可选的 = 或 => 分隔符: |x| = body  或  |x| => block
                let has_fat_arrow = self.check(&Token::FatArrow);
                if self.check(&Token::Eq) || has_fat_arrow {
                    self.advance(); // consume = or =>
                }
                self.skip_newlines();
                if has_fat_arrow {
                    // => 后是块体（支持多语句缩进块）
                    if self.check(&Token::Indent) {
                        self.advance(); // skip Indent
                        let stmts = self.parse_block()?;
                        Ok(Expr::Closure { params, body: Box::new(Expr::BlockExpr(stmts)) })
                    } else {
                        let body = self.parse_expr()?;
                        Ok(Expr::Closure { params, body: Box::new(body) })
                    }
                } else {
                    // = 后是表达式体
                    // 如果 body 在缩进块内（如 let 语句中），跳过外层的 Indent
                    if self.check(&Token::Indent) {
                        self.advance(); // skip Indent
                    }
                    let body = self.parse_expr()?;
                    Ok(Expr::Closure { params, body: Box::new(body) })
                }
            }
            Token::Try => {
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let mut body = Vec::new();
                while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                    self.skip_newlines();
                    if self.check(&Token::Dedent) || self.check(&Token::Eof) { break; }
                    // 允许直接 catch 在新的 try-catch 中
                    if self.check(&Token::Catch) || self.check(&Token::Else) || self.check(&Token::Finally) {
                        break;
                    }
                    body.push(self.parse_stmt()?);
                }
                self.expect(Token::Dedent)?;

                let mut catches = Vec::new();
                while self.check(&Token::Catch) {
                    self.advance();
                    let pattern = if self.check(&Token::Pipe_)
                        || matches!(self.peek(), Token::Ident(_))
                        || matches!(self.peek(), Token::Self_)
                        || self.check(&Token::Underscore)
                        || matches!(self.peek(), Token::IntLit(_))
                    {
                        self.parse_pattern()?
                    } else {
                        Pattern::Wildcard
                    };
                    let guard = if self.check(&Token::If) {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    self.expect(Token::Colon)?;
                    self.skip_newlines();
                    self.expect(Token::Indent)?;
                    let catch_body = self.parse_block()?;
                    self.expect(Token::Dedent)?;
                    catches.push(MatchArm { pattern, guard, body: catch_body });
                }

                let else_body = if self.check(&Token::Else) {
                    self.advance();
                    self.expect(Token::Colon)?;
                    self.skip_newlines();
                    self.expect(Token::Indent)?;
                    let b = self.parse_block()?;
                    self.expect(Token::Dedent)?;
                    Some(b)
                } else {
                    None
                };

                let finally_body = if self.check(&Token::Finally) {
                    self.advance();
                    self.expect(Token::Colon)?;
                    self.skip_newlines();
                    self.expect(Token::Indent)?;
                    let b = self.parse_block()?;
                    self.expect(Token::Dedent)?;
                    Some(b)
                } else {
                    None
                };

                Ok(Expr::TryCatch { body, catches, else_body, finally_body })
            }
            Token::Spawn | Token::Go => {
                let expr = self.parse_expr()?;
                Ok(Expr::Spawn(Box::new(expr)))
            }
            Token::Await => {
                let inner = self.parse_expr()?;
                Ok(Expr::Await(Box::new(inner)))
            }
            Token::MagicMethod(name) => {
                // __name__, __doc__, __is_macro__ 等魔法标识符
                // 作为普通标识符表达式使用
                Ok(Expr::Ident(name))
            }
            _ => Err(format!("Unexpected token in expression: {:?}", tok)),
        }
    }

    /// 解析 pattern（match arm / guard let 等）
    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        // 跳过 ref/mut 模式修饰符（语义上忽略，仅语法兼容）
        while self.check(&Token::Ref) || self.check(&Token::Mut) {
            self.advance();
        }
        let first = self.advance();
        match first {
            // 负数模式: case -999 =>
            Token::Minus => {
                match self.advance() {
                    Token::IntLit(n) => Ok(Pattern::Int(-n)),
                    t => Err(format!("Expected integer after minus in pattern, got {:?}", t)),
                }
            }
            Token::IntLit(n) => {
                // 范围模式: n..m 或 n..=m
                if self.check(&Token::DotDot) {
                    self.advance(); // ..
                    match self.advance() {
                        Token::IntLit(end) => Ok(Pattern::Range { start: n, end, inclusive: false }),
                        t => Err(format!("Expected end of range pattern, got {:?}", t)),
                    }
                } else if self.check(&Token::DotDotEq) {
                    self.advance(); // ..=
                    match self.advance() {
                        Token::IntLit(end) => Ok(Pattern::Range { start: n, end, inclusive: true }),
                        t => Err(format!("Expected end of range pattern, got {:?}", t)),
                    }
                } else {
                    Ok(Pattern::Int(n))
                }
            }
            Token::FloatLit(f) => {
                Ok(Pattern::Str(f.to_string()))
            }
            Token::StrLit(s) => {
                Ok(Pattern::Str(s))
            }
            Token::True => Ok(Pattern::Bool(true)),
            Token::False => Ok(Pattern::Bool(false)),
            Token::Underscore => Ok(Pattern::Wildcard),
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
                        if matches!(self.peek(), Token::Ident(_)) && self.peek_n(1) == &Token::Colon {
                            self.advance(); // field name
                            self.advance(); // :
                            pats.push(self.parse_pattern()?);
                        } else {
                            pats.push(self.parse_pattern()?);
                        }
                        if self.check(&Token::Comma) { self.advance(); }
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
                } else {
                    Ok(Pattern::Ident(name))
                }
            }
            Token::LParen => {
                let mut patterns = Vec::new();
                while !self.check(&Token::RParen) {
                    patterns.push(self.parse_pattern()?);
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RParen)?;
                if patterns.len() == 1 {
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
                            Pattern::Ident(name)
                        } else {
                            Pattern::Wildcard
                        };
                        patterns.push(rest_pat);
                    } else {
                        patterns.push(self.parse_pattern()?);
                    }
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RBrack)?;
                Ok(Pattern::List(patterns))
            }
            Token::LBrace => {
                // 字典模式: {"port": p} — 键值对匹配
                let mut patterns = Vec::new();
                while !self.check(&Token::RBrace) {
                    // 跳过键（字符串字面量）
                    self.advance(); // key string
                    self.expect(Token::Colon)?; // :
                    patterns.push(self.parse_pattern()?);
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RBrace)?;
                Ok(Pattern::List(patterns))
            }
            _ => Err(format!("Unexpected token in pattern: {:?}", first)),
        }
    }
}
