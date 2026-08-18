// Lang-Zone 编译器 — ir/mod.rs
// LZIR-H 模块入口
//
// LZIR（Lang-Zone Intermediate Representation）是编译器的跨后端共享契约。
// 前端产出 LZIR，后端只消费 LZIR 发射目标代码。
//
// 形态：强类型树 / ANF 风格，每 Expr 携带 IrType + Span。

pub mod types;
pub mod node;
pub mod display;
pub mod builder;
pub mod codegen;
pub mod codegen_cython;
pub mod duck_check;
pub mod lz_codegen;

pub use builder::build_ir;
pub use duck_check::check_duck_satisfaction;

/// LZIR-H 版本号（节点兼容性标识）
pub const IR_VERSION: u32 = 1;

/// LZIR 缓存文件魔数（序列化缓存头）
pub const IR_MAGIC: &[u8; 4] = b"LZIR";

/// LZIR 稳定性契约（IR 稳定性契约）：
///
/// 版本语义（对齐 semver 主/次版本）：
/// - 主版本（IR_VERSION 十位以上）：节点结构破坏性变更（删除/重命名字段、
///   改变枚举变体语义）→ 必须 bump 主版本，旧缓存全部失效。
/// - 次版本（IR_VERSION 个位）：向后兼容的增量（新增可选字段、新增枚举变体、
///   新增节点类型）→ 可 bump 次版本，旧缓存仍可读（serde 默认忽略未知字段）。
///
/// 缓存失效规则：
/// - 缓存文件头 = IR_MAGIC + IR_VERSION + 源码哈希（source_hash）。
/// - 任一不匹配 → 缓存失效，需重新构建 IR。
/// - 序列化格式：开发期 JSON（可读、可 diff），生产期 bincode（紧凑、快）。
///
/// 兼容性保证：
/// - 同一主版本内，旧缓存可被新编译器读取（向后兼容）。
/// - 跨主版本，禁止读取旧缓存（必须重建）。
pub const IR_COMPAT_RULES: &str = "\
LZIR stability contract:
- major version bump: breaking node-structure changes (field removal/rename, variant semantic change)
- minor version bump: backward-compatible additions (new optional field, new variant, new node)
- cache header: IR_MAGIC + IR_VERSION + source_hash; mismatch => cache invalid
- dev format: JSON; production format: bincode
- same-major caches are readable by newer compilers; cross-major caches must be rebuilt";

// ── IrModule 顶层结构 ──

/// 模块依赖边（模块边界：依赖图）
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct ModuleDep {
    /// 依赖的模块名
    pub module: String,
    /// 依赖来源（import 语句位置，含文件路径）
    pub span: node::Span,
}

/// LZIR 顶层模块 — 一个 .lz 文件编译后的 IR 根节点
#[derive(Debug, Clone)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct IrModule {
    pub name: String,
    pub directive: node::ModuleDirective,
    pub items: Vec<node::Item>,
    pub prelude: Vec<String>,
    pub version: u32,
    /// 源文件路径（Span 完备化：模块级文件定位）
    pub file_path: Option<String>,
    /// 源文件文本（增量编译缓存失效：源码哈希）
    pub source_text: Option<String>,
    /// 模块导出符号表（模块边界：跨模块引用）
    pub exports: Vec<String>,
    /// 模块依赖（模块边界：依赖图）
    pub dependencies: Vec<ModuleDep>,
}

impl IrModule {
    pub fn new(name: String) -> Self {
        IrModule {
            name,
            directive: node::ModuleDirective::default(),
            items: vec![],
            prelude: vec![],
            version: IR_VERSION,
            file_path: None,
            source_text: None,
            exports: vec![],
            dependencies: vec![],
        }
    }

    /// 设置源文件路径（并注入到模块级 span 定位）
    pub fn with_file_path(mut self, file_path: impl Into<String>) -> Self {
        self.file_path = Some(file_path.into());
        self
    }

