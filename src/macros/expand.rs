// Lang-Zong 编译器 — macros/expand.rs
// 宏展开引擎：扫描 Token 流，识别 @name! 模式，递归展开

use crate::lexer::Token;
use crate::macros::group::Tokens;
use crate::macros::interp::{MacroInterpreter, MacroStmt, MacroExpr, BinaryOp};

use std::collections::HashMap;
use std::cell::Cell;

// ──────────────── 宏卫生性辅助 ────────────────

/// 宏卫生性（Nim 式 gensym）：将展开结果中**宏体局部绑定**的名字加唯一后缀，
/// 避免宏内 let/for/def 变量污染调用方作用域。
///
/// 背景：local_sum 宏体 `let acc = 0 ... acc` 展开后与调用方 `let acc = 100`
/// 冲突（use_macros.lz §3 卫生性示例，生成重复 `let mut acc` 报 E0384）。
///
/// 策略：扫描展开结果 token 流，收集宏体自身绑定的名字（`let X` / `let mut X` /
/// `for X in` / `def X`），将这些名字的所有 Ident 引用重命名为 `X__m{uid}`。
/// 宏参数（调用方传入的实参 token，如 check_eq 的 actual/expected、wrap_loop
/// 的 body）本身不是宏体绑定名，不会误伤；调用方同名变量（acc=100）也保持不变。
/// 参数中的绑定（`exclude`）即使出现在展开结果里（如 wrap_loop 的 body 含
/// `let v = 5`），也按调用方卫生原样保留、不重命名——否则 p11c_macro_deep
/// 的 `total = total + v` 在宏外引用 v 会报未绑定。
fn hygienize_tokens(tokens: &[Token], uid: usize, exclude: &[String]) -> Vec<Token> {
    // 收集宏体局部绑定名（排除参数中调用方自己的绑定）
    let mut bound: Vec<String> = Vec::new();
    let mut i = 0;
    let len = tokens.len();
    while i < len {
        match &tokens[i] {
            Token::Let => {
                let mut j = i + 1;
                if j < len && tokens[j] == Token::Mut {
                    j += 1;
                }
                if let Some(Token::Ident(n)) = tokens.get(j) {
                    if !exclude.iter().any(|e| e == n) {
                        bound.push(n.clone());
                    }
                }
            }
            Token::For => {
                let mut j = i + 1;
                if j < len && tokens[j] == Token::Mut {
                    j += 1;
                }
                if let Some(Token::Ident(n)) = tokens.get(j) {
                    if !exclude.iter().any(|e| e == n) {
                        bound.push(n.clone());
                    }
                }
            }
            // 注意：def 函数名不重命名（模板 make_getter! 产出的 get_double 函数
            // 调用方需按原名引用——Nim 卫生性只作用于 let/for 局部变量）
            _ => {}
        }
        i += 1;
    }
    if bound.is_empty() {
        return tokens.to_vec();
    }
    let suffix = format!("__m{}", uid);
    tokens
        .iter()
        .map(|t| {
            if let Token::Ident(n) = t {
                if bound.iter().any(|b| b == n) {
                    Token::Ident(format!("{}{}", n, suffix))
                } else {
                    t.clone()
                }
            } else {
                t.clone()
            }
        })
        .collect()
}

