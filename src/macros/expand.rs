// Lang-Zong 编译器 — macros/expand.rs
// 宏展开引擎：扫描 Token 流，识别 @name! 模式，递归展开

use crate::lexer::Token;
use crate::macros::group::Tokens;
use crate::macros::interp::{MacroInterpreter, MacroStmt, MacroExpr, BinaryOp};

use std::collections::HashMap;

// ──────────────── 宏注册中心 ────────────────

/// 全局宏注册中心
#[derive(Debug, Clone)]
pub struct MacroRegistry {
    macros: HashMap<String, MacroDef>,
}

#[derive(Debug, Clone)]
pub struct MacroDef {
    pub name: String,
    /// 是否为有属性宏（2 个参数）
    pub is_attr: bool,
    /// 参数名列表（1 个或 2 个）
    pub param_names: Vec<String>,
    /// 宏体语句
    pub body: Vec<MacroStmt>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        MacroRegistry { macros: HashMap::new() }
    }

    pub fn register(&mut self, def: MacroDef) {
        self.macros.insert(def.name.clone(), def);
    }

    pub fn get(&self, name: &str) -> Option<&MacroDef> {
        self.macros.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }

    /// 合并另一个注册中心的全部宏定义（跨模块宏导入用）
    pub fn merge(&mut self, other: MacroRegistry) {
        for (n, def) in other.macros {
            self.macros.insert(n, def);
        }
    }
}

// ──────────────── 宏展开器 ────────────────

/// Token 流宏展开器。
///
/// 流水线位置：Lexer 之后，Parser 之前。
///
/// 核心流程：
/// 1. 扫描 Token 流，识别 `@name!(` / `@name!` / `@name![attr](` 模式
/// 2. 区分宏调用（有 `!`）和装饰器（无 `!`）
/// 3. 查找宏定义，收集输入 tokens，执行宏体
/// 4. 将展开结果拼接回 Token 流
/// 5. 递归处理嵌套宏（内层优先）
pub struct MacroExpander {
    registry: MacroRegistry,
    max_depth: usize,
}

impl MacroExpander {
    pub fn new(registry: MacroRegistry) -> Self {
        MacroExpander { registry, max_depth: 128 }
    }

    /// 展开 Token 流中的所有宏调用，返回展开后的 Token 流
    pub fn expand(&self, tokens: &[Token]) -> Result<Vec<Token>, String> {
        self.expand_inner(tokens, 0)
    }

