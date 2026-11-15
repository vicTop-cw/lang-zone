// Lang-Zong 编译器 — macros/expand.rs
// 宏展开引擎：扫描 Token 流，识别 @name! 模式，递归展开

use crate::lexer::Token;
use crate::macros::group::Tokens;
use crate::macros::interp::{MacroExpr, MacroInterpreter, MacroStmt};

use std::cell::Cell;
use std::collections::HashMap;

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
        MacroRegistry {
            macros: HashMap::new(),
        }
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
        TemplateRegistry {
            templates: HashMap::new(),
        }
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
        TemplateExpander {
            registry,
            max_depth: 128,
            hygiene_counter: Cell::new(0),
            check_mode: CheckMode::default(),
        }
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
            return Err(format!(
                "template expansion depth exceeded (max {})",
                self.max_depth
            ));
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
                while excl_idx < len && matches!(&tokens[excl_idx], Token::Newline | Token::Indent)
                {
                    excl_idx += 1;
                }
                let has_exclam = excl_idx < len && tokens[excl_idx] == Token::Exclamation;
                if has_exclam && self.registry.contains(&name) {
                    // 模板调用 name!(...)
                    let after_exclam = if excl_idx + 1 < len {
                        Some(&tokens[excl_idx + 1])
                    } else {
                        None
                    };
                    if after_exclam == Some(&Token::LParen) {
                        if let Some((input_tokens, input_end)) = self.collect_bracket_group(
                            tokens,
                            excl_idx + 2,
                            Token::LParen,
                            Token::RParen,
                        ) {
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
    fn expand_template(
        &self,
        name: &str,
        input: &[Token],
        depth: usize,
    ) -> Result<Vec<Token>, String> {
        let def = self
            .registry
            .get(name)
            .ok_or_else(|| format!("undefined template '{}'", name))?;

        // 剥离缩进 token
        let cleaned: Vec<Token> = input
            .iter()
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

        let result = interp
            .execute_stmts(&def.body)
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
            let exclude = arg_groups
                .iter()
                .flat_map(|g| collect_param_bindings(g))
                .collect::<Vec<_>>();
            hygienize_tokens(&result.tokens, uid, &exclude)
        };

        // 先递归展开结果中的嵌套模板/宏（内层各自检查）——产物可能仍含
        // 嵌套 name!/@name! 调用，先展开再检查（strict 完整 Parser 不误报）
        let expanded = self.expand_inner(&hygienic, depth + 1)?;

        // 逐层硬检查（08 §3.6 规则 4）：对本层**完全展开**的产物按 check_mode 校验
        if self.check_mode != CheckMode::Loose {
            check_expanded_tokens(
                &expanded,
                &format!("template '{}' 第 {} 层展开", name, depth + 1),
                self.check_mode,
            )?;
        }

        Ok(expanded)
    }

    // ──────────────── Token 收集辅助函数 ────────────────
    fn collect_bracket_group(
        &self,
        tokens: &[Token],
        start: usize,
        open: Token,
        close: Token,
    ) -> Option<(Vec<Token>, usize)> {
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
            Token::LParen => {
                paren += 1;
                current.push(t.clone());
            }
            Token::RParen => {
                paren -= 1;
                current.push(t.clone());
            }
            Token::LBrack => {
                bracket += 1;
                current.push(t.clone());
            }
            Token::RBrack => {
                bracket -= 1;
                current.push(t.clone());
            }
            Token::LBrace => {
                brace += 1;
                current.push(t.clone());
            }
            Token::RBrace => {
                brace -= 1;
                current.push(t.clone());
            }
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
        return Err(format!(
            "{}: 缩进未闭合（剩余 {} 层 Indent）",
            ctx, indent_depth
        ));
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
        MacroExpander {
            registry,
            max_depth: 128,
            hygiene_counter: Cell::new(0),
            check_mode: CheckMode::default(),
        }
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
            return Err(format!(
                "macro expansion depth exceeded (max {})",
                self.max_depth
            ));
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
                    || matches!(
                        &tokens[i - 1],
                        Token::Newline | Token::Indent | Token::Dedent
                    );
                // 找到紧跟的 Ident（跳过 Newline/Indent）
                let mut name_idx = i + 1;
                while name_idx < len && matches!(&tokens[name_idx], Token::Newline | Token::Indent)
                {
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
                while name_end < len && matches!(&tokens[name_end], Token::Newline | Token::Indent)
                {
                    name_end += 1;
                }
                if name_end + 1 < len
                    && tokens[name_end] == Token::Dot
                    && matches!(&tokens[name_end + 1], Token::Ident(_))
                {
                    if let Token::Ident(n2) = &tokens[name_end + 1] {
                        name = n2.clone();
                        name_end += 2;
                    }
                }

                // 检查 name 后面是否有 !（跳过空白；别名解析后从 name_end 开始）
                let mut excl_idx = name_end;
                while excl_idx < len && matches!(&tokens[excl_idx], Token::Newline | Token::Indent)
                {
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
                    let after_exclam = if excl_idx + 1 < len {
                        Some(&tokens[excl_idx + 1])
                    } else {
                        None
                    };

                    // 检查是否有属性 [attr]
                    let has_attr = after_exclam == Some(&Token::LBrack);

                    if has_attr {
                        // 有属性宏 @name![attr](input)
                        let attr_start = excl_idx + 2; // 跳过 [
                        if let Some((attr_tokens, attr_end)) = self.collect_bracket_group(
                            tokens,
                            attr_start,
                            Token::LBrack,
                            Token::RBrack,
                        ) {
                            if attr_end + 1 >= len {
                                // 属性收集完成但文件结束 — 保留原 token 不展开
                                result.push(tokens[i].clone());
                                i += 1;
                                continue;
                            }
                            let after_attr = &tokens[attr_end + 1];
                            if after_attr == &Token::LParen {
                                if let Some((input_tokens, input_end)) = self.collect_bracket_group(
                                    tokens,
                                    attr_end + 2,
                                    Token::LParen,
                                    Token::RParen,
                                ) {
                                    let expanded = self.expand_attr_macro(
                                        &name,
                                        &attr_tokens,
                                        &input_tokens,
                                        depth,
                                    )?;
                                    result.extend(expanded);
                                    i = input_end + 1;
                                    continue;
                                }
                            } else {
                                // 有属性宏作用于声明：@name![attr] decl
                                let decl_tokens = self.collect_decl_tokens(tokens, attr_end + 1);
                                let decl_end = attr_end + 1 + decl_tokens.len();
                                let mut expanded = self.expand_attr_macro(
                                    &name,
                                    &attr_tokens,
                                    &decl_tokens,
                                    depth,
                                )?;
                                rebalance_expanded_indents(&mut expanded);
                                result.extend(expanded);
                                i = decl_end;
                                continue;
                            }
                        }
                    } else if after_exclam == Some(&Token::LParen) {
                        // 无属性宏 @name!(input)
                        if let Some((input_tokens, input_end)) = self.collect_bracket_group(
                            tokens,
                            excl_idx + 2,
                            Token::LParen,
                            Token::RParen,
                        ) {
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
                        let no_param = self
                            .registry
                            .get(&name)
                            .map_or(false, |d| d.param_names.is_empty());
                        if no_param {
                            let expanded = self.expand_macro(&name, &[], None, depth)?;
                            result.extend(expanded);
                            i = after_name; // ! 后，保留后续 token
                            continue;
                        }
                        let decl_tokens = self.collect_decl_tokens(tokens, after_name);
                        let decl_end = after_name + decl_tokens.len();
                        if !decl_tokens.is_empty() {
                            let mut expanded =
                                self.expand_macro(&name, &decl_tokens, None, depth)?;
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
    fn expand_macro(
        &self,
        name: &str,
        input: &[Token],
        attr: Option<&[Token]>,
        depth: usize,
    ) -> Result<Vec<Token>, String> {
        const LANG_MACROS: &[&str] = &["rust", "py", "tnr", "scala", "c"];
        if LANG_MACROS.contains(&name) {
            let (src, form) = if input.len() == 1 {
                match &input[0] {
                    Token::RawStrLit(s) => (s.clone(), crate::lexer::EmbedForm::Raw),
                    Token::TripleStrLit(s) => (s.clone(), crate::lexer::EmbedForm::Raw),
                    Token::FStrLit(s) => (s.clone(), crate::lexer::EmbedForm::Interp),
                    _ => (
                        tokens_to_string(input),
                        crate::lexer::EmbedForm::Indent,
                    ),
                }
            } else {
                (
                    tokens_to_string(input),
                    crate::lexer::EmbedForm::Indent,
                )
            };
            return Ok(vec![Token::EmbedBlock {
                lang: name.to_string(),
                src,
                form,
            }]);
        }

        let def = self
            .registry
            .get(name)
            .ok_or_else(|| format!("undefined macro '{}'", name))?;

        // 剥离缩进 token（括号内的 Indent/Dedent 不应该传递）
        let cleaned: Vec<Token> = input
            .iter()
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

        let result = interp
            .execute_stmts(&def.body)
            .map_err(|e| format!("macro '{}' expansion error: {}", name, e))?;

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
            check_expanded_tokens(
                &expanded,
                &format!("macro '{}' 第 {} 层展开", name, depth + 1),
                self.check_mode,
            )?;
        }

        Ok(expanded)
    }

    /// 展开有属性宏调用
    fn expand_attr_macro(
        &self,
        name: &str,
        attr: &[Token],
        input: &[Token],
        depth: usize,
    ) -> Result<Vec<Token>, String> {
        self.expand_macro(name, input, Some(attr), depth)
    }

    // ──────────────── Token 收集辅助函数 ────────────────

    /// 收集括号/方括号内的 token 序列（括号匹配，支持嵌套）
    /// 调用方已经跳过了开括号，所以 depth 从 1 开始
    fn collect_bracket_group(
        &self,
        tokens: &[Token],
        start: usize,
        open: Token,
        close: Token,
    ) -> Option<(Vec<Token>, usize)> {
        if start >= tokens.len() {
            return None;
        }
        let mut depth: i32 = 1; // 调用方已消费开括号
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
                Token::Def
                | Token::Struct
                | Token::Enum
                | Token::Trait
                | Token::Impl
                | Token::Const
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

// ──────────────── 宏定义解析器（已拆至 macros/parse.rs，此处仅转发，保持外部路径不变） ────────────────

pub use super::parse::{
    extract_macro_defs, extract_template_defs, has_bin_macro_declaration, parse_macro_expr,
};
pub(super) use super::parse::tokens_to_string;

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
            Token::IntLit(1),
            Token::Comma,
            Token::IntLit(2),
            Token::RParen,
        ];
        let (result, end) = expander
            .collect_bracket_group(&tokens, 0, Token::LParen, Token::RParen)
            .unwrap();
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
        let (result, end) = expander
            .collect_bracket_group(&tokens, 0, Token::LParen, Token::RParen)
            .unwrap();
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
            body: vec![MacroStmt::Expr(MacroExpr::Ident("input".into()))],
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
            body: vec![MacroStmt::Expr(MacroExpr::BacktickBlock {
                tokens: vec![Token::Ident("input".into()), Token::Star, Token::IntLit(2)],
                prefix: crate::macros::group::BacktickPrefix::F,
            })],
        });

        let expander = MacroExpander::new(registry);
        let tokens = vec![
            Token::At,
            Token::Ident("inner".into()),
            Token::Exclamation,
            Token::LParen,
            Token::IntLit(5),
            Token::RParen,
        ];
        let result = expander.expand(&tokens).unwrap();
        // 5 * 2 的展开取决于 f``` 插值处理
        // 这里验证至少没有崩溃
        assert!(!result.is_empty());
    }

    // 验证扩展后的 compile-time 数值运算：算术、比较、逻辑、一元负号、FloatLit
    #[test]
    fn test_compile_time_numeric_ops() {
        use crate::macros::interp::{MacroExpr, MacroInterpreter, BinaryOp};

        let mut interp = MacroInterpreter::new();

        // 算术
        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(10)),
            op: BinaryOp::Plus,
            right: Box::new(MacroExpr::IntLit(32)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::IntLit(42)]);

        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(100)),
            op: BinaryOp::Minus,
            right: Box::new(MacroExpr::IntLit(3)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::IntLit(97)]);

        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(6)),
            op: BinaryOp::Star,
            right: Box::new(MacroExpr::IntLit(7)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::IntLit(42)]);

        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(84)),
            op: BinaryOp::Slash,
            right: Box::new(MacroExpr::IntLit(2)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::IntLit(42)]);

        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(100)),
            op: BinaryOp::Percent,
            right: Box::new(MacroExpr::IntLit(58)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::IntLit(42)]);

        // 比较
        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(42)),
            op: BinaryOp::EqEq,
            right: Box::new(MacroExpr::IntLit(42)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::True]);

        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(1)),
            op: BinaryOp::Lt,
            right: Box::new(MacroExpr::IntLit(2)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::True]);

        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(5)),
            op: BinaryOp::Gt,
            right: Box::new(MacroExpr::IntLit(3)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::True]);

        // 逻辑
        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::BoolLit(true)),
            op: BinaryOp::And,
            right: Box::new(MacroExpr::BoolLit(true)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::True]);

        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::BoolLit(true)),
            op: BinaryOp::Or,
            right: Box::new(MacroExpr::BoolLit(false)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::True]);

        // 一元负号
        let expr = MacroExpr::UnaryMinus(Box::new(MacroExpr::IntLit(42)));
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::IntLit(-42)]);
    }

    // 验证 `+` 的双重语义：两边都是纯数值走算术加法，任一边含字符串/标识符走字符串拼接
    #[test]
    fn test_plus_string_concat_vs_arithmetic() {
        use crate::macros::group::Tokens;
        use crate::macros::interp::{BinaryOp, MacroExpr, MacroInterpreter};

        // 1) 两边都是 StrLit → 字符串拼接，且相邻 StrLit 合并为单个
        let mut interp = MacroInterpreter::new();
        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::StrLit("const ".into())),
            op: BinaryOp::Plus,
            right: Box::new(MacroExpr::StrLit("X".into())),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::StrLit("const X".into())]);

        // 2) 左边 StrLit、右边 Ident（形如 quote("const " + name)）→ 字符串拼接
        let mut interp2 = MacroInterpreter::new();
        interp2.bind_param(
            "X".into(),
            Tokens::new(vec![Token::Ident("LIMIT".into())]),
        );
        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::StrLit("const ".into())),
            op: BinaryOp::Plus,
            right: Box::new(MacroExpr::Ident("X".into())),
        };
        let tok = interp2.eval_expr(&expr).unwrap();
        assert_eq!(
            tok.tokens,
            vec![Token::StrLit("const ".into()), Token::Ident("LIMIT".into())]
        );

        // 3) 整条链 "const " + name + " = " + value（多层 Binary::Plus）→ 字符串拼接，
        //    而非对 IntLit(100) 做算术、也非报 expected integer
        let mut interp3 = MacroInterpreter::new();
        interp3.bind_param(
            "name".into(),
            Tokens::new(vec![Token::Ident("LIMIT".into())]),
        );
        interp3.bind_param(
            "value".into(),
            Tokens::new(vec![Token::IntLit(100)]),
        );
        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::Binary {
                left: Box::new(MacroExpr::Binary {
                    left: Box::new(MacroExpr::StrLit("const ".into())),
                    op: BinaryOp::Plus,
                    right: Box::new(MacroExpr::Ident("name".into())),
                }),
                op: BinaryOp::Plus,
                right: Box::new(MacroExpr::StrLit(" = ".into())),
            }),
            op: BinaryOp::Plus,
            right: Box::new(MacroExpr::Ident("value".into())),
        };
        let tok = interp3.eval_expr(&expr).unwrap();
        assert_eq!(
            tok.tokens,
            vec![
                Token::StrLit("const ".into()),
                Token::Ident("LIMIT".into()),
                Token::StrLit(" = ".into()),
                Token::IntLit(100),
            ]
        );

        // 4) 两边纯数值 IntLit(1) + IntLit(2) → 仍是算术加法
        let expr = MacroExpr::Binary {
            left: Box::new(MacroExpr::IntLit(1)),
            op: BinaryOp::Plus,
            right: Box::new(MacroExpr::IntLit(2)),
        };
        let tok = interp.eval_expr(&expr).unwrap();
        assert_eq!(tok.tokens, vec![Token::IntLit(3)]);
    }
}

