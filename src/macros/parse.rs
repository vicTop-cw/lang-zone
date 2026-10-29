// Lang-Zong 编译器 — macros/parse.rs
// 宏定义解析器：从 Token 流提取 macro / template 定义并解析宏表达式
// （move-only 自 expand.rs 原样搬出；expand.rs 保留转发，保持 crate::macros::expand::<fn> 外部路径不变）

use crate::lexer::Token;
use crate::macros::interp::{BinaryOp, MacroExpr, MacroStmt};

use super::expand::{MacroDef, MacroRegistry, TemplateDef, TemplateRegistry};

// ──────────────── 宏定义解析器（从 Token 流提取宏定义） ────────────────

/// 检测 token 流是否含 `#!bin macro` 宏模块声明（08 §2.1）：
/// 独立声明行的 Token::Macro（前一个 token 是文件开头或 Newline，后跟 Newline/Eof）。
/// 注意：`import macro X` 中的 macro 前有 Import/From，不会被误判。
/// pub：main.rs 在宏展开前用原始 token 流检测宏模块（展开会消费声明）
pub fn has_bin_macro_declaration(tokens: &[Token]) -> bool {
    let len = tokens.len();
    for i in 0..len {
        if tokens[i] == Token::Macro {
            // 前一个 token：文件开头或 Newline（独立声明行）
            let prev_ok = i == 0 || matches!(&tokens[i - 1], Token::Newline);
            // 后一个 token：Newline 或 Eof（声明行结束，无宏名）
            let next_ok = (i + 1 < len && matches!(&tokens[i + 1], Token::Newline)) || i + 1 >= len;
            if prev_ok && next_ok {
                return true;
            }
        }
    }
    false
}