    /// 设置源文件文本（增量编译缓存失效用）
    pub fn with_source_text(mut self, source_text: impl Into<String>) -> Self {
        self.source_text = Some(source_text.into());
        self
    }

    /// 源码哈希（增量编译缓存失效：源码变更 → 哈希变化 → 缓存失效）
    pub fn source_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.name.hash(&mut h);
        if let Some(st) = &self.source_text {
            st.hash(&mut h);
        }
        h.finish()
    }

    /// 收集模块导出符号表（模块边界：跨模块引用）
    ///
    /// 规则：
    /// - directive.public 显式声明 → 优先采用
    /// - 未显式声明 → 收集所有顶层 FnDef/StructDef/EnumDef/TraitDef/ConstDef/TypeAliasDef
    pub fn collect_exports(&self) -> Vec<String> {
        if !self.directive.public.is_empty() {
            return self.directive.public.clone();
        }
        let mut exports = Vec::new();
        for item in &self.items {
            match item {
                node::Item::FnDef(f) => exports.push(f.name.clone()),
                node::Item::StructDef(s) => exports.push(s.name.clone()),
                node::Item::EnumDef(e) => exports.push(e.name.clone()),
                node::Item::TraitDef(t) => exports.push(t.name.clone()),
                node::Item::Const(c) => exports.push(c.name.clone()),
                node::Item::TypeAlias(t) => exports.push(t.name.clone()),
                _ => {}
            }
        }
        exports
    }

    /// 收集模块依赖（模块边界：依赖图）
    ///
    /// 来源：
    /// - directive.deps（`@deps` 指令显式声明）
    /// - UseStmt（`import` / `from ... import ...` 语句）
    pub fn collect_dependencies(&self) -> Vec<ModuleDep> {
        let mut deps: Vec<ModuleDep> = Vec::new();
        for d in &self.directive.deps {
            deps.push(ModuleDep {
                module: d.clone(),
                span: node::Span::unknown_with_file(
                    self.file_path.clone().unwrap_or_default(),
                ),
            });
        }
        for item in &self.items {
            if let node::Item::Use(u) = item {
                if let Some(first) = u.path.first() {
                    deps.push(ModuleDep {
                        module: first.clone(),
                        span: node::Span::unknown_with_file(
                            self.file_path.clone().unwrap_or_default(),
                        ),
                    });
                }
            }
        }
        deps
    }

    /// 刷新模块边界信息（exports + dependencies）
    pub fn refresh_module_boundaries(&mut self) {
        self.exports = self.collect_exports();
        self.dependencies = self.collect_dependencies();
    }

    /// Span 完备性检查：统计 IR 树中 unknown span 数量
    ///
    /// 返回 (总 span 数, unknown span 数, 无文件路径 span 数)。
    /// unknown = line==0 && col==0（宏展开/合成节点）。
    pub fn check_span_completeness(&self) -> (usize, usize, usize) {
        let mut total = 0usize;
        let mut unknown = 0usize;
        let mut no_file = 0usize;
        let mut count_span = |s: &node::Span| {
            total += 1;
            if s.is_unknown() {
                unknown += 1;
            }
            if !s.has_file() {
                no_file += 1;
            }
        };
        for item in &self.items {
            match item {
                node::Item::FnDef(f) => {
                    count_span(&f.span);
                    count_block_spans(&f.body, &mut count_span);
                }
                node::Item::StructDef(s) => {
                    count_span(&s.span);
                    for m in &s.methods {
                        count_span(&m.span);
                        count_block_spans(&m.body, &mut count_span);
                    }
                }
                node::Item::EnumDef(e) => {
                    count_span(&e.span);
                    for m in &e.methods {
                        count_span(&m.span);
                        count_block_spans(&m.body, &mut count_span);
                    }
                }
                node::Item::Impl(i) => {
                    for m in &i.methods {
                        count_span(&m.span);
                        count_block_spans(&m.body, &mut count_span);
                    }
                }
                node::Item::Test(t) => {
                    count_block_spans(&t.body, &mut count_span);
                }
                node::Item::CheckerBlock { body, .. } => {
                    count_block_spans(body, &mut count_span);
                }
                _ => {}
            }
        }
        (total, unknown, no_file)
    }

    /// JSON 序列化（开发期缓存格式，可读可 diff）
    #[cfg(feature = "infer")]
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("LZIR JSON serialize error: {e}"))
    }

    /// 从 JSON 反序列化
    #[cfg(feature = "infer")]
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| format!("LZIR JSON deserialize error: {e}"))
    }

    /// bincode 序列化（生产期缓存格式，紧凑快速）
    #[cfg(feature = "infer")]
    pub fn to_bincode(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("LZIR bincode serialize error: {e}"))
    }

    /// 从 bincode 反序列化
    #[cfg(feature = "infer")]
    pub fn from_bincode(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("LZIR bincode deserialize error: {e}"))
    }
}

