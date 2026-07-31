// Lang-Zong 编译器 — scope 模块
// ──────────────────────────────────────────────

pub mod escape;  // 闭包逃逸分析
// 词法作用域追踪：嵌套作用域栈 + 变量声明记录 + 遮蔽检测
//
// 设计对标：
//   - Rust `rustc_resolve` 的作用域栈（rib stack）
//   - Zig `Scope` 结构体（编译期作用域链）
//   - 本模块零外部依赖，纯 std 实现

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════
// ScopeKind — 作用域类型
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// 函数/方法体内作用域
    Function,
    /// 普通代码块 {}
    Block,
    /// while / for 循环体
    Loop,
    /// if / elif / else 分支
    Branch,
    /// 构建块 =: / ~: / *: 闭包
    BuildBlock,
}

impl fmt::Display for ScopeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeKind::Function => write!(f, "function"),
            ScopeKind::Block => write!(f, "block"),
            ScopeKind::Loop => write!(f, "loop body"),
            ScopeKind::Branch => write!(f, "branch"),
            ScopeKind::BuildBlock => write!(f, "build block"),
        }
    }
}

// ═══════════════════════════════════════════════════════
// VarDecl — 变量声明记录
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct VarDecl {
    /// 变量名（原始 LZ 标识符）
    pub name: String,
    /// 声明所在的作用域深度（0 = 函数级，1+ = 嵌套）
    pub depth: usize,
    /// 是否可变
    pub mutable: bool,
    /// 是否是引用绑定 (ref x = ...)
    pub is_ref: bool,
    /// 是否是 comptime 编译期变量
    pub comptime: bool,
    /// 是否具有所有权（owned 关键字），允许 move 入闭包
    pub is_owned: bool,
    /// 声明时的源位置（可选，用于错误/警告报告）
    pub span: Option<crate::lexer::Span>,
}

impl VarDecl {
    pub fn new(name: impl Into<String>, depth: usize) -> Self {
        Self {
            name: name.into(),
            depth,
            mutable: false,
            is_ref: false,
            comptime: false,
            is_owned: false,
            span: None,
        }
    }

    pub fn with_mutable(mut self, m: bool) -> Self { self.mutable = m; self }
    pub fn with_ref(mut self, r: bool) -> Self { self.is_ref = r; self }
    pub fn with_comptime(mut self, c: bool) -> Self { self.comptime = c; self }
    pub fn with_owned(mut self, o: bool) -> Self { self.is_owned = o; self }
    pub fn with_span(mut self, s: crate::lexer::Span) -> Self { self.span = Some(s); self }
}

// ═══════════════════════════════════════════════════════
// Scope — 单个作用域层
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct Scope {
    kind: ScopeKind,
    depth: usize,
    /// 本作用域内声明的变量名 → 变量信息
    vars: HashMap<String, VarDecl>,
}

impl Scope {
    fn new(kind: ScopeKind, depth: usize) -> Self {
        Self { kind, depth, vars: HashMap::new() }
    }

    fn declare(&mut self, decl: VarDecl) -> Result<(), ScopeError> {
        if self.vars.contains_key(&decl.name) {
            return Err(ScopeError::duplicate(&decl.name, self.kind));
        }
        self.vars.insert(decl.name.clone(), decl);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&VarDecl> {
        self.vars.get(name)
    }

    fn names(&self) -> impl Iterator<Item = &String> {
        self.vars.keys()
    }
}

// ═══════════════════════════════════════════════════════
// ScopeStack — 嵌套作用域栈
// ═══════════════════════════════════════════════════════

/// 词法作用域栈。
/// 从栈底到栈顶依次为：函数作用域 → block → loop → ...
/// 变量查找从栈顶向下逐层搜索。
#[derive(Debug, Clone)]
pub struct ScopeStack {
    /// 作用域栈（栈底=函数级）
    scopes: Vec<Scope>,
    /// 全局深度计数器
    depth_counter: usize,
    /// 遮蔽警告开关
    warn_shadow: bool,
}

impl ScopeStack {
    /// 新建空作用域栈（无任何作用域）
    pub fn new() -> Self {
        Self { scopes: Vec::new(), depth_counter: 0, warn_shadow: true }
    }

    /// 以函数级作用域初始化
    pub fn new_function() -> Self {
        let mut ss = Self::new();
        ss.scopes.push(Scope::new(ScopeKind::Function, 0));
        ss
    }

