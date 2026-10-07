// Lang-Zone 编译器 — ir/codegen/stmt_gen.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::types_emit::is_bigint_ty;
use super::types_emit::is_complex_ty;
use super::*;

impl CodeGen {
    pub(crate) fn gen_param(&self, p: &Param) -> String {
        if p.name == "self" {
            // self → &self / &mut self / self 取决于 is_mut + ty ref修饰
            match (&p.ty, p.is_mut) {
                (IrType::Self_, true) => "&mut self".into(),
                (IrType::Self_, false) => "&self".into(),
                (IrType::MutRef(_), _) => "&mut self".into(),
                (IrType::Ref(_), _) => "&self".into(),
                _ => {
                    // Fallback: treat any self param as &self (LZ semantics: self is borrowed by default)
                    if p.is_mut { "&mut self" } else { "&self" }.into()
                }
            }
        } else {
            // duck 类型参数 — 代码生成层用 `_` 占位，语义校验在编译期完成
            // 实际 Rust 输出不包含 duck 字段约束
            if matches!(&p.ty, IrType::Duck { .. }) {
                format!("{}: T_DUCK_{}", p.name, p.name.to_uppercase())
            } else if matches!(&p.ty, IrType::Any) {
                // Any 类型参数省略类型注解，让 Rust 从上下文推断（用于 map/filter 闭包）
                p.name.clone()
            } else if p.is_ref {
                // ref x: T → &T（不可变引用）；mut ref x: T → &mut T（可变引用）
                // 特殊处理 str 类型：生成 &str 而非 &String
                // p.ty 可能是 Str（builder 标记 is_ref 但保持 ty 为 Str）或 Ref(Str)

                let is_str_ty = matches!(&p.ty, IrType::Str)
                    || matches!(&p.ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str));
                if is_str_ty {
                    format!("{}: &str", p.name)
                } else if p.is_mut {
                    format!("{}: &mut {}", p.name, self.rust_type(&p.ty))
                } else {
                    format!("{}: &{}", p.name, self.rust_type(&p.ty))
                }
            } else {
                format!("{}: {}", p.name, self.rust_type(&p.ty))
            }
        }
    }

    // ── Block / Stmt 生成 ──

    /// BUG-CG-004（轮次12）：将 LZ Pattern 转为 Rust 模式串，用于 try/catch 结果基
    /// 分支的 `match __err_val { <pattern> => ... }`。支持 Wildcard / Ident /
    /// Enum（递归 args）。`line`/`column`/`file` 与 Rust 内置宏同名 → 降级加下划线。
    pub(crate) fn pattern_to_rust_pat(&self, p: &Pattern) -> String {
        match p {
            Pattern::Wildcard => "_".to_string(),
            Pattern::Ident(name) => {
                if matches!(name.as_str(), "line" | "column" | "file") {
                    format!("{}_", name)
                } else {
                    name.clone()
                }
            }
            Pattern::Enum {
                enum_name,
                variant,
                args,
            } => {
                let args_s: Vec<String> =
                    args.iter().map(|a| self.pattern_to_rust_pat(a)).collect();
                format!("{}::{}(", enum_name, variant) + &args_s.join(", ") + ")"
            }
            _ => "_".to_string(),
        }
    }

    /// 递归收集 Pattern 中所有绑定的标识符名（含 Enum 变体参数里的 Ident），
    /// 用于把 line/column/file 这类与 Rust 内置宏同名的绑定登记进 downgraded_vars，
    /// 使 body 里的引用同步改名，避免 E0423（裸 `line` 被解析成 `line!` 宏）。
    pub(crate) fn pattern_idents(&self, p: &Pattern) -> Vec<String> {
        match p {
            Pattern::Ident(name) => vec![name.clone()],
            Pattern::Enum { args, .. } => {
                args.iter().flat_map(|a| self.pattern_idents(a)).collect()
            }
            _ => Vec::new(),
        }
    }

    pub(crate) fn gen_block_inner(&mut self, block: &Block) {
        // 块级 defer 收集：接管外层 pending，本块内的 defer 体在循环后逆序 emit。
        let saved_deferred = std::mem::take(&mut self.deferred);
        // 块内含 defer 时，尾语句不得包 `return`——否则 cleanup 会落在 return 之后
        // 成为不可达 dead code。cleanup 在块末尾（尾语句之后）逆序执行，故尾语句
        // 按普通语句 emit、整块正常落到结尾返回。
        let has_defer = block.stmts.iter().any(|s| matches!(s, Stmt::Defer { .. }));
        let saved_suppress = self.suppress_tail_return;
        let saved_semi = self.force_stmt_semicolon;
        // 外层块的尾值捕获标记不得泄漏进嵌套块（嵌套块尾语句误捕获 →
        // return 落在 if 分支内、外层标记被提前 take → 外层丢尾值）
        let saved_pending = std::mem::take(&mut self.pending_tail_capture);
        // 函数体（非闭包/块值上下文）内含 defer 且尾语句是值语句（ExprStmt/TryCatch）
        // 时：尾值先捕获到临时变量，flush defer cleanup 后再 return —— 否则尾值被
        // force_stmt_semicolon 丢弃、cleanup 落在块尾 → 函数落尾返回 ()
        // （combo-defer-guard / combo_defer_guard_try 等 E0308 家族）。
        let capture_tail = has_defer
            && !self.in_lambda_block
            && !self.suppress_tail_return
            && self.current_fn_raises.is_none()
            && !self.is_main
            && self
                .current_ret_ty
                .as_ref()
                .map_or(false, |t| !matches!(t, IrType::Unit))
            && matches!(block.stmts.last(), Some(Stmt::ExprStmt { .. }));
        if has_defer {
            self.suppress_tail_return = true;
            self.force_stmt_semicolon = true;
        }
        if capture_tail {
            self.pending_tail_capture = Some("__tail_val_1".to_string());
        }
        let n = block.stmts.len();
        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == n - 1;
            self.gen_stmt(stmt, is_last);
        }
        self.flush_deferred();
        if let Some(tv) = self.pending_tail_capture.take() {
            self.emit_line(&format!("return {};", tv));
        }
        self.suppress_tail_return = saved_suppress;
        self.force_stmt_semicolon = saved_semi;
        self.deferred = saved_deferred;
        self.pending_tail_capture = saved_pending;
    }

    /// 在块结尾逆序（LIFO）emit 已收集的 defer 体（BUG-IR-002 方案 A 内联脱糖）。
    pub(crate) fn flush_deferred(&mut self) {
        let defs = std::mem::take(&mut self.deferred);
        for blk in defs.into_iter().rev() {
            for s in &blk.stmts {
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.gen_stmt(s, false);
                self.suppress_tail_return = saved;
            }
        }
    }

    // ════════════════════════════════════════════════════════════════
    // 修饰符装饰器新语义轴 → Rust 目标类型（T04）
    //
    // 设计真值：workbuddy/plan/2026-09-14-moddec-架构设计.md §1.4。
    // 组合约定（与 `moddec_emit` 一致）：**内部可变轴在内、共享轴在外**
    // （`Arc<Mutex<T>>` / `Arc<AtomicI64>`）；惰性轴走 `OnceCell`/`OnceLock`。
    // ════════════════════════════════════════════════════════════════

    /// 预扫描整个模块，登记修饰符装饰器目标类型所需的 std 导入项与原子类型。
    ///
    /// 仅在模块实际使用共享 / 内部可变 / 惰性轴时登记；未使用时**不产生任何导入**，
    /// 从而保证无装饰器模块的产物逐字节不变（L2/L3 golden 冻结比对）。
    pub(crate) fn collect_moddec_imports(&mut self, module: &IrModule) {
        self.moddec_std_imports.clear();
        self.moddec_atomic_types.clear();
        for item in &module.items {
            match item {
                Item::Const(c) => {
                    let base = self.rust_type(&c.ty);
                    // 顶层常量经 `gen_const_def` 发射：惰性轴用**完全限定**的
                    // `std::sync::LazyLock`（无需导入），故此处不登记惰性导入。
                    self.note_moddec_imports(&c.mods, &base, false);
                }
                Item::FnDef(f) => self.note_moddec_imports_in_block(&f.body),
                Item::Impl(i) => {
                    for m in &i.methods {
                        self.note_moddec_imports_in_block(&m.body);
                    }
                }
                Item::Test(t) => self.note_moddec_imports_in_block(&t.body),
                Item::CheckerBlock { body, .. } => self.note_moddec_imports_in_block(body),
                _ => {}
            }
        }
    }

    /// 递归遍历语句块，登记其中 let 绑定的修饰轴所需导入。
    pub(crate) fn note_moddec_imports_in_block(&mut self, block: &Block) {
        self.note_moddec_imports_in_stmts(&block.stmts);
    }

    /// 递归遍历语句序列（覆盖块 / 分支 / 循环 / match 臂 / try-catch 等嵌套）。
    pub(crate) fn note_moddec_imports_in_stmts(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            match s {
                Stmt::Let {
                    ty, mods, is_ref, ..
                } => {
                    // `ref` 绑定走独立（`&x`）发射路径，不做包装 → 无需登记导入。
                    if !*is_ref {
                        let base = self.rust_type(ty);
                        self.note_moddec_imports(mods, &base, true);
                    }
                }
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.note_moddec_imports_in_block(then_branch);
                    if let Some(e) = else_branch {
                        self.note_moddec_imports_in_block(e);
                    }
                }
                Stmt::For {
                    body, else_body, ..
                }
                | Stmt::While {
                    body, else_body, ..
                } => {
                    self.note_moddec_imports_in_block(body);
                    if let Some(e) = else_body {
                        self.note_moddec_imports_in_block(e);
                    }
                }
                Stmt::WhileLet { body, .. }
                | Stmt::BlockLabel { body, .. }
                | Stmt::Defer { body }
                | Stmt::CheckerBlock { body, .. } => self.note_moddec_imports_in_block(body),
                Stmt::Match { arms, .. } => {
                    for a in arms {
                        self.note_moddec_imports_in_block(&a.body);
                    }
                }
                Stmt::Block { stmts } => self.note_moddec_imports_in_stmts(stmts),
                Stmt::TryCatch {
                    body,
                    catches,
                    else_body,
                    finally_body,
                } => {
                    self.note_moddec_imports_in_block(body);
                    for (_p, b) in catches {
                        self.note_moddec_imports_in_block(b);
                    }
                    if let Some(e) = else_body {
                        self.note_moddec_imports_in_block(e);
                    }
                    if let Some(f) = finally_body {
                        self.note_moddec_imports_in_block(f);
                    }
                }
                _ => {}
            }
        }
    }

    /// 登记单个修饰轴集合所需 std 导入（无包装轴时不登记）。
    ///
    /// `base_ty` 为内层基础 Rust 类型串（用于 `@atomic` 的具体标量映射）；
    /// `lazy_imports` 为是否登记惰性容器导入（顶层常量用完全限定 `LazyLock`，传 `false`）。
    pub(crate) fn note_moddec_imports(&mut self, mods: &IrMods, base_ty: &str, lazy_imports: bool) {
        if !mods.needs_wrapper() {
            return;
        }
        // 惰性轴：局部 → `std::cell::OnceCell`；线程安全 → `std::sync::OnceLock`。
        if mods.lazy && lazy_imports {
            let (module, item): (&'static str, &'static str) = match mods.origin.as_deref() {
                Some("once") | Some("lazy_static") => ("std::sync", "OnceLock"),
                _ => ("std::cell", "OnceCell"),
            };
            if !self.known_types.contains(item) {
                self.moddec_std_imports
                    .entry(module)
                    .or_default()
                    .insert(item);
            }
        }
        // 内部可变轴。
        match mods.interior {
            InteriorMode::Cell => {
                if !self.known_types.contains("Cell") {
                    self.moddec_std_imports
                        .entry("std::cell")
                        .or_default()
                        .insert("Cell");
                }
            }
            InteriorMode::RefCell => {
                if !self.known_types.contains("RefCell") {
                    self.moddec_std_imports
                        .entry("std::cell")
                        .or_default()
                        .insert("RefCell");
                }
            }
            InteriorMode::Mutex => {
                if !self.known_types.contains("Mutex") {
                    self.moddec_std_imports
                        .entry("std::sync")
                        .or_default()
                        .insert("Mutex");
                }
            }
            InteriorMode::RwLock => {
                if !self.known_types.contains("RwLock") {
                    self.moddec_std_imports
                        .entry("std::sync")
                        .or_default()
                        .insert("RwLock");
                }
            }
            InteriorMode::Atomic => {
                // 仅登记具体标量原子类型（非标量占位 `Atomic<T>` 交由下游 rustc 报错）。
                let atom = moddec_emit::atomic_type(base_ty);
                if atom.starts_with("Atomic")
                    && !atom.contains('<')
                    && !self.known_types.contains(&atom)
                {
                    self.moddec_atomic_types.insert(atom);
                }
            }
            InteriorMode::None => {}
        }
        // 共享轴：`Rc` / `Arc` 已由 prelude 无条件导入，仅 `Weak` 需显式导入。
        if mods.shared == SharedMode::Weak && !self.known_types.contains("Weak") {
            self.moddec_std_imports
                .entry("std::rc")
                .or_default()
                .insert("Weak");
        }
    }

    /// 计算 let 绑定在新语义轴下的 Rust 目标类型与初值包装（T04）。
    ///
    /// 返回 `Some((目标类型, 初值表达式))`；`mods` 无包装轴时返回 `None`，
    /// 调用方保持原有（无包装）发射逻辑，实现零回归。
    ///
    /// - 惰性轴（`@lazy` / `@lazy_mut` / `@once` / `@lazy_static`）：`OnceCell` / `OnceLock`；
    ///   `@lazy`/`@lazy_mut` 走**延迟**求值（绑定处不求值，登记 [`Self::lazy_bindings`]），
    ///   `@once`/`@lazy_static` 走即时 `OnceLock::from`。惰性容器自身即一次性线程安全单元，
    ///   故共享轴并入其中（不再额外包 `Arc`），与 QA L4「按 origin 派发」契约一致。
    /// - 其余：内部可变轴在内、共享轴在外（`Arc<Mutex<T>>` / `Arc<AtomicI64>`）。
    /// - `base_ty == None`（未知基础类型）→ 内层用 `_` 占位，交由 Rust 从初值推断。
    ///
    /// `lz_name` 为源码变量名（访问点 `ExprKind::Var` 按此名拦截），
    /// `safe_name` 为发射用的安全 Rust 标识符（可能因关键字降级 / 冲突重命名而不同）。
    pub(crate) fn moddec_wrap_let(
        &mut self,
        lz_name: &str,
        safe_name: &str,
        base_ty: Option<&str>,
        value_s: &str,
        mods: &IrMods,
    ) -> Option<(String, String)> {
        if !mods.needs_wrapper() {
            return None;
        }
        let inner_ty = base_ty.unwrap_or("_");

        // ── 惰性轴（优先于共享/内部可变轴）──
        if mods.lazy {
            let lazy_ty = moddec_emit::wrap_lazy(inner_ty, mods.origin.as_deref());
            let ctor = match mods.origin.as_deref() {
                Some("once") | Some("lazy_static") => "OnceLock",
                _ => "OnceCell",
            };
            let value = if mods.is_deferred_lazy() {
                // 延迟惰性：绑定处不求值（AC4），登记首次访问求值表达式。
                self.lazy_bindings.insert(
                    lz_name.to_string(),
                    (safe_name.to_string(), value_s.to_string()),
                );
                format!("{ctor}::new()")
            } else {
                format!("{ctor}::from({value_s})")
            };
            return Some((lazy_ty, value));
        }

        // ── 内部可变轴（内）→ 共享轴（外）──
        let interior_ty = moddec_emit::wrap_interior(inner_ty, mods.interior);
        let target_ty = moddec_emit::wrap_shared(&interior_ty, mods.shared);

        let inner_val = match mods.interior {
            InteriorMode::None => value_s.to_string(),
            InteriorMode::Cell => format!("Cell::new({value_s})"),
            InteriorMode::RefCell => format!("RefCell::new({value_s})"),
            InteriorMode::Mutex => format!("Mutex::new({value_s})"),
            InteriorMode::RwLock => format!("RwLock::new({value_s})"),
            InteriorMode::Atomic => {
                format!("{}::new({value_s})", moddec_emit::atomic_type(inner_ty))
            }
        };
        let value = match mods.shared {
            SharedMode::None => inner_val,
            SharedMode::Rc => format!("Rc::new({inner_val})"),
            SharedMode::Arc => format!("Arc::new({inner_val})"),
            // `Weak` 无「由值直接构造」的 API：生成空 `Weak`（编译期合法；
            // 运行期 `upgrade()` 返回 `None`，语义由调用方管理所有权后补齐）。
            SharedMode::Weak => "Weak::new()".to_string(),
        };
        Some((target_ty, value))
    }

    pub(crate) fn gen_stmt(&mut self, stmt: &Stmt, is_last: bool) {
        self.emit_line(&format!(
            "// STMT:{}",
            if matches!(stmt, Stmt::Let { .. }) {
                "Let"
            } else if matches!(stmt, Stmt::ExprStmt { .. }) {
                "Expr"
            } else if matches!(stmt, Stmt::TryCatch { .. }) {
                "Try"
            } else if matches!(stmt, Stmt::Block { .. }) {
                "Block"
            } else if matches!(stmt, Stmt::Defer { .. }) {
                "Defer"
            } else if matches!(stmt, Stmt::If { .. }) {
                "If"
            } else if matches!(stmt, Stmt::For { .. }) {
                "For"
            } else {
                "Other"
            }
        ));
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                is_mut,
                is_ref,
                mods,
            } => {
                // IR-003：嵌套 def 转本地闭包产物 `let name = <Lambda>`（ty=Fn 且值
                // 为 Lambda）→ 值位置，需 Arc<dyn Fn … + Send + Sync> 标注并装箱
                let is_fn_lambda_let = matches!(ty, IrType::Fn { .. })
                    && matches!(value.kind, ExprKind::Lambda { .. });
                // 关键字降级变量（Ok/Some/None/Err 用作变量名）：注册并重命名为 name_
                // line/column/file 与 Rust 内置宏（line!/column!/file!）冲突，同样降级
                if matches!(
                    name.as_str(),
                    "Ok" | "Some" | "None" | "Err" | "line" | "column" | "file"
                ) {
                    self.downgraded_vars.insert(name.clone());
                }
                // 模块级全局变量：不生成局部 let，改为 unsafe 赋值（全局已 static mut 声明）
                if self.global_vars.contains_key(name.as_str()) {
                    self.emit_line(&format!(
                        "unsafe {{ {} = {}; }}",
                        name,
                        self.gen_expr(value)
                    ));
                    return;
                }
                // LZ: Let{is_mut:true} = 无 let 关键字的赋值
                //   - 首次出现: "let mut x = val"
                //   - 已声明过: "x = val"（纯赋值）
                // 生成安全的变量名：处理关键字降级 + 模块级 static 冲突（E0530）
                let safe_name = if self.downgraded_vars.contains(name.as_str())
                    || self.global_vars.contains_key(name.as_str())
                    || self.top_level_static_names.contains(name.as_str())
                {
                    format!("{}_", name)
                } else {
                    name.clone()
                };
                // 自引用重绑定（`let parts = parts + [p]`）：虽未写 mut，但 value
                // 引用自身，LZ 语义为重绑定（Python 风格），生成赋值而非 shadow-let
                // （Rust shadow-let 在循环内不会累积，vector.__str__ 曾返回 [""]）
                let self_rebind = !*is_mut
                    && self.declared.contains(&safe_name)
                    && expr_mentions_var(value, &safe_name);
                // 循环内重新声明已声明变量：LZ 语义为重新赋值，生成赋值而非 shadow-let
                // （Rust shadow-let 在循环内不会累积，iter.lz collect 无限循环）
                let loop_rebind =
                    self.loop_depth > 0 && self.declared.contains(&safe_name) && !*is_mut;
                if (*is_mut || self_rebind || loop_rebind) && self.declared.contains(&safe_name) {
                    // ref 绑定变量（ref r = x）：r = v → *r = v（解引用赋值修改原值）
                    if self.ref_bindings.contains(name.as_str()) {
                        self.emit_line(&format!("*{} = {};", safe_name, self.gen_expr(value)));
                        return;
                    }
                    if self.mutated_consts.contains(name) {
                        self.emit_line(&format!(
                            "unsafe {{ {} = {}; }}",
                            safe_name,
                            self.gen_expr(value)
                        ));
                    } else {
                        self.emit_line(&format!("{} = {};", safe_name, self.gen_expr(value)));
                    }
                    return;
                }
                // 如果发生了重命名，用新名称注册 declared
                self.declared.insert(safe_name.clone());
                // 登记字符串类型局部变量：let 绑定显式 ty=Str 或值为字符串字面量/
                // 字符串插值/字符串方法链（f-string 用 {} 而非 {:?}，避免 Debug 引号）
                if matches!(ty, IrType::Str)
                    || matches!(ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str))
                    || self.is_str_producing(value)
                {
                    self.str_typed_vars.insert(safe_name.clone());
                }
                // 字段插值判定要的是「这个变量的 IR 类型」，比 str_typed_vars 宽一档：
                // builder 在 value 上带的 ty 已能认出 struct 值（`let res = MyResource(...)`），
                // 而 str_typed_vars 只认字符串。
                self.fstr_var_types.insert(name.clone(), value.ty.clone());
                self.fstr_var_types
                    .insert(safe_name.clone(), value.ty.clone());
                // ref 绑定（ref r = x / let ref r = x，02-变量与绑定 §5、13-指针与引用 §2.1）：
                //   ref r = x     → let r = &mut x;（无 let 前缀，默认可变引用）
                //   let ref r = x → let r = &x;（let 强制不可变引用）
                //   ref r = 42    → let mut __lz_ref_r = 42; let r = &mut __lz_ref_r;
                //   let ref r = 42 → let __lz_ref_r = 42; let r = &__lz_ref_r;
                if *is_ref {
                    let val_s = self.gen_expr(value);
                    let ref_kw = if *is_mut { "&mut " } else { "&" };
                    let is_literal = matches!(&value.kind, ExprKind::Lit(_))
                        || matches!(&value.kind, ExprKind::StructCtor { .. });
                    if is_literal {
                        // 字面量/构造取引用：先建临时变量，再引用它
                        let tmp = format!("__lz_ref_{}", safe_name);
                        let tmp_mut = if *is_mut { "mut " } else { "" };
                        self.emit_line(&format!("let {}{} = {};", tmp_mut, tmp, val_s));
                        self.emit_line(&format!("let {} = {}{};", safe_name, ref_kw, tmp));
                    } else {
                        self.emit_line(&format!("let {} = {}{};", safe_name, ref_kw, val_s));
                    }
                    self.ref_bindings.insert(safe_name.clone());
                    return;
                }
                // 模块级函数/常量名冲突时（E0530，如 math.lz 的 `let sign` 遮蔽
                // 模块级 `fn sign`）声明被重命名为 sign_，引用处 Var 也需同步解析：
                // 登记到 param_renames（与参数重命名同一机制），否则 `sign * x`
                // 会解析到模块级 fn sign（E0369 cannot multiply fn by f64）
                if safe_name != name.as_str() {
                    self.param_renames.insert(name.clone(), safe_name.clone());
                }
                // LZ 语义（00-词法基础.md:35）：`let` = 不可变绑定 → 生成 Rust `let`；
                // `mut x = ...`（is_mut）才生成 `let mut`。例外：`_` 通配符不能有 mut
                // （Rust E0573），且不可变绑定不能有 mut 关键字（E0596）。
                // 自动 mut：LZ `let v = vec; v.push(1)` 未写 mut，但该变量在函数体
                // 内被可变使用（方法调用接收者/赋值目标），需生成 `let mut`（E0596）
                let need_auto_mut =
                    !*is_mut && safe_name != "_" && self.auto_mut_locals.contains(&safe_name);
                let mut_kw = if safe_name == "_" || (!*is_mut && !need_auto_mut) {
                    ""
                } else {
                    "mut "
                };
                let skip_ty = *ty == IrType::Any
                    || *ty == IrType::Unit
                    || matches!(ty, IrType::Duck { .. })
                    || matches!(ty, IrType::Generic(_))
                    || (matches!(ty, IrType::Fn { .. }) && !is_fn_lambda_let)
                    || self.has_unresolved_dotted_assoc(ty)
                    // ref V（set_default 返回 &V，V 未绑定泛型）：跳过标注
                    // （E0425 cannot find type V）
                    || matches!(ty, IrType::Ref(inner)
                        if self.has_unbound_named(inner)
                            || matches!(inner.as_ref(), IrType::Generic(_)))
                    || matches!(ty, IrType::MutRef(inner)
                        if self.has_unbound_named(inner)
                            || matches!(inner.as_ref(), IrType::Generic(_)))
                    || matches!(ty, IrType::Option(inner) if matches!(inner.as_ref(), IrType::Any))
                    || matches!(ty, IrType::Result { ok, err }
                        if matches!(ok.as_ref(), IrType::Any)
                            || matches!(err.as_ref(), IrType::Any))
                    // Result<T, Rc<T>> 中 T 是未绑定泛型（Named("T") 或 Generic("T")）：
                    // 跳过类型标注（box.lz `let result: Result<T, Rc<T>> = rc.try_unwrap()`，
                    // E0425 cannot find type `T`）
                    || matches!(ty, IrType::Result { ok, err }
                        if self.has_unbound_named(&ok)
                            || self.has_unbound_named(&err)
                            || matches!(ok.as_ref(), IrType::Generic(_))
                            || matches!(err.as_ref(), IrType::Generic(_)))
                    || matches!(ty, IrType::Option(inner) if self.has_unbound_named(&inner)
                        || matches!(inner.as_ref(), IrType::Generic(_)))
                    || if let IrType::Named { path, args } = ty {
                        path == "Range" || path == "Nil" || path == "Dict" || path == "Set"
                            || path == "Future"  // Future<T> 是 trait 不是具体类型，无法用于变量标注
                            || path == "Iterator"  // Iterator<T> 生成 impl Trait，变量标注需跳过（E0562）
                            || args.is_empty()
                            || args.iter().any(|a| matches!(a, IrType::Generic(_)))
                            || args.iter().any(|a| matches!(a, IrType::Any))
                            || args.iter().any(|a| matches!(a, IrType::Named { path: p, args: pa }
                                if pa.is_empty()
                                    && !self.known_types.contains(p.as_str())
                                    && !self.emitted_types.contains(p.as_str())
                                    && !self.top_level_static_names.contains(p.as_str())))
                    } else {
                        false
                    };
                // BUG-4：bigint/complex 注解在 IR 里是 `Named{path:"bigint", args:[]}`
                // （builder 的 bigint 映射产出物），会命中上面 `args.is_empty()` 而被
                // 丢弃标注 → 函数体内 `let x: bigint = 123…` 坍塌成裸 i128。
                // 二者是内置标量、Rust 类型名恒定可解析，标注始终可发射。
                let skip_ty = skip_ty && !(is_bigint_ty(ty) || is_complex_ty(ty));
                // 空容器需要类型提示 Vec<_> / HashMap<_, _>（Nil 类型除外）
                // Dir/Set 空容器：即使 skip_ty 为 true，也强制输出类型标注（Rust 无法推断 K, V）
                let is_empty_container = match &value.kind {
                    ExprKind::ListLit(elems) => {
                        elems.is_empty()
                            && !matches!(ty, IrType::Named { path, .. } if path == "Nil")
                    }
                    ExprKind::StructCtor { name: n, fields } => n == "Dict" && fields.is_empty(),
                    _ => false,
                };
                // 空 Dict/Set 强制输出类型标注
                let force_ty = is_empty_container
                    && matches!(ty, IrType::Named { path, .. } if path == "Dict" || path == "Set");
                let mut ty_str = if is_empty_container {
                    // 优先使用声明的类型；若无则使用占位符
                    if !skip_ty || force_ty {
                        format!(": {}", self.rust_type(ty))
                    } else if let ExprKind::StructCtor { name: n, .. } = &value.kind {
                        if n == "Dict" {
                            ": std::collections::HashMap<_, _>".to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        format!(": {}", self.rust_type(ty))
                    }
                } else if skip_ty {
                    // None 字面量/构造/变量：类型未知时用 Option<i64> 默认，避免 Rust 无法推断
                    let is_none = matches!(&value.kind, ExprKind::Lit(LitKind::None_))
                        || matches!(&value.kind, ExprKind::StructCtor { name: n, .. } if n == "None")
                        || matches!(&value.kind, ExprKind::Var(n) if n == "None");
                    if is_none {
                        ": Option<i64>".to_string()
                    } else {
                        String::new()
                    }
                } else {
                    format!(": {}", self.rust_type(ty))
                };
                // IR-003：fn 值 let（嵌套 def 闭包）显式标注 Arc<dyn Fn … + Send + Sync>，否则
                // `let inner = Arc::new(closure)` 无法推断目标 trait object（E0282/E0308）
                if is_fn_lambda_let {
                    ty_str = format!(": {}", self.fn_value_type(ty));
                }
                // walrus 变量预声明（let 绑定中的 := 需要先声明变量再赋值）
                self.emit_walrus_predecls(value);
                // 元组解构（let (a,b,c) = tuple 或 __destruct_ 临时）→ 对源元组 clone 避免 move（LZ 元组可重复解构）
                let is_tuple_destr = (safe_name.starts_with('(') && safe_name.contains(','))
                    || safe_name.starts_with("__destruct_");
                let saved_box_lambda = self.box_lambda;
                self.box_lambda = is_fn_lambda_let;
                // Let 绑定值生成前注入声明类型到 current_expected_ty，
                // 使块内 IndexGet/构造器等能跟随期望类型（如 let esc: String = ...）
                let prev_expected_for_let = self.current_expected_ty.borrow().clone();
                *self.current_expected_ty.borrow_mut() = Some(ty.clone());
                let value_s = if is_tuple_destr && matches!(ty, IrType::Tuple(_)) {
                    format!("({}).clone()", self.gen_expr(value))
                } else if is_empty_container {
                    match ty {
                        IrType::Named { path, .. } if path == "Dict" || path == "HashMap" => {
                            "std::collections::BTreeMap::new()".to_string()
                        }
                        IrType::Named { path, .. } if path == "Set" || path == "HashSet" => {
                            "std::collections::HashSet::new()".to_string()
                        }
                        _ => "Vec::new()".to_string(),
                    }
                } else {
                    self.gen_expr(value)
                };
                *self.current_expected_ty.borrow_mut() = prev_expected_for_let;
                self.box_lambda = saved_box_lambda;
                // 登记 `Arc<dyn Fn … + Send + Sync>` 载体变量名 + 参数个数（闭包装箱 let / 返回 fn 的调用
                // 结果），供实参边界生成转发闭包（见 fn_value_fwd）
                let fn_arity = match ty {
                    IrType::Fn { params, .. } => Some(params.len()),
                    _ => match &value.ty {
                        IrType::Fn { params, .. } => Some(params.len()),
                        _ => None,
                    },
                };
                let is_rc_fn_let = is_fn_lambda_let
                    || (matches!(&value.kind, ExprKind::Call { .. })
                        && matches!(&value.ty, IrType::Fn { .. }));
                if let Some(arity) = fn_arity {
                    if is_rc_fn_let {
                        self.fn_value_carriers.insert(safe_name.clone(), arity);
                        self.fn_value_carriers.insert(name.clone(), arity);
                    }
                }
                // BUG-SG-002/003：`T?` 位置自动 Some 包装。
                // `let z: int? = 10` / `let cfg: Config? = Config { .. }` 在 Rust
                // 侧是 `Option<T> = T`（E0308）。注解可空而初始值非空 → 补 Some。
                // 元组解构/空容器分支产出的同样是非 Option 值，一并适用。
                let value_s = if needs_some_wrap(ty, value) {
                    format!("Some({})", value_s)
                } else {
                    value_s
                };
                // default 关键字桥（let 触发点）：let x: T = default
                // 当 T 实现了 __implicit_default__ → 生成 <T as ImplicitDefault>::__implicit_default__()
                let value_s = if let ExprKind::Default = &value.kind {
                    if let IrType::Named {
                        path: target_path, ..
                    } = ty
                    {
                        if self.is_known_type(target_path) {
                            let has_implicit_default = self
                                .struct_method_names_map
                                .get(target_path)
                                .map(|ms| ms.contains("__implicit_default__"))
                                .unwrap_or(false);
                            if has_implicit_default {
                                let target_rust_ty = self.rust_type(&IrType::Named {
                                    path: target_path.to_string(),
                                    args: vec![],
                                });
                                format!(
                                    "<{} as lz_builtins::runtime::ImplicitDefault>::__implicit_default__()",
                                    target_rust_ty
                                )
                            } else {
                                value_s
                            }
                        } else {
                            value_s
                        }
                    } else {
                        value_s
                    }
                } else {
                    value_s
                };
                // 隐式转换桥（let 触发点）：let x: TargetTy = src_val
                // 当 TargetTy 是 Named 类型且 src_val 类型不匹配 → 插入 __implicit_from__ 桥
                // 注意：不依赖 skip_ty，因为无泛型参数的 struct（如 Celsius）会被 skip_ty 跳过
                let value_s = if let IrType::Named {
                    path: _target_path, ..
                } = ty
                {
                    if *ty != IrType::Any && *ty != IrType::Unit {
                        if let Some(bridge) = self.build_implicit_bridge(ty, value, &value_s) {
                            bridge
                        } else {
                            value_s
                        }
                    } else {
                        value_s
                    }
                } else {
                    value_s
                };
                // __implicit_copy__ 桥（Mojo 风格隐式复制）：
                // 当目标类型实现了 __implicit_copy__，且源类型与目标类型相同 →
                // 桥接为 <T as ImplicitCopy>::__implicit_copy__(&value_s)
                // 注：必须放在 implicit_from 桥之后，避免类型不匹配时错误触发
                // 仅当值是变量引用时才触发（避免对构造表达式误触发）
                let value_s = if let IrType::Named {
                    path: target_path, ..
                } = ty
                {
                    if *ty != IrType::Any && *ty != IrType::Unit {
                        let has_implicit_copy = self
                            .struct_method_names_map
                            .get(target_path)
                            .map(|ms| ms.contains("__implicit_copy__"))
                            .unwrap_or(false);
                        if has_implicit_copy {
                            // 仅当源类型与目标类型相同，且值是变量引用时触发
                            if let IrType::Named { path: src_path, .. } = &value.ty {
                                if src_path == target_path {
                                    if let ExprKind::Var(_) = &value.kind {
                                        let target_rust_ty = self.rust_type(&IrType::Named {
                                            path: target_path.to_string(),
                                            args: vec![],
                                        });
                                        format!(
                                            "<{} as lz_builtins::runtime::ImplicitCopy>::__implicit_copy__(&{})",
                                            target_rust_ty, value_s
                                        )
                                    } else {
                                        value_s
                                    }
                                } else {
                                    value_s
                                }
                            } else {
                                value_s
                            }
                        } else {
                            value_s
                        }
                    } else {
                        value_s
                    }
                } else {
                    value_s
                };
                // Result 基 try 块内：raises 函数返回 Result，需 ? 解包
                // 也包括 raises 函数（非 try/catch）的 let 语句，让 ? 在纯函数中生效
                let (value_s, ty_str) = if (self.in_result_try || self.current_fn_raises.is_some())
                    && matches!(&value.ty, IrType::Result { .. })
                {
                    let unwrapped_ty = if let IrType::Result { ok, .. } = &value.ty {
                        format!(": {}", self.rust_type(ok))
                    } else {
                        ty_str.clone()
                    };
                    (format!("({})?", value_s), unwrapped_ty)
                } else {
                    (value_s, ty_str)
                };
                // T04：修饰符装饰器新语义轴 → Rust 目标类型 / 初值包装。
                // `mods` 无包装轴时 `moddec_wrap_let` 返回 `None`，保持原逻辑（零回归）。
                let (ty_str, value_s) = {
                    let base_ty = if skip_ty {
                        None
                    } else {
                        Some(self.rust_type(ty))
                    };
                    match self.moddec_wrap_let(name, &safe_name, base_ty.as_deref(), &value_s, mods)
                    {
                        Some((wrapped_ty, wrapped_val)) => {
                            (format!(": {}", wrapped_ty), wrapped_val)
                        }
                        None => (ty_str, value_s),
                    }
                };
                self.emit_line(&format!(
                    "let {}{}{} = {};",
                    mut_kw, safe_name, ty_str, value_s
                ));
                // __init__ 构造后调用点注入：let x = Struct { .. } 且 Struct 有 __init__ →
                // 在同一语句后追加 x.__lz_init(); 以触发用户定义的初始化逻辑。
                // 仅当变量可变时注入（__lz_init 需要 &mut self）。
                if mut_kw == "mut " && safe_name != "_" {
                    let ctor_name = match &value.kind {
                        ExprKind::StructCtor { name, .. } => Some(name.clone()),
                        ExprKind::Call { callee, .. } => {
                            if let ExprKind::Var(n) = &callee.kind {
                                Some(n.clone())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some(ctor_name) = ctor_name {
                        let has_init = self
                            .struct_method_names_map
                            .get(&ctor_name)
                            .map(|ms| ms.contains("__init__"))
                            .unwrap_or(false);
                        // 仅当 __init__ 除 self 外无其他参数时自动注入调用点。
                        let only_self = self
                            .struct_init_params_map
                            .get(&ctor_name)
                            .map(|ps| ps.iter().filter(|(name, _)| name != "self").count() == 0)
                            .unwrap_or(true);
                        if has_init && only_self {
                            self.emit_line(&format!("{}.__init__();", safe_name));
                        }
                    }
                }
            }
            Stmt::Assign { target, value } => {
                // Dict/HashMap 索引赋值 → .insert() 替代（HashMap 不实现 IndexMut）
                if let ExprKind::IndexGet { base, key } = &target.kind {
                    let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                    if is_dict {
                        let key_s = self.gen_expr(key);
                        let val_s = self.gen_expr(value);
                        // 嵌套 dict 链（settings["theme"]["color"] = v）：base 本身是
                        // IndexGet 时需**可变引用链** .get_mut(&k).unwrap()——否则
                        // gen_expr(base) 的 .get(&k).cloned().unwrap() 克隆内层 dict，
                        // insert 作用在克隆上，原 dict 不变（polish_09 断言失败）
                        if let ExprKind::IndexGet {
                            base: base2,
                            key: key2,
                        } = &base.kind
                        {
                            let is_dict2 = matches!(&base2.ty, IrType::Named { path, .. }
                                if path == "Dict" || path == "HashMap");
                            if is_dict2 {
                                let base2_s = self.gen_expr(base2);
                                let key2_s = self.gen_expr(key2);
                                let inner = format!("({}).get_mut(&{}).unwrap()", base2_s, key2_s);
                                self.emit_line(&format!("{}.insert({}, {});", inner, key_s, val_s));
                                return;
                            }
                        }
                        let base_s = self.gen_expr(base);
                        self.emit_line(&format!("{}.insert({}, {});", base_s, key_s, val_s));
                        return;
                    }
                    // 用户 struct 索引赋值 → .__setitem__(key, value)（key 保持 i64，内部 self.items[i] 再转 usize）
                    let is_struct =
                        matches!(&base.ty, IrType::Named { path, .. } if self.is_known_type(path));
                    if is_struct {
                        let base_s = self.gen_expr(base);
                        let key_s = self.gen_expr(key);
                        let val_s = self.gen_expr(value);
                        self.emit_line(&format!("({}).__setitem__({}, {});", base_s, key_s, val_s));
                        return;
                    }
                    // checker 块 ps.args[k] = v：元素为 Box<dyn Any>，需 Box::new 包装
                    let is_params_args = matches!(&base.kind,
                        ExprKind::FieldAccess { field, .. } if field == "args");
                    if is_params_args {
                        let base_s = self.gen_expr(base);
                        let key_s = self.gen_index_key(key, base);
                        let val_s = self.gen_expr(value);
                        self.emit_line(&format!("{}[{}] = Box::new({});", base_s, key_s, val_s));
                        return;
                    }
                }
                // _ = expr → 丢弃语句，生成 let _ = expr（仅取副作用）
                if matches!(&target.kind, ExprKind::Var(n) if n == "_") {
                    self.emit_line(&format!("let _ = {};", self.gen_expr(value)));
                    return;
                }
                // 全局可变变量赋值 → unsafe { count = value; }
                if let ExprKind::Var(gname) = &target.kind {
                    if self.global_vars.contains_key(gname.as_str()) {
                        let val_s = self.gen_expr(value);
                        self.emit_line(&format!("unsafe {{ {} = {}; }}", gname, val_s));
                        return;
                    }
                }
                let target_s = self.gen_target_expr(target);
                let val_s = self.gen_expr(value);
                // ref 绑定变量（ref r = x）：r = v → *r = v（跨块赋值，同块走 Let 分支）
                if let ExprKind::Var(name) = &target.kind {
                    if self.ref_bindings.contains(name.as_str()) {
                        self.emit_line(&format!("*{} = {};", target_s, val_s));
                        return;
                    }
                }
                // ref mut 模式绑定（case Some(ref mut c)）：c 是 &mut 引用，
                // c = c + 1 需生成 *c = *c + 1（解引用赋值，E0384 修复）
                if let ExprKind::Var(name) = &target.kind {
                    if self.ref_mut_bindings.contains(name.as_str()) {
                        // 值侧 c 也需解引用：*c = *c + 1（LZ ref mut 语义：修改引用指向的值）
                        let val_deref = if let ExprKind::BinOp { lhs, rhs, op } = &value.kind {
                            let l = if matches!(lhs.kind, ExprKind::Var(ref n) if n == name) {
                                format!("*{}", self.gen_expr(lhs))
                            } else {
                                self.gen_expr(lhs)
                            };
                            let r = if matches!(rhs.kind, ExprKind::Var(ref n) if n == name) {
                                format!("*{}", self.gen_expr(rhs))
                            } else {
                                self.gen_expr(rhs)
                            };
                            format!("{} {} {}", l, self.binop_str(op), r)
                        } else {
                            val_s
                        };
                        self.emit_line(&format!("*{} = {};", target_s, val_deref));
                        return;
                    }
                }
                // 模块级可变变量 → 需 unsafe 块
                if self.mutated_consts.contains(&target_s) {
                    self.emit_line(&format!("unsafe {{ {} = {}; }}", target_s, val_s));
                } else {
                    self.emit_line(&format!("{} = {};", target_s, val_s));
                }
            }
            Stmt::Return { value } => {
                if self.in_gen_build {
                    // 生成器构建块闭包内 return 等价提前退出收集（闭包返回 ()）
                    self.emit_line("return;");
                } else if self.in_generator {
                    // iterator 体内 return 等价 raise：终止迭代并抛出
                    // （return expr 为错误信息；return 无值 → 空 panic）
                    if let Some(v) = value {
                        self.emit_line(&format!("panic!(\"{{:?}}\", {});", self.gen_expr(v)));
                    } else {
                        self.emit_line("panic!(\"generator return\");");
                    }
                } else if let Some(v) = value {
                    // BUG-CG-004（收口）：raises 函数返回类型升级为 Result<T, E>，
                    // 故 `return X`（X 自身非 Result）需包成 `return Ok(X)`。
                    // raises 函数中 `return X` 返回 `()` 时也需包（签名是 Result<(), E>）
                    let wrap_ok =
                        self.current_fn_raises.is_some() && !matches!(&v.ty, IrType::Result { .. });
                    // `return self`：self 是 &self 引用。
                    // 返回类型是引用（`-> ref Self`，如 inspect）时直接 return self；
                    // 返回 owned 值时需 clone（`fn or(&self) -> Option<T>` 中
                    // `return self` → `return self.clone()`，E0308 expected Option<T>）
                    let ret_is_ref = matches!(&self.current_ret_ty, Some(IrType::Ref(_) | IrType::MutRef(_)))
                        || matches!(&self.current_ret_ty, Some(IrType::Named { path, .. }) if path == "Self")
                        // `-> &Self`（inspect 等方法）返回引用：current_ret_ty 可能为 None
                        // （builder 对 ref Self 推断失败），按函数签名判断
                        || self.current_fn_ret_is_ref;
                    // current_ret_ty 可能为 None（builder 对 match 包裹的返回类型
                    // 推断失败，如 filter），此时仅凭签名判断：不返回引用即需 clone
                    let ret_is_unit = self.current_ret_ty == Some(IrType::Unit);
                    if matches!(&v.kind, ExprKind::Var(n) if n == "self" || n == "self_")
                        && !ret_is_unit
                        && !ret_is_ref
                    {
                        // ref str 的 self.clone() 返回 &str（&str: Clone），需 to_string
                        // 转 String（string.lz replace/__str__ `return self`，E0308）
                        let ret_is_string = matches!(&self.current_ret_ty,
                            Some(IrType::Named { path, .. }) if path == "String" || path == "str")
                            || matches!(&self.current_ret_ty, Some(IrType::Str));
                        if ret_is_string {
                            self.emit_line(if wrap_ok {
                                "return Ok(self.to_string());"
                            } else {
                                "return self.to_string();"
                            });
                        } else {
                            self.emit_line(if wrap_ok {
                                "return Ok(self.clone());"
                            } else {
                                "return self.clone();"
                            });
                        }
                    } else {
                        // Iterator impl 的 next：自定义 `enum Option<T>`（lz_std/option.lz）
                        // 与 std Option 同名冲突——签名强制 std::option::Option<T>（E0053），
                        // body 返回的自定义 Option 需 match 转换（E0308 expected
                        // std::option::Option<T>, found Option<T>）
                        let saved_box_lambda = self.box_lambda;
                        self.box_lambda = matches!(&self.current_ret_ty, Some(IrType::Fn { .. }))
                            && matches!(&v.kind, ExprKind::Lambda { .. })
                            && !self.in_lambda_block;
                        let ret_s = self.gen_expr(v);
                        self.box_lambda = saved_box_lambda;
                        // `return self`（&str）返回 String（__str__ 尾表达式 `= self`）：
                        // 需 to_string（&str: Clone 返回 &str，E0308 expected String）
                        let ret_is_string = matches!(&self.current_ret_ty,
                            Some(IrType::Named { path, .. }) if path == "String" || path == "str")
                            || matches!(&self.current_ret_ty, Some(IrType::Str));
                        if ret_is_string && (ret_s == "self" || ret_s == "(self)") {
                            self.emit_line(if wrap_ok {
                                "return Ok(self.to_string());"
                            } else {
                                "return self.to_string();"
                            });
                            return;
                        }
                        let ret_is_option = matches!(&v.ty, IrType::Named { path, .. } if path == "Option")
                            || matches!(&v.ty, IrType::Option(_));
                        if self.in_iterator_impl
                            && self.known_types.contains("Option")
                            && ret_is_option
                        {
                            self.emit_line(&format!(
                                "return match {} {{ Option::Some(__v) => Some(__v), Option::None => None }};",
                                ret_s
                            ));
                        } else {
                            // ref str 的尾表达式 self（string.lz __str__ `= self` 返回
                            // String）：self 是 &str 需 to_string（&str: Clone 返回 &str）
                            let ret_is_string = matches!(&self.current_ret_ty,
                                Some(IrType::Named { path, .. }) if path == "String" || path == "str")
                                || matches!(&self.current_ret_ty, Some(IrType::Str));
                            if ret_is_string
                                && matches!(&v.kind, ExprKind::Var(n) if n == "self" || n == "self_")
                            {
                                self.emit_line(if wrap_ok {
                                    "return Ok(self.to_string());"
                                } else {
                                    "return self.to_string();"
                                });
                            } else {
                                self.emit_line(&format!(
                                    "return {};",
                                    if wrap_ok {
                                        format!("Ok({})", ret_s)
                                    } else {
                                        ret_s
                                    }
                                ));
                            }
                        }
                    }
                } else {
                    self.emit_line("return;");
                }
            }
            Stmt::ExprStmt { expr } => {
                self.emit_walrus_predecls(expr);
                // 嵌套 Fn 返回（fn -> fn -> T）：内层闭包作为外层返回值需 Arc::new 包装
                // （factory_chain: |a| => |b| => x + a + b → move |a| { Arc::new(move |b| {...}) }）。
                // 仅在 Lambda 块体内生效；函数体本身的尾表达式（外层闭包）不包装
                let nested_fn_body = self.nested_fn_ret
                    && self.in_lambda_block
                    && matches!(&expr.kind, ExprKind::Lambda { .. });
                let saved_box_lambda = self.box_lambda;
                self.box_lambda = matches!(&self.current_ret_ty, Some(IrType::Fn { .. }))
                    && matches!(&expr.kind, ExprKind::Lambda { .. })
                    && !self.in_lambda_block;
                let expr_s = if nested_fn_body {
                    format!("Arc::new({})", self.gen_expr(expr))
                } else {
                    self.gen_expr(expr)
                };
                self.box_lambda = saved_box_lambda;
                // BUG-CG-004（收口）：raises 函数尾表达式需包成 Ok(...)（expr 自身已是 Result 则不包）
                // 但不包裹返回 () 的表达式（如 skip_ws()）
                // 也不包裹返回 Result<(), _> 的表达式（skip_ws 返回 Result<(), _>，不应再包 Ok）
                // 也不包裹对已知 Unit 返回方法的调用（如 skip_ws，builder 推断类型为 Any 但实际返回 ()）
                let is_known_unit_call = matches!(&expr.kind,
                    ExprKind::MethodCall { method, .. }
                    if method == "skip_ws"
                );
                let ret_is_unit = matches!(&self.current_ret_ty, Some(IrType::Unit));
                let wrap_ok = self.current_fn_raises.is_some()
                    && !matches!(&expr.ty, IrType::Result { .. })
                    && !ret_is_unit
                    && !is_known_unit_call;
                if is_known_unit_call {}
                if is_last && !self.is_main && !self.suppress_tail_return && !self.in_generator {
                    // 非 main 函数尾表达式 → return expr;
                    // 返回引用（`-> &T` / `-> &mut T`）时尾表达式 self.字段：
                    // 生成 &self.field / &mut self.field，而非 borrow_self 误加的
                    // self.field.clone()（box.lz get/get_mut，E0308 expected &T, found T）
                    let ret_ref_field = self.current_fn_ret_is_ref
                        && matches!(&expr.kind, ExprKind::FieldAccess { base, .. }
                            if matches!(&base.kind, ExprKind::Var(n) if n == "self" || n == "self_"));
                    if ret_ref_field {
                        let field = match &expr.kind {
                            ExprKind::FieldAccess { field, .. } => field.clone(),
                            _ => unreachable!(),
                        };
                        let prefix = if matches!(&self.current_ret_ty, Some(IrType::MutRef(_))) {
                            "&mut "
                        } else {
                            "&"
                        };
                        self.emit_line(&format!(
                            "return {};",
                            if wrap_ok {
                                format!("Ok({}{}.{})", prefix, "self", field)
                            } else {
                                format!("{}{}.{}", prefix, "self", field)
                            }
                        ));
                    } else if matches!(&expr.kind, ExprKind::Var(n) if n == "self" || n == "self_")
                        && !self.current_fn_ret_is_ref
                    {
                        // 尾表达式 `self` 返回 owned（顶层 self-def 的链式方法
                        // `def inc(mut self: T) -> T = ...; self`）：self 是
                        // &self/&mut self 引用，需 clone 为 owned（E0308）
                        self.emit_line(if wrap_ok {
                            "return Ok(self.clone());"
                        } else {
                            "return self.clone();"
                        });
                    } else {
                        self.emit_line(&format!(
                            "return {};",
                            if wrap_ok {
                                format!("Ok({})", expr_s)
                            } else {
                                expr_s
                            }
                        ));
                    }
                } else if is_last && self.pending_tail_capture.is_some() {
                    // defer+尾值捕获：尾值存入临时变量，flush defer cleanup 后
                    // 在块尾统一 `return __tail_val_1`（否则被分号丢弃 → 返回 ()）
                    self.emit_line(&format!(
                        "let {} = {};",
                        self.pending_tail_capture.as_ref().unwrap(),
                        expr_s
                    ));
                } else if is_last && self.suppress_tail_return && self.force_stmt_semicolon {
                    // 循环体尾表达式：非值上下文，需加分号（否则 E0308）
                    self.emit_line(&format!("{};", expr_s));
                } else if is_last && self.suppress_tail_return && self.force_unit_tail {
                    // 块内含无值 return（return;）→ 尾表达式丢弃值（expr;），
                    // 使闭包返回类型为 ()，避免与 return; 冲突（E0308）
                    self.emit_line(&format!("{};", expr_s));
                } else if is_last
                    && self.in_if_else_branch
                    && matches!(
                        &expr.kind,
                        ExprKind::MethodCall { .. } | ExprKind::Call { .. }
                    )
                {
                    // if/else 分支内尾方法调用：丢弃返回值，统一分支类型为 ()
                    self.emit_line(&format!("let _ = {};", expr_s));
                } else if is_last && self.suppress_tail_return {
                    // match arm / 块表达式尾值 → 裸表达式（无分号，作为块值）
                    self.emit_line(&format!("{}", expr_s));
                } else if is_last {
                    // main 函数尾表达式 → expr;
                    self.emit_line(&format!("{};", expr_s));
                } else if self.in_if_else_branch
                    && matches!(
                        &expr.kind,
                        ExprKind::MethodCall { .. } | ExprKind::Call { .. }
                    )
                {
                    // if/else 分支内方法调用需 `let _ = expr` 丢弃返回值，
                    // 以统一分支类型为 `()`（否则 E0308 if and else have incompatible types）
                    self.emit_line(&format!("let _ = {};", expr_s));
                } else {
                    self.emit_line(&format!("{};", expr_s));
                }
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.emit_walrus_predecls(cond);
                if let Some(else_blk) = else_branch {
                    self.emit_line(&format!("if {} {{", self.gen_bool_cond(cond)));
                    self.indent += 1;
                    let saved = self.in_if_else_branch;
                    self.in_if_else_branch = true;
                    self.gen_block_inner(then_branch);
                    self.indent -= 1;
                    self.emit_line("} else {");
                    self.indent += 1;
                    self.gen_block_inner(else_blk);
                    self.in_if_else_branch = saved;
                    self.indent -= 1;
                    self.emit_line("}");
                } else {
                    self.emit_line(&format!("if {} {{", self.gen_bool_cond(cond)));
                    self.indent += 1;
                    self.gen_block_inner(then_branch);
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
            Stmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
            } => {
                self.emit_walrus_predecls(iter);
                // for/else：循环正常结束（非 break）执行 else 体（规范 05-控制流.md §13.2）。
                // Rust 无 for/else 语法，用 labeled block：break 'label 跳出整个块跳过 else
                let else_label = else_body.as_ref().map(|_| {
                    self.loop_else_counter += 1;
                    let label = format!("__lz_loop_else_{}", self.loop_else_counter);
                    self.emit_line(&format!("'{}: {{", label));
                    label
                });
                if else_label.is_some() {
                    self.indent += 1;
                }
                self.loop_else_stack.push(else_label.clone());
                // 顶层静态集合（LazyLock<Vec<..>>）不能用 into_iter()（共享引用不可 move），
                // 改用 .iter().cloned()（LZ 元素均 Clone）
                let use_lazy_iter = if let ExprKind::Var(name) = &iter.kind {
                    self.is_collection_type(&iter.ty)
                        && (self.top_level_static_names.contains(name))
                } else {
                    false
                };
                // ref 参数（iterable: &I）不能直接 .into_iter()：&I 的 IntoIterator impl
                // 会 move *iterable（E0507）。先 clone 为 owned 再迭代（I: Clone 泛型 bound）
                let iter_is_ref = matches!(&iter.ty, IrType::Ref(_) | IrType::MutRef(_))
                    || (matches!(&iter.kind, ExprKind::Var(n) if n == "self") && self.borrow_self);
                // Vec<List> 内含引用元素（Vec<&T>）：.into_iter() 产生 &T，
                // 需要 owned T 时（如 JsonValue::String(key)）用 .map(|x| x.clone()) 取值。
                let iter_vec_ref_elem = matches!(
                    &iter.ty,
                    IrType::Named { path, args }
                        if (path == "Vec" || path == "List")
                            && args.first().map_or(false, |a| matches!(
                                a,
                                IrType::Ref(_) | IrType::MutRef(_)
                            ))
                );
                // variadic 切片参数（`..: T` → Rust `args: &[T]`）：
                // `.into_iter()` 给出 &T，但 LZ 语义要求按值迭代。
                // 用 `.iter().copied()`（Copy 类型）取值。
                let iter_is_variadic_slice = matches!(&iter.kind, ExprKind::Var(name)
                    if self.current_variadic_params.contains(name));
                // self（&Vec<T>）上的 for 循环：.into_iter() 有 &Vec/Vec 双 IntoIterator
                // 歧义（E0034 multiple into_iter found），用 .iter() 明确（item=&T）
                let iter_is_self_borrow =
                    matches!(&iter.kind, ExprKind::Var(n) if n == "self") && self.borrow_self;
                let iter_expr = |cg: &Self| -> String {
                    let s = cg.gen_expr(iter);
                    // 字符串 for 迭代：String 不实现 IntoIterator（E0599），
                    // 需用 .chars() 逐字符迭代（`for c in "abcd"`）
                    let iter_is_str = matches!(&iter.ty, IrType::Str)
                        || matches!(&iter.ty, IrType::Named { path, .. }
                            if path == "str" || path == "String");
                    if iter_is_str {
                        format!("({}).chars()", s)
                    } else if iter_vec_ref_elem {
                        // Vec<&T>：.into_iter() 产生 &T，下游需要 owned T（如 JsonValue::String(key)、
                        // .get(&key) 的 Borrow 约束）→ .map(|x| x.clone()) 取值
                        format!("({}).into_iter().map(|x| x.clone())", s)
                    } else if iter_is_self_borrow {
                        format!("({}).iter()", s)
                    } else if iter_is_ref {
                        format!("(*{}).clone().into_iter()", s)
                    } else if iter_is_variadic_slice {
                        // variadic 切片 &[T]：.iter().copied() 给出 T（Copy 类型）；
                        // variadic kwargs &HashMap/Dict：item 为 (&K,&V) 元组，
                        // `.copied()` 要求单引用 &T → E0271，改用 .iter() 按
                        // (&K,&V) 引用解包（loop 变量得到 &K/&V，Display/比较均可用）。
                        let variadic_is_dict = matches!(
                            &iter.ty,
                            IrType::Named { path, .. }
                                if path == "HashMap" || path == "Dict"
                        ) || matches!(
                            &iter.ty,
                            IrType::Ref(inner) | IrType::MutRef(inner)
                                if matches!(
                                    inner.as_ref(),
                                    IrType::Named { path, .. }
                                        if path == "HashMap" || path == "Dict"
                                )
                        );
                        if variadic_is_dict {
                            format!("({}).iter()", s)
                        } else {
                            format!("({}).iter().copied()", s)
                        }
                    } else {
                        format!("({}).into_iter()", s)
                    }
                };
                let iter_s = if let Some(g) = guard {
                    let base = if use_lazy_iter {
                        format!("({}).iter().cloned()", self.gen_expr(iter))
                    } else {
                        iter_expr(self)
                    };
                    // guard 中若使用 var.field（struct 字段），闭包参数用引用 |p| 以自动解引用；
                    // 否则（原始类型比较）用 |&x| 按值解构（Copy）
                    let guard_s = self.gen_expr(g);
                    let uses_field = guard_s.contains(&format!("{}.", var));
                    // guard 将 var 作为值传递（如 keep(it)）→ 若非 Copy 元素需闭包内 clone
                    let elem_is_primitive = matches!(
                        iter.ty,
                        IrType::Int | IrType::F64 | IrType::Bool | IrType::Str
                    ) || matches!(&iter.ty, IrType::Named { path, args } if path == "List" && args.first().map_or(false,
                            |a| matches!(a, IrType::Int | IrType::F64 | IrType::Bool | IrType::Str)));
                    let passes_by_value =
                        !uses_field && !elem_is_primitive && guard_s.contains(var);
                    if passes_by_value {
                        // 元素为非 Copy 的 struct/enum：|it| 引用参数 + 闭包内 (*it).clone() 供 guard 按值使用
                        // 注意：替换 var 必须边界感知——`i % 2 == 0` 中字面量生成 `2i64`，
                        // 无脑 replace("i", "i_owned") 会把后缀 i64 里的 i 也替换成
                        // i_owned64（invalid suffix `i_owned64`）
                        let guard_owned =
                            replace_ident_boundary(&guard_s, var, &format!("{}_owned", var));
                        format!(
                            "{}.filter(|{}| {{ let {}_owned = (*{}).clone(); {} }})",
                            base, var, var, var, guard_owned,
                        )
                    } else {
                        let pat = if uses_field {
                            format!("|{}|", var)
                        } else {
                            format!("|&{}|", var)
                        };
                        format!("{}.filter({} {})", base, pat, guard_s)
                    }
                } else if use_lazy_iter {
                    format!("({}).iter().cloned()", self.gen_expr(iter))
                } else {
                    iter_expr(self)
                };
                self.emit_line(&format!("for {} in {} {{", var, iter_s));
                self.indent += 1;
                // For loop body should not emit return for tail expressions
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                // 循环体不是值上下文：尾表达式需加分号（否则 std::thread::spawn(...) 裸生成 E0308）
                let saved_semi = self.force_stmt_semicolon;
                self.force_stmt_semicolon = true;
                self.loop_depth += 1;
                let saved_declared = self.declared.clone();
                self.gen_block_inner(body);
                self.declared = saved_declared;
                self.loop_depth -= 1;
                self.force_stmt_semicolon = saved_semi;
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}");
                self.loop_else_stack.pop();
                if let (Some(label), Some(eb)) = (else_label, else_body) {
                    // else 体尾表达式即块值（return/发散语句时块类型为 !，可强转函数返回类型）；
                    // 不追加 break 'label（会让块尾变为 () 与返回类型冲突 E0308）
                    self.gen_block_inner(&eb);
                    self.indent -= 1;
                    self.emit_line("}");
                    let _ = label;
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
            } => {
                self.emit_walrus_predecls(cond);
                // while/else：循环正常结束（非 break）执行 else 体（规范 05-控制流.md §13.3）。
                // Rust 无 while/else 语法，用 labeled block：break 'label 跳出整个块跳过 else；
                // else 体以 return/尾表达式结束，块类型由尾语句决定（已验证 rustc 接受）
                let else_label = else_body.as_ref().map(|_| {
                    self.loop_else_counter += 1;
                    let label = format!("__lz_loop_else_{}", self.loop_else_counter);
                    self.emit_line(&format!("'{}: {{", label));
                    label
                });
                if else_label.is_some() {
                    self.indent += 1;
                }
                self.loop_else_stack.push(else_label.clone());
                // while true → loop (Rust warns about while true)
                let is_infinite =
                    guard.is_none() && matches!(&cond.kind, ExprKind::Lit(LitKind::Bool(true)));
                let cond_s = if let Some(g) = guard {
                    format!("({}) && ({})", self.gen_expr(cond), self.gen_expr(g))
                } else if is_infinite {
                    String::new()
                } else {
                    self.gen_expr(cond)
                };
                if is_infinite {
                    self.emit_line("loop {");
                } else {
                    self.emit_line(&format!("while {} {{", cond_s));
                }
                self.indent += 1;
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.loop_depth += 1;
                let saved_declared = self.declared.clone();
                self.gen_block_inner(body);
                self.declared = saved_declared;
                self.loop_depth -= 1;
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}");
                self.loop_else_stack.pop();
                if let (Some(label), Some(eb)) = (else_label, else_body) {
                    // else 体尾表达式即块值（return/发散语句时块类型为 !，可强转函数返回类型）；
                    // 不追加 break 'label（会让块尾变为 () 与返回类型冲突 E0308）
                    self.gen_block_inner(&eb);
                    self.indent -= 1;
                    self.emit_line("}");
                    let _ = label;
                }
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                let expr_s = self.gen_expr(expr);
                let pat_s = self.gen_pattern(pattern);
                // 模式提取会移动 expr 的值：Var 表达式 clone 一次避免循环内移动
                let expr_s = if matches!(&expr.kind, ExprKind::Var(_)) {
                    format!("{}.clone()", expr_s)
                } else {
                    expr_s
                };
                let cond_s = if let Some(g) = guard {
                    format!("let {} = {} && {}", pat_s, expr_s, self.gen_expr(g))
                } else {
                    format!("let {} = {}", pat_s, expr_s)
                };
                self.emit_line(&format!("while {} {{", cond_s));
                self.indent += 1;
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.loop_depth += 1;
                let saved_declared = self.declared.clone();
                self.gen_block_inner(body);
                self.declared = saved_declared;
                self.loop_depth -= 1;
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}");
            }
            Stmt::Match { scrutinee, arms } => {
                // size_hint 方法体内的 match scrutinee（如 `match (hi_a, hi_b)` 匹配
                // Option 元组）不应做 usize 转换——TupleLit 的 size_hint 转换只适用于
                // 返回元组（iter.lz Zip::size_hint `let hi = match (hi_a, hi_b)`，
                // E0308 expected usize, found Option 修复）
                let saved_size_hint = self.current_fn_is_size_hint;
                self.current_fn_is_size_hint = false;
                let scrut_s = self.gen_expr(scrutinee);
                self.current_fn_is_size_hint = saved_size_hint;
                // 保留原始表达式字符串（dict 模式守卫/值绑定用），
                // 因为 scrut_str 可能被 else { scrut_s } 分支 move 走
                let scrut_orig = scrut_s.clone();
                // String 类型模式匹配：match name { "hello" => } 需要 &str
                // self (引用) → clone 以获得 owned 值用于模式匹配提取
                // 其他变量 → clone 以防止局部移动（如 Result::Err(e) 移动 e）
                let scrut_str = if matches!(&scrutinee.ty, IrType::Str) {
                    format!("{}.as_str()", scrut_s)
                } else if scrut_s == "self" {
                    "self.clone()".to_string()
                } else if matches!(&scrutinee.kind, ExprKind::FieldAccess { .. }) {
                    // ref mut 绑定（FlatMap 的 `case Some(ref mut inner_iter)`）需要
                    // owned（&mut self.inner 与臂内赋值冲突 E0499）：保留 clone；
                    // 返回 Option<&T>（Peekable 的 peek）借用匹配 &self.peeked，
                    // Some(item) 绑定 &I::Item（无 move E0507、无 E0277 转换）；
                    // 返回值（__next__ 返回 Option<I::Item>）保留 clone（Some(item)
                    // 是值，E0308 expected I::Item, found &I::Item）
                    let ret_is_ref_opt = matches!(&self.current_ret_ty,
                        Some(IrType::Option(inner)) if matches!(&**inner, IrType::Ref(_) | IrType::MutRef(_)))
                        || matches!(&self.current_ret_ty, Some(IrType::Named { path, args })
                            if path == "Option"
                                && args.first().map_or(false, |a| matches!(a, IrType::Ref(_) | IrType::MutRef(_))));
                    let has_ref_mut = arms
                        .iter()
                        .any(|a| !self.collect_ref_mut_bindings(&a.pattern).is_empty());
                    if has_ref_mut {
                        scrut_s
                    } else if ret_is_ref_opt {
                        if scrut_s.ends_with(".clone()") {
                            format!("&{}", scrut_s.trim_end_matches(".clone()"))
                        } else {
                            format!("&{}", scrut_s)
                        }
                    } else {
                        scrut_s
                    }
                } else if matches!(&scrutinee.kind, ExprKind::Var(_)) {
                    format!("{}.clone()", scrut_s)
                } else {
                    scrut_s
                };
                // 列表模式（[a, b, c] / [first, ..rest]）匹配 Vec/List：Rust 数组模式
                // 只能匹配 slice，需先 .as_slice()（否则 E0529 expected array/slice）
                let has_list_pat = arms.iter().any(|a| pattern_is_list(&a.pattern));
                let scrut_str = if has_list_pat
                    && matches!(&scrutinee.ty, IrType::Named { path, .. } if path == "List" || path == "Vec")
                {
                    format!("{}.as_slice()", scrut_str)
                } else {
                    scrut_str
                };
                // 若 match 是尾语句（其值流向外层块），保持裸 match 表达式；
                // 若为非尾语句（值被丢弃），arm 产出非 () 值时直接 `match { };`
                // 会报 E0308（expected (), found T），需用 let _ = 丢弃值。
                let discard = !is_last;
                // type-pack 异质元组（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的 args
                // 编译为切片 &[Ts]，元组模式 `(a,)` / `(a, ..)` 需转为切片模式
                // `[a]` / `[a, ..]`（Rust 切片模式），臂体 a 绑定 &Ts
                let is_slice_scrutinee = matches!(
                    &scrutinee.ty,
                    IrType::Named { path, .. } if path == "List" || path == "Vec" || path == "Tuple"
                );
                let open = if discard {
                    format!("let _ = match {} {{", scrut_str)
                } else {
                    format!("match {} {{", scrut_str)
                };
                self.emit_line(&open);
                self.indent += 1;
                let mut arm_i = 0usize;
                for arm in arms {
                    // 字典模式（{"k": p}）：Rust 无原生 HashMap 模式 → 生成
                    // `_ if <scrut>.contains_key("k")` 守卫 + 臂体内 `let p = <scrut>["k"];`
                    let dict_entries = match &arm.pattern {
                        Pattern::Dict(entries) => Some(entries.clone()),
                        _ => None,
                    };
                    let mut pre_body: Vec<String> = Vec::new();
                    let pat_s = if dict_entries.is_some() {
                        "_".to_string()
                    } else if is_slice_scrutinee {
                        // 切片上下文：元组模式 (a,) / (a, ..) → 切片模式 [a] / [a, ..]
                        self.gen_slice_pattern(&arm.pattern)
                    } else if let Pattern::Struct { name, fields } = &arm.pattern {
                        // struct 提取器（对应 Scala unapply / unapplySeq）：
                        // case struct 或显式实现 __unapply__ 时，经 __unapply__ 完成提取，
                        // 而非字段解构（字段可私有，且支持自定义提取逻辑）。
                        if self.struct_has_unapply.contains(name) {
                            let bind_var = format!("__lz_ext_{}", arm_i);
                            let has_rest =
                                fields.iter().any(|(_, p)| matches!(p, Pattern::Rest(_)));
                            if has_rest && self.struct_has_unapply_seq.contains(name) {
                                // 变长提取：__unapply_seq__ 返回 Vec<T>（字段同构）
                                pre_body
                                    .push(format!("let __seq = {}.__unapply_seq__();", bind_var));
                                let mut idx = 0usize;
                                for (_, p) in fields {
                                    if let Pattern::Rest(rest_name) = p {
                                        let rname = rest_name
                                            .clone()
                                            .unwrap_or_else(|| "__rest".to_string());
                                        pre_body.push(format!(
                                            "let {} = __seq[{}..].to_vec();",
                                            rname, idx
                                        ));
                                    } else {
                                        let bname = self.gen_pattern(p);
                                        pre_body.push(format!(
                                            "let {} = __seq[{}].clone();",
                                            bname, idx
                                        ));
                                        idx += 1;
                                    }
                                }
                                format!("{} @ {} {{ .. }}", bind_var, name)
                            } else {
                                // 定长提取：__unapply__ 返回 (f1, f2, ...)
                                let binds: Vec<String> =
                                    fields.iter().map(|(_, p)| self.gen_pattern(p)).collect();
                                pre_body.push(format!(
                                    "let ({}) = {}.__unapply__();",
                                    binds.join(", "),
                                    bind_var
                                ));
                                format!("{} @ {} {{ .. }}", bind_var, name)
                            }
                        } else {
                            self.gen_pattern(&arm.pattern)
                        }
                    } else {
                        self.gen_pattern(&arm.pattern)
                    };
                    let guard_s = if let Some(entries) = &dict_entries {
                        let conds: Vec<String> = entries
                            .iter()
                            .map(|(k, _)| {
                                format!(
                                    "{}.contains_key(\"{}\")",
                                    scrut_orig,
                                    k.replace('"', "\\\"")
                                )
                            })
                            .collect();
                        format!(" if {}", conds.join(" && "))
                    } else {
                        arm.guard
                            .as_ref()
                            .map(|g| format!(" if {}", self.gen_expr(g)))
                            .unwrap_or_default()
                    };
                    self.emit_line(&format!("{} => {{", format!("{}{}", pat_s, guard_s)));
                    self.indent += 1;
                    // 字典模式值绑定：let p = <scrut>["k"];
                    if let Some(entries) = &dict_entries {
                        for (k, p) in entries {
                            let bind_name = self.gen_pattern(p);
                            self.emit_line(&format!(
                                "let {} = {}[\"{}\"].clone();",
                                bind_name,
                                scrut_orig,
                                k.replace('"', "\\\"")
                            ));
                        }
                    }
                    // 为递归枚举 Box 字段自动插入 let binding = *binding; 解引用
                    let box_bindings = self.collect_box_pattern_bindings(&arm.pattern);
                    for b in &box_bindings {
                        self.emit_line(&format!("let {} = *{};", b, b));
                    }
                    // struct 提取器（__unapply__ / __unapply_seq__）注入的绑定
                    for line in &pre_body {
                        self.emit_line(line);
                    }
                    // 收集 `ref mut` 模式绑定名：臂体内 c = c + 1 需生成 *c = *c + 1
                    // （E0384：ref mut c 绑定为 &mut，直接赋值给不可变引用报错）
                    let saved_ref_mut = self.ref_mut_bindings.clone();
                    self.ref_mut_bindings = self.collect_ref_mut_bindings(&arm.pattern);
                    // type-pack 切片模式绑定（03d §2.8 方案 B）：`[a]` / `[a, ..]` 中
                    // a 绑定 &Ts（引用），臂体内引用 a 需生成 a.clone()（E0308 修复）
                    let saved_slice_clone = self.slice_clone_bindings.clone();
                    if is_slice_scrutinee {
                        let mut bindings = Vec::new();
                        self.collect_slice_bindings(&arm.pattern, &mut bindings);
                        for b in bindings {
                            self.slice_clone_bindings.insert(b);
                        }
                    }
                    // Match arm body 不应生成 return（值应流向 match 表达式外层）
                    let saved = self.suppress_tail_return;
                    self.suppress_tail_return = true;
                    // 各 match 臂作用域独立：臂内 `let mut x` 不应影响其他臂的
                    // declared 判定（否则前一臂声明的同名变量使后一臂 `let mut x`
                    // 被当成赋值 `x = ...` 生成，E0425 cannot find value，v180 缺陷）
                    let saved_declared = self.declared.clone();
                    // 枚举变体字段绑定：注册已知类型到 str_typed_vars（f-string 用 {} 而非 {:?}）
                    let saved_str_bindings: Vec<String> =
                        self.str_typed_vars.iter().cloned().collect();
                    self.register_enum_variant_str_bindings(&arm.pattern);
                    self.gen_block_inner(&arm.body);
                    // 恢复 str_typed_vars 到臂前状态（避免跨臂泄漏）
                    self.str_typed_vars = saved_str_bindings.into_iter().collect();
                    self.declared = saved_declared;
                    self.suppress_tail_return = saved;
                    self.ref_mut_bindings = saved_ref_mut;
                    self.slice_clone_bindings = saved_slice_clone;
                    self.indent -= 1;
                    self.emit_line("}");
                    arm_i += 1;
                }
                // type-pack 切片模式（03d §2.8 方案 B）：args 是 &[Ts]，若用户臂未
                // 覆盖空切片 `&[]`，自动追加通配兜底臂（否则 E0004 non-exhaustive）
                if is_slice_scrutinee
                    && !arms.iter().any(|a| matches!(a.pattern, Pattern::Wildcard))
                {
                    self.emit_line("_ => {");
                    self.indent += 1;
                    self.emit_line("panic!(\"unexpected empty args\");");
                    self.indent -= 1;
                    self.emit_line("}");
                }
                self.indent -= 1;
                // discard=true 时需以 `};` 关闭（let _ = match {...};），否则仅 `}`
                self.emit_line(if discard { "};" } else { "}" });
            }
            Stmt::Break => {
                // plain block（block NAME: → (|| { ... })() 闭包）内顶层 break：
                // 闭包内裸 break 非法（E0267），应生成 return 退出闭包（跳出 block）。
                // 循环内的 break 仍跳出循环（loop_depth > 0）。
                if self.plain_block_depth > 0 && self.loop_depth == 0 {
                    self.emit_line("return; // break block");
                    return;
                }
                // 循环带 else 子句时：break 需跳出 labeled block 跳过 else 体
                if let Some(Some(label)) = self.loop_else_stack.last() {
                    self.emit_line(&format!("break '{};", label));
                } else {
                    self.emit_line("break;");
                }
            }
            Stmt::BreakLabel { label: _, value: _ } => {
                // block 内 break label → 无值 return（退出闭包）
                // 注意：`break NAME with v`（触发 checker 块）不走本分支——parser 将其
                // 解析为 BlockCall，builder 转为 Call，checker 打包调用已实现（见
                // ExprKind::Call 的 __Params 打包分支）；本分支仅覆盖纯标签跳出
                self.emit_line("return; // break block");
            }
            Stmt::Continue => self.emit_line("continue;"),
            Stmt::BlockLabel { label, body } => {
                // plain 块：压缩为无参闭包（定义即执行，闭包语义）
                // block scan: ... break scan → (|| { ... return; })()
                //
                // **尾位置例外**：命名块恰好是函数尾表达式时，块值就是返回值。
                // 原先一律发 `(|| { .. })();` 会把值丢掉 ⇒ E0308
                // （`pub fn f(..) -> i64 { (|| { .. })(); }`）。
                // 此时先把闭包结果绑到临时量再 return；块内 `break label`
                // 仍编译成 `return;`（退出闭包）⇒ 该路径块值为 ()，语义一致。
                let tail_pos = is_last && !self.suppress_tail_return && !self.is_main;
                if tail_pos {
                    self.emit_line(&format!("let __blk_val = (|| {{ // block '{} tail", label));
                } else {
                    self.emit_line(&format!("(|| {{ // block '{}", label));
                }
                self.indent += 1;
                self.plain_block_depth += 1;
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.gen_block_inner(body);
                self.suppress_tail_return = saved;
                self.plain_block_depth -= 1;
                self.indent -= 1;
                self.emit_line("})();");
                if tail_pos {
                    self.emit_line("return __blk_val;");
                }
            }
            Stmt::CheckerBlock { .. } => {
                // checker 块已提升为模块级 Item::CheckerBlock（惰性登记）
                // 此处为占位语句，不生成内联代码
                self.emit_line("();  // checker block (defined at module level)");
            }
            Stmt::Pass => {
                // pass 占位：非 Unit 返回函数中（如 box.lz `fn get(&self) -> &T` 的
                // 内建占位方法）生成 unimplemented!()，否则 `()` 与返回类型不匹配（E0308）
                let ret_is_unit = matches!(self.current_ret_ty, None | Some(IrType::Unit));
                if !ret_is_unit {
                    self.emit_line("unimplemented!()");
                } else {
                    self.emit_line("();  // pass");
                }
            }
            Stmt::TypeAlias { name, ty } => {
                self.emit_line(&format!("// type {} = {};", name, self.rust_type(ty)));
            }
            Stmt::Raise { value } => {
                // BUG-CG-004（轮次12）：raises 函数内 raise → `return Result::Err(..)`，
                // 使错误经 Result 传播（而非 panic!）。非 raises 函数保持 panic!（catch_unwind 仍可捕获）。
                // `return Err` 在表达式位同样合法（从外层函数/闭包返回），故无需在 builder 层提升。
                if self.current_fn_raises.is_some() {
                    self.emit_line(&format!("return Result::Err({});", self.gen_expr(value)));
                } else {
                    self.emit_line(&format!("panic!(\"{{:?}}\", {});", self.gen_expr(value)));
                }
            }
            Stmt::Assert { cond, message } => {
                // `assert cond, "msg"` → assert!(cond, "{:?}", msg)（消息串，规范 SYNTAX/15 §六）
                match message {
                    Some(m) => self.emit_line(&format!(
                        "assert!({}, \"{{:?}}\", {});",
                        self.gen_expr(cond),
                        self.gen_expr(m)
                    )),
                    None => self.emit_line(&format!("assert!({});", self.gen_expr(cond))),
                }
            }
            Stmt::Yield { value } => {
                // 生成器构建块（func *:）闭包内：yield 参数包 → push 到闭包收集器 __bb
                if self.in_gen_build {
                    let val_s = self.gen_expr(value);
                    let is_copy = matches!(&value.ty, IrType::Int | IrType::F64 | IrType::Bool)
                        || matches!(&value.ty, IrType::Named { path, .. }
                            if path == "String" && val_s.contains(".clone()"));
                    let val_s = if is_copy {
                        val_s
                    } else {
                        format!("{}.clone()", val_s)
                    };
                    self.emit_line(&format!("__bb.push({});", val_s));
                    return;
                }
                // 非 Copy 泛型值（T）push 需 clone，避免 move（E0382/E0507）
                let val_s = self.gen_expr(value);
                let is_copy = matches!(&value.ty, IrType::Int | IrType::F64 | IrType::Bool)
                    || matches!(&value.ty, IrType::Named { path, .. }
                        if path == "String" && val_s.contains(".clone()"));
                let val_s = if is_copy {
                    val_s
                } else {
                    format!("{}.clone()", val_s)
                };
                self.emit_line(&format!("__gen_vec.push({});", val_s));
            }
            Stmt::YieldFrom { iter } => {
                if self.in_gen_build {
                    self.emit_line(&format!(
                        "__bb.extend({}.into_iter());",
                        self.gen_expr(iter)
                    ));
                    return;
                }
                self.emit_line(&format!("// yield from {}", self.gen_expr(iter)));
                self.emit_line(&format!(
                    "__gen_vec.extend({}.into_iter());",
                    self.gen_expr(iter)
                ));
            }
            Stmt::Defer { body } => {
                // BUG-IR-002 方案 A（内联脱糖）：仅收集 defer 体，不在原地生成。
                // 所属块（gen_block_inner / Stmt::Block）退出前由 flush_deferred 逆序（LIFO）
                // 原样内联 emit；体语句与块同作用域，无闭包捕获，规避 &mut 接收者 E0499。
                self.deferred.push(body.clone());
            }
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                self.emit_line("// TRY_START");
                // try/catch → std::panic::catch_unwind pattern
                let has_catch = !catches.is_empty();
                let has_else = else_body.is_some();
                let has_finally = finally_body.is_some();

                // ── BUG-CG-004（轮次12）：try/catch 结果基分支 ──
                // 当 try 体类型为 Result<T,E>（体内调用了 raises 函数，或体本身返回 Result），
                // raise 已被 builder 改写为 `return Err(...)`，不再触发 panic。此时必须改用
                // `match` 捕获 Err（而非 catch_unwind 捕获 unwind），否则 catch 永不触发。
                // 非 Result 体（panic 基）仍走下方 catch_unwind 路径，保持向后兼容。
                // 显式 Ok(...) / Err(...) 构造器（无 raises 调用，失败靠 unwrap 等
                // panic）→ 必须走 panic 基 catch_unwind。若误判为 Result 基，
                // `Ok(__ok_val) => __ok_val` 臂是闭包返回值（整个内层 Result），
                // 而 catch 臂尾是 `Err(...)` 构造值 → 两臂类型不兼容（operators.lz
                // parse_int 回归 E0308）。builder 的 Ok/Err 构造器类型推断会把这些
                // 尾表达式标成 Result，故此处按构造器形态排除。
                let is_result_ctor = |e: &Expr| {
                    matches!(&e.kind, ExprKind::Call { callee, .. }
                        if matches!(&callee.kind, ExprKind::Var(n) if n == "Ok" || n == "Err"))
                };
                let tail_expr = body.stmts.iter().rev().find_map(|s| match s {
                    Stmt::ExprStmt { expr } => Some(expr),
                    _ => None,
                });
                let mut use_result_try = matches!(&body.ty, IrType::Result { .. })
                    && !tail_expr.map(is_result_ctor).unwrap_or(false);
                // body 尾表达式非 Result（如 print(v) 返回 Unit），但 body 内含
                // raises 函数调用（`let v = checked(21)` → v: Result<int,str>），
                // 仍需走 Result 路径。扫描 body 的 Let/Expr 语句检测 Result 类型
                // （显式 Ok/Err 构造器不是 raises 调用，跳过）。
                if !use_result_try {
                    'scan: for stmt in &body.stmts {
                        let (ty, expr) = match stmt {
                            Stmt::Let { value, .. } => (Some(&value.ty), Some(value)),
                            Stmt::ExprStmt { expr } => (Some(&expr.ty), Some(expr)),
                            _ => (None, None),
                        };
                        if let Some(t) = ty {
                            if matches!(t, IrType::Result { .. })
                                && !expr.map(is_result_ctor).unwrap_or(false)
                            {
                                use_result_try = true;
                                break 'scan;
                            }
                        }
                    }
                }
                if use_result_try {
                    // 从 body.ty 或 body 内语句推断 Result<ok, err> 类型
                    let result_ty_from_body = if let IrType::Result { .. } = &body.ty {
                        Some(body.ty.clone())
                    } else {
                        body.stmts.iter().find_map(|stmt| {
                            let ty = match stmt {
                                Stmt::Let { value, .. } => Some(&value.ty),
                                Stmt::ExprStmt { expr } => Some(&expr.ty),
                                _ => None,
                            };
                            match ty {
                                Some(IrType::Result { ok, err }) => Some(IrType::Result {
                                    ok: ok.clone(),
                                    err: err.clone(),
                                }),
                                _ => None,
                            }
                        })
                    };
                    if let Some(IrType::Result {
                        ok: _inner_ok,
                        err: err_ty,
                    }) = &result_ty_from_body
                    {
                        // 闭包 ok 类型 = body 尾表达式的「解包后」类型：
                        // - 尾即 raises 调用（body.ty 本身是 Result，如 `checked_div(a,b)`）：
                        //   闭包直接返回该 Result，ok 取内层 ok（i64），不可再包一层
                        //   （旧回归：rust_type(&body.ty) 产出 Result<Result<..>,E> → E0308）
                        // - 尾非 Result（如 print(v)→Unit，Result 藏在 body 的 Let 里）：
                        //   Let 已带 `?` 解包，闭包 ok = body.ty 原样（如 ()）
                        let ok_rust = if let IrType::Result { ok, .. } = &body.ty {
                            self.rust_type(ok)
                        } else {
                            self.rust_type(&body.ty)
                        };
                        let err_rust = self.rust_type(err_ty);
                        // 闭包返回 Result<ok, err>：体内 raise→return Err 仅从该闭包返回，
                        // 不会提前返回外层函数。
                        self.emit_line(&format!(
                            "let __try_result: Result<{}, {}> = (|| -> Result<{}, {}> {{",
                            ok_rust, err_rust, ok_rust, err_rust
                        ));
                        self.indent += 1;
                        let saved = self.suppress_tail_return;
                        self.suppress_tail_return = true;
                        let saved_rt = self.in_result_try;
                        self.in_result_try = true;
                        self.gen_block_inner(body);
                        self.in_result_try = saved_rt;
                        // body 尾表达式非 Result 时（如 print(v)→Unit），闭包体末尾
                        // 需包 Ok() 使返回类型匹配 Result<(), E>
                        if !matches!(&body.ty, IrType::Result { .. }) {
                            let last = self.last_emitted_line().to_string();
                            if !last.ends_with(';') && !last.ends_with('}') && !last.is_empty() {
                                self.append_to_last_line(";");
                            }
                            self.emit_line("Ok(())");
                        }
                        self.suppress_tail_return = saved;
                        self.indent -= 1;
                        self.emit_line("})();");

                        self.emit_line("let __try_val = match __try_result {");
                        self.indent += 1;
                        if has_else {
                            self.emit_line("Ok(__ok_val) => {");
                            self.indent += 1;
                            let _saved = self.suppress_tail_return;
                            self.suppress_tail_return = true;
                            self.gen_block_inner(else_body.as_ref().unwrap());
                            self.suppress_tail_return = _saved;
                            self.indent -= 1;
                            self.emit_line("},");
                        } else {
                            self.emit_line("Ok(__ok_val) => __ok_val,");
                        }
                        self.emit_line("Err(__err_val) => {");
                        self.indent += 1;
                        // 结果基可真正按类型匹配 Err（catch_unwind 只能字符串化），
                        // 支持多 catch 分支（与 panic 基只取末支不同）。
                        self.emit_line("match __err_val {");
                        self.indent += 1;
                        for (pat, block) in catches.iter() {
                            // 模式绑定里 line/column/file 会被降级为 `line_`（见 pattern_to_rust_pat），
                            // 必须同步登记进 downgraded_vars，否则 body 里引用仍是裸 `line`，
                            // 未绑定 → 被 Rust 解析成 `line!` 宏（E0423）。与 panic 基 catch 保持一致。
                            if let Some(p) = pat {
                                for id in self.pattern_idents(p) {
                                    if matches!(id.as_str(), "line" | "column" | "file") {
                                        self.downgraded_vars.insert(id);
                                    }
                                }
                            }
                            let pat_str = match pat {
                                Some(Pattern::Enum {
                                    enum_name,
                                    variant,
                                    args,
                                }) => {
                                    let args_s: Vec<String> =
                                        args.iter().map(|a| self.pattern_to_rust_pat(a)).collect();
                                    format!("{}::{}(", enum_name, variant)
                                        + &args_s.join(", ")
                                        + ")"
                                }
                                Some(Pattern::Ident(name)) => {
                                    if matches!(name.as_str(), "line" | "column" | "file") {
                                        format!("{}_", name)
                                    } else {
                                        name.clone()
                                    }
                                }
                                Some(Pattern::Wildcard) | None => "_".to_string(),
                                _ => "_".to_string(),
                            };
                            self.emit_line(&format!("{} => {{", pat_str));
                            self.indent += 1;
                            self.gen_block_inner(block);
                            self.indent -= 1;
                            self.emit_line("},");
                        }
                        // 未匹配 Err：重新抛出（外层函数有 raises → return Err；否则 panic）
                        self.emit_line("_ => {");
                        self.indent += 1;
                        if self.current_fn_raises.is_some() {
                            self.emit_line("return Err(__err_val);");
                        } else {
                            self.emit_line(&format!(
                                "panic!(\"uncaught error: {{:?}}\", __err_val);"
                            ));
                        }
                        self.indent -= 1;
                        self.emit_line("}");
                        self.indent -= 1;
                        self.emit_line("}"); // end match __err_val
                        self.indent -= 1;
                        self.emit_line("},"); // end Err arm
                        self.indent -= 1;
                        self.emit_line("};"); // end match __try_result

                        // ── finally cleanup + return value ──
                        if has_finally {
                            self.emit_line("let __final_val = __try_val;");
                            let _saved = self.suppress_tail_return;
                            self.suppress_tail_return = true;
                            self.gen_block_inner(finally_body.as_ref().unwrap());
                            self.suppress_tail_return = _saved;
                            if !self.last_emitted_line().ends_with(';')
                                && !self.last_emitted_line().ends_with('}')
                                && !self.last_emitted_line().is_empty()
                            {
                                self.append_to_last_line(";");
                            }
                            self.emit_line("__final_val");
                        } else {
                            self.emit_line("__try_val");
                        }
                    }
                    return;
                }

                // ── catch_unwind wrapping（panic 基，非 Result 体） ──
                // suppress_tail_return = true: closure body's last expr is the return value (no explicit return)
                self.emit_line("let __panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {");
                self.indent += 1;
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.gen_block_inner(body);
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}));");

                if has_catch || has_else {
                    self.emit_line("let __try_val = match __panic_result {");
                    self.indent += 1;
                    // try/else 语义（05-控制流.md §13.4）：try 成功时执行 else 体，
                    // 表达式值为 else 体的值；无 else 时值为 try 体末表达式。
                    // 存在 else 时 Ok 分支返回 else 块值（否则 Ok 返回 i64、Err 返回
                    // String → match arms 类型不兼容 E0308，如 combo-defer-guard.lz）
                    if has_else {
                        self.emit_line("Ok(_val) => {");
                        self.indent += 1;
                        let _saved = self.suppress_tail_return;
                        self.suppress_tail_return = true;
                        self.gen_block_inner(else_body.as_ref().unwrap());
                        self.suppress_tail_return = _saved;
                        self.indent -= 1;
                        self.emit_line("},");
                    } else {
                        self.emit_line("Ok(val) => val,");
                    }
                    self.emit_line("Err(_panic) => {");
                    self.indent += 1;
                    // Catch handlers: suppress tail return — values flow through match expr
                    // (explicit return statements still work via Stmt::Return handler)
                    let _saved = self.suppress_tail_return;
                    self.suppress_tail_return = true;
                    if catches.len() > 1 {
                        // Multi-catch: emit only the last catch arm (catch-all).
                        // catch_unwind can't do type-specific downcasting at codegen level.
                        // Specific-type catches are emitted as comments for documentation.
                        for (i, (pat, block)) in catches.iter().enumerate() {
                            if i < catches.len() - 1 {
                                // Specific-type catch → comment only
                                let pat_str = match pat {
                                    Some(Pattern::Ident(name)) => name.clone(),
                                    Some(pat) => format!("{:?}", pat),
                                    None => "(catch-all)".into(),
                                };
                                self.emit_line(&format!("// catch {}: (specific-type catch not supported with catch_unwind)", pat_str));
                            } else {
                                // Last arm is the catch-all（也支持 Enum 模式绑定：ParseError(line, msg)）
                                let var_names: Vec<String> = match pat {
                                    Some(Pattern::Ident(name)) => vec![name.clone()],
                                    Some(Pattern::Enum { args, .. }) => args
                                        .iter()
                                        .filter_map(|a| {
                                            if let Pattern::Ident(n) = a {
                                                Some(n.clone())
                                            } else {
                                                None
                                            }
                                        })
                                        .collect(),
                                    _ => Vec::new(),
                                };
                                for var_name in &var_names {
                                    // line/column/file 与 Rust 内置宏冲突 → 降级重命名
                                    if matches!(var_name.as_str(), "line" | "column" | "file") {
                                        self.downgraded_vars.insert(var_name.clone());
                                    }
                                    let safe = if self.downgraded_vars.contains(var_name.as_str()) {
                                        format!("{}_", var_name)
                                    } else {
                                        var_name.clone()
                                    };
                                    self.emit_line(&format!(
                                        "let {} = format!(\"{{:?}}\", _panic);",
                                        safe
                                    ));
                                    self.declared.insert(var_name.clone());
                                }
                                self.gen_block_inner(block);
                            }
                        }
                    } else {
                        for (pat, block) in catches {
                            // Bind catch variable from panic info
                            // Pattern can be simple Ident or Enum variant with args (e.g. MathError.DivByZero(msg))
                            let var_names: Vec<String> = match pat {
                                Some(Pattern::Ident(name)) => vec![name.clone()],
                                Some(Pattern::Enum { args, .. }) => args
                                    .iter()
                                    .filter_map(|a| {
                                        if let Pattern::Ident(n) = a {
                                            Some(n.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect(),
                                _ => Vec::new(),
                            };
                            for var_name in &var_names {
                                // line/column/file 与 Rust 内置宏冲突 → 降级重命名
                                if matches!(var_name.as_str(), "line" | "column" | "file") {
                                    self.downgraded_vars.insert(var_name.clone());
                                }
                                let safe = if self.downgraded_vars.contains(var_name.as_str()) {
                                    format!("{}_", var_name)
                                } else {
                                    var_name.clone()
                                };
                                self.emit_line(&format!(
                                    "let {} = format!(\"{{:?}}\", _panic);",
                                    safe
                                ));
                                self.declared.insert(var_name.clone());
                            }
                            self.gen_block_inner(block);
                        }
                    }
                    self.suppress_tail_return = _saved;
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("};");
                    // else_body 已在 Ok 分支内联为 match arm 值（try/else 语义 §13.4），
                    // 此处不再重复生成（否则 else 块尾值类型与语句上下文冲突 E0308）
                } else {
                    // No catch/else: unwrap the result (re-panics on error)
                    self.emit_line("let __try_val = __panic_result.unwrap();");
                }
                self.emit_line("// TRY_END");

                // ── finally cleanup + return value ──
                if has_finally {
                    // Save value, run cleanup statements, then return value
                    self.emit_line("let __final_val = __try_val;");
                    // Emit all finally statements with semicolons (suppress tail = true → bare expr, then append ;)
                    let _saved = self.suppress_tail_return;
                    self.suppress_tail_return = true;
                    self.gen_block_inner(finally_body.as_ref().unwrap());
                    self.suppress_tail_return = _saved;
                    // Fix: ensure last finally statement ends with ; before __final_val
                    if !self.last_emitted_line().ends_with(';')
                        && !self.last_emitted_line().ends_with('}')
                        && !self.last_emitted_line().is_empty()
                    {
                        self.append_to_last_line(";");
                    }
                    self.emit_line("__final_val");
                } else {
                    self.emit_line("__try_val");
                }
            }
            Stmt::Block { stmts } => {
                self.emit_line("// BLOCK_OPEN");
                self.emit_line("{");
                self.indent += 1;
                // Block 中的 tail stmt 不应用 return 包裹（defer 等场景）
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                // 块级作用域：块内新声明的变量在块结束后不可见。
                // 保存 declared 快照，块内正常累积（继承外层变量以支持 `x = v` 对外层赋值），
                // 块结束时恢复——否则第二个 test 块的 `let mut d` 会被当成已声明变量的
                // 纯赋值（d = Dict()，E0425 cannot find value `d`）
                let saved_declared = self.declared.clone();
                // 块级作用域：块内新声明的变量在块结束后不可见。
                let saved_deferred = std::mem::take(&mut self.deferred);
                let saved_semi = self.force_stmt_semicolon;
                let has_defer = stmts.iter().any(|s| matches!(s, Stmt::Defer { .. }));
                if has_defer {
                    self.suppress_tail_return = true;
                    self.force_stmt_semicolon = true;
                }
                let n = stmts.len();
                for (i, s) in stmts.iter().enumerate() {
                    self.gen_stmt(s, i == n - 1);
                }
                self.flush_deferred();
                self.declared = saved_declared;
                self.deferred = saved_deferred;
                self.suppress_tail_return = saved;
                self.force_stmt_semicolon = saved_semi;
                self.indent -= 1;
                self.emit_line("}");
                self.emit_line("// BLOCK_CLOSE");
            }
            #[allow(unreachable_patterns)]
            _ => unreachable!("TODO: unsupported stmt kind: {:?}", stmt),
        }
    }

    // ── Expr 生成 ──

    /// 生成索引 key：Rust 的 Vec/切片/字符串索引需要 usize，
    /// 而 LZ 的 int 是 i64，因此对整数索引自动转换为 usize。
    /// 对 HashMap/Dict 保持引用语义（contains_key/get 需要 &K）。
    pub(crate) fn gen_index_key(&self, key: &Expr, base: &Expr) -> String {
        // Range 切片 key（string.lz slice `self[start..end]`）：AST Range →
        // StructCtor{name:"Range"}，start/end 需转 usize（str/Vec 索引要求 usize）
        if let ExprKind::StructCtor { name, fields } = &key.kind {
            if name == "Range" {
                let start = fields
                    .iter()
                    .find(|(n, _)| n == "start")
                    .map(|(_, v)| format!("(({}) as usize)", self.gen_expr(v)));
                let end = fields
                    .iter()
                    .find(|(n, _)| n == "end")
                    .map(|(_, v)| format!("(({}) as usize)", self.gen_expr(v)));
                let inclusive = fields.iter().any(|(n, v)| {
                    n == "inclusive" && matches!(&v.kind, ExprKind::Lit(LitKind::Bool(true)))
                });
                return match (start, end) {
                    (Some(s), Some(e)) if inclusive => format!("{}..={}", s, e),
                    (Some(s), Some(e)) => format!("{}..{}", s, e),
                    (Some(s), None) => format!("{}..", s),
                    (None, Some(e)) => format!("..{}", e),
                    _ => "0usize..0usize".to_string(),
                };
            }
        }
        let is_dict =
            matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
        // String/str 不能用 usize 索引（Rust 限制），需用 chars().nth()
        let is_str = matches!(&base.ty, IrType::Str)
            || matches!(&base.ty, IrType::Named { path, .. } if path == "String" || path == "str");
        // 容器（Vec/List）索引的 key 需为 usize：
        // - 整数 key（i64）直接转换
        // - 类型未知（Any）的变量 key（如 for 循环变量 items[i]）也转换，
        //   避免 Rust 切片索引需要 usize（E0277）
        let is_container = matches!(&base.ty, IrType::Named { path, .. }
            if path == "Vec" || path == "List" || path == "Array" || path == "Set" || path == "HashSet");
        let key_is_numeric = matches!(&key.ty, IrType::Int)
            || (is_container
                && matches!(&key.ty, IrType::Any)
                && !matches!(&key.kind, ExprKind::Var(n) if n == "pass"))
            || (matches!(&key.ty, IrType::Any) && matches!(&key.kind, ExprKind::Var(_)));
        // 若 key 类型含泛型参数（K, V, T 等）：不可能是数值索引——跳过 as usize
        let key_is_numeric = key_is_numeric && !matches!(&key.ty, IrType::Generic(_));
        // 对整数 key（i64）转换为 usize，除非目标是 dict（其 key 不是数值索引）或 String/str
        if !is_dict && !is_str && key_is_numeric {
            let key_s = self.gen_expr(key);
            // key 是复合表达式（如 self.len() - 1）时需整体加括号再 as usize，
            // 否则 `A - 1 as usize` 的 as 只应用到尾部（E0277 i64 - usize）
            format!("(({}) as usize)", key_s)
        } else {
            let key_s = self.gen_expr(key);
            // 在容器索引（Vec/List）场景下，若 key 是 self 的 int 字段（impl 内），也转 usize
            // 结构模式：self.container[self.index] → base 与 key 均为 self.字段
            let is_self_field_key = matches!(&key.kind,
                ExprKind::FieldAccess { base: b, .. } if matches!(&b.kind, ExprKind::Var(n) if n == "self"));
            let is_self_field_base = matches!(&base.kind,
                ExprKind::FieldAccess { base: b, .. } if matches!(&b.kind, ExprKind::Var(n) if n == "self"));
            let is_container_base = matches!(&base.ty, IrType::Named { path, .. }
                if path == "Vec" || path == "List" || path == "Array" || path == "HashMap" || path == "Dict" || path == "Set");
            if !is_dict && !is_str && is_self_field_key && (is_self_field_base || is_container_base)
            {
                format!("({} as usize)", key_s)
            } else {
                key_s
            }
        }
    }

    /// 生成赋值目标表达式（不放 unsafe 包装，用于 Stmt::Assign 等）
    pub(crate) fn gen_target_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Var(name) => name.clone(),
            ExprKind::FieldAccess { base, field } => {
                // type-pack 异质元组索引（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的 args
                // 编译为切片 &[Ts]，`args.0` 映射为 `args[0]`（Rust 切片索引）；
                // 数字字段名仅在 base 是集合/切片类型时按索引处理
                let is_numeric_field =
                    !field.is_empty() && field.chars().all(|c| c.is_ascii_digit());
                if is_numeric_field
                    && matches!(
                        &base.ty,
                        IrType::Named { path, .. } if path == "List" || path == "Vec" || path == "Tuple"
                    )
                {
                    format!("{}[{}]", self.gen_target_expr(base), field)
                } else {
                    // 关联类型路径（06c-trait定义.md §五）：`I.Item` → `I::Item`
                    // （泛型参数上的关联类型用 ::，E0423 expected value, found type parameter）
                    // base 是泛型参数（I/A/B 等）且 field 大写开头（Item）时按关联类型处理
                    let base_s = self.gen_target_expr(base);
                    let field_is_upper = field.chars().next().map_or(false, |c| c.is_uppercase());
                    let base_is_generic = matches!(&base.kind, ExprKind::Var(n)
                        if n != "self"
                            && !self.downgraded_vars.contains(n.as_str())
                            && !self.global_vars.contains_key(n.as_str())
                            && !self.known_types.contains(n.as_str())
                            && !self.emitted_types.contains(n.as_str())
                            && (self.in_generic_fn || self.in_impl_generic || !self.param_renames.is_empty() || self.current_variadic_params.contains(n.as_str())));
                    if field_is_upper && base_is_generic {
                        format!("{}::{}", base_s, field)
                    } else if field_is_upper
                        && matches!(&base.kind, ExprKind::Var(n)
                            if n != "self"
                                && !self.downgraded_vars.contains(n.as_str())
                                && !self.global_vars.contains_key(n.as_str())
                                && !self.known_types.contains(n.as_str())
                                && !self.emitted_types.contains(n.as_str())
                                && n.chars().next().map_or(false, |c| c.is_uppercase()))
                    {
                        // 未声明类型名上的大写字段（Ordering.Less / Result.Ok）→
                        // 枚举变体访问 Ordering::Less（Rust 枚举变体需 :: 连接），
                        // 否则生成 `Ordering.Less` 报语法错误
                        format!("{}::{}", base_s, field)
                    } else {
                        // 命名字段枚举字段访问: enum E: X(v: i64) 中 `x.v`
                        // → match &x { E::X { v, .. } => v.clone(), _ => unreachable!() }
                        // （Rust 不允许对枚举值直接 .v 访问，需解构）
                        let named_enum_field: Option<String> = match &base.ty {
                            IrType::Named { path, .. } => {
                                self.enum_variant_named_fields
                                    .iter()
                                    .find(|((e, _), _)| e == path)
                                    .filter(|((_, _), fields)| fields.contains(field))
                                    .map(|((_, variant), _)| {
                                        format!(
                                            "match &{} {{ {}::{} {{ {}, .. }} => {}.clone(), _ => unreachable!() }}",
                                            base_s, path, variant, field, field
                                        )
                                    })
                            }
                            _ => None,
                        };
                        if let Some(expr_s) = named_enum_field {
                            expr_s
                        } else {
                            format!("{}.{}", base_s, field)
                        }
                    }
                }
            }
            ExprKind::IndexGet { base, key } => {
                let key_s = self.gen_index_key(key, base);
                let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                let key_expr = if is_dict {
                    format!("&{}", key_s)
                } else {
                    key_s
                };
                let idx_s = format!("{}[{}]", self.gen_target_expr(base), key_expr);
                // `return self[i]`（__getitem__ 返回 ref T）或 `Some(self[i])`
                // （返回 Option<ref T>）：Rust 的 a[i] 是 *index()（T 值），
                // 需 & 取引用（E0308 expected &T, found T）
                let ret_is_ref_like = matches!(
                    &self.current_ret_ty,
                    Some(IrType::Ref(_) | IrType::MutRef(_))
                ) || matches!(&self.current_ret_ty, Some(IrType::Option(inner))
                            if matches!(&**inner, IrType::Ref(_) | IrType::MutRef(_)))
                    || matches!(&self.current_ret_ty, Some(IrType::Named { path, args })
                            if path == "Option"
                                && args.first().map_or(false, |a| matches!(a, IrType::Ref(_) | IrType::MutRef(_))));
                if ret_is_ref_like && matches!(&base.kind, ExprKind::Var(n) if n == "self") {
                    format!("&{}", idx_s)
                } else {
                    idx_s
                }
            }
            _ => self.gen_expr(expr),
        }
    }
}