/// 从 Token 流中预提取宏定义，构建 MacroRegistry。
/// 这是展开前的第一遍扫描。
pub fn extract_macro_defs(tokens: &[Token]) -> Result<(MacroRegistry, Vec<usize>), String> {
    // 宏模块限定：定义 macro 的文件必须含 `#!bin macro` 声明（08 §2.1）
    let has_decl = has_bin_macro_declaration(tokens);
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

            // `#!bin macro` 宏模块声明：lexer 把整行产生单个 Token::Macro，
            // 其后紧跟 Newline 或 Eof（跳过空白前检查——skip_blanks 会吞掉
            // Newline）。消费该 token，避免残留到 Parser（A3 lexer 识别
            // #!bin macro 后；macro_demo.lz 占位文件仅声明无宏定义）。
            // import macro X 的 macro 后跟 Ident 模块名，不会进入此分支。
            if i < len && (tokens[i] == Token::Newline || tokens[i] == Token::Eof) {
                consumed_ranges.push(start);
                consumed_ranges.push(i);
                i += 1;
                continue;
            }

            // 跳过空白和换行
            i = skip_blanks(tokens, i, len);

            // 宏名
            let name = match tokens.get(i) {
                Some(Token::Ident(n)) => {
                    i += 1;
                    n.clone()
                }
                _ => {
                    // 非宏定义 → 跳过继续
                    i += 1;
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
            i = skip_to(
                tokens,
                i,
                len,
                &Token::LParen,
                &format!("expected '(' after macro name '{}'", name),
            )?;
            i += 1; // 跳过 (

            // 解析参数: name: Tokens 或 name: Tokens, name2: Tokens
            let mut param_names = Vec::new();
            let mut is_attr = false;
            let param_loop_max = 100;
            let mut param_iter = 0;
            loop {
                param_iter += 1;
                if param_iter > param_loop_max {
                    return Err(format!(
                        "parameter parsing exceeded limit in macro '{}'",
                        name
                    ));
                }
                i = skip_blanks(tokens, i, len);
                match tokens.get(i) {
                    Some(Token::RParen) => {
                        i += 1;
                        break;
                    }
                    Some(Token::Ident(pname)) => {
                        param_names.push(pname.clone());
                        i += 1;
                        // 变参标记 `items..: Tokens`（08 §3.3 示例）：参数名后跟 `..`
                        // lexer 把 `..` 合并为 Token::DotDot（`...` 为 DotDotDot）
                        i = skip_blanks(tokens, i, len);
                        if matches!(&tokens[i], Token::DotDot | Token::DotDotDot) {
                            i += 1;
                        }
                        // 跳过 : 类型注解并校验——macro 签名固定（08 §3.1）：
                        // 全部参数类型必须为 Tokens（纯 Token 级转换，无自由类型参数）。
                        // 省略 `: Tokens` 注解视为默认 Tokens；显式非 Tokens 类型报错
                        // （自由参数（int/str）应改用 template，§四）
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Colon {
                            i += 1;
                            i = skip_blanks(tokens, i, len);
                            if !matches!(&tokens[i], Token::Ident(s) if s == "Tokens") {
                                return Err(format!(
                                    "macro '{}' 参数 `{}` 类型必须为 Tokens（签名固定 Tokens -> Tokens；自由参数（int/str 等）请改用 template）",
                                    name, pname
                                ));
                            }
                            i += 1;
                        }
                        // 检查逗号或右括号
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Comma {
                            i += 1;
                            is_attr = param_names.len() >= 2;
                        }
                        if i < len && tokens[i] == Token::RParen {
                            i += 1;
                            break;
                        }
                    }
                    _ => {
                        return Err(format!(
                            "expected parameter name in macro '{}' at token {}",
                            name, i
                        ))
                    }
                }
            }

            // 签名校验：macro 参数与返回类型必须为 Tokens（08 §3.1「签名固定 Tokens -> Tokens」）
            // 参数类型在循环中已跳过 Ident("Tokens")，此处校验返回类型
            i = skip_to(
                tokens,
                i,
                len,
                &Token::Arrow,
                &format!("expected '->' in macro '{}'", name),
            )?;
            i += 1;
            i = skip_blanks(tokens, i, len);
            if !matches!(&tokens[i], Token::Ident(s) if s == "Tokens") {
                return Err(format!(
                    "macro '{}' 返回类型必须为 Tokens（签名固定 Tokens -> Tokens）",
                    name
                ));
            }
            i += 1;

            // 跳过 =
            i = skip_to(
                tokens,
                i,
                len,
                &Token::Eq,
                &format!("expected '=' in macro '{}'", name),
            )?;
            i += 1;

            // 收集宏体
            let (body_tokens, decl_end) = collect_indented_block_with_end(tokens, i)?;
            let body = parse_macro_body(&body_tokens)?;
            i = decl_end;

            // 宏模块限定：定义 macro 的文件必须含 `#!bin macro` 声明（08 §2.1）
            if !has_decl {
                return Err(format!(
                    "macro '{}' 定义在非宏模块文件中（缺少 '#!bin macro' 首行声明，规范 08 §2.1）",
                    name
                ));
            }
            registry.register(MacroDef {
                name,
                is_attr,
                param_names,
                body,
            });
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

/// 预扫描 Token 流，提取所有 `template` 定义并注册到 TemplateRegistry。
/// 与 extract_macro_defs 的区别：template 参数签名自由（str/int/Tokens/泛型），
/// 需记录参数类型；返回类型必须为 Tokens。返回 (registry, 模板定义 token 范围)。
pub fn extract_template_defs(tokens: &[Token]) -> Result<(TemplateRegistry, Vec<usize>), String> {
    // 宏模块限定：定义 template 的文件必须含 `#!bin macro` 声明（08 §2.1）
    let has_decl = has_bin_macro_declaration(tokens);
    let mut registry = TemplateRegistry::new();
    let mut consumed_ranges: Vec<usize> = Vec::new();
    let mut i = 0;
    let len = tokens.len();
    let max_scan = len + 1024;

    let mut iter_count = 0;
    while i < len && iter_count < max_scan {
        iter_count += 1;
        if tokens[i] == Token::Template {
            let start = i;
            i += 1; // 跳过 template

            i = skip_blanks(tokens, i, len);

            // 模板名（可能紧跟 !，如 `template make!` — 名字本身不含 !）
            let name = match tokens.get(i) {
                Some(Token::Ident(n)) => {
                    i += 1;
                    n.clone()
                }
                _ => continue,
            };
            // 跳过名字后的 !（`template make!<T>` 形式）
            i = skip_blanks(tokens, i, len);
            if i < len && tokens[i] == Token::Exclamation {
                i += 1;
            }
            // 跳过可选泛型参数 `<T>`（模板泛型）
            i = skip_blanks(tokens, i, len);
            if i < len && tokens[i] == Token::Lt {
                let mut depth = 1;
                i += 1;
                while i < len && depth > 0 {
                    if tokens[i] == Token::Lt {
                        depth += 1;
                    } else if tokens[i] == Token::Gt {
                        depth -= 1;
                    }
                    i += 1;
                }
            }

            // 解析参数: name: Type, ...（类型自由）
            let mut param_names: Vec<String> = Vec::new();
            let mut param_types: Vec<String> = Vec::new();
            i = skip_blanks(tokens, i, len);
            if tokens.get(i) != Some(&Token::LParen) {
                continue; // 非模板定义（如 import 相关），跳过
            }
            i += 1; // 跳过 (
            let param_loop_max = 100;
            let mut param_iter = 0;
            loop {
                param_iter += 1;
                if param_iter > param_loop_max {
                    return Err(format!(
                        "parameter parsing exceeded limit in template '{}'",
                        name
                    ));
                }
                i = skip_blanks(tokens, i, len);
                match tokens.get(i) {
                    Some(Token::RParen) => {
                        i += 1;
                        break;
                    }
                    Some(Token::Ident(pname)) => {
                        param_names.push(pname.clone());
                        i += 1;
                        // 跳过 : Type
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Colon {
                            i += 1;
                        }
                        i = skip_blanks(tokens, i, len);
                        // 类型名：Ident（含 Tokens/str/int）或点路径（module.Tokens）
                        if let Some(Token::Ident(tn)) = tokens.get(i) {
                            let mut ty = tn.clone();
                            i += 1;
                            while i < len && tokens[i] == Token::Dot {
                                i += 1;
                                if let Some(Token::Ident(seg)) = tokens.get(i) {
                                    ty = format!("{}.{}", ty, seg);
                                    i += 1;
                                }
                            }
                            param_types.push(ty);
                        } else {
                            param_types.push(String::new());
                        }
                        // 变参标记 `..`（fields..: Tokens）
                        i = skip_blanks(tokens, i, len);
                        if i + 1 < len && tokens[i] == Token::Dot && tokens[i + 1] == Token::Dot {
                            i += 2;
                        }
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Comma {
                            i += 1;
                        }
                        if i < len && tokens[i] == Token::RParen {
                            i += 1;
                            break;
                        }
                    }
                    _ => {
                        return Err(format!(
                            "expected parameter name in template '{}' at token {}",
                            name, i
                        ))
                    }
                }
            }

            // 跳过 -> Tokens（返回类型必须为 Tokens，08 §四「template 的产物一定是
            // Token 流」——参数签名自由（str/int/Tokens/泛型），但返回必须 Tokens）
            i = skip_to(
                tokens,
                i,
                len,
                &Token::Arrow,
                &format!("expected '->' in template '{}'", name),
            )?;
            i += 1;
            i = skip_blanks(tokens, i, len);
            if !matches!(&tokens[i], Token::Ident(s) if s == "Tokens") {
                return Err(format!(
                    "template '{}' 返回类型必须为 Tokens（参数签名自由，但产物一定是 Token 流，规范 08 §四）",
                    name
                ));
            }
            i += 1;

            // 跳过 =
            i = skip_to(
                tokens,
                i,
                len,
                &Token::Eq,
                &format!("expected '=' in template '{}'", name),
            )?;
            i += 1;

            // 收集模板体
            let (body_tokens, decl_end) = collect_indented_block_with_end(tokens, i)?;
            let body = parse_macro_body(&body_tokens)?;
            i = decl_end;

            // 宏模块限定：定义 template 的文件必须含 `#!bin macro` 声明（08 §2.1）
            if !has_decl {
                return Err(format!(
                    "template '{}' 定义在非宏模块文件中（缺少 '#!bin macro' 首行声明，规范 08 §2.1）",
                    name
                ));
            }
            registry.register(TemplateDef {
                name,
                param_names,
                param_types,
                body,
            });
            consumed_ranges.push(start);
            consumed_ranges.push(i);
        } else {
            i += 1;
        }
    }

    if iter_count >= max_scan {
        return Err("template definition extraction exceeded scan limit".to_string());
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
fn skip_to(
    tokens: &[Token],
    mut i: usize,
    len: usize,
    target: &Token,
    err_msg: &str,
) -> Result<usize, String> {
    while i < len && &tokens[i] != target {
        if tokens[i] == Token::Newline {
            i += 1;
            continue;
        }
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
fn collect_indented_block_with_end(
    tokens: &[Token],
    start: usize,
) -> Result<(Vec<Token>, usize), String> {
    let mut result = Vec::new();
    let mut i = start;
    let mut indent_depth = 0;
    let mut first_indent = false;

    // 跳过 body 起始的 Newline（`=` 后换行，多行缩进 body 的前导换行）——
    // 否则 `Newline if !first_indent` 会把多行 body 的起始换行误当单行结束，
    // 导致 body 未收集、宏定义残留 expand_input
    while i < tokens.len() && tokens[i] == Token::Newline {
        i += 1;
    }

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
            Token::Newline if !first_indent => {
                // 单行 body（`macro id(...) = input`，`=` 后无 Indent）：
                // 收集到行尾 Newline 即结束，否则把下一个 macro/template
                // 定义吃进 body（"unexpected token Macro in macro body"）
                return Ok((result, i));
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
        if i >= len {
            break;
        }

        // 检测反引号块 ``` ... ```（前缀形式 f``` / r```：Ident("f"/"r") 在反引号前）
        // token 顺序：`f``` → Ident("f") Backtick Backtick Backtick；纯 ``` → Backtick...
        let backtick_prefix = if tokens[i] == Token::Backtick {
            crate::macros::group::BacktickPrefix::None
        } else if let Token::Ident(s) = &tokens[i] {
            if (s == "f" || s == "r") && i + 1 < len && tokens[i + 1] == Token::Backtick {
                if s == "f" {
                    crate::macros::group::BacktickPrefix::F
                } else {
                    crate::macros::group::BacktickPrefix::R
                }
            } else {
                crate::macros::group::BacktickPrefix::None
            }
        } else {
            crate::macros::group::BacktickPrefix::None
        };
        let is_backtick_block = tokens[i] == Token::Backtick
            || (backtick_prefix != crate::macros::group::BacktickPrefix::None
                && i + 1 < len
                && tokens[i + 1] == Token::Backtick);
        if is_backtick_block {
            let block_start = if tokens[i] == Token::Backtick {
                i
            } else {
                i + 1 // 跳过 Ident("f"/"r") 前缀，反引号块从下一个 Backtick 开始
            };
            let (block_tokens, next_i) = collect_backtick_block(tokens, block_start)?;
            stmts.push(MacroStmt::Expr(MacroExpr::BacktickBlock {
                tokens: block_tokens,
                prefix: backtick_prefix,
            }));
            i = next_i;
            continue;
        }

        match &tokens[i] {
            Token::Let => {
                // let name = expr
                i += 1;
                while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) {
                    i += 1;
                }
                let name = match &tokens[i] {
                    Token::Ident(n) => {
                        i += 1;
                        n.clone()
                    }
                    _ => return Err("expected variable name after let".to_string()),
                };
                while i < len && tokens[i] != Token::Eq {
                    if matches!(&tokens[i], Token::Newline | Token::Indent | Token::Colon) {
                        i += 1;
                        continue;
                    }
                    if matches!(&tokens[i], Token::Ident(s) if s == "Tokens") {
                        i += 1;
                        continue;
                    }
                    break;
                }
                if i < len && tokens[i] == Token::Eq {
                    i += 1;
                }
                let (value_expr, next_i) = parse_macro_expr(tokens, i)?;
                stmts.push(MacroStmt::Let {
                    name,
                    value: value_expr,
                });
                i = next_i;
            }
            Token::If => {
                // if cond: body (可选 else: body)
                i += 1;
                let (cond, next_i) = parse_macro_expr(tokens, i)?;
                i = next_i;
                while i < len && tokens[i] != Token::Colon {
                    i += 1;
                }
                if i < len {
                    i += 1;
                } // 跳过 :

                // 收集 then_body（缩进块内）
                let (then_body, next_i) = collect_stmt_block(tokens, i)?;
                i = next_i;

                // 检查 else
                let mut else_body = None;
                while i < len && matches!(&tokens[i], Token::Newline | Token::Dedent) {
                    i += 1;
                }
                if i < len && tokens[i] == Token::Else {
                    i += 1;
                    while i < len && tokens[i] != Token::Colon {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    let (else_stmts, next_i) = collect_stmt_block(tokens, i)?;
                    else_body = Some(else_stmts);
                    i = next_i;
                }
                stmts.push(MacroStmt::If {
                    cond,
                    then_body,
                    else_body,
                });
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
                while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) {
                    i += 1;
                }
                let var = match &tokens[i] {
                    Token::Ident(n) => {
                        i += 1;
                        n.clone()
                    }
                    _ => return Err("expected loop variable after for".to_string()),
                };
                while i < len && tokens[i] != Token::In {
                    if matches!(&tokens[i], Token::Newline | Token::Indent) {
                        i += 1;
                        continue;
                    }
                    return Err("expected 'in' in for loop".to_string());
                }
                i += 1; // skip 'in'
                let (iter_expr, next_i) = parse_macro_expr(tokens, i)?;
                i = next_i;
                while i < len && tokens[i] != Token::Colon {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                let (body, next_i) = collect_stmt_block(tokens, i)?;
                stmts.push(MacroStmt::For {
                    var,
                    iter: iter_expr,
                    body,
                });
                i = next_i;
            }
            Token::Ident(name) => {
                // 可能是函数调用或标识符表达式
                let mut next_i = i + 1;
                while next_i < len && matches!(&tokens[next_i], Token::Newline | Token::Indent) {
                    next_i += 1;
                }
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
                    while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) {
                        i += 1;
                    }
                    if i < len && tokens[i] == Token::Plus && stmts.len() > 0 {
                        // 处理二元表达式: expr + expr（plus 专用）
                        let left = match &stmts[stmts.len() - 1] {
                            MacroStmt::Expr(e) => e.clone(),
                            _ => break,
                        };
                        stmts.pop();
                        i += 1; // 跳过 +
                        while i < len && matches!(&tokens[i], Token::Newline | Token::Indent) {
                            i += 1;
                        }
                        let (right, next_i) = parse_macro_expr(tokens, i)?;
                        stmts.push(MacroStmt::Expr(MacroExpr::Binary {
                            left: Box::new(left),
                            op: crate::macros::interp::BinaryOp::Plus,
                            right: Box::new(right),
                        }));
                        i = next_i;
                    }
                }
            }
            _ => {
                return Err(format!(
                    "unexpected token {:?} in macro body at position {}",
                    tokens[i], i
                ));
            }
        }
    }
    Ok(stmts)
}

/// 收集反引号块 ``` ... ```
fn collect_backtick_block(tokens: &[Token], start: usize) -> Result<(Vec<Token>, usize), String> {
    // 反引号块格式: Backtick(×1-3), Newline, tokens..., Newline, Backtick(×1-3)
    // 三反引号 ``` 由 lexer 拆成 3 个 Backtick token——需识别开头连续反引号数，
    // 内容收集到结尾**相同数量**的连续反引号处（否则在第一个反引号处截断，
    // 剩余反引号残留到流中导致解析错误）
    let mut open_count = 0;
    let mut i = start;
    while i < tokens.len() && tokens[i] == Token::Backtick {
        open_count += 1;
        i += 1;
    }
    if open_count == 0 {
        return Err("expected backtick block".to_string());
    }
    // 跳过开头 Newline/Indent
    while i < tokens.len() && matches!(&tokens[i], Token::Newline | Token::Indent) {
        i += 1;
    }
    let content_start = i;
    // 找到结尾连续的 open_count 个 Backtick
    while i < tokens.len() {
        if tokens[i] == Token::Backtick {
            let mut close_count = 0;
            while i < tokens.len() && tokens[i] == Token::Backtick {
                close_count += 1;
                i += 1;
            }
            if close_count >= open_count {
                let result = tokens[content_start..i - close_count].to_vec();
                return Ok((result, i));
            }
        } else {
            i += 1;
        }
    }
    Err("unclosed backtick block".to_string())
}

/// 收集语句块（缩进块内的一组语句）
fn collect_stmt_block(tokens: &[Token], start: usize) -> Result<(Vec<MacroStmt>, usize), String> {
    let mut i = start;
    while i < tokens.len() && matches!(&tokens[i], Token::Newline | Token::Indent) {
        i += 1;
    }

    let mut block_tokens = Vec::new();
    let mut depth = 1; // 当前在缩进块内
    while i < tokens.len() && depth > 0 {
        match &tokens[i] {
            Token::Indent => {
                depth += 1;
                block_tokens.push(tokens[i].clone());
            }
            Token::Dedent => {
                depth -= 1;
                if depth > 0 {
                    block_tokens.push(tokens[i].clone());
                }
            }
            _ => {
                block_tokens.push(tokens[i].clone());
            }
        }
        i += 1;
    }
    let stmts = parse_macro_body(&block_tokens)?;
    Ok((stmts, i))
}
/// 解析宏表达式（分层递归下降，优先级从低到高）
/// 优先级：|| < && < 比较(== != < > <= >=) < 加减(+ -) < 乘除模(* / %) < 一元负号 < 原子
/// 中缀运算符统一映射到 BinaryOp（`+` 保持 BinaryOp::Plus，运行时按类型分派字符串拼接
/// 与算术加法）。此前的 parse_macro_expr_or 与主函数逻辑重复、优先级语义混乱，已废弃。
pub fn parse_macro_expr(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    let (mut expr, mut i) = parse_macro_and(tokens, start)?;
    // 逻辑或：||（优先级最低，左结合）
    while i < tokens.len() && tokens[i] == Token::PipePipe {
        i += 1;
        let (right, ni) = parse_macro_and(tokens, i)?;
        expr = MacroExpr::Binary {
            left: Box::new(expr),
            op: BinaryOp::Or,
            right: Box::new(right),
        };
        i = ni;
    }
    Ok((expr, i))
}

/// 解析逻辑与表达式：&&
fn parse_macro_and(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    let (mut expr, mut i) = parse_macro_cmp(tokens, start)?;
    while i < tokens.len() && tokens[i] == Token::AmpAmp {
        i += 1;
        let (right, ni) = parse_macro_cmp(tokens, i)?;
        expr = MacroExpr::Binary {
            left: Box::new(expr),
            op: BinaryOp::And,
            right: Box::new(right),
        };
        i = ni;
    }
    Ok((expr, i))
}

/// 解析比较表达式：== != < > <= >=
fn parse_macro_cmp(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    let (mut expr, mut i) = parse_macro_additive(tokens, start)?;
    while i < tokens.len()
        && matches!(
            &tokens[i],
            Token::EqEq | Token::NotEq | Token::Lt | Token::Gt | Token::Le | Token::Ge
        )
    {
        let op_token = tokens[i].clone();
        i += 1;
        let (right, ni) = parse_macro_additive(tokens, i)?;
        let op = match op_token {
            Token::EqEq => BinaryOp::EqEq,
            Token::NotEq => BinaryOp::NotEq,
            Token::Lt => BinaryOp::Lt,
            Token::Gt => BinaryOp::Gt,
            Token::Le => BinaryOp::Le,
            Token::Ge => BinaryOp::Ge,
            _ => unreachable!(),
        };
        expr = MacroExpr::Binary {
            left: Box::new(expr),
            op,
            right: Box::new(right),
        };
        i = ni;
    }
    Ok((expr, i))
}

/// 解析加减表达式：+ -
fn parse_macro_additive(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    let (mut expr, mut i) = parse_macro_multiplicative(tokens, start)?;
    while i < tokens.len() && matches!(&tokens[i], Token::Plus | Token::Minus) {
        let op_token = tokens[i].clone();
        i += 1;
        let (right, ni) = parse_macro_multiplicative(tokens, i)?;
        let op = match op_token {
            Token::Plus => BinaryOp::Plus,
            Token::Minus => BinaryOp::Minus,
            _ => unreachable!(),
        };
        expr = MacroExpr::Binary {
            left: Box::new(expr),
            op,
            right: Box::new(right),
        };
        i = ni;
    }
    Ok((expr, i))
}

/// 解析乘除模表达式：* / %
fn parse_macro_multiplicative(
    tokens: &[Token],
    start: usize,
) -> Result<(MacroExpr, usize), String> {
    let (mut expr, mut i) = parse_macro_unary(tokens, start)?;
    while i < tokens.len() && matches!(&tokens[i], Token::Star | Token::Slash | Token::Percent) {
        let op_token = tokens[i].clone();
        i += 1;
        let (right, ni) = parse_macro_unary(tokens, i)?;
        let op = match op_token {
            Token::Star => BinaryOp::Star,
            Token::Slash => BinaryOp::Slash,
            Token::Percent => BinaryOp::Percent,
            _ => unreachable!(),
        };
        expr = MacroExpr::Binary {
            left: Box::new(expr),
            op,
            right: Box::new(right),
        };
        i = ni;
    }
    Ok((expr, i))
}

/// 解析一元表达式：-expr（右结合）
fn parse_macro_unary(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    let mut i = start;
    while i < tokens.len() && matches!(&tokens[i], Token::Newline | Token::Indent) {
        i += 1;
    }
    if i < tokens.len() && tokens[i] == Token::Minus {
        let (expr, ni) = parse_macro_unary(tokens, i + 1)?;
        Ok((MacroExpr::UnaryMinus(Box::new(expr)), ni))
    } else {
        parse_macro_primary_inner(tokens, i)
    }
}

/// 解析单个 primary 宏表达式（无二元运算符）
fn parse_macro_primary_inner(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    if start >= tokens.len() {
        return Ok((MacroExpr::IntLit(0), start));
    }
    let mut i = start;
    while i < tokens.len() && matches!(&tokens[i], Token::Newline | Token::Indent) {
        i += 1;
    }

    match &tokens[i] {
        Token::Ident(name) => {
            // 可能是函数调用或标识符
            let mut next_i = i + 1;
            while next_i < tokens.len() && matches!(&tokens[next_i], Token::Newline | Token::Indent) {
                next_i += 1;
            }
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
            while j < tokens.len() && matches!(&tokens[j], Token::Newline | Token::Indent) {
                j += 1;
            }
            let (cond, nj) = parse_macro_expr(tokens, j)?;
            j = nj;
            while j < tokens.len() && tokens[j] != Token::Colon {
                j += 1;
            }
            if j < tokens.len() {
                j += 1;
            }
            let (then_expr, nj) = parse_macro_expr(tokens, j)?;
            j = nj;
            let mut else_expr = None;
            while j < tokens.len() && matches!(&tokens[j], Token::Newline | Token::Dedent) {
                j += 1;
            }
            if j < tokens.len() && tokens[j] == Token::Else {
                j += 1;
                while j < tokens.len() && tokens[j] != Token::Colon {
                    j += 1;
                }
                if j < tokens.len() {
                    j += 1;
                }
                let (else_e, nj) = parse_macro_expr(tokens, j)?;
                else_expr = Some(Box::new(else_e));
                j = nj;
            }
            Ok((
                MacroExpr::IfExpr {
                    cond: Box::new(cond),
                    then_expr: Box::new(then_expr),
                    else_expr,
                },
                j,
            ))
        }
        Token::IntLit(n) => Ok((MacroExpr::IntLit(*n), i + 1)),
        Token::StrLit(s) => Ok((MacroExpr::StrLit(s.clone()), i + 1)),
        Token::True => Ok((MacroExpr::BoolLit(true), i + 1)),
        Token::False => Ok((MacroExpr::BoolLit(false), i + 1)),
        Token::Minus => {
            // 一元负号：-expr
            let (expr, ni) = parse_macro_primary_inner(tokens, i + 1)?;
            Ok((MacroExpr::UnaryMinus(Box::new(expr)), ni))
        }
        Token::FloatLit(n) => Ok((MacroExpr::FloatLit(*n), i + 1)),
        Token::Backtick => {
            let (block_tokens, next_i) = collect_backtick_block(tokens, i)?;
            Ok((
                MacroExpr::BacktickBlock {
                    tokens: block_tokens,
                    prefix: crate::macros::group::BacktickPrefix::None,
                },
                next_i,
            ))
        }
        _ => {
            return Err(format!(
                "unexpected token {:?} in macro expression at position {}",
                tokens[i], i
            ));
        }
    }
}

pub(super) fn tokens_to_string(tokens: &[Token]) -> String {
    let mut s = String::new();
    for t in tokens {
        match t {
            Token::Ident(name) => {
                s.push_str(name);
                s.push(' ');
            }
            Token::StrLit(val)
            | Token::RawStrLit(val)
            | Token::FStrLit(val)
            | Token::TripleStrLit(val) => s.push_str(val),
            Token::IntLit(n) => s.push_str(&n.to_string()),
            Token::Int128Lit(n) => s.push_str(&n.to_string()),
            Token::BigIntLit(n) => s.push_str(n),
            Token::FloatLit(n) => s.push_str(&n.to_string()),
            Token::True => s.push_str("true"),
            Token::False => s.push_str("false"),
            Token::Newline => s.push('\n'),
            Token::Indent => s.push_str("    "),
            Token::Comma => s.push_str(", "),
            Token::Colon => s.push_str(": "),
            Token::LParen => s.push('('),
            Token::RParen => s.push(')'),
            Token::LBrack => s.push('['),
            Token::RBrack => s.push(']'),
            Token::LBrace => s.push('{'),
            Token::RBrace => s.push('}'),
            Token::Eq => s.push_str(" = "),
            Token::Plus => s.push_str(" + "),
            Token::Minus => s.push_str(" - "),
            Token::Star => s.push_str(" * "),
            Token::Slash => s.push_str(" / "),
            _ => {}
        }
    }
    s
}

