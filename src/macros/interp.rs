// Lang-Zong 编译器 — macros/interp.rs
// 编译期宏解释器：在编译时执行宏体，操作 Tokens 类型

use crate::lexer::Token;
use crate::macros::group::{Tokens, TokenGroupKind, BacktickPrefix, TokenTree};
#[cfg(test)]
use crate::macros::group::Delimiter;
use crate::macros::pattern::{
    TokenPattern, ReplaceRule,
    apply_remove, apply_replace,
};
use std::collections::HashMap;

// ──────────────── 解释器 ────────────────

/// 编译期宏解释器。
/// 执行宏体的受限 lz 代码，操作 Tokens 值。
pub struct MacroInterpreter {
    /// 变量环境：变量名 → Tokens 值
    variables: HashMap<String, Tokens>,
    /// 上下文键值对（嵌套宏通信）
    context: HashMap<String, String>,
    /// 当前深度（递归保护）
    depth: usize,
    /// 最大递归深度
    #[allow(dead_code)]
    max_depth: usize,
}

impl MacroInterpreter {
    pub fn new() -> Self {
        MacroInterpreter {
            variables: HashMap::new(),
            context: HashMap::new(),
            depth: 0,
            max_depth: 128,
        }
    }

    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    pub fn with_context(&mut self, ctx: HashMap<String, String>) {
        self.context = ctx;
    }

    /// 绑定宏参数
    pub fn bind_param(&mut self, name: String, value: Tokens) {
        self.variables.insert(name, value);
    }

    // ──────────────── 语句执行 ────────────────

    /// 执行宏体语句序列，返回最后一个表达式的 Tokens 值
    pub fn execute_stmts(&mut self, stmts: &[MacroStmt]) -> Result<Tokens, String> {
        let mut last = Tokens::empty();
        for stmt in stmts {
            last = self.execute_stmt(stmt)?;
        }
        Ok(last)
    }

    fn execute_stmt(&mut self, stmt: &MacroStmt) -> Result<Tokens, String> {
        match stmt {
            MacroStmt::Let { name, value } => {
                let val = self.eval_expr(value)?;
                self.variables.insert(name.clone(), val);
                Ok(Tokens::empty())
            }
            MacroStmt::Expr(expr) => {
                self.eval_expr(expr)
            }
            MacroStmt::If { cond, then_body, else_body } => {
                let cond_val = self.eval_expr(cond)?;
                let is_true = !cond_val.is_empty()
                    && !matches!(cond_val.tokens.first(), Some(Token::False))
                    && !matches!(cond_val.tokens.first(), Some(Token::Ident(name)) if name == "None");
                if is_true {
                    self.execute_stmts(then_body)
                } else if let Some(else_stmts) = else_body {
                    self.execute_stmts(else_stmts)
                } else {
                    Ok(Tokens::empty())
                }
            }
            MacroStmt::Return(expr) => {
                self.eval_expr(expr)
            }
            MacroStmt::For { var, iter, body } => {
                let iter_tokens = self.eval_expr(iter)?;
                // 将 Tokens 按 token 逐个迭代
                let mut last = Tokens::empty();
                for token in &iter_tokens.tokens {
                    self.variables.insert(var.clone(), Tokens::new(vec![token.clone()]));
                    last = self.execute_stmts(body)?;
                }
                Ok(last)
            }
        }
    }

    // ──────────────── 表达式求值 ────────────────

    fn eval_expr(&mut self, expr: &MacroExpr) -> Result<Tokens, String> {
        match expr {
            MacroExpr::BacktickBlock { tokens, prefix } => {
                self.eval_backtick(tokens, *prefix)
            }
            MacroExpr::Ident(name) => {
                self.variables.get(name)
                    .cloned()
                    .ok_or_else(|| format!("undefined variable '{}'", name))
            }
            MacroExpr::Call { func, args } => {
                self.eval_builtin(func, args)
            }
            MacroExpr::Binary { left, op, right } => {
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                #[allow(unreachable_patterns)]
                match op {
                    BinaryOp::Plus => {
                        // 合并相邻 StrLit：quote("a" + name + "b") 产生
                        // [StrLit("a"), StrLit("World"), StrLit("b")] → 单个 StrLit，
                        // 否则 parser 把它们解析为分散语句而非字符串拼接
                        let mut merged = l.concat(r);
                        merged.tokens = merge_str_lits(merged.tokens);
                        Ok(merged)
                    }
                    _ => Err(format!("unsupported binary op {:?} in macro", op)),
                }
            }
            MacroExpr::IfExpr { cond, then_expr, else_expr } => {
                let cond_val = self.eval_expr(cond)?;
                let is_true = !cond_val.is_empty()
                    && !matches!(cond_val.tokens.first(), Some(Token::False))
                    && !matches!(cond_val.tokens.first(), Some(Token::Ident(name)) if name == "None");
                if is_true {
                    self.eval_expr(then_expr)
                } else if let Some(else_e) = else_expr {
                    self.eval_expr(else_e)
                } else {
                    Ok(Tokens::empty())
                }
            }
            MacroExpr::IntLit(n) => {
                Ok(Tokens::new(vec![Token::IntLit(*n)]))
            }
            MacroExpr::StrLit(s) => {
                Ok(Tokens::new(vec![Token::StrLit(s.clone())]))
            }
            MacroExpr::BoolLit(b) => {
                Ok(Tokens::new(vec![if *b { Token::True } else { Token::False }]))
            }
        }
    }