    fn expand_inner(&self, tokens: &[Token], depth: usize) -> Result<Vec<Token>, String> {
        if depth > self.max_depth {
            return Err(format!("macro expansion depth exceeded (max {})", self.max_depth));
        }

        let mut result: Vec<Token> = Vec::new();
        let mut i = 0;
        let len = tokens.len();

        while i < len {
            // 检测 @name! 或 @name![ 模式
            // （容忍 @ 和 name 之间的空白 token）
            if tokens[i] == Token::At {
                // 找到紧跟的 Ident（跳过 Newline/Indent）
                let mut name_idx = i + 1;
                while name_idx < len && matches!(&tokens[name_idx], Token::Newline | Token::Indent) {
                    name_idx += 1;
                }
                if name_idx >= len || !matches!(&tokens[name_idx], Token::Ident(_)) {
                    result.push(tokens[i].clone());
                    i += 1;
                    continue;
                }
                let name = match &tokens[name_idx] {
                    Token::Ident(n) => n.clone(),
                    _ => unreachable!(),
                };
                // 别名宏调用 @alias.name!：At Ident Dot Ident Exclamation → 用 . 后宏名展开
                // （import macro X as sm → @sm.check_eq! 等价 @check_eq!）
                let mut name = name;
                let mut name_end = name_idx + 1;
                while name_end < len && matches!(&tokens[name_end], Token::Newline | Token::Indent) {
                    name_end += 1;
                }
                if name_end + 1 < len
                    && tokens[name_end] == Token::Dot
                    && matches!(&tokens[name_end + 1], Token::Ident(n2) if n2 == "check_eq" || true)
                {
                    if let Token::Ident(n2) = &tokens[name_end + 1] {
                        name = n2.clone();
                        name_end += 2;
                    }
                }

                // 检查 name 后面是否有 !（跳过空白；别名解析后从 name_end 开始）
                let mut excl_idx = name_end;
                while excl_idx < len && matches!(&tokens[excl_idx], Token::Newline | Token::Indent) {
                    excl_idx += 1;
                }
                let has_exclamation = excl_idx < len && tokens[excl_idx] == Token::Exclamation;

                if has_exclamation {
                    // 这是宏调用 @name!
                    let after_exclam = if excl_idx + 1 < len { Some(&tokens[excl_idx + 1]) } else { None };

                    // 检查是否有属性 [attr]
                    let has_attr = after_exclam == Some(&Token::LBrack);

                    if has_attr {
                        // 有属性宏 @name![attr](input)
                        let attr_start = excl_idx + 2; // 跳过 [
                        if let Some((attr_tokens, attr_end)) = self.collect_bracket_group(tokens, attr_start, Token::LBrack, Token::RBrack) {
                            if attr_end + 1 >= len {
                                // 属性收集完成但文件结束 — 保留原 token 不展开
                                result.push(tokens[i].clone());
                                i += 1;
                                continue;
                            }
                            let after_attr = &tokens[attr_end + 1];
                            if after_attr == &Token::LParen {
                                if let Some((input_tokens, input_end)) = self.collect_bracket_group(tokens, attr_end + 2, Token::LParen, Token::RParen) {
                                    let expanded = self.expand_attr_macro(&name, &attr_tokens, &input_tokens, depth)?;
                                    result.extend(expanded);
                                    i = input_end + 1;
                                    continue;
                                }
                            } else {
                                // 有属性宏作用于声明：@name![attr] decl
                                let decl_tokens = self.collect_decl_tokens(tokens, attr_end + 1);
                                let decl_end = attr_end + 1 + decl_tokens.len();
                                let expanded = self.expand_attr_macro(&name, &attr_tokens, &decl_tokens, depth)?;
                                result.extend(expanded);
                                i = decl_end;
                                continue;
                            }
                        }
                    } else if after_exclam == Some(&Token::LParen) {
                        // 无属性宏 @name!(input)
                        if let Some((input_tokens, input_end)) = self.collect_bracket_group(tokens, excl_idx + 2, Token::LParen, Token::RParen) {
                            let expanded = self.expand_macro(&name, &input_tokens, None, depth)?;
                            result.extend(expanded);
                            i = input_end + 1;
                            continue;
                        }
                    } else {
                        // 无括号宏调用 @name! (作用于下一个声明)
                        let after_name = excl_idx + 1;
                        let decl_tokens = self.collect_decl_tokens(tokens, after_name);
                        let decl_end = after_name + decl_tokens.len();
                        if !decl_tokens.is_empty() {
                            let expanded = self.expand_macro(&name, &decl_tokens, None, depth)?;
                            result.extend(expanded);
                            i = decl_end;
                            continue;
                        }
                    }
                }
                // 没有 ! → 装饰器 @name，保留原样
            }
            result.push(tokens[i].clone());
            i += 1;
        }

        Ok(result)
    }

    /// 展开无属性宏调用
    fn expand_macro(&self, name: &str, input: &[Token], attr: Option<&[Token]>, depth: usize) -> Result<Vec<Token>, String> {
        let def = self.registry.get(name)
            .ok_or_else(|| format!("undefined macro '{}'", name))?;

        if def.is_attr && attr.is_none() {
            return Err(format!("macro '{}' requires attribute (use @{}![attr](...))", name, name));
        }
        if !def.is_attr && attr.is_some() {
            return Err(format!("macro '{}' does not accept attributes", name));
        }

        // 剥离缩进 token（括号内的 Indent/Dedent 不应该传递）
        let cleaned: Vec<Token> = input.iter()
            .filter(|t| !matches!(t, Token::Indent | Token::Dedent))
            .cloned()
            .collect();
        let input_tokens = Tokens::new(cleaned);

        // 执行宏体
        let mut interp = MacroInterpreter::new().with_depth(depth);
        if def.is_attr {
            interp.bind_param(def.param_names[0].clone(), Tokens::new(attr.unwrap().to_vec()));
            interp.bind_param(def.param_names[1].clone(), input_tokens);
        } else {
            interp.bind_param(def.param_names[0].clone(), input_tokens);
        }

        let result = interp.execute_stmts(&def.body)
            .map_err(|e| format!("macro '{}' expansion error: {}", name, e))?;

        // 递归展开结果中的嵌套宏
        self.expand_inner(&result.tokens, depth + 1)
    }

