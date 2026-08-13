// Lang-Zong 编译器 — comptime 模块
// 编译期求值引擎 + `inspect` 源码内饰库（编译期专用）
//
// 集成位置：codegen 在遇到 Stmt::Comptime / Expr::Comptime / comptime let/const/fn 时
// 调用 ComptimeContext + ComptimeEvaluator 求值，成功则内联字面量（或不产出代码），
// 失败则降级为 compile_error!（编译失败，而非运行时错误）。
//
// inspect 命名空间仅在 comptime 上下文中可用；运行时不可调用、不生成代码。

use crate::ast::*;
use crate::types::Type;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════
// 编译期值域
// ═══════════════════════════════════════════════════════════════════

/// 编译期求值产生的值。它 **不是** 运行时值，仅存在于编译期。
#[derive(Debug, Clone)]
pub enum ComptimeValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    None,
    /// 同构列表
    List(Vec<ComptimeValue>),
    /// 异构元组
    Tuple(Vec<ComptimeValue>),
    /// 键值映射（对应 dict / kwargs）
    Map(HashMap<String, ComptimeValue>),
    /// 类型值：编译期持有的 `Type`（如 `int`、`List[str]`）
    Type(Type),
    /// inspect 返回的结构化对象
    Inspect(InspectObject),
}

impl ComptimeValue {
    /// 将编译期值内联为 Rust 字面量。Type / Inspect 等不可内联，返回 Err。
    pub fn to_rust_literal(&self) -> Result<String, String> {
        match self {
            ComptimeValue::Int(i) => Ok(i.to_string()),
            ComptimeValue::Float(f) => Ok(if f.is_nan() { "f64::NAN".into() }
                else if f.is_infinite() && *f > 0.0 { "f64::INFINITY".into() }
                else if f.is_infinite() { "f64::NEG_INFINITY".into() }
                else { format!("{f:?}") }),
            ComptimeValue::Bool(b) => Ok(b.to_string()),
            ComptimeValue::Str(s) => Ok(format!("{:?}", s)),
            ComptimeValue::None => Ok("()".into()),
            ComptimeValue::List(xs) => {
                let items: Result<Vec<_>, _> = xs.iter().map(|x| x.to_rust_literal()).collect();
                Ok(format!("vec![{}]", items?.join(", ")))
            }
            ComptimeValue::Tuple(xs) => {
                let items: Result<Vec<_>, _> = xs.iter().map(|x| x.to_rust_literal()).collect();
                let inner = items?.join(", ");
                Ok(if xs.len() == 1 { format!("({inner},)") } else { format!("({inner})") })
            }
            ComptimeValue::Map(_) => Err("Map 类型不能直接内联为字面量".into()),
            ComptimeValue::Type(_) => Err("Type 值不能内联为运行代码".into()),
            ComptimeValue::Inspect(_) => Err("Inspect 对象不能内联为运行代码".into()),
        }
    }