    // ──────────────── 反引号块求值 ────────────────

    fn eval_backtick(&mut self, tokens: &[Token], prefix: BacktickPrefix) -> Result<Tokens, String> {
        match prefix {
            BacktickPrefix::None => {
                // 普通 ``` — 直接返回 tokens 副本
                Ok(Tokens::new(tokens.to_vec()))
            }
            BacktickPrefix::F => {
                // f``` — 处理 $(expr) 插值
                let mut result: Vec<Token> = Vec::new();
                let mut i = 0;
                while i < tokens.len() {
                    // 检测 $(expr) 模式: Dollar + LParen
                    if tokens[i] == Token::Dollar && i + 1 < tokens.len()
                        && tokens[i + 1] == Token::LParen
                    {
                        // 找到匹配的 RParen（需要括号计数）
                        let start = i + 2; // 跳过 $( 中的 (
                        let mut depth = 1;
                        let mut j = start;
                        while j < tokens.len() && depth > 0 {
                            match tokens[j] {
                                Token::LParen => depth += 1,
                                Token::RParen => depth -= 1,
                                _ => {}
                            }
                            if depth > 0 { j += 1; }
                        }
                        if depth != 0 {
                            return Err("unmatched parenthesis in $() interpolation".to_string());
                        }
                        // 插值表达式 tokens 在 tokens[start..j] 中
                        // 简化处理：尝试识别简单的标识符或字面量
                        let interp_tokens = &tokens[start..j];
                        let val = self.eval_interp_tokens(interp_tokens)?;
                        result.extend(val.tokens);
                        i = j + 1; // 跳过 RParen
                    } else {
                        result.push(tokens[i].clone());
                        i += 1;
                    }
                }
                Ok(Tokens::new(result))
            }
            BacktickPrefix::R => {
                // r``` — 原始模式：全部保留原样（包括 $ 和 @）
                Ok(Tokens::new(tokens.to_vec()))
            }
        }
    }

    /// 对 $(...) 中的插值 tokens 求值
    fn eval_interp_tokens(&mut self, tokens: &[Token]) -> Result<Tokens, String> {
        if tokens.is_empty() {
            return Ok(Tokens::empty());
        }
        // 简单场景：单个标识符 → 查变量
        if tokens.len() == 1 {
            if let Token::Ident(name) = &tokens[0] {
                if let Some(val) = self.variables.get(name) {
                    return Ok(val.clone());
                }
                // 尝试作为内置函数调用
                if is_builtin(name) {
                    return Ok(Tokens::new(vec![Token::Ident(name.clone())]));
                }
            }
            // 字面量
            return Ok(Tokens::new(tokens.to_vec()));
        }
        // 复杂表达式（如 `$(x + 1)` / `$(len(items))`）：用宏表达式解析器
        // 解析为 MacroExpr 再求值，返回其结果 Tokens（08 §3.2 f``` 插值）
        let (expr, _end) = crate::macros::expand::parse_macro_expr(tokens, 0)?;
        self.eval_expr(&expr)
    }

    // ──────────────── 内置函数求值 ────────────────

