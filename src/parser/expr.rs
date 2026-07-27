// Lang-Zong 编译器 — parser/expr.rs
// 表达式解析（优先级递降）+ Pattern 解析（作为 Parser 的 trait 扩展）

use crate::lexer::Token;
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
    fn parse_primary(&mut self) -> Result<Expr, String>;
    fn parse_pattern(&mut self) -> Result<Pattern, String>;
}

impl ParserExprExt for Parser {
    // ─── 表达式解析（优先级递降） ───

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let left = self.parse_or()?;
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

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.check(&Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::Or, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_not()?;
        while self.check(&Token::And) {
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
                Token::Is => {
                    self.advance();
                    let right = self.parse_pipe()?;
                    left = Expr::Binary { left: Box::new(left), op: BinOp::Is, right: Box::new(right) };
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
            // 右侧是函数调用
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
                Vec::new()
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
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    let name = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected field/method, got {:?}", t)),
                    };
                    if self.check(&Token::LParen) {
                        self.advance();
                        let mut args = Vec::new();
                        while !self.check(&Token::RParen) {
                            // 关键字参数: name: value
                            let arg = if let Token::Ident(_) = self.peek() {
                                if self.peek_n(1) == &Token::Colon {
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
                            args.push(self.parse_expr()?);
                            if self.check(&Token::Comma) { self.advance(); }
                        }
                        self.expect(Token::RParen)?;
                        expr = Expr::Call {
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
                Token::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    while !self.check(&Token::RParen) {
                        // 关键字参数: name: value
                        let arg = if let Token::Ident(_) = self.peek() {
                            if self.peek_n(1) == &Token::Colon {
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
                    self.expect(Token::RParen)?;
                    expr = Expr::Call { func: Box::new(expr), args };
                }
                Token::LBrack => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBrack)?;
                    expr = Expr::Index { receiver: Box::new(expr), index: Box::new(index) };
                }
                Token::Question => {
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
            Token::None_ => Ok(Expr::NoneLit),
            Token::Some_ => {
                self.expect(Token::LParen)?;
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call { func: Box::new(Expr::Ident("Some".into())), args: vec![inner] })
            }
            Token::Ok_ => {
                self.expect(Token::LParen)?;
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call { func: Box::new(Expr::Ident("Ok".into())), args: vec![inner] })
            }
            Token::Err_ => {
                self.expect(Token::LParen)?;
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call { func: Box::new(Expr::Ident("Err".into())), args: vec![inner] })
            }
            Token::Ident(name) => {
                // 检查是否是泛型调用: foo<T>(args)
                if self.check(&Token::Lt) {
                    let turbofish = self.collect_turbofish_args()?;
                    if self.check(&Token::LParen) {
                        self.advance();
                        let mut args = Vec::new();
                        while !self.check(&Token::RParen) {
                            args.push(self.parse_expr()?);
                            if self.check(&Token::Comma) { self.advance(); }
                        }
                        self.expect(Token::RParen)?;
                        return Ok(Expr::Call {
                            func: Box::new(Expr::Ident(
                                format!("{}::<{}>", name, turbofish.join(", "))
                            )),
                            args,
                        });
                    }
                }
                Ok(Expr::Ident(name))
            }
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
                    Ok(first) // 括号仅用于分组
                }
            }
            Token::LBrack => {
                // [] 空列表
                if self.check(&Token::RBrack) {
                    self.advance();
                    return Ok(Expr::ListLit(Vec::new()));
                }
                let first = self.parse_expr()?;
                // 推导式: [x for x in iter]
                if let Token::For = self.peek() {
                    // 简单推导式: [output for var in iter]
                    self.advance(); // for
                    let var = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected var, got {:?}", t)),
                    };
                    self.expect(Token::In)?;
                    let iter = self.parse_expr()?;
                    let cond = if self.check(&Token::If) {
                        self.advance();
                        Some(Box::new(self.parse_expr()?))
                    } else {
                        None
                    };
                    self.expect(Token::RBrack)?;
                    return Ok(Expr::ListComprehension {
                        output: Box::new(first),
                        var,
                        iter: Box::new(iter),
                        cond,
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
                // 是否是集合字面量？ {a, b, c}  vs 字典字面量 {k: v, k2: v2}
                if self.check(&Token::Colon) {
                    self.advance();
                    let val = self.parse_expr()?;
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
            Token::Match => {
                let expr = self.parse_expr()?;
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let mut arms = Vec::new();
                while self.check(&Token::Case) || self.check(&Token::Pipe_) {
                    if self.check(&Token::Case) { self.advance(); }
                    // 模式支持 | 分隔多模式
                    let mut patterns = Vec::new();
                    loop {
                        patterns.push(self.parse_pattern()?);
                        if self.check(&Token::Pipe_) {
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
                    // 对于多模式，每个模式复制一份 arm（简化处理）
                    for pat in patterns {
                        // 接受 : 或 => 作为 arm 分隔符
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
                        arms.push(MatchArm { pattern: pat, guard: guard.clone(), body });
                    }
                    self.skip_newlines();
                }
                self.expect(Token::Dedent)?;
                Ok(Expr::Match { expr: Box::new(expr), arms })
            }
            Token::Pipe_ => {
                // 闭包: |x, y| x + y
                let mut params = Vec::new();
                while !self.check(&Token::Pipe_) {
                    match self.advance() {
                        Token::Ident(n) => params.push(n),
                        t => return Err(format!("Expected param, got {:?}", t)),
                    }
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.advance(); // consume |
                let body = self.parse_expr()?;
                Ok(Expr::Closure { params, body: Box::new(body) })
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
                        || matches!(self.peek(), Token::Some_)
                        || matches!(self.peek(), Token::Ok_)
                        || matches!(self.peek(), Token::Err_)
                        || matches!(self.peek(), Token::None_)
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
            Token::Spawn => {
                let expr = self.parse_expr()?;
                Ok(Expr::Spawn(Box::new(expr)))
            }
            Token::Panic => {
                self.expect(Token::LParen)?;
                let msg = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Panic(Box::new(msg)))
            }
            Token::Await => {
                let inner = self.parse_expr()?;
                Ok(Expr::Await(Box::new(inner)))
            }
            _ => Err(format!("Unexpected token in expression: {:?}", tok)),
        }
    }

    /// 解析 pattern（match arm / guard let 等）
    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        let first = self.advance();
        match first {
            Token::IntLit(n) => {
                Ok(Pattern::Int(n))
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
            Token::None_ => Ok(Pattern::Ident("None".to_string())),
            Token::Some_ | Token::Ok_ | Token::Err_ => {
                // 枚举变体模式（可能后跟 (patterns)）
                let name = first.to_string();
                let patterns = if self.check(&Token::LParen) {
                    self.advance();
                    let mut pats = Vec::new();
                    while !self.check(&Token::RParen) {
                        pats.push(self.parse_pattern()?);
                        if self.check(&Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RParen)?;
                    pats
                } else {
                    Vec::new()
                };
                if !patterns.is_empty() {
                    Ok(Pattern::Variant(name, patterns))
                } else {
                    Ok(Pattern::Ident(name))
                }
            }
            Token::Ident(n) => {
                // 元组变体: Some(x, y) 或多模式: Some(a) | None
                let patterns = if self.check(&Token::LParen) {
                    self.advance();
                    let mut pats = Vec::new();
                    while !self.check(&Token::RParen) {
                        pats.push(self.parse_pattern()?);
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

                if !patterns.is_empty() {
                    Ok(Pattern::Variant(n, patterns))
                } else {
                    Ok(Pattern::Ident(n))
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
            _ => Err(format!("Unexpected token in pattern: {:?}", first)),
        }
    }
}

/// 辅助方法：收集 turbofish 泛型参数 `::<T, U>`
impl Parser {
    fn collect_turbofish_args(&mut self) -> Result<Vec<String>, String> {
        self.expect(Token::Lt)?;
        let mut args = Vec::new();
        loop {
            match self.advance() {
                Token::Ident(n) => args.push(n),
                t => return Err(format!("Expected type in turbofish, got {:?}", t)),
            }
            if self.check(&Token::Comma) { self.advance(); }
            if self.check(&Token::Gt) {
                self.advance();
                break;
            }
            if self.check(&Token::Shr) {
                self.advance();
                self.pending_gt += 1;
                break;
            }
        }
        Ok(args)
    }
}