/// 递归统计 Block 内所有 span（含嵌套 Block / Expr）
fn count_block_spans(block: &node::Block, count: &mut impl FnMut(&node::Span)) {
    count(&block.span);
    for stmt in &block.stmts {
        count_stmt_spans(stmt, count);
    }
}

fn count_stmt_spans(stmt: &node::Stmt, count: &mut impl FnMut(&node::Span)) {
    use node::Stmt;
    match stmt {
        Stmt::Let { value, .. } => count_expr_spans(value, count),
        Stmt::Assign { target, value } => {
            count_expr_spans(target, count);
            count_expr_spans(value, count);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                count_expr_spans(v, count);
            }
        }
        Stmt::ExprStmt { expr } => count_expr_spans(expr, count),
        Stmt::If { then_branch, else_branch, .. } => {
            count_block_spans(then_branch, count);
            if let Some(b) = else_branch {
                count_block_spans(b, count);
            }
        }
        Stmt::For { body, else_body, .. } => {
            count_block_spans(body, count);
            if let Some(b) = else_body {
                count_block_spans(b, count);
            }
        }
        Stmt::While { body, else_body, .. } => {
            count_block_spans(body, count);
            if let Some(b) = else_body {
                count_block_spans(b, count);
            }
        }
        Stmt::WhileLet { body, .. } => count_block_spans(body, count),
        Stmt::Match { arms, .. } => {
            for arm in arms {
                count_block_spans(&arm.body, count);
            }
        }
        Stmt::Raise { value } => count_expr_spans(value, count),
        Stmt::Assert { cond, message } => {
            count_expr_spans(cond, count);
            if let Some(m) = message {
                count_expr_spans(m, count);
            }
        }
        Stmt::Yield { value } => count_expr_spans(value, count),
        Stmt::YieldFrom { iter } => count_expr_spans(iter, count),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                count_expr_spans(v, count);
            }
        }
        Stmt::BlockLabel { body, .. } => count_block_spans(body, count),
        Stmt::CheckerBlock { body, .. } => count_block_spans(body, count),
        Stmt::Defer { body } => count_block_spans(body, count),
        Stmt::TryCatch { body, catches, else_body, finally_body } => {
            count_block_spans(body, count);
            for (_, b) in catches {
                count_block_spans(b, count);
            }
            if let Some(b) = else_body {
                count_block_spans(b, count);
            }
            if let Some(b) = finally_body {
                count_block_spans(b, count);
            }
        }
        Stmt::Block { stmts } => {
            for s in stmts {
                count_stmt_spans(s, count);
            }
        }
        _ => {}
    }
}