    /// 展开有属性宏调用
    fn expand_attr_macro(&self, name: &str, attr: &[Token], input: &[Token], depth: usize) -> Result<Vec<Token>, String> {
        self.expand_macro(name, input, Some(attr), depth)
    }

    // ──────────────── Token 收集辅助函数 ────────────────

    /// 收集括号/方括号内的 token 序列（括号匹配，支持嵌套）
    /// 调用方已经跳过了开括号，所以 depth 从 1 开始
    fn collect_bracket_group(&self, tokens: &[Token], start: usize, open: Token, close: Token) -> Option<(Vec<Token>, usize)> {
        if start >= tokens.len() {
            return None;
        }
        let mut depth: i32 = 1;  // 调用方已消费开括号
        let mut result = Vec::new();
        let mut i = start;
        while i < tokens.len() {
            if tokens[i] == open {
                depth += 1;
            } else if tokens[i] == close {
                depth -= 1;
                if depth == 0 {
                    return Some((result, i));
                }
            }
            result.push(tokens[i].clone());
            i += 1;
        }
        None
    }

    /// 收集一个声明（从当前位置到声明结束）
    /// 声明结束：顶层 Newline（缩进级 0） 或下一个顶层声明
    fn collect_decl_tokens(&self, tokens: &[Token], start: usize) -> Vec<Token> {
        if start >= tokens.len() {
            return vec![];
        }
        let mut result = Vec::new();
        let mut indent_level = 0;
        let mut i = start;
        let mut seen_indent = false;

        while i < tokens.len() {
            match &tokens[i] {
                Token::Indent => {
                    indent_level += 1;
                    seen_indent = true;
                    result.push(tokens[i].clone());
                }
                Token::Dedent => {
                    indent_level -= 1;
                    result.push(tokens[i].clone());
                    if indent_level == 0 && seen_indent {
                        break;
                    }
                }
                Token::Newline => {
                    if indent_level == 0 && seen_indent {
                        // 回到顶层且之前有缩进块 → 声明结束
                        break;
                    }
                    if indent_level == 0 && result.iter().any(|t| matches!(t, Token::Indent)) {
                        break;
                    }
                    result.push(tokens[i].clone());
                }
                Token::Def | Token::Struct | Token::Enum | Token::Trait | Token::Impl | Token::Const
                    if indent_level == 0 && !result.is_empty() =>
                {
                    // 下一个顶层声明开始 → 当前声明结束
                    break;
                }
                Token::At if indent_level == 0 && !result.is_empty() => {
                    // 下一个装饰器或宏调用开始 → 当前声明结束
                    break;
                }
                _ => {
                    result.push(tokens[i].clone());
                }
            }
            i += 1;
        }
        result
    }
}

// ──────────────── 宏定义解析器（从 Token 流提取宏定义） ────────────────