    fn eval_builtin(&mut self, func: &str, args: &[MacroExpr]) -> Result<Tokens, String> {
        match func {
            "is_empty" => {
                if args.len() != 1 { return Err("is_empty requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                Ok(Tokens::new(vec![if val.is_empty() { Token::True } else { Token::False }]))
            }
            "len" => {
                if args.len() != 1 { return Err("len requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                Ok(Tokens::new(vec![Token::IntLit(val.len() as i64)]))
            }
            "first" => {
                if args.len() != 1 { return Err("first requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                Ok(val.first())
            }
            "rest" => {
                if args.len() != 1 { return Err("rest requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                Ok(val.rest())
            }
            "classify" => {
                if args.len() != 1 { return Err("classify requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                let kind_str = match val.kind {
                    TokenGroupKind::Expr => "Expr",
                    TokenGroupKind::Decl => "Decl",
                    TokenGroupKind::Stmt => "Stmt",
                    TokenGroupKind::Pattern => "Pattern",
                    TokenGroupKind::Type => "Type",
                    TokenGroupKind::Any => "Any",
                };
                Ok(Tokens::new(vec![Token::StrLit(kind_str.to_string())]))
            }
            "to_string" => {
                if args.len() != 1 { return Err("to_string requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                Ok(Tokens::new(vec![Token::StrLit(val.to_string())]))
            }
            "set_context" => {
                if args.len() != 2 { return Err("set_context requires 2 args".to_string()); }
                let k = self.eval_expr(&args[0])?;
                let v = self.eval_expr(&args[1])?;
                self.context.insert(k.to_string(), v.to_string());
                Ok(Tokens::empty())
            }
            "get_context" => {
                if args.len() != 1 { return Err("get_context requires 1 arg".to_string()); }
                let k = self.eval_expr(&args[0])?;
                match self.context.get(&k.to_string()) {
                    Some(v) => Ok(Tokens::new(vec![Token::StrLit(v.clone())])),
                    None => Ok(Tokens::new(vec![Token::Ident("None".to_string())])),
                }
            }
            "assert_parent" => {
                if args.len() != 1 { return Err("assert_parent requires 1 arg".to_string()); }
                let expected = self.eval_expr(&args[0])?;
                match self.context.get("__parent_name") {
                    Some(parent) if parent == &expected.to_string() => Ok(Tokens::empty()),
                    _ => Err(format!("expected parent macro '{}'", expected.to_string())),
                }
            }
            "inside_parent" => {
                if args.len() != 1 { return Err("inside_parent requires 1 arg".to_string()); }
                let name = self.eval_expr(&args[0])?;
                let is_inside = self.context.get("__parent_name")
                    .map_or(false, |p| p == &name.to_string());
                Ok(Tokens::new(vec![if is_inside { Token::True } else { Token::False }]))
            }

            // ── 宏 API 工具集 ──

            // quote(tokens) → 原样返回；但其中的 StrLit 内容需重新词法分析为
            // 代码 token（模板/macro 产物是 LZ 代码，字符串字面量只是源码文本载体）
            "quote" => {
                if args.len() != 1 { return Err("quote requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                let mut out: Vec<Token> = Vec::new();
                for t in val.tokens {
                    if let Token::StrLit(s) = &t {
                        // 字符串内容 → 重新 lex 为代码 token（guard/print 等语句）。
                        // LZ 语句用换行分隔，分号是 Rust 风格残留，过滤掉（否则
                        // parser 报 Unexpected token: Semicolon）。
                        // 前导空白需 trim：lexer 把行首空格当缩进（Indent token），
                        // `" * 2)"` 会混入 Indent 导致 Expected RParen, got Indent
                        let trimmed = s.trim_start();
                        // 缩进重平衡（08 §3.6）：字符串以换行 + 行首缩进空白结尾
                        // （如 `"for i in 0..2:\n    "` 拼接 body）时，lexer 在
                        // 行首空白 + EOF 处 break，不产生 Indent token——拼接的
                        // body 丢失缩进 → "Expected Indent, got Let"。检测到该
                        // 形态时在 lex 结果末尾补一个 Indent，使 body 进入块内。
                        let needs_indent = match trimmed.rsplit_once('\n') {
                            Some((_, tail)) => {
                                !tail.is_empty()
                                    && tail.chars().all(|c| c == ' ' || c == '\t')
                            }
                            None => false,
                        };
                        let mut lexer = crate::lexer::Lexer::new(trimmed);
                        let mut toks: Vec<Token> = lexer
                            .tokenize()
                            .into_iter()
                            .filter(|t| !matches!(t, Token::Eof | Token::Semicolon))
                            .collect();
                        if needs_indent {
                            // lexer 的 EOF 清理会删除字符串末尾 `\n` 产生的 Newline
                            // （tokenize 尾部 `while last==Newline pop`），此处补回
                            // Newline + Indent，使拼接的 body 进入块内：
                            // `"for i in 0..2:\n    " + body` →
                            //   for i in 0..2: <Newline> <Indent> <body>
                            if toks.last() != Some(&Token::Newline) {
                                toks.push(Token::Newline);
                            }
                            toks.push(Token::Indent);
                        }
                        out.extend(toks);
                    } else {
                        out.push(t);
                    }
                }
                // 缩进自平衡（08 §3.6）：quote 产物（含拼接的参数 body）中
                // 未闭合的 Indent 在末尾补匹配 Dedent，使产物独立合法、插入
                // 任意缩进上下文不残留未闭合缩进（light_check 缩进配对）。
                // 同时过滤参数 body 带入的 Eof/Semicolon（collect_decl_tokens
                // 可能收集到文件末尾的 Eof，混入产物会提前终止 parser 解析）。
                out.retain(|t| !matches!(t, Token::Eof | Token::Semicolon));
                let net_indent = out
                    .iter()
                    .map(|t| match t {
                        Token::Indent => 1i32,
                        Token::Dedent => -1i32,
                        _ => 0,
                    })
                    .sum::<i32>();
                if net_indent > 0 {
                    for _ in 0..net_indent {
                        out.push(Token::Dedent);
                    }
                }
                Ok(Tokens::new(out))
            }

            // merge_tokens(a, b, ...) → 拼接多个 Tokens
            "merge_tokens" => {
                if args.is_empty() { return Err("merge_tokens requires at least 1 arg".to_string()); }
                let mut result = self.eval_expr(&args[0])?;
                for arg in &args[1..] {
                    let next = self.eval_expr(arg)?;
                    result = result.concat(next);
                }
                Ok(result)
            }

            // token_count(stream) → 顶层 token 数
            "token_count" => {
                if args.len() != 1 { return Err("token_count requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                Ok(Tokens::new(vec![Token::IntLit(val.len() as i64)]))
            }

            // is_empty_tokens(stream) → 是否为空
            "is_empty_tokens" => {
                if args.len() != 1 { return Err("is_empty_tokens requires 1 arg".to_string()); }
                let val = self.eval_expr(&args[0])?;
                Ok(Tokens::new(vec![if val.is_empty() { Token::True } else { Token::False }]))
            }

            // filter_tokens(stream, kind) → 按类型过滤
            "filter_tokens" => {
                if args.len() != 2 { return Err("filter_tokens requires 2 args: (source, kind)".to_string()); }
                let src = self.eval_expr(&args[0])?;
                let kind_val = self.eval_expr(&args[1])?;
                let kind = kind_val.to_string().trim_matches('"').to_string();
                let filtered: Vec<Token> = src.tokens.into_iter()
                    .filter(|t| token_matches_kind(t, &kind))
                    .collect();
                Ok(Tokens::new(filtered))
            }

            // remove_tokens(source, pattern) → 移除匹配的 token 序列
            "remove_tokens" => {
                if args.len() != 2 { return Err("remove_tokens requires 2 args: (source, pattern)".to_string()); }
                let src = self.eval_expr(&args[0])?;
                let pattern_val = self.eval_expr(&args[1])?;
                let pattern = TokenPattern::parse(&pattern_val.tokens)
                    .map_err(|e| format!("remove_tokens: invalid pattern: {}", e))?;
                let result = apply_remove(&src.tokens, &pattern);
                Ok(Tokens::new(result))
            }

            // replace_tokens(source, rules) → 查找替换
            "replace_tokens" => {
                if args.len() != 2 { return Err("replace_tokens requires 2 args: (source, rules)".to_string()); }
                let src = self.eval_expr(&args[0])?;
                let rules_val = self.eval_expr(&args[1])?;
                // rules 格式: from_pattern => to_tokens, from2 => to2, ...
                let rules = ReplaceRule::parse(&rules_val.tokens)
                    .map_err(|e| format!("replace_tokens: invalid rules: {}", e))?;
                let result = apply_replace(&src.tokens, &rules);
                Ok(Tokens::new(result))
            }

            // token_stream(source) → 解析为树形 TokenTree 结构
            "token_stream" => {
                if args.len() != 1 { return Err("token_stream requires 1 arg".to_string()); }
                let src = self.eval_expr(&args[0])?;
                let tree = TokenTree::parse_all(&src.tokens)
                    .map_err(|e| format!("token_stream: {}", e))?;
                Ok(Tokens::with_tree(src.tokens.clone(), tree))
            }

            // ── 自举关键内置函数 ──

            // take(tokens, n) — 取前 n 个 token
            "take" => {
                if args.len() != 2 { return Err("take requires 2 args: (tokens, n)".to_string()); }
                let val = self.eval_expr(&args[0])?;
                let n_val = self.eval_expr(&args[1])?;
                let n = parse_int_arg(&n_val)?;
                let end = n.min(val.len());
                Ok(Tokens::new(val.tokens[..end].to_vec()))
            }

            // drop(tokens, n) — 去掉前 n 个 token
            "drop_tokens" => {
                if args.len() != 2 { return Err("drop_tokens requires 2 args: (tokens, n)".to_string()); }
                let val = self.eval_expr(&args[0])?;
                let n_val = self.eval_expr(&args[1])?;
                let n = parse_int_arg(&n_val)?;
                if n >= val.len() {
                    Ok(Tokens::empty())
                } else {
                    Ok(Tokens::new(val.tokens[n..].to_vec()))
                }
            }

            // split_at(tokens, sep) — 按分隔符拆分 Token 序列
            "split_at" => {
                if args.len() != 2 { return Err("split_at requires 2 args: (tokens, sep)".to_string()); }
                let val = self.eval_expr(&args[0])?;
                let sep_val = self.eval_expr(&args[1])?;
                if sep_val.is_empty() {
                    return Err("split_at: separator cannot be empty".to_string());
                }
                let sep = &sep_val.tokens[0];
                let mut parts: Vec<Vec<Token>> = Vec::new();
                let mut current: Vec<Token> = Vec::new();
                for t in val.tokens {
                    if &t == sep {
                        if !current.is_empty() {
                            parts.push(std::mem::take(&mut current));
                        }
                    } else {
                        current.push(t);
                    }
                }
                if !current.is_empty() {
                    parts.push(current);
                }
                // 返回第一个分割结果（多部分场景由递归处理）
                if parts.is_empty() {
                    Ok(Tokens::empty())
                } else {
                    Ok(Tokens::new(parts.remove(0)))
                }
            }

            // join(tokens, sep) — 用分隔符连接 Tokens 中的 token 序列
            "join" => {
                if args.len() != 2 { return Err("join requires 2 args: (tokens, sep)".to_string()); }
                let val = self.eval_expr(&args[0])?;
                let sep_val = self.eval_expr(&args[1])?;
                if sep_val.is_empty() {
                    return Ok(val);
                }
                let sep_token = &sep_val.tokens[0];
                let mut result: Vec<Token> = Vec::new();
                for (idx, t) in val.tokens.iter().enumerate() {
                    if idx > 0 {
                        result.push(sep_token.clone());
                    }
                    result.push(t.clone());
                }
                Ok(Tokens::new(result))
            }

            // error(msg) — 编译期报错
            "error" => {
                if args.len() != 1 { return Err("error requires 1 arg: (message)".to_string()); }
                let msg = self.eval_expr(&args[0])?;
                Err(format!("macro error: {}", msg.to_string().trim_matches('"')))
            }

            // assert_eq(a, b) — 断言两个 Token 流相等
            "assert_eq" => {
                if args.len() != 2 { return Err("assert_eq requires 2 args: (a, b)".to_string()); }
                let a = self.eval_expr(&args[0])?;
                let b = self.eval_expr(&args[1])?;
                if a.tokens != b.tokens {
                    Err(format!("assertion failed: expected '{}', got '{}'",
                        b.to_string(), a.to_string()))
                } else {
                    Ok(Tokens::empty())
                }
            }

            // contains(tokens, pattern_token) — 检查是否包含某 token
            "contains" => {
                if args.len() != 2 { return Err("contains requires 2 args: (tokens, token)".to_string()); }
                let val = self.eval_expr(&args[0])?;
                let pat_val = self.eval_expr(&args[1])?;
                if pat_val.is_empty() {
                    return Ok(Tokens::new(vec![Token::False]));
                }
                let found = val.tokens.contains(&pat_val.tokens[0]);
                Ok(Tokens::new(vec![if found { Token::True } else { Token::False }]))
            }

            _ => Err(format!("unknown builtin function '{}'", func)),
        }
    }
}

fn is_builtin(name: &str) -> bool {
    matches!(name, "is_empty" | "len" | "first" | "rest" | "classify"
        | "to_string" | "set_context" | "get_context" | "assert_parent" | "inside_parent"
        | "quote" | "merge_tokens" | "token_count" | "is_empty_tokens"
        | "filter_tokens" | "remove_tokens" | "replace_tokens" | "token_stream"
        | "take" | "drop_tokens" | "split_at" | "join" | "error" | "assert_eq" | "contains")
}

/// 从 Tokens 中解析整数参数
fn parse_int_arg(val: &Tokens) -> Result<usize, String> {
    if val.is_empty() {
        return Ok(0);
    }
    match &val.tokens[0] {
        Token::IntLit(n) => Ok(*n as usize),
        _ => Err(format!("expected integer, got {:?}", val.tokens.first())),
    }
}

/// 判断 token 是否属于指定类别（用于 filter_tokens）
fn token_matches_kind(token: &Token, kind: &str) -> bool {
    match kind {
        "ident" => matches!(token, Token::Ident(_)),
        "literal" => matches!(token, Token::IntLit(_) | Token::FloatLit(_) | Token::StrLit(_)
            | Token::FStrLit(_) | Token::RawStrLit(_) | Token::True | Token::False),
        "keyword" => matches!(token,
            Token::Def | Token::Struct | Token::Enum | Token::Trait | Token::Impl | Token::Const
            | Token::If | Token::Elif | Token::Else | Token::Match | Token::Case | Token::Guard
            | Token::For | Token::In | Token::While | Token::Loop | Token::Break | Token::Continue
            | Token::Return | Token::With | Token::Defer | Token::Try | Token::Catch | Token::Finally
            | Token::Raise | Token::Raises | Token::Async | Token::Await
            | Token::Spawn | Token::Select | Token::Yield | Token::Mut | Token::Ref | Token::Owned
            | Token::Where | Token::Import | Token::From | Token::As | Token::Macro | Token::Comptime
            | Token::Self_ | Token::And | Token::Or | Token::Not | Token::Is
        ),
        "operator" => matches!(token,
            Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent
            | Token::StarStar | Token::Eq | Token::EqEq | Token::NotEq | Token::Lt | Token::Gt
            | Token::Le | Token::Ge | Token::PlusEq | Token::MinusEq
            | Token::StarEq | Token::SlashEq | Token::PercentEq | Token::Amp | Token::Pipe_
            | Token::Caret | Token::CaretOp | Token::CaretInfix | Token::Shl | Token::Shr
            | Token::AmpAmp | Token::PipePipe | Token::Arrow | Token::FatArrow
            | Token::Pipe | Token::BackPipe | Token::Question | Token::QuestionQuestion
            | Token::SafeNav | Token::Exclamation | Token::At | Token::Dollar
            | Token::BuildAssign | Token::BuildCall | Token::BuildGen
        ),
        "delimiter" => matches!(token,
            Token::LParen | Token::RParen | Token::LBrack | Token::RBrack
            | Token::LBrace | Token::RBrace
        ),
        _ => false,
    }
}

// ──────────────── 宏体 AST（受限 lz 语法） ────────────────

/// 宏体语句
#[derive(Debug, Clone)]
pub enum MacroStmt {
    Let {
        name: String,
        value: MacroExpr,
    },
    Expr(MacroExpr),
    If {
        cond: MacroExpr,
        then_body: Vec<MacroStmt>,
        else_body: Option<Vec<MacroStmt>>,
    },
    For {
        var: String,
        iter: MacroExpr,
        body: Vec<MacroStmt>,
    },
    Return(MacroExpr),
}

/// 宏体表达式
#[derive(Debug, Clone)]
pub enum MacroExpr {
    BacktickBlock {
        tokens: Vec<Token>,
        prefix: BacktickPrefix,
    },
    Ident(String),
    Call {
        func: String,
        args: Vec<MacroExpr>,
    },
    Binary {
        left: Box<MacroExpr>,
        op: BinaryOp,
        right: Box<MacroExpr>,
    },
    IfExpr {
        cond: Box<MacroExpr>,
        then_expr: Box<MacroExpr>,
        else_expr: Option<Box<MacroExpr>>,
    },
    IntLit(i64),
    StrLit(String),
    BoolLit(bool),
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Plus,
    // 未来可扩展其他操作符
}

// ──────────────── 单元测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_macro_body() {
        let mut interp = MacroInterpreter::new();
        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::BacktickBlock {
                tokens: vec![],
                prefix: BacktickPrefix::None,
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_identity_macro() {
        let mut interp = MacroInterpreter::new();
        let input = Tokens::new(vec![Token::IntLit(42)]);
        interp.bind_param("input".into(), input);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::BacktickBlock {
                tokens: vec![],
                prefix: BacktickPrefix::None,
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_f_backtick_interpolation() {
        let mut interp = MacroInterpreter::new();
        let input = Tokens::new(vec![Token::IntLit(42)]);
        interp.bind_param("input".into(), input);

        // f``` $input + 1 ```
        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::BacktickBlock {
                // 模拟 f``` $input + 1 ``` — 注意这里 $(expr) 需要特殊 token 模式
                // 在真实场景中，Dollar + LParen 会被词法分析为插值标记
                tokens: vec![
                    Token::Dollar, // $ 符号
                    Token::LParen,
                    Token::Ident("input".into()),
                    Token::RParen,
                    Token::Plus,
                    Token::IntLit(1),
                ],
                prefix: BacktickPrefix::F,
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        // $input → 42, 所以应该是 42 + 1
        assert_eq!(result.to_string(), "42+1");
    }

    #[test]
    fn test_r_backtick_raw() {
        let mut interp = MacroInterpreter::new();
        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::BacktickBlock {
                tokens: vec![
                    Token::At,
                    Token::Ident("some_macro".into()),
                    Token::Exclamation, // !
                    Token::LParen,
                    Token::Ident("x".into()),
                    Token::RParen,
                ],
                prefix: BacktickPrefix::R,
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        // r``` 原样返回
        assert!(result.to_string().contains("@some_macro!"));
    }

    #[test]
    fn test_let_binding() {
        let mut interp = MacroInterpreter::new();
        let body: Vec<MacroStmt> = vec![
            MacroStmt::Let {
                name: "x".into(),
                value: MacroExpr::BacktickBlock {
                    tokens: vec![Token::Ident("hello".into())],
                    prefix: BacktickPrefix::None,
                },
            },
            MacroStmt::Expr(MacroExpr::Ident("x".into())),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert_eq!(result.to_string(), "hello");
    }

    #[test]
    fn test_builtin_is_empty() {
        let mut interp = MacroInterpreter::new();
        // 绑定空 Tokens
        let empty = Tokens::empty();
        interp.bind_param("t".into(), empty);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "is_empty".into(),
                args: vec![MacroExpr::Ident("t".into())],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert_eq!(result.tokens, vec![Token::True]);
    }

    #[test]
    fn test_builtin_first_rest() {
        let mut interp = MacroInterpreter::new();
        let t = Tokens::new(vec![
            Token::Ident("a".into()),
            Token::Ident("b".into()),
        ]);
        interp.bind_param("t".into(), t);

        // first(t)
        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "first".into(),
                args: vec![MacroExpr::Ident("t".into())],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert_eq!(result.to_string(), "a");
    }

    // ── 新增 API 测试 ──

    #[test]
    fn test_builtin_quote() {
        let mut interp = MacroInterpreter::new();
        let input = Tokens::new(vec![Token::IntLit(42)]);
        interp.bind_param("x".into(), input);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "quote".into(),
                args: vec![MacroExpr::Ident("x".into())],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert_eq!(result.tokens, vec![Token::IntLit(42)]);
    }

    #[test]
    fn test_builtin_merge_tokens() {
        let mut interp = MacroInterpreter::new();
        let a = Tokens::new(vec![Token::Ident("foo".into())]);
        let b = Tokens::new(vec![Token::LParen, Token::RParen]);
        interp.bind_param("a".into(), a);
        interp.bind_param("b".into(), b);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "merge_tokens".into(),
                args: vec![
                    MacroExpr::Ident("a".into()),
                    MacroExpr::Ident("b".into()),
                ],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.to_string(), "foo()");
    }

    #[test]
    fn test_builtin_token_count() {
        let mut interp = MacroInterpreter::new();
        let t = Tokens::new(vec![Token::IntLit(1), Token::Plus, Token::IntLit(2)]);
        interp.bind_param("t".into(), t);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "token_count".into(),
                args: vec![MacroExpr::Ident("t".into())],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert_eq!(result.tokens, vec![Token::IntLit(3)]);
    }

    #[test]
    fn test_builtin_filter_tokens_idents() {
        let mut interp = MacroInterpreter::new();
        let t = Tokens::new(vec![
            Token::Def,
            Token::Ident("foo".into()),
            Token::LParen,
            Token::Ident("x".into()),
            Token::RParen,
        ]);
        interp.bind_param("t".into(), t);
        let kind = Tokens::new(vec![Token::StrLit("ident".into())]);
        interp.bind_param("k".into(), kind);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "filter_tokens".into(),
                args: vec![
                    MacroExpr::Ident("t".into()),
                    MacroExpr::Ident("k".into()),
                ],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        // 应该只保留 Ident("foo") 和 Ident("x")
        assert_eq!(result.len(), 2);
        assert_eq!(result.tokens[0], Token::Ident("foo".into()));
        assert_eq!(result.tokens[1], Token::Ident("x".into()));
    }

    #[test]
    fn test_builtin_remove_tokens() {
        let mut interp = MacroInterpreter::new();
        let src = Tokens::new(vec![
            Token::Ident("debug".into()),
            Token::LParen,
            Token::StrLit("msg".into()),
            Token::RParen,
            Token::Semicolon,
            Token::Ident("real".into()),
            Token::LParen,
            Token::RParen,
        ]);
        let pattern = Tokens::new(vec![
            Token::Ident("debug".into()),
            Token::LParen,
            Token::Underscore,
            Token::RParen,
        ]);
        interp.bind_param("s".into(), src);
        interp.bind_param("p".into(), pattern);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "remove_tokens".into(),
                args: vec![
                    MacroExpr::Ident("s".into()),
                    MacroExpr::Ident("p".into()),
                ],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        // debug(...) 被移除，剩下 ; real()
        assert_eq!(result.len(), 4);
        assert_eq!(result.tokens[0], Token::Semicolon);
        assert_eq!(result.tokens[1], Token::Ident("real".into()));
    }

    #[test]
    fn test_builtin_replace_tokens() {
        let mut interp = MacroInterpreter::new();
        let src = Tokens::new(vec![
            Token::Ident("old".into()),
            Token::Dot,
            Token::Ident("field".into()),
        ]);
        let rules = Tokens::new(vec![
            Token::Ident("old".into()),
            Token::FatArrow,
            Token::Ident("new".into()),
        ]);
        interp.bind_param("s".into(), src);
        interp.bind_param("r".into(), rules);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "replace_tokens".into(),
                args: vec![
                    MacroExpr::Ident("s".into()),
                    MacroExpr::Ident("r".into()),
                ],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert_eq!(result.tokens[0], Token::Ident("new".into()));
        assert_eq!(result.tokens[1], Token::Dot);
        assert_eq!(result.tokens[2], Token::Ident("field".into()));
    }

    #[test]
    fn test_builtin_token_stream() {
        let mut interp = MacroInterpreter::new();
        let t = Tokens::new(vec![
            Token::Ident("foo".into()),
            Token::LParen,
            Token::IntLit(42),
            Token::RParen,
        ]);
        interp.bind_param("t".into(), t);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "token_stream".into(),
                args: vec![MacroExpr::Ident("t".into())],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        // token_stream 应该填充 tree
        assert!(result.tree.is_some());
        let tree = result.tree.unwrap();
        assert_eq!(tree.len(), 2);
        // 验证第一个是 Atom("foo")
        assert_eq!(tree[0], TokenTree::Atom(Token::Ident("foo".into())));
        // 验证第二个是 Group(Paren, [IntLit(42)])
        assert!(matches!(&tree[1], TokenTree::Group(Delimiter::Paren, _)));
        if let TokenTree::Group(Delimiter::Paren, children) = &tree[1] {
            assert_eq!(children.len(), 1);
            assert_eq!(children[0], TokenTree::Atom(Token::IntLit(42)));
        }
    }

    #[test]
    fn test_builtin_token_stream_nested() {
        let mut interp = MacroInterpreter::new();
        let t = Tokens::new(vec![
            Token::Ident("bar".into()),
            Token::LParen,
            Token::LParen,
            Token::IntLit(1),
            Token::RParen,
            Token::Comma,
            Token::LBrace,
            Token::Ident("k".into()),
            Token::RBrace,
            Token::RParen,
        ]);
        interp.bind_param("t".into(), t);

        let body: Vec<MacroStmt> = vec![
            MacroStmt::Expr(MacroExpr::Call {
                func: "token_stream".into(),
                args: vec![MacroExpr::Ident("t".into())],
            }),
        ];
        let result = interp.execute_stmts(&body).unwrap();
        assert!(result.tree.is_some());
        let tree = result.tree.unwrap();
        assert_eq!(tree.len(), 2);
        // bar + ( (1), {k} )
        if let TokenTree::Group(Delimiter::Paren, children) = &tree[1] {
            // children: (1) , {k}
            assert!(children.len() >= 2);
        } else {
            panic!("expected Group");
        }
    }
}

/// 合并相邻的 StrLit token：`quote("a" + name + "b")` 参数插值产生
/// [StrLit("a"), StrLit("World"), StrLit("b")]，需合并为单个 StrLit
/// （否则 parser 解析为分散表达式语句而非字符串拼接）
fn merge_str_lits(tokens: Vec<Token>) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    for t in tokens {
        match t {
            Token::StrLit(s) => {
                if let Some(Token::StrLit(last)) = out.last_mut() {
                    last.push_str(&s);
                } else {
                    out.push(Token::StrLit(s));
                }
            }
            other => out.push(other),
        }
    }
    out
}