    /// 初始化函数作用域并批量注入参数
    pub fn new_function_with_params(
        params: impl IntoIterator<Item = (String, bool)>, // (name, mutable)
    ) -> Self {
        let mut ss = Self::new_function();
        for (name, mutable) in params {
            let decl = VarDecl::new(&name, 0).with_mutable(mutable);
            let _ = ss.scopes[0].declare(decl); // 参数不会重复
        }
        ss
    }

    // ── 作用域进出 ──

    /// 进入新作用域（如 while / for / if 分支）
    pub fn push(&mut self, kind: ScopeKind) {
        self.depth_counter += 1;
        self.scopes.push(Scope::new(kind, self.depth_counter));
    }

    /// 退出当前作用域
    pub fn pop(&mut self) {
        if self.scopes.len() <= 1 {
            // 不允许弹出函数级作用域
            return;
        }
        self.scopes.pop();
    }

    /// 作用域深度（0 = 函数级）
    pub fn depth(&self) -> usize {
        self.scopes.len().saturating_sub(1)
    }

    /// 当前作用域类型
    pub fn current_kind(&self) -> Option<ScopeKind> {
        self.scopes.last().map(|s| s.kind)
    }

    // ── 变量声明 ──

    /// 在当前作用域声明变量。
    /// 如果与上层作用域同名且开启 warn_shadow，返回警告但声明成功；同层重复声明返回错误。
    pub fn declare(&mut self, decl: VarDecl) -> Result<Option<ScopeWarning>, ScopeError> {
        let name = decl.name.clone();

        // 检查同层重复（不可变借用）
        if self.scopes.last().map(|s| s.get(&name).is_some()).unwrap_or(false) {
            let kind = self.scopes.last().unwrap().kind;
            return Err(ScopeError::duplicate(&name, kind));
        }

        // 检查遮蔽（在获取可变引用之前完成不可变借用）
        let shadow = if self.warn_shadow {
            self.lookup_from_below(&name).map(|existing| {
                let current_depth = self.scopes.len().saturating_sub(1);
                ScopeWarning::shadow(&name, existing.depth, current_depth)
            })
        } else {
            None
        };

        // 获取可变引用并声明
        let current = self.scopes.last_mut()
            .ok_or_else(|| ScopeError::no_scope(&name))?;
        current.declare(decl)?;
        Ok(shadow)
    }

    /// 在当前作用域中快速声明（忽略遮蔽警告）
    pub fn declare_quiet(&mut self, name: &str) -> Result<(), ScopeError> {
        self.declare(VarDecl::new(name, self.depth())).map(|_| ())
    }

    /// 检查变量是否已在当前作用域存在
    pub fn contains_in_current(&self, name: &str) -> bool {
        self.scopes.last().map(|s| s.get(name).is_some()).unwrap_or(false)
    }

    // ── 变量查找 ──

    /// 从当前作用域向上查找变量，返回声明信息
    pub fn lookup(&self, name: &str) -> Option<&VarDecl> {
        for scope in self.scopes.iter().rev() {
            if let Some(decl) = scope.get(name) {
                return Some(decl);
            }
        }
        None
    }

    /// 从当前作用域以下一层开始查找（用于遮蔽检测，不包含当前层）
    fn lookup_from_below(&self, name: &str) -> Option<&VarDecl> {
        // 跳过最顶层（当前作用域），在以下各层查找
        let len = self.scopes.len();
        if len < 2 { return None; }
        for scope in self.scopes[..len - 1].iter().rev() {
            if let Some(decl) = scope.get(name) {
                return Some(decl);
            }
        }
        None
    }

    /// 检查是否存在变量（任意作用域）
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// 变量声明的深度。0 = 当前作用域，越大越外层
    pub fn declaration_depth(&self, name: &str) -> Option<usize> {
        self.lookup(name).map(|d| d.depth)
    }

    // ── 逃逸检测 ──

    /// 检测变量是否会"逃逸"到外层——即在当前作用域声明，但在外层被使用。
    /// 这目前仅做告警，不阻止代码生成（Rust 编译器最终会检查）。
    pub fn check_escape(&self, name: &str) -> Option<ScopeWarning> {
        let decl = self.lookup(name)?;
        let current_depth = self.depth();
        if decl.depth == current_depth && self.scopes.len() > 1 {
            Some(ScopeWarning::escape(name, decl.depth))
        } else {
            None
        }
    }