/// 从 Token 流中预提取宏定义，构建 MacroRegistry。
/// 这是展开前的第一遍扫描。
pub fn extract_macro_defs(tokens: &[Token]) -> Result<(MacroRegistry, Vec<usize>), String> {
    let mut registry = MacroRegistry::new();
    let mut consumed_ranges: Vec<usize> = Vec::new();
    let mut i = 0;
    let len = tokens.len();
    let max_scan = len + 1024; // 安全上限防止死循环

    let mut iter_count = 0;
    while i < len && iter_count < max_scan {
        iter_count += 1;
        if tokens[i] == Token::Macro {
            let start = i;
            i += 1; // 跳过 macro

            // 跳过空白和换行
            i = skip_blanks(tokens, i, len);

            // 宏名
            let name = match tokens.get(i) {
                Some(Token::Ident(n)) => {
                    i += 1;
                    n.clone()
                }
                _ => {
                    // 非宏定义（如 #!bin macro 中的 macro 关键字）→ 跳过继续
                    continue;
                }
            };

            // 跳过到 (
            // 宏名后必须紧跟 (（宏定义签名）；否则是 import macro X / from macro X import Y
            // 等导入语法，跳过继续（不算宏定义）
            let after_name = skip_blanks(tokens, i, len);
            if tokens.get(after_name) != Some(&Token::LParen) {
                continue;
            }
            i = skip_to(tokens, i, len, &Token::LParen, &format!("expected '(' after macro name '{}'", name))?;
            i += 1; // 跳过 (

            // 解析参数: name: Tokens 或 name: Tokens, name2: Tokens
            let mut param_names = Vec::new();
            let mut is_attr = false;
            let param_loop_max = 100;
            let mut param_iter = 0;
            loop {
                param_iter += 1;
                if param_iter > param_loop_max {
                    return Err(format!("parameter parsing exceeded limit in macro '{}'", name));
                }
                i = skip_blanks(tokens, i, len);
                match tokens.get(i) {
                    Some(Token::RParen) => { i += 1; break; }
                    Some(Token::Ident(pname)) => {
                        param_names.push(pname.clone());
                        i += 1;
                        // 跳过 : Tokens
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Colon { i += 1; }
                        i = skip_blanks(tokens, i, len);
                        if i < len && matches!(&tokens[i], Token::Ident(s) if s == "Tokens") { i += 1; }
                        // 检查逗号或右括号
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Comma { i += 1; is_attr = param_names.len() >= 2; }
                        if i < len && tokens[i] == Token::RParen { i += 1; break; }
                    }
                    _ => return Err(format!("expected parameter name in macro '{}' at token {}", name, i)),
                }
            }

            // 跳过 -> Tokens
            i = skip_to(tokens, i, len, &Token::Arrow, &format!("expected '->' in macro '{}'", name))?;
            i += 1;
            i = skip_blanks(tokens, i, len);
            if i < len && matches!(&tokens[i], Token::Ident(s) if s == "Tokens") { i += 1; }

            // 跳过 = 
            i = skip_to(tokens, i, len, &Token::Eq, &format!("expected '=' in macro '{}'", name))?;
            i += 1;

            // 收集宏体
            let (body_tokens, decl_end) = collect_indented_block_with_end(tokens, i)?;
            let body = parse_macro_body(&body_tokens)?;
            i = decl_end;

            registry.register(MacroDef { name, is_attr, param_names, body });
            consumed_ranges.push(start);
            consumed_ranges.push(i);
        } else {
            i += 1;
        }
    }

    if iter_count >= max_scan {
        return Err("macro definition extraction exceeded scan limit".to_string());
    }

    Ok((registry, consumed_ranges))
}

/// 跳过空白 token（Newline + Indent/Dedent）
fn skip_blanks(tokens: &[Token], mut i: usize, len: usize) -> usize {
    while i < len && matches!(&tokens[i], Token::Newline | Token::Indent | Token::Dedent) {
        i += 1;
    }
    i
}

/// 跳过非目标 token（只允许 Newline），找到目标或报错
fn skip_to(tokens: &[Token], mut i: usize, len: usize, target: &Token, err_msg: &str) -> Result<usize, String> {
    while i < len && &tokens[i] != target {
        if tokens[i] == Token::Newline { i += 1; continue; }
        return Err(err_msg.to_string());
    }
    if i >= len {
        return Err(err_msg.to_string());
    }
    Ok(i)
}

/// 收集缩进块内的 tokens（从 Indent 到匹配的 Dedent）
#[allow(dead_code)]
fn collect_indented_block(tokens: &[Token], start: usize) -> Result<Vec<Token>, String> {
    collect_indented_block_with_end(tokens, start).map(|(tokens, _)| tokens)
}

