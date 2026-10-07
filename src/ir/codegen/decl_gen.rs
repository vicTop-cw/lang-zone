// Lang-Zone 编译器 — ir/codegen/decl_gen.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::scan::rewrite_parallel_block;
use super::types_emit::bigint_from_code;
use super::types_emit::bigint_lit_code;
use super::types_emit::is_bigint_ty;
use super::types_emit::is_complex_ty;
use super::types_emit::BIGINT_RS;
use super::*;

/// 体内是否对该形参名做赋值（`x = ..` / `x += ..`）。
///
/// 用途：默认形参在 Rust 侧是 `Option<T>` 形参 + `let x = x.unwrap_or(..)` 影子，
/// TCO 改写后的重赋落在影子上，需要 `let mut x`（否则 E0384）。只为确有赋值的
/// 形参加 mut，避免给普通函数引入 unused_mut 告警。显式栈遍历，防深体爆栈。
pub(crate) fn block_assigns_param(body: &Block, pname: &str) -> bool {
    fn expr_targets(e: &Expr, pname: &str) -> bool {
        matches!(&e.kind, ExprKind::Var(n) if n == pname)
    }
    let mut stmts: Vec<&[Stmt]> = vec![&body.stmts];
    let mut stack: Vec<&Expr> = Vec::new();
    // 单一工作队列交替处理语句片与表达式：表达式里发现的 BlockExpr 块要能回到
    // 语句阶段（分成两个先后循环会漏掉「表达式 → 块 → 语句」这条路径）。
    loop {
        if let Some(ss) = stmts.pop() {
            for st in ss {
            match st {
                Stmt::Assign { target, .. } if expr_targets(target, pname) => return true,
                Stmt::Let { value, .. } => stack.push(value),
                Stmt::Assign { value, .. } => stack.push(value),
                Stmt::Return { value: Some(v) } => stack.push(v),
                Stmt::ExprStmt { expr } => stack.push(expr),
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    stmts.push(&then_branch.stmts);
                    if let Some(b) = else_branch {
                        stmts.push(&b.stmts);
                    }
                }
                Stmt::Block { stmts: inner } => stmts.push(inner),
                Stmt::For {
                    body, else_body, ..
                } => {
                    stmts.push(&body.stmts);
                    if let Some(b) = else_body {
                        stmts.push(&b.stmts);
                    }
                }
                Stmt::While {
                    body, else_body, ..
                } => {
                    stmts.push(&body.stmts);
                    if let Some(b) = else_body {
                        stmts.push(&b.stmts);
                    }
                }
                Stmt::WhileLet { body, .. } => stmts.push(&body.stmts),
                Stmt::BlockLabel { body, .. } => stmts.push(&body.stmts),
                Stmt::Defer { body } => stmts.push(&body.stmts),
                Stmt::CheckerBlock { body, .. } => stmts.push(&body.stmts),
                Stmt::Match { arms, .. } => {
                    for a in arms {
                        stmts.push(&a.body.stmts);
                    }
                }
                Stmt::TryCatch {
                    body,
                    catches,
                    else_body,
                    finally_body,
                } => {
                    stmts.push(&body.stmts);
                    for (_, b) in catches {
                        stmts.push(&b.stmts);
                    }
                    if let Some(b) = else_body {
                        stmts.push(&b.stmts);
                    }
                    if let Some(b) = finally_body {
                        stmts.push(&b.stmts);
                    }
                }
                Stmt::Raise { value } => stack.push(value),
                Stmt::Yield { value } => stack.push(value),
                _ => {}
            }
            }
            continue;
        }
        let Some(e) = stack.pop() else { break };
        // walrus / 表达式赋值：`x += 1` 编译为 AssignExpr
        if let ExprKind::AssignExpr { target, .. } = &e.kind {
            if expr_targets(target, pname) {
                return true;
            }
        }
        if let ExprKind::BlockExpr { block } = &e.kind {
            stmts.push(&block.stmts);
        }
        match &e.kind {
            ExprKind::Call { callee, args, .. } => {
                stack.push(callee);
                stack.extend(args.iter());
            }
            ExprKind::MethodCall {
                receiver, args, ..
            } => {
                stack.push(receiver);
                stack.extend(args.iter());
            }
            ExprKind::BinOp { lhs, rhs, .. } => {
                stack.push(lhs);
                stack.push(rhs);
            }
            ExprKind::UnOp { operand, .. } => stack.push(operand),
            ExprKind::IfExpr { cond, then, els } => {
                stack.push(cond);
                stack.push(then);
                stack.push(els);
            }
            ExprKind::TupleLit(es)
            | ExprKind::Tuple(es)
            | ExprKind::ListLit(es)
            | ExprKind::List(es) => stack.extend(es.iter()),
            ExprKind::Cast { expr, .. }
            | ExprKind::Spread(expr)
            | ExprKind::Paren(expr)
            | ExprKind::ImplicitConvert { source: expr, .. } => stack.push(expr),
            ExprKind::Dict(pairs) => stack.extend(pairs.iter().flat_map(|(k, v)| vec![k, v])),
            ExprKind::StructCtor { fields, .. } => stack.extend(fields.iter().map(|(_, e)| e)),
            ExprKind::EnumCtor { args, .. } | ExprKind::MagicCall { args, .. } => {
                stack.extend(args.iter())
            }
            ExprKind::FieldAccess { base, .. } => stack.push(base),
            ExprKind::IndexGet { base, key } | ExprKind::IndexSet { base, key, .. } => {
                stack.push(base);
                stack.push(key);
            }
            ExprKind::Pipe {
                receiver,
                callee,
                args,
            } => {
                stack.push(receiver);
                stack.push(callee);
                stack.extend(args.iter());
            }
            _ => {}
        }
    }
    false
}