    /// 闭包逃逸检查：给定被捕获的变量列表，返回没有所有权声明、不应被捕获的变量。
    /// 只有 `owned` 声明的局部变量允许被逃逸闭包捕获（move 语义）。
    /// 参数、模块级变量天然允许（生命周期够长）。
    ///
    /// 返回 Vec<String> = 缺失 `owned` 的捕获变量名。
    pub fn check_closure_escapes(&self, captures: &[String]) -> Vec<String> {
        let mut violations = Vec::new();
        let fn_depth = 0; // 函数级作用域深度

        for name in captures {
            if let Some(decl) = self.lookup(name) {
                // 规则 1: 函数参数（depth==0 且是最外层）→ 允许（生命周期够长）
                // 规则 2: 模块级/全局 → 允许（静态生命周期）
                // 规则 3: computed/捕获的变量 → 需要 owned
                if decl.depth > fn_depth && !decl.is_owned {
                    // 局部非 owned 变量被逃逸闭包捕获 → 违规
                    violations.push(name.clone());
                }
            }
        }
        violations
    }

    // ── 遍历 ──

    /// 当前作用域内的所有变量名
    pub fn names_in_current(&self) -> impl Iterator<Item = &String> {
        self.scopes.last().into_iter().flat_map(|s| s.names())
    }

    /// 所有可见的变量名（当前作用域 + 所有外层）
    pub fn names_visible(&self) -> impl Iterator<Item = &String> {
        self.scopes.iter().rev().flat_map(|s| s.names())
    }

    /// 所有已声明的变量（去重，内层优先）
    pub fn all_vars(&self) -> Vec<&VarDecl> {
        let mut seen = HashMap::new();
        for scope in self.scopes.iter().rev() {
            for (name, decl) in &scope.vars {
                seen.entry(name).or_insert(decl);
            }
        }
        seen.into_values().collect()
    }

    /// 当前作用域栈高度
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// 启用/禁用遮蔽警告
    pub fn set_warn_shadow(&mut self, enabled: bool) {
        self.warn_shadow = enabled;
    }
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new_function()
    }
}