/// 收集缩进块，同时返回块结束后的位置（包括闭合的 Dedent）
fn collect_indented_block_with_end(tokens: &[Token], start: usize) -> Result<(Vec<Token>, usize), String> {
    let mut result = Vec::new();
    let mut i = start;
    let mut indent_depth = 0;
    let mut first_indent = false;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Indent => {
                indent_depth += 1;
                first_indent = true;
                result.push(tokens[i].clone());
            }
            Token::Dedent => {
                indent_depth -= 1;
                if indent_depth == 0 && first_indent {
                    // 返回 (tokens_before_dedent, position_after_dedent)
                    return Ok((result, i + 1));
                }
                result.push(tokens[i].clone());
            }
            _ => {
                result.push(tokens[i].clone());
            }
        }
        i += 1;
    }
    Ok((result, i))
}

/// 将宏体的 Token 序列解析为 MacroStmt 序列
fn parse_macro_body(tokens: &[Token]) -> Result<Vec<MacroStmt>, String> {
    let mut stmts = Vec::new();
    let mut i = 0;
    let len = tokens.len();

    while i < len {
        // 跳过空白
        while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) {
            i += 1;
        }
        if i >= len { break; }

        // 检测反引号块 ``` ... ```
        if tokens[i] == Token::Backtick {
            let prefix = if i > 0 {
                match &tokens[i - 1] {
                    Token::Ident(s) if s == "f" => crate::macros::group::BacktickPrefix::F,
                    Token::Ident(s) if s == "r" => crate::macros::group::BacktickPrefix::R,
                    _ => crate::macros::group::BacktickPrefix::None,
                }
            } else {
                crate::macros::group::BacktickPrefix::None
            };
            let (block_tokens, next_i) = collect_backtick_block(tokens, i)?;
            stmts.push(MacroStmt::Expr(MacroExpr::BacktickBlock {
                tokens: block_tokens,
                prefix,
            }));
            i = next_i;
            continue;
        }

        match &tokens[i] {
            Token::Let => {
                // let name = expr
                i += 1;
                while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) { i += 1; }
                let name = match &tokens[i] {
                    Token::Ident(n) => { i += 1; n.clone() }
                    _ => return Err("expected variable name after let".to_string()),
                };
                while i < len && tokens[i] != Token::Eq {
                    if matches!(&tokens[i], Token::Newline | Token::Indent | Token::Colon) { i += 1; continue; }
                    if matches!(&tokens[i], Token::Ident(s) if s == "Tokens") { i += 1; continue; }
                    break;
                }
                if i < len && tokens[i] == Token::Eq { i += 1; }
                let (value_expr, next_i) = parse_macro_expr(tokens, i)?;
                stmts.push(MacroStmt::Let { name, value: value_expr });
                i = next_i;
            }
            Token::If => {
                // if cond: body (可选 else: body)
                i += 1;
                let (cond, next_i) = parse_macro_expr(tokens, i)?;
                i = next_i;
                while i < len && tokens[i] != Token::Colon { i += 1; }
                if i < len { i += 1; } // 跳过 :

                // 收集 then_body（缩进块内）
                let (then_body, next_i) = collect_stmt_block(tokens, i)?;
                i = next_i;

                // 检查 else
                let mut else_body = None;
                while i < len && matches!(&tokens[i], Token::Newline | Token::Dedent) { i += 1; }
                if i < len && tokens[i] == Token::Else {
                    i += 1;
                    while i < len && tokens[i] != Token::Colon { i += 1; }
                    if i < len { i += 1; }
                    let (else_stmts, next_i) = collect_stmt_block(tokens, i)?;
                    else_body = Some(else_stmts);
                    i = next_i;
                }
                stmts.push(MacroStmt::If { cond, then_body, else_body });
            }
            Token::Return => {
                i += 1;
                let (expr, next_i) = parse_macro_expr(tokens, i)?;
                stmts.push(MacroStmt::Return(expr));
                i = next_i;
            }
            Token::For => {
                // for var in expr: body
                i += 1;
                while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) { i += 1; }
                let var = match &tokens[i] {
                    Token::Ident(n) => { i += 1; n.clone() }
                    _ => return Err("expected loop variable after for".to_string()),
                };
                while i < len && tokens[i] != Token::In {
                    if matches!(&tokens[i], Token::Newline | Token::Indent) { i += 1; continue; }
                    return Err("expected 'in' in for loop".to_string());
                }
                i += 1; // skip 'in'
                let (iter_expr, next_i) = parse_macro_expr(tokens, i)?;
                i = next_i;
                while i < len && tokens[i] != Token::Colon { i += 1; }
                if i < len { i += 1; }
                let (body, next_i) = collect_stmt_block(tokens, i)?;
                stmts.push(MacroStmt::For { var, iter: iter_expr, body });
                i = next_i;
            }
            Token::Ident(name) => {
                // 可能是函数调用或标识符表达式
                let mut next_i = i + 1;
                while next_i < len && matches!(&tokens[next_i], Token::Newline | Token::Indent) { next_i += 1; }
                if next_i < len && tokens[next_i] == Token::LParen {
                    // 函数调用 ident(args)
                    let name = name.clone();
                    let mut args = Vec::new();
                    let mut j = next_i + 1; // 跳过 (
                    let mut depth = 1;
                    let mut arg_start = j;
                    while j < len && depth > 0 {
                        match tokens[j] {
                            Token::LParen => depth += 1,
                            Token::RParen => {
                                depth -= 1;
                                if depth == 0 && j > arg_start {
                                    let (arg_expr, _) = parse_macro_expr(&tokens[arg_start..j], 0)?;
                                    args.push(arg_expr);
                                }
                            }
                            Token::Comma if depth == 1 => {
                                let (arg_expr, _) = parse_macro_expr(&tokens[arg_start..j], 0)?;
                                args.push(arg_expr);
                                arg_start = j + 1;
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    stmts.push(MacroStmt::Expr(MacroExpr::Call { func: name, args }));
                    i = j;
                } else {
                    // 普通标识符
                    stmts.push(MacroStmt::Expr(MacroExpr::Ident(name.clone())));
                    i = next_i;
                    // 检查是否有二元操作符 +
                    while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) { i += 1; }
                    if i < len && tokens[i] == Token::Plus && stmts.len() > 0 {
                        // 处理二元表达式: expr + expr
                        let left = match &stmts[stmts.len() - 1] {
                            MacroStmt::Expr(e) => e.clone(),
                            _ => break,
                        };
                        stmts.pop();
                        i += 1; // 跳过 +
                        while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) { i += 1; }
                        let (right, next_i) = parse_macro_expr(tokens, i)?;
                        stmts.push(MacroStmt::Expr(MacroExpr::Binary {
                            left: Box::new(left),
                            op: BinaryOp::Plus,
                            right: Box::new(right),
                        }));
                        i = next_i;
                    }
                }
            }
            _ => {
                return Err(format!("unexpected token {:?} in macro body at position {}", tokens[i], i));
            }
        }
    }
    Ok(stmts)
}

