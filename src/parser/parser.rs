// Lang-Zong 编译器 — parser/parser.rs
// Parser 核心 + 顶层解析方法

use crate::lexer::Token;
use crate::types::Type;
use crate::ast::*;
use super::stmt::ParserStmtExt;
use super::expr::ParserExprExt;

// ──────────────── Parser ────────────────

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    pub(super) pending_gt: usize, // 处理嵌套泛型 >> 分裂为两个 >
    /// 最近一次 parse_generic_params 解析到的内联约束 (type_param → bounds)
    pending_inline_bounds: Vec<(String, Vec<Type>)>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, pending_gt: 0, pending_inline_bounds: Vec::new() }
    }

    pub(super) fn peek(&self) -> &Token {
        if self.pending_gt > 0 {
            return &Token::Gt;
        }
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    pub(super) fn peek_n(&self, n: usize) -> &Token {
        if n == 0 {
            return self.peek();
        }
        // 如果 pending_gt 存在，偏移一位
        if self.pending_gt > 0 {
            return self.tokens.get(self.pos + n - 1).unwrap_or(&Token::Eof);
        }
        self.tokens.get(self.pos + n).unwrap_or(&Token::Eof)
    }

    pub(super) fn advance(&mut self) -> Token {
        if self.pending_gt > 0 {
            self.pending_gt -= 1;
            return Token::Gt;
        }
        let t = self.peek().clone();
        self.pos += 1;
        t
    }

    pub(super) fn skip_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline) {
            self.advance();
        }
    }

    pub(super) fn check(&self, expected: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(expected)
    }

    pub(super) fn expect(&mut self, expected: Token) -> Result<Token, String> {
        let t = self.advance();
        if std::mem::discriminant(&t) == std::mem::discriminant(&expected) {
            Ok(t)
        } else {
            Err(format!("Expected {:?}, got {:?} at pos {}", expected, t, self.pos))
        }
    }

    // ─── 顶层 ───

    pub fn parse_module(&mut self) -> Result<Module, String> {
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut traits = Vec::new();
        let mut impls = Vec::new();
        let mut consts = Vec::new();
        let mut type_aliases = Vec::new();
        let mut tests = Vec::new();
        let mut top_level_builds = Vec::new();
        let mut module_name = None;
        let mut magic_blocks: Vec<MagicDef> = Vec::new();

        // 词法错误（构建块符号留白违规等）在解析阶段拒绝
        for t in &self.tokens {
            if let Token::LexError(m) = t {
                return Err(m.clone());
            }
        }

        self.skip_newlines();

        while self.peek() != &Token::Eof {
            let mut decorators = Vec::new();
            // 解析装饰器
            while self.check(&Token::At) {
                decorators.push(self.parse_decorator()?);
                self.skip_newlines();
            }

            match self.peek() {
                Token::Def => {
                    let mut f = self.parse_function(false)?;
                    f.decorators = decorators;
                    functions.push(f);
                }
                Token::Iterator => {
                    let mut f = self.parse_function(false)?;
                    f.is_iterator = true;
                    f.decorators = decorators;
                    functions.push(f);
                }
                Token::Async => {
                    self.advance();
                    let mut f = self.parse_function(false)?;
                    f.is_async = true;
                    f.decorators = decorators;
                    functions.push(f);
                }
                Token::Struct => {
                    let mut s = self.parse_struct_like(false)?;
                    s.decorators = decorators;
                    structs.push(s);
                }
                Token::Enum => {
                    let mut s = self.parse_struct_like(true)?;
                    s.decorators = decorators;
                    structs.push(s);
                }
                Token::Trait => {
                    let t = self.parse_trait()?;
                    traits.push(t);
                }
                Token::Impl => {
                    let i = self.parse_impl()?;
                    impls.push(i);
                }
                Token::Import => {
                    imports.push(self.parse_import()?);
                }
                Token::From => {
                    imports.push(self.parse_from_import()?);
                }
                Token::Const => {
                    let c = self.parse_const()?;
                    consts.push(c);
                }
                Token::Test | Token::Suite | Token::Assert => {
                    let stmt = self.parse_stmt()?;
                    tests.push(stmt);
                }
                Token::Let => {
                    // 顶层 let x = 1 → 不可变全局常量
                    self.advance();
                    let stmt = self.parse_binding_stmt_let()?;
                    if let Stmt::Let { name, ty, value, mutable, .. } = stmt {
                        consts.push(ConstDef { name, ty, value, mutable });
                    }
                }
                Token::Mut | Token::Ref | Token::Owned => {
                    // 顶层变量绑定: mut y: int = 0, ref r = &x
                    let stmt = self.parse_binding_stmt()?;
                    if let Stmt::Let { name, ty, value, mutable, .. } = stmt {
                        consts.push(ConstDef { name, ty, value, mutable });
                    }
                }
                Token::Newline => { self.advance(); }
                Token::MagicMethod(ref magic_name) => {
                    // 模块级魔法属性: __name__ = "value", __all__ = [...]
                    let name = magic_name.clone();
                    self.advance(); // 消费 MagicMethod
                    if self.check(&Token::Eq) {
                        self.advance(); // =
                        let value = self.parse_expr()?;
                        // 存储已知魔法属性到 Module
                        match name.as_str() {
                            "__name__" => {
                                if let Expr::StrLit(s) = &value {
                                    module_name = Some(s.clone());
                                }
                            }
                            _ => {} // __all__, __bridge__ 等暂存 consts
                        }
                        consts.push(ConstDef {
                            name, ty: None, value, mutable: false,
                        });
                    } else if self.check(&Token::Colon) {
                        // magic __str__: 块 — 跳过整个块
                        self.advance(); // :
                        self.skip_newlines();
                        if self.check(&Token::Indent) {
                            self.advance();
                            let mut depth = 1;
                            while depth > 0 && !self.check(&Token::Eof) {
                                match self.advance() {
                                    Token::Indent => depth += 1,
                                    Token::Dedent => depth -= 1,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Token::Comptime => {
                    // comptime: 编译期块 — 解析内容
                    self.advance();
                    if self.check(&Token::Colon) {
                        self.advance();
                        self.skip_newlines();
                        if self.check(&Token::Indent) {
                            self.advance();
                            let block = self.parse_block()?;
                            self.expect(Token::Dedent)?;
                            // 将 comptime 块体内容存入 consts（标记 comptime 语义）
                            for stmt in &block {
                                if let Stmt::Let { name, ty, value, mutable, .. } = stmt {
                                    consts.push(ConstDef {
                                        name: name.clone(),
                                        ty: ty.clone(),
                                        value: value.clone(),
                                        mutable: *mutable,
                                    });
                                }
                            }
                        } else if !self.check(&Token::Newline) && !self.check(&Token::Eof) {
                            // 单行: comptime x = expr
                            let stmt = self.parse_stmt()?;
                            if let Stmt::Let { name, ty, value, mutable, .. } = &stmt {
                                consts.push(ConstDef {
                                    name: name.clone(),
                                    ty: ty.clone(),
                                    value: value.clone(),
                                    mutable: *mutable,
                                });
                            }
                        }
                    }
                }
                Token::Duck => {
                    // duck 声明 — 跳过解析直到遇到下一个顶层声明或 EOF
                    self.advance(); // duck keyword
                    // 跳过名称行
                    while !self.check(&Token::Newline) && !self.check(&Token::Eof) { self.advance(); }
                    // 跳过 Body 缩进块（跟踪 indent/dedent 深度）
                    let mut depth = 0;
                    loop {
                        if self.check(&Token::Eof) { break; }
                        if self.check(&Token::Indent) { depth += 1; self.advance(); continue; }
                        if self.check(&Token::Dedent) {
                            depth -= 1;
                            self.advance();
                            if depth <= 0 { break; }
                            continue;
                        }
                        if self.check(&Token::Newline) && depth == 0 {
                            // 空行在顶层，继续跳过
                            self.advance();
                            continue;
                        }
                        self.advance();
                    }
                }
                _ => {
                    // 尝试解析为顶层赋值（全局变量）
                    if let Token::Ident(_) = self.peek() {
                        // magic __xxx__ 块: magic __str__: def __str__(self: T) -> ...
                        if self.peek().to_string() == "magic" && matches!(self.peek_n(1), Token::MagicMethod(_)) {
                            self.advance(); // magic
                            let magic_name = match self.advance() {
                                Token::MagicMethod(n) => n,
                                t => return Err(format!("Expected magic method name, got {:?}", t)),
                            };
                            self.expect(Token::Colon)?;
                            self.skip_newlines();
                            if self.check(&Token::Indent) {
                                self.advance();
                                while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                                    if self.check(&Token::Def) {
                                        let f = self.parse_function(false)?;
                                        magic_blocks.push(MagicDef { method_name: magic_name.clone(), function: f });
                                    } else {
                                        self.advance();
                                    }
                                }
                                if self.check(&Token::Dedent) { self.advance(); }
                            }
                        } else if self.peek().to_string() == "type" {
                            // type alias: type UserId = int
                            self.advance(); // type
                            let name = match self.advance() {
                                Token::Ident(n) => n,
                                t => return Err(format!("Expected type alias name, got {:?}", t)),
                            };
                            // 可选泛型参数: type Name<T> = ...
                            let generics = if self.check(&Token::Lt) {
                                self.advance();
                                let mut gens = Vec::new();
                                loop {
                                    match self.advance() {
                                        Token::Ident(g) => gens.push(g),
                                        Token::Gt | Token::Shr => break,
                                        Token::Comma => {}
                                        Token::Eof => return Err("Unterminated generic params".into()),
                                        _ => {}
                                    }
                                }
                                gens
                            } else {
                                vec![]
                            };
                            self.expect(Token::Eq)?;
                            let ty = self.parse_type()?;
                            type_aliases.push(TypeAliasDef { name, generics, ty });
                        } else if self.peek_n(1) == &Token::Colon {
                            let name = self.advance().to_string();
                            self.advance(); // 消费 :
                            let ty = self.parse_type()?;
                            self.expect(Token::Eq)?;
                            let value = self.parse_expr()?;
                            consts.push(ConstDef { name, ty: Some(ty), value, mutable: false });
                        } else {
                            let stmt = self.parse_stmt()?;
                            // 将 Let / Assign 转为 const（简化处理）
                            match stmt {
                                Stmt::Let { name, ty, value, mutable, .. } => {
                                    consts.push(ConstDef { name, ty, value, mutable });
                                }
                                Stmt::Assign { target, op, value } => {
                                    // 赋值语句如 y += 10 在顶层暂时跳过
                                    // 实际应放在 main 函数中
                                    let name = match target {
                                        Expr::Ident(n) => n,
                                        _ => "_".to_string(),
                                    };
                                    if op == AssignOp::Eq {
                                        consts.push(ConstDef { name, ty: None, value, mutable: false });
                                    }
                                    // 复合赋值在顶层忽略（应放在函数中）
                                }
                                Stmt::Expr(Expr::BuildBlock { kind: _, lhs, body }) => {
                                    // 顶层构建块：Var 类型的 =: 构建块，保存用于 codegen
                                    let name = match &*lhs {
                                        Expr::Ident(n) => n.clone(),
                                        _ => format!("__build_{}", top_level_builds.len()),
                                    };
                                    top_level_builds.push((name, body));
                                }
                                _ => {}
                            }
                        }
                    } else {
                        return Err(format!("Unexpected token at top level: {:?}", self.peek()));
                    }
                }
            }
            self.skip_newlines();
        }

        Ok(Module { name: module_name, imports, functions, structs, traits, impls, consts, type_aliases, tests, top_level_builds, magic_blocks })
    }

    fn parse_decorator(&mut self) -> Result<Decorator, String> {
        self.expect(Token::At)?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected decorator name, got {:?}", t)),
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
        Ok(Decorator { name, args })
    }

    fn parse_import(&mut self) -> Result<ImportStmt, String> {
        self.expect(Token::Import)?;
        let mut path = Vec::new();
        loop {
            match self.advance() {
                Token::Ident(n) => path.push(n),
                t => return Err(format!("Expected module name, got {:?}", t)),
            }
            // 路径分隔符接受 ::（Rust 风格）或 .（LZ 风格）
            if self.check(&Token::PathSep) || self.check(&Token::Dot) {
                // :: 后跟 { 表示花括号形式，跳出循环单独处理
                let next_is_brace = self.peek_n(1) == &Token::LBrace;
                if next_is_brace {
                    break;
                }
                self.advance();
            } else {
                break;
            }
        }
        // 花括号形式: import X::{a, b} 或 import X.{a, b}
        let items = if (self.check(&Token::PathSep) || self.check(&Token::Dot)) && self.peek_n(1) == &Token::LBrace {
            self.advance(); // :: 或 .
            self.advance(); // {
            let mut items = Vec::new();
            loop {
                match self.advance() {
                    Token::Ident(n) => items.push(n),
                    t => return Err(format!("Expected import item, got {:?}", t)),
                }
                if self.check(&Token::Comma) { self.advance(); } else { break; }
            }
            self.expect(Token::RBrace)?;
            items
        } else {
            Vec::new()
        };
        let alias = if self.check(&Token::As) {
            self.advance();
            match self.advance() {
                Token::Ident(n) => Some(n),
                t => return Err(format!("Expected alias, got {:?}", t)),
            }
        } else {
            None
        };
        Ok(ImportStmt { path, alias, items, is_from: false })
    }

    fn parse_from_import(&mut self) -> Result<ImportStmt, String> {
        self.expect(Token::From)?;
        let mut path = Vec::new();
        // 支持相对导入: from .utils, from ..common
        while self.check(&Token::Dot) || self.check(&Token::DotDot) {
            let seg = match self.advance() {
                Token::Dot => "self".to_string(),
                Token::DotDot => "super".to_string(),
                t => t.to_string(),
            };
            path.push(seg);
        }
        loop {
            match self.peek() {
                Token::Ident(_) => path.push(self.advance().to_string()),
                Token::PathSep | Token::Dot => { self.advance(); }
                _ => break,
            }
            // 在 ident 之后检查是否是 :: 或 .（统一为路径分隔符）
            if !self.check(&Token::PathSep) && !self.check(&Token::Dot) && !matches!(self.peek(), Token::Ident(_)) {
                break;
            }
        }
        self.expect(Token::Import)?;
        let mut items = Vec::new();
        loop {
            match self.advance() {
                Token::Ident(n) => items.push(n),
                Token::Star => items.push("*".to_string()),
                t => return Err(format!("Expected import item, got {:?}", t)),
            }
            if self.check(&Token::Comma) { self.advance(); } else { break; }
        }
        Ok(ImportStmt { path, alias: None, items, is_from: true })
    }

    fn parse_const(&mut self) -> Result<ConstDef, String> {
        self.expect(Token::Const)?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected const name, got {:?}", t)),
        };
        let ty = if self.check(&Token::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(Token::Eq)?;
        let value = self.parse_expr()?;
        Ok(ConstDef { name, ty, value, mutable: false })
    }

    // ─── 函数 ───

    pub(super) fn parse_function(&mut self, no_body: bool) -> Result<Function, String> {
        // 接受 def 或 iterator 关键字
        match self.advance() {
            Token::Def => {}
            Token::Iterator => {}
            t => return Err(format!("Expected def or iterator, got {:?}", t)),
        }
        let mut name = match self.advance() {
            Token::Ident(n) => n,
            Token::MagicMethod(n) => n,
            t => return Err(format!("Expected function name, got {:?}", t)),
        };
        // 点号分隔函数名: def config.get(...)
        while self.check(&Token::Dot) {
            self.advance();
            name.push('.');
            match self.advance() {
                Token::Ident(seg) => name.push_str(&seg),
                t => return Err(format!("Expected identifier after . in function name, got {:?}", t)),
            }
        }

        // checker 注解: def test_val[check_positive](x: int) -> int = x
        if self.check(&Token::LBrack) {
            self.advance(); // [
            // 跳过 checker 内容
            let mut depth = 1;
            while depth > 0 && !self.check(&Token::Eof) {
                if self.check(&Token::LBrack) { depth += 1; }
                if self.check(&Token::RBrack) { depth -= 1; }
                self.advance();
            }
        }

        // 泛型参数
        let generics = if self.check(&Token::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        // 参数
        self.expect(Token::LParen)?;
        let (params, variadic) = self.parse_params()?;
        self.expect(Token::RParen)?;

        // 返回类型: 使用 `-> type`（与参数类型注解 `:` 区分）
        let return_type = if self.check(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        // raises 异常类型标注（允许在返回类型后换行）
        self.skip_newlines();

        // 处理下一行缩进：where 子句可能在缩进块内
        let consumed_indent_for_body = if self.check(&Token::Indent) {
            self.advance();
            true
        } else {
            false
        };

        let raises = if self.check(&Token::Raises) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        // where 子句（允许换行）
        let mut where_clause = if self.check(&Token::Where) {
            self.parse_where_clause()?
        } else {
            Vec::new()
        };
        // 合并内联约束 (T: Ordered → where_clause)
        for (tp, bds) in std::mem::take(&mut self.pending_inline_bounds) {
            where_clause.push(WhereBound { type_param: tp, bounds: bds });
        }

        // 函数体
        let (body, is_abstract) = if no_body {
            (Vec::new(), false)
        } else {
            self.expect(Token::Eq)?;
            self.skip_newlines();
            if self.check(&Token::DotDot) || self.check(&Token::DotDotDot) {
                if self.check(&Token::DotDotDot) {
                    self.advance(); // DotDotDot (3 dots)
                } else {
                    self.advance(); // DotDot (2 dots)
                    if self.check(&Token::Dot) {
                        self.advance(); // 3rd dot
                    }
                }
                (Vec::new(), true)
            } else if self.check(&Token::Indent) {
                self.advance();
                let b = self.parse_block()?;
                self.expect(Token::Dedent)?;
                (b, false)
            } else if consumed_indent_for_body {
                // where 子句已经消费了外层 Indent，body 在同一个缩进块内
                let b = self.parse_block()?;
                (b, false)
            } else {
                let s = self.parse_stmt()?;
                (vec![s], false)
            }
        };

        // 消费与 consumed_indent_for_body 匹配的外层 Dedent
        if consumed_indent_for_body {
            self.expect(Token::Dedent)?;
        }

        Ok(Function {
            name, generics, params, return_type, raises,
            where_clause, body, is_async: false, is_abstract, is_iterator: false,
            decorators: Vec::new(), variadic,
        })
    }

    fn parse_generic_params(&mut self) -> Result<Vec<String>, String> {
        self.expect(Token::Lt)?;
        let mut params = Vec::new();
        // 内联约束: T: Ordered → 合并进 where_clause
        let mut inline_bounds: Vec<(String, Vec<Type>)> = Vec::new();
        self.pending_inline_bounds.clear();
        loop {
            let name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected generic param, got {:?}", t)),
            };
            // 内联 trait 约束: T: Ordered + Clone
            let mut bounds: Vec<Type> = Vec::new();
            if self.check(&Token::Colon) {
                self.advance();
                loop {
                    let b = self.parse_type()?;
                    bounds.push(b);
                    if self.check(&Token::Plus) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            if !bounds.is_empty() {
                inline_bounds.push((name.clone(), bounds));
            }
            params.push(name);
            // 默认类型: T = int
            if self.check(&Token::Eq) {
                self.advance(); // =
                // 跳过默认类型表达式，直到 , 或 >
                let mut depth = 0;
                loop {
                    if self.check(&Token::Comma) || self.check(&Token::Gt) || self.check(&Token::Shr) {
                        break;
                    }
                    if self.check(&Token::Lt) { depth += 1; }
                    if self.check(&Token::Gt) || self.check(&Token::Shr) {
                        if depth == 0 { break; }
                        depth -=1;
                    }
                    self.advance();
                }
            }
            if self.check(&Token::Comma) { self.advance(); continue; }
            // 闭合 >：Gt 或 Shr（嵌套泛型 >>）
            if self.check(&Token::Gt) {
                self.advance();
                break;
            }
            if self.check(&Token::Shr) {
                self.advance();
                self.pending_gt += 1;
                break;
            }
            break;
        }
        if !inline_bounds.is_empty() {
            self.pending_inline_bounds = inline_bounds;
        }
        Ok(params)
    }

    /// 解析函数参数列表，返回 (params, variadic_mode)
    /// `..` 最多出现 2 次，分隔 仅位置/仅关键字/args+kwargs 参数
    fn parse_params(&mut self) -> Result<(Vec<Param>, VariadicMode), String> {
        let mut params = Vec::new();
        let mut dotdot_positions: Vec<usize> = Vec::new();

        while !self.check(&Token::RParen) {
            // 检查 `..` 分隔符
            if self.check(&Token::DotDot) {
                if dotdot_positions.len() >= 2 {
                    return Err("`..` 分隔符最多出现 2 次".to_string());
                }
                self.advance(); // 消费 DotDot
                dotdot_positions.push(params.len());

                // `..` 后可选逗号继续
                if self.check(&Token::Comma) { self.advance(); }
                continue;
            }

            // 参数默认不可变；mut 修饰按需添加
            let mut is_mut = false;
            let mut is_owned = false;
            let mut is_ref = false;

            // 参数修饰符（名前修饰：mut / ref / owned / owend）
            loop {
                match self.peek() {
                    Token::Mut => { self.advance(); is_mut = true; }
                    Token::Ref => { self.advance(); is_ref = true; }
                    Token::Owned => { self.advance(); is_owned = true; }
                    _ => break,
                }
            }

            let name = match self.advance() {
                Token::Ident(n) => n,
                Token::Self_ => "self".to_string(),
                t => return Err(format!("Expected param name, got {:?}", t)),
            };

            // 参数可以无类型注解（默认为泛型推断）
            let ty = if self.check(&Token::Comma) || self.check(&Token::RParen) || self.check(&Token::DotDot) {
                if name == "self" { Type::Self_ } else { Type::Any }
            } else {
                self.expect(Token::Colon)?;

                // 类型前缀修饰符（名后修饰：name: owed str）
                if self.check(&Token::Owned) {
                    self.advance();
                    is_owned = true;
                }
                // ref 在类型前缀位置 → 同样支持
                if self.check(&Token::Ref) {
                    self.advance();
                    is_ref = true;
                }

                self.parse_type()?
            };

            // 默认值
            let default = if self.check(&Token::Eq) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };

            params.push(Param { name, ty, default, is_mut, is_owned, is_ref });
            if self.check(&Token::Comma) { self.advance(); }
        }

        let variadic = match dotdot_positions.len() {
            0 => VariadicMode::None,
            1 => VariadicMode::Single { dotdot_at: dotdot_positions[0] },
            2 => VariadicMode::Double {
                first_at: dotdot_positions[0],
                second_at: dotdot_positions[1],
            },
            _ => unreachable!(),
        };

        Ok((params, variadic))
    }

    fn parse_where_clause(&mut self) -> Result<Vec<WhereBound>, String> {
        self.expect(Token::Where)?;
        let mut bounds = Vec::new();
        loop {
            let type_param = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected type param in where, got {:?}", t)),
            };
            self.expect(Token::Colon)?;

            let mut trait_bounds = Vec::new();
            loop {
                let b = self.parse_type()?;
                trait_bounds.push(b);
                if self.check(&Token::Plus) {
                    self.advance();
                } else {
                    break;
                }
            }
            bounds.push(WhereBound { type_param, bounds: trait_bounds });

            // 多个约束用逗号分隔（或换行后继续）
            if self.check(&Token::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }
        Ok(bounds)
    }

    // ─── 测试名称解析 ───

    /// 解析 test/suite 的名称：支持引号形式 `"name"` 或自由文本形式 `name words`
    pub(super) fn parse_test_name(&mut self) -> Result<String, String> {
        if matches!(self.peek(), Token::StrLit(_)) {
            // 引号形式: test "add works":
            if let Token::StrLit(name) = self.advance() {
                return Ok(name);
            }
            return Err("Expected string literal".into());
        }
        // 自由文本形式: test add works:
        // 收集 test 之后、本行最后一个 : 之前的所有 token 作为名称
        let mut parts = Vec::new();
        loop {
            if self.check(&Token::Colon) {
                break;
            }
            // 任意非冒号 token 拼接为名称（包括关键字、数字等）
            parts.push(self.advance().to_string());
        }
        if parts.is_empty() {
            return Err("Expected test name".into());
        }
        Ok(parts.join(" "))
    }

    // ─── 类型解析 ───

    pub(super) fn parse_type(&mut self) -> Result<Type, String> {
        let base = match self.advance() {
            Token::Duck => {
                // duck { name: Type, name: Type, ... }
                self.expect(Token::LBrace)?;
                let mut fields = Vec::new();
                while !self.check(&Token::RBrace) {
                    let name = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected field name in duck type, got {:?}", t)),
                    };
                    self.expect(Token::Colon)?;
                    let field_ty = self.parse_type()?;
                    fields.push((name, field_ty));
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RBrace)?;
                return Ok(Type::Duck { fields });
            }
            Token::Ident(n) => {
                let base_ty = match n.as_str() {
                    "int" => Type::Int,
                    "f64" => Type::F64,
                    "float" => Type::Float,
                    "str" => Type::Str,
                    "bool" => Type::Bool,
                    "None" => Type::None_,
                    "Never" => Type::Never,
                    "Any" => Type::Any,
                    "fn" => {
                        // 函数类型: fn(int, str) -> bool
                        self.expect(Token::LParen)?;
                        let mut params = Vec::new();
                        while !self.check(&Token::RParen) {
                            params.push(self.parse_type()?);
                            if self.check(&Token::Comma) { self.advance(); }
                        }
                        self.expect(Token::RParen)?;
                        let ret = if self.check(&Token::Arrow) {
                            self.advance();
                            self.parse_type()?
                        } else {
                            Type::Unit
                        };
                        return Ok(Type::Fn { params, ret: Box::new(ret) });
                    }
                    "Simd" => {
                        // Simd[T, N] — SIMD 向量类型
                        self.expect(Token::Lt)?;
                        let elem = self.parse_type()?;
                        self.expect(Token::Comma)?;
                        let width = match self.advance() {
                            Token::IntLit(n) => {
                                if n < 1 || n > 64 {
                                    return Err(format!("Simd width must be between 1 and 64, got {}", n));
                                }
                                n as usize
                            }
                            t => return Err(format!("Expected integer width for Simd, got {:?}", t)),
                        };
                        self.expect(Token::Gt)?;
                        return Ok(Type::Simd { elem: Box::new(elem), width });
                    }
                    _ => Type::Named(n),
                };
                // 泛型参数 List<int>, Dict<K,V>, Option<T>, Result<T,E>
                if self.check(&Token::Lt) {
                    self.advance(); // consume <
                    let mut inner = Vec::new();
                    loop {
                        inner.push(self.parse_type()?);
                        if self.check(&Token::Comma) { self.advance(); }
                        // 闭合 >：可能是 Gt 或 Shr（嵌套泛型 >>）
                        if self.check(&Token::Gt) {
                            self.advance();
                            break;
                        }
                        if self.check(&Token::Shr) {
                            // >> 分裂：消耗一个 >，留一个 pending
                            self.advance();
                            self.pending_gt += 1;
                            break;
                        }
                        if self.check(&Token::Ge) {
                            // >= 情况（不应出现在类型中，但防御性处理）
                            self.advance();
                            break;
                        }
                    }
                    // 识别特殊容器类型
                    return match &base_ty {
                        Type::Named(name) if name == "Option" && inner.len() == 1 => {
                            Ok(Type::Option(Box::new(inner.into_iter().next().unwrap())))
                        }
                        Type::Named(name) if name == "Result" && inner.len() == 2 => {
                            let mut iter = inner.into_iter();
                            Ok(Type::Result {
                                ok: Box::new(iter.next().unwrap()),
                                err: Box::new(iter.next().unwrap()),
                            })
                        }
                        _ => Ok(Type::Generic {
                            base: Box::new(base_ty),
                            args: inner,
                        }),
                    };
                }
                base_ty
            }
            Token::Ref => {
                let inner = self.parse_type()?;
                Type::Ref(Box::new(inner))
            }
            Token::Self_ => Type::Self_,
            Token::LParen => {
                // 函数类型 (T) -> U 或元组类型 (T, U)
                let mut inner = Vec::new();
                while !self.check(&Token::RParen) {
                    inner.push(self.parse_type()?);
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RParen)?;
                if self.check(&Token::Arrow) {
                    self.advance();
                    let ret = self.parse_type()?;
                    Type::Fn {
                        params: inner,
                        ret: Box::new(ret),
                    }
                } else {
                    Type::Tuple(inner)
                }
            }
            Token::Amp => {
                let inner = self.parse_type()?;
                Type::Ref(Box::new(inner))
            }
            t => return Err(format!("Expected type, got {:?}", t)),
        };

        // int? 语法糖：int? → Option<int>, str? → Option<str>
        if self.check(&Token::Question) {
            self.advance();
            return Ok(Type::Optional(Box::new(base)));
        }

        Ok(base)
    }

    // ─── struct / enum ───

    fn parse_struct_like(&mut self, is_enum: bool) -> Result<StructDef, String> {
        self.advance(); // skip struct/enum
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected name, got {:?}", t)),
        };

        let generics = if self.check(&Token::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        // 支持 = : =: 三种分隔符
        // enum Color: Red, Green, Blue  或  enum Color = Red, Green, Blue
        // struct Box =: body ...  (构建块)
        if !self.check(&Token::Eq) && !self.check(&Token::Colon) && !self.check(&Token::BuildAssign) {
            let t = self.advance();
            return Err(format!("Expected Eq, Colon or BuildAssign, got {:?} at pos {}", t, self.pos));
        }
        self.advance(); // consume = : or =:
        self.skip_newlines();

        // 单行 struct: struct Box<T> = value: T
        // 或 struct Point = x: f64, y: f64
        if !self.check(&Token::Indent) {
            let mut fields = Vec::new();
            let methods = Vec::new();
            // 解析单行字段（支持逗号分隔多个字段）
            if !self.check(&Token::Newline) && !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                loop {
                    let f_name = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("struct 字段需换行缩进书写，或单行用逗号分隔。当前: {:?}", t)),
                    };
                    self.expect(Token::Colon)?;
                    let f_type = self.parse_type()?;
                    fields.push(Field { name: f_name, ty: f_type, default: None });
                    if self.check(&Token::Comma) {
                        self.advance();
                        // 跳过逗号后的空格/换行（检查是否还有下一个字段）
                        self.skip_newlines();
                        if self.check(&Token::Newline) || self.check(&Token::Dedent) || self.check(&Token::Eof) {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            return Ok(StructDef { name, generics, fields, methods, magic_methods: Vec::new(), is_enum, decorators: Vec::new(), repr_attr: None });
        }

        self.expect(Token::Indent)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut magic_methods = Vec::new();
        let mut repr_attr: Option<String> = None;

        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            self.skip_newlines();
            if self.check(&Token::Dedent) || self.check(&Token::Eof) { break; }

            // __slots__ = "C" | "packed" | "align(N)" | "transparent"  → #[repr(...)]
            if matches!(self.peek(), Token::MagicMethod(_)) {
                let id = self.peek().to_string();
                if id == "__slots__" {
                    self.advance(); // consume __slots__
                    self.expect(Token::Eq)?;
                    let val = match self.advance() {
                        Token::StrLit(s) => s,
                        Token::Ident(s) => s, // 允许未加引号: __slots__ = C
                        t => return Err(format!("Expected string literal for __slots__, got {:?}", t)),
                    };
                    // 验证 repr 值
                    let valid = val == "C" || val == "packed" || val == "transparent"
                        || val.starts_with("align(");
                    if !valid {
                        return Err(format!("Invalid __slots__ value '{}': expected 'C', 'packed', 'align(N)', or 'transparent'", val));
                    }
                    repr_attr = Some(val);
                    self.skip_newlines();
                    continue;
                }
            }

            // magic __new__(...) / magic __xxx__(...) → 解析为 Function
            if let Token::Ident(ref name) = self.peek() {
                if name == "magic" && matches!(self.peek_n(1), Token::MagicMethod(_)) {
                    self.advance(); // magic
                    let magic_name = match self.advance() {
                        Token::MagicMethod(n) => n,
                        t => return Err(format!("Expected magic method name, got {:?}", t)),
                    };
                    // 解析参数列表
                    let (params, variadic) = if self.check(&Token::LParen) {
                        self.advance();
                        let result = self.parse_params()?;
                        self.expect(Token::RParen)?;
                        result
                    } else {
                        (Vec::new(), VariadicMode::None)
                    };
                    // 解析返回类型
                    let return_type = if self.check(&Token::Arrow) {
                        self.advance();
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    // 解析函数体: : indent body dedent 或 = expr
                    let body = if self.check(&Token::Colon) {
                        self.advance();
                        self.skip_newlines();
                        self.expect(Token::Indent)?;
                        let body = self.parse_block()?;
                        self.expect(Token::Dedent)?;
                        body
                    } else if self.check(&Token::Eq) {
                        self.advance();
                        self.skip_newlines();
                        if self.check(&Token::Indent) {
                            self.advance();
                            let body = self.parse_block()?;
                            self.expect(Token::Dedent)?;
                            body
                        } else {
                            let expr = self.parse_expr()?;
                            vec![Stmt::Return(Some(expr))]
                        }
                    } else {
                        Vec::new()
                    };
                    magic_methods.push(Function {
                        name: magic_name.clone(),
                        generics: Vec::new(),
                        params,
                        return_type,
                        raises: None,
                        where_clause: Vec::new(),
                        body,
                        is_async: false,
                        is_abstract: false,
                        is_iterator: false,
                        decorators: Vec::new(),
                        variadic,
                    });
                    self.skip_newlines();
                    continue;
                }
            }

            match self.peek() {
                Token::Def => {
                    methods.push(self.parse_function(false)?);
                }
                Token::Iterator => {
                    let mut f = self.parse_function(false)?;
                    f.is_iterator = true;
                    methods.push(f);
                }
                Token::Async => {
                    self.advance();
                    let mut f = self.parse_function(false)?;
                    f.is_async = true;
                    methods.push(f);
                }
                Token::Ident(_) | Token::MagicMethod(_) | Token::True | Token::False => {
                    let f_name = self.advance().to_string();
                    // 处理 enum 变体: Some(T), Ok(T), Err(E), Circle(x: f64, y: f64)
                    let f_type = if self.check(&Token::LParen) {
                        self.advance();
                        // 判断是否为关键字参数风格（命名字段）: Circle(x: f64, y: f64)
                        let has_named_fields = matches!(self.peek(), Token::Ident(_))
                            && self.peek_n(1) == &Token::Colon;
                        if has_named_fields {
                            let mut field_types = Vec::new();
                            while !self.check(&Token::RParen) {
                                match self.advance() {
                                    Token::Ident(_) => {}
                                    t => return Err(format!("Expected field name, got {:?}", t)),
                                }
                                self.expect(Token::Colon)?;
                                field_types.push(self.parse_type()?);
                                if self.check(&Token::Comma) { self.advance(); }
                            }
                            self.expect(Token::RParen)?;
                            if field_types.len() == 1 {
                                field_types.into_iter().next().unwrap()
                            } else {
                                Type::Tuple(field_types)
                            }
                        } else {
                            let mut types = Vec::new();
                            while !self.check(&Token::RParen) {
                                types.push(self.parse_type()?);
                                if self.check(&Token::Comma) { self.advance(); }
                            }
                            self.expect(Token::RParen)?;
                            if types.len() == 1 {
                                types.into_iter().next().unwrap()
                            } else {
                                Type::Tuple(types)  // 多字段变体: Color(f64, f64, f64)
                            }
                        }
                    } else if self.check(&Token::Colon) {
                        self.advance();
                        self.parse_type()?
                    } else {
                        Type::Unit  // unit variant, no type
                    };
                    let default = if self.check(&Token::Eq) {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    fields.push(Field { name: f_name, ty: f_type, default });
                }
                _ => { break; }
            }
            self.skip_newlines();
        }

        self.expect(Token::Dedent)?;
        Ok(StructDef { name, generics, fields, methods, magic_methods, is_enum, decorators: Vec::new(), repr_attr })
    }

    // ─── trait ───

    fn parse_trait(&mut self) -> Result<TraitDef, String> {
        self.advance(); // skip trait
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected trait name, got {:?}", t)),
        };
        let generics = if self.check(&Token::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };
        self.expect(Token::Eq)?;
        self.skip_newlines();
        self.expect(Token::Indent)?;

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            self.skip_newlines();
            if self.check(&Token::Dedent) || self.check(&Token::Eof) { break; }

            match self.peek() {
                Token::Def | Token::Iterator => {
                    // trait 方法可能有默认实现
                    let f = self.parse_function(false)?;
                    methods.push(f);
                }
                Token::Ident(_) => {
                    let f_name = self.advance().to_string();
                    if self.check(&Token::Colon) {
                        self.advance();
                        let f_type = self.parse_type()?;
                        fields.push(Field { name: f_name, ty: f_type, default: None });
                    }
                }
                _ => { break; }
            }
            self.skip_newlines();
        }
        self.expect(Token::Dedent)?;
        Ok(TraitDef { name, generics, methods, fields })
    }

    // ─── impl ───

    fn parse_impl(&mut self) -> Result<ImplDef, String> {
        self.advance(); // skip impl

        let generics = if self.check(&Token::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        let first_name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected type/trait name, got {:?}", t)),
        };

        let (trait_name, type_name) = if self.check(&Token::For) {
            self.advance();
            let tn = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected type name, got {:?}", t)),
            };
            (Some(first_name), tn)
        } else {
            (None, first_name)
        };

        self.expect(Token::Eq)?;
        self.skip_newlines();
        self.expect(Token::Indent)?;

        let mut methods = Vec::new();
        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            self.skip_newlines();
            if self.check(&Token::Dedent) || self.check(&Token::Eof) { break; }
            methods.push(self.parse_function(false)?);
            self.skip_newlines();
        }
        self.expect(Token::Dedent)?;

        let mut where_clause = if self.check(&Token::Where) {
            self.parse_where_clause()?
        } else {
            Vec::new()
        };
        // 合并内联约束
        for (tp, bds) in std::mem::take(&mut self.pending_inline_bounds) {
            where_clause.push(WhereBound { type_param: tp, bounds: bds });
        }

        Ok(ImplDef { trait_name, type_name, generics, where_clause, methods })
    }
}
