// Lang-Zong 编译器 — parser/parser.rs
// Parser 核心 + 顶层解析方法

use super::expr::ParserExprExt;
use super::stmt::ParserStmtExt;
use crate::ast::*;
use crate::lexer::Token;
use crate::types::Type;

// ──────────────── Parser ────────────────

pub struct Parser {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    pub(super) pending_gt: usize, // 处理嵌套泛型 >> 分裂为两个 >
    /// 最近一次 parse_generic_params 解析到的内联约束 (type_param → bounds)
    pending_inline_bounds: Vec<(String, Vec<Type>)>,
    /// 最近一次 parse_generic_params 解析到的默认类型 (type_param → default type)
    pending_generic_defaults: Vec<(String, Type)>,
    /// 是否为宏模块（首行 `#!bin macro`）——由调用方在宏展开前用原始
    /// token 流检测并设置（展开会消费 #!bin macro 声明，Parser 自身无法检测）
    pub is_macro: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            pending_gt: 0,
            pending_inline_bounds: Vec::new(),
            pending_generic_defaults: Vec::new(),
            is_macro: false,
        }
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
            Err(format!(
                "Expected {:?}, got {:?} at pos {}",
                expected, t, self.pos
            ))
        }
    }

    // ─── 顶层 ───

    pub fn parse_module(&mut self) -> Result<Module, String> {
        // 宏模块检测：首行 `#!bin macro` 声明（lexer 对整行产生单个 Token::Macro，
        // 后跟 Newline/Eof）。宏/template 仅能定义在宏模块，__is_macro__ 据此填充
        let mut is_macro_module = false;
        let mut ti = 0;
        while ti < self.tokens.len() && matches!(&self.tokens[ti], Token::Newline | Token::Indent) {
            ti += 1;
        }
        if ti < self.tokens.len() && self.tokens[ti] == Token::Macro {
            let next = self.tokens.get(ti + 1);
            if next.map_or(true, |t| matches!(t, Token::Newline | Token::Eof)) {
                is_macro_module = true;
            }
        }

        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut traits = Vec::new();
        let mut impls = Vec::new();
        let mut consts = Vec::new();
        let mut type_aliases = Vec::new();
        let mut tests = Vec::new();
        let mut top_level_builds = Vec::new();
        let mut top_stmts: Vec<Stmt> = Vec::new();
        let mut duck_defs = Vec::new();
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
            // 跳过意外的 Dedent（嵌套块结束标记）
            if self.check(&Token::Dedent) {
                self.advance();
                self.skip_newlines();
                continue;
            }
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
                    if let Stmt::Let {
                        name,
                        ty,
                        value,
                        mutable,
                        ..
                    } = stmt
                    {
                        consts.push(ConstDef {
                            name,
                            ty,
                            value,
                            mutable,
                        });
                    }
                }
                Token::Mut | Token::Ref | Token::Owned => {
                    // 顶层变量绑定: mut y: int = 0, ref r = &x
                    let stmt = self.parse_binding_stmt()?;
                    if let Stmt::Let {
                        name,
                        ty,
                        value,
                        mutable,
                        ..
                    } = stmt
                    {
                        consts.push(ConstDef {
                            name,
                            ty,
                            value,
                            mutable,
                        });
                    }
                }
                Token::Newline => {
                    self.advance();
                }
                Token::Block => {
                    // 模块级 block 定义 / 触发调用
                    // 作为顶层构建块语句处理
                    let stmt = self.parse_stmt()?;
                    top_stmts.push(stmt);
                }
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
                            name,
                            ty: None,
                            value,
                            mutable: false,
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
                    // 顶层循环用 match peek() 未消费 token：先 advance 消费 comptime
                    self.advance();
                    // comptime def f(...) — 编译期函数（仅编译期存在，不生成运行时代码）
                    if self.check(&Token::Def) {
                        let mut f = self.parse_function(false)?;
                        f.is_comptime = true;
                        f.decorators = decorators;
                        functions.push(f);
                        continue; // 继续处理后续 top-level 语句（不能 break 跳出顶层循环）
                    }
                    if self.check(&Token::Colon) {
                        self.advance();
                        self.skip_newlines();
                        if self.check(&Token::Indent) {
                            self.advance();
                            let block = self.parse_block()?;
                            self.expect(Token::Dedent)?;
                            // 将 comptime 块体内容存入 consts（标记 comptime 语义）
                            for stmt in &block {
                                if let Stmt::Let {
                                    name,
                                    ty,
                                    value,
                                    mutable,
                                    ..
                                } = stmt
                                {
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
                            if let Stmt::Let {
                                name,
                                ty,
                                value,
                                mutable,
                                ..
                            } = &stmt
                            {
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
                    // 解析 duck 声明: duck Name = def method(...) -> Ret ...
                    let d = self.parse_duck_def()?;
                    duck_defs.push(d);
                }
                Token::Macro | Token::Template => {
                    // 宏定义兜底解析：正常流程下宏/template 定义已由 main.rs 的
                    // token 层展开管线（MacroExpander/TemplateExpander 交替展开，
                    // 08 §3.6）在解析前移除并展开，IR 后端直接消费展开后的 token
                    // 流；此处仅作兜底（宏定义未被前置过滤时），解析为普通函数定义
                    // （参数/返回按 Tokens 类型保存，body 为 quote(...) 表达式）
                    self.advance(); // consume macro/template
                    let name = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected macro name, got {:?}", t)),
                    };
                    // 参数
                    self.expect(Token::LParen)?;
                    let (params, _variadic) = self.parse_params()?;
                    self.expect(Token::RParen)?;
                    // 可选返回类型: -> Tokens
                    let return_type = if self.check(&Token::Arrow) {
                        self.advance();
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    self.expect(Token::Eq)?;
                    self.skip_newlines();
                    // 跳过可能的外层 Indent（缩进块体）
                    if self.check(&Token::Indent) {
                        self.advance();
                    }
                    let body = self.parse_expr()?;
                    if self.check(&Token::Dedent) {
                        self.advance();
                    }
                    functions.push(Function {
                        name,
                        generics: vec![],
                        generic_defaults: vec![],
                        params,
                        return_type,
                        raises: None,
                        where_clause: vec![],
                        body: vec![Stmt::Return(Some(body))],
                        is_async: false,
                        is_abstract: false,
                        is_iterator: false,
                        is_magic: false,
                        is_comptime: false,
                        decorators: vec![],
                        variadic: crate::ast::VariadicMode::None,
                        checker_param: None,
                        default_checker: None,
                    });
                }
                _ => {
                    // 尝试解析为顶层赋值（全局变量）
                    if let Token::Ident(_) = self.peek() {
                        // magic __xxx__ 块: magic __str__: def __str__(self: T) -> ...
                        if self.peek().to_string() == "magic"
                            && matches!(self.peek_n(1), Token::MagicMethod(_))
                        {
                            self.advance(); // magic
                            let magic_name = match self.advance() {
                                Token::MagicMethod(n) => n,
                                t => {
                                    return Err(format!("Expected magic method name, got {:?}", t))
                                }
                            };
                            self.expect(Token::Colon)?;
                            self.skip_newlines();
                            if self.check(&Token::Indent) {
                                self.advance();
                                while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
                                    if self.check(&Token::Def) {
                                        let f = self.parse_function(false)?;
                                        magic_blocks.push(MagicDef {
                                            method_name: magic_name.clone(),
                                            function: f,
                                        });
                                    } else {
                                        self.advance();
                                    }
                                }
                                if self.check(&Token::Dedent) {
                                    self.advance();
                                }
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
                                        Token::Eof => {
                                            return Err("Unterminated generic params".into())
                                        }
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
                            consts.push(ConstDef {
                                name,
                                ty: Some(ty),
                                value,
                                mutable: false,
                            });
                        } else {
                            let stmt = self.parse_stmt()?;
                            // 将 Let / Assign 转为 const（简化处理）
                            match stmt {
                                Stmt::Let {
                                    name,
                                    ty,
                                    value,
                                    mutable,
                                    ..
                                } => {
                                    consts.push(ConstDef {
                                        name,
                                        ty,
                                        value,
                                        mutable,
                                    });
                                }
                                Stmt::Assign { target, op, value } => {
                                    // 赋值语句如 y += 10 在顶层暂时跳过
                                    // 实际应放在 main 函数中
                                    let name = match target {
                                        Expr::Ident(n) => n,
                                        _ => "_".to_string(),
                                    };
                                    if op == AssignOp::Eq {
                                        consts.push(ConstDef {
                                            name,
                                            ty: None,
                                            value,
                                            mutable: false,
                                        });
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

        Ok(Module {
            name: module_name,
            // 源文件路径由 main.rs 在解析后注入（parser 不感知文件路径）
            file_path: None,
            imports,
            functions,
            structs,
            traits,
            impls,
            consts,
            type_aliases,
            tests,
            top_level_builds,
            top_stmts,
            duck_defs,
            magic_blocks,
            // main.rs 用原始 token 流（展开前）检测设置；自检作兜底
            is_macro: self.is_macro || is_macro_module,
        })
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
                if self.check(&Token::Comma) {
                    self.advance();
                }
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
        // import macro X（宏命名空间导入，IR 后端同普通 import 处理）
        if self.check(&Token::Macro) {
            self.advance();
        }
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
        let items = if (self.check(&Token::PathSep) || self.check(&Token::Dot))
            && self.peek_n(1) == &Token::LBrace
        {
            self.advance(); // :: 或 .
            self.advance(); // {
            let mut items = Vec::new();
            loop {
                match self.advance() {
                    Token::Ident(n) => items.push(n),
                    t => return Err(format!("Expected import item, got {:?}", t)),
                }
                if self.check(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
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
        Ok(ImportStmt {
            path,
            alias,
            items,
            is_from: false,
        })
    }

    fn parse_from_import(&mut self) -> Result<ImportStmt, String> {
        self.expect(Token::From)?;
        // from macro X import Y（宏命名空间导入，IR 后端同普通 import 处理）
        if self.check(&Token::Macro) {
            self.advance();
        }
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
                Token::PathSep | Token::Dot => {
                    self.advance();
                }
                _ => break,
            }
            // 在 ident 之后检查是否是 :: 或 .（统一为路径分隔符）
            if !self.check(&Token::PathSep)
                && !self.check(&Token::Dot)
                && !matches!(self.peek(), Token::Ident(_))
            {
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
            if self.check(&Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(ImportStmt {
            path,
            alias: None,
            items,
            is_from: true,
        })
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
        Ok(ConstDef {
            name,
            ty,
            value,
            mutable: false,
        })
    }

    // ─── 函数 ───

    pub(super) fn parse_function(&mut self, no_body: bool) -> Result<Function, String> {
        // 跳过装饰器 (@unsafe 等)
        let mut decorators = Vec::new();
        while self.check(&Token::At) {
            self.advance(); // consume @
            let name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected decorator name after @, got {:?}", t)),
            };
            decorators.push(Decorator {
                name,
                args: Vec::new(),
            });
            self.skip_newlines();
        }
        // 接受 def / iterator / magic 关键字
        let is_magic = match self.advance() {
            Token::Def => false,
            Token::Iterator => false,
            Token::Ident(ref s) if s == "magic" => true,
            t => return Err(format!("Expected def, iterator or magic, got {:?}", t)),
        };
        let mut name = match self.advance() {
            Token::Ident(n) => n,
            Token::MagicMethod(n) => n,
            Token::And => "and".to_string(),
            Token::Or => "or".to_string(),
            Token::Not => "not".to_string(),
            Token::In => "in".to_string(),
            t => return Err(format!("Expected function name, got {:?}", t)),
        };
        // 点号分隔函数名: def config.get(...)
        while self.check(&Token::Dot) {
            self.advance();
            name.push('.');
            match self.advance() {
                Token::Ident(seg) => name.push_str(&seg),
                t => {
                    return Err(format!(
                        "Expected identifier after . in function name, got {:?}",
                        t
                    ))
                }
            }
        }

        // checker 注解:
        //   [ps: __Params]  → 定义检查站参数（新的 def/block 自带 ps 参数）
        //   [checker_name]  → 引用已有检查站（无类型注解）
        //   [None]          → 显式无检查站
        let (checker_param, default_checker) = if self.check(&Token::LBrack) {
            self.advance(); // [
            let first = match self.advance() {
                Token::Ident(n) => n,
                t => {
                    return Err(format!(
                        "Expected checker name or ps:Type after [, got {:?}",
                        t
                    ))
                }
            };
            if first == "None" {
                self.expect(Token::RBrack)?;
                (None, None)
            } else if self.check(&Token::Colon) {
                // [ps: __Params] → 定义检查站参数
                self.advance(); // :
                self.parse_type()?; // skip __Params type
                self.expect(Token::RBrack)?;
                (Some(first), None)
            } else {
                // [checker_name] → 引用已有检查站
                self.expect(Token::RBrack)?;
                (None, Some(first))
            }
        } else {
            (None, None)
        };

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

        // where 子句（允许换行和缩进）
        // def name<T>() -> R where T: Bound
        //     where T: Bound2 =
        self.skip_newlines();
        if self.check(&Token::Indent) {
            self.advance(); // skip Indent before where
        }
        let mut where_clause = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&Token::Indent) {
                self.advance(); // skip Indent before where
            }
            if self.check(&Token::Where) {
                where_clause.extend(self.parse_where_clause()?);
            } else {
                break;
            }
        }
        // 合并内联约束 (T: Ordered → where_clause)
        for (tp, bds) in std::mem::take(&mut self.pending_inline_bounds) {
            where_clause.push(WhereBound {
                type_param: tp,
                bounds: bds,
            });
        }

        // 函数体
        let (body, is_abstract) = if no_body {
            (Vec::new(), false)
        } else {
            self.skip_newlines(); // 允许 = 在下一行
                                  // 支持 `:` 作为 block body 分隔符（def main(): <Indent>）
            let has_colon_body = self.check(&Token::Colon);
            if has_colon_body {
                self.advance(); // consume :
            } else if self.check(&Token::Dedent) {
                // 直接遇到 Dedent（trait 抽象方法 `def reset(mut self)` 后无 body）：
                // 返回抽象方法且不消费 Dedent（否则 trait 块结束符丢失，
                // 后续 trait/struct 被误当表达式，E0554）
                return Ok(Function {
                    name,
                    generics,
                    generic_defaults: std::mem::take(&mut self.pending_generic_defaults),
                    params,
                    return_type,
                    raises,
                    where_clause,
                    body: Vec::new(),
                    is_async: false,
                    is_abstract: true,
                    is_iterator: false,
                    is_magic,
                    is_comptime: false,
                    decorators: Vec::new(),
                    variadic,
                    checker_param: None,
                    default_checker: None,
                });
            } else if !self.check(&Token::Eq) && !self.check(&Token::Dedent) {
                // 没有 = body → 抽象方法声明（仅签名）
                // 但如果下一个是 Dedent 或 Struct 等顶层 token，也视为无 body
                if matches!(
                    self.peek(),
                    Token::Dedent
                        | Token::Struct
                        | Token::Enum
                        | Token::Trait
                        | Token::Impl
                        | Token::Def
                        | Token::Iterator
                ) {
                    return Ok(Function {
                        name,
                        generics,
                        generic_defaults: std::mem::take(&mut self.pending_generic_defaults),
                        params,
                        return_type,
                        raises,
                        where_clause,
                        body: Vec::new(),
                        is_async: false,
                        is_abstract: true,
                        is_iterator: false,
                        is_magic,
                        is_comptime: false,
                        decorators: Vec::new(),
                        variadic,
                        checker_param: None,
                        default_checker: None,
                    });
                }
                self.expect(Token::Eq)?;
            } else {
                self.advance(); // consume =
            }
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
            name,
            generics,
            generic_defaults: std::mem::take(&mut self.pending_generic_defaults),
            params,
            return_type,
            raises,
            where_clause,
            body,
            is_async: false,
            is_abstract,
            is_iterator: false,
            is_magic,
            is_comptime: false,
            decorators: Vec::new(),
            variadic,
            checker_param,
            default_checker,
        })
    }

    fn parse_generic_params(&mut self) -> Result<Vec<String>, String> {
        self.expect(Token::Lt)?;
        let mut params = Vec::new();
        // 内联约束: T: Ordered → 合并进 where_clause
        // 注意：不能 clear pending_inline_bounds——parse_impl 会调用本函数两次
        // （impl<A: Iterator> 声明泛型 + Zip<A,B> 类型实参），第二次调用若 clear
        // 会丢掉第一次收集的 `A: Iterator` 约束（E0220 associated type not found）
        let mut inline_bounds: Vec<(String, Vec<Type>)> = Vec::new();
        loop {
            let name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected generic param, got {:?}", t)),
            };
            // type-pack 变长泛型参数（03d-可变参数.md §2.8 方案 B）：`Ts...`
            // 解析为普通泛型参数 Ts（变长展开能力待 codegen 实现，先保证解析通过）
            if self.check(&Token::DotDotDot) {
                self.advance(); // consume ...
            } else if self.check(&Token::DotDot) && self.peek_n(1) == &Token::Dot {
                self.advance(); // consume ..
                self.advance(); // consume .
            }
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
            // 默认类型: T = int（§四）→ 解析并存入 pending_generic_defaults
            // 注意：需在 push(name) 之前 clone，避免 name 被移动后借用
            if self.check(&Token::Eq) {
                self.advance(); // =
                let default_ty = self.parse_type()?;
                self.pending_generic_defaults.push((name.clone(), default_ty));
            }
            params.push(name);
            if self.check(&Token::Comma) {
                self.advance();
                continue;
            }
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
            // 用 extend 而非赋值：parse_impl 会调用本函数两次（impl<A: Iterator>
            // 声明泛型 + Zip<A,B> 类型实参），第二次 inline_bounds 为空时
            // 不能覆盖第一次收集的 `A: Iterator` 约束（E0220）
            self.pending_inline_bounds.extend(inline_bounds);
        }
        Ok(params)
    }

    /// 解析函数参数列表，返回 (params, variadic_mode)
    /// `..` 是变参注入标记（最多 2 次）：单 `..` 无注解 → 注入 args（元素 Any）；
    /// `..: Tuple<T>` → args-only；`..: Dict<K,V>` → kwargs-only；双 `..` → args + kwargs。
    /// `/` `*` 是安全分隔符（仅分割、不注入），与 `..` 互斥（见 03d-可变参数.md §三）。
    fn parse_params(&mut self) -> Result<(Vec<Param>, VariadicMode), String> {
        let mut params = Vec::new();
        let mut dotdot_positions: Vec<usize> = Vec::new();
        let mut dotdot_annots: Vec<Option<Type>> = Vec::new();
        let mut has_slash_star = false;

        while !self.check(&Token::RParen) {
            // 检查 `..` 变参注入标记
            if self.check(&Token::DotDot) {
                if dotdot_positions.len() >= 2 {
                    return Err("`..` 最多出现 2 次".to_string());
                }
                self.advance(); // 消费 ..
                dotdot_positions.push(params.len());
                // 可选类型注解: ..: Tuple<T> / ..: Dict<K,V>
                let annot = if self.check(&Token::Colon) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                dotdot_annots.push(annot);
                // `..` 后可选逗号继续
                if self.check(&Token::Comma) {
                    self.advance();
                }
                continue;
            }
            // `/` `*` 安全分隔符（仅分割，不注入）
            if self.check(&Token::Slash) || self.check(&Token::Star) {
                self.advance();
                has_slash_star = true;
                if self.check(&Token::Comma) {
                    self.advance();
                }
                continue;
            }

            // 参数默认不可变；mut 修饰按需添加
            let mut is_mut = false;
            let mut is_owned = false;
            let mut is_ref = false;

            // 参数修饰符（名前修饰：mut / ref / owned / owend）
            loop {
                match self.peek() {
                    Token::Mut => {
                        self.advance();
                        is_mut = true;
                    }
                    Token::Ref => {
                        self.advance();
                        is_ref = true;
                    }
                    Token::Owned => {
                        self.advance();
                        is_owned = true;
                    }
                    _ => break,
                }
            }

            let name = match self.advance() {
                Token::Ident(n) => n,
                Token::Self_ => "self".to_string(),
                Token::From => "from".to_string(),
                t => return Err(format!("Expected param name, got {:?}", t)),
            };

            // 参数可以无类型注解（默认为泛型推断）
            let ty = if self.check(&Token::Comma)
                || self.check(&Token::RParen)
                || self.check(&Token::DotDot)
                || self.check(&Token::Slash)
                || self.check(&Token::Star)
            {
                if name == "self" {
                    Type::Self_
                } else {
                    Type::Any
                }
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

            params.push(Param {
                name,
                ty,
                default,
                is_mut,
                is_owned,
                is_ref,
            });
            if self.check(&Token::Comma) {
                self.advance();
            }
        }

        // `/` `*` 与 `..` 互斥：同一签名不允许混用（文档 §3.3）
        if has_slash_star && !dotdot_positions.is_empty() {
            return Err("`/` `*` 与 `..` 不能混用在同一签名".to_string());
        }

        let variadic = match dotdot_positions.len() {
            0 => VariadicMode::None,
            1 => {
                let at = dotdot_positions[0];
                let annot = dotdot_annots[0].clone();
                match annot {
                    // ..: Dict<K,V> → kwargs-only（值类型 V）
                    Some(Type::Generic { base, args })
                        if matches!(*base, Type::Named(ref b) if b == "Dict") =>
                    {
                        let value_ty = args.last().cloned();
                        VariadicMode::KwargsOnly { dotdot_at: at, value_ty }
                    }
                    // ..: Tuple<T> → args-only（元素类型 T）
                    Some(Type::Tuple(ts)) => VariadicMode::ArgsOnly {
                        dotdot_at: at,
                        elem_ty: ts.first().cloned(),
                        elem_tys: ts.clone(),
                    },
                    Some(Type::Generic { base, args })
                        if matches!(*base, Type::Named(ref b) if b == "Tuple") =>
                    {
                        VariadicMode::ArgsOnly {
                            dotdot_at: at,
                            elem_ty: args.first().cloned(),
                            elem_tys: args.clone(),
                        }
                    }
                    // .. 无注解 → args-only（元素 Any）
                    _ => VariadicMode::ArgsOnly { dotdot_at: at, elem_ty: None, elem_tys: vec![] },
                }
            }
            2 => VariadicMode::Both {
                first_at: dotdot_positions[0],
                args_elem_ty: dotdot_annots[0]
                    .clone()
                    .and_then(|t| match t {
                        Type::Tuple(ts) => ts.first().cloned(),
                        Type::Generic { base, args }
                            if matches!(*base, Type::Named(ref b) if b == "Tuple") =>
                        {
                            args.first().cloned()
                        }
                        _ => None,
                    }),
                second_at: dotdot_positions[1],
                kwargs_value_ty: dotdot_annots[1]
                    .clone()
                    .and_then(|t| match t {
                        Type::Generic { base, args }
                            if matches!(*base, Type::Named(ref b) if b == "Dict") =>
                        {
                            args.last().cloned()
                        }
                        _ => None,
                    }),
            },
            _ => unreachable!(),
        };

        Ok((params, variadic))
    }

    /// 解析单个函数参数（名称、类型注解、默认值、修饰符），
    /// 供 duck 方法签名等需要独立解析单个参数的场景复用。
    fn parse_single_param(&mut self, allow_default: bool) -> Result<Param, String> {
        // 参数默认不可变；mut 修饰按需添加
        let mut is_mut = false;
        let mut is_owned = false;
        let mut is_ref = false;

        // 参数修饰符（名前修饰：mut / ref / owned / owend）
        loop {
            match self.peek() {
                Token::Mut => {
                    self.advance();
                    is_mut = true;
                }
                Token::Ref => {
                    self.advance();
                    is_ref = true;
                }
                Token::Owned => {
                    self.advance();
                    is_owned = true;
                }
                _ => break,
            }
        }

        let name = match self.advance() {
            Token::Ident(n) => n,
            Token::Self_ => "self".to_string(),
            Token::From => "from".to_string(),
            t => return Err(format!("Expected param name, got {:?}", t)),
        };

        // 参数可以无类型注解（默认为泛型推断）
        let ty = if self.check(&Token::Comma)
            || self.check(&Token::RParen)
            || self.check(&Token::DotDot)
        {
            if name == "self" {
                Type::Self_
            } else {
                Type::Any
            }
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
        let default = if allow_default && self.check(&Token::Eq) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Param {
            name,
            ty,
            default,
            is_mut,
            is_owned,
            is_ref,
        })
    }

    fn parse_where_clause(&mut self) -> Result<Vec<WhereBound>, String> {
        self.expect(Token::Where)?;
        let mut bounds = Vec::new();
        loop {
            // where 子句约束参数支持关联类型路径：`I.Item: Add<...>`（06c-trait定义.md §五）
            // 或 trait 内 `Self.Item: ...`（Self 是 Token::Self_ 特殊 token）。
            // 读类型参数后若跟 `.` + Ident（关联类型），拼接为 "I.Item" / "Self.Item" 形式
            let mut type_param = match self.advance() {
                Token::Ident(n) => n,
                Token::Self_ => "Self".to_string(),
                t => return Err(format!("Expected type param in where, got {:?}", t)),
            };
            while self.check(&Token::Dot) {
                self.advance(); // .
                let seg = match self.advance() {
                    Token::Ident(s) => s,
                    t => return Err(format!("Expected assoc type in where, got {:?}", t)),
                };
                type_param = format!("{}.{}", type_param, seg);
            }
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
            bounds.push(WhereBound {
                type_param,
                bounds: trait_bounds,
            });

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
        let mut base = match self.advance() {
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
                    if self.check(&Token::Comma) {
                        self.advance();
                    }
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
                            if self.check(&Token::Comma) {
                                self.advance();
                            }
                        }
                        self.expect(Token::RParen)?;
                        let ret = if self.check(&Token::Arrow) {
                            self.advance();
                            self.parse_type()?
                        } else {
                            Type::Unit
                        };
                        return Ok(Type::Fn {
                            params,
                            ret: Box::new(ret),
                        });
                    }
                    "Simd" => {
                        // Simd[T, N] — SIMD 向量类型
                        self.expect(Token::Lt)?;
                        let elem = self.parse_type()?;
                        self.expect(Token::Comma)?;
                        let width = match self.advance() {
                            Token::IntLit(n) => {
                                if n < 1 || n > 64 {
                                    return Err(format!(
                                        "Simd width must be between 1 and 64, got {}",
                                        n
                                    ));
                                }
                                n as usize
                            }
                            t => {
                                return Err(format!("Expected integer width for Simd, got {:?}", t))
                            }
                        };
                        self.expect(Token::Gt)?;
                        return Ok(Type::Simd {
                            elem: Box::new(elem),
                            width,
                        });
                    }
                    _ => Type::Named(n),
                };
                // 泛型参数 List<int>, Dict<K,V>, Option<T>, Result<T,E>
                // 也支持 Tuple<T1, T2, ..> 中的 `..` 通配（03d §2.3：约束位置参数各自类型）
                if self.check(&Token::Lt) {
                    self.advance(); // consume <
                    let mut inner = Vec::new();
                    loop {
                        if self.check(&Token::DotDot) {
                            // `..` 通配：位置参数数量不限，push 占位 Any 后继续
                            self.advance();
                            inner.push(Type::Any);
                            if self.check(&Token::Comma) {
                                self.advance();
                            }
                            if self.check(&Token::Gt) {
                                self.advance();
                                break;
                            }
                            continue;
                        }
                        inner.push(self.parse_type()?);
                        // 命名泛型参数（关联类型绑定）：`Add<Output = I.Item>`
                        // （06c-trait定义.md §五：trait 泛型用关联类型绑定）——
                        // `Output = Type` 形式：把整个绑定作为 Named("Output = I.Item")
                        // 保留（codegen 渲染为 `Add<Output = I::Item>`），
                        // 而不是仅记录 Output 导致 E0425 cannot find type `Output`
                        if self.check(&Token::Eq) {
                            let name = inner.pop().unwrap_or(Type::Any);
                            self.advance(); // consume =
                            let value = self.parse_type()?; // 关联类型值
                            inner.push(Type::Named(format!(
                                "{} = {}",
                                name.to_string(),
                                value.to_string()
                            )));
                        }
                        // type-pack 元素 `Ts...`（03d §2.8 方案 B）：消费尾部 `...`
                        // （parse_type 解析 Ident(Ts) 后残留 DotDotDot）
                        if self.check(&Token::DotDotDot) {
                            self.advance(); // consume ...
                        } else if self.check(&Token::DotDot) && self.peek_n(1) == &Token::Dot {
                            self.advance(); // consume ..
                            self.advance(); // consume .
                        }
                        if self.check(&Token::Comma) {
                            self.advance();
                        }
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
                    let generic_ty = match &base_ty {
                        Type::Named(name) if name == "Option" && inner.len() == 1 => {
                            Type::Option(Box::new(inner.into_iter().next().unwrap()))
                        }
                        Type::Named(name) if name == "Result" && inner.len() == 2 => {
                            let mut iter = inner.into_iter();
                            Type::Result {
                                ok: Box::new(iter.next().unwrap()),
                                err: Box::new(iter.next().unwrap()),
                            }
                        }
                        _ => Type::Generic {
                            base: Box::new(base_ty),
                            args: inner,
                        },
                    };
                    // 泛型类型后也支持 ? 后缀: List<T>? → Optional<List<T>>
                    if self.check(&Token::Question) {
                        self.advance();
                        return Ok(Type::Optional(Box::new(generic_ty)));
                    }
                    return Ok(generic_ty);
                }
                base_ty
            }
            Token::Ref => {
                let inner = self.parse_type()?;
                Type::Ref(Box::new(inner))
            }
            Token::Mut => {
                let inner = self.parse_type()?;
                Type::MutRef(Box::new(inner))
            }
            Token::Self_ => Type::Self_,
            Token::LParen => {
                // 函数类型 (T) -> U 或元组类型 (T, U)
                let mut inner = Vec::new();
                while !self.check(&Token::RParen) {
                    inner.push(self.parse_type()?);
                    if self.check(&Token::Comma) {
                        self.advance();
                    }
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

        // 支持路径类型: Self.Item 或 std.collections.List<T>
        while self.check(&Token::Dot) {
            self.advance(); // consume .
            let next = match self.advance() {
                Token::Ident(n) => n,
                t => {
                    return Err(format!(
                        "Expected identifier after . in type path, got {:?}",
                        t
                    ))
                }
            };
            // 构建路径类型: Self.Item → Named("Self.Item")
            let path = match &base {
                Type::Named(n) => format!("{}.{}", n, next),
                Type::Self_ => format!("Self.{}", next),
                _ => format!("{}.{}", "<>", next), // 回退
            };
            base = Type::Named(path);
            // 检查泛型参数: Self.Item<T>
            if self.check(&Token::Lt) {
                self.advance();
                let mut inner = Vec::new();
                loop {
                    inner.push(self.parse_type()?);
                    if self.check(&Token::Comma) {
                        self.advance();
                    }
                    if self.check(&Token::Gt) {
                        self.advance();
                        break;
                    }
                }
                base = Type::Generic {
                    base: Box::new(base),
                    args: inner,
                };
            }
        }

        // int? 语法糖：int? → Option<int>, str? → Option<str>
        if self.check(&Token::Question) {
            self.advance();
            return Ok(Type::Optional(Box::new(base)));
        }

        Ok(base)
    }

    // ─── struct / enum ───

    pub fn parse_struct_like(&mut self, is_enum: bool) -> Result<StructDef, String> {
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
        self.skip_newlines(); // 允许分隔符前换行（如文档注释后的空行）
        if !self.check(&Token::Eq) && !self.check(&Token::Colon) && !self.check(&Token::BuildAssign)
        {
            let t = self.advance();
            return Err(format!(
                "Expected Eq, Colon or BuildAssign, got {:?} at pos {}",
                t, self.pos
            ));
        }
        self.advance(); // consume = : or =:
        self.skip_newlines();

        // 单行 struct: struct Box<T> = value: T
        // 或 struct Point = x: f64, y: f64
        if !self.check(&Token::Indent) {
            let mut fields = Vec::new();
            let methods = Vec::new();
            // 解析单行字段（支持逗号分隔多个字段）
            if !self.check(&Token::Newline)
                && !self.check(&Token::Dedent)
                && !self.check(&Token::Eof)
            {
                loop {
                    let f_name = match self.advance() {
                        Token::Ident(n) => n,
                        t => {
                            return Err(format!(
                                "struct 字段需换行缩进书写，或单行用逗号分隔。当前: {:?}",
                                t
                            ))
                        }
                    };
                    self.expect(Token::Colon)?;
                    let f_type = self.parse_type()?;
                    fields.push(Field {
                        name: f_name,
                        ty: f_type,
                        default: None,
                    });
                    if self.check(&Token::Comma) {
                        self.advance();
                        // 跳过逗号后的空格/换行（检查是否还有下一个字段）
                        self.skip_newlines();
                        if self.check(&Token::Newline)
                            || self.check(&Token::Dedent)
                            || self.check(&Token::Eof)
                        {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            return Ok(StructDef {
                name,
                generics,
                generic_bounds: std::mem::take(&mut self.pending_inline_bounds),
                generic_defaults: std::mem::take(&mut self.pending_generic_defaults),
                fields,
                methods,
                magic_methods: Vec::new(),
                is_enum,
                decorators: Vec::new(),
                repr_attr: None,
            });
        }

        self.expect(Token::Indent)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut magic_methods = Vec::new();
        let mut repr_attr: Option<String> = None;

        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            self.skip_newlines();
            if self.check(&Token::Dedent) || self.check(&Token::Eof) {
                break;
            }

            // __slots__ = "C" | "packed" | "align(N)" | "transparent"  → #[repr(...)]
            if matches!(self.peek(), Token::MagicMethod(_)) {
                let id = self.peek().to_string();
                if id == "__slots__" {
                    self.advance(); // consume __slots__
                    self.expect(Token::Eq)?;
                    let val = match self.advance() {
                        Token::StrLit(s) => s,
                        Token::Ident(s) => s, // 允许未加引号: __slots__ = C
                        t => {
                            return Err(format!(
                                "Expected string literal for __slots__, got {:?}",
                                t
                            ))
                        }
                    };
                    // 验证 repr 值
                    let valid = val == "C"
                        || val == "packed"
                        || val == "transparent"
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
                        generic_defaults: Vec::new(),
                        params,
                        return_type,
                        raises: None,
                        where_clause: Vec::new(),
                        body,
                        is_async: false,
                        is_abstract: false,
                        is_iterator: false,
                        is_magic: false,
                        is_comptime: false,
                        decorators: Vec::new(),
                        variadic,
                        checker_param: None,
                        default_checker: None,
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
                                if self.check(&Token::Comma) {
                                    self.advance();
                                }
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
                                if self.check(&Token::Comma) {
                                    self.advance();
                                }
                            }
                            self.expect(Token::RParen)?;
                            if types.len() == 1 {
                                types.into_iter().next().unwrap()
                            } else {
                                Type::Tuple(types) // 多字段变体: Color(f64, f64, f64)
                            }
                        }
                    } else if self.check(&Token::Colon) {
                        self.advance();
                        self.parse_type()?
                    } else {
                        Type::Unit // unit variant, no type
                    };
                    let default = if self.check(&Token::Eq) {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    fields.push(Field {
                        name: f_name,
                        ty: f_type,
                        default,
                    });
                }
                _ => {
                    break;
                }
            }
            self.skip_newlines();
        }

        // 消费所有嵌套块的 Dedent（struct/impl/trait 嵌套时可能有多层）
        while self.check(&Token::Dedent) {
            self.advance();
            self.skip_newlines();
        }
        Ok(StructDef {
            name,
            generics,
            generic_bounds: std::mem::take(&mut self.pending_inline_bounds),
            generic_defaults: std::mem::take(&mut self.pending_generic_defaults),
            fields,
            methods,
            magic_methods,
            is_enum,
            decorators: Vec::new(),
            repr_attr,
        })
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
        // 支持 trait Name: Bound1 + Bound2 =  (trait bounds/supertraits)
        // 或 trait Name =  (无 bounds)
        let mut supertraits = Vec::new();
        if self.check(&Token::Colon) {
            self.advance(); // consume :
                            // 解析 supertrait 类型直到 =（trait DoubleEndedIterator: Iterator）
            loop {
                if self.check(&Token::Eq)
                    || self.check(&Token::Eof)
                    || self.check(&Token::Newline)
                    || self.check(&Token::Indent)
                {
                    break;
                }
                let st = self.parse_type()?;
                supertraits.push(st);
                if self.check(&Token::Plus) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(Token::Eq)?;
        self.skip_newlines();
        self.expect(Token::Indent)?;

        let mut methods = Vec::new();
        let mut fields = Vec::new();
        // trait 内声明的关联类型（§五 `type Item`）收集
        let mut assoc_types = Vec::new();

        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            self.skip_newlines();
            if self.check(&Token::Dedent) || self.check(&Token::Eof) {
                break;
            }

            match self.peek() {
                Token::Def | Token::Iterator => {
                    // trait 方法：既可能是抽象声明（`def measure(self) -> int` 无 body，
                    // parse_function 内部检测 Dedent 等返回 is_abstract），也可能是
                    // 默认实现（`def describe(self) -> str = f"..."` 带 body）。
                    // 用 no_body=false 让两种形态都正确解析。
                    let f = self.parse_function(false)?;
                    methods.push(f);
                }
                Token::Ident(ref kw) if kw == "type" => {
                    // trait 关联类型声明（§五）: `type Item` 或带 bound 的
                    // `type Iter: Iterator<Item = Self.Item>`
                    self.advance(); // type
                    let assoc_name = match self.advance() {
                        Token::Ident(n) => n,
                        t => {
                            return Err(format!(
                                "Expected assoc type name in trait, got {:?}",
                                t
                            ))
                        }
                    };
                    assoc_types.push(assoc_name);
                    // 可选 bound：`type Iter: Iterator<...>` — 消费 bound 类型
                    // （bound 的完整 Rust 输出暂由关联类型使用处承担）
                    if self.check(&Token::Colon) {
                        self.advance(); // :
                        let _ = self.parse_type()?;
                    }
                }
                Token::Ident(_) => {
                    let f_name = self.advance().to_string();
                    if self.check(&Token::Colon) {
                        self.advance();
                        let f_type = self.parse_type()?;
                        fields.push(Field {
                            name: f_name,
                            ty: f_type,
                            default: None,
                        });
                    }
                }
                _ => {
                    break;
                }
            }
            self.skip_newlines();
        }
        while self.check(&Token::Dedent) {
            self.advance();
            self.skip_newlines();
        }
        Ok(TraitDef {
            name,
            generics,
            generic_defaults: std::mem::take(&mut self.pending_generic_defaults),
            supertraits,
            methods,
            fields,
            assoc_types,
        })
    }

    // ─── impl ───

    fn parse_impl(&mut self) -> Result<ImplDef, String> {
        self.advance(); // skip impl

        let mut generics = if self.check(&Token::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };
        // 立即转存头部内联约束（`impl<A: Iterator> ...`）：后续解析 impl 方法体
        // 时 parse_function 会 take 清空 pending_inline_bounds，导致 `A: Iterator`
        // 约束丢失（E0220 associated type not found）
        let head_inline_bounds = std::mem::take(&mut self.pending_inline_bounds);

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

        // 支持 impl 类型上的泛型参数: impl<T> Box<T> =
        // 也支持 `impl Box<T>` 形式（类型名后的泛型参数一并归入 generics）。
        // 注意：`impl<T> Iterator for Once<T>` 中 Once<T> 的 T 是 impl 泛型 T 的
        // 使用，不能重复收集（否则生成 impl<T, T> for Once<T, T>，E0403/E0107）；
        // 仅当类型参数名不在已有 generics 中时才收集（如 `impl Box<T>` 无 impl 泛型）
        if self.check(&Token::Lt) {
            let mut impl_gen = self.parse_generic_params()?;
            for g in impl_gen.drain(..) {
                if !generics.iter().any(|x| x == &g) {
                    generics.push(g);
                }
            }
        }

        // 支持 = 和 : 两种分隔符；也支持 where 子句位于分隔符之前
        // （换行/缩进形式，与 parse_function 一致）：
        //   impl<I: Iterator, J: Iterator, B> Iterator for FlatMap<I, J, B>
        //       where J: Iterator<Item = B>
        //       =
        let mut pre_where = Vec::new();
        let mut consumed_indent_for_where = false;
        loop {
            self.skip_newlines();
            if self.check(&Token::Indent) {
                self.advance(); // skip Indent before where
                consumed_indent_for_where = true;
            }
            if self.check(&Token::Where) {
                pre_where.extend(self.parse_where_clause()?);
            } else {
                break;
            }
        }
        if !self.check(&Token::Eq) && !self.check(&Token::Colon) {
            let t = self.advance();
            return Err(format!(
                "Expected Eq or Colon, got {:?} at pos {}",
                t, self.pos
            ));
        }
        self.advance(); // consume = or :
        self.skip_newlines();
        if !consumed_indent_for_where {
            self.expect(Token::Indent)?;
        }

        let mut methods = Vec::new();
        let mut assoc_type_bindings = Vec::new();
        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            self.skip_newlines();
            if self.check(&Token::Dedent) || self.check(&Token::Eof) {
                break;
            }
            // impl 块中的关联类型绑定: type Item = int（§五）
            if let Token::Ident(ref s) = self.peek() {
                if s == "type" {
                    self.advance(); // consume 'type'
                    let bind_name = match self.advance() {
                        Token::Ident(n) => n,
                        t => {
                            return Err(format!(
                                "Expected assoc type name in impl, got {:?}",
                                t
                            ))
                        }
                    };
                    self.expect(Token::Eq)?;
                    let bind_ty = self.parse_type()?;
                    assoc_type_bindings.push((bind_name, bind_ty));
                    continue;
                }
            }
            methods.push(self.parse_function(false)?);
            self.skip_newlines();
        }
        while self.check(&Token::Dedent) {
            self.advance();
            self.skip_newlines();
        }

        let mut where_clause = if self.check(&Token::Where) {
            self.parse_where_clause()?
        } else {
            Vec::new()
        };
        // 合并分隔符之前的 where 子句（impl ... where ... = 换行形式）
        where_clause.splice(0..0, pre_where);
        // 合并 impl 头部内联约束（`impl<A: Iterator>` 的 A: Iterator，已提前转存
        // 避免被 parse_function 清空）与后续内联约束
        for (tp, bds) in head_inline_bounds {
            where_clause.push(WhereBound {
                type_param: tp,
                bounds: bds,
            });
        }
        for (tp, bds) in std::mem::take(&mut self.pending_inline_bounds) {
            where_clause.push(WhereBound {
                type_param: tp,
                bounds: bds,
            });
        }

        Ok(ImplDef {
            trait_name,
            type_name,
            generics,
            generic_defaults: std::mem::take(&mut self.pending_generic_defaults),
            where_clause,
            methods,
            assoc_type_bindings,
        })
    }

    /// 解析 duck 声明: duck Name<T> = def method(self) -> Ret .field: Type ...
    fn parse_duck_def(&mut self) -> Result<DuckDef, String> {
        self.advance(); // duck keyword
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(format!("Expected duck name, got {:?}", t)),
        };
        // 可选的泛型参数: duck Mapper<T, R> = ...
        let generics = if self.check(&Token::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };
        // 可选的嵌套约束: duck D<T> where T: Iterable = ...（§2.4）
        let where_clause = if self.check(&Token::Where) {
            self.parse_where_clause()?
        } else {
            Vec::new()
        };
        self.expect(Token::Eq)?;
        self.skip_newlines();
        self.expect(Token::Indent)?;

        let mut methods = Vec::new();
        let mut fields = Vec::new();
        let mut assoc_types = Vec::new();
        let mut satisfies = Vec::new();
        let mut sealed = false;
        let mut match_rules = Vec::new();
        let mut param_reqs = Vec::new();

        while !self.check(&Token::Dedent) && !self.check(&Token::Eof) {
            // default 修饰（§11.4③）：`default def fallback(self) -> ()`
            // 必须在软关键字行判断之前消费，否则 default 会走普通标识符分支
            let mut is_default = false;
            if let Token::Ident(kw) = self.peek() {
                if kw == "default" && self.peek_n(1) == &Token::Def {
                    self.advance();
                    is_default = true;
                }
            }
            // ── 软关键字行（§11）：satisfies / sealed / match / require / optional ──
            if let Token::Ident(kw) = self.peek() {
                match kw.as_str() {
                    "satisfies" => {
                        self.advance();
                        match self.advance() {
                            Token::Ident(n) => satisfies.push(n),
                            t => {
                                return Err(format!(
                                    "Expected duck name after satisfies, got {:?}",
                                    t
                                ))
                            }
                        }
                        self.skip_newlines();
                        continue;
                    }
                    "sealed" => {
                        self.advance();
                        sealed = true;
                        self.skip_newlines();
                        continue;
                    }
                    "match" => {
                        self.advance();
                        // match /pattern/ at_least(N) / at_most(N) / exact(N)
                        //（模式用字符串字面量承载，如 match "get_\w+" at_least(1)）
                        let pattern = match self.advance() {
                            Token::StrLit(s) => s,
                            Token::RawStrLit(s) => s,
                            t => {
                                return Err(format!(
                                    "Expected regex pattern in duck match, got {:?}",
                                    t
                                ))
                            }
                        };
                        let count_kw = match self.advance() {
                            Token::Ident(n) => n,
                            t => {
                                return Err(format!(
                                    "Expected count constraint in duck match, got {:?}",
                                    t
                                ))
                            }
                        };
                        self.expect(Token::LParen)?;
                        let n = match self.advance() {
                            Token::IntLit(n) => n as usize,
                            t => {
                                return Err(format!(
                                    "Expected int in match constraint, got {:?}",
                                    t
                                ))
                            }
                        };
                        self.expect(Token::RParen)?;
                        let range = match count_kw.as_str() {
                            "at_least" => (n, usize::MAX),
                            "at_most" => (0, n),
                            "exact" => (n, n),
                            other => {
                                return Err(format!(
                                    "Expected at_least/at_most/exact, got {}",
                                    other
                                ))
                            }
                        };
                        match_rules.push(DuckMatchRule { pattern, range });
                        self.skip_newlines();
                        continue;
                    }
                    "require" | "optional" => {
                        let is_required = kw == "require";
                        self.advance();
                        self.expect(Token::LParen)?;
                        let mut names = Vec::new();
                        while !self.check(&Token::RParen) && !self.check(&Token::Eof) {
                            self.skip_newlines();
                            match self.advance() {
                                Token::Ident(n) => names.push(n),
                                Token::Self_ => names.push("self".to_string()),
                                t => {
                                    return Err(format!(
                                        "Expected param name in duck require/optional, got {:?}",
                                        t
                                    ))
                                }
                            }
                            // 可选类型标注 name: type
                            if self.check(&Token::Colon) {
                                self.advance();
                                self.parse_type()?;
                            }
                            if self.check(&Token::Comma) {
                                self.advance();
                            }
                        }
                        self.expect(Token::RParen)?;
                        param_reqs.push(DuckParamReq {
                            is_required,
                            names,
                        });
                        self.skip_newlines();
                        continue;
                    }
                    _ => {}
                }
            }
            if self.check(&Token::Def) {
                self.advance(); // def
                // 多泛型关系 duck 的类型前缀: def T.map(self) -> R
                let mut owner = None;
                if let Token::Ident(n) = self.peek() {
                    if self.peek_n(1) == &Token::Dot {
                        owner = Some(n.clone());
                        self.advance(); // 前缀类型名
                        self.advance(); // .
                    }
                }
                // 方法名：普通标识符 或 正则模式 "get_\w+"（§8.4）
                let mut name_pattern = None;
                let method_name = match self.advance() {
                    Token::Ident(n) => n,
                    Token::MagicMethod(n) => n,
                    Token::StrLit(p) => {
                        name_pattern = Some(p);
                        "__regex__".to_string()
                    }
                    t => return Err(format!("Expected method name in duck, got {:?}", t)),
                };
                // 解析参数（可含参数数量约束: exact(N)/min(N)/max(N)/range(L,R)）
                self.expect(Token::LParen)?;
                let mut param_range = None;
                let mut params = Vec::new();
                while !self.check(&Token::RParen) && !self.check(&Token::Eof) {
                    self.skip_newlines();
                    // 检测参数数量约束关键字: exact / min / max / range
                    if let Token::Ident(ref kw) = self.peek() {
                        if matches!(kw.as_str(), "exact" | "min" | "max" | "range")
                            && self.peek_n(1) == &Token::LParen
                        {
                            let kw = kw.clone();
                            self.advance(); // 关键字
                            self.advance(); // (
                            let first = match self.advance() {
                                Token::IntLit(n) => n as usize,
                                t => {
                                    return Err(format!(
                                        "Expected int in {} constraint, got {:?}",
                                        kw, t
                                    ))
                                }
                            };
                            let (lo, hi) = match kw.as_str() {
                                "exact" => (first, first),
                                "min" => (first, usize::MAX),
                                "max" => (0, first),
                                "range" => {
                                    self.expect(Token::Comma)?;
                                    let hi = match self.advance() {
                                        Token::IntLit(n) => n as usize,
                                        t => {
                                            return Err(format!(
                                                "Expected int for range max, got {:?}",
                                                t
                                            ))
                                        }
                                    };
                                    (first, hi)
                                }
                                _ => unreachable!(),
                            };
                            self.expect(Token::RParen)?;
                            param_range = Some((lo, hi));
                            if self.check(&Token::Comma) {
                                self.advance();
                            }
                            continue;
                        }
                    }
                    // 解析普通参数
                    let p = self.parse_single_param(false)?;
                    params.push(p);
                    if self.check(&Token::Comma) {
                        self.advance();
                    }
                }
                self.expect(Token::RParen)?;
                // 返回类型
                let ret = if self.check(&Token::Arrow) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                methods.push(DuckMethod {
                    owner,
                    name: method_name,
                    name_pattern,
                    params,
                    return_type: ret,
                    param_range,
                    is_default,
                });
            } else if self.check(&Token::Dot) {
                // .field_name: Type（无前缀）
                self.advance(); // .
                let field_name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(format!("Expected field name after . in duck, got {:?}", t)),
                };
                self.expect(Token::Colon)?;
                let field_ty = self.parse_type()?;
                fields.push(DuckField {
                    owner: None,
                    name: field_name,
                    ty: field_ty,
                    rel: None,
                });
            } else if let Token::Ident(ref kw) = self.peek() {
                if kw == "type" {
                    // 关联类型约束: `type I.Item`（§2.3）或 `type Item`（当前类型）
                    self.advance(); // type
                    let owner = if self.peek_n(1) == &Token::Dot {
                        match self.advance() {
                            Token::Ident(n) => Some(n),
                            t => {
                                return Err(format!(
                                    "Expected type prefix in duck assoc type, got {:?}",
                                    t
                                ))
                            }
                        }
                    } else {
                        None
                    };
                    if owner.is_some() {
                        self.advance(); // .
                    }
                    let assoc_name = match self.advance() {
                        Token::Ident(n) => n,
                        t => {
                            return Err(format!(
                                "Expected assoc type name in duck, got {:?}",
                                t
                            ))
                        }
                    };
                    assoc_types.push(DuckAssocType {
                        owner,
                        name: assoc_name,
                    });
                } else if self.peek_n(1) == &Token::Dot {
                    // 多泛型关系 duck 的字段前缀: A.x: f64 / A.x == B.y / A.x: B.y（§2.2）
                    let owner = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected type prefix in duck, got {:?}", t)),
                    };
                    self.advance(); // .
                    let field_name = match self.advance() {
                        Token::Ident(n) => n,
                        t => return Err(format!("Expected field name in duck, got {:?}", t)),
                    };
                    // A.x == B.y：类型等同关系
                    if self.check(&Token::EqEq) {
                        self.advance();
                        let rel_owner = match self.advance() {
                            Token::Ident(n) => n,
                            t => {
                                return Err(format!(
                                    "Expected type prefix after == in duck field, got {:?}",
                                    t
                                ))
                            }
                        };
                        self.advance(); // .
                        let rel_name = match self.advance() {
                            Token::Ident(n) => n,
                            t => {
                                return Err(format!(
                                    "Expected field name after == in duck field, got {:?}",
                                    t
                                ))
                            }
                        };
                        fields.push(DuckField {
                            owner: Some(owner),
                            name: field_name,
                            ty: Type::Any,
                            rel: Some((rel_owner, rel_name)),
                        });
                        continue;
                    }
                    self.expect(Token::Colon)?;
                    // A.x: B.y：简写形式（类型等同），区别于 A.x: f64（显式类型）
                    if let Token::Ident(_rel_owner) = self.peek() {
                        if self.peek_n(1) == &Token::Dot {
                            let rel_owner = match self.advance() {
                                Token::Ident(n) => n,
                                _ => unreachable!(),
                            };
                            self.advance(); // .
                            let rel_name = match self.advance() {
                                Token::Ident(n) => n,
                                t => {
                                    return Err(format!(
                                        "Expected field name after : in duck field, got {:?}",
                                        t
                                    ))
                                }
                            };
                            fields.push(DuckField {
                                owner: Some(owner),
                                name: field_name,
                                ty: Type::Any,
                                rel: Some((rel_owner, rel_name)),
                            });
                            continue;
                        }
                    }
                    let field_ty = self.parse_type()?;
                    fields.push(DuckField {
                        owner: Some(owner),
                        name: field_name,
                        ty: field_ty,
                        rel: None,
                    });
                } else {
                    self.advance(); // skip unexpected
                }
            } else {
                // skip unexpected tokens
                self.advance();
            }
            self.skip_newlines();
        }
        self.expect(Token::Dedent)?;

        Ok(DuckDef {
            name,
            generics,
            where_clause,
            assoc_types,
            satisfies,
            sealed,
            match_rules,
            param_reqs,
            methods,
            fields,
        })
    }
}