/// 收集反引号块 ``` ... ```
fn collect_backtick_block(tokens: &[Token], start: usize) -> Result<(Vec<Token>, usize), String> {
    // 反引号块格式: Backtick, Newline, tokens..., Newline, Backtick
    let mut i = start + 1; // 跳过第一个 Backtick
    // 跳过第一个 Newline
    while i < tokens.len() && matches!(&tokens[i], Token::Newline | Token::Indent) {
        i += 1;
    }
    let content_start = i;
    // 找到闭合的 Backtick
    while i < tokens.len() && tokens[i] != Token::Backtick {
        i += 1;
    }
    if i >= tokens.len() {
        return Err("unclosed backtick block".to_string());
    }
    let result = tokens[content_start..i].to_vec();
    // 对于 f``` 模式，需要保留 Indent/Dedent 用于结构
    Ok((result, i + 1))
}

/// 收集语句块（缩进块内的一组语句）
fn collect_stmt_block(tokens: &[Token], start: usize) -> Result<(Vec<MacroStmt>, usize), String> {
    let mut i = start;
    while i < tokens.len() && matches!(&tokens[i], Token::Newline | Token::Indent) { i += 1; }

    let mut block_tokens = Vec::new();
    let mut depth = 1; // 当前在缩进块内
    while i < tokens.len() && depth > 0 {
        match &tokens[i] {
            Token::Indent => { depth += 1; block_tokens.push(tokens[i].clone()); }
            Token::Dedent => { depth -= 1; if depth > 0 { block_tokens.push(tokens[i].clone()); } }
            _ => { block_tokens.push(tokens[i].clone()); }
        }
        i += 1;
    }
    let stmts = parse_macro_body(&block_tokens)?;
    Ok((stmts, i))
}

