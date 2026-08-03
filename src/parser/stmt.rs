// Lang-Zong 编译器 — parser/stmt.rs
// 语句解析 + 构建块语义验证（作为 Parser 的 trait 扩展）

use crate::lexer::Token;
use crate::ast::*;
use super::parser::Parser;
use super::expr::ParserExprExt;

/// Parser 的语句解析扩展 trait
pub trait ParserStmtExt {
    fn parse_block(&mut self) -> Result<Vec<Stmt>, String>;
    fn parse_stmt(&mut self) -> Result<Stmt, String>;
    fn parse_binding_stmt(&mut self) -> Result<Stmt, String>;
    fn parse_binding_stmt_let(&mut self) -> Result<Stmt, String>;
    fn parse_build_block_body(&mut self) -> Result<Vec<Stmt>, String>;
    /// 解析可能后跟构建块的值表达式：支持直接构建块（*:/~:/^:）和无值构建块（value *:/~:/^:）
    fn parse_maybe_build_value(&mut self) -> Result<Expr, String>;
    fn validate_build_block(&self, kind: BuildKind, body: &[Stmt]) -> Result<(), String>;
    fn collect_yields<'a>(&self, stmts: &'a [Stmt], out: &mut Vec<&'a Expr>, has_yield: &mut bool);
    fn first_yield<'a>(&self, stmts: &'a [Stmt]) -> Option<&'a Stmt>;
    fn build_block_payload<'a>(&self, body: &'a [Stmt]) -> Option<&'a Expr>;
    fn is_valid_build_params(&self, e: &Expr) -> bool;
}