impl CodeGen {
    pub(crate) fn gen_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => {
                // 检测方法定义语法 `fn X.method()` → 生成 impl X { fn method() }
                if let Some((ty_name, _method_name)) = f.name.split_once('.') {
                    // 收集所有同类型的方法定义（因 gen_item 逐个调用，此处按需即时生成 impl）
                    self.emit_line(&format!("impl {} {{", ty_name));
                    self.indent += 1;
                    // 临时替换函数名为纯方法名
                    let mut mf = f.clone();
                    mf.name = f.name.split('.').last().unwrap_or(&f.name).to_string();
                    // 方法在 impl 块内不需要 pub
                    self.gen_fn_def(&mf);
                    self.indent -= 1;
                    self.emit_line("}");
                    self.buf.push('\n');
                } else {
                    self.gen_fn_def(f);
                }
            }
            Item::StructDef(s) => self.gen_struct_def(s),
            Item::EnumDef(e) => self.gen_enum_def(e),
            Item::TraitDef(t) => self.gen_trait_def(t),
            Item::Impl(i) => self.gen_impl_def(i),
            Item::Use(u) => self.gen_use_stmt(u),
            Item::Const(c) => self.gen_const_def(c),
            Item::TypeAlias(_) => { /* 已提前生成，跳过 */ }
            Item::Test(t) => self.gen_test_def(t),
            Item::CheckerBlock {
                name,
                ps_name: _,
                default_checker,
                body,
                captured,
            } => {
                // checker 块 → fn NAME(ps: &mut __Params)
                // 捕获的外层局部变量（block 闭包语义，规范 05b-block命名块.md §三）：
                // 追加 &mut 参数（out: &mut Vec<i64> 等），调用点传 &mut out
                let captured_params: Vec<String> = captured
                    .iter()
                    .map(|(n, t)| format!("{}: &mut {}", n, self.rust_type(t)))
                    .collect();
                let sig = if captured_params.is_empty() {
                    format!("fn {name}(ps: &mut __Params) {{")
                } else {
                    format!(
                        "fn {name}(ps: &mut __Params, {}) {{",
                        captured_params.join(", ")
                    )
                };
                self.emit_line(&sig);
                self.indent += 1;
                // 登记当前 checker fn 的捕获参数名：递归调用（break NAME with /
                // block NAME[(...)]）时捕获变量已是 &mut 参数，直接传名而非 &mut 名；
                // 同时加入 ref_mut_bindings：捕获变量是 &mut 引用，`depth = depth + 1`
                // 需生成 `*depth = *depth + 1`（E0369 修复）
                let saved_checker_captures = self.current_checker_captures.clone();
                let saved_ref_mut = self.ref_mut_bindings.clone();
                for (n, _) in captured {
                    self.current_checker_captures.insert(n.clone());
                    self.ref_mut_bindings.insert(n.clone());
                }
                if let Some(ref chk_name) = default_checker {
                    // 区分两类 default_checker：
                    //  - checker 块（fn NAME(ps: &mut __Params)）→ NAME(ps);
                    //  - 普通函数 `__Params -> __Params`（如 def double_ps(ps: __Params)）→
                    //    值变换：*ps = NAME(ps.clone());（否则 E0308 类型不匹配）
                    let is_checker_block = self.checker_blocks.contains(chk_name.as_str());
                    if !is_checker_block {
                        // 值变换函数（__Params -> __Params）：取出当前 ps 值传入，写回结果。
                        // 用 mem::replace（__Params 含 Box<dyn Any> 不可 Clone，且 new() 提供空值）
                        self.emit_line(&format!(
                            "*ps = {chk_name}(std::mem::replace(ps, __Params::new()));"
                        ));
                    } else {
                        // default_checker 若也有捕获，同参数传递
                        let extra = self.checker_extra_args(chk_name);
                        if extra.is_empty() {
                            self.emit_line(&format!("{chk_name}(ps);"));
                        } else {
                            self.emit_line(&format!("{chk_name}(ps, {});", extra.join(", ")));
                        }
                    }
                }
                self.gen_block_inner(body);
                self.current_checker_captures = saved_checker_captures;
                self.ref_mut_bindings = saved_ref_mut;
                self.indent -= 1;
                self.emit_line("}");
            }
            Item::DuckDef(d) => self.gen_duck_def(d),
            Item::EmbedBlock { lang, src, .. } => {
                if lang == "tnr" {
                    #[cfg(feature = "tnr-embed")]
                    {
                        self.gen_embed_tnr(src);
                    }
                    #[cfg(not(feature = "tnr-embed"))]
                    {
                        self.emit_line("// #[embed(tnr)] — tnr-embed feature 未启用");
                        for line in src.lines() {
                            self.emit_line(&format!("// {}", line));
                        }
                    }
                } else {
                    self.emit_line(&format!("// #[embed({})]", lang));
                    for line in src.lines() {
                        self.emit_line(line);
                    }
                }
            }
        }
    }

    #[cfg(feature = "tnr-embed")]
    pub(crate) fn gen_embed_tnr(&mut self, src: &str) {
        self.emit_line("// #[embed(tnr)] → tnr lib 转译 → Rust 模块");
        match tnr::frontend::parser::parse(src) {
            Ok(prog) => match tnr::ir::lower_program_to_lir_ast(&prog, tnr::ir::Backend::Rust) {
                Ok(lir) => match tnr::codegen::lirgen::transpile_lir(&lir) {
                    Ok(rust_src) => {
                        for line in rust_src.lines() {
                            self.emit_line(line);
                        }
                    }
                    Err(e) => {
                        self.emit_line(&format!("// tnr transpile error: {:?}", e));
                    }
                },
                Err(e) => {
                    self.emit_line(&format!("// tnr lower error: {:?}", e));
                }
            },
            Err(e) => {
                self.emit_line(&format!("// tnr parse error: {:?}", e));
            }
        }
    }

    /// 查询 checker 块捕获变量在调用点的实参列表（block 闭包语义，规范 05b-block命名块.md §三）。
    /// - 模块级/函数级调用：捕获变量是局部变量 → 传 `&mut out`
    /// - checker fn 体内递归调用：捕获变量已是 fn 的 &mut 参数 → 直接传 `out`
    pub(crate) fn checker_extra_args(&self, name: &str) -> Vec<String> {
        self.checker_captures
            .get(name)
            .map(|caps| {
                caps.iter()
                    .map(|(n, _)| {
                        if self.current_checker_captures.contains(n) {
                            n.clone()
                        } else {
                            format!("&mut {}", n)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn gen_fn_def(&mut self, f: &FnDef) {
        // 未具体化的 type-pack 函数（如 `def show<Ts...>(..: Tuple<Ts...>)` 但
        // 整个程序没有任何调用点，无法从调用点推断 Ts... 的具体异质元组类型）：
        // 跳过生成。若硬生成泛型切片签名 + 元组字段访问（args.0）会触发
        // E0609/E0308；而该函数未被调用，不生成是安全且语义正确的。
        if self.typepack_param.contains_key(&f.name)
            && self
                .typepack_sigs
                .get(&f.name)
                .map_or(true, |sigs| sigs.is_empty())
        {
            return;
        }
        self.declared.clear();
        // 每函数重新收集字符串类型局部变量（f-string 插值 {} 用）
        self.str_typed_vars.clear();
        // 每函数清空延迟惰性绑定表（`@lazy`），避免跨函数泄漏（T04/AC4）
        self.lazy_bindings.clear();
        // 预扫描函数体，收集需自动加 mut 的局部 let 变量名
        // （LZ `let v = vec; v.push(1)` 未写 mut，但 Rust 需可变绑定，E0596）
        self.auto_mut_locals.clear();
        self.deferred.clear(); // 每函数清空块级 defer 收集（gen_block_inner 内 mem::take 接管）
        scan_auto_mut_locals(&f.body, &mut self.auto_mut_locals);
        // 记录当前函数名（嵌套 gen_fn_def 时保存/恢复，确保 move 修复按正确
        // 函数作用域查 fn_use_count）。use_count 由 gen_module 预计算。
        let saved_name = std::mem::take(&mut self.cur_fn_name);
        self.cur_fn_name = f.name.clone();
        if std::env::var("LZ_DBG_FN").is_ok() {
            eprintln!(
                "DBG fn: name={} in_ext={} cur_ext={:?} params={:?}",
                f.name,
                self.in_ext_trait,
                self.current_ext_trait,
                f.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
            );
        }
        // 记录当前函数是否为 async（用于 __go 的异步/同步分派）
        self.current_fn_is_async = f.is_async || (f.name == "main" && block_has_await(&f.body));
        // @parallel：标记当前函数为并行模式（map 方法调用生成并行版本）
        self.cur_fn_is_parallel = f
            .intrinsics
            .iter()
            .any(|i| matches!(i.kind, IntrinsicKind::Parallel));
        // @parallel：对函数体做 AST 级变换（xs.map(f) → lz_builtins::__lz_par_map(xs, f)）
        let eff_body: Block = if self.cur_fn_is_parallel {
            let mut b = f.body.clone();
            rewrite_parallel_block(&mut b);
            b
        } else {
            f.body.clone()
        };
        // 记录当前是否在生成 impl Iterator 的 size_hint 方法体（返回元组需 usize）
        self.current_fn_is_size_hint =
            self.in_iterator_impl && (f.name == "size_hint" || f.name == "__size_hint__");
        // 记录当前函数是否返回引用（`-> &Self` / `-> ref T`）：builder 对 ref 返回
        // 推断可能为 None，Stmt::Return 中 `return self` 需据此判断是否 clone
        // （在 sig 生成后按 ` -> &` 前缀设置，见下方 ret 计算处）
        self.current_fn_ret_is_ref = false;
        // 记录 self 是否以共享引用接收（&self），用于对 self.字段 值表达式自动 .clone()
        self.borrow_self = f
            .params
            .iter()
            .find(|p| p.name == "self")
            .map_or(false, |p| !p.is_mut && !p.is_owned && !is_consuming_self(f));
        // 收集当前函数的 variadic 参数名
        self.current_variadic_params.clear();
        for p in &f.params {
            if p.variadic {
                self.current_variadic_params.insert(p.name.clone());
            }
        }
        // 检测参数名与模块级名称冲突 → 重命名参数（E0530）
        self.param_renames.clear();
        for p in &f.params {
            if p.name != "self" && self.top_level_static_names.contains(&p.name) {
                self.param_renames
                    .insert(p.name.clone(), format!("{}_", p.name));
            }
        }
        // 登记字符串类型参数：f-string 插值用 {}（Debug 会给 String 加引号）
        for p in &f.params {
            let is_str = matches!(&p.ty, IrType::Str)
                || matches!(&p.ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str));
            if is_str {
                let pname = self
                    .param_renames
                    .get(&p.name)
                    .cloned()
                    .unwrap_or_else(|| p.name.clone());
                self.str_typed_vars.insert(pname);
            }
        }
        // 收集泛型参数上的 duck 字段约束（a.field → a.__field_field() trait accessor）
        // 注意：duck_field_members 的 key 用「实际参数名」（如 a），
        // 因为函数体内字段访问的 base 是参数名，不是泛型参数名（A）
        // 字段归属：duck 字段约束 owner 前缀（如 A）对应「该 duck 泛型参数在 bound
        // 实参中的位置」；若本参数对应的函数泛型出现在该位置，则字段属于本参数。
        self.duck_field_members.clear();
        for p in &f.params {
            // 两种情况：
            // 1. 参数类型是泛型参数（T）且其 bound 是 duck → 收集该 duck 字段
            // 2. 参数类型直接是 duck 名（pet: Pet）→ 收集 duck 定义的全部字段
            if let IrType::Named { path, .. } = &p.ty {
                if let Some(d) = self.duck_defs.get(path) {
                    let field_names: std::collections::HashSet<String> = d
                        .fields
                        .iter()
                        .filter(|df| df.owner.is_none())
                        .map(|df| df.name.clone())
                        .collect();
                    if !field_names.is_empty() {
                        self.duck_field_members.insert(p.name.clone(), field_names);
                    }
                    continue;
                }
            }
            let IrType::Generic(gname) = &p.ty else {
                continue;
            };
            let mut field_names: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            // 找到该泛型参数对应的 duck 约束
            if let Some(g) = f.generics.iter().find(|g| &g.name == gname) {
                for b in &g.bounds {
                    if let IrType::Named { path, args } = b {
                        if let Some(d) = self.duck_defs.get(path) {
                            // 本函数泛型 gname 在 bound 实参中的位置 → duck 泛型参数名
                            let duck_owner_for_self: Option<String> = args
                                .iter()
                                .position(|ba| {
                                    matches!(ba, IrType::Generic(n) if n == gname)
                                        || matches!(ba, IrType::Named { path, .. } if path == gname)
                                })
                                .and_then(|i| d.generics.get(i))
                                .map(|dg| dg.name.clone());
                            for df in &d.fields {
                                // 字段属于本泛型：无 owner 前缀，或
                                // owner == 本泛型在该 bound 中对应的 duck 泛型参数
                                let belongs = match &df.owner {
                                    None => true,
                                    Some(o) => {
                                        duck_owner_for_self.as_ref().map_or(false, |d| d == o)
                                    }
                                };
                                if belongs {
                                    field_names.insert(df.name.clone());
                                }
                            }
                        }
                    }
                }
            }
            if !field_names.is_empty() {
                self.duck_field_members.insert(p.name.clone(), field_names);
            }
        }
        // 检测 duck 参数 → 自动注入泛型类型
        // duck 类型在 IR 中为 Named(path)，需同时匹配 duck_defs 登记的名字
        let is_duck_ty = |ty: &IrType| -> bool {
            match ty {
                IrType::Duck { .. } => true,
                IrType::Named { path, .. } => self.duck_defs.contains_key(path.as_str()),
                _ => false,
            }
        };
        let duck_name_of = |ty: &IrType| -> Option<String> {
            match ty {
                IrType::Named { path, .. } if self.duck_defs.contains_key(path.as_str()) => {
                    Some(path.clone())
                }
                _ => None,
            }
        };
        // 同 duck 约束的多个参数统一为单一泛型（min(a: C, b: C) -> C）：
        // 各生成独立泛型参数会 E0277（`a < b.clone()` 要求 T0: PartialOrd<T1>），
        // 统一后 a/b/返回值同类型，自引用方法 __lt__(Self) 直接可用。
        // 同 duck 的泛型按首次出现顺序命名 DuckParam0/1/…
        let mut duck_groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, p) in f.params.iter().enumerate() {
            if let Some(name) = duck_name_of(&p.ty) {
                match duck_groups.iter_mut().find(|(n, _)| n == &name) {
                    Some((_, idxs)) => idxs.push(i),
                    None => duck_groups.push((name, vec![i])),
                }
            }
        }
        let mut duck_param_names: HashMap<usize, String> = HashMap::new();
        for (g, (_, idxs)) in duck_groups.iter().enumerate() {
            let pname = format!("DuckParam{}", g);
            for &i in idxs {
                duck_param_names.insert(i, pname.clone());
            }
        }
        // duck 参数 → 泛型参数名（同 duck 的多参数共享一个泛型，去重：
        // min(a: C, b: C) → [DuckParam0] 而非 [DuckParam0, DuckParam0] E0403）
        let mut duck_params: Vec<String> = f
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| is_duck_ty(&p.ty))
            .map(|(i, _)| duck_param_names[&i].clone())
            .collect();
        duck_params.dedup();
        let duck_indices: Vec<usize> = f
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| is_duck_ty(&p.ty))
            .map(|(i, _)| i)
            .collect();
        // duck 参数 → 泛型参数名 + trait bound（DuckParam0: Pet）。
        // 附加 Clone：非消耗魔术方法（__lt__ 等）参数需 .clone() 避免 move 复用，
        // 泛型参数无 Clone bound 会 E0599（与 impl 泛型 Clone+Debug 约定一致）
        let duck_bounds: Vec<String> = duck_groups
            .iter()
            .map(|(name, idxs)| {
                format!(
                    "{}: {} + std::clone::Clone",
                    duck_param_names[&idxs[0]], name
                )
            })
            .collect();

        let has_ducks = !duck_params.is_empty();
        let is_math = f.intrinsics.iter().any(|intr| matches!(&intr.kind, IntrinsicKind::Export(targets) if targets.iter().any(|t| t == "Math")));

        // type-pack 具体化（03d §2.8 方案 B）：..: Tuple<Ts...> 函数按调用点
        // 实参类型生成具体 Rust 元组签名 args: (T1, T2, T3)，同时移除 pack 泛型
        // 参数（函数不再是泛型，避免 rustc 对具体元组做 T 类型推断失败）。
        // pack 泛型名在 FnDef.generics 中，故需过滤后再生成泛型声明。
        let typepack_concrete: Option<Vec<IrType>> = self
            .typepack_sigs
            .get(&f.name)
            .and_then(|sigs| sigs.first().cloned());
        let is_typepack_concretized =
            self.typepack_param.contains_key(&f.name) && typepack_concrete.is_some();
        let effective_generics: Vec<GenericParam> = {
            let mut gs: Vec<GenericParam> = if is_typepack_concretized {
                let pack = self.typepack_param.get(&f.name).unwrap();
                f.generics
                    .iter()
                    .filter(|g| g.name != *pack)
                    .cloned()
                    .collect()
            } else {
                f.generics.clone()
            };
            // impl<T>/struct<T> 块内的方法：builder 已把外层泛型合并进方法的
            // generics，若再声明一次会生成 `impl<T> X<T> { fn m<T>(..) }` →
            // E0403（T 重复）。剔除与当前块同名的泛型，方法自身泛型（如 <U>）保留。
            if !self.current_impl_generics.is_empty() {
                gs.retain(|g| !self.current_impl_generics.contains(&g.name));
            }
            gs
        };

        let (cmp_eq, cmp_ord) = self
            .fn_cmp
            .get(&f.name)
            .cloned()
            .unwrap_or_else(|| (HashSet::new(), HashSet::new()));

        let generics = if has_ducks {
            let base = self.gen_fn_generics(&effective_generics, &cmp_eq, &cmp_ord);
            if base.is_empty() {
                format!("<{}>", duck_params.join(", "))
            } else {
                format!(
                    "<{}, {}>",
                    base.trim_matches(|c| c == '<' || c == '>'),
                    duck_params.join(", ")
                )
            }
        } else {
            self.gen_fn_generics(&effective_generics, &cmp_eq, &cmp_ord)
        };

        // @math where 子句：每个泛型参数都需要算术 trait bounds
        let math_where = if is_math && !f.generics.is_empty() {
            let clauses: Vec<String> = f
                .generics
                .iter()
                .map(|g| {
                    // From<i32>：泛型函数体内整数字面量经 T::from(2i32) 转换
                    // （f64 未实现 From<i64>（精度损失被禁），From<i32> 两者都有；
                    // 否则 `x * 2` 中 2 无法推断为 T，E0308）
                    format!(
                        "    {}: std::ops::Add<Output={}> + std::ops::Mul<Output={}> + Copy + std::convert::From<i32>",
                        g.name, g.name, g.name
                    )
                })
                .collect();
            if clauses.is_empty() {
                String::new()
            } else {
                format!("\nwhere\n{}", clauses.join(",\n"))
            }
        } else {
            String::new()
        };
        // duck 参数 trait bound（DuckParam0: Pet）并入 where 子句
        let duck_where = if duck_bounds.is_empty() {
            String::new()
        } else if math_where.is_empty() {
            format!("\nwhere\n{}", duck_bounds.join(",\n"))
        } else {
            format!(",\n{}", duck_bounds.join(",\n"))
        };

        // 字段关系 duck 的 where 投影约束（§2.2 `A.id == B.id`）：
        // 关系字段在 trait 中用关联类型 __Field_x 表达，泛型函数体内比较两侧字段时，
        // 需要 `<A as Duck<...>>::__Field_x: PartialEq<<B as Duck<...>>::__Field_x>` 约束
        let mut rel_clauses: Vec<String> = Vec::new();
        for g in &f.generics {
            for b in &g.bounds {
                let IrType::Named { path, args } = b else {
                    continue;
                };
                let Some(d) = self.duck_defs.get(path) else {
                    continue;
                };
                for df in &d.fields {
                    let Some((rel_owner, rel_name)) = &df.rel else {
                        continue;
                    };
                    let owner_matches = match &df.owner {
                        None => true,
                        Some(o) => o == &g.name,
                    };
                    if !owner_matches {
                        continue;
                    }
                    let args_str = args
                        .iter()
                        .map(|a| self.rust_type(a))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let left = format!(
                        "<{} as {}<{}>>::__Field_{}",
                        g.name, path, args_str, df.name
                    );
                    // 右侧：找函数泛型 rel_owner 的同名 duck bound（如 B: LinkedFields<B, A>）
                    let right = f
                        .generics
                        .iter()
                        .find(|g2| &g2.name == rel_owner)
                        .and_then(|g2| {
                            g2.bounds.iter().find_map(|b2| {
                                if let IrType::Named {
                                    path: p2,
                                    args: args2,
                                } = b2
                                {
                                    if p2 == path {
                                        let s2 = args2
                                            .iter()
                                            .map(|a| self.rust_type(a))
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        return Some(format!(
                                            "<{} as {}<{}>>::__Field_{}",
                                            rel_owner, path, s2, rel_name
                                        ));
                                    }
                                }
                                None
                            })
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "<{} as {}<{}>>::__Field_{}",
                                rel_owner, path, args_str, rel_name
                            )
                        });
                    rel_clauses.push(format!("    {}: PartialEq<{}>", left, right));
                }
                // 关联类型 Debug 约束（§2.3）：泛型函数体内 print/format 关联类型值
                // 需要 `<T as HasItem<T>>::Item: std::fmt::Debug`
                for a in &d.assoc_types {
                    let belongs = match &a.owner {
                        None => true,
                        Some(o) => {
                            let oi = d.generics.iter().position(|g2| &g2.name == o);
                            match oi {
                                Some(i) => args.get(i).map_or(false, |ba| {
                                    matches!(ba, IrType::Generic(n) if n == &g.name)
                                        || matches!(ba, IrType::Named { path, .. } if path == &g.name)
                                }),
                                None => false,
                            }
                        }
                    };
                    if !belongs {
                        continue;
                    }
                    let args_str = args
                        .iter()
                        .map(|a| self.rust_type(a))
                        .collect::<Vec<_>>()
                        .join(", ");
                    rel_clauses.push(format!(
                        "    <{} as {}<{}>>::{}: std::fmt::Debug",
                        g.name, path, args_str, a.name
                    ));
                }
            }
        }
        let rel_where = if rel_clauses.is_empty() {
            String::new()
        } else if math_where.is_empty() {
            format!("\nwhere\n{}", rel_clauses.join(",\n"))
        } else {
            // math_where 已是 \nwhere\nclauses 形式，关系约束追加为额外子句
            format!("{},\n{}", math_where.trim_end(), rel_clauses.join(",\n"))
        };
        // 额外 where 约束（引用 impl 级泛型的 where 子句，如 `impl<K,V> Dict<K,V>`
        // 方法 `where K: Eq + Hash`——K 不在方法泛型中，builder 保留到 FnDef.where_clause）
        let extra_where = if f.where_clause.is_empty() {
            String::new()
        } else {
            let clauses: Vec<String> = f
                .where_clause
                .iter()
                .map(|(tp, bounds)| {
                    let bs: Vec<String> = bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                    // 关联类型路径 `I.Item` → `I::Item`（where 子句中 Rust 用 ::）
                    let tp_rust = tp.replace('.', "::");
                    format!("    {}: {}", tp_rust, bs.join(" + "))
                })
                .collect();
            if rel_where.is_empty() && math_where.is_empty() && duck_where.is_empty() {
                format!("\nwhere\n{}", clauses.join(",\n"))
            } else {
                format!(",\n{}", clauses.join(",\n"))
            }
        };

        let params: Vec<String> = f
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let pname = self
                    .param_renames
                    .get(&p.name)
                    .cloned()
                    .unwrap_or_else(|| p.name.clone());
                if duck_indices.contains(&i) {
                    // 参数 → 泛型名直接查 duck_param_names（duck_params 已去重，
                    // 不能用位置索引；min(a: C, b: C) 两参数都映射 DuckParam0）
                    format!("{}: {}", pname, duck_param_names[&i])
                } else if p.name == "self" {
                    // self 参数修饰：ref self → &self；mut self → &mut self；owned self → self
                    // 消耗型魔术方法（__enter__/__iter__）以 owned self 接收以便 move 字段
                    // 算术运算符保留 &self 以避免调用方多次复用实例时发生 move
                    let consumes_self = f.name == "__enter__" || f.name == "__iter__";
                    // impl Iterator 中 __next__/next 必须为 &mut self（std Iterator trait
                    // 要求，否则 E0053 types differ in mutability）
                    if self.in_iterator_impl && (f.name == "next" || f.name == "__next__") {
                        "&mut self".into()
                    } else if p.is_mut {
                        "&mut self".into()
                    } else if p.is_owned || (p.is_ref == false && consumes_self && !is_math) {
                        "self".into()
                    } else {
                        "&self".into()
                    }
                } else {
                    let ty_str = if p.variadic {
                        if p.name == "kwargs" {
                            // kwargs 注入: &HashMap<String, V>（值类型 = p.ty）
                            format!("&HashMap<String, {}>", self.rust_type(&p.ty))
                        } else if let IrType::Tuple(items) = &p.ty {
                            // type-pack（..: Tuple<Ts...>）：args 参数类型为
                            // Tuple([Generic("Ts")])，按调用点具体化为
                            // `args: (T1, T2, T3)`（异质元组，03d §2.8 方案 B）
                            if items.len() == 1 && matches!(&items[0], IrType::Generic(_)) {
                                if let Some(sig) = self.typepack_sigs.get(&f.name) {
                                    if let Some(concrete) = sig.first() {
                                        let parts: Vec<String> =
                                            concrete.iter().map(|t| self.rust_type(t)).collect();
                                        format!("({})", parts.join(", "))
                                    } else {
                                        format!("&[{}]", self.rust_type(&items[0]))
                                    }
                                } else {
                                    // 无调用点（未具体化）：回退到同构切片（旧行为）
                                    format!("&[{}]", self.rust_type(&items[0]))
                                }
                            } else {
                                // 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` →
                                // args: (T1, T2, Vec<Box<dyn Any>>)（前 N 位置精确类型，
                                // 尾部 `..` 通配收集为 Box<dyn Any>）
                                let prefix: Vec<String> =
                                    items.iter().map(|t| self.rust_type(t)).collect();
                                format!("({}, Vec<Box<dyn Any>>)", prefix.join(", "))
                            }
                        } else {
                            format!("&[{}]", self.rust_type(&p.ty))
                        }
                    } else if p.default.is_some() {
                        format!("Option<{}>", self.rust_type(&p.ty))
                    } else if p.is_ref {
                        // ref x: T → &T；mut ref x: T → &mut T
                        // 特殊处理 str 类型：生成 &str 而非 &String
                        if matches!(&p.ty, IrType::Str) {
                            "&str".into()
                        } else if p.is_mut {
                            format!("&mut {}", self.rust_type(&p.ty))
                        } else {
                            format!("&{}", self.rust_type(&p.ty))
                        }
                    } else {
                        // fn(...) 类型参数 → impl Fn(...)：可接受闭包（03e §五），
                        // 直接生成 fn 指针无法接收 move 闭包（E0308）。
                        // 用 impl Fn（非 FnMut）：闭包体内调用此类参数得到的是 Fn 闭包，
                        // 才能作为 fn 值载体返回（compose 返回 |x| g(f(x))，E0596）。
                        // for_each 类「接收 FnMut 用户闭包」需求由 lib_iterator 负责
                        // （该库当前 ignored，待转正时再按需细化 FnMut 形参）。
                        if let IrType::Fn { params, ret } = &p.ty {
                            let ps: Vec<String> =
                                params.iter().map(|pt| self.rust_type(pt)).collect();
                            // struct 构造方法（new）的 fn 形参：实参已在调用处转成
                            // fn 指针（`(lambda) as fn(..)`），声明需同步用 fn 指针——
                            // impl FnMut opaque 无 Clone，存入 fn 字段报 E0599
                            // （lib_iterator MapIter::new/FilterIter::new）
                            if f.name == "new" {
                                format!("fn({}) -> {}", ps.join(", "), self.rust_type(ret))
                            } else {
                                // BUG-9 的连带收紧：本函数**返回 fn 值**时，体内会把 fn
                                // 形参捕获进 `Arc<dyn Fn … + Send + Sync>` 载体，形参自身
                                // 必须 Send + Sync，否则 compose 一类「组合并返回闭包」
                                // 报 E0277（rustc 原文：consider restricting opaque type
                                // `impl Fn(b) -> c + 'static` with trait `Send`）。
                                // 只在这一种函数收紧；map/filter/for_each 等返回非 fn 值
                                // 的 HOF 形参保持原样，既有的借用捕获语义不受影响。
                                let send_bound = if matches!(&f.ret_ty, IrType::Fn { .. }) {
                                    " + Send + Sync"
                                } else {
                                    ""
                                };
                                format!(
                                    "impl Fn({}) -> {} + 'static{}",
                                    ps.join(", "),
                                    self.rust_type(ret),
                                    send_bound
                                )
                            }
                    } else {
                        let ty_str = self.rust_type(&p.ty).to_string();
                        let ty_str = if let IrType::Named { path, args } = &p.ty {
                            if path == "Iterator" && !args.is_empty() {
                                format!("impl Iterator<Item = {}>", self.rust_type(&args[0]))
                            } else {
                                ty_str
                            }
                        } else {
                            ty_str
                        };

                        // 只有显式 is_ref 的 Str 参数才生成 &str；
                        // 普通 String 参数保持 String，避免调用点传 String 实参时 E0308
                        if p.is_ref && matches!(&p.ty, IrType::Str) {
                            "&str".into()
                        } else {
                            ty_str
                        }
                    }
                    };
                    // __Params 值参数（checker 链值函数，如 def double_ps(ps: __Params)）
                    // 体内会写 ps.args，必须生成 `mut ps: __Params`（否则 E0596）
                    let ty_is_params =
                        matches!(&p.ty, IrType::Named { path, .. } if path == "__Params");
                    // Iterator 参数：.next() 需要 &mut self，必须生成 `mut it: impl Iterator<..>`
                    // 注意：形参可能是 `&Iterator`（Ref(Named)），需同时匹配裸 Named 和 Ref 包裹
                    let ty_is_iterator = matches!(&p.ty, IrType::Named { path, .. } if path == "Iterator")
                        || matches!(&p.ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Named { path, .. } if path == "Iterator"));
                    // fn 类型参数生成 impl Fn：调用无需可变借用，不再强制 `mut f`
                    if p.is_mut || ty_is_params || ty_is_iterator {
                        format!("mut {}: {}", pname, ty_str)
                    } else {
                        format!("{}: {}", pname, ty_str)
                    }
                }
            })
            .collect();
        let has_yield = block_has_yield(&f.body);
        // 生成器函数内 return 等价 raise（iterator 体内 return 终止并抛出）
        let saved_generator = self.in_generator;
        self.in_generator = has_yield;
        // 泛型函数（@math 等）体内整数字面量不附加 i64 后缀（E0308 修复）；
        // impl<T> 泛型块方法自身无 generics，也按泛型上下文处理（in_impl_generic）
        let saved_generic_fn = self.in_generic_fn;
        // type-pack 具体化后函数无泛型（Ts 已被具体类型替换），字面量按
        // 具体上下文生成（i64 后缀），与调用点类型一致（否则 E0308 i32 vs i64）
        self.in_generic_fn =
            (!f.generics.is_empty() && !is_typepack_concretized) || self.in_impl_generic;
        let saved_math_fn = self.in_math_fn;
        self.in_math_fn = is_math;
        // Rust 不允许 async main，对于 async main 使用 block_on 包装
        let is_async_main = f.is_async && f.name == "main";
        // LZ 允许 def main() -> int：Rust main 只能返回 ()，需生成内部函数
        // __lz_main() -> i64 + pub fn main() { std::process::exit(__lz_main() as i32); }
        let is_typed_main = f.name == "main" && !is_async_main && f.ret_ty != IrType::Unit;
        let ret = if is_typed_main {
            format!(" -> {}", self.rust_type(&f.ret_ty))
        } else if f.name == "main" && !is_async_main {
            String::new() // Rust main always returns ()
        } else if is_async_main {
            String::new() // async main 也返回 ()（block_on 内部处理）
        } else if has_yield {
            // 生成器返回类型：-> Y 表示每次 yield 的值为 Y（规范 14-生成器 §五/§八）。
            // - `-> int`          → Vec<i64>
            // - `-> Iter<R>`      → Vec<Iter<R>>（嵌套迭代器，Iter 映射为 Vec）
            // - `-> Iterator<T>`  → Vec<T>（trait 无法装 Vec，解包内部类型）
            let elem = match &f.ret_ty {
                IrType::Named { path, .. } if path == "Iter" => f.ret_ty.clone(),
                IrType::Named { path, args } if path == "Iterator" => {
                    args.first().cloned().unwrap_or(IrType::Int)
                }
                other => other.clone(),
            };
            format!(" -> Vec<{}>", self.rust_type(&elem))
        } else if f.ret_ty != IrType::Unit {
            // `impl Iterator` 内 `size_hint` 的返回类型：std Iterator 要求
            // `(usize, Option<usize>)`，而 LZ 写 `(int, Option<int>)`（i64）——
            // 生成时转为 `(usize, Option<usize>)`（否则 E0053 类型不兼容）
            if self.in_iterator_impl && (f.name == "size_hint" || f.name == "__size_hint__") {
                format!(" -> (usize, Option<usize>)")
            } else if self.in_iterator_impl && (f.name == "next" || f.name == "__next__") {
                // `impl Iterator` 的 next：必须返回 `std::option::Option<Item>`。
                // 自定义 `enum Option<T>`（lz_std/option.lz）与 std Option 同名，
                // 裸 `Option<T>` 会解析到自定义枚举（E0053 类型不兼容）
                let item = match &f.ret_ty {
                    IrType::Named { path, args }
                        if path == "Option" || path == "std::option::Option" =>
                    {
                        args.first().cloned().unwrap_or(IrType::Any)
                    }
                    IrType::Option(inner) => (**inner).clone(),
                    other => other.clone(),
                };
                format!(" -> std::option::Option<{}>", self.rust_type(&item))
            } else {
                let ret_ty_str = match &f.ret_ty {
                    IrType::Fn { .. } => {
                        // LZ `fn` 值返回：统一走 fn_value_type 的 Arc<dyn Fn … + Send + Sync>
                        // （IR-003：支持捕获闭包作返回，载体可 clone —— LZ fn 值是值语义；
                        //   BUG-9：带 + Send + Sync 才能被 `go` 捕获进 thread::spawn）。
                        // 单层 fn(int)->int → Arc<dyn Fn(i64) -> i64 + Send + Sync>
                        // （原 impl Fn(P) -> i64 为误生成，E0308）。
                        self.fn_value_type(&f.ret_ty)
                    }
                    // 返回类型是 duck 约束名（min -> Comparable）：Rust 无自由 trait
                    // 类型，渲染为统一后的 duck 泛型参数名（E0277 修复）
                    IrType::Named { path, .. } if self.duck_defs.contains_key(path.as_str()) => f
                        .params
                        .iter()
                        .enumerate()
                        .find(
                            |(_, p)| matches!(&p.ty, IrType::Named { path: pn, .. } if pn == path),
                        )
                        .and_then(|(i, _)| duck_param_names.get(&i).cloned())
                        .unwrap_or_else(|| self.rust_type(&f.ret_ty)),
                    _ => self.rust_type(&f.ret_ty),
                };
                format!(" -> {}", ret_ty_str)
            }
        } else {
            String::new()
        };
        let async_kw = if f.is_async && !is_async_main {
            "async "
        } else {
            ""
        };
        // 记录当前函数是否返回引用（`-> &Self` / `-> ref T`）：builder 对 ref 返回
        // 推断可能为 None，Stmt::Return 中 `return self` 需据此判断是否 clone。
        // 基于生成签名 ` -> &` 前缀判断（rust_type 对 Ref(Self_) 输出 &Self）
        self.current_fn_ret_is_ref = ret.trim_start().starts_with("-> &");
        // 记录当前函数是否返回引用（`-> &Self` / `-> ref T`）：builder 对 ref 返回
        // 推断可能为 None，Stmt::Return 中 `return self` 需据此判断是否 clone。
        // 基于生成签名 ` -> &` 前缀判断（rust_type 对 Ref(Self_) 输出 &Self）
        self.current_fn_ret_is_ref = ret.trim_start().starts_with("-> &");
        let is_method = f.params.first().map_or(false, |p| p.name == "self");
        let vis = if is_method { "" } else { "pub " };

        let fn_name = if is_typed_main {
            "__lz_main".to_string()
        } else {
            let raw = f.name.clone();
            // LZ 迭代协议（规范 06d §五）：`impl Iterator for X` 中 `__next__` 魔术
            // 方法映射为 std::iter::Iterator 的 `next`、`__size_hint__` → `size_hint`
            let mapped = if self.in_iterator_impl {
                match raw.as_str() {
                    "__next__" => "next".to_string(),
                    "__size_hint__" => "size_hint".to_string(),
                    _ => raw.clone(),
                }
            } else if self.in_ext_trait {
                // 扩展 trait（ListExt/StrExt 等）方法名与调用点映射一致
                self.ext_trait_method_name(&raw, "")
            } else {
                raw.clone()
            };
            self.mangled_fn_name(
                mapped,
                &f.params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
            )
        };
        // @memoize：函数前置缓存 static（OnceLock 风格：Mutex<Option<Vec<(Key, Ret)>>>）
        let is_memoize = f
            .intrinsics
            .iter()
            .any(|i| matches!(i.kind, IntrinsicKind::Memoize));
        // @curry：柯里化（n 元 → n 层嵌套一元闭包）
        let is_curry = f
            .intrinsics
            .iter()
            .any(|i| matches!(i.kind, IntrinsicKind::Curry));
        let memo_static_name = if is_memoize {
            let sn = format!("__LZ_MEMO_{}", f.name.to_uppercase());
            let key_types: Vec<String> = f.params.iter().map(|p| self.rust_type(&p.ty)).collect();
            let ret_ty_s = self.rust_type(&f.ret_ty);
            self.emit_line(&format!(
                "static {}: std::sync::Mutex<Option<Vec<(({},), {})>>> = std::sync::Mutex::new(None);",
                sn,
                key_types.join(", "),
                ret_ty_s
            ));
            sn
        } else {
            String::new()
        };

        let sig = if is_curry && f.params.len() >= 2 {
            // fn a(A) -> Arc<dyn Fn(B) -> Arc<dyn Fn(C) -> R + Send + Sync> + Send + Sync>
            let mut ret_chain = self.rust_type(&f.ret_ty);
            for p in &f.params[1..] {
                ret_chain = format!(
                    "Arc<dyn Fn({}) -> {} + Send + Sync>",
                    self.rust_type(&p.ty),
                    ret_chain
                );
            }
            format!(
                "{}{}{}fn {}{}({}) -> {}",
                if f.is_test { "#[test]\n" } else { "" },
                vis,
                "",
                fn_name,
                generics,
                params[0],
                ret_chain
            )
        } else {
            format!(
                "{}{}{}fn {}{}({}){}{}{}{}{}",
                if f.is_test { "#[test]\n" } else { "" },
                vis,
                async_kw,
                fn_name,
                generics,
                params.join(", "),
                ret,
                math_where,
                duck_where,
                rel_where,
                extra_where,
            )
        };

        self.emit_line(&format!("{} {{", sig));
        self.indent += 1;

        // @curry：嵌套闭包链生成后直接收尾（跳过 checker/默认参数/extern 等普通流程）
        if is_curry && f.params.len() >= 2 {
            for p in &f.params[1..] {
                let pname = self
                    .param_renames
                    .get(&p.name)
                    .cloned()
                    .unwrap_or_else(|| p.name.clone());
                self.declared.insert(pname);
            }
            let last_idx = f.params.len() - 1;
            for (i, p) in f.params[1..].iter().enumerate() {
                let pname = self
                    .param_renames
                    .get(&p.name)
                    .cloned()
                    .unwrap_or_else(|| p.name.clone());
                let pty = self.rust_type(&p.ty);
                if i == last_idx - 1 {
                    self.emit_line(&format!(
                        "Arc::new(move |{}: {}| -> {} {{",
                        pname,
                        pty,
                        self.rust_type(&f.ret_ty)
                    ));
                } else {
                    self.emit_line(&format!("Arc::new(move |{}: {}| {{", pname, pty));
                }
                self.indent += 1;
            }
            self.gen_block_inner(&eff_body);
            for _ in 1..f.params.len() {
                self.indent -= 1;
                self.emit_line("})");
            }
            self.indent -= 1;
            self.emit_line("}");
            self.cur_fn_name = saved_name;
            return;
        }

        // @memoize：函数体包装——先查缓存（命中直接返回），未命中计算结果并缓存
        if is_memoize {
            let key_names: Vec<String> = f
                .params
                .iter()
                .map(|p| {
                    self.param_renames
                        .get(&p.name)
                        .cloned()
                        .unwrap_or_else(|| p.name.clone())
                })
                .collect();
            self.emit_line(&format!("let __lz_memo_key = ({},);", key_names.join(", ")));
            self.emit_line("{");
            self.indent += 1;
            self.emit_line(&format!(
                "let mut __lz_memo_guard = {}.lock().unwrap();",
                memo_static_name
            ));
            self.emit_line("if __lz_memo_guard.is_none() { *__lz_memo_guard = Some(Vec::new()); }");
            self.emit_line("let __lz_memo = __lz_memo_guard.as_mut().unwrap();");
            self.emit_line(
                "if let Some(__lz_hit) = __lz_memo.iter().find(|(k, _)| *k == __lz_memo_key) {",
            );
            self.indent += 1;
            self.emit_line("return __lz_hit.1.clone();");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
            // 主体包进内部闭包：body 内的 return 只返回闭包值（不跳出缓存逻辑）
            self.emit_line(&format!(
                "let __lz_result = (move || -> {} {{",
                self.rust_type(&f.ret_ty)
            ));
            self.indent += 1;
        }

        // 生成器：body 包含 Yield → prepend __gen_vec
        if has_yield {
            self.emit_line("let mut __gen_vec = Vec::new();");
        }

        // checker 注入：有 default_checker 时打包参数→调checker→拆包
        //
        // 拆包影子 `let n = …` 会遮蔽同名形参；尾递归优化（TCO）改写后循环体内
        // `n = ..` 重赋的正是这个影子绑定，故体内确有对该形参赋值时影子必须 `mut`，
        // 否则 E0384（def_checker.lz 的 fib/fact：checker 局部 n 遮蔽 mut 形参 n，
        // TCO 循环再赋值 → cannot assign twice to immutable variable `n`）。
        // 与下方「默认参数 unwrap 影子」同一判据（block_assigns_param）。
        let body_assigns = |pname: &str| block_assigns_param(&f.body, pname);
        if let Some(ref checker_name) = f.default_checker {
            let user_params: Vec<(String, String)> = f
                .params
                .iter()
                .filter(|p| p.name != "self")
                .map(|p| {
                    let pname = self
                        .param_renames
                        .get(&p.name)
                        .cloned()
                        .unwrap_or_else(|| p.name.clone());
                    (pname, self.rust_type(&p.ty))
                })
                .collect();
            let boxed: Vec<String> = user_params
                .iter()
                .map(|(n, _)| format!("Box::new({})", n))
                .collect();
            self.emit_line(&format!("let mut __ps = __Params {{ args: vec![{}], kwargs: std::collections::HashMap::new() }};", boxed.join(", ")));
            // checker 块（fn NAME(ps: &mut __Params)）→ NAME(&mut __ps);
            // 普通值函数（fn NAME(ps: __Params) -> __Params）→ __ps = NAME(__ps);
            // raises 值函数（fn NAME(ps: __Params) -> __Params raises E）Rust 侧
            // 返回 Result<__Params, E>，需解包：校验失败（Err）即 panic 报错，
            // 成功（Ok）取回 __Params（否则 __ps = Result → E0308）。
            if self.checker_blocks.contains(checker_name) {
                self.emit_line(&format!("{}(&mut __ps);", checker_name));
            } else if self.raises_fn_names.contains(checker_name) {
                self.emit_line(&format!(
                    "__ps = match {}(__ps) {{ Ok(v) => v, Err(e) => panic!(\"checker failed: {{:?}}\", e) }};",
                    checker_name
                ));
            } else {
                self.emit_line(&format!("__ps = {}(__ps);", checker_name));
            }
            for (i, (pname, pty)) in user_params.iter().enumerate() {
                // 形参本体名（f.params[i].name）用于 body_assigns 判定；pname 是
                // 可能的重命名后名字。TCO/体内赋值命中时影子加 mut（见上方注释）。
                let orig_name = f
                    .params
                    .iter()
                    .filter(|p| p.name != "self")
                    .nth(i)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| pname.clone());
                let kw = if body_assigns(&orig_name) { "mut " } else { "" };
                let line = format!("let {}{}: {} = (*__ps.args[{}usize].downcast_ref::<{}>().expect(\"checker arg cast failed\"));", kw, pname, pty, i, pty);
                self.emit_line(&line);
            }
        }

        // 默认参数 unwrap: greet(name: str = "World") → let name = name.unwrap_or_else(|| "World".to_string());
        //
        // 尾递归优化（TCO）改写后，形参会在循环体内被重赋（`acc = ..`），此时 Rust侧
        // 绑定是**这个 unwrap影子**而非 `Option<T>` 形参本身，故影子必须 `mut`，
        // 否则 E0384。仅为「体内确有对该形参赋值」的形参加 mut，避免给普通函数
        // 引入 unused_mut 告警。（body_assigns 闭包已在 checker 注入段之前定义。）
        for p in &f.params {
            if let Some(ref default_val) = p.default {
                let pname = self
                    .param_renames
                    .get(&p.name)
                    .cloned()
                    .unwrap_or_else(|| p.name.clone());
                let def_s = self.gen_expr(default_val);
                let kw = if body_assigns(&p.name) { "mut " } else { "" };
                self.emit_line(&format!(
                    "let {}{} = {}.unwrap_or_else(|| {});",
                    kw, p.name, pname, def_s
                ));
            }
        }

        // 函数体
        self.current_ret_ty = Some(f.ret_ty.clone());
        self.current_fn_ret_ty = Some(f.ret_ty.clone());
        // BUG-CG-004（轮次12）：记录当前函数 raises 异常类型，供 try/catch 结果基分支判断
        self.current_fn_raises = f.raises.clone();
        // f-string 静态判定表：本函数形参的类型（`{r.name}` 要能查到 r 是 Rec）
        let saved_fstr_params = std::mem::take(&mut self.fstr_var_types);
        self.fstr_var_types = f
            .params
            .iter()
            .filter(|prm| prm.name != "self")
            .map(|prm| (prm.name.clone(), prm.ty.clone()))
            .collect();
        // 登记顶层 def 名称（IR-003：顶层函数作值时需 Arc::new(f)，局部 fn-let 已装箱不重包）
        self.top_level_fns.insert(f.name.clone());
        // 嵌套 Fn 返回类型（fn -> fn -> T）：内层闭包返回值需 Arc::new 包装
        let saved_nested_fn_ret = self.nested_fn_ret;
        self.nested_fn_ret = matches!(&f.ret_ty, IrType::Fn { ret, .. }
            if matches!(ret.as_ref(), IrType::Fn { .. }));
        // typed main（def main() -> int）走 __lz_main 内部函数，尾表达式需 return
        self.is_main = f.name == "main" && !is_typed_main;

        // I4：@export(Rust/Python/C) 自动登记（不改变生成产物）
        // 符号在 registry 注入时登记，供 L2 中继路由与 E2E 审计使用。
        if self.bridge_registry.is_some() {
            if let Some(export_targets) = f.intrinsics.iter().find_map(|i| {
                if let IntrinsicKind::Export(t) = &i.kind {
                    Some(t.clone())
                } else {
                    None
                }
            }) {
                let lang = export_targets
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Rust".into());
                let sig_params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, self.rust_type(&p.ty)))
                    .collect();
                let sig = format!(
                    "fn({}) -> {}",
                    sig_params.join(", "),
                    self.rust_type(&f.ret_ty)
                );
                if let Some(reg) = &mut self.bridge_registry {
                    let _ = reg.register_symbol(&f.name, &lang, &sig);
                }
            }
        }

        // #[extern(lang)]：函数体由外部实现提供 → 分发调用（无 extern 关键字）
        let extern_targets: Option<Vec<String>> = f.intrinsics.iter().find_map(|i| {
            if let IntrinsicKind::Extern(targets) = &i.kind {
                Some(targets.clone())
            } else {
                None
            }
        });
        // #[embed(lang)]：内嵌代码段原样插入函数体（G7）
        let embed: Option<(String, String)> = f.intrinsics.iter().find_map(|i| {
            if let IntrinsicKind::Embed { lang, code } = &i.kind {
                Some((lang.clone(), code.clone()))
            } else {
                None
            }
        });
        if let Some((lang, code)) = embed {
            // G7：内嵌代码段——registry 注入时登记符号，函数体原样输出
            // 原生代码（不生成 LZ 语义 body，返回类型/参数由用户保证一致）。
            if self.bridge_registry.is_some() {
                let sig_params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, self.rust_type(&p.ty)))
                    .collect();
                let sig = format!(
                    "fn({}) -> {}",
                    sig_params.join(", "),
                    self.rust_type(&f.ret_ty)
                );
                if let Some(reg) = &mut self.bridge_registry {
                    let _ = reg.register_symbol(&f.name, &lang, &sig);
                }
            }
            for line in code.lines() {
                self.emit_line(line);
            }
        } else if let Some(ext) = extern_targets {
            let lang = ext.first().cloned().unwrap_or_else(|| "Rust".into());
            // I3：extern 自动登记（L2 中继打通）——注入 registry 时，
            // #[extern(lang)] 声明自动 register_symbol + 台账 REGISTER。
            // 签名形如 "fn(a: int, b: int) -> Ext"，供 E_TYPE 参数个数校验。
            // 先计算签名（借用 self），再可变借用 registry，避免 E0502。
            let extern_sig = if self.bridge_registry.is_some() {
                let sig_params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, self.rust_type(&p.ty)))
                    .collect();
                Some(format!("fn({}) -> Ext", sig_params.join(", ")))
            } else {
                None
            };
            if let (Some(reg), Some(sig)) = (&mut self.bridge_registry, extern_sig.as_deref()) {
                let _ = reg.register_symbol(&f.name, &lang, sig);
            }
            let arg_list: Vec<String> = f
                .params
                .iter()
                .map(|p| {
                    let pn = self
                        .param_renames
                        .get(&p.name)
                        .cloned()
                        .unwrap_or_else(|| p.name.clone());
                    format!("{}.to_string()", pn)
                })
                .collect();
            self.emit_line(&format!(
                "let __lz_ext_args: Vec<String> = vec![{}];",
                arg_list.join(", ")
            ));
            self.emit_line(&format!(
                "let __lz_ext_ret = __lz_ext_call(\"{}\", \"{}\", __lz_ext_args);",
                lang, f.name
            ));
            self.emit_line("return __lz_ext_ret;");
        } else if is_async_main {
            // async main → 使用 block_on 包装：fn main() { __block_on(async { body }) }
            // @init：模块初始化函数按声明顺序注入 async 块开头
            if !self.init_fns.is_empty() {
                let init_fns = self.init_fns.clone();
                self.emit_line("let __async_main = async {");
                self.indent += 1;
                for init_name in &init_fns {
                    self.emit_line(&format!("{}();", init_name));
                }
                self.gen_block_inner(&eff_body);
                self.indent -= 1;
                self.emit_line("};");
                self.emit_line("__block_on(__async_main);");
            } else {
                self.emit_line("let __async_main = async {");
                self.indent += 1;
                self.gen_block_inner(&eff_body);
                self.indent -= 1;
                self.emit_line("};");
                self.emit_line("__block_on(__async_main);");
            }
        } else {
            // @init：模块初始化函数按声明顺序注入 main 开头
            if f.name == "main" && !self.init_fns.is_empty() {
                let init_fns = self.init_fns.clone();
                for init_name in &init_fns {
                    self.emit_line(&format!("{}();", init_name));
                }
            }
            self.gen_block_inner(&eff_body);
        }

        // @memoize：收尾——将计算结果写入缓存并返回
        if is_memoize {
            self.indent -= 1;
            self.emit_line("})();");
            self.emit_line("{");
            self.indent += 1;
            self.emit_line(&format!(
                "let mut __lz_memo_guard = {}.lock().unwrap();",
                memo_static_name
            ));
            self.emit_line("let __lz_memo = __lz_memo_guard.as_mut().unwrap();");
            self.emit_line("__lz_memo.push((__lz_memo_key.clone(), __lz_result.clone()));");
            self.indent -= 1;
            self.emit_line("}");
            self.emit_line("__lz_result");
        }
        self.nested_fn_ret = saved_nested_fn_ret;
        self.current_ret_ty = None;
        self.current_fn_raises = None;
        self.fstr_var_types = saved_fstr_params;
        self.is_main = false;
        self.in_generator = saved_generator;
        self.in_generic_fn = saved_generic_fn;
        self.in_math_fn = saved_math_fn;

        // 生成器：追加 return __gen_vec
        if has_yield {
            self.emit_line("return __gen_vec;");
        }

        self.indent -= 1;
        self.emit_line("}");

        // typed main：追加 pub fn main() 包装（std::process::exit 接收退出码）
        if is_typed_main {
            self.emit_line("pub fn main() {");
            self.indent += 1;
            self.emit_line("std::process::exit(__lz_main() as i32);");
            self.indent -= 1;
            self.emit_line("}");
        }

        // 恢复嵌套前的 cur_fn_name（见函数起始处说明）
        self.cur_fn_name = saved_name;
    }

    pub(crate) fn gen_struct_def(&mut self, s: &StructDef) {
        if self.emitted_types.contains(&s.name) {
            return;
        }
        self.emitted_types.insert(s.name.clone());
        // 记录字段信息，供 __new__ 补齐默认字段
        self.struct_fields_info.insert(
            s.name.clone(),
            s.fields
                .iter()
                .map(|f| (f.name.clone(), f.ty.clone()))
                .collect(),
        );
        if s.has_new {
            self.struct_has_new.insert(s.name.clone());
        }
        // case struct 自动配提取魔法方法；也与显式实现 __unapply__ / __unapply_seq__ 的普通 struct 一致。
        if s.is_case {
            self.case_structs.insert(s.name.clone());
            self.struct_has_unapply.insert(s.name.clone());
            if !s.fields.is_empty() {
                let first = &s.fields[0].ty;
                if s.fields.iter().all(|f| &f.ty == first) {
                    self.struct_has_unapply_seq.insert(s.name.clone());
                }
            }
        }
        if s.methods.iter().any(|m| m.name == "__unapply__") {
            self.struct_has_unapply.insert(s.name.clone());
        }
        if s.methods.iter().any(|m| m.name == "__unapply_seq__") {
            self.struct_has_unapply_seq.insert(s.name.clone());
        }

        let generics = self.gen_generics(&s.generics);
        // __clone__ / __repr__ 自定义语义（06d §十 / §五）：定义了这些魔法方法
        // 的 struct 不得 derive(Clone)/derive(Debug)（手动 impl 会 E0119），
        // 改为生成委托 impl——使 `a.clone()` / `format!("{:?}", a)` 走用户
        // 自定义逻辑。注意方法可能定义在 impl 块（ImplDef.methods），须查
        // struct_method_names（已合并 impl 块方法），仅查 s.methods 会漏判。
        let method_names_set = self.struct_method_names(&s.name);
        let has_clone_magic = method_names_set.contains("__clone__")
            || s.methods.iter().any(|m| m.name == "__clone__");
        let has_repr_magic =
            method_names_set.contains("__repr__") || s.methods.iter().any(|m| m.name == "__repr__");
        let has_eq_magic =
            method_names_set.contains("__eq__") || s.methods.iter().any(|m| m.name == "__eq__");
        // derive 与手动 impl 互斥：__eq__ 由 gen_magic_trait_impls 手动生成 PartialEq，
        // __repr__ 手动生成 Debug，__clone__ 手动生成 Clone，均不再 derive。
        // @derive(...) 装饰器合并：默认 derive（Debug/Clone/PartialEq）基础上追加用户请求的 trait
        let mut derives_all: Vec<String> = vec!["Debug".into(), "Clone".into(), "PartialEq".into()];
        if has_repr_magic {
            derives_all.retain(|d| d != "Debug");
        }
        if has_clone_magic {
            derives_all.retain(|d| d != "Clone");
        }
        if has_eq_magic {
            derives_all.retain(|d| d != "PartialEq");
        }
        for u in &s.derives {
            if !derives_all.iter().any(|d| d == u) {
                derives_all.push(u.clone());
            }
        }
        self.emit_line(&format!("#[derive({})]", derives_all.join(", ")));
        self.emit_line(&format!("pub struct {}{} {{", s.name, generics));
        self.indent += 1;
        for field in &s.fields {
            // 递归字段自动 Box：字段类型直接/间接引用 struct 自身时（如 next: Self?），
            // 生成 Box<...> 避免 Rust 无限大小类型错误（E0072）。
            // Self 字段在 struct 定义内解析为自身类型名（递归替换包裹类型）。
            let self_ty = IrType::Named {
                path: s.name.clone(),
                args: s
                    .generics
                    .iter()
                    .map(|g| IrType::Generic(g.name.clone()))
                    .collect(),
            };
            let field_ty = replace_self(&field.ty, &self_ty);
            let needs_box = field_needs_box(&field_ty, &s.name);
            let ty_str = if needs_box {
                // Option<Self> → Option<Box<Self>>；裸 Self → Box<Self>；Vec<Self> → Vec<Box<Self>>
                if let IrType::Option(inner) = &field_ty {
                    format!("Option<Box<{}>>", self.rust_type(inner))
                } else if let IrType::Named { path, args } = &field_ty {
                    if path == "Option" {
                        format!("Option<Box<{}>>", self.rust_type(&field_ty))
                    } else if path == "Vec" || path == "List" {
                        format!("Vec<Box<{}>>", self.rust_type(&args[0]))
                    } else {
                        format!("Box<{}>", self.rust_type(&field_ty))
                    }
                } else {
                    format!("Box<{}>", self.rust_type(&field_ty))
                }
            } else {
                self.rust_type(&field_ty)
            };
            self.emit_line(&format!("pub {}: {},", field.name, ty_str));
        }
        // 未使用的泛型参数（box.lz `struct Box<T> { _ptr: int }`）：Rust 报
        // E0392 type parameter never used。自动追加 PhantomData 字段。
        for g in &s.generics {
            let used = s.fields.iter().any(|f| type_refers_to(&f.ty, &g.name));
            if !used {
                let rt = self.rust_type(&IrType::Generic(g.name.clone()));
                self.emit_line(&format!(
                    "pub _lz_phantom_{}: std::marker::PhantomData<{}>,",
                    g.name, rt
                ));
                self.struct_phantom_generics
                    .entry(s.name.clone())
                    .or_default()
                    .push(g.name.clone());
            }
        }
        self.indent -= 1;
        self.emit_line("}");

        // 如果 struct 有 __new__ 或 __init__，生成 impl 块
        if s.has_new || s.has_init {
            self.buf.push('\n');
            let impl_generics = if s.generics.is_empty() {
                String::new()
            } else {
                let params: Vec<String> = s
                    .generics
                    .iter()
                    .map(|g| format!("{}: Clone + std::fmt::Debug", g.name))
                    .collect();
                format!("<{}>", params.join(", "))
            };
            self.emit_line(&format!("impl{} {}{} {{", impl_generics, s.name, generics));
            self.indent += 1;
            // 生成 __new__ 函数签名（如有）
            if s.has_new {
                let params: Vec<String> = s
                    .new_params
                    .iter()
                    .map(|(n, t)| format!("{}: {}", n, self.rust_type(t)))
                    .collect();
                let ret_ty = s
                    .new_ret_ty
                    .as_ref()
                    .map(|t| self.rust_type(t))
                    .unwrap_or_else(|| format!("{}{}", s.name, generics));
                self.emit_line(&format!(
                    "pub fn __new__({}) -> {} {{",
                    params.join(", "),
                    ret_ty
                ));
                self.indent += 1;
                // body: 优先使用用户定义的 __new__ 体（struct 体内定义时保留），否则生成占位体
                let prev_in_new_body = self.in_new_body;
                self.in_new_body = true; // 抑制体内 kwarg 构造路由到 __new__（避免无限递归）
                if let Some(body) = &s.new_body {
                    self.gen_block_inner(body);
                } else {
                    self.emit_line(&format!(
                        "{}{} {{ {} }}",
                        s.name,
                        generics,
                        s.fields
                            .iter()
                            .map(|f| format!(
                                "{}: {}",
                                f.name,
                                if s.new_params.iter().any(|(n, _)| n == &f.name) {
                                    f.name.clone()
                                } else {
                                    self.default_value_for(&f.ty)
                                }
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                self.in_new_body = prev_in_new_body;
                self.indent -= 1;
                self.emit_line("}");
            }
            // 生成 __init__ 方法（如有）
            if s.has_init {
                let init_params: Vec<String> = s
                    .init_params
                    .iter()
                    .filter(|(n, _)| n != "self")
                    .map(|(n, t)| format!("{}: {}", n, self.rust_type(t)))
                    .collect();
                self.emit_line(&format!(
                    "pub fn __init__(&mut self{}) {{",
                    if init_params.is_empty() {
                        String::new()
                    } else {
                        format!(", {}", init_params.join(", "))
                    }
                ));
                self.indent += 1;
                if let Some(body) = &s.init_body {
                    self.gen_block_inner(body);
                } else {
                    self.emit_line("// __init__ body (user-defined initialization)");
                }
                self.indent -= 1;
                self.emit_line("}");
            }
            self.indent -= 1;
            self.emit_line("}");
        }

        // 如果 struct 有 __implicit_from__，生成 ImplicitFrom trait impl
        if !s.implicit_froms.is_empty() {
            self.buf.push('\n');
            // 生成 ImplicitFrom trait 定义（首次使用时）
            self.emit_line("// trait ImplicitFrom<T> { fn implicit_from(value: T) -> Self; }");
            for src_ty in &s.implicit_froms {
                let src_rust = self.rust_type(src_ty);
                let impl_generics = if s.generics.is_empty() {
                    String::new()
                } else {
                    let params: Vec<String> = s
                        .generics
                        .iter()
                        .map(|g| format!("{}: Clone + std::fmt::Debug", g.name))
                        .collect();
                    format!("<{}>", params.join(", "))
                };
                self.emit_line(&format!(
                    "impl{} ImplicitFrom<{}> for {}{} {{",
                    impl_generics, src_rust, s.name, generics
                ));
                self.indent += 1;
                let ret_ty = format!("{}{}", s.name, generics);
                self.emit_line(&format!(
                    "fn __implicit_from__(value: {}) -> {} {{",
                    src_rust, ret_ty
                ));
                self.indent += 1;
                // 构造调用：使用关键字构造，value 映射到第一个字段
                self.emit_line(&format!(
                    "{} {{ {}: value, ..{}::default() }}",
                    ret_ty,
                    s.fields.first().map(|f| f.name.as_str()).unwrap_or("_"),
                    ret_ty
                ));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // 方法（impl 块）
        if !s.methods.is_empty() {
            self.buf.push('\n');
            // 为泛型参数添加 Clone + Debug 约束
            // Clone 支持 self.clone() 提取值，Debug 支持 f-string {:?} 插值
            let impl_generics = if s.generics.is_empty() {
                String::new()
            } else {
                let params: Vec<String> = s
                    .generics
                    .iter()
                    .map(|g| {
                        if g.bounds.is_empty() {
                            format!("{}: Clone + std::fmt::Debug", g.name)
                        } else {
                            let bounds: Vec<String> =
                                g.bounds.iter().map(|b| self.rust_type(b)).collect();
                            format!(
                                "{}: Clone + std::fmt::Debug + {}",
                                g.name,
                                bounds.join(" + ")
                            )
                        }
                    })
                    .collect();
                format!("<{}>", params.join(", "))
            };
            self.emit_line(&format!("impl{} {}{} {{", impl_generics, s.name, generics));
            self.indent += 1;
            // 泛型 struct（struct MyIterator<T>）内联方法按泛型上下文处理：
            // Option.None 生成 Option::None 由返回类型推断（magic_methods.lz __next__ E0308）
            let saved_impl_generic = self.in_impl_generic;
            self.in_impl_generic = !s.generics.is_empty();
            let saved_impl_generics = std::mem::take(&mut self.current_impl_generics);
            self.current_impl_generics = s.generics.iter().map(|g| g.name.clone()).collect();
            let saved_fstr_self = self.fstr_self_type.take();
            self.fstr_self_type = Some(s.name.clone());
            for m in &s.methods {
                // 登记 struct 方法的默认参数信息，供调用点补 None
                // 键用 struct::method 格式，避免不同 struct 的同名方法（如 new）冲突
                let default_count = m.params.iter().filter(|p| p.default.is_some()).count();

                if default_count > 0 {
                    let method_key = format!("{}::{}", s.name, m.name);

                    self.fn_param_info
                        .insert(method_key, (m.params.len(), default_count));
                }
                // 登记 new 方法的参数表，供 kwarg 构造路由
                // 注意：只登记魔术 __new__（构造器），不登记普通 new 方法
                // 普通 new 方法体内部 struct 构造应走字面量路由，不应路由到 new 自身（否则无限递归）
                if m.name == "__new__" {
                    let params: Vec<(String, IrType)> = m
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), p.ty.clone()))
                        .collect();

                    self.struct_new_params_map.insert(s.name.clone(), params);
                }
                self.gen_fn_def(m);
                self.buf.push('\n');
            }
            self.current_impl_generics = saved_impl_generics;
            self.fstr_self_type = saved_fstr_self;
            self.in_impl_generic = saved_impl_generic;
            self.indent -= 1;
            self.emit_line("}");
        }

        // 为 struct 内联魔法方法补 trait impl（Display/Debug/Iterator/IntoIterator/AddAssign），
        // 覆盖 gen_struct_def 路径（struct 内魔法方法不走 emit_impl）。与 emit_impl 共用同一逻辑。
        // 泛型 struct 的 trait impl 须与方法块使用相同的 Clone + Debug 约束，否则方法调用
        // 因约束未满足报 E0599（MyList<T>/MyIterator<T> 实测）。
        if !s.methods.is_empty() {
            self.buf.push('\n');
            let impl_generics = if s.generics.is_empty() {
                String::new()
            } else {
                let params: Vec<String> = s
                    .generics
                    .iter()
                    .map(|g| {
                        if g.bounds.is_empty() {
                            format!("{}: Clone + std::fmt::Debug", g.name)
                        } else {
                            let bounds: Vec<String> =
                                g.bounds.iter().map(|b| self.rust_type(b)).collect();
                            format!(
                                "{}: Clone + std::fmt::Debug + {}",
                                g.name,
                                bounds.join(" + ")
                            )
                        }
                    })
                    .collect();
                format!("<{}>", params.join(", "))
            };
            let for_ty = format!("{}{}", s.name, generics);
            self.gen_magic_trait_impls(&for_ty, &impl_generics, &s.methods);
        }
    }

    pub(crate) fn gen_enum_def(&mut self, e: &EnumDef) {
        // 去重：同名 enum 已生成则跳过
        if self.emitted_types.contains(&e.name) {
            return;
        }
        self.emitted_types.insert(e.name.clone());

        let generics = self.gen_generics(&e.generics);
        // @derive(...) 装饰器合并：默认 derive 基础上追加用户请求的 trait
        let mut derives_all: Vec<String> = vec!["Debug".into(), "Clone".into(), "PartialEq".into()];
        for u in &e.derives {
            if !derives_all.iter().any(|d| d == u) {
                derives_all.push(u.clone());
            }
        }
        let _derives_partial_eq = derives_all.iter().any(|d| d == "PartialEq" || d == "Eq");
        self.emit_line(&format!("#[derive({})]", derives_all.join(", ")));
        self.emit_line(&format!("pub enum {}{} {{", e.name, generics));
        self.indent += 1;
        for variant in &e.variants {
            if variant.fields.is_empty() {
                self.emit_line(&format!("{},", variant.name));
            } else {
                let named = variant.fields.iter().all(|f| !f.name.is_empty());
                if named {
                    self.emit_line(&format!("{} {{", variant.name));
                    self.indent += 1;
                    for f in &variant.fields {
                        let mut rust_ty = self.rust_type(&f.ty);
                        if type_refers_to(&f.ty, &e.name) {
                            rust_ty = format!("Box<{}>", rust_ty);
                        }
                        // 枚举变体字段自动继承枚举可见性，不允许 pub 限定（E0449）
                        self.emit_line(&format!("{}: {},", f.name, rust_ty));
                    }
                    self.indent -= 1;
                    self.emit_line("},");
                } else {
                    let types: Vec<String> = variant
                        .fields
                        .iter()
                        .map(|f| {
                            let mut rust_ty = self.rust_type(&f.ty);
                            if type_refers_to(&f.ty, &e.name) {
                                rust_ty = format!("Box<{}>", rust_ty);
                            }
                            rust_ty
                        })
                        .collect();
                    self.emit_line(&format!("{}({}),", variant.name, types.join(", ")));
                }
            }
        }
        self.indent -= 1;
        self.emit_line("}");

        // 方法（impl 块）
        if !e.methods.is_empty() {
            self.buf.push('\n');
            // 枚举方法 impl：为泛型参数添加 Clone + Debug 约束
            // Clone 支持 self.clone() 提取值，Debug 支持 f-string {:?} 插值
            let impl_generics = if e.generics.is_empty() {
                String::new()
            } else {
                let params: Vec<String> = e
                    .generics
                    .iter()
                    .map(|g| {
                        if g.bounds.is_empty() {
                            format!("{}: Clone + std::fmt::Debug", g.name)
                        } else {
                            let bounds: Vec<String> =
                                g.bounds.iter().map(|b| self.rust_type(b)).collect();
                            format!(
                                "{}: Clone + std::fmt::Debug + {}",
                                g.name,
                                bounds.join(" + ")
                            )
                        }
                    })
                    .collect();
                format!("<{}>", params.join(", "))
            };
            self.emit_line(&format!("impl{} {}{} {{", impl_generics, e.name, generics));
            self.indent += 1;
            for m in &e.methods {
                self.gen_fn_def(m);
                self.buf.push('\n');
            }
            self.indent -= 1;
            self.emit_line("}");
        }

        // 枚举 __eq__ 自动生成已移除——与 lz codegen 对齐（lz codegen 不自动生成枚举 __eq__）
    }

    pub(crate) fn gen_trait_def(&mut self, t: &TraitDef) {
        let generics = self.gen_generics(&t.generics);
        let supertraits = if t.supertraits.is_empty() {
            String::new()
        } else {
            // supertrait 名不能用 dyn（`trait X: dyn Iterator` 非法，invalid dyn
            // keyword）：走 rust_type_name（不触发 trait_names 的 dyn 生成）
            let st: Vec<String> = t
                .supertraits
                .iter()
                .map(|s| match s {
                    IrType::Named { path, args } if args.is_empty() => self.rust_type_name(path),
                    _ => self.rust_type(s),
                })
                .collect();
            format!(": {}", st.join(" + "))
        };
        self.emit_line(&format!(
            "pub trait {}{}{} {{",
            t.name, generics, supertraits
        ));
        self.indent += 1;
        // trait 自身泛型名（用于下方 trait 方法签名去重，避免 E0403 重复声明）
        let trait_gen_names: Vec<String> = t.generics.iter().map(|g| g.name.clone()).collect();
        // 关联类型声明（§五 `type Item`）→ Rust trait 关联类型
        for a in &t.assoc_types {
            self.emit_line(&format!("type {};", a));
        }
        for sig in &t.methods {
            // 关联/默认方法签名里 builder 已把 trait 的外层泛型合并进 sig.generics，
            // 若原样重声明会生成 `trait SetExt<T> { fn len<T>(..) }` → E0403（T 重复）。
            // 剔除与 trait 同名的外层泛型（方法自身泛型如 <C> 保留），对齐
            // gen_fn_def 的 current_impl_generics 去重逻辑。
            let m_gen = if sig.generics.is_empty() {
                String::new()
            } else {
                let (ceq, cor) = self
                    .fn_cmp
                    .get(&sig.name)
                    .cloned()
                    .unwrap_or_else(|| (HashSet::new(), HashSet::new()));
                let filtered: Vec<GenericParam> = sig
                    .generics
                    .iter()
                    .filter(|g| !trait_gen_names.contains(&g.name))
                    .cloned()
                    .collect();
                self.gen_fn_generics(&filtered, &ceq, &cor)
            };
            let has_body = sig.body.is_some();
            // 如果第一个参数是 Self，转为 &self（trait 方法与 impl 块签名需一致）
            let params: Vec<String> = sig
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if i == 0 && matches!(p, IrType::Self_) {
                        "&self".to_string()
                    } else if i == 0
                        && matches!(p, IrType::MutRef(inner) if matches!(&**inner, IrType::Self_))
                    {
                        "&mut self".to_string()
                    } else if i == 0
                        && matches!(p, IrType::Ref(inner) if matches!(&**inner, IrType::Self_))
                    {
                        // ref self（&Self）：生成 &self 方法（否则 _p0: &Self 是
                        // 关联函数，E0038 trait Error is not dyn compatible）
                        "&self".to_string()
                    } else if has_body {
                        // 默认方法体用真实参数名（E0425 cannot find value other）
                        let pname = sig
                            .params_names
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("_p{}", i));
                        format!("{}: {}", pname, self.trait_sig_type(p, &t.name))
                    } else {
                        // trait 抽象方法参数需带参数名（否则 `fn configure(&self, Dict<..>)`
                        // 报语法错误，combo-trait-impl.lz）。FnSig 不保存参数名，
                        // 按位置生成 _pN（Rust trait 实现允许参数名不同）
                        format!("_p{}: {}", i, self.trait_sig_type(p, &t.name))
                    }
                })
                .collect();
            // trait 方法 where 约束（try_from ... where Self: Sized / map ... where
            // Self: Iterator）：生成到方法签名（E0277 Self is not Sized / an iterator）
            let m_where = if sig.where_clause.is_empty() {
                // trait Iterator（自定义，LZ 迭代协议）的方法：完全限定
                // <Self as std::iter::Iterator>::Item 需 Self: std::iter::Iterator
                // 约束（E0277 Self is not an iterator）。但该约束会使 trait 失去
                // dyn 兼容性（E0038 the trait Iterator is not dyn compatible），
                // 因此仅当方法体（默认方法）或签名类型中确实引用了关联类型
                // Self.Item 时才追加；纯抽象方法（如 lib_iterator 的
                // `def next(mut self) -> Option<int>`）签名不含关联类型，
                // 不应加，否则 collect(mut iter: dyn Iterator) 报 E0038
                // BUG-traits-SelfItem：递归检测类型中是否出现 Self.Item / Self. 关联类型
                // （含 fn(...) 参数、Option<T> 嵌套），缺失会导致 `where Self:
                // std::iter::Iterator` 漏加 → E0277 Self is not an iterator。
                fn ty_has_self_item(ty: &IrType) -> bool {
                    match ty {
                        IrType::Named { path, args } => {
                            path.contains("Self.")
                                || path == "Self"
                                || args.iter().any(ty_has_self_item)
                        }
                        IrType::Ref(i) | IrType::MutRef(i) => ty_has_self_item(i),
                        IrType::Option(i) => ty_has_self_item(i),
                        IrType::Result { ok, err } => ty_has_self_item(ok) || ty_has_self_item(err),
                        IrType::Tuple(e) => e.iter().any(ty_has_self_item),
                        IrType::Fn { params, ret } => {
                            params.iter().any(ty_has_self_item) || ty_has_self_item(ret)
                        }
                        _ => false,
                    }
                }
                let uses_self_item = sig.body.is_some()
                    || ty_has_self_item(&sig.ret)
                    || sig.params.iter().any(ty_has_self_item);
                if t.name == "Iterator" && self.custom_iterator_is_protocol && uses_self_item {
                    "\nwhere\nSelf: std::iter::Iterator".to_string()
                } else {
                    String::new()
                }
            } else {
                // trait Iterator 的方法：where 约束里的 Self::Item（sum 的
                // where Self.Item: Add）需完全限定（E0221 歧义），并追加
                // Self: std::iter::Iterator（E0277 Self is not an iterator）
                let is_iter_trait = t.name == "Iterator" && self.custom_iterator_is_protocol;
                let mut wc: Vec<String> = sig
                    .where_clause
                    .iter()
                    .map(|(tp, bounds)| {
                        let bs: Vec<String> = bounds
                            .iter()
                            .map(|b| {
                                let bs = self.gen_trait_bound(b);
                                if is_iter_trait {
                                    bs.replace("Self::", "<Self as std::iter::Iterator>::")
                                } else {
                                    bs
                                }
                            })
                            .collect();
                        let tp_s = tp.replace(".", "::");
                        let tp_s = if is_iter_trait && tp == "Self.Item" {
                            "<Self as std::iter::Iterator>::Item".to_string()
                        } else {
                            tp_s
                        };
                        format!("{}: {}", tp_s, bs.join(" + "))
                    })
                    .collect();
                if is_iter_trait && !sig.where_clause.iter().any(|(tp, _)| tp == "Self") {
                    wc.push("Self: std::iter::Iterator".to_string());
                }
                format!("\nwhere\n{}", wc.join(",\n"))
            };
            let ret = if sig.ret != IrType::Unit {
                format!(" -> {}", self.trait_sig_type(&sig.ret, &t.name))
            } else {
                String::new()
            };
            // trait 默认方法（带 body）：生成方法体而非分号结尾的抽象签名
            if let Some(block) = &sig.body {
                let mut child = CodeGen::new();
                child.current_fn_raises = self.current_fn_raises.clone();
                child.current_ret_ty = self.current_ret_ty.clone();
                child.current_fn_ret_ty = self.current_fn_ret_ty.clone();
                child.enum_variant_named_fields = self.enum_variant_named_fields.clone();
                child.emitted_types = self.emitted_types.clone();
                child.enum_variants = self.enum_variants.clone();
                child.enum_variant_fields = self.enum_variant_fields.clone();
                child.fn_param_info = self.fn_param_info.clone();
                child.in_generator = self.in_generator;
                child.suppress_tail_return = true;
                // 继承 struct 方法名集合（trait 默认方法体内调用用户 struct 的
                // next/f 等方法时 user_plain 判定需要，否则误映射 __next__）
                child.struct_method_names_map = self.struct_method_names_map.clone();
                child.struct_init_params_map = self.struct_init_params_map.clone();
                child.struct_new_params_map = self.struct_new_params_map.clone();
                child.in_new_body = self.in_new_body;
                // 未使用泛型的 PhantomData 补全需传递（trait 默认方法构造 FlatMap
                // 等适配器 struct 时，否则 E0063 missing field _lz_phantom_B）
                child.struct_phantom_generics = self.struct_phantom_generics.clone();
                // trait 默认方法 self 是 &Self（`self: &Self` 参数）：比较时解引用
                // （self < other → *self < *other，E0369）
                child.borrow_self = true;
                child.gen_block_inner(block);
                self.emit_line(&format!(
                    "fn {}{}({}){}{} {{",
                    sig.name,
                    m_gen,
                    params.join(", "),
                    ret,
                    m_where
                ));
                self.indent += 1;
                self.emit_line(&child.buf);
                self.indent -= 1;
                self.emit_line("}");
            } else {
                self.emit_line(&format!(
                    "fn {}{}({}){}{};",
                    sig.name,
                    m_gen,
                    params.join(", "),
                    ret,
                    m_where
                ));
            }
        }
        self.indent -= 1;
        self.emit_line("}");
    }

    /// 生成 trait 方法签名中的类型：`Self.Item`（§五 关联类型引用）→ `Self::Item`，
    /// 其余类型走 rust_type。仅用于 trait 方法签名。
    pub(crate) fn trait_sig_type(&self, ty: &IrType, current_trait: &str) -> String {
        match ty {
            IrType::Named { path, args } => {
                // `Self.Item`：path 含点号且前缀是 Self
                if let Some((owner, member)) = path.split_once('.') {
                    if owner == "Self" {
                        if args.is_empty() {
                            // 完全限定语法仅用于 trait Iterator（where Self:
                            // std::iter::Iterator 时 Self::Item 歧义 E0221）；
                            // 其他 trait（TryFrom<T>/DoubleEndedIterator: Iterator）
                            // 用简单 Self::member（避免 E0107 missing generics /
                            // E0576 cannot find associated type in supertrait）
                            if current_trait == "Iterator" && self.custom_iterator_is_protocol {
                                // 完全限定需用 std::iter::Iterator（where Self:
                                // std::iter::Iterator 的约束）——Map 等适配器 struct
                                // 的字段类型（I::Item → std Item）与 f 参数一致，
                                // 否则 E0308 expected fn(std Item), found fn(custom Item)
                                return format!("<Self as std::iter::Iterator>::{}", member);
                            }
                            return format!("Self::{}", member);
                        }
                        let inner: Vec<String> = args
                            .iter()
                            .map(|a| self.trait_sig_type(a, current_trait))
                            .collect();
                        return format!(
                            "<Self as {}>::{}<{}>",
                            current_trait,
                            member,
                            inner.join(", ")
                        );
                    }
                }
                if args.is_empty() {
                    self.rust_type(ty)
                } else {
                    let inner: Vec<String> = args
                        .iter()
                        .map(|a| self.trait_sig_type(a, current_trait))
                        .collect();
                    format!("{}<{}>", path, inner.join(", "))
                }
            }
            IrType::Option(inner) => {
                format!("Option<{}>", self.trait_sig_type(inner, current_trait))
            }
            IrType::Tuple(items) => {
                let inner: Vec<String> = items
                    .iter()
                    .map(|i| self.trait_sig_type(i, current_trait))
                    .collect();
                format!("({})", inner.join(", "))
            }
            IrType::Ref(inner) => format!("&{}", self.trait_sig_type(inner, current_trait)),
            IrType::MutRef(inner) => {
                format!("&mut {}", self.trait_sig_type(inner, current_trait))
            }
            IrType::Result { ok, err } => format!(
                "Result<{}, {}>",
                self.trait_sig_type(ok, current_trait),
                self.trait_sig_type(err, current_trait)
            ),
            // fn 类型参数（map 的 f: fn(Self::Item) -> B）：内部 Self::Item 也需
            // 完全限定（E0221），否则 fn(Self::Item) 走 rust_type 未转换
            IrType::Fn { params, ret } => {
                let ps: Vec<String> = params
                    .iter()
                    .map(|p| self.trait_sig_type(p, current_trait))
                    .collect();
                format!(
                    "fn({}) -> {}",
                    ps.join(", "),
                    self.trait_sig_type(ret, current_trait)
                )
            }
            other => self.rust_type(other),
        }
    }

    pub(crate) fn gen_impl_def(&mut self, i: &ImplDef) {
        // Rust impl 泛型不允许默认类型参数（E0741），剥离默认值仅保留 bounds；
        // 追加 Clone + Debug bound（LZ 值语义自动 .clone()，泛型需可 Clone）
        let mut stripped: Vec<GenericParam> = i
            .generics
            .iter()
            .map(|g| {
                let mut bounds = g.bounds.clone();
                for b in ["Clone", "std::fmt::Debug"] {
                    let tb = self.gen_trait_bound(&IrType::named(b));
                    if !bounds.iter().any(|x| self.gen_trait_bound(x) == tb) {
                        bounds.push(IrType::named(b));
                    }
                }
                GenericParam {
                    name: g.name.clone(),
                    bounds,
                    default: None,
                }
            })
            .collect();
        // BUG-dict-Ord：Dict/HashMap → BTreeMap<K,V> 要求键泛型 K: Ord。
        // 仅给第一个泛型（键 K）注入 Ord，不污染值泛型 V（Set/HashSet 不走此分支）。
        if matches!(&i.for_type, IrType::Named { path, .. } if path == "Dict" || path == "HashMap")
        {
            if let Some(k) = stripped.first_mut() {
                let ord = IrType::named("Ord");
                if !k
                    .bounds
                    .iter()
                    .any(|b| self.gen_trait_bound(b) == self.gen_trait_bound(&ord))
                {
                    k.bounds.push(ord);
                }
            }
        }
        let generics = self.gen_generics(&stripped);
        let trait_part = i
            .trait_
            .as_ref()
            .map(|t| {
                // impl 目标的 trait 名不能用 dyn（`impl dyn Iterator for X` 非法，
                // E0437 expected a trait, found type）：trait 名走 rust_type_name
                // 不触发 trait_names 的 dyn 生成（dyn 仅用于 &dyn Trait 引用场景）
                let name = match t {
                    IrType::Named { path, args } if args.is_empty() => {
                        // LZ 迭代协议：`impl Iterator for X` 需 std::iter::Iterator
                        //（__next__ → next 映射，in_iterator_impl）；traits.lz 自定义
                        // trait Iterator 遮蔽会报 E0407 method next is not a member。
                        // trait_assoc.lz 自定义 `trait Iterator`（get/peek，非协议）
                        // 时使用本地 trait 名（E0407 method get is not a member）
                        if path == "Iterator" && self.custom_iterator_is_protocol {
                            "std::iter::Iterator".to_string()
                        } else {
                            self.rust_type_name(path)
                        }
                    }
                    _ => self.rust_type(t),
                };
                format!("{} for ", name)
            })
            .unwrap_or_default();
        // LZ 迭代协议（规范 06d-内置魔法trait和全局函数.md §五）：
        // `impl Iterator for X` 用 `__next__`/`__size_hint__` 魔术方法实现，
        // 生成 std::iter::Iterator impl 时方法名需映射为 `next`/`size_hint`（E0407）
        let saved_iterator_impl = self.in_iterator_impl;
        self.in_iterator_impl = matches!(
            &i.trait_,
            Some(IrType::Named { path, .. }) if path == "Iterator" && self.custom_iterator_is_protocol
        );
        // 扩展 trait 场景：方法名需与调用点映射一致（slice → lz_slice）
        let saved_ext_trait = self.in_ext_trait;
        let saved_ext_trait_name = self.current_ext_trait.clone();
        // 外部类型/原始类型扩展（E0116/E0390 修复）：`impl Dict<K,V>` / `impl Set<T>` /
        // `impl List<T>` / `impl str` 等对 type alias / 原始类型的 inherent impl 在 Rust 中
        // 非法（类型定义在外部 crate / 原始类型禁止 inherent impl）。生成扩展 trait：
        //   trait DictExt { fn len(&self) -> i64; ... }
        //   impl<K: Clone + Debug, V: Clone + Debug> DictExt for HashMap<K, V> { ... }
        // 调用点 d.len() 需要 trait 在作用域——同文件顶层定义自动可见。
        let ext_trait_name = match &i.for_type {
            IrType::Named { path, .. }
                if !self.emitted_types.contains(path.as_str())
                    && !self.known_types.contains(path.as_str()) =>
            {
                match path.as_str() {
                    "Dict" | "HashMap" => Some("DictExt".to_string()),
                    "Set" | "HashSet" => Some("SetExt".to_string()),
                    "List" | "Vec" => Some("ListExt".to_string()),
                    "str" | "String" => Some("StrExt".to_string()),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(ext_name) = &ext_trait_name {
            // 扩展 trait 场景：方法名需与调用点映射一致（slice → lz_slice）
            self.in_ext_trait = true;
            self.current_ext_trait = Some(ext_name.clone());
            // 扩展 trait 声明：方法签名（无 body）
            // 扩展 trait 的泛型需携带 impl 端同名泛型的约束（如 `impl<T: Clone+Debug>
            // Set<T>` → `trait SetExt<T: Clone + Debug>`），否则 trait 方法签名中 T
            // 无约束、而 impl 方法体要求 T: Clone+Debug → E0276（impl 比 trait 约束更严）。
            let trait_gen_names: Vec<String> = stripped.iter().map(|g| g.name.clone()).collect();
            // 带约束的泛型串：用于 `trait SetExt<T: Clone + Debug>` 声明（声明处允许约束）
            let trait_gen_str = if trait_gen_names.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    stripped
                        .iter()
                        .map(|g| {
                            let b: Vec<String> =
                                g.bounds.iter().map(|x| self.gen_trait_bound(x)).collect();
                            if b.is_empty() {
                                g.name.clone()
                            } else {
                                format!("{}: {}", g.name, b.join(" + "))
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            // 仅名字的泛型串：用于 `impl ... SetExt<T> for HashSet<T>` 的 trait 引用
            // （trait 路径里不允许写约束 `SetExt<T: Clone+Debug>`，否则 E0229）
            let trait_gen_ref = if trait_gen_names.is_empty() {
                String::new()
            } else {
                format!("<{}>", trait_gen_names.join(", "))
            };
            self.emit_line(&format!("trait {}{} {{", ext_name, trait_gen_str));
            self.indent += 1;
            for m in &i.methods {
                // 方法自身泛型参数（map<U>/map<K2,V2> 等）需在 trait 签名中声明，
                // 否则 E0425 cannot find type `U`
                let m_gen = {
                    let (ceq, cor) = self
                        .fn_cmp
                        .get(&m.name)
                        .cloned()
                        .unwrap_or_else(|| (HashSet::new(), HashSet::new()));
                    // 剔除与扩展 trait 同名的外层泛型（如 `impl<T> Set<T>` 的 T），
                    // 否则生成 `trait SetExt<T> { fn len<T>(..) }` → E0403（T 重复），
                    // 且与 impl 端 `fn len(&self)`（已去重）参数数不一致 → E0049。
                    let filtered: Vec<GenericParam> = m
                        .generics
                        .iter()
                        .filter(|g| !trait_gen_names.contains(&g.name))
                        .cloned()
                        .collect();
                    self.gen_fn_generics(&filtered, &ceq, &cor)
                };
                // 参数渲染与 impl 端保持一致：Fn 类型参数 → `impl Fn(...)`（闭包），
                // 否则 trait 声明 `fn(&V) -> U` 只有 1 个类型参数而 impl 端
                // `impl Fn(&V) -> U` 有 2 个（E0049 type parameter count mismatch）
                let params: Vec<String> = m
                    .params
                    .iter()
                    .map(|p| {
                        if p.name == "self" {
                            self.gen_param(p)
                        } else if let IrType::Fn { params: fp, ret } = &p.ty {
                            let ps: Vec<String> = fp.iter().map(|pt| self.rust_type(pt)).collect();
                            // 与 impl 端闭包参数渲染保持一致（均带 `+ 'static`），
                            // 否则 trait 声明 `impl Fn(&T) -> U` 与 impl 端
                            // `impl Fn(&T) -> U + 'static` 约束不一致 → E0276。
                            format!(
                                "{}: impl Fn({}) -> {} + 'static",
                                p.name,
                                ps.join(", "),
                                self.rust_type(ret)
                            )
                        } else {
                            self.gen_param(p)
                        }
                    })
                    .collect();
                let ret = if m.ret_ty != IrType::Unit {
                    format!(" -> {}", self.rust_type(&m.ret_ty))
                } else {
                    String::new()
                };
                // 方法 where 约束（如 `where K: Eq + Hash`，K 为 impl 级泛型）：
                // trait 声明需与 impl 端一致（E0276 impl has stricter requirements）
                let m_where = if m.where_clause.is_empty() {
                    String::new()
                } else {
                    let wc: Vec<String> = m
                        .where_clause
                        .iter()
                        .map(|(tp, bounds)| {
                            let bs: Vec<String> =
                                bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                            format!("{}: {}", tp, bs.join(" + "))
                        })
                        .collect();
                    format!("\nwhere\n{}", wc.join(",\n"))
                };
                self.emit_line(&format!(
                    "fn {}{}({}){}{};",
                    self.ext_trait_method_name(&m.name, ext_name),
                    m_gen,
                    params.join(", "),
                    ret,
                    m_where
                ));
            }
            self.indent -= 1;
            self.emit_line("}");
            // inherent impl（Peekable 等单独 impl）也需 where 约束（I::Item: Clone）
            let wc_in = if i.where_clause.is_empty() {
                String::new()
            } else {
                let wc: Vec<String> = i
                    .where_clause
                    .iter()
                    .map(|(tp, bounds)| {
                        let bs: Vec<String> =
                            bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                        let tp_s = tp.replace(".", "::");
                        format!("{}: {}", tp_s, bs.join(" + "))
                    })
                    .collect();
                format!(" where {}", wc.join(", "))
            };
            self.emit_line(&format!(
                "impl{} {} for {}{} {{",
                generics,
                format!("{}{}", ext_name, trait_gen_ref),
                self.rust_type(&i.for_type),
                wc_in
            ));
        } else {
            // impl 级 where 约束（`impl ... for Peekable<I> where I::Item: Clone`：
            // 关联类型约束，Option<I::Item>: Clone 需要 I::Item: Clone，E0599）
            // 需在 { 之前生成，否则 non-item in item list
            let wc_s = if i.where_clause.is_empty() {
                String::new()
            } else {
                let wc: Vec<String> = i
                    .where_clause
                    .iter()
                    .map(|(tp, bounds)| {
                        let bs: Vec<String> =
                            bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                        let tp_s = tp.replace(".", "::");
                        format!("{}: {}", tp_s, bs.join(" + "))
                    })
                    .collect();
                format!(" where {}", wc.join(", "))
            };
            self.emit_line(&format!(
                "impl{} {}{}{} {{",
                generics,
                trait_part,
                self.rust_type(&i.for_type),
                wc_s
            ));
        }
        self.indent += 1;
        // 关联类型绑定（§五 `type Item = T`）→ Rust 关联类型实现
        for (name, ty) in &i.assoc_type_bindings {
            self.emit_line(&format!("type {} = {};", name, self.rust_type(ty)));
        }
        // 泛型 impl 块（impl<T> ...）内方法按泛型上下文处理：
        // Option.None 生成 Option::None 由返回类型推断（magic_methods.lz __next__ E0308）
        let saved_impl_generic = self.in_impl_generic;
        self.in_impl_generic = !i.generics.is_empty();
        let saved_impl_generics = std::mem::take(&mut self.current_impl_generics);
        self.current_impl_generics = i.generics.iter().map(|g| g.name.clone()).collect();
        let saved_fstr_self = self.fstr_self_type.take();
        self.fstr_self_type = match &i.for_type {
            IrType::Named { path, .. } => Some(path.clone()),
            _ => None,
        };
        for m in &i.methods {
            // __new__ 体内抑制 kwarg→__new__ 路由（避免 Self(v:...) 无限递归）
            let prev_in_new_body = self.in_new_body;
            if m.name == "__new__" || m.name == "new" {
                self.in_new_body = true;
            }
            self.gen_fn_def(m);
            self.in_new_body = prev_in_new_body;
            self.buf.push('\n');
        }
        self.current_impl_generics = saved_impl_generics;
        self.fstr_self_type = saved_fstr_self;
        self.in_impl_generic = saved_impl_generic;
        self.in_iterator_impl = saved_iterator_impl;
        self.in_ext_trait = saved_ext_trait;
        self.current_ext_trait = saved_ext_trait_name;
        self.indent -= 1;
        self.emit_line("}");
        // 自动生成 PartialEq：struct 定义 `__eq__` 魔术方法时（box.lz `Box/Rc/Arc`
        // 的 `def __eq__(ref self, ref other: Box<T>) where T: Eq`），
        // `assert_eq!(result, Ok(100))` 需 Result<T, Rc<T>>: PartialEq（E0369）——
        // 委托 __eq__ 生成 impl，并携带 __eq__ 的 where 约束（T: Eq）。
        // 枚举已有 #[derive(PartialEq)]（codegen 自动），跳过避免 E0119 冲突
        let _enum_derives_partial_eq = matches!(&i.for_type, IrType::Named { path, .. }
            if self.enum_variants.values().any(|en| en == path));
        // 外部/内置类型（Vec/str/String/HashMap…）：Rust 孤儿规则禁止为外部类型
        // 实现外部 trait（E0117），且 std 已提供 PartialEq，跳过自动 impl
        let _is_external_type = matches!(&i.for_type, IrType::Named { path, .. }
            if matches!(path.as_str(),
                "List" | "Vec" | "Dict" | "HashMap" | "Set" | "HashSet" | "String" | "str"));
        // 自动生成 PartialEq（__eq__） + 其他魔法方法 trait impl。
        // 枚举已有 #[derive(PartialEq)]，跳过避免 E0119 冲突；外部/内置类型
        // （Vec/str/String…）Rust 孤儿规则禁止实现外部 trait，跳过。
        let enum_derives_partial_eq = matches!(&i.for_type, IrType::Named { path, .. }
            if self.enum_variants.values().any(|en| en == path));
        let is_external_type = matches!(&i.for_type, IrType::Named { path, .. }
            if matches!(path.as_str(),
                "List" | "Vec" | "Dict" | "HashMap" | "Set" | "HashSet" | "String" | "str"));
        if i.trait_.is_none() && !enum_derives_partial_eq && !is_external_type {
            self.gen_magic_trait_impls(&self.rust_type(&i.for_type), &generics, &i.methods);
        }
    }

    pub(crate) fn gen_use_stmt(&mut self, u: &UseStmt) {
        // 映射 LZ 类型名 → Rust 类型名（仅在 import 路径中使用）
        // 以及相对路径前缀映射：. → self, .. → super
        let lz_to_rust: HashMap<&str, &str> = [
            ("List", "Vec"),
            ("Dict", "BTreeMap"),
            ("Set", "HashSet"),
            ("String", "String"),
            ("Nil", "()"),
            ("int", "i64"),
            ("str", "String"),
            ("f64", "f64"),
            ("bool", "bool"),
            (".", "self"),
            ("..", "super"),
        ]
        .iter()
        .cloned()
        .collect();

        // LZ 内建函数/类型：由 codegen 直接生成，不需要 Rust use 语句
        let builtin_items: std::collections::HashSet<&str> = [
            "print", "read", "len", "panic", "type", "range", "spawn", "await", "yield", "comptime",
        ]
        .iter()
        .cloned()
        .collect();

        // 已知的 LZ 模块路径 → Rust 模块路径映射
        // 空字符串 = 无 Rust 对应模块，跳过 use 语句生成
        let known_module_paths: std::collections::HashSet<&str> = [
            "std::io",          // → std::io
            "std::collections", // → std::collections
            "std::sync",        // → std::sync
            "std::rc",          // → std::rc
            "std::time",        // → std::time
            "std::thread",      // → std::thread
            "std::net",         // → std::net
            "std::fs",          // → std::fs
            "std::env",         // → std::env
            "std::process",     // → std::process
            "std::path",        // → std::path
            "std::hash",        // → std::hash
            "std::iter",        // std::iter (稳定)
            "std::mem",         // std::mem (稳定)
            "std::fmt",         // std::fmt (稳定)
            "std::cmp",         // std::cmp (稳定)
            "std::str",         // std::str (稳定)
            "std::marker",      // std::marker (稳定)
            "std::any",         // std::any (稳定)
            "std::convert",     // std::convert (稳定)
            "std::cell",        // std::cell (稳定)
            "std::os",          // std::os (稳定)
        ]
        .iter()
        .cloned()
        .collect();

        // prelude 已导入的项（不需要重复导入）
        let prelude_items: std::collections::HashSet<&str> =
            ["HashMap", "HashSet", "Rc", "Arc", "Vec"]
                .iter()
                .cloned()
                .collect();

        let path: Vec<String> = u
            .path
            .iter()
            .map(|seg| {
                lz_to_rust
                    .get(seg.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| seg.clone())
            })
            .collect();
        let path_str = path.join("::");

        // 相对导入（self::、super::）无法在生成的文件中解析，跳过
        if path_str.starts_with("self::") || path_str.starts_with("super::") {
            return;
        }

        // P1：embed 模块自动路由——embed 生成的模块（如 foo_tnr）直接生成 use 语句
        if self.embed_modules.contains(path_str.as_str()) {
            if u.is_from && u.items.len() == 1 && u.items[0] == "*" {
                self.emit_line(&format!("use {}::*;", path_str));
            } else if u.is_from && !u.items.is_empty() {
                self.emit_line(&format!("use {}::{{{}}};", path_str, u.items.join(", ")));
            } else {
                self.emit_line(&format!("use {};", path_str));
            }
            return;
        }

        // 非相对路径：检查是否为已知模块或已知模块的子路径
        let is_known = known_module_paths.contains(path_str.as_str());
        let parent_path = path_str.rsplitn(2, "::").nth(1).unwrap_or("");
        let parent_is_known = known_module_paths.contains(parent_path);
        let is_std_root = path_str == "std";
        if !is_known && !parent_is_known && !is_std_root {
            // P3：bridge registry 路由——非 embed、非已知 std 模块，尝试桥接
            if let Some(reg) = &self.bridge_registry {
                let result = reg.resolve_import_full(&path, &u.items);
                if !result.rust_path.is_empty() && result.rust_path != path_str {
                    if u.is_from && !u.items.is_empty() && u.items[0] != "*" {
                        self.emit_line(&format!(
                            "use {}::{{{}}};",
                            result.rust_path,
                            u.items.join(", ")
                        ));
                    } else {
                        self.emit_line(&format!("use {};", result.rust_path));
                    }
                    return;
                }
            }
            // 桥接也未匹配，跳过
            return;
        }

        if u.is_from {
            if u.items.is_empty() {
                if !known_module_paths.contains(path_str.as_str()) && path_str != "std" {
                    return;
                }
                self.emit_line(&format!("use {};", path_str));
            } else if u.items.len() == 1 && u.items[0] == "*" {
                if !known_module_paths.contains(path_str.as_str()) {
                    return;
                }
                self.emit_line(&format!("use {}::*;", path_str));
            } else {
                // 过滤掉内建函数和已在 prelude 中的项
                let items: Vec<String> = u
                    .items
                    .iter()
                    .filter(|item| !builtin_items.contains(item.as_str()))
                    .map(|item| {
                        lz_to_rust
                            .get(item.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| item.clone())
                    })
                    .filter(|rust_item| !prelude_items.contains(rust_item.as_str()))
                    .collect();
                if items.is_empty() {
                    return;
                }
                self.emit_line(&format!("use {}::{{{}}};", path_str, items.join(", ")));
            }
        } else {
            // import std.io → use std::io;
            // import std.math → 跳过（无 Rust 对应模块）
            // import std::cell::Cell → use std::cell::Cell;（叶为已知模块内具体项）
            let leaf_is_known_item = parent_is_known && !path.last().map_or(false, |s| s == "*");
            if !known_module_paths.contains(path_str.as_str())
                && path_str != "std"
                && !leaf_is_known_item
            {
                return;
            }
            if leaf_is_known_item {
                self.emit_line(&format!("use {};", path_str));
            } else {
                self.emit_line(&format!("use {};", path_str));
            }
        }
    }

    pub(crate) fn gen_const_def(&mut self, c: &ConstDef) {
        let is_mutated = self.mutated_consts.contains(&c.name);
        // 顶层 let→const 位的期望类型注入（对齐 gen_stmt 的 Let 分支）：
        // 改前这里从不把声明类型送进 current_expected_ty，于是 const 值里的
        // bigint 字面量（BUG-2 族）和嵌套空列表（BUG-5 族）都拿不到期望类型。
        let prev_expected_for_const = self.current_expected_ty.borrow().clone();
        *self.current_expected_ty.borrow_mut() = Some(c.ty.clone());
        // const 不支持 .to_string()，直接用 &str
        let (ty_str, val_str) = match &c.ty {
            IrType::Str => {
                if let ExprKind::Lit(LitKind::Str(s)) = &c.value.kind {
                    let escaped = s.escape_default().to_string();
                    ("&str".into(), format!("\"{}\"", escaped))
                } else {
                    (self.rust_type(&c.ty), self.gen_expr(&c.value))
                }
            }
            // BUG-2：BigInt 字面量直接生成 BigInt::from(...)，不依赖 gen_lit 的类型判断。
            // 分派条件用 is_bigint_ty（覆盖 `IrType::BigInt` 与注解形态
            // `Named{path:"bigint"/"BigInt"}`）——原来只匹配 `IrType::BigInt`，
            // 而 `let x: bigint = …` 在 IR 里落成 Named 形态，本臂从不命中，
            // 于是走 `_ =>` 的 gen_expr 分支发出裸 `…i128`（E0308）。
            _ty if is_bigint_ty(_ty) => {
                // 期望类型注入：BigInt 的算术/比较非常量可求值，顶层 let 会被
                // 发成 const ⇒ E0015，needs_lazy 已把它归入 LazyLock 惰性求值
                // （BUG-3），此处按运行期表达式生成。
                // 期望类型由函数开头的 prev_expected_for_const 统一注入
                let val = match &c.value.kind {
                    ExprKind::Lit(LitKind::Int128(n)) => bigint_from_code(*n),
                    ExprKind::Lit(LitKind::Int(n)) => {
                        format!("{BIGINT_RS}::from({n})")
                    }
                    ExprKind::Lit(LitKind::BigInt(s)) => bigint_lit_code(s),
                    _ => self.gen_expr(&c.value),
                };
                (self.rust_type(&c.ty), val)
            }
            _ => (self.rust_type(&c.ty), self.gen_expr(&c.value)),
        };
        *self.current_expected_ty.borrow_mut() = prev_expected_for_const;
        let kw = if is_mutated { "static mut" } else { "const" };
        // 需要使用 lhs!() 惰性初始化的情况：
        // 1. 集合类型（Vec, HashMap, HashSet）— 不能 const 初始化（需要 .to_string() 等）
        // 2. 包含 catch_unwind 等非 const 调用的值
        let needs_lazy = !is_mutated
            && (matches!(&c.ty,
                IrType::Named { path, .. }
                if ["Vec","List","HashMap","HashSet","Dict","Set"].contains(&path.as_str())
            ) || matches!(&c.ty, IrType::Tuple(_))
                // BUG-3：BigInt / Complex 的构造（`BigInt::from`）与算术运算符
                // 都不是 const fn ⇒ 顶层 let 发成 const 会撞 E0015
                // 「cannot call non-const fn in constants」，与集合类型同款走惰性静态
                || is_bigint_ty(&c.ty)
                || is_complex_ty(&c.ty)
                || val_str.contains("catch_unwind")
                || val_str.contains("__try_result")
                || val_str.contains("LazyLock")
                || val_str.contains(".to_string()")
                // T04：`@lazy_static` / `@once` 顶层静态 → LazyLock 惰性静态
                || c.mods.lazy);
        if needs_lazy {
            self.lazy_static_names.insert(c.name.clone());
            let lazy_ty = self.rust_type(&c.ty);
            let lazy_val = if val_str.contains("__gen_vec") {
                format!(
                    "{{ let mut __gen_vec: Vec<_> = Vec::new(); {}; __gen_vec }}",
                    val_str
                )
            } else {
                val_str.clone()
            };
            self.emit_line(&format!(
                "static {}: std::sync::LazyLock<{}> = std::sync::LazyLock::new(|| {});",
                c.name, lazy_ty, lazy_val
            ));
        } else {
            self.emit_line(&format!("{} {}: {} = {};", kw, c.name, ty_str, val_str));
        }
    }

    pub(crate) fn gen_type_alias_def(&mut self, ta: &TypeAliasDef) {
        // 泛型类型别名：type MaybeNode<T> = Option<Node<T>> → pub type MaybeNode<T> = ...
        let generics_s = if ta.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", ta.generics.join(", "))
        };
        self.emit_line(&format!(
            "pub type {}{} = {};",
            ta.name,
            generics_s,
            self.rust_type(&ta.ty)
        ));
    }

    pub(crate) fn gen_test_def(&mut self, t: &TestDef) {
        self.emit_line("#[test]");
        // 测试名可能含空格（如 "string concat"），需转换为合法 Rust 标识符
        let safe_name = sanitize_ident(&t.name);
        self.emit_line(&format!("fn {}() {{", safe_name));
        self.indent += 1;
        // 每个测试函数是独立作用域：清空已声明变量集合，避免前一个 test 的
        // setup 变量泄漏到本 test（否则被误判为已声明变量 → `base = ...` 赋值 E0425）
        let saved_declared = self.declared.clone();
        let saved_str = self.str_typed_vars.clone();
        self.gen_block_inner(&t.body);
        self.declared = saved_declared;
        self.str_typed_vars = saved_str;
        self.indent -= 1;
        self.emit_line("}");
    }

    /// duck 类型约束 → Rust trait
    pub(crate) fn gen_duck_def(&mut self, d: &DuckDef) {
        let generics = if d.generics.is_empty() {
            String::new()
        } else {
            let gs: Vec<String> = d.generics.iter().map(|g| g.name.clone()).collect();
            format!("<{}>", gs.join(", "))
        };
        // 多泛型关系 duck（有 owner 前缀方法）：方法给默认实现，
        // 由自动生成的 impl 按 owner 选择性覆写（编译期结构检查保证正确性）
        let has_owners = d.methods.iter().any(|m| m.owner.is_some());
        self.emit_line(&format!("pub trait {}{} {{", d.name, generics));
        self.indent += 1;
        // 关联类型约束（§2.3 `type I.Item`）→ Rust trait 关联类型声明
        for a in &d.assoc_types {
            self.emit_line(&format!("type {};", a.name));
        }
        // 字段约束 → 生成 accessor 方法
        for f in &d.fields {
            // 关系字段（A.id == B.id / A.name: B.name）：无显式类型，
            // 用关联类型表达「两侧类型相等」（§2.2），impl 时由具体类型指定
            if f.rel.is_some() {
                self.emit_line(&format!("type __Field_{};", f.name));
                self.emit_line(&format!(
                    "fn __field_{}(&self) -> &Self::__Field_{} {{ unimplemented!() }}",
                    f.name, f.name
                ));
                continue;
            }
            let rt = self.rust_type(&f.ty);
            if has_owners || f.owner.is_some() {
                self.emit_line(&format!(
                    "fn __field_{}(&self) -> &{} {{ unimplemented!() }}",
                    f.name, rt
                ));
            } else {
                self.emit_line(&format!("fn __field_{}(&self) -> &{};", f.name, rt));
            }
        }
        // 方法签名
        for m in &d.methods {
            let params: Vec<String> = m
                .params
                .iter()
                .map(|p| {
                    if p.name == "self" {
                        if p.is_mut {
                            "&mut self".to_string()
                        } else {
                            "&self".to_string()
                        } // LZ 默认即引用
                    } else {
                        format!("{}: {}", p.name, self.duck_sig_type(&p.ty, d))
                    }
                })
                .collect();
            let ret = self.duck_sig_type(&m.ret_ty, d);
            if has_owners {
                self.emit_line(&format!(
                    "fn {}({}) -> {} {{ unimplemented!() }}",
                    m.name,
                    params.join(", "),
                    ret
                ));
            } else {
                self.emit_line(&format!("fn {}({}) -> {};", m.name, params.join(", "), ret));
            }
        }
        // PhantomData 占位方法：确保所有 duck 泛型参数被 trait 使用（避免 E0392）
        if !d.generics.is_empty() {
            let gs: Vec<String> = d.generics.iter().map(|g| g.name.clone()).collect();
            self.emit_line(&format!(
                "fn _lz_duck_phantom(&self) -> std::marker::PhantomData<({})> {{ std::marker::PhantomData }}",
                gs.join(", ")
            ));
        }
        self.indent -= 1;
        self.emit_line("}");
    }

    /// 自动生成 duck 结构匹配的 Rust impl：
    /// 对每个在调用点被用作 duck 约束实参的具体类型，生成 `impl Duck<...> for Type<...> { ... }`，
    /// 方法体委托到该类型自己的同名方法（结构匹配 → 运行时零开销）。
    /// 支持多泛型关系 duck（Mapper<T,R>）与泛型具体类型（Wrapper<T>）：
    /// 通过 duck 方法签名与具体类型方法签名的 unify，反推 duck 泛型参数 → 具体类型的绑定。
    /// 顶层 self-def → 按结构体分组发射 impl 块（BUG-CG-002/TY-002，E0568 修复）。
    /// `def m(self: S, ...)` 发射为 `impl S { fn m(&self, ...) }`，
    /// 方法体生成复用 gen_fn_def（is_method 路径渲染 &self / &mut self）。
    /// mut self 标记：self_p.is_mut 已在归属判定时登记，此处透传——
    /// gen_fn_def 的 self 参数渲染逻辑按 p.is_mut 生成 &mut self。
    pub(crate) fn gen_self_fn_impls(&mut self, module: &IrModule) {
        if self.self_fns.is_empty() {
            return;
        }
        // 按 struct 分组保持方法序稳定（源码顺序）
        let mut groups: Vec<(String, Vec<&FnDef>)> = Vec::new();
        for item in &module.items {
            if let Item::FnDef(f) = item {
                if let Some((sname, _)) = self.self_fns.get(&f.name) {
                    if let Some(g) = groups.iter_mut().find(|(s, _)| s == sname) {
                        g.1.push(f);
                    } else {
                        groups.push((sname.clone(), vec![f]));
                    }
                }
            }
        }
        for (sname, fns) in groups {
            self.emit_line(&format!("impl {} {{", sname));
            self.indent += 1;
            for f in fns {
                self.gen_fn_def(f);
                self.buf.push('\n');
            }
            self.indent -= 1;
            self.emit_line("}");
            self.buf.push('\n');
        }
    }

    pub(crate) fn gen_duck_auto_impls(&mut self, module: &IrModule) {
        let pairs = crate::ir::duck_check::collect_duck_impls(module);
        if pairs.is_empty() {
            return;
        }
        // 索引 duck 定义与具体类型定义
        let mut duck_defs: HashMap<&str, &DuckDef> = HashMap::new();
        let mut struct_defs: HashMap<&str, &StructDef> = HashMap::new();
        for item in &module.items {
            match item {
                Item::DuckDef(d) => {
                    duck_defs.insert(d.name.as_str(), d);
                }
                Item::StructDef(s) => {
                    struct_defs.insert(s.name.as_str(), s);
                }
                _ => {}
            }
        }
        let mut emitted: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (type_name, duck_name, initial_bindings) in &pairs {
            let Some(duck) = duck_defs.get(duck_name.as_str()) else {
                continue;
            };
            let Some(sdef) = struct_defs.get(type_name.as_str()) else {
                continue;
            };
            // 同一 (类型, duck) 只生成一份泛型 impl（不同调用点泛型实参由 Rust 推断）
            let dedup = format!("{}::{}", type_name, duck_name);
            if !emitted.insert(dedup) {
                continue;
            }
            // 反推 duck 泛型参数 → 具体类型表达式（调用点绑定 + 方法签名 unify 补全）
            let Some(subst) =
                crate::ir::duck_check::infer_duck_bindings(duck, sdef, initial_bindings)
            else {
                continue;
            };
            // 具体类型自身的泛型参数名（如 Wrapper 的 T）
            let concrete_generics: Vec<String> =
                sdef.generics.iter().map(|g| g.name.clone()).collect();
            // impl 目标类型表达式：TypeName<T1, T2>
            let self_ir = if concrete_generics.is_empty() {
                IrType::named(type_name)
            } else {
                IrType::named_with(
                    type_name,
                    concrete_generics
                        .iter()
                        .map(|n| IrType::Generic(n.clone()))
                        .collect(),
                )
            };
            let self_str = self.rust_type(&self_ir);
            // duck 泛型参数名（供 trait 泛型实参顺序）
            let duck_names: Vec<String> = duck.generics.iter().map(|g| g.name.clone()).collect();
            // trait 泛型实参（按 duck 泛型参数顺序）
            let trait_args: Vec<String> = duck_names
                .iter()
                .map(|n| self.rust_type(&subst[n]))
                .collect();
            // impl 泛型参数：与具体类型定义一致（Clone + Debug bound）
            let impl_generics = if concrete_generics.is_empty() {
                String::new()
            } else {
                let params: Vec<String> = sdef
                    .generics
                    .iter()
                    .map(|g| {
                        if g.bounds.is_empty() {
                            format!("{}: Clone + std::fmt::Debug", g.name)
                        } else {
                            let bounds: Vec<String> =
                                g.bounds.iter().map(|b| self.rust_type(b)).collect();
                            format!(
                                "{}: Clone + std::fmt::Debug + {}",
                                g.name,
                                bounds.join(" + ")
                            )
                        }
                    })
                    .collect();
                format!("<{}>", params.join(", "))
            };
            self.emit_line(&format!(
                "impl{} {}{} for {} {{",
                impl_generics,
                duck.name,
                if trait_args.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", trait_args.join(", "))
                },
                self_str
            ));
            self.indent += 1;
            // 关联类型绑定（§2.3 `type I.Item`）：trait 声明了关联类型，
            // impl 必须提供具体值。推断：具体类型第一个泛型参数，否则 Any→i64。
            for a in &duck.assoc_types {
                let belongs = match &a.owner {
                    None => true,
                    Some(g) => {
                        matches!(subst.get(g), Some(IrType::Named { path, .. }) if path == type_name)
                    }
                };
                if !belongs {
                    continue;
                }
                // 具体类型第一个泛型参数（如 MyIter<T> 的 T）作为关联类型值
                let assoc_ty = concrete_generics
                    .first()
                    .map(|n| IrType::Generic(n.clone()))
                    .unwrap_or_else(|| IrType::Any);
                let rt = self.rust_type(&assoc_ty);
                self.emit_line(&format!("type {} = {};", a.name, rt));
            }
            // 字段约束 → 直接访问字段（只生成属于本类型的字段约束）
            for f in &duck.fields {
                let belongs = match &f.owner {
                    None => true,
                    Some(g) => {
                        matches!(subst.get(g), Some(IrType::Named { path, .. }) if path == type_name)
                    }
                };
                if !belongs {
                    continue;
                }
                // 关系字段（A.id == B.id）：trait 用关联类型 __Field_x，
                // impl 需绑定关联类型 = 具体类型该字段的实际类型，并覆写 accessor
                if f.rel.is_some() {
                    // 找到具体类型中同名字段的类型
                    let field_ty = sdef
                        .fields
                        .iter()
                        .find(|sf| sf.name == f.name)
                        .map(|sf| sf.ty.clone())
                        .unwrap_or_else(|| IrType::Any);
                    let rt = self.rust_type(&field_ty);
                    self.emit_line(&format!("type __Field_{} = {};", f.name, rt));
                    self.emit_line(&format!(
                        "fn __field_{}(&self) -> &Self::__Field_{} {{",
                        f.name, f.name
                    ));
                    self.indent += 1;
                    self.emit_line(&format!("&self.{}", f.name));
                    self.indent -= 1;
                    self.emit_line("}");
                    continue;
                }
                let fty = crate::ir::duck_check::substitute(&f.ty, &subst);
                let rt = self.rust_type(&fty);
                self.emit_line(&format!("fn __field_{}(&self) -> &{} {{", f.name, rt));
                self.indent += 1;
                self.emit_line(&format!("&self.{}", f.name));
                self.indent -= 1;
                self.emit_line("}");
            }
            // 方法约束 → 委托到具体类型的同名方法（只生成属于本类型的约束）
            for m in &duck.methods {
                let belongs = match &m.owner {
                    None => true,
                    Some(g) => {
                        matches!(subst.get(g), Some(IrType::Named { path, .. }) if path == type_name)
                    }
                };
                if !belongs {
                    continue;
                }
                let params: Vec<String> = m
                    .params
                    .iter()
                    .map(|p| {
                        if p.name == "self" {
                            if p.is_mut {
                                "&mut self".to_string()
                            } else {
                                "&self".to_string()
                            }
                        } else {
                            // 先替换 duck 泛型引用（R→Fahrenheit），再处理关联类型引用
                            // （I.Item → Self::Item），保证 impl 签名类型均有定义
                            let ty = crate::ir::duck_check::substitute(&p.ty, &subst);
                            format!("{}: {}", p.name, self.duck_sig_type(&ty, duck))
                        }
                    })
                    .collect();
                let args: Vec<String> = m
                    .params
                    .iter()
                    .map(|p| {
                        if p.name == "self" {
                            "self".to_string()
                        } else {
                            p.name.clone()
                        }
                    })
                    .collect();
                let ret = if m.ret_ty == IrType::Unit {
                    String::new()
                } else {
                    let ty = crate::ir::duck_check::substitute(&m.ret_ty, &subst);
                    format!(" -> {}", self.duck_sig_type(&ty, duck))
                };
                self.emit_line(&format!("fn {}({}){} {{", m.name, params.join(", "), ret));
                self.indent += 1;
                self.emit_line(&format!("{}::{}({})", sdef.name, m.name, args.join(", ")));
                self.indent -= 1;
                self.emit_line("}");
            }
            self.indent -= 1;
            self.emit_line("}");
            self.buf.push('\n');
        }
    }
}