fn count_expr_spans(expr: &node::Expr, count: &mut impl FnMut(&node::Span)) {
    use node::ExprKind;
    count(&expr.span);
    match &expr.kind {
        ExprKind::Call { callee, args, .. } => {
            count_expr_spans(callee, count);
            for a in args {
                count_expr_spans(a, count);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            count_expr_spans(receiver, count);
            for a in args {
                count_expr_spans(a, count);
            }
        }
        ExprKind::FieldAccess { base, .. } => count_expr_spans(base, count),
        ExprKind::IndexGet { base, key } => {
            count_expr_spans(base, count);
            count_expr_spans(key, count);
        }
        ExprKind::IndexSet { base, key, value } => {
            count_expr_spans(base, count);
            count_expr_spans(key, count);
            count_expr_spans(value, count);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            count_expr_spans(lhs, count);
            count_expr_spans(rhs, count);
        }
        ExprKind::AssignExpr { target, value } => {
            count_expr_spans(target, count);
            count_expr_spans(value, count);
        }
        ExprKind::UnOp { operand, .. } => count_expr_spans(operand, count),
        ExprKind::IfExpr { cond, then, els } => {
            count_expr_spans(cond, count);
            count_expr_spans(then, count);
            count_expr_spans(els, count);
        }
        ExprKind::Lambda { body, .. } => count_expr_spans(body, count),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                count_expr_spans(e, count);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                count_expr_spans(a, count);
            }
        }
        ExprKind::GenExpr { yield_of } => count_expr_spans(yield_of, count),
        ExprKind::Cast { expr, .. } => count_expr_spans(expr, count),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                count_expr_spans(a, count);
            }
        }
        ExprKind::BlockExpr { block } => count_block_spans(block, count),
        ExprKind::TupleLit(es) | ExprKind::Tuple(es) | ExprKind::ListLit(es) | ExprKind::List(es) => {
            for e in es {
                count_expr_spans(e, count);
            }
        }
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs {
                count_expr_spans(k, count);
                count_expr_spans(v, count);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                count_expr_spans(s, count);
            }
            count_expr_spans(end, count);
        }
        ExprKind::Pipe { receiver, callee, args } => {
            count_expr_spans(receiver, count);
            count_expr_spans(callee, count);
            for a in args {
                count_expr_spans(a, count);
            }
        }
        ExprKind::Paren(inner) => count_expr_spans(inner, count),
        ExprKind::ImplicitConvert { source, .. } => count_expr_spans(source, count),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::macros::expand::{extract_macro_defs, MacroExpander};

    /// 编译 LZ 源码字符串 → AST → IR
    fn lz_to_ir(source: &str) -> Result<IrModule, String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();

        let (registry, _ranges) = extract_macro_defs(&tokens)
            .map_err(|e| format!("Macro error: {e}"))?;
        let expander = MacroExpander::new(registry);
        let expanded = expander.expand(&tokens)
            .map_err(|e| format!("Expand error: {e}"))?;

        let mut parser = Parser::new(expanded);
        let module = parser.parse_module()
            .map_err(|e| format!("Parse error: {e}"))?;

        builder::build_ir(&module)
            .map_err(|e| format!("IR build error: {e}"))
    }

    #[test]
    fn ir_simple_function() {
        let source = "
def add(x: int, y: int) -> int =
    x + y
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("LZIR v1"));
        assert!(text.contains("fn add"));
        assert!(text.contains("x: int"));
        assert!(text.contains("y: int"));
    }

    #[test]
    fn ir_let_binding() {
        let source = "
def demo() -> int =
    let x = 42
    let y = x + 1
    y
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("LZIR v1"));
        assert!(text.contains("fn demo"));
    }

    #[test]
    fn ir_if_else() {
        let source = "
def check_val(x: int) -> str =
    if x > 0:
        \"positive\"
    else:
        \"non-positive\"
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("fn check_val"));
    }

    #[test]
    fn ir_struct_def() {
        let source = "
struct Point =
    x: int
    y: int

def dist(p: Point) -> f64 =
    0.0
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("struct Point"));
    }
}