impl fmt::Display for ScopeStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, scope) in self.scopes.iter().enumerate() {
            let indent = "  ".repeat(i);
            writeln!(f, "{}Scope {} ({:?}):", indent, scope.depth, scope.kind)?;
            for name in scope.names() {
                let decl = scope.get(name).unwrap();
                let extra = match (decl.mutable, decl.is_ref, decl.comptime) {
                    (true, false, false) => " (mut)".to_string(),
                    (false, true, false) => " (ref)".to_string(),
                    (false, false, true) => " (comptime)".to_string(),
                    _ => String::new(),
                };
                writeln!(f, "{}  - {}{}", indent, name, extra)?;
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════
// ScopeError — 作用域错误
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum ScopeError {
    /// 同作用域重复声明
    Duplicate { name: String, kind: ScopeKind },
    /// 不在任何作用域内声明
    NoScope { name: String },
}

impl ScopeError {
    pub fn duplicate(name: &str, kind: ScopeKind) -> Self {
        Self::Duplicate { name: name.to_string(), kind }
    }
    pub fn no_scope(name: &str) -> Self {
        Self::NoScope { name: name.to_string() }
    }
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeError::Duplicate { name, kind } => {
                write!(f, "duplicate variable '{}' in same {} scope", name, kind)
            }
            ScopeError::NoScope { name } => {
                write!(f, "cannot declare '{}': no active scope", name)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════
// ScopeWarning — 作用域警告
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum ScopeWarning {
    /// 变量遮蔽：内层声明了与外层同名的变量
    Shadow { name: String, outer_depth: usize, inner_depth: usize },
    /// 潜在逃逸：在非函数级作用域声明的变量可能逃逸
    Escape { name: String, depth: usize },
}

impl ScopeWarning {
    pub fn shadow(name: &str, outer_depth: usize, inner_depth: usize) -> Self {
        Self::Shadow { name: name.to_string(), outer_depth, inner_depth }
    }
    pub fn escape(name: &str, depth: usize) -> Self {
        Self::Escape { name: name.to_string(), depth }
    }
}

impl fmt::Display for ScopeWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeWarning::Shadow { name, outer_depth, inner_depth } => {
                write!(f, "variable '{}' shadows declaration at depth {} (declared at depth {})",
                    name, outer_depth, inner_depth)
            }
            ScopeWarning::Escape { name, depth } => {
                write!(f, "variable '{}' declared at depth {} may escape its scope", name, depth)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════
// CaptureSet — 闭包捕获集
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// 按值捕获（move 语义）
    ByValue,
    /// 按引用捕获
    ByRef,
}

#[derive(Debug, Clone, Default)]
pub struct CaptureSet {
    captures: HashMap<String, CaptureMode>,
}

impl CaptureSet {
    pub fn new() -> Self {
        Self { captures: HashMap::new() }
    }

    /// 记录一个捕获变量
    pub fn capture(&mut self, name: &str, mode: CaptureMode) {
        // 如果已经以 ByValue 捕获，不降级为 ByRef
        if let Some(existing) = self.captures.get(name) {
            if *existing == CaptureMode::ByValue { return; }
        }
        self.captures.insert(name.to_string(), mode);
    }

    /// 收集表达式中引用的外部变量（相对于给定的作用域栈）
    pub fn collect_from_expr(&mut self, expr: &crate::ast::Expr, scope: &ScopeStack) {
        self.collect_expr_impl(expr, scope)
    }

    fn collect_expr_impl(&mut self, expr: &crate::ast::Expr, scope: &ScopeStack) {
        use crate::ast::Expr;
        match expr {
            Expr::Ident(name) => {
                // 不追踪函数名和模块路径（它们不是"捕获"）
                if scope.contains(name) {
                    self.capture(name, CaptureMode::ByRef);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.collect_expr_impl(left, scope);
                self.collect_expr_impl(right, scope);
            }
            Expr::Unary { operand, .. } => {
                self.collect_expr_impl(operand, scope);
            }
            Expr::Call { func, args, .. } => {
                self.collect_expr_impl(func, scope);
                for arg in args { self.collect_expr_impl(arg, scope); }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.collect_expr_impl(receiver, scope);
                for arg in args { self.collect_expr_impl(arg, scope); }
            }
            Expr::FieldAccess { receiver, .. } | Expr::PathAccess { receiver, .. } | Expr::Index { receiver, .. } => {
                self.collect_expr_impl(receiver, scope);
            }
            Expr::ListLit(elems) | Expr::TupleLit(elems) | Expr::SetLit(elems) => {
                for e in elems {
                    self.collect_expr_impl(e, scope);
                }
            }
            Expr::DictLit(pairs) => {
                for (k, v) in pairs {
                    self.collect_expr_impl(k, scope);
                    self.collect_expr_impl(v, scope);
                }
            }
            Expr::If { cond, then_body, elif_clauses, else_body } => {
                self.collect_expr_impl(cond, scope);
                for s in then_body { self.collect_stmt_impl(s, scope); }
                for (c, body) in elif_clauses {
                    self.collect_expr_impl(c, scope);
                    for s in body { self.collect_stmt_impl(s, scope); }
                }
                if let Some(body) = else_body {
                    for s in body { self.collect_stmt_impl(s, scope); }
                }
            }
            Expr::Match { expr: matched, arms } => {
                self.collect_expr_impl(matched, scope);
                for arm in arms {
                    if let Some(g) = &arm.guard { self.collect_expr_impl(g, scope); }
                    for s in &arm.body { self.collect_stmt_impl(s, scope); }
                }
            }
            _ => {}
        }
    }

    /// 收集语句块中引用的外部变量
    pub fn collect_from_stmts(&mut self, stmts: &[crate::ast::Stmt], scope: &ScopeStack) {
        for stmt in stmts {
            self.collect_stmt_impl(stmt, scope);
        }
    }

    fn collect_stmt_impl(&mut self, stmt: &crate::ast::Stmt, scope: &ScopeStack) {
        use crate::ast::Stmt;
        match stmt {
            Stmt::Expr(e) => self.collect_expr_impl(e, scope),
            Stmt::Let { value, .. } | Stmt::Const { value, .. } => self.collect_expr_impl(value, scope),
            Stmt::Assign { target, value, .. } => {
                self.collect_expr_impl(target, scope);
                self.collect_expr_impl(value, scope);
            }
            Stmt::Return(Some(e)) => self.collect_expr_impl(e, scope),
            Stmt::Yield(Some(e)) => self.collect_expr_impl(e, scope),
            Stmt::YieldFrom { expr, transform } => {
                self.collect_expr_impl(expr, scope);
                if let Some(f) = transform { self.collect_expr_impl(f, scope); }
            }
            Stmt::Raise(e) => self.collect_expr_impl(e, scope),
            Stmt::While { cond, body, .. } => {
                self.collect_expr_impl(cond, scope);
                for s in body { self.collect_stmt_impl(s, scope); }
            }
            Stmt::For { iter, body, .. } => {
                self.collect_expr_impl(iter, scope);
                for s in body { self.collect_stmt_impl(s, scope); }
            }
            _ => {}
        }
    }

    pub fn is_empty(&self) -> bool { self.captures.is_empty() }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &CaptureMode)> {
        self.captures.iter()
    }

    /// 生成 Rust 闭包的 capture 子句（如 "move "）
    /// 只要有任何 ByValue 捕获，就返回 "move "，否则 ""
    pub fn rust_move_prefix(&self) -> &'static str {
        if self.captures.values().any(|m| *m == CaptureMode::ByValue) {
            "move "
        } else {
            ""
        }
    }
}

// ═══════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_basic_declare_lookup() {
        let mut ss = ScopeStack::new_function();
        ss.declare_quiet("x").unwrap();
        ss.declare_quiet("y").unwrap();
        assert!(ss.contains("x"));
        assert!(ss.contains("y"));
        assert!(!ss.contains("z"));
        assert_eq!(ss.lookup("x").unwrap().depth, 0);
    }

    #[test]
    fn test_scope_nested_lookup() {
        let mut ss = ScopeStack::new_function();
        ss.declare_quiet("outer").unwrap();

        ss.push(ScopeKind::Block);
        ss.declare_quiet("inner").unwrap();

        // 外层可见
        assert!(ss.contains("outer"));
        // 内层可见
        assert!(ss.contains("inner"));

        ss.pop();

        // 退出内层后，inner 不再可见
        assert!(!ss.contains("inner"));
        assert!(ss.contains("outer"));
    }

    #[test]
    fn test_scope_duplicate_error() {
        let mut ss = ScopeStack::new_function();
        ss.declare_quiet("x").unwrap();
        let err = ss.declare_quiet("x").unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn test_scope_shadow_warning() {
        let mut ss = ScopeStack::new_function();
        ss.declare_quiet("x").unwrap();

        ss.push(ScopeKind::Block);
        let warn = ss.declare(VarDecl::new("x", 1)).unwrap();
        assert!(warn.is_some());
        let w = warn.unwrap();
        match w {
            ScopeWarning::Shadow { ref name, .. } => assert_eq!(name, "x"),
            _ => panic!("expected shadow warning"),
        }

        ss.pop();
        // 外层 x 仍在
        assert!(ss.contains("x"));
    }

    #[test]
    fn test_scope_no_shadow_when_disabled() {
        let mut ss = ScopeStack::new_function();
        ss.set_warn_shadow(false);
        ss.declare_quiet("x").unwrap();

        ss.push(ScopeKind::Block);
        let warn = ss.declare(VarDecl::new("x", 1)).unwrap();
        assert!(warn.is_none());
    }

    #[test]
    fn test_scope_nested_depth_tracking() {
        let mut ss = ScopeStack::new_function();
        assert_eq!(ss.depth(), 0);

        ss.push(ScopeKind::Loop);
        assert_eq!(ss.depth(), 1);
        ss.declare_quiet("i").unwrap();
        assert_eq!(ss.lookup("i").unwrap().depth, 1);

        ss.push(ScopeKind::Branch);
        assert_eq!(ss.depth(), 2);
        ss.declare_quiet("j").unwrap();
        assert_eq!(ss.lookup("j").unwrap().depth, 2);

        ss.pop();
        assert_eq!(ss.depth(), 1);
        assert!(!ss.contains("j"));
        assert!(ss.contains("i"));
    }

    #[test]
    fn test_capture_set_basic() {
        let mut ss = ScopeStack::new_function();
        ss.declare_quiet("x").unwrap();
        ss.declare_quiet("y").unwrap();

        let mut cs = CaptureSet::new();
        use crate::ast::Expr;
        let expr = Expr::Binary {
            left: Box::new(Expr::Ident("x".into())),
            op: crate::ast::expr::BinOp::Add,
            right: Box::new(Expr::Ident("y".into())),
        };
        cs.collect_from_expr(&expr, &ss);

        assert!(cs.captures.contains_key("x"));
        assert!(cs.captures.contains_key("y"));
    }

    #[test]
    fn test_scope_function_params() {
        let ss = ScopeStack::new_function_with_params(vec![
            ("a".into(), false),
            ("b".into(), true),
        ]);
        assert!(ss.contains("a"));
        assert!(ss.contains("b"));
        assert_eq!(ss.lookup("a").unwrap().mutable, false);
        assert_eq!(ss.lookup("b").unwrap().mutable, true);
    }
}