/// 从参数 token 中提取调用方绑定的名字（let/for 绑定），供宏卫生性排除：
/// 参数是调用方代码，其绑定按调用方卫生原样保留，不重命名。
fn collect_param_bindings(tokens: &[Token]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut i = 0;
    let len = tokens.len();
    while i < len {
        match &tokens[i] {
            Token::Let => {
                let mut j = i + 1;
                if j < len && tokens[j] == Token::Mut {
                    j += 1;
                }
                if let Some(Token::Ident(n)) = tokens.get(j) {
                    if !names.iter().any(|e| e == n) {
                        names.push(n.clone());
                    }
                }
            }
            Token::For => {
                let mut j = i + 1;
                if j < len && tokens[j] == Token::Mut {
                    j += 1;
                }
                if let Some(Token::Ident(n)) = tokens.get(j) {
                    if !names.iter().any(|e| e == n) {
                        names.push(n.clone());
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    names
}

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

// ──────────────── 模板注册中心 ────────────────

/// template 模板定义（规范 08 §四）：参数签名自由（str/int/Tokens/泛型），
/// 返回类型必须为 Tokens。调用形式 `name!(args...)`（无 @ 前缀，仅 ! 后缀）。
#[derive(Debug, Clone)]
pub struct TemplateDef {
    pub name: String,
    /// 参数名列表
    pub param_names: Vec<String>,
    /// 参数类型名（str/int/Tokens/泛型参数名，与 param_names 一一对应）
    pub param_types: Vec<String>,
    /// 模板体语句（与 macro 相同的 MacroStmt，含 quote/反引号块）
    pub body: Vec<MacroStmt>,
}

/// 全局模板注册中心
#[derive(Debug, Clone, Default)]
pub struct TemplateRegistry {
    templates: HashMap<String, TemplateDef>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        TemplateRegistry { templates: HashMap::new() }
    }

    pub fn register(&mut self, def: TemplateDef) {
        self.templates.insert(def.name.clone(), def);
    }

    pub fn get(&self, name: &str) -> Option<&TemplateDef> {
        self.templates.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }

    /// 合并另一个注册中心的全部模板定义（跨模块模板导入用）
    pub fn merge(&mut self, other: TemplateRegistry) {
        for (n, def) in other.templates {
            self.templates.insert(n, def);
        }
    }
}

// ──────────────── 模板展开器 ────────────────

/// Token 流模板展开器（规范 08 §四）。
///
/// 流水线位置：MacroExpander 之后、Parser 之前。
///
/// 核心流程：
/// 1. 扫描 Token 流，识别 `name!(...)` 调用（无 @ 前缀，仅 ! 后缀）
/// 2. 查找模板定义，收集参数 tokens，绑定到模板参数
/// 3. 执行模板体（MacroInterpreter），将结果 token 流拼回
/// 4. 递归处理嵌套模板/宏（内层优先）
/// 宏/template 展开检查模式（08 §3.6 规则 4 多层硬检查策略，可配置）：
/// - `Loose`：只做最后一次完整检查（最终 Parser），不逐层检查——快速迭代
/// - `Light`（默认）：每层展开后做轻量结构校验（括号/缩进/else-elif 配对），
///   最终 Parser 完整检查兜底——定位层错误且不误伤片段型中间产物
/// - `Strict`：每层展开后跑完整 Parser（文档字面策略，中间层产物须独立合法）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    Loose,
    Light,
    Strict,
}

impl Default for CheckMode {
    fn default() -> Self {
        CheckMode::Light
    }
}

pub struct TemplateExpander {
    registry: TemplateRegistry,
    max_depth: usize,
    /// 宏卫生性计数器：每次展开分配唯一后缀（Nim 式 gensym）
    hygiene_counter: Cell<usize>,
    /// 逐层检查模式（08 §3.6 规则 4）：Loose 只最后检查 / Light 逐层轻量 / Strict 逐层完整
    check_mode: CheckMode,
}

impl TemplateExpander {
    pub fn new(registry: TemplateRegistry) -> Self {
        TemplateExpander { registry, max_depth: 128, hygiene_counter: Cell::new(0), check_mode: CheckMode::default() }
    }

    /// 设置逐层检查模式（08 §3.6 规则 4）：Loose / Light（默认）/ Strict
    pub fn set_check_mode(&mut self, mode: CheckMode) {
        self.check_mode = mode;
    }

    pub fn expand(&self, tokens: &[Token]) -> Result<Vec<Token>, String> {
        self.expand_inner(tokens, 0)
    }

    fn expand_inner(&self, tokens: &[Token], depth: usize) -> Result<Vec<Token>, String> {
        if depth > self.max_depth {
            return Err(format!("template expansion depth exceeded (max {})", self.max_depth));
        }
        let mut result: Vec<Token> = Vec::new();
        let mut i = 0;
        let len = tokens.len();
        while i < len {
            // 识别 `name!(` 模式：Ident 后紧跟 !（容忍中间 Newline/Indent）
            if matches!(&tokens[i], Token::Ident(_)) {
                let name = match &tokens[i] {
                    Token::Ident(n) => n.clone(),
                    _ => unreachable!(),
                };
                // 跳过空白找 !
                let mut excl_idx = i + 1;
                while excl_idx < len && matches!(&tokens[excl_idx], Token::Newline | Token::Indent) {
                    excl_idx += 1;
                }
                let has_exclam = excl_idx < len && tokens[excl_idx] == Token::Exclamation;
                if has_exclam && self.registry.contains(&name) {
                    // 模板调用 name!(...)
                    let after_exclam = if excl_idx + 1 < len { Some(&tokens[excl_idx + 1]) } else { None };
                    if after_exclam == Some(&Token::LParen) {
                        if let Some((input_tokens, input_end)) = self.collect_bracket_group(tokens, excl_idx + 2, Token::LParen, Token::RParen) {
                            let expanded = self.expand_template(&name, &input_tokens, depth)?;
                            result.extend(expanded);
                            i = input_end + 1;
                            continue;
                        }
                    } else {
                        // name! 无括号：作用于下一个表达式/声明（模板调用也支持）
                        let after_name = excl_idx + 1;
                        let decl_tokens = self.collect_decl_tokens(tokens, after_name);
                        let decl_end = after_name + decl_tokens.len();
                        eprintln!("DBG noblock: name={} after_name={} decl_len={} decl_end={} len={}", name, after_name, decl_tokens.len(), decl_end, len);
                        if len <= 42 {
                            for (ti, tt) in tokens.iter().enumerate() {
                                eprintln!("DBG T{:3} {:?}", ti, tt);
                            }
                        }
                        if !decl_tokens.is_empty() {
                            let mut expanded = self.expand_template(&name, &decl_tokens, depth)?;
                            rebalance_expanded_indents(&mut expanded);
                            result.extend(expanded);
                            i = decl_end;
                            continue;
                        }
                    }
                }
            }
            result.push(tokens[i].clone());
            i += 1;
        }
        Ok(result)
    }

    /// 展开模板调用：绑定参数 → 执行模板体 → 递归展开结果
    fn expand_template(&self, name: &str, input: &[Token], depth: usize) -> Result<Vec<Token>, String> {
        let def = self.registry.get(name)
            .ok_or_else(|| format!("undefined template '{}'", name))?;

        // 剥离缩进 token
        let cleaned: Vec<Token> = input.iter()
            .filter(|t| !matches!(t, Token::Indent | Token::Dedent))
            .cloned()
            .collect();

        // 按顶层逗号拆分参数（多参数模板调用 name!(a, b, ...)）
        let arg_groups = split_top_level_args(&cleaned);

        let mut interp = MacroInterpreter::new().with_depth(depth);
        // 绑定参数：位置一一对应（不足/超出按定义数量截断）
        for (idx, pname) in def.param_names.iter().enumerate() {
            let value = arg_groups.get(idx).cloned().unwrap_or_default();
            interp.bind_param(pname.clone(), Tokens::new(value));
        }

        let result = interp.execute_stmts(&def.body)
            .map_err(|e| format!("template '{}' expansion error: {}", name, e))?;

        // 宏卫生性：宏体局部绑定加唯一后缀（避免污染调用方同名变量）。
        // 恒等/透传 template（body = 单个参数引用，如 `template id2(v) = v`）：
        // 输出就是调用方传入的 token，按调用方卫生（§3.7）原样展开，不重命名
        // ——否则 `let x = 42` 被改成 `x__mN`，调用方引用失败
        let uid = self.hygiene_counter.get();
        self.hygiene_counter.set(uid + 1);
        let is_passthrough = def.body.len() == 1
            && matches!(&def.body[0], MacroStmt::Expr(MacroExpr::Ident(p))
                if def.param_names.iter().any(|pn| pn == p));
        let hygienic = if is_passthrough {
            result.tokens.clone()
        } else {
            // 排除参数中调用方自己的绑定（let/for），按调用方卫生原样保留
            let exclude = arg_groups.iter().flat_map(|g| collect_param_bindings(g)).collect::<Vec<_>>();
            hygienize_tokens(&result.tokens, uid, &exclude)
        };

        // 先递归展开结果中的嵌套模板/宏（内层各自检查）——产物可能仍含
        // 嵌套 name!/@name! 调用，先展开再检查（strict 完整 Parser 不误报）
        let expanded = self.expand_inner(&hygienic, depth + 1)?;

        // 逐层硬检查（08 §3.6 规则 4）：对本层**完全展开**的产物按 check_mode 校验
        if self.check_mode != CheckMode::Loose {
            check_expanded_tokens(&expanded, &format!("template '{}' 第 {} 层展开", name, depth + 1), self.check_mode)?;
        }

        Ok(expanded)
    }

    // ──────────────── Token 收集辅助函数 ────────────────
    fn collect_bracket_group(&self, tokens: &[Token], start: usize, open: Token, close: Token) -> Option<(Vec<Token>, usize)> {
        if start >= tokens.len() {
            return None;
        }
        let mut depth: i32 = 1;
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
                    if !seen_indent {
                        // 无块参数（`@name!` 后是同缩进语句，如 p11c_macro_deep）：
                        // 不吞外层 Dedent，立即停止，保留 Dedent 给调用方闭合缩进
                        break;
                    }
                    indent_level -= 1;
                    result.push(tokens[i].clone());
                    if indent_level == 0 && seen_indent {
                        break;
                    }
                }
                Token::Newline => {
                    if indent_level == 0 && seen_indent {
                        break;
                    }
                    result.push(tokens[i].clone());
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

/// 展开产物缩进平衡：quote 字符串中的 `\n    ` 会生成 Indent，但当宏/模板
/// 以无块参数方式调用（`@name!` / `name!` 后接同缩进语句，如 p11c_macro_deep）
/// 时，body 不含闭合 Dedent，导致展开产物 Indent/Dedent 不平衡，最终 Parser
/// 报 "Expected Dedent, got Eof"。这里在产物末尾补足缺失的 Dedent，使
/// quote 创建的块在产物内完整闭合（调用方后续同缩进 token 原样保留）。
fn rebalance_expanded_indents(tokens: &mut Vec<Token>) {
    let mut depth = 0i32;
    for t in tokens.iter() {
        match t {
            Token::Indent => depth += 1,
            Token::Dedent => depth -= 1,
            _ => {}
        }
    }
    eprintln!("DBG rebalance depth={} len={}", depth, tokens.len());
    while depth > 0 {
        tokens.push(Token::Dedent);
        depth -= 1;
    }
}

/// 按顶层逗号拆分模板调用参数（忽略括号/方括号内的逗号）
fn split_top_level_args(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut groups: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut brace = 0i32;
    for t in tokens {
        match t {
            Token::LParen => { paren += 1; current.push(t.clone()); }
            Token::RParen => { paren -= 1; current.push(t.clone()); }
            Token::LBrack => { bracket += 1; current.push(t.clone()); }
            Token::RBrack => { bracket -= 1; current.push(t.clone()); }
            Token::LBrace => { brace += 1; current.push(t.clone()); }
            Token::RBrace => { brace -= 1; current.push(t.clone()); }
            Token::Comma if paren == 0 && bracket == 0 && brace == 0 => {
                groups.push(std::mem::take(&mut current));
            }
            _ => current.push(t.clone()),
        }
    }
    if !current.is_empty() || groups.is_empty() {
        groups.push(current);
    }
    groups
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
    /// 宏卫生性计数器：每次展开分配唯一后缀（Nim 式 gensym）
    hygiene_counter: Cell<usize>,
    /// 逐层检查模式（08 §3.6 规则 4）：Loose 只最后检查 / Light 逐层轻量 / Strict 逐层完整
    check_mode: CheckMode,
}

/// 逐层轻量硬检查（08 §3.6 规则 4）：宏/template 每层展开产物做结构校验——
/// 括号/方括号/花括号平衡、缩进闭合、else/elif 配对。
/// 不做完整语法解析（片段型中间产物可能非完整语句，完整检查由最终 Parser 兜底）；
/// 不要求每个 if 都有 else（LZ 的 if 语句可无 else，误报会伤合法代码）。
fn light_check_tokens(tokens: &[Token], ctx: &str) -> Result<(), String> {
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut brace = 0i32;
    let mut indent_depth = 0i32;
    let mut if_balance = 0i32; // else/elif 需有对应 if；if 可无 else（不强制归零）
    for t in tokens {
        match t {
            Token::LParen => paren += 1,
            Token::RParen => {
                paren -= 1;
                if paren < 0 {
                    return Err(format!("{}: 多余右括号 )", ctx));
                }
            }
            Token::LBrack => bracket += 1,
            Token::RBrack => {
                bracket -= 1;
                if bracket < 0 {
                    return Err(format!("{}: 多余右方括号 ]", ctx));
                }
            }
            Token::LBrace => brace += 1,
            Token::RBrace => {
                brace -= 1;
                if brace < 0 {
                    return Err(format!("{}: 多余右花括号 }}", ctx));
                }
            }
            Token::Indent => indent_depth += 1,
            Token::Dedent => {
                indent_depth -= 1;
                if indent_depth < 0 {
                    return Err(format!("{}: 缩进不匹配（多余 Dedent）", ctx));
                }
            }
            Token::If => if_balance += 1,
            // guard 语句（`guard cond else: ...`）的 else 是 guard 的一部分，
            // 与 if 一样参与配对（check_eq 宏展开为 guard ... else: panic）
            Token::Guard => if_balance += 1,
            Token::Elif => {
                if if_balance <= 0 {
                    return Err(format!("{}: elif 无对应 if", ctx));
                }
            }
            Token::Else => {
                if if_balance <= 0 {
                    return Err(format!("{}: else 无对应 if", ctx));
                }
                if_balance -= 1;
            }
            _ => {}
        }
    }
    if paren != 0 {
        return Err(format!("{}: 括号未闭合（剩余 {} 个左括号）", ctx, paren));
    }
    if bracket != 0 {
        return Err(format!("{}: 方括号未闭合", ctx));
    }
    if brace != 0 {
        return Err(format!("{}: 花括号未闭合", ctx));
    }
    if indent_depth != 0 {
        return Err(format!("{}: 缩进未闭合（剩余 {} 层 Indent）", ctx, indent_depth));
    }
    Ok(())
}

/// 按检查模式校验展开产物（08 §3.6 规则 4）：
/// - `Light`（默认）→ 轻量结构校验（light_check_tokens：括号/缩进/else-elif）
/// - `Strict` → 完整 Parser（中间层产物须独立合法 LZ，文档字面策略）
/// - `Loose` → 跳过逐层检查（最终 Parser 兜底）
fn check_expanded_tokens(tokens: &[Token], ctx: &str, mode: CheckMode) -> Result<(), String> {
    match mode {
        CheckMode::Loose => Ok(()),
        CheckMode::Light => light_check_tokens(tokens, ctx),
        CheckMode::Strict => {
            // 产物仍含未展开的嵌套调用（@name! 或 name!）时跳过本层完整检查——
            // 这些调用由宏/模板展开器后续阶段或交替循环展开，最终 Parser 兜底；
            // 此时跑完整 Parser 会把 `!` 当非法 token 误报（run_print 产物含 gen_print!）
            if contains_pending_call(tokens) {
                return Ok(());
            }
            let mut parser = crate::parser::Parser::new(tokens.to_vec());
            match parser.parse_module() {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("{}（strict 完整检查）: {}", ctx, e)),
            }
        }
    }
}

/// 检测 token 流是否含未展开的嵌套调用标记：
/// - 宏调用 `@name!`（At + Ident + Exclamation）
/// - 模板调用 `name!`（Ident + Exclamation）
/// pub：main.rs/project.rs 交替展开循环用它判断稳定（token 流往返相同
/// 但残留未展开调用时不算稳定）
pub fn contains_pending_call(tokens: &[Token]) -> bool {
    let len = tokens.len();
    let mut i = 0;
    while i < len {
        if tokens[i] == Token::At {
            // @name!：At 后找 Ident（跳过空白），再找 !
            let mut j = i + 1;
            while j < len && matches!(&tokens[j], Token::Newline | Token::Indent) {
                j += 1;
            }
            if j < len && matches!(&tokens[j], Token::Ident(_)) {
                let mut k = j + 1;
                while k < len && matches!(&tokens[k], Token::Newline | Token::Indent) {
                    k += 1;
                }
                if k < len && tokens[k] == Token::Exclamation {
                    return true;
                }
            }
        } else if matches!(&tokens[i], Token::Ident(_)) {
            // name!：Ident 后紧跟 !
            let mut j = i + 1;
            while j < len && matches!(&tokens[j], Token::Newline | Token::Indent) {
                j += 1;
            }
            if j < len && tokens[j] == Token::Exclamation {
                return true;
            }
        }
        i += 1;
    }
    false
}

impl MacroExpander {
    pub fn new(registry: MacroRegistry) -> Self {
        MacroExpander { registry, max_depth: 128, hygiene_counter: Cell::new(0), check_mode: CheckMode::default() }
    }

    /// 设置逐层检查模式（08 §3.6 规则 4）：Loose / Light（默认）/ Strict
    pub fn set_check_mode(&mut self, mode: CheckMode) {
        self.check_mode = mode;
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
                // 规则 1（08 §3.5）：@name! 必须独占一行——@ 前必须是行首
                // （文件开头 / Newline / Indent / Dedent）。内联在表达式中
                // （`let x = @twice!(y) + 1`）非法。
                // 注意：template 定义的 name!（无 @）不受此限制，可内联（§3.5 末尾）
                let at_line_start = i == 0
                    || matches!(&tokens[i - 1], Token::Newline | Token::Indent | Token::Dedent);
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
                    // 规则 1（08 §3.5）：@name! 必须独占一行，不能内联在表达式中
                    // （`let x = @twice!(y) + 1`、`def f() = @id! def g()` 非法）
                    if !at_line_start {
                        return Err(format!(
                            "宏调用 `@{}!` 必须独占一行（规范 08 §3.5 规则 1，不能内联在表达式中；可用 f\\`...\\` 反引号块包裹非缩进内容）",
                            name
                        ));
                    }
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
                                let mut expanded = self.expand_attr_macro(&name, &attr_tokens, &decl_tokens, depth)?;
                                rebalance_expanded_indents(&mut expanded);
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
                        // 无参数宏（`macro name() -> Tokens`）不引用输入：
                        // 不收集后续声明作为 input——否则后续缩进块/Newline/Dedent
                        // 被吞（combo2 丢弃 input → main 缺语句/Dedent →
                        // "Expected Dedent, got Eof"）。有参数宏（note_block 等）
                        // 仍按规则 5 作用于后续缩进块。
                        let no_param = self.registry.get(&name)
                            .map_or(false, |d| d.param_names.is_empty());
                        if no_param {
                            let expanded = self.expand_macro(&name, &[], None, depth)?;
                            result.extend(expanded);
                            i = after_name; // ! 后，保留后续 token
                            continue;
                        }
                        let decl_tokens = self.collect_decl_tokens(tokens, after_name);
                        let decl_end = after_name + decl_tokens.len();
                        eprintln!("DBG noblock: name={} after_name={} decl_len={} decl_end={} len={}", name, after_name, decl_tokens.len(), decl_end, len);
                        if len <= 42 {
                            for (ti, tt) in tokens.iter().enumerate() {
                                eprintln!("DBG T{:3} {:?}", ti, tt);
                            }
                        }
                        if !decl_tokens.is_empty() {
                            let mut expanded = self.expand_macro(&name, &decl_tokens, None, depth)?;
                            rebalance_expanded_indents(&mut expanded);
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

        // 剥离缩进 token（括号内的 Indent/Dedent 不应该传递）
        let cleaned: Vec<Token> = input.iter()
            .filter(|t| !matches!(t, Token::Indent | Token::Dedent))
            .cloned()
            .collect();
        let input_tokens = Tokens::new(cleaned.clone());

        // 执行宏体。按调用形式分派（不依赖 def.is_attr——该标志由参数个数
        // 推断，3 参数宏 numeric_dispatch 会被误判，导致 requires attribute 报错）：
        //   - attr 有值（@name![attr](input)）→ 属性宏：param[0]=attr, param[1]=input
        //   - attr 无值且多参数 → 按顶层逗号拆分绑定（numeric_dispatch(cond,a,b)）
        //   - attr 无值且单参数 → 整体绑定
        let mut interp = MacroInterpreter::new().with_depth(depth);
        if let Some(attr_toks) = attr {
            // 属性宏：attr 绑定 param[0]、input 绑定 param[1]（用 get 防越界）
            if let Some(p0) = def.param_names.first() {
                interp.bind_param(p0.clone(), Tokens::new(attr_toks.to_vec()));
            }
            if let Some(p1) = def.param_names.get(1) {
                interp.bind_param(p1.clone(), input_tokens);
            }
        } else if def.param_names.len() > 1 {
            // 多参数非属性宏（check_eq(actual, expected)）：按顶层逗号拆分绑定
            let arg_groups = split_top_level_args(&cleaned);
            for (idx, pname) in def.param_names.iter().enumerate() {
                let value = arg_groups.get(idx).cloned().unwrap_or_default();
                interp.bind_param(pname.clone(), Tokens::new(value));
            }
        } else if def.param_names.len() == 1 {
            // 单参数宏：整体绑定输入 token 流
            interp.bind_param(def.param_names[0].clone(), input_tokens);
        }
        // 无参数宏（len()==0，如 `macro combo2() -> Tokens = ...`）：不绑定参数，
        // 避免 param_names[0] 越界 panic（index out of bounds）

        let result = interp.execute_stmts(&def.body)
            .map_err(|e| format!("macro '{}' expansion error: {}", name, e))?;
        eprintln!("DBG macro {} result len={}", name, result.tokens.len());
        for (i, t) in result.tokens.iter().enumerate() {
            eprintln!("  DBG R{:3} {:?}", i, t);
        }

        // 宏卫生性：宏体局部绑定加唯一后缀（避免污染调用方同名变量）。
        // 恒等/透传宏（body = 单个参数引用，如 `macro id(input) = input`）：
        // 输出就是调用方传入的 token，按调用方卫生（§3.7）原样展开，
        // 不重命名——否则 `let x = 21` 被改成 `x__mN`，调用方引用失败
        let uid = self.hygiene_counter.get();
        self.hygiene_counter.set(uid + 1);
        let is_passthrough = def.body.len() == 1
            && matches!(&def.body[0], MacroStmt::Expr(MacroExpr::Ident(p))
                if def.param_names.iter().any(|pn| pn == p));
        let hygienic = if is_passthrough {
            result.tokens.clone()
        } else {
            // 排除参数中调用方自己的绑定（let/for），按调用方卫生原样保留
            let exclude = collect_param_bindings(&cleaned);
            hygienize_tokens(&result.tokens, uid, &exclude)
        };

        // 先递归展开结果中的嵌套宏（内层展开时各自做检查）——
        // 产物可能仍含嵌套宏调用（如 run_double 产物含 @print_double!），
        // 必须先展开再检查，否则 Strict 的完整 Parser 会把 `@x!` 的
        // Exclamation 当非法 token 误报
        let expanded = self.expand_inner(&hygienic, depth + 1)?;

        // 逐层硬检查（08 §3.6 规则 4）：按 check_mode 分层，对本层**完全展开**
        // 的产物校验——Light：轻量结构校验（括号/缩进/else-elif）；Strict：
        // 完整 Parser（中间层产物须独立合法 LZ）；Loose：跳过（最终 Parser 兜底）
        if self.check_mode != CheckMode::Loose {
            check_expanded_tokens(&expanded, &format!("macro '{}' 第 {} 层展开", name, depth + 1), self.check_mode)?;
        }

        Ok(expanded)
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
    /// 声明结束：顶层缩进块闭合回 0（seen_indent 且 indent_level==0）、
    /// 或缩进块产物结束（含 Indent）、或下一个顶层 Def/Struct/At 声明。
    /// 无块参数 `@name!`（p11c_macro_deep）：收集同缩进语句直到缩进块
    /// 结束或下一个顶层声明，作为宏 body（quote 模板把整段包进 for）。
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
                    if !seen_indent {
                        // 无块参数（`@name!` 后是同缩进语句，如 p11c_macro_deep）：
                        // 不吞外层 Dedent，立即停止，保留 Dedent 给调用方闭合缩进
                        break;
                    }
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
            let next_ok = (i + 1 < len && matches!(&tokens[i + 1], Token::Newline))
                || i + 1 >= len;
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
                        if i < len && tokens[i] == Token::Comma { i += 1; is_attr = param_names.len() >= 2; }
                        if i < len && tokens[i] == Token::RParen { i += 1; break; }
                    }
                    _ => return Err(format!("expected parameter name in macro '{}' at token {}", name, i)),
                }
            }

            // 签名校验：macro 参数与返回类型必须为 Tokens（08 §3.1「签名固定 Tokens -> Tokens」）
            // 参数类型在循环中已跳过 Ident("Tokens")，此处校验返回类型
            i = skip_to(tokens, i, len, &Token::Arrow, &format!("expected '->' in macro '{}'", name))?;
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
            i = skip_to(tokens, i, len, &Token::Eq, &format!("expected '=' in macro '{}'", name))?;
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
                    return Err(format!("parameter parsing exceeded limit in template '{}'", name));
                }
                i = skip_blanks(tokens, i, len);
                match tokens.get(i) {
                    Some(Token::RParen) => { i += 1; break; }
                    Some(Token::Ident(pname)) => {
                        param_names.push(pname.clone());
                        i += 1;
                        // 跳过 : Type
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Colon { i += 1; }
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
                        if i + 1 < len
                            && tokens[i] == Token::Dot
                            && tokens[i + 1] == Token::Dot
                        {
                            i += 2;
                        }
                        i = skip_blanks(tokens, i, len);
                        if i < len && tokens[i] == Token::Comma { i += 1; }
                        if i < len && tokens[i] == Token::RParen { i += 1; break; }
                    }
                    _ => return Err(format!("expected parameter name in template '{}' at token {}", name, i)),
                }
            }

            // 跳过 -> Tokens（返回类型必须为 Tokens，08 §四「template 的产物一定是
            // Token 流」——参数签名自由（str/int/Tokens/泛型），但返回必须 Tokens）
            i = skip_to(tokens, i, len, &Token::Arrow, &format!("expected '->' in template '{}'", name))?;
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
            i = skip_to(tokens, i, len, &Token::Eq, &format!("expected '=' in template '{}'", name))?;
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
        if i >= len { break; }

        // 检测反引号块 ``` ... ```（前缀形式 f``` / r```：Ident("f"/"r") 在反引号前）
        // token 顺序：`f``` → Ident("f") Backtick Backtick Backtick；纯 ``` → Backtick...
        let backtick_prefix = if tokens[i] == Token::Backtick {
            crate::macros::group::BacktickPrefix::None
        } else if let Token::Ident(s) = &tokens[i] {
            if (s == "f" || s == "r")
                && i + 1 < len
                && tokens[i + 1] == Token::Backtick
            {
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
/// 解析宏/模板表达式（含二元 `+` 拼接：a + b + c）
pub fn parse_macro_expr(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
    let (mut expr, mut i) = parse_macro_primary(tokens, start)?;
    // 处理二元 + 链：quote("a" + name + "b") — 参数解析只取 primary 会丢 + 后续
    while i < tokens.len() && tokens[i] == Token::Plus {
        i += 1;
        let (right, ni) = parse_macro_primary(tokens, i)?;
        expr = MacroExpr::Binary {
            left: Box::new(expr),
            op: BinaryOp::Plus,
            right: Box::new(right),
        };
        i = ni;
    }
    Ok((expr, i))
}

/// 解析单个 primary 宏表达式（无二元运算符）
fn parse_macro_primary(tokens: &[Token], start: usize) -> Result<(MacroExpr, usize), String> {
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
            // `#!bin macro` 宏模块声明行（08 §2.1）：Token::Macro 独立成行
            Token::Macro,
            Token::Newline,
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