    /// 布尔判定（用于条件控制流）
    pub fn truthy(&self) -> bool {
        match self {
            ComptimeValue::Int(i) => *i != 0,
            ComptimeValue::Float(f) => *f != 0.0 && !f.is_nan(),
            ComptimeValue::Bool(b) => *b,
            ComptimeValue::Str(s) => !s.is_empty(),
            ComptimeValue::None => false,
            ComptimeValue::List(xs) => !xs.is_empty(),
            ComptimeValue::Tuple(xs) => !xs.is_empty(),
            ComptimeValue::Map(m) => !m.is_empty(),
            ComptimeValue::Type(_) => true,
            ComptimeValue::Inspect(_) => true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Inspect 数据结构（对齐 Python 3.12+ inspect / `inspect.Parameter` 等）
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}
impl ParameterKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParameterKind::PositionalOnly => "POSITIONAL_ONLY",
            ParameterKind::PositionalOrKeyword => "POSITIONAL_OR_KEYWORD",
            ParameterKind::VarPositional => "VAR_POSITIONAL",
            ParameterKind::KeywordOnly => "KEYWORD_ONLY",
            ParameterKind::VarKeyword => "VAR_KEYWORD",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub kind: ParameterKind,
    pub annotation: Option<Type>,
    pub default: Option<Box<ComptimeValue>>,
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_annotation: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub doc: Option<String>,
    pub functions: Vec<String>,
    pub structs: Vec<String>,
    pub traits: Vec<String>,
    pub consts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_annotation: Option<Type>,
    pub is_comptime: bool,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub bases: Vec<String>,
    pub methods: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MroInfo {
    pub name: String,
    pub mro: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ClassTree {
    pub name: String,
    pub children: Vec<ClassTree>,
}

#[derive(Debug, Clone)]
pub struct Abstracts {
    pub name: String,
    pub abstract_methods: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub function: Option<String>,
    pub filename: String,
    pub lineno: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub filename: String,
    pub source: String,
    pub first_lineno: i64,
    pub lines: Vec<String>,
}

/// Inspect 结构化对象
#[derive(Debug, Clone)]
pub enum InspectObject {
    Module(ModuleInfo),
    Function(FunctionInfo),
    Class(ClassInfo),
    Signature(Signature),
    Parameter(Parameter),
    Mro(MroInfo),
    ClassTree(ClassTree),
    Abstracts(Abstracts),
    Frame(FrameInfo),
    Source(SourceInfo),
}

// ═══════════════════════════════════════════════════════════════════
// 编译期上下文
// ═══════════════════════════════════════════════════════════════════

/// 编译期求值的最大嵌套深度（Zig 式 runaway 保护）
const MAX_COMPTIME_DEPTH: u32 = 256;

/// 编译期上下文
pub struct ComptimeContext<'a> {
    /// 当前编译模块的完整 AST
    pub module: &'a Module,
    /// 符号表：编译期变量名 → 值
    pub symtab: HashMap<String, ComptimeValue>,
    /// 嵌套深度（每次 eval_expr / eval_stmt +1，超限报错）
    depth: u32,
    /// 可选的源码文本（供 `inspect.getsource` 等使用）
    source: Option<String>,
}

impl<'a> ComptimeContext<'a> {
    pub fn new(module: &'a Module) -> Self {
        ComptimeContext {
            module,
            symtab: HashMap::new(),
            depth: 0,
            source: None,
        }
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    pub fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_COMPTIME_DEPTH {
            return Err(format!(
                "comptime 嵌套深度超限（{} > MAX={}），疑似死循环", self.depth - 1, MAX_COMPTIME_DEPTH
            ));
        }
        Ok(())
    }

    pub fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 编译期求值器
// ═══════════════════════════════════════════════════════════════════

pub struct ComptimeEvaluator;

impl ComptimeEvaluator {
    // ── 表达式求值 ──

    /// 对表达式求值，返回 CompileTimeValue 或错误
    pub fn eval_expr(e: &Expr, ctx: &mut ComptimeContext) -> Result<ComptimeValue, String> {
        ctx.enter()?;
        let result = Self::eval_expr_inner(e, ctx);
        ctx.leave();
        result
    }

    fn eval_expr_inner(e: &Expr, ctx: &mut ComptimeContext) -> Result<ComptimeValue, String> {
        match e {
            // 字面量
            Expr::IntLit(i) => Ok(ComptimeValue::Int(*i)),
            Expr::FloatLit(f) => Ok(ComptimeValue::Float(*f)),
            Expr::BoolLit(b) => Ok(ComptimeValue::Bool(*b)),
            Expr::StrLit(s) | Expr::FStrLit(s) | Expr::RawStrLit(s) => Ok(ComptimeValue::Str(s.clone())),
            Expr::NoneLit => Ok(ComptimeValue::None),
            Expr::Ident(name) => ctx.symtab.get(name.as_str())
                .cloned()
                .ok_or_else(|| format!("未定义的编译期变量 `{}`", name)),

            // 容器
            Expr::ListLit(elems) => {
                let xs: Result<Vec<_>, _> = elems.iter().map(|e| Self::eval_expr(e, ctx)).collect();
                Ok(ComptimeValue::List(xs?))
            }
            Expr::TupleLit(elems) => {
                let xs: Result<Vec<_>, _> = elems.iter().map(|e| Self::eval_expr(e, ctx)).collect();
                Ok(ComptimeValue::Tuple(xs?))
            }
            Expr::DictLit(pairs) => {
                let mut map = HashMap::new();
                for (k, v) in pairs {
                    let k = Self::eval_expr(k, ctx)?;
                    let v = Self::eval_expr(v, ctx)?;
                    let key = match k {
                        ComptimeValue::Str(s) => s,
                        _ => return Err("dict 编译期键仅支持字符串".into()),
                    };
                    map.insert(key, v);
                }
                Ok(ComptimeValue::Map(map))
            }

            // 运算
            Expr::Binary { left, op, right } => {
                let l = Self::eval_expr(left, ctx)?;
                let r = Self::eval_expr(right, ctx)?;
                Self::apply_binop(l, op, r)
            }
            Expr::Unary { op, operand } => {
                let v = Self::eval_expr(operand, ctx)?;
                Self::apply_unary(op, v)
            }

            // 控制流表达式
            Expr::If { cond, then_body, elif_clauses, else_body } => {
                if Self::eval_expr(cond, ctx)?.truthy() {
                    Ok(Self::eval_block(then_body, ctx)?.unwrap_or(ComptimeValue::None))
                } else {
                    let mut matched = false;
                    let mut result = ComptimeValue::None;
                    for (c, b) in elif_clauses {
                        if Self::eval_expr(c, ctx)?.truthy() {
                            result = Self::eval_block(b, ctx)?.unwrap_or(ComptimeValue::None);
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        if let Some(b) = else_body {
                            result = Self::eval_block(b, ctx)?.unwrap_or(ComptimeValue::None);
                        }
                    }
                    Ok(result)
                }
            }

            // 编译期表达式包装
            Expr::Comptime(inner) => Self::eval_expr(inner, ctx),

            // 调用 — inspect 内建或 comptime 函数
            Expr::Call { func, args, .. } => {
                let func_name = match func.as_ref() {
                    Expr::Ident(name) => name.clone(),
                    Expr::FieldAccess { receiver, field } => {
                        let rcv = match receiver.as_ref() {
                            Expr::Ident(n) => n.clone(),
                            other => return Err(format!("inspect 调用不支持复杂接收器: {:?}", other)),
                        };
                        if rcv == "inspect" {
                            format!("inspect::{}", field)
                        } else {
                            return Err(format!("不支持对 `{}` 的编译期方法调用", rcv));
                        }
                    }
                    other => return Err(format!("编译期仅支持函数名调用: {:?}", other)),
                };

                let args: Vec<ComptimeValue> = args.iter()
                    .map(|a| Self::eval_expr(a, ctx))
                    .collect::<Result<Vec<_>, _>>()?;

                if func_name.starts_with("inspect::") {
                    let inspect_fn = func_name.trim_start_matches("inspect::");
                    Self::eval_inspect_call(inspect_fn, &args, ctx)
                } else if func_name == "len" && args.len() == 1 {
                    // 内建 len()：列表/元组/字符串长度（simple_hash `len(s)`）
                    match &args[0] {
                        ComptimeValue::List(xs) => Ok(ComptimeValue::Int(xs.len() as i64)),
                        ComptimeValue::Tuple(xs) => Ok(ComptimeValue::Int(xs.len() as i64)),
                        ComptimeValue::Str(s) => Ok(ComptimeValue::Int(s.len() as i64)),
                        other => Err(format!("编译期 len 不支持 {:?}", other)),
                    }
                } else if func_name == "print" {
                    // 编译期 print：调试输出（08b §4.1），不产生运行时代码
                    let parts: Vec<String> = args
                        .iter()
                        .map(|a| a.to_rust_literal().unwrap_or_else(|_| format!("{:?}", a)))
                        .collect();
                    eprintln!("[comptime] {}", parts.join(" "));
                    Ok(ComptimeValue::None)
                } else {
                    // 编译期函数调用：查模块内同名函数，绑定参数后求值函数体
                    // （comptime def / 纯函数编译期执行，如生成查找表、计算哈希）
                    let f = ctx.module.functions.iter().find(|f| f.name == func_name)
                        .ok_or_else(|| format!("编译期函数 `{}` 未找到", func_name))?;
                    let mut fctx = ComptimeContext::new(ctx.module);
                    // 继承外层 depth：递归函数调用时深度限制生效（否则无限递归栈溢出）
                    fctx.depth = ctx.depth;
                    // 继承外层编译期符号（顶层 const 等）
                    fctx.symtab = ctx.symtab.clone();
                    // 位置绑定参数（self 不参与；默认参数缺省时跳过）
                    for (p, v) in f.params.iter().zip(args.iter()) {
                        fctx.symtab.insert(p.name.clone(), v.clone());
                    }
                    // 求值函数体：return 值或块尾表达式值即结果
                    match Self::eval_block(&f.body, &mut fctx)? {
                        Some(v) => Ok(v),
                        None => Err(format!("编译期函数 `{}` 无返回值", func_name)),
                    }
                }
            }

            // 字段访问（inspect 对象属性读取）
            Expr::FieldAccess { receiver, field } => {
                let rcv = Self::eval_expr(receiver, ctx)?;
                Self::get_field(&rcv, field)
            }

            // comptime: 块 — 块尾表达式值为结果（规范 08b §2.1）
            Expr::BlockExpr(stmts) => {
                Ok(Self::eval_block(stmts, ctx)?.unwrap_or(ComptimeValue::None))
            }

            // 列表方法调用：push（编译期构建查找表 `primes.push(n)`）
            Expr::MethodCall { receiver, method, args } => {
                let recv = Self::eval_expr(receiver, ctx)?;
                match (method.as_str(), recv) {
                    ("push", ComptimeValue::List(mut xs)) => {
                        if args.len() != 1 {
                            return Err("push 需 1 个参数".into());
                        }
                        let v = Self::eval_expr(&args[0], ctx)?;
                        xs.push(v);
                        Ok(ComptimeValue::List(xs))
                    }
                    ("len", ComptimeValue::List(xs)) => Ok(ComptimeValue::Int(xs.len() as i64)),
                    ("len", ComptimeValue::Str(s)) => Ok(ComptimeValue::Int(s.len() as i64)),
                    (m, other) => Err(format!("编译期不支持对 {:?} 调用方法 `{}`", other, m)),
                }
            }
            // 索引：列表 `xs[i]` / 字符串 `s[i]` / 字典 `d["key"]`
            Expr::Index { receiver, index } => {
                let recv = Self::eval_expr(receiver, ctx)?;
                match recv {
                    ComptimeValue::List(xs) => {
                        let idx = match Self::eval_expr(index, ctx)? {
                            ComptimeValue::Int(i) => i,
                            other => return Err(format!("编译期索引需整数，got {:?}", other)),
                        };
                        xs.get(idx as usize)
                            .cloned()
                            .ok_or_else(|| format!("编译期索引越界: {} >= {}", idx, xs.len()))
                    }
                    ComptimeValue::Tuple(xs) => {
                        let idx = match Self::eval_expr(index, ctx)? {
                            ComptimeValue::Int(i) => i,
                            other => return Err(format!("编译期索引需整数，got {:?}", other)),
                        };
                        xs.get(idx as usize)
                            .cloned()
                            .ok_or_else(|| format!("编译期索引越界: {} >= {}", idx, xs.len()))
                    }
                    ComptimeValue::Str(s) => {
                        let idx = match Self::eval_expr(index, ctx)? {
                            ComptimeValue::Int(i) => i,
                            other => return Err(format!("编译期索引需整数，got {:?}", other)),
                        };
                        s.chars()
                            .nth(idx as usize)
                            .map(|c| ComptimeValue::Int(c as i64))
                            .ok_or_else(|| format!("编译期索引越界: {} >= {}", idx, s.len()))
                    }
                    ComptimeValue::Map(m) => {
                        // 字典索引 d["key"]（08b §7：dict 索引可用）
                        let key = match Self::eval_expr(index, ctx)? {
                            ComptimeValue::Str(k) => k,
                            other => return Err(format!("编译期 dict 索引需字符串键，got {:?}", other)),
                        };
                        m.get(&key)
                            .cloned()
                            .ok_or_else(|| format!("编译期 dict 无键 `{}`", key))
                    }
                    other => Err(format!("编译期不支持对 {:?} 索引", other)),
                }
            }
            Expr::Match { .. } => Err("编译期不支持 match 表达式".into()),
            Expr::BuildBlock { .. } => Err("编译期不支持构建块".into()),
            Expr::PathAccess { .. } => Err("编译期不支持路径访问".into()),
            Expr::KwArg { .. } => Err("编译期不支持关键字参数".into()),
            Expr::SetLit(..) => Err("编译期不支持 set 字面量".into()),
            _ => Err(format!("编译期不支持该表达式: {:?}", e)),
        }
    }

    // ── 语句求值 ──

    /// 对单个语句求值。返回 Some(v) 表示遇到 return v；None 表示正常结束。
    fn eval_stmt(s: &Stmt, ctx: &mut ComptimeContext) -> Result<Option<ComptimeValue>, String> {
        match s {
            Stmt::Pass => Ok(None),
            Stmt::FnDef { .. } => {
                // 内嵌函数暂不支持编译期求值
                Ok(None)
            }
            Stmt::TypeAlias { .. } => {
                // 类型别名是类型层声明，编译期求值无需处理（值为 Unit）
                Ok(None)
            }
            Stmt::Expr(e) => {
                // `if cond: return x` 语句：if 分支体内的 return 是函数返回值信号，
                // 不能走 eval_expr（其 If 分支把 return 值当表达式值丢弃），
                // 需按语句级处理并传播 return（否则递归函数 factorial 无法终止，
                // 无限递归栈溢出）
                if let Expr::If { cond, then_body, elif_clauses, else_body } = e {
                    if Self::eval_expr(cond, ctx)?.truthy() {
                        return Self::eval_block(then_body, ctx);
                    }
                    for (c, b) in elif_clauses {
                        if Self::eval_expr(c, ctx)?.truthy() {
                            return Self::eval_block(b, ctx);
                        }
                    }
                    if let Some(b) = else_body {
                        return Self::eval_block(b, ctx);
                    }
                    return Ok(None);
                }
                // `primes.push(n)` 表达式语句：求值后写回 receiver 变量（副作用），
                // 否则 push 结果被丢弃，查找表构建失败（fib_table 空列表）
                if let Expr::MethodCall { receiver, method, args } = e {
                    if method == "push" {
                        if let Expr::Ident(name) = receiver.as_ref() {
                            let v = Self::eval_expr(e, ctx)?;
                            ctx.symtab.insert(name.clone(), v);
                            return Ok(None);
                        }
                    }
                }
                Self::eval_expr(e, ctx)?;
                Ok(None)
            }
            Stmt::Let { name, value, .. } => {
                let v = Self::eval_expr(value, ctx)?;
                ctx.symtab.insert(name.clone(), v);
                Ok(None)
            }
            Stmt::Const { name, value, .. } => {
                let v = Self::eval_expr(value, ctx)?;
                ctx.symtab.insert(name.clone(), v);
                Ok(None)
            }
            Stmt::Assign { target, value, .. } => {
                let name = match target {
                    Expr::Ident(n) => n.clone(),
                    _ => return Err("comptime 赋值目标必须是简单标识符".into()),
                };
                let v = Self::eval_expr(value, ctx)?;
                ctx.symtab.insert(name, v);
                Ok(None)
            }
            Stmt::Return(Some(e)) => Ok(Some(Self::eval_expr(e, ctx)?)),
            Stmt::Return(None) => Ok(Some(ComptimeValue::None)),
            Stmt::Comptime { body } => Self::eval_block(body, ctx),
            Stmt::Assert { expr, expected } => {
                let ok = Self::eval_expr(expr, ctx)?.truthy();
                if !ok {
                    let msg = match expected {
                        Some(exp) => format!("comptime assert 失败：期望 {}，实际 falsy",
                            Self::eval_expr(exp, ctx)?.to_rust_literal().unwrap_or_default()),
                        None => "comptime assert 失败".into(),
                    };
                    return Err(msg);
                }
                Ok(None)
            }
            Stmt::Check { expr, message: _ } => {
                // check 在 comptime 中等同于 assert（静默失败在运行时才有意义）
                let ok = Self::eval_expr(expr, ctx)?.truthy();
                if !ok {
                    return Err("comptime check 失败".into());
                }
                Ok(None)
            }
            Stmt::While { cond, body, .. } => {
                let mut guard = 0;
                while Self::eval_expr(cond, ctx)?.truthy() {
                    guard += 1;
                    if guard > 1_000_000 {
                        return Err("comptime while 步数超限（疑似死循环）".into());
                    }
                    if let Some(v) = Self::eval_block_loop(body, ctx)? {
                        // break（无值 → None 信号）→ 跳出循环继续；否则是 return 值
                        if matches!(v, ComptimeValue::None) {
                            break;
                        }
                        return Ok(Some(v));
                    }
                }
                Ok(None)
            }
            Stmt::For { var, iter, body, .. } => {
                let coll = Self::eval_expr(iter, ctx)?;
                let items = match coll {
                    ComptimeValue::List(xs) => xs,
                    ComptimeValue::Tuple(xs) => xs,
                    _ => return Err("comptime for 仅支持遍历 list/tuple".into()),
                };
                eprintln!("DBG comptime for: var={} items={}", var, items.len());
                for (it_idx, it) in items.iter().enumerate() {
                    eprintln!("DBG comptime for iter {}: {:?}", it_idx, it);
                    // 元组解构 `for (kind, name) in members:`：parser 把 var 存为
                    // "(kind, name)" 字符串（08b §7.3 示例），解析名字后逐一绑定
                    let trimmed = var.trim();
                    if trimmed.starts_with('(') && trimmed.ends_with(')') {
                        let inner = &trimmed[1..trimmed.len() - 1];
                        let names: Vec<String> = inner
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        match &it {
                            ComptimeValue::Tuple(vs) if vs.len() == names.len() => {
                                for (n, v) in names.iter().zip(vs.iter()) {
                                    ctx.symtab.insert(n.clone(), v.clone());
                                }
                            }
                            other => {
                                return Err(format!(
                                    "comptime for 解构: 期望 {}-元组，got {:?}",
                                    names.len(),
                                    other
                                ))
                            }
                        }
                    } else {
                        ctx.symtab.insert(var.clone(), it.clone());
                    }
                    if let Some(v) = Self::eval_block_loop(body, ctx)? {
                        // break（无值 → None 信号）→ 跳出循环继续；否则是 return 值
                        if matches!(v, ComptimeValue::None) {
                            break;
                        }
                        return Ok(Some(v));
                    }
                }
                Ok(None)
            }
            Stmt::Guard { cond, else_body, .. } => {
                // guard b != 0 else: body — 条件假时执行 else_body，然后中止
                if let Some(c) = cond {
                    if !Self::eval_expr(c, ctx)?.truthy() {
                        Self::eval_block(else_body, ctx)?;
                        return Err("guard 条件不满足，中止编译".into());
                    }
                }
                Ok(None)
            }
            Stmt::With { expr, alias, body } => {
                let v = Self::eval_expr(expr, ctx)?;
                if let Some(name) = alias {
                    ctx.symtab.insert(name.clone(), v);
                }
                Self::eval_block(body, ctx)
            }
            Stmt::Defer(body) => {
                // defer 在编译期：直接求值 body（defer 语义只对运行时生效）
                Self::eval_block(body, ctx)
            }
            Stmt::Raise(e) => {
                let msg = Self::eval_expr(e, ctx)?.to_rust_literal().unwrap_or_default();
                Err(format!("comptime raise: {}", msg))
            }
            Stmt::Loop(body) => {
                // comptime loop：== while true
                let mut guard = 0;
                loop {
                    guard += 1;
                    if guard > 1_000_000 {
                        return Err("comptime loop 步数超限（疑似死循环）".into());
                    }
                    if let Some(v) = Self::eval_block(body, ctx)? {
                        return Ok(Some(v));
                    }
                }
            }
            Stmt::Break(v) => {
                match v {
                    Some(e) => return Ok(Some(Self::eval_expr(e, ctx)?)),
                    None => return Ok(Some(ComptimeValue::None)),
                }
            }
            Stmt::Continue => Ok(None),
            Stmt::Yield(v) => {
                match v {
                    Some(e) => return Ok(Some(Self::eval_expr(e, ctx)?)),
                    None => return Ok(Some(ComptimeValue::None)),
                }
            }
            // 不支持编译期求值的语句：直接跳过（值为 Unit）
            Stmt::Test { .. }
            | Stmt::Suite { .. }
            | Stmt::YieldFrom(_)
            | Stmt::LetTuple { .. }
            | Stmt::WhileLet { .. }
            | Stmt::BreakLabel { .. }
            | Stmt::Block { .. }
            | Stmt::CheckerBlock { .. }
            | Stmt::BlockCall { .. }
            | Stmt::EnumDef(_) => Ok(None),
        }
    }

    // ── 块求值 ──

    /// 求值语句块。Some(v) 表示 return v；None 正常结束。
    pub fn eval_block(stmts: &[Stmt], ctx: &mut ComptimeContext) -> Result<Option<ComptimeValue>, String> {
        let n = stmts.len();
        // 块尾表达式（Stmt::Expr）的值作为块结果（规范 08b §2.1「块尾表达式的值
        // 即为 comptime 结果」）。逐语句求值，遇到 return 提前返回。
        for (i, s) in stmts.iter().enumerate() {
            let is_last = i + 1 == n;
            if is_last {
                if let Stmt::Expr(e) = s {
                    let v = Self::eval_expr(e, ctx)?;
                    return Ok(Some(v));
                }
            }
            if let Some(v) = Self::eval_stmt(s, ctx)? {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    /// 循环体求值：逐语句执行，仅传播显式 return（不把块尾表达式值当返回值）。
    /// 与 eval_block 的区别：`for x in [1,2,3]: out.push(x)` 中 push 是表达式语句，
    /// 若按块尾值处理会提前 return 导致只迭代一次。
    fn eval_block_loop(stmts: &[Stmt], ctx: &mut ComptimeContext) -> Result<Option<ComptimeValue>, String> {
        for s in stmts {
            if let Some(v) = Self::eval_stmt(s, ctx)? {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    // ── 二元运算 ──

    fn apply_binop(l: ComptimeValue, op: &BinOp, r: ComptimeValue) -> Result<ComptimeValue, String> {
        use BinOp::*;
        match (&l, op, &r) {
            // 整数算术
            (ComptimeValue::Int(a), Add, ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a + *b)),
            (ComptimeValue::Int(a), Sub, ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a - *b)),
            (ComptimeValue::Int(a), Mul, ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a * *b)),
            (ComptimeValue::Int(a), Div, ComptimeValue::Int(b)) => {
                if *b == 0 { Err("整数除法：除数为零".into()) }
                else { Ok(ComptimeValue::Int(*a / *b)) }
            }
            (ComptimeValue::Int(a), Mod, ComptimeValue::Int(b)) => {
                if *b == 0 { Err("整数取模：除数为零".into()) }
                else { Ok(ComptimeValue::Int(*a % *b)) }
            }
            (ComptimeValue::Int(a), Pow, ComptimeValue::Int(b)) => {
                Ok(ComptimeValue::Int(a.saturating_pow(*b as u32)))
            }

            // 浮点算术
            (ComptimeValue::Float(a), Add, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(*a + *b)),
            (ComptimeValue::Float(a), Sub, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(*a - *b)),
            (ComptimeValue::Float(a), Mul, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(*a * *b)),
            (ComptimeValue::Float(a), Div, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(*a / *b)),
            (ComptimeValue::Float(a), Mod, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(*a % *b)),
            (ComptimeValue::Float(a), Pow, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a.powf(*b))),

            // 字符串加法（拼接）
            (ComptimeValue::Str(a), Add, ComptimeValue::Str(b)) => Ok(ComptimeValue::Str(format!("{}{}", a, b))),

            // 整数与浮点自动提升
            (ComptimeValue::Int(a), Add, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(*a as f64 + *b)),
            (ComptimeValue::Float(a), Add, ComptimeValue::Int(b)) => Ok(ComptimeValue::Float(*a + *b as f64)),
            (ComptimeValue::Int(a), Mul, ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(*a as f64 * *b)),
            (ComptimeValue::Float(a), Mul, ComptimeValue::Int(b)) => Ok(ComptimeValue::Float(*a * *b as f64)),

            // 比较运算
            (ComptimeValue::Int(a), Eq,  ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(*a == *b)),
            (ComptimeValue::Int(a), Ne,  ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(*a != *b)),
            (ComptimeValue::Int(a), Lt,  ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(*a < *b)),
            (ComptimeValue::Int(a), Le,  ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(*a <= *b)),
            (ComptimeValue::Int(a), Gt,  ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(*a > *b)),
            (ComptimeValue::Int(a), Ge,  ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(*a >= *b)),
            (ComptimeValue::Float(a), Eq, ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(*a == *b)),
            (ComptimeValue::Float(a), Ne, ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(*a != *b)),
            (ComptimeValue::Float(a), Lt, ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(*a < *b)),
            (ComptimeValue::Float(a), Le, ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(*a <= *b)),
            (ComptimeValue::Float(a), Gt, ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(*a > *b)),
            (ComptimeValue::Float(a), Ge, ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(*a >= *b)),
            (ComptimeValue::Bool(a), Eq, ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a == *b)),
            (ComptimeValue::Bool(a), Ne, ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a != *b)),
            (ComptimeValue::Str(a), Eq, ComptimeValue::Str(b)) => Ok(ComptimeValue::Bool(*a == *b)),
            (ComptimeValue::Str(a), Ne, ComptimeValue::Str(b)) => Ok(ComptimeValue::Bool(*a != *b)),

            // 逻辑运算
            (ComptimeValue::Bool(a), And, ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a && *b)),
            (ComptimeValue::Bool(a), Or,  ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a || *b)),

            // Bitwise
            (ComptimeValue::Int(a), BitAnd, ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a & *b)),
            (ComptimeValue::Int(a), BitOr,  ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a | *b)),
            (ComptimeValue::Int(a), BitXor, ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a ^ *b)),
            (ComptimeValue::Int(a), Shl,    ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a << *b)),
            (ComptimeValue::Int(a), Shr,    ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a >> *b)),
            (ComptimeValue::Int(a), BitOr,  ComptimeValue::Bool(b)) => Ok(ComptimeValue::Int(*a | *b as i64)),
            (ComptimeValue::Bool(a), BitOr, ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(*a as i64 | *b)),
            (ComptimeValue::Bool(a), BitAnd,ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a && *b)),


            // 剩余未匹配算子 → 报错
            (l_val, op_val, r_val) => Err(format!("编译期不支持 {:?} {:?} {:?} 的运算", l_val, op_val, r_val)),
        }
    }

    // ── 一元运算 ──

    fn apply_unary(op: &UnaryOp, v: ComptimeValue) -> Result<ComptimeValue, String> {
        use UnaryOp::*;
        match (op, v) {
            (Neg, ComptimeValue::Int(i)) => Ok(ComptimeValue::Int(-i)),
            (Neg, ComptimeValue::Float(f)) => Ok(ComptimeValue::Float(-f)),
            (Not, v) => Ok(ComptimeValue::Bool(!v.truthy())),
            _ => Err(format!("编译期不支持 `{:?}` 的一元运算", op)),
        }
    }

    // ── Inspect 对象字段读取 ──

    fn get_field(v: &ComptimeValue, field: &str) -> Result<ComptimeValue, String> {
        let obj = match v {
            ComptimeValue::Inspect(obj) => obj,
            _ => return Err(format!("不是 inspect 对象：{:?}", v)),
        };
        match (obj, field) {
            (InspectObject::Module(m), "name") => Ok(ComptimeValue::Str(m.name.clone())),
            (InspectObject::Module(m), "doc") => match &m.doc {
                Some(d) => Ok(ComptimeValue::Str(d.clone())),
                None => Ok(ComptimeValue::None),
            },
            (InspectObject::Module(m), "functions") => Ok(ComptimeValue::List(
                m.functions.iter().map(|s| ComptimeValue::Str(s.clone())).collect())),
            (InspectObject::Module(m), "structs") => Ok(ComptimeValue::List(
                m.structs.iter().map(|s| ComptimeValue::Str(s.clone())).collect())),
            (InspectObject::Module(m), "traits") => Ok(ComptimeValue::List(
                m.traits.iter().map(|s| ComptimeValue::Str(s.clone())).collect())),
            (InspectObject::Module(m), "consts") => Ok(ComptimeValue::List(
                m.consts.iter().map(|s| ComptimeValue::Str(s.clone())).collect())),
            (InspectObject::Function(f), "name") => Ok(ComptimeValue::Str(f.name.clone())),
            (InspectObject::Function(f), "parameters") => Ok(ComptimeValue::List(
                f.parameters.iter().map(|p| ComptimeValue::Inspect(InspectObject::Parameter(p.clone()))).collect())),
            (InspectObject::Function(f), "return_annotation") => match &f.return_annotation {
                Some(t) => Ok(ComptimeValue::Type(t.clone())),
                None => Ok(ComptimeValue::None),
            },
            (InspectObject::Function(f), "is_comptime") => Ok(ComptimeValue::Bool(f.is_comptime)),
            (InspectObject::Class(c), "name") => Ok(ComptimeValue::Str(c.name.clone())),
            (InspectObject::Class(c), "bases") => Ok(ComptimeValue::List(
                c.bases.iter().map(|s| ComptimeValue::Str(s.clone())).collect())),
            (InspectObject::Class(c), "methods") => Ok(ComptimeValue::List(
                c.methods.iter().map(|s| ComptimeValue::Str(s.clone())).collect())),
            (InspectObject::Signature(s), "name") => match &s.name {
                Some(n) => Ok(ComptimeValue::Str(n.clone())),
                None => Ok(ComptimeValue::None),
            },
            (InspectObject::Signature(s), "parameters") => Ok(ComptimeValue::List(
                s.parameters.iter().map(|p| ComptimeValue::Inspect(InspectObject::Parameter(p.clone()))).collect())),
            (InspectObject::Signature(s), "return_annotation") => match &s.return_annotation {
                Some(t) => Ok(ComptimeValue::Type(t.clone())),
                None => Ok(ComptimeValue::None),
            },
            (InspectObject::Parameter(p), "name") => Ok(ComptimeValue::Str(p.name.clone())),
            (InspectObject::Parameter(p), "kind") => Ok(ComptimeValue::Str(p.kind.as_str().to_string())),
            (InspectObject::Parameter(p), "annotation") => match &p.annotation {
                Some(t) => Ok(ComptimeValue::Type(t.clone())),
                None => Ok(ComptimeValue::None),
            },
            (InspectObject::Mro(m), "name") => Ok(ComptimeValue::Str(m.name.clone())),
            (InspectObject::Mro(m), "mro") => Ok(ComptimeValue::List(
                m.mro.iter().map(|s| ComptimeValue::Str(s.clone())).collect())),
            (InspectObject::Frame(f), "function") => match &f.function {
                Some(n) => Ok(ComptimeValue::Str(n.clone())),
                None => Ok(ComptimeValue::None),
            },
            (InspectObject::Frame(f), "filename") => Ok(ComptimeValue::Str(f.filename.clone())),
            (InspectObject::Frame(f), "lineno") => match f.lineno {
                Some(n) => Ok(ComptimeValue::Int(n)),
                None => Ok(ComptimeValue::None),
            },
            (InspectObject::Source(s), "filename") => Ok(ComptimeValue::Str(s.filename.clone())),
            (InspectObject::Source(s), "source") => Ok(ComptimeValue::Str(s.source.clone())),
            (InspectObject::Source(s), "first_lineno") => Ok(ComptimeValue::Int(s.first_lineno)),
            (InspectObject::Source(s), "lines") => Ok(ComptimeValue::List(
                s.lines.iter().map(|l| ComptimeValue::Str(l.clone())).collect())),
            _ => Err(format!("inspect 对象不支持字段 `{}`", field)),
        }
    }

    // ── inspect 内建函数分发 ──

    fn eval_inspect_call(
        name: &str, args: &[ComptimeValue], ctx: &mut ComptimeContext,
    ) -> Result<ComptimeValue, String> {
        match name {
            // ── 类型检视 ──
            "getmembers" => {
                if args.is_empty() {
                    // 获取当前模块的全部成员
                    let m = Self::module_info(ctx.module);
                    let mut members = Vec::new();
                    for f in &m.functions {
                        members.push(("function".into(), ComptimeValue::Str(f.clone())));
                    }
                    for s in &m.structs {
                        members.push(("struct".into(), ComptimeValue::Str(s.clone())));
                    }
                    for t in &m.traits {
                        members.push(("trait".into(), ComptimeValue::Str(t.clone())));
                    }
                    for c in &m.consts {
                        members.push(("const".into(), ComptimeValue::Str(c.clone())));
                    }
                    Ok(ComptimeValue::List(members.into_iter().map(|(kind, val)|
                        ComptimeValue::Tuple(vec![ComptimeValue::Str(kind), val])
                    ).collect()))
                } else {
                    Err("getmembers 暂只支无参调用（全模块）".into())
                }
            }
            "getmodulename" => {
                Ok(ComptimeValue::Str(ctx.module.name.clone().unwrap_or_default()))
            }
            "ismodule" => {
                let name = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("ismodule 需字符串参数")?;
                Ok(ComptimeValue::Bool(name == ctx.module.name.as_deref().unwrap_or("")))
            }
            "isclass" => {
                let name = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("isclass 需字符串参数")?;
                Ok(ComptimeValue::Bool(ctx.module.structs.iter().any(|s| s.name == name)))
            }
            "isfunction" => {
                let name = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("isfunction 需字符串参数")?;
                Ok(ComptimeValue::Bool(ctx.module.functions.iter().any(|f| f.name == name)))
            }
            "ismethod" => {
                if args.len() < 2 { return Err("ismethod 需 2 参数 (class_name, method_name)".into()); }
                let cls = args[0].as_str().ok_or("ismethod cls 需字符串")?;
                let method = args[1].as_str().ok_or("ismethod method 需字符串")?;
                let has = ctx.module.structs.iter()
                    .filter(|s| s.name == cls)
                    .any(|s| s.methods.iter().any(|m| m.name == method));
                Ok(ComptimeValue::Bool(has))
            }

            // ── 签名 ──
            "signature" => {
                let name = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("signature 需字符串参数（函数名）")?;
                let f = ctx.module.functions.iter().find(|f| f.name == name)
                    .ok_or_else(|| format!("未找到函数 `{}`", name))?;
                let params: Vec<Parameter> = f.params.iter().map(|p| Parameter {
                    name: p.name.clone(),
                    kind: ParameterKind::PositionalOrKeyword,
                    annotation: Some(p.ty.clone()),
                    default: None,
                }).collect();
                Ok(ComptimeValue::Inspect(InspectObject::Signature(Signature {
                    name: Some(f.name.clone()),
                    parameters: params,
                    return_annotation: f.return_type.clone(),
                })))
            }

            // ── 源码 ──
            "getsource" | "getsourcefile" | "getsourcelines" | "getdoc" | "getcomments" => {
                Err(format!("inspect::{} 需要注入源码文本（ComptimeContext::with_source）", name))
            }

            // ── 当前模块信息 ──
            "module_info" => {
                let m = Self::module_info(ctx.module);
                Ok(ComptimeValue::Inspect(InspectObject::Module(m)))
            }
            "function_info" => {
                let name = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("function_info 需字符串参数（函数名）")?;
                let f = ctx.module.functions.iter().find(|f| f.name == name)
                    .ok_or_else(|| format!("未找到函数 `{}`", name))?;
                let params: Vec<Parameter> = f.params.iter().map(|p| Parameter {
                    name: p.name.clone(),
                    kind: ParameterKind::PositionalOrKeyword,
                    annotation: Some(p.ty.clone()),
                    default: None,
                }).collect();
                Ok(ComptimeValue::Inspect(InspectObject::Function(FunctionInfo {
                    name: f.name.clone(),
                    parameters: params,
                    return_annotation: f.return_type.clone(),
                    is_comptime: false,
                })))
            }

            // ── 类型层级 ──
            "getmro" => {
                let name = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("getmro 需字符串参数（类名）")?;
                let _ = ctx.module.structs.iter().find(|s| s.name == name)
                    .ok_or_else(|| format!("未找到类 `{}`", name))?;
                let mro = vec![name.clone()];
                Ok(ComptimeValue::Inspect(InspectObject::Mro(MroInfo { name: name.clone(), mro })))
            }
            "getabstracts" => {
                let name = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("getabstracts 需字符串参数（类名）")?;
                Ok(ComptimeValue::Inspect(InspectObject::Abstracts(Abstracts {
                    name: name.clone(),
                    abstract_methods: Vec::new(), // 待实现具体抽象方法检测
                })))
            }

            // ── 编译时断言 ──
            "assert_module_has" => {
                let target = args.first().and_then(|a| match a { ComptimeValue::Str(s) => Some(s.clone()), _ => None })
                    .ok_or("assert_module_has 需字符串参数")?;
                let m = Self::module_info(ctx.module);
                let ok = m.functions.contains(&target)
                    || m.structs.contains(&target)
                    || m.traits.contains(&target)
                    || m.consts.contains(&target);
                if !ok {
                    return Err(format!("编译时断言：模块中未找到 `{}`", target));
                }
                Ok(ComptimeValue::None)
            }

            // ── 字段/成员检查 ──
            "has_field" => {
                if args.len() < 2 { return Err("has_field 需 2 参数 (struct, field)".into()); }
                let s_name = args[0].as_str().ok_or("has_field struct 名需字符串")?;
                let f_name = args[1].as_str().ok_or("has_field field 名需字符串")?;
                let has = ctx.module.structs.iter()
                    .filter(|s| s.name == s_name)
                    .any(|s| s.fields.iter().any(|f| f.name == f_name));
                Ok(ComptimeValue::Bool(has))
            }

            _ => Err(format!("未知的 inspect 函数: `{}`", name)),
        }
    }

    // ── 辅助：模块信息提取 ──

    fn module_info(m: &Module) -> ModuleInfo {
        ModuleInfo {
            name: m.name.clone().unwrap_or_default(),
            doc: Some(String::new()),
            functions: m.functions.iter().map(|f| f.name.clone()).collect(),
            structs: m.structs.iter().map(|s| s.name.clone()).collect(),
            traits: m.traits.iter().map(|t| t.name.clone()).collect(),
            consts: m.consts.iter().map(|c| c.name.clone()).collect(),
        }
    }
}

// 为参数提取添加辅助 trait
trait AsStr {
    fn as_str(&self) -> Option<&str>;
}
impl AsStr for ComptimeValue {
    fn as_str(&self) -> Option<&str> {
        match self {
            ComptimeValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }
}