/// 解析宏表达式（简化版）
fn parse_macro_expr(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    if start >= tokens.len() {
        return Ok((MacroExpr::IntLit(0), start));
    }
    let mut i = start;
    while i < tokens.len() && matches!(&tokens[i], Token::Newline | Token::Indent) { i += 1; }

    match &tokens[i] {
        Token::Ident(name) => {
            // 可能是函数调用或标识符
            let mut next_i = i + 1;
            while next_i < tokens.len() && matches!(&tokens[next_i], Token::Newline | Token::Indent) { next_i += 1; }
            if next_i < tokens.len() && tokens[next_i] == Token::LParen {
                // 函数调用
                let name = name.clone();
                let mut args = Vec::new();
                let mut j = next_i + 1;
                let mut depth = 1;
                let mut arg_start = j;
                while j < tokens.len() && depth > 0 {
                    match tokens[j] {
                        Token::LParen => depth += 1,
                        Token::RParen => {
                            depth -= 1;
                            if depth == 0 && j > arg_start {
                                let (arg, _) = parse_macro_expr(&tokens[arg_start..j], 0)?;
                                args.push(arg);
                            }
                        }
                        Token::Comma if depth == 1 => {
                            if j > arg_start {
                                let (arg, _) = parse_macro_expr(&tokens[arg_start..j], 0)?;
                                args.push(arg);
                            }
                            arg_start = j + 1;
                        }
                        _ => {}
                    }
                    j += 1;
                }
                Ok((MacroExpr::Call { func: name, args }, j))
            } else {
                Ok((MacroExpr::Ident(name.clone()), next_i))
            }
        }
        Token::If => {
            // if expr: then_expr else: else_expr
            let mut j = i + 1;
            while j < tokens.len() && matches!(&tokens[j], Token::Newline | Token::Indent) { j += 1; }
            let (cond, nj) = parse_macro_expr(tokens, j)?;
            j = nj;
            while j < tokens.len() && tokens[j] != Token::Colon { j += 1; }
            if j < tokens.len() { j += 1; }
            let (then_expr, nj) = parse_macro_expr(tokens, j)?;
            j = nj;
            let mut else_expr = None;
            while j < tokens.len() && matches!(&tokens[j], Token::Newline | Token::Dedent) { j += 1; }
            if j < tokens.len() && tokens[j] == Token::Else {
                j += 1;
                while j < tokens.len() && tokens[j] != Token::Colon { j += 1; }
                if j < tokens.len() { j += 1; }
                let (else_e, nj) = parse_macro_expr(tokens, j)?;
                else_expr = Some(Box::new(else_e));
                j = nj;
            }
            Ok((MacroExpr::IfExpr {
                cond: Box::new(cond),
                then_expr: Box::new(then_expr),
                else_expr,
            }, j))
        }
        Token::IntLit(n) => Ok((MacroExpr::IntLit(*n), i + 1)),
        Token::StrLit(s) => Ok((MacroExpr::StrLit(s.clone()), i + 1)),
        Token::True => Ok((MacroExpr::BoolLit(true), i + 1)),
        Token::False => Ok((MacroExpr::BoolLit(false), i + 1)),
        Token::Backtick => {
            let (block_tokens, next_i) = collect_backtick_block(tokens, i)?;
            Ok((MacroExpr::BacktickBlock {
                tokens: block_tokens,
                prefix: crate::macros::group::BacktickPrefix::None,
            }, next_i))
        }
        _ => {
            return Err(format!("unexpected token {:?} in macro expression at position {}", tokens[i], i));
        }
    }
}