impl ParserStmtExt for Parser {
    // ─── 语句块 ───

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            self.skip_newlines();
            if self.check(&Token::Dedent) || self.check(&Token::Eof) { break; }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek() {
            Token::Let => {
                // let x = 1  → 不可变绑定
                // let ref r = x  → 不可变引用
                self.advance();
                self.parse_binding_stmt_let()
            }
            Token::Mut | Token::Ref | Token::Const | Token::Owned => {
                self.parse_binding_stmt()
            }
            Token::Return => {
                self.advance();
                let expr = if !self.check(&Token::Newline) && !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                Ok(Stmt::Return(expr))
            }
            Token::Yield => {
                self.advance();
                // yield from expr
                if self.check(&Token::From) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    return Ok(Stmt::YieldFrom(expr));
                }
                let expr = if !self.check(&Token::Newline) && !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                Ok(Stmt::Yield(expr))
            }
            Token::Comptime => {
                // comptime: <缩进块> — 编译期求值块
                self.advance();
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let body = self.parse_block()?;
                self.expect(Token::Dedent)?;
                Ok(Stmt::Comptime { body })
            }
            Token::If => {
                let expr = self.parse_expr()?;
                Ok(Stmt::Expr(expr))
            }
            Token::Match => {
                let expr = self.parse_expr()?;
                Ok(Stmt::Expr(expr))
            }
            Token::While => {
                self.advance();
                let cond = self.parse_comprehension_iter()?;  // 不消费 if，避免与 guard 冲突
                // while cond if guard:
                let guard = if self.check(&Token::If) {
                    self.advance();
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let body = self.parse_block()?;
                self.expect(Token::Dedent)?;
                // while ... else:
                let else_body = if self.check(&Token::Else) {
                    self.advance();
                    self.expect(Token::Colon)?;
                    self.skip_newlines();
                    self.expect(Token::Indent)?;
                    let eb = self.parse_block()?;
                    self.expect(Token::Dedent)?;
                    Some(eb)
                } else {
                    None
                };
                Ok(Stmt::While { cond, guard, body, else_body })
            }
            Token::For => {
                self.advance();
                // 支持解构: for (idx, val) in iter → var = "(idx, val)"
                let var = if self.check(&Token::LParen) {
                    self.advance(); // (
                    let mut elems: Vec<String> = Vec::new();
                    while !self.check(&Token::RParen) && !self.check(&Token::Eof) {
                        match self.advance() {
                            Token::Ident(n) => elems.push(n),
                            Token::Underscore => elems.push("_".to_string()),
                            Token::Comma | Token::DotDotDot | Token::DotDot => {}
                            t => return Err(format!("Expected variable, got {:?}", t)),
                        }
                    }
                    self.expect(Token::RParen)?;
                    format!("({})", elems.join(", "))
                } else {
                    match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected variable, got {:?}", t)),
                    }
                };
                self.expect(Token::In)?;
                let iter = self.parse_comprehension_iter()?;
                // for var in iter if guard:
                let guard = if self.check(&Token::If) {
                    self.advance();
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let body = self.parse_block()?;
                self.expect(Token::Dedent)?;
                // for ... else:
                let else_body = if self.check(&Token::Else) {
                    self.advance();
                    self.expect(Token::Colon)?;
                    self.skip_newlines();
                    self.expect(Token::Indent)?;
                    let eb = self.parse_block()?;
                    self.expect(Token::Dedent)?;
                    Some(eb)
                } else {
                    None
                };
                Ok(Stmt::For { var, iter, guard, body, else_body })
            }
            Token::Loop => {
                self.advance();
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let body = self.parse_block()?;
                self.expect(Token::Dedent)?;
                Ok(Stmt::Loop(body))
            }
            Token::Break => {
                self.advance();
                let expr = if !self.check(&Token::Newline) && !self.check(&Token::Dedent) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                Ok(Stmt::Break(expr))
            }
            Token::Continue => {
                self.advance();
                Ok(Stmt::Continue)
            }
            Token::Guard => {
                self.advance();
                // 支持三种形式:
                //   guard cond else: block                (条件守卫，块形式)
                //   guard cond else expr                  (内联守卫)
                //   guard cond success_expr else fail_expr (带成功值的守卫)
                //   guard let PATTERN = EXPR else: VALUE  (模式守卫 → Rust let...else)
                let (cond, let_binding) = if self.check(&Token::Let) {
                    self.advance();
                    let pattern = self.parse_pattern()?;
                    self.expect(Token::Eq)?;
                    let expr = self.parse_expr()?;
                    (None, Some((pattern, expr)))
                } else {
                    (Some(self.parse_expr()?), None)
                };
                // guard cond success_expr else fail_expr: cond 后非 Else 即 success 值
                let success_expr = if !self.check(&Token::Else) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(Token::Else)?;
                let else_body = if self.check(&Token::Colon) {
                    self.advance();
                    self.skip_newlines();
                    if self.check(&Token::Indent) {
                        self.advance();
                        let b = self.parse_block()?;
                        self.expect(Token::Dedent)?;
                        b
                    } else {
                        vec![self.parse_stmt()?]
                    }
                } else {
                    // 内联形式: guard cond else VALUE
                    let val = self.parse_expr()?;
                    vec![Stmt::Expr(val)]
                };
                Ok(Stmt::Guard { cond, let_binding, success_expr, else_body })
            }
            Token::Defer => {
                self.advance();
                // 支持两种形式: defer: block 和 defer expr
                if self.check(&Token::Colon) {
                    self.advance();
                    self.skip_newlines();
                    let body = if self.check(&Token::Indent) {
                        self.advance();
                        let b = self.parse_block()?;
                        self.expect(Token::Dedent)?;
                        b
                    } else {
                        vec![self.parse_stmt()?]
                    };
                    Ok(Stmt::Defer(body))
                } else {
                    // 内联 defer: defer print("cleanup")
                    let expr = self.parse_expr()?;
                    Ok(Stmt::Defer(vec![Stmt::Expr(expr)]))
                }
            }
            Token::Raise => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Stmt::Raise(expr))
            }
            Token::With => {
                self.advance();
                // 使用 parse_pipe() 而非 parse_expr()，避免 'as' 被当作类型转换运算符吞掉
                // with expr as alias: — 'as' 在此上下文是别名绑定，不是类型转换
                let expr = self.parse_pipe()?;
                let alias = if self.check(&Token::As) {
                    self.advance();
                    match self.advance() {
                        Token::Ident(n) => Some(n),
                        t => return Err(format!("Expected alias, got {:?}", t)),
                    }
                } else {
                    None
                };
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let body = self.parse_block()?;
                self.expect(Token::Dedent)?;
                Ok(Stmt::With { expr, alias, body })
            }
            Token::Spawn | Token::Go => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Stmt::Expr(Expr::Spawn(Box::new(expr))))
            }
            Token::Test => {
                self.advance();
                let name = self.parse_test_name()?;
                self.expect(Token::Colon)?;
                self.skip_newlines();
                let body = if self.check(&Token::Indent) {
                    self.advance();
                    let b = self.parse_block()?;
                    self.expect(Token::Dedent)?;
                    b
                } else {
                    vec![self.parse_stmt()?]
                };
                Ok(Stmt::Test { name, body })
            }
            Token::Assert => {
                self.advance();
                let expr = self.parse_expr()?;
                // assert expr == expected → assert_eq!(expr, expected)
                if let Expr::Binary { left, op: BinOp::Eq, right } = expr {
                    Ok(Stmt::Assert { expr: *left, expected: Some(*right) })
                } else if let Expr::Binary { left, op: BinOp::Ne, right } = expr {
                    Ok(Stmt::Assert { expr: *left, expected: Some(Expr::Unary { op: UnaryOp::Not, operand: right }) })
                } else {
                    Ok(Stmt::Assert { expr, expected: None })
                }
            }
            Token::Check => {
                self.advance();
                let expr = self.parse_expr()?;
                let message = if self.check(&Token::Comma) {
                    self.advance();
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                Ok(Stmt::Check { expr, message })
            }
            Token::Def => {
                let func = self.parse_function(false)?;
                Ok(Stmt::FnDef { func })
            }
            Token::Async => {
                self.advance(); // consume async
                let mut func = self.parse_function(false)?;
                func.is_async = true;
                Ok(Stmt::FnDef { func })
            }
            Token::Suite => {
                self.advance();
                let name = self.parse_test_name()?;
                self.expect(Token::Colon)?;
                self.skip_newlines();
                self.expect(Token::Indent)?;
                let mut setup: Option<Vec<Stmt>> = None;
                let mut teardown: Option<Vec<Stmt>> = None;
                let mut tests = Vec::new();
                while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                    self.skip_newlines();
                    if self.check(&Token::Dedent) || self.check(&Token::Eof) { break; }
                    // 检查 setup / teardown 关键字
                    match self.peek() {
                        Token::Setup => {
                            self.advance();
                            self.expect(Token::Colon)?;
                            self.skip_newlines();
                            let body = if self.check(&Token::Indent) {
                                self.advance();
                                let b = self.parse_block()?;
                                self.expect(Token::Dedent)?;
                                b
                            } else {
                                vec![self.parse_stmt()?]
                            };
                            setup = Some(body);
                        }
                        Token::Teardown => {
                            self.advance();
                            self.expect(Token::Colon)?;
                            self.skip_newlines();
                            let body = if self.check(&Token::Indent) {
                                self.advance();
                                let b = self.parse_block()?;
                                self.expect(Token::Dedent)?;
                                b
                            } else {
                                vec![self.parse_stmt()?]
                            };
                            teardown = Some(body);
                        }
                        _ => {
                            tests.push(self.parse_stmt()?);
                        }
                    }
                }
                self.expect(Token::Dedent)?;
                Ok(Stmt::Suite { name, setup, teardown, tests })
            }
            _ => {
                // 局部类型别名: type Name = Type
                if let Token::Ident(ref name) = self.peek() {
                    if name == "type" {
                        self.advance(); // 消费 type
                        let alias_name = match self.advance() {
                            Token::Ident(n) => n,
                            t => return Err(format!("Expected type alias name, got {:?}", t)),
                        };
                        self.expect(Token::Eq)?;
                        let alias_ty = self.parse_type()?;
                        return Ok(Stmt::TypeAlias { name: alias_name, ty: alias_ty });
                    }
                }
                // pass 占位符
                if let Token::Ident(ref name) = self.peek() {
                    if name == "pass" {
                        self.advance();
                        return Ok(Stmt::Pass);
                    }
                }
                // 类型化本地绑定: name: T = value
                if let Token::Ident(_) = self.peek() {
                    if self.peek_n(1) == &Token::Colon {
                        let name = self.advance().to_string();
                        self.advance(); // 消费 :
                        let ty = self.parse_type()?;
                        self.expect(Token::Eq)?;
                        let value = self.parse_expr()?;
                        return Ok(Stmt::Let { name, mutable: true, is_ref: false, ty: Some(ty), value });
                    }
                }
                // 尝试解析为表达式语句或赋值
                let expr = self.parse_expr()?;
                let _lhs_name = match &expr {
                    Expr::Ident(n) => n.clone(),
                    _ => "_".to_string(),
                };

                // 构建块（变量）: <lhs> =: <缩进块>
                if self.check(&Token::BuildAssign) {
                    self.advance();
                    let body = self.parse_build_block_body()?;
                    self.validate_build_block(BuildKind::Var, &body)?;
                    return Ok(Stmt::Expr(Expr::BuildBlock {
                        kind: BuildKind::Var,
                        lhs: Box::new(expr),
                        body,
                    }));
                }

                // 构建块（索引）: <container> ^: <缩进块>
                if self.check(&Token::BuildIndex) {
                    self.advance();
                    let body = self.parse_build_block_body()?;
                    self.validate_build_block(BuildKind::Index, &body)?;
                    return Ok(Stmt::Expr(Expr::BuildBlock {
                        kind: BuildKind::Index,
                        lhs: Box::new(expr),
                        body,
                    }));
                }

                // 检查双 token 位运算复合赋值: &=, |=, ^=, <<=, >>=
                {
                    let peeked = self.peek();
                    let peek2 = self.peek_n(1);
                    let compound = match (peeked, peek2) {
                        (Token::Amp, Token::Eq) => { self.advance(); self.advance(); Some(AssignOp::Eq) }
                        (Token::Pipe_, Token::Eq) => { self.advance(); self.advance(); Some(AssignOp::Eq) }
                        (Token::CaretOp, Token::Eq) | (Token::CaretInfix, Token::Eq) => { self.advance(); self.advance(); Some(AssignOp::Eq) }
                        (Token::Shl, Token::Eq) => { self.advance(); self.advance(); Some(AssignOp::Eq) }
                        (Token::Shr, Token::Eq) => { self.advance(); self.advance(); Some(AssignOp::Eq) }
                        (Token::StarStar, Token::Eq) => { self.advance(); self.advance(); Some(AssignOp::Eq) }
                        _ => None,
                    };
                    if let Some(op) = compound {
                        let value = self.parse_expr()?;
                        return Ok(Stmt::Assign { target: expr, op, value });
                    }
                }

                // 检查是否是赋值
                match self.peek() {
                    Token::Eq => {
                        self.advance();
                        let value = self.parse_maybe_build_value()?;
                        // ident = value → Let 绑定（非赋值），默认可变
                        match expr {
                            Expr::Ident(name) => {
                                Ok(Stmt::Let { name, mutable: true, is_ref: false, ty: None, value })
                            }
                            _ => Ok(Stmt::Assign { target: expr, op: AssignOp::Eq, value })
                        }
                    }
                    Token::PlusEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::AddEq, value })
                    }
                    Token::MinusEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::SubEq, value })
                    }
                    Token::StarEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::MulEq, value })
                    }
                    Token::SlashEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::DivEq, value })
                    }
                    Token::PercentEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::ModEq, value })
                    }
                    Token::AndEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::AndEq, value })
                    }
                    Token::OrEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::OrEq, value })
                    }
                    Token::XorEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::XorEq, value })
                    }
                    Token::ShlEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::ShlEq, value })
                    }
                    Token::ShrEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::ShrEq, value })
                    }
                    Token::PowEq => {
                        self.advance();
                        let value = self.parse_expr()?;
                        Ok(Stmt::Assign { target: expr, op: AssignOp::PowEq, value })
                    }
                    _ => Ok(Stmt::Expr(expr))
                }
            }
        }
    }

    fn parse_binding_stmt(&mut self) -> Result<Stmt, String> {
        // Lang-Zong 默认绑定可变更（copy-by-default 模型）：mut 为 no-op 兼容
        let mut mutable = true;
        let mut is_ref = false;
        let mut is_const = false;
        let _is_owned = false;

        loop {
            match self.peek() {
                Token::Mut => { self.advance(); mutable = true; }
                Token::Ref => { self.advance(); is_ref = true; }
                Token::Const => { self.advance(); is_const = true; }
                Token::Owned => { self.advance(); /* _is_owned = true; */ mutable = true; }
                _ => break,
            }
        }

        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected variable name, got {:?}", t)),
        };

        let ty = if self.check(&Token::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(Token::Eq)?;
        // 构建块（直接）: mut name = *: / ~: <缩进块>
        let value = self.parse_maybe_build_value()?;

        if is_const {
            Ok(Stmt::Const { name, ty, value })
        } else {
            Ok(Stmt::Let { name, mutable, is_ref, ty, value })
        }
    }

    /// let 前缀的绑定：不可变（let ref = 不可变引用）
    fn parse_binding_stmt_let(&mut self) -> Result<Stmt, String> {
        let mut is_ref = false;
        let mut is_const = false;

        loop {
            match self.peek() {
                Token::Mut => { self.advance(); /* let mut 冗余，默认不可变；允许但忽略 */ }
                Token::Ref => { self.advance(); is_ref = true; }
                Token::Const => { self.advance(); is_const = true; }
                Token::Owned => { self.advance(); /* let owned 语义上排斥，忽略 */ }
                _ => break,
            }
        }

        // 支持解构绑定: let (a, b) = expr
        if self.check(&Token::LParen) {
            // 解析 tuple 解构模式，收集所有名字
            self.advance(); // consume (
            let mut names = Vec::new();
            loop {
                match self.advance() {
                    Token::Ident(n) => names.push(n),
                    Token::Underscore => names.push("_".to_string()),
                    Token::RParen => break,
                    Token::Comma | Token::DotDot | Token::DotDotDot => {
                        // skip separators and rest patterns
                        if self.check(&Token::RParen) { self.advance(); break; }
                        continue;
                    }
                    t => {
                        // skip type annotations in destructure patterns
                        if self.check(&Token::Colon) {
                            self.advance(); // :
                            self.parse_type()?;
                            if self.check(&Token::RParen) { self.advance(); break; }
                            continue;
                        }
                        return Err(format!("Expected variable name in destructuring, got {:?}", t));
                    }
                }
            }

            if names.is_empty() {
                return Err("Destructuring pattern must contain at least one variable".to_string());
            }

            let ty = if self.check(&Token::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };

            self.expect(Token::Eq)?;
            let value = self.parse_maybe_build_value()?;

            if names.len() == 1 {
                // Single name destructuring → regular Let
                let name = names.into_iter().next().unwrap();
                if is_const {
                    return Ok(Stmt::Const { name, ty, value });
                } else {
                    return Ok(Stmt::Let { name, mutable: false, is_ref, ty, value });
                }
            }

            return Ok(Stmt::LetTuple { names, ty, value });
        }

        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected variable name, got {:?}", t)),
        };

        let ty = if self.check(&Token::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(Token::Eq)?;
        // 构建块（直接）: let name = *: / ~: <缩进块>
        let value = self.parse_maybe_build_value()?;

        if is_const {
            Ok(Stmt::Const { name, ty, value })
        } else {
            // let 前缀 → 不可变绑定
            Ok(Stmt::Let { name, mutable: false, is_ref, ty, value })
        }
    }

    // ─── 构建块值解析 ───

    /// 解析可能后跟构建块的值表达式
    fn parse_maybe_build_value(&mut self) -> Result<Expr, String> {
        // 情况1: 直接构建块（*:/~:/^: body）- 无前置值
        if self.check(&Token::BuildCall) || self.check(&Token::BuildGen) || self.check(&Token::BuildIndex) {
            let kind = if self.check(&Token::BuildCall) {
                BuildKind::Call
            } else if self.check(&Token::BuildGen) {
                BuildKind::Gen
            } else {
                BuildKind::Index
            };
            self.advance();
            let body = self.parse_build_block_body()?;
            self.validate_build_block(kind, &body)?;
            return Ok(Expr::BuildBlock {
                kind,
                lhs: Box::new(Expr::TupleLit(Vec::new())),
                body,
            });
        }
        // 情况2: 普通值，可能是值后跟构建块
        let value = self.parse_expr()?;
        if self.check(&Token::BuildCall) || self.check(&Token::BuildGen) || self.check(&Token::BuildIndex) {
            let kind = if self.check(&Token::BuildCall) {
                BuildKind::Call
            } else if self.check(&Token::BuildGen) {
                BuildKind::Gen
            } else {
                BuildKind::Index
            };
            self.advance();
            let body = self.parse_build_block_body()?;
            self.validate_build_block(kind, &body)?;
            return Ok(Expr::BuildBlock {
                kind,
                lhs: Box::new(value),
                body,
            });
        }
        // 情况3: 纯值，无构建块
        Ok(value)
    }

    // ─── 构建块 ───

    /// 解析构建块体：符号后必须换行并缩进，跟一个缩进块
    fn parse_build_block_body(&mut self) -> Result<Vec<Stmt>, String> {
        self.skip_newlines();
        self.expect(Token::Indent)?;
        let body = self.parse_block()?;
        self.expect(Token::Dedent)?;
        Ok(body)
    }

    /// 语义检查：构建块返回值 / yield 载荷必须是元组、字典、结构体构造或实现 BuildParams trait 的表达式
    fn validate_build_block(&self, kind: BuildKind, body: &[Stmt]) -> Result<(), String> {
        match kind {
            BuildKind::Var => {
                // 变量构建块：内部可执行任意逻辑；yield 仅允许在生成器构建块中
                if let Some(bad) = self.first_yield(body) {
                    return Err(format!(
                        "变量构建块(=:) 内部不允许使用 yield；yield 仅允许在生成器构建块(*:) 中（位置 {:?}）",
                        bad
                    ));
                }
                Ok(())
            }
            BuildKind::Call => {
                if let Some(bad) = self.first_yield(body) {
                    return Err(format!(
                        "调用构建块(~:) 内部不允许使用 yield；yield 仅允许在生成器构建块(*:) 中（位置 {:?}）",
                        bad
                    ));
                }
                // 调用构建块必须产生返回值（return EXPR 或尾部表达式），且须满足 BuildParams 约束
                match self.build_block_payload(body) {
                    Some(e) if self.is_valid_build_params(e) => Ok(()),
                    Some(_) => Err(
                        "调用构建块(~:) 的返回值必须是元组 / 字典 / 结构体构造，或实现 BuildParams trait 的表达式"
                            .to_string(),
                    ),
                    None => Err(
                        "调用构建块(~:) 必须产生返回值（尾部表达式或 return <params>）"
                            .to_string(),
                    ),
                }
            }
            BuildKind::Gen => {
                // 生成器构建块：每个 yield 载荷须满足 BuildParams 约束
                let mut yields: Vec<&Expr> = Vec::new();
                let mut has_yield = false;
                self.collect_yields(body, &mut yields, &mut has_yield);
                if !has_yield {
                    return Err(
                        "生成器构建块(*:) 必须至少包含一个 yield（用于逐步产出构建参数包）".to_string(),
                    );
                }
                for y in &yields {
                    if !self.is_valid_build_params(y) {
                        return Err(
                            "生成器构建块(*:) 的 yield 必须是元组 / 字典 / 结构体构造，或实现 BuildParams trait 的表达式"
                                .to_string(),
                        );
                    }
                }
                Ok(())
            }
            BuildKind::Index => Ok(()),
        }
    }

    /// 收集所有 yield 载荷表达式（含嵌套控制流与 if/match 表达式分支）。
    /// `has_yield` 记录是否存在任何 yield（含空参数包的裸 yield）。
    fn collect_yields<'a>(&self, stmts: &'a [Stmt], out: &mut Vec<&'a Expr>, has_yield: &mut bool) {
        for s in stmts {
            match s {
                Stmt::Yield(Some(e)) => {
                    out.push(e);
                    *has_yield = true;
                }
                Stmt::Yield(None) => {
                    // 空参数包（裸 yield），合法产出
                    *has_yield = true;
                }
                Stmt::While { body, .. } => self.collect_yields(body, out, has_yield),
                Stmt::For { body, .. } => self.collect_yields(body, out, has_yield),
                Stmt::Loop(body) => self.collect_yields(body, out, has_yield),
                Stmt::Guard { else_body, .. } => self.collect_yields(else_body, out, has_yield),
                Stmt::With { body, .. } => self.collect_yields(body, out, has_yield),
                Stmt::Expr(Expr::If { then_body, elif_clauses, else_body, .. }) => {
                    self.collect_yields(then_body, out, has_yield);
                    for (_, b) in elif_clauses {
                        self.collect_yields(b, out, has_yield);
                    }
                    if let Some(b) = else_body {
                        self.collect_yields(b, out, has_yield);
                    }
                }
                Stmt::Expr(Expr::Match { arms, .. }) => {
                    for arm in arms {
                        self.collect_yields(&arm.body, out, has_yield);
                    }
                }
                _ => {}
            }
        }
    }

    /// 返回第一个出现的 yield 表达式（用于 Var/Call 块禁止 yield 的检查）
    fn first_yield<'a>(&self, stmts: &'a [Stmt]) -> Option<&'a Stmt> {
        for s in stmts {
            match s {
                Stmt::Yield(_) => return Some(s),
                Stmt::While { body, .. } => {
                    if let Some(f) = self.first_yield(body) {
                        return Some(f);
                    }
                }
                Stmt::For { body, .. } => {
                    if let Some(f) = self.first_yield(body) {
                        return Some(f);
                    }
                }
                Stmt::Loop(body) => {
                    if let Some(f) = self.first_yield(body) {
                        return Some(f);
                    }
                }
                Stmt::Guard { else_body, .. } => {
                    if let Some(f) = self.first_yield(else_body) {
                        return Some(f);
                    }
                }
                Stmt::With { body, .. } => {
                    if let Some(f) = self.first_yield(body) {
                        return Some(f);
                    }
                }
                Stmt::Expr(Expr::If { then_body, elif_clauses, else_body, .. }) => {
                    if let Some(f) = self.first_yield(then_body) {
                        return Some(f);
                    }
                    for (_, b) in elif_clauses {
                        if let Some(f) = self.first_yield(b) {
                            return Some(f);
                        }
                    }
                    if let Some(b) = else_body {
                        if let Some(f) = self.first_yield(b) {
                            return Some(f);
                        }
                    }
                }
                Stmt::Expr(Expr::Match { arms, .. }) => {
                    for arm in arms {
                        if let Some(f) = self.first_yield(&arm.body) {
                            return Some(f);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// 提取构建块的"产出参数"表达式：优先 return EXPR，否则取尾部表达式
    fn build_block_payload<'a>(&self, body: &'a [Stmt]) -> Option<&'a Expr> {
        for s in body {
            if let Stmt::Return(Some(e)) = s {
                return Some(e);
            }
        }
        if let Some(Stmt::Expr(e)) = body.last() {
            Some(e)
        } else {
            None
        }
    }

    /// BuildParams 约束：载荷须为元组 / 字典 / 结构体构造（Call）/ 方法调用 / 标识符 / 显式 move(try)
    fn is_valid_build_params(&self, e: &Expr) -> bool {
        match e {
            Expr::TupleLit(_) | Expr::DictLit(_) => true,
            Expr::Call { .. } | Expr::MethodCall { .. } | Expr::Ident(_) => true,
            Expr::KwArg { .. } => true,
            Expr::Move(inner) | Expr::Try(inner) => self.is_valid_build_params(inner),
            _ => false,
        }
    }
}