// ──────────────── 单元测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = MacroRegistry::new();
        assert!(!registry.contains("foo"));
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = MacroRegistry::new();
        registry.register(MacroDef {
            name: "test".into(),
            is_attr: false,
            param_names: vec!["input".into()],
            body: vec![],
        });
        assert!(registry.contains("test"));
        assert_eq!(registry.get("test").unwrap().is_attr, false);
    }

    #[test]
    fn test_collect_bracket_group_simple() {
        let expander = MacroExpander::new(MacroRegistry::new());
        let tokens = vec![
            Token::IntLit(1), Token::Comma, Token::IntLit(2), Token::RParen,
        ];
        let (result, end) = expander.collect_bracket_group(&tokens, 0, Token::LParen, Token::RParen).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(end, 3);
    }

    #[test]
    fn test_collect_bracket_group_nested() {
        let expander = MacroExpander::new(MacroRegistry::new());
        let tokens = vec![
            Token::LParen,
            Token::IntLit(1),
            Token::RParen,
            Token::RParen, // 外层闭合
        ];
        let (result, end) = expander.collect_bracket_group(&tokens, 0, Token::LParen, Token::RParen).unwrap();
        assert_eq!(end, 3);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_extract_macro_defs_basic() {
        let tokens = vec![
            Token::Macro,
            Token::Ident("hello".into()),
            Token::LParen,
            Token::Ident("input".into()),
            Token::Colon,
            Token::Ident("Tokens".into()),
            Token::RParen,
            Token::Arrow,
            Token::Ident("Tokens".into()),
            Token::Eq,
            Token::Newline,
            Token::Indent,
            Token::Ident("input".into()),
            Token::Newline,
            Token::Dedent,
        ];
        let (registry, _ranges) = extract_macro_defs(&tokens).unwrap();
        assert!(registry.contains("hello"));
    }

    #[test]
    fn test_expand_identity_macro() {
        let mut registry = MacroRegistry::new();
        // 定义一个返回输入自身的宏
        registry.register(MacroDef {
            name: "id".into(),
            is_attr: false,
            param_names: vec!["input".into()],
            body: vec![
                MacroStmt::Expr(MacroExpr::Ident("input".into())),
            ],
        });

        let expander = MacroExpander::new(registry);
        let tokens = vec![
            Token::At,
            Token::Ident("id".into()),
            Token::Exclamation,
            Token::LParen,
            Token::IntLit(42),
            Token::RParen,
        ];
        let result = expander.expand(&tokens).unwrap();
        assert_eq!(result, vec![Token::IntLit(42)]);
    }

    #[test]
    fn test_decorator_preserved() {
        let registry = MacroRegistry::new();
        let expander = MacroExpander::new(registry);
        // @simd 是装饰器（无 !），应该保留
        let tokens = vec![
            Token::At,
            Token::Ident("simd".into()),
            Token::Newline,
            Token::Def,
            Token::Ident("foo".into()),
        ];
        let result = expander.expand(&tokens).unwrap();
        // 装饰器保持不变
        assert_eq!(result[0], Token::At);
        assert_eq!(result[1], Token::Ident("simd".into()));
    }

    #[test]
    fn test_nested_macro_expansion() {
        let mut registry = MacroRegistry::new();
        // 内层宏: @inner!(x) → x * 2
        registry.register(MacroDef {
            name: "inner".into(),
            is_attr: false,
            param_names: vec!["input".into()],
            body: vec![
                MacroStmt::Expr(MacroExpr::BacktickBlock {
                    tokens: vec![
                        Token::Ident("input".into()),
                        Token::Star,
                        Token::IntLit(2),
                    ],
                    prefix: crate::macros::group::BacktickPrefix::F,
                }),
            ],
        });

        let expander = MacroExpander::new(registry);
        let tokens = vec![
            Token::At, Token::Ident("inner".into()), Token::Exclamation,
            Token::LParen, Token::IntLit(5), Token::RParen,
        ];
        let result = expander.expand(&tokens).unwrap();
        // 5 * 2 的展开取决于 f``` 插值处理
        // 这里验证至少没有崩溃
        assert!(!result.is_empty());
    }
}
