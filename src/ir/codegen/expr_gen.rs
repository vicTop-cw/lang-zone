// Lang-Zone 编译器 — ir/codegen/expr_gen.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::scan::cmp_walk_block;
use super::scan::count_vars_block;
use super::scan::CmpInfo;
use super::types_emit::bigint_from_code;
use super::types_emit::bigint_lit_code;
use super::types_emit::is_bigint_ty;
use super::types_emit::is_complex_ty;
use super::types_emit::BIGINT_RS;
use super::types_emit::COMPLEX_RS;
use super::*;

impl CodeGen {
    /// 比较约束传播：预扫描模块内所有 FnDef 体（含 impl/trait 默认方法），
    /// 经调用图不动点计算「参与比较运算」的泛型集合，存入 `self.fn_cmp`。
    /// 调用图传播：若 F 调用 G（G 比较其泛型 → 需 Eq/Ord），且调用实参类型中
    /// 含 F 的泛型，则 F 的该泛型也需 Eq/Ord（如 merge_sort→merge、
    /// unique→内置 contains）。内置比较函数（contains/index）按 builtin_cmp 处理。
    pub(crate) fn compute_cmp_constraints(&mut self, module: &IrModule) {
        // 内置比较函数：调用点要求「流动泛型」具备 Eq（PartialEq + Eq）
        let builtin_cmp: HashMap<String, (bool, bool)> = {
            let mut m = HashMap::new();
            m.insert("contains".to_string(), (true, false));
            m.insert("index".to_string(), (true, false));
            m
        };
        // 汇总所有 FnDef（含 impl/trait 默认方法）的 (名称, 泛型, 函数体)
        let mut fns: Vec<(String, Vec<String>, Block)> = Vec::new();
        for item in &module.items {
            match item {
                Item::FnDef(f) => fns.push((
                    f.name.clone(),
                    f.generics.iter().map(|g| g.name.clone()).collect(),
                    f.body.clone(),
                )),
                Item::Impl(i) => {
                    for m in &i.methods {
                        fns.push((
                            m.name.clone(),
                            m.generics.iter().map(|g| g.name.clone()).collect(),
                            m.body.clone(),
                        ));
                    }
                }
                Item::TraitDef(t) => {
                    for m in &t.methods {
                        if let Some(body) = &m.body {
                            fns.push((
                                m.name.clone(),
                                m.generics.iter().map(|g| g.name.clone()).collect(),
                                body.clone(),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        // 直接比较集合 + 调用点
        let mut direct: HashMap<String, (HashSet<String>, HashSet<String>)> = HashMap::new();
        let mut calls: HashMap<String, Vec<(String, HashSet<String>)>> = HashMap::new();
        for (name, my_gen, body) in &fns {
            let my_gen_set: HashSet<String> = my_gen.iter().cloned().collect();
            let mut info = CmpInfo::default();
            cmp_walk_block(body, &my_gen_set, &mut info);
            direct.insert(name.clone(), (info.eq, info.ord));
            calls.insert(name.clone(), info.calls);
        }
        // 初始 fn_cmp = 直接集合
        let mut fn_cmp: HashMap<String, (HashSet<String>, HashSet<String>)> = direct;
        // 调用图不动点传播
        loop {
            let mut pending: Vec<(String, HashSet<String>, bool, bool)> = Vec::new();
            for (fname, calllist) in &calls {
                for (callee, flow) in calllist {
                    let (eqf, ordf) = if let Some((eq, ord)) = fn_cmp.get(callee) {
                        (!eq.is_empty(), !ord.is_empty())
                    } else if let Some((eq, ord)) = builtin_cmp.get(callee) {
                        (*eq, *ord)
                    } else {
                        (false, false)
                    };
                    if eqf || ordf {
                        pending.push((fname.clone(), flow.clone(), eqf, ordf));
                    }
                }
            }
            let mut changed = false;
            for (fname, flow, eqf, ordf) in pending {
                if let Some((feq, ford)) = fn_cmp.get_mut(&fname) {
                    for g in flow {
                        if eqf && feq.insert(g.clone()) {
                            changed = true;
                        }
                        if ordf && ford.insert(g) {
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        self.fn_cmp = fn_cmp;
    }

    /// 变量引用计数：预计算各函数（含 impl/trait 默认方法）体内变量引用次数，
    /// 存入 `self.fn_use_count`，供 clone_if_multiuse 按函数名查表，实现 move
    /// 语义修复（被多次使用的非 Copy 变量在调用实参处克隆，避免 E0382）。
    pub(crate) fn compute_use_counts(&mut self, module: &IrModule) {
        for item in &module.items {
            match item {
                Item::FnDef(f) => {
                    let mut uc = HashMap::new();
                    count_vars_block(&f.body, &mut uc);
                    self.fn_use_count.insert(f.name.clone(), uc);
                }
                Item::Impl(i) => {
                    for m in &i.methods {
                        let mut uc = HashMap::new();
                        count_vars_block(&m.body, &mut uc);
                        self.fn_use_count.insert(m.name.clone(), uc);
                    }
                }
                Item::TraitDef(t) => {
                    for m in &t.methods {
                        if let Some(body) = &m.body {
                            let mut uc = HashMap::new();
                            count_vars_block(body, &mut uc);
                            self.fn_use_count.insert(m.name.clone(), uc);
                        }
                    }
                }
                Item::StructDef(s) => {
                    for m in &s.methods {
                        let mut uc = HashMap::new();
                        count_vars_block(&m.body, &mut uc);
                        self.fn_use_count.insert(m.name.clone(), uc);
                    }
                }
                _ => {}
            }
        }
    }

    /// LZ fn 值（载体 `Arc<dyn Fn … + Send + Sync>`）不实现 `Fn`/`FnMut`——std 只为 `Box<dyn Fn>` 提供
    /// 这些 impl。把它作为实参传给 `impl Fn(…) + 'static` 形参或迭代器 `FnMut` 方法时，
    /// 套一层持有 Arc clone 的转发闭包：既满足 bound，又不 move 原变量（LZ fn 值是值语义，
    /// 否则 E0382 use of moved value）。参数个数：Var 实参查 fn_value_carriers 登记表（IR 常把
    /// 偏应用变量标成非 Fn），调用/块实参取自身 `IrType::Fn` 的参数表。
    pub(crate) fn fn_value_fwd(&self, s: &str, a: &Expr) -> Option<String> {
        let arity = match &a.kind {
            ExprKind::Var(name) => self.fn_value_carriers.get(name.as_str()).copied(),
            ExprKind::Call { .. } | ExprKind::BlockExpr { .. } => match &a.ty {
                IrType::Fn { params, .. } => Some(params.len()),
                _ => None,
            },
            _ => None,
        };
        let arity = arity?;
        let fwd: Vec<String> = (0..arity).map(|k| format!("__lz_fv{}", k)).collect();
        Some(format!(
            "{{ let __lz_fv = std::sync::Arc::clone(&{}); move |{}| __lz_fv({}) }}",
            s,
            fwd.join(", "),
            fwd.join(", ")
        ))
    }

    /// move 语义修复：若实参为「被多次使用的非 Copy 变量」，自动 .clone()，
    /// 避免 `for x in xs: push(seen, x); push(result, x)` 类的二次移动 E0382。
    /// Copy 类型克隆无害（i64.clone() 等价），此处统一克隆以保证安全。
    pub(crate) fn clone_if_multiuse(&self, s: String, a: &Expr) -> String {
        if s.contains(".clone()") {
            return s;
        }
        // &str 类型变量需要 to_string() 而非 clone()
        if matches!(&a.ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str))
            || matches!(&a.ty, IrType::Str)
        {
            if let ExprKind::Var(name) = &a.kind {
                if let Some(cnt) = self
                    .fn_use_count
                    .get(&self.cur_fn_name)
                    .and_then(|m| m.get(name))
                {
                    if *cnt > 1 {
                        return format!("{}.to_string()", s);
                    }
                }
            }
        }
        if let ExprKind::Var(name) = &a.kind {
            if let Some(cnt) = self
                .fn_use_count
                .get(&self.cur_fn_name)
                .and_then(|m| m.get(name))
            {
                if *cnt > 1 {
                    return format!("{}.clone()", s);
                }
            }
        }
        s
    }

    /// 类型中是否含未解析的关联类型路径（`Vec<I::Item>` 中 I 不在当前作用域，
    /// 如 main 里引用 collect_list 的泛型参数 I → E0433 cannot find type `I`）。
    /// 有此类路径时跳过变量类型标注，让 Rust 从右侧推断。
    /// 类型中是否含未解析的关联类型路径（`Vec<I::Item>` 中 I 不在当前作用域，
    /// 如 main 里引用 collect_list 的泛型参数 I → E0433 cannot find type `I`）。
    /// 有此类路径时跳过变量类型标注，让 Rust 从右侧推断。
    pub(crate) fn has_unbound_named(&self, ty: &IrType) -> bool {
        match ty {
            IrType::Named { path, args } if args.is_empty() => {
                !self.known_types.contains(path.as_str())
                    && !self.emitted_types.contains(path.as_str())
                    && !self.top_level_static_names.contains(path.as_str())
                    && !self.impl_types.contains(path.as_str())
                    && path != "Option"
                    && path != "Result"
                    && path != "String"
                    && path != "List"
                    && path != "Dict"
                    && path != "Set"
                    && path != "Vec"
                    && path != "HashMap"
                    && path != "HashSet"
            }
            _ => false,
        }
    }

    pub(crate) fn has_unresolved_dotted_assoc(&self, ty: &IrType) -> bool {
        match ty {
            IrType::Named { path, args } => {
                if let Some((owner, _)) = path.split_once('.') {
                    // Self.Item 在 impl 中合法（Self 关键字）；其余点号路径的 owner
                    // 必须在作用域内（已知类型/已声明变量），否则无法解析
                    if owner != "Self"
                        && !self.known_types.contains(owner)
                        && !self.emitted_types.contains(owner)
                        && !self.impl_types.contains(owner)
                        && !self.top_level_static_names.contains(owner)
                        && !self.declared.contains(owner)
                        && !self.param_renames.contains_key(owner)
                    {
                        return true;
                    }
                }
                args.iter().any(|a| self.has_unresolved_dotted_assoc(a))
            }
            IrType::Option(inner) => self.has_unresolved_dotted_assoc(inner),
            IrType::Result { ok, err } => {
                self.has_unresolved_dotted_assoc(ok) || self.has_unresolved_dotted_assoc(err)
            }
            IrType::Tuple(items) => items.iter().any(|i| self.has_unresolved_dotted_assoc(i)),
            IrType::Ref(inner) | IrType::MutRef(inner) => self.has_unresolved_dotted_assoc(inner),
            _ => false,
        }
    }

    pub(crate) fn gen_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Lit(lit) => self.gen_lit(lit, &expr.ty),
            ExprKind::Var(name) => {
                if name == "pass" {
                    "()".into()
                } else if let Some((sym, init)) = self.lazy_bindings.get(name.as_str()) {
                    // @lazy 延迟绑定（T04/AC4）：首次访问 `get_or_init` 求值一次，
                    // 之后返回缓存（`OnceCell::get_or_init` / `OnceLock::get_or_init` 接收 &self）。
                    format!("*{}.get_or_init(|| {})", sym, init)
                } else if self.downgraded_vars.contains(name.as_str()) {
                    format!("{}_", name)
                } else if self.global_vars.contains_key(name.as_str()) {
                    format!("unsafe {{ {} }}", name)
                } else if self.mutated_consts.contains(name) {
                    format!("unsafe {{ {} }}", name)
                } else if let Some(renamed) = self.param_renames.get(name) {
                    renamed.clone()
                } else if let Some(enum_name) = self.enum_variants.get(name.as_str()) {
                    // 裸枚举变体名作为表达式（`return Less`）：生成完整路径 Ordering::Less，
                    // 否则 Rust 报 E0425 cannot find value `Less`
                    format!("{}::{}", enum_name, name)
                } else if name.contains('.') && !self.downgraded_vars.contains(name.as_str()) {
                    // 关联类型路径表达式（iter.lz `I.Item.default()`）：
                    // `I.Item` 需转成 `I::Item`（泛型参数上的关联类型用 ::，E0423）
                    name.replace('.', "::")
                } else if self.lazy_static_names.contains(name.as_str()) {
                    // 模块级 LazyLock 静态集合：表达式访问需解引用 + clone，
                    // 否则 `config.and_then(...)` 报 E0507（cannot move out of dereference）
                    format!("(*{}).clone()", name)
                } else if self.slice_clone_bindings.contains(name.as_str()) {
                    // type-pack 切片模式绑定（03d §2.8 方案 B）：臂体内引用 a
                    // 需 a.clone()（a 绑定 &Ts，返回/使用需 owned Ts，E0308 修复）
                    format!("{}.clone()", name)
                } else if matches!(&expr.ty, IrType::Fn { .. })
                    && self.top_level_fns.contains(name.as_str())
                {
                    // IR-003：顶层 def 作一等值 → 装箱为 Arc<dyn Fn … + Send + Sync>（可 clone，
                    // Box<dyn Fn> 不实现 Clone → E0599）。
                    // 局部 fn-let 已在 let 值位置装箱，不重复装箱（避免双层 Rc）
                    format!("Arc::new({})", name)
                } else {
                    name.clone()
                }
            }
            ExprKind::Call {
                callee,
                args,
                type_args,
            } => {
                // ord(s[i])：字符串单字符下标已按 char 码点 i64 生成（v165 语义），
                // 再包 lz_builtins::ord(char) 会 E0308 expected char, found i64。
                // 此时 ord 为恒等：直接发射下标表达式（lib_hashmap._str_hash 实测）。
                if let ExprKind::Var(name) = &callee.kind {
                    if name == "ord" && args.len() == 1 {
                        if let ExprKind::IndexGet { base, key } = &args[0].kind {
                            if matches!(base.ty, IrType::Str) {
                                // ord(s[i]) → char as i64；
                                // 手动生成 char 索引（不走 IndexGet 的 .to_string() 路径）
                                let base_s = self.gen_expr(base);
                                let key_s = self.gen_expr(key);
                                return format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = (({}) as usize); if __i >= __cs.len() {{ '\\0' as i64 }} else {{ __cs[__i] as i64 }}}}", base_s, key_s);
                            }
                        }
                    }
                }
                // 构造器调用 Rc(...)/Box(...)/Arc(...) → Rc::new(...)/Box::new(...)/Arc::new(...)
                // （BUG-CONSTRUCTOR 修复，box.lz / magic_methods.lz）：直接调用语法与 MethodCall
                // 路径 `Rc.new(x)` → `Rc::new(x)` 对称。否则 Rc 被当作未知外部函数生成
                // `fn Rc(__a0: i64) -> i64 { i64::MAX }` 桩，导致编译失败。
                if let ExprKind::Var(cname) = &callee.kind {
                    // 仅当 Rc/Box/Arc 不是本模块用户自定义类型（struct/enum）时才路由到
                    // std 的 `::new`。否则会误伤 box.lz 等自定义 `struct Box/Rc/Arc`——
                    // 其构造应走已有的 `Name::__new__` 路由（见 13586-13595），而非 `Box::new`
                    // （用户 struct 没有 std 的 `new` 关联函数，E0599）。
                    if (cname == "Rc" || cname == "Box" || cname == "Arc")
                        && !self.is_known_type(cname)
                    {
                        let ctor_args: Vec<String> =
                            args.iter().map(|a| self.gen_expr(a)).collect();
                        return format!("{}::new({})", cname, ctor_args.join(", "));
                    }
                }
                // std 模块链调用拦截（BUG-SB-001 修复，2026-09-03）：
                // `time.Duration.fromMillis(1500)` 的 IR 是 Call {
                //   callee: FieldAccess { base: FieldAccess { base: Var("time"), field: "Duration" },
                //                         field: "fromMillis" } } →
                //   生成 `std::time::Duration::from_millis(1500)`。
                if let ExprKind::FieldAccess {
                    base,
                    field: method,
                } = &callee.kind
                {
                    if let ExprKind::FieldAccess {
                        base: mod_base,
                        field: type_name,
                    } = &base.kind
                    {
                        if let ExprKind::Var(module) = &mod_base.kind {
                            const STD_MODULES: &[(&str, &[&str])] = &[
                                ("time", &["Duration", "Instant"]),
                                ("thread", &["Thread", "JoinHandle"]),
                            ];
                            if let Some((rust_mod, types)) = STD_MODULES
                                .iter()
                                .find(|(lz_name, _)| *lz_name == module.as_str())
                            {
                                if types.contains(&type_name.as_str()) {
                                    let rust_method = match method.as_str() {
                                        "fromMillis" => "from_millis",
                                        "toMillis" => "as_millis",
                                        "fromSecs" | "fromSeconds" => "from_secs",
                                        "fromMicros" => "from_micros",
                                        "fromNanos" => "from_nanos",
                                        _ => method.as_str(),
                                    };
                                    let args_s: Vec<String> = args
                                        .iter()
                                        .map(|a| {
                                            let s = self.gen_expr(a);
                                            // Duration 构造函数参数是 u64：
                                            // `1500i64` → `1500u64`（E0308 修复）
                                            if type_name == "Duration"
                                                && matches!(a.ty, IrType::Int)
                                            {
                                                s.trim_end_matches("i64").to_string() + "u64"
                                            } else {
                                                s
                                            }
                                        })
                                        .collect();
                                    return format!(
                                        "std::{}::{}::{}({})",
                                        rust_mod,
                                        type_name,
                                        rust_method,
                                        args_s.join(", ")
                                    );
                                }
                            }
                        }
                    }
                }
                // 顶层 self-def 调用改写（BUG-CG-002/TY-002，E0568 修复）：
                // `inc(c)` → `c.inc()`；`get_count(c, 1)` → `c.get_count(1)`。
                // 方法语法是借用调用（&self/&mut self），不走值语义自动 clone。
                // 需在通用 callee 处理（args_s 值语义 clone 注入）之前拦截。
                // （~: 元组解包叠加 self-def 调用属极端边缘组合，不支持）
                if let ExprKind::Var(name) = &callee.kind {
                    if self.self_fns.contains_key(name) && !args.is_empty() {
                        let recv_s = self.gen_expr(&args[0]);
                        let mut rest: Vec<String> =
                            args[1..].iter().map(|a| self.gen_expr(a)).collect();
                        // 默认参数处理：检查方法是否有默认参数，如有则补 None
                        if let Some(&(total_params, def_count)) = self.fn_param_info.get(name) {
                            let required = total_params - def_count;
                            if rest.len() < required {
                                while rest.len() < required {
                                    rest.push("/* missing arg */".to_string());
                                }
                            }
                            while rest.len() < total_params {
                                rest.push("None".to_string());
                            }
                        }
                        return if rest.is_empty() {
                            format!("{}.{}()", recv_s, name)
                        } else {
                            format!("{}.{}({})", recv_s, name, rest.join(", "))
                        };
                    }
                }
                let callee_s = self.gen_expr(callee);
                // 如果 callee 是 Lambda（立即调用闭包），需要用括号包裹
                // move || { body }() → (move || { body })()
                let callee_s = if matches!(&callee.kind, ExprKind::Lambda { .. }) {
                    format!("({})", callee_s)
                } else {
                    callee_s
                };
                // 函数重载分派：根据实参类型选择对应的 mangled 版本
                let callee_s = if let ExprKind::Var(name) = &callee.kind {
                    if let Some(sigs) = self.overload_sigs.get(name) {
                        if sigs.len() > 1 {
                            // 从实参 IR 类型匹配签名
                            if let Some(sel) = self.match_overload(name, sigs, args) {
                                sel
                            } else {
                                callee_s
                            }
                        } else {
                            callee_s
                        }
                    } else {
                        callee_s
                    }
                } else {
                    callee_s
                };
                // 函数参数调用（iter.lz `predicate(item)`，predicate: fn(ref I.Item) -> bool）：
                // callee 是 Fn 类型变量且其参数是 ref，实参自动取引用（&item），
                // 否则 E0308 expected &<I as IntoIterator>::Item, found associated type
                let callee_fn_refs: Option<Vec<bool>> = match &callee.ty {
                    // callee 是 fn 类型表达式（Var 或 self.pred.clone() 等字段访问）：
                    // 参数是 ref 时实参自动取引用（&item），否则 E0308 expected
                    // &Item, found Item（iter.lz find / traits.lz Filter 的 predicate(item)）
                    IrType::Fn { params, .. } => Some(
                        params
                            .iter()
                            .map(|p| matches!(p, IrType::Ref(_) | IrType::MutRef(_)))
                            .collect(),
                    ),
                    _ => None,
                };

                // 检测 ~: 元组解包模式：连续的 UnpackBuildCall 参数
                let has_unpack = args.iter().any(|a| {
                    matches!(
                        &a.kind,
                        ExprKind::MagicCall {
                            kind: MagicKind::UnpackBuildCall,
                            ..
                        }
                    )
                });

                // 收集 unpack 的 packed 表达式和索引
                let (unpack_packed, unpack_indices): (Option<String>, Vec<String>) = if has_unpack {
                    let mut packed_s = String::new();
                    let mut idx_list = Vec::new();
                    for a in args.iter() {
                        if let ExprKind::MagicCall {
                            kind: MagicKind::UnpackBuildCall,
                            args: ua,
                        } = &a.kind
                        {
                            if ua.len() >= 2 {
                                if packed_s.is_empty() {
                                    packed_s = self.gen_expr(&ua[0]);
                                }
                                // 元组索引必须是裸整数（无类型后缀）
                                match &ua[1].kind {
                                    ExprKind::Lit(LitKind::Int(n)) => idx_list.push(n.to_string()),
                                    _ => idx_list.push(self.gen_expr(&ua[1])),
                                }
                            }
                        }
                    }
                    (Some(packed_s), idx_list)
                } else {
                    (None, Vec::new())
                };

                let mut args_s: Vec<String> = if has_unpack {
                    // 为所有 unpack 参数生成 __t.0, __t.1 等引用
                    let mut result_args: Vec<String> = Vec::new();
                    let mut idx_iter = unpack_indices.iter();
                    for a in args.iter() {
                        if matches!(
                            &a.kind,
                            ExprKind::MagicCall {
                                kind: MagicKind::UnpackBuildCall,
                                ..
                            }
                        ) {
                            if let Some(idx) = idx_iter.next() {
                                result_args.push(format!("__t.{}", idx));
                            } else {
                                result_args.push(self.gen_expr(a));
                            }
                        } else {
                            result_args.push(self.gen_expr(a));
                        }
                    }
                    result_args
                } else {
                    // 实参位置期望类型注入：callee 为具名函数且 fn_param_types 已知时，
                    // 生成实参时设置 current_expected_ty（Option::None 等跟随实参类型）
                    let callee_expected: Option<Vec<IrType>> = match &callee.kind {
                        ExprKind::Var(n) => self.fn_param_types.get(n).cloned(),
                        _ => None,
                    };
                    let prev_expected = self.current_expected_ty.borrow().clone();
                    let out = args
                        .iter()
                        .enumerate()
                        .map(|(i, a)| {
                            *self.current_expected_ty.borrow_mut() =
                                callee_expected.as_ref().and_then(|pts| pts.get(i).cloned());
                            let s = self.gen_expr(a);
                            // fn 值实参（Arc 载体）→ 转发闭包（见 fn_value_fwd）
                            if let Some(fwd) = self.fn_value_fwd(&s, a) {
                                return fwd;
                            }
                            // fn(...) 类型形参接收 lambda 实参：需转 fn 指针。参数声明
                            // 生成 impl FnMut（可收闭包），但存入 fn 字段/传给 fn 形参时
                            // opaque impl 无法 .clone()/赋值（E0599/E0308，lib_iterator
                            // MapIter.new(f)）。无捕获闭包可 `as fn(...)` 强转。
                            if let Some(expected_ty) = callee_expected.as_ref().and_then(|pts| pts.get(i)) {
                                // 检测形参是否为 `&fn(...)` 引用类型：若是，命名函数需 `&(f as fn(...))`
                                // 而非 `&(f) as fn(...)`（后者 `&` 优先级高于 `as`，会编译失败）
                                let is_ref_fn = matches!(expected_ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Fn { .. }));
                                if let IrType::Fn { params, ret } = expected_ty {
                                    if matches!(&a.kind, ExprKind::Lambda { .. }) {
                                        let ps: Vec<String> =
                                            params.iter().map(|pt| self.rust_type(pt)).collect();
                                        let type_text =
                                            format!("fn({}) -> {}", ps.join(", "), self.rust_type(ret));
                                        let chars: Vec<char> = type_text.chars().collect();
                                        let has_placeholder = chars.iter().enumerate().any(|(i, c)| {
                                            c.is_ascii_alphabetic()
                                                && (i == 0 || !chars[i - 1].is_ascii_alphanumeric())
                                                && (i + 1 >= chars.len()
                                                    || !chars[i + 1].is_ascii_alphanumeric())
                                        });
                                        if !has_placeholder {
                                            return format!("({}) as {}", s, type_text);
                                        }
                                    } else if matches!(&a.kind, ExprKind::Var(_)) {
                                        // 命名函数（如 `add`）传给 fn 形参：fn item 需强转为 fn pointer
                                        // 仅对 fn_param_types 中登记的顶层函数做此转换（函数参数变量如 `f` 已是 fn pointer，跳过）
                                        if let ExprKind::Var(name) = &a.kind {
                                            if self.fn_param_types.contains_key(name) {
                                                let ps: Vec<String> =
                                                    params.iter().map(|pt| self.rust_type(pt)).collect();
                                                let type_text =
                                                    format!("fn({}) -> {}", ps.join(", "), self.rust_type(ret));
                                                if is_ref_fn {
                                                    return format!("&({} as {})", s, type_text);
                                                } else {
                                                    return format!("({} as {})", s, type_text);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            s
                        })
                        .collect();
                    *self.current_expected_ty.borrow_mut() = prev_expected;
                    out
                };
                // 函数参数调用（predicate: fn(ref X) -> bool）：callee 是 Fn 变量且
                // 参数为 ref 时，实参自动取引用（&item），否则 E0308 expected &
                if let Some(ref_flags) = &callee_fn_refs {
                    for (i, s) in args_s.iter_mut().enumerate() {
                        if i < ref_flags.len() && ref_flags[i] && !s.starts_with('&') {
                            // 检查实参是否已是 &fn(...) 类型：若是则不再加 &（避免 `&&fn(...)`）
                            let arg = args.get(i);
                            let is_already_ref_fn = matches!(arg, Some(a) if matches!(&a.ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Fn { .. })))
                                || matches!(arg, Some(a) if matches!(&a.kind, ExprKind::Var(_)) && matches!(&a.ty, IrType::Fn { .. }));
                            if !is_already_ref_fn {
                                *s = format!("&{}", s);
                            }
                        }
                    }
                }
                // Fn 类型 callee（闭包/函数参数变量，如 filter 的 `pred(item)`）：
                // 具名函数有 fn_param_types 自动 clone，但 Fn 变量不在该表内。
                // 其非 ref 实参若是变量/索引且类型非 Copy（T 泛型等），按值传会 move
                // （E0382，filter 中 pred(item) 后 item 再被引用）→ 自动 .clone()。
                // 跳过 ref/mut ref 参数（已在上面转 &x / &mut x）。
                if let Some(ref_flags) = &callee_fn_refs {
                    for (i, a) in args.iter().enumerate() {
                        if i >= ref_flags.len() || ref_flags[i] || i >= args_s.len() {
                            continue;
                        }
                        let arg_is_var = matches!(
                            &a.kind,
                            ExprKind::Var(_)
                                | ExprKind::IndexGet { .. }
                                | ExprKind::FieldAccess { .. }
                        );
                        let arg_is_copy = matches!(&a.ty, IrType::Int | IrType::F64 | IrType::Bool);
                        let arg_is_assoc = if let IrType::Named { path, .. } = &a.ty {
                            // 仅关联类型（含 :: 如 I::Item，或以 < 开头的 <I as Trait>::Item）
                            // 豁免自动 clone；普通大写开头的结构体类型（Cow/Point/Dog 等）
                            // 仍需 clone（E0382：combined/structs/polish_16_duck 回归）
                            path.contains("::") || path.starts_with('<')
                        } else {
                            false
                        };
                        if arg_is_var && !arg_is_copy && !arg_is_assoc {
                            let s = &args_s[i];
                            let is_none_lit = s.trim_end() == "None";
                            if !s.starts_with('&')
                                && !s.ends_with(".clone()")
                                && !s.contains("::")
                                && !is_none_lit
                            {
                                let is_ref_str = matches!(&a.ty, IrType::Ref(inner)
                                    if matches!(inner.as_ref(), IrType::Str)
                                        || matches!(inner.as_ref(), IrType::Named { path, .. }
                                            if path == "str" || path == "String"));
                                args_s[i] = if is_ref_str {
                                    format!("{}.to_string()", s)
                                } else {
                                    format!("{}.clone()", s)
                                };
                            }
                        }
                    }
                }
                // LZ 值语义：非 Copy 类型的变量按值传给用户函数会移动（E0382）；
                // 若实参是变量且参数类型非 Copy（Str/Option/Named 等），自动 .clone()。
                // 排除 ref/mut ref 参数（下面单独处理 &x / &mut x）。
                if let Some(callee_name) = match &callee.kind {
                    ExprKind::Var(n) => Some(n.clone()),
                    _ => None,
                } {
                    // checker 块调用：callee 参数类型为 __Params（validate_port((r))）→
                    // 打包实参为 __Params 并传 &mut __ps；捕获变量追加 &mut 实参
                    if let Some(callee_ptypes) = self.fn_param_types.get(&callee_name).cloned() {
                        if callee_ptypes.len() == 1
                            && matches!(&callee_ptypes[0], IrType::Named { path, .. } if path == "__Params")
                        {
                            let packed_args: Vec<String> =
                                args_s.iter().map(|a| format!("Box::new({})", a)).collect();
                            let extra = self.checker_extra_args(&callee_name);
                            let call = if extra.is_empty() {
                                format!("{}(&mut __ps)", callee_name)
                            } else {
                                format!("{}(&mut __ps, {})", callee_name, extra.join(", "))
                            };
                            return format!(
                                "{{ let mut __ps = __Params {{ args: vec![{}], kwargs: std::collections::HashMap::new() }}; {}; }}",
                                packed_args.join(", "),
                                call
                            );
                        }
                    }
                    if let Some(callee_ptypes) = self.fn_param_types.get(&callee_name).cloned() {
                        let ref_flags = self
                            .fn_ref_params
                            .get(&callee_name)
                            .cloned()
                            .unwrap_or_default();
                        for (i, a) in args.iter().enumerate() {
                            if i >= callee_ptypes.len() {
                                break;
                            }
                            let is_ref_param = ref_flags.get(i).map_or(false, |(r, _)| *r);
                            if is_ref_param {
                                continue;
                            }
                            let param_is_copy = matches!(
                                &callee_ptypes[i],
                                IrType::Int | IrType::F64 | IrType::Bool
                            );
                            // Iterator<T> 参数（生成 impl Iterator<Item=T>）：
                            // 实参为 List/Vec 时需自动 .into_iter()（Vec 不是 Iterator，E0277）
                            let param_is_iterator = matches!(
                                &callee_ptypes[i],
                                IrType::Named { path, .. } if path == "Iterator"
                            );
                            let arg_is_vec = matches!(
                                &a.ty,
                                IrType::Named { path, .. } if path == "List" || path == "Vec"
                            );
                            if param_is_iterator && arg_is_vec && i < args_s.len() {
                                let s = &args_s[i];
                                if !s.starts_with('&') && !s.contains(".into_iter()") {
                                    args_s[i] = format!("{}.into_iter()", s);
                                }
                            }
                            let arg_is_var = matches!(
                                &a.kind,
                                ExprKind::Var(_)
                                    | ExprKind::IndexGet { .. }
                                    | ExprKind::FieldAccess { .. }
                            );
                            let arg_is_copy =
                                matches!(&a.ty, IrType::Int | IrType::F64 | IrType::Bool);
                            let arg_is_assoc = if let IrType::Named { path, .. } = &a.ty {
                                // 仅关联类型（含 :: 如 I::Item，或以 < 开头的 <I as Trait>::Item）
                                // 豁免自动 clone；普通大写开头的结构体类型（Cow/Point/Dog 等）
                                // 仍需 clone（E0382：combined/structs/polish_16_duck 回归）
                                path.contains("::") || path.starts_with('<')
                            } else {
                                false
                            };
                            // Fn 类型参数（impl Fn(...) opaque）不可 clone（E0599）：
                            // 实参是闭包变量时直接传引用即可，不自动 .clone()
                            let param_is_fn = matches!(&callee_ptypes[i], IrType::Fn { .. });
                            // 缺陷 B 修复：实参为 IndexGet（`f(ts[0])`）且元素类型
                            // 非 Copy 时，从 Vec 索引取出即 move（E0507），需自动
                            // .clone()——与变量实参的 E0382 处理对齐
                            if !param_is_copy
                                && arg_is_var
                                && !arg_is_copy
                                && !param_is_fn
                                && !arg_is_assoc
                                && i < args_s.len()
                            {
                                let s = &args_s[i];
                                let is_none_lit = s.trim_end() == "None";
                                if !s.starts_with('&')
                                    && !s.ends_with(".clone()")
                                    && !s.contains("::")
                                    && !is_none_lit
                                {
                                    // 字符串实参按形态层归一（与 Fn 变量调用路径 9759 对齐）：
                                    // &str / &String 借用视图传给拥有值形参（String）需
                                    // .to_string()，不能用 .clone()（&str.clone() 仍是 &str，E0308）。
                                    let is_ref_str = matches!(
                                        &a.ty,
                                        IrType::Ref(inner)
                                            if matches!(inner.as_ref(), IrType::Str)
                                            || matches!(
                                                inner.as_ref(),
                                                IrType::Named { path, .. }
                                                    if path == "str" || path == "String"
                                            )
                                    );
                                    args_s[i] = if is_ref_str {
                                        format!("{}.to_string()", s)
                                    } else {
                                        format!("{}.clone()", s)
                                    };
                                }
                            }
                        }
                    }
                    // ref/mut ref 参数：调用点自动传 &x / &mut x
                    if let Some(ref_flags) = self.fn_ref_params.get(&callee_name).cloned() {
                        for (i, _a) in args.iter().enumerate() {
                            if i >= ref_flags.len() {
                                break;
                            }
                            let (is_ref, is_mut) = ref_flags[i];
                            if is_ref && i < args_s.len() {
                                let s = &args_s[i];
                                // 避免重复引用（已是 &x 或 &mut x 时跳过）
                                if !s.starts_with('&') {
                                    args_s[i] = if is_mut {
                                        format!("&mut {}", s)
                                    } else {
                                        // Range 实参（0i64..5i64）取引用需括号：
                                        // `&(0i64..5i64)`，否则解析为 `(&0i64)..5i64`
                                        // （iter.lz collect_list(&0i64..5i64)，E0308 expected &i64 found i64）
                                        if s.contains("..") {
                                            format!("&({})", s)
                                        } else {
                                            format!("&{}", s)
                                        }
                                    };
                                }
                            }
                        }
                    }
                }

                // 泛型类型参数 → turbofish 语法: foo::<T>(args)
                let turbofish = if !type_args.is_empty() {
                    let types: Vec<String> =
                        type_args.iter().map(|t| self.rust_type_name(t)).collect();
                    format!("::<{}>", types.join(", "))
                } else {
                    String::new()
                };

                // 默认参数：函数有 def_count 个默认参数，调用方少传了 → 补 None

                // 检查是否是 struct 构造函数调用（如 RangeIter(current: start, end: end)）
                let fn_param_key = if matches!(&callee.kind, ExprKind::Var(_))
                    && self.struct_fields_info.contains_key(&callee_s)
                {
                    // 尝试用 "Type::new" 查找 struct 的构造函数
                    format!("{}::new", callee_s)
                } else {
                    callee_s.clone()
                };

                if let Some(&(total_params, def_count)) = self.fn_param_info.get(&fn_param_key) {
                    let required = total_params - def_count;
                    if args_s.len() < required {
                        // 少传了必需参数——这是编译器 bug，插入占位符
                        while args_s.len() < required {
                            args_s.push("/* missing arg */".to_string());
                        }
                    }
                    // 补默认参数：将显式传入的后几个参数包裹在 Some() 中
                    let explicit_default_args = if args_s.len() > required {
                        args_s.len() - required
                    } else {
                        0
                    };
                    for i in required..args_s.len() {
                        let arg_idx = i - required;
                        if arg_idx < explicit_default_args {
                            args_s[i] = format!("Some({})", args_s[i]);
                        }
                    }
                    // 补 None 填充未提供的默认参数
                    while args_s.len() < total_params {
                        args_s.push("None".to_string());
                    }
                }

                // 推导式展开: comp!(|x| body, iter[, cond]) → (iter).into_iter().filter(|x| cond).map(|x| body).collect()
                if callee_s == "comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        let lambda = strip_lambda_type(lambda);
                        // 迭代源为变量时 clone（值语义，同一容器可被多个推导式复用，
                        // 否则 E0382 use of moved value，如 lib_sort quick_sort 两次
                        // `[x for x in rest ...]`）
                        let iter_owned = if let Some(iter_expr) = args.get(1) {
                            if matches!(&iter_expr.kind, ExprKind::Var(_)) {
                                format!("({}).clone()", iter)
                            } else {
                                iter.clone()
                            }
                        } else {
                            iter.clone()
                        };
                        // 第三个参数存在 → 过滤条件
                        if let Some(cond) = args_s.get(2) {
                            // 泛型上下文（元素为泛型 T 非 Copy）：filter 闭包 `|&x|`
                            // 会 move 出共享引用（E0507 cannot move out of shared
                            // reference，lib_sort quick_sort），改用 filter_map：
                            // 闭包参数为 owned 值，比较借用不 move，返回 Some/None 保留元素。
                            if self.in_generic_fn {
                                if let Some((params, body)) = lambda_split(cond) {
                                    let p =
                                        params.first().cloned().unwrap_or_else(|| "x".to_string());
                                    let fm = format!(
                                        "|{}| {{ if ({}) {{ Some({}) }} else {{ None }} }}",
                                        p, body, p
                                    );
                                    return format!(
                                        "({}).into_iter().filter_map({}).map({}).collect::<Vec<_>>()",
                                        iter_owned, fm, lambda
                                    );
                                }
                            }
                            let cond = strip_lambda_type_with_ref(cond);
                            // filter 谓词仅只读外部变量（如 `x <= pivot`），借用捕获
                            // 而非 move：pivot 可被多个推导式复用，否则 E0382
                            // use of moved value（lib_sort quick_sort）
                            let cond_borrowed = cond.trim_start_matches("move ").to_string();
                            return format!(
                                "({}).into_iter().filter({}).map({}).collect::<Vec<_>>()",
                                iter_owned, cond_borrowed, lambda
                            );
                        }
                        return format!(
                            "({}).into_iter().map({}).collect::<Vec<_>>()",
                            iter_owned, lambda
                        );
                    }
                    return format!("vec![]");
                }
                // dict_comp!(|x| (k, v), iter[, cond]) → (iter).into_iter().filter(|&x| cond).map(|x| (k,v)).collect()
                if callee_s == "dict_comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        let iter_method = if let Some(iter_expr) = args.get(1) {
                            if matches!(&iter_expr.ty, IrType::Str) {
                                ".chars()"
                            } else {
                                ".into_iter()"
                            }
                        } else {
                            ".into_iter()"
                        };
                        let lambda = strip_lambda_type(lambda);
                        if let Some(cond) = args_s.get(2) {
                            let cond = strip_lambda_type_with_ref(cond);
                            return format!(
                                "({}){}.filter({}).map({}).collect::<HashMap<_,_>>()",
                                iter, iter_method, cond, lambda
                            );
                        }
                        return format!(
                            "({}){}.map({}).collect::<HashMap<_,_>>()",
                            iter, iter_method, lambda
                        );
                    }
                    return format!("HashMap::new()");
                }
                // set_comp!(|x| elem, iter[, cond]) → (iter).into_iter().filter(|&x| cond).map(|x| elem).collect()
                if callee_s == "set_comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        let iter_method = if let Some(iter_expr) = args.get(1) {
                            if matches!(&iter_expr.ty, IrType::Str) {
                                ".chars()"
                            } else {
                                ".into_iter()"
                            }
                        } else {
                            ".into_iter()"
                        };
                        let lambda = strip_lambda_type(lambda);
                        if let Some(cond) = args_s.get(2) {
                            let cond = strip_lambda_type_with_ref(cond);
                            return format!(
                                "({}){}.filter({}).map({}).collect::<HashSet<_>>()",
                                iter, iter_method, cond, lambda
                            );
                        }
                        return format!(
                            "({}){}.map({}).collect::<HashSet<_>>()",
                            iter, iter_method, lambda
                        );
                    }
                    return format!("HashSet::new()");
                }

                // 多 for 推导链: comp_outer!(|x| ..., iter, cond) → flat_map + collect
                // comp_mid!(|x| ..., iter, cond) → flat_map（不 collect）
                // comp_leaf!(|x| body, iter, cond) → map（不 collect）
                // dict_comp_* / set_comp_* 同理（collect HashMap/HashSet）
                for (prefix, collect_ty) in [
                    ("comp_", "Vec<_>"),
                    ("dict_comp_", "HashMap<_,_>"),
                    ("set_comp_", "HashSet<_>"),
                ] {
                    if let Some(suffix) = callee_s.strip_prefix(prefix) {
                        if suffix == "outer!" || suffix == "mid!" || suffix == "leaf!" {
                            if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                                let iter_method = if let Some(iter_expr) = args.get(1) {
                                    if matches!(&iter_expr.ty, IrType::Str) {
                                        ".chars()"
                                    } else {
                                        ".into_iter()"
                                    }
                                } else {
                                    ".into_iter()"
                                };
                                let lambda = strip_lambda_type(lambda);
                                let op = if suffix == "leaf!" { "map" } else { "flat_map" };
                                let chain = match args_s.get(2) {
                                    Some(cond) => {
                                        let cond = strip_lambda_type_with_ref(cond);
                                        format!(
                                            "({}){}.filter({}).{}({})",
                                            iter, iter_method, cond, op, lambda
                                        )
                                    }
                                    None => {
                                        format!("({}){}.{}({})", iter, iter_method, op, lambda)
                                    }
                                };
                                if suffix == "outer!" {
                                    return format!("{}.collect::<{}>()", chain, collect_ty);
                                }
                                return chain;
                            }
                            return format!("{}::new()", collect_ty);
                        }
                    }
                }

                // 检测 callee 是否为 FieldAccess 形式 Type.Variant → Type::Variant
                // 仅当 field 是大写开头（枚举变体）时才用 ::；小写开头为方法调用，用 .
                if let ExprKind::FieldAccess { base, field } = &callee.kind {
                    // 用户导入模块（含别名）的函数调用 m.add(...) → add(...)：
                    // 模块项已平铺生成到同一 Rust 文件，直接调用 field 即可
                    // （避免走方法调用路径把 add 误映射成 insert）
                    if matches!(&base.kind, ExprKind::Var(base_name)
                        if self.imported_modules.contains(base_name.as_str()))
                    {
                        return format!("{}({})", field, args_s.join(", "));
                    }
                    let base_s = self.gen_expr(base);
                    let known_modules = ["std", "core", "alloc", "crate", "self", "super"];
                    let is_std_module = known_modules.contains(&base_s.as_str());
                    let is_var_base = matches!(&base.kind, ExprKind::Var(_));
                    let is_known_type = is_var_base && self.is_known_type_or_enum(&base_s);
                    let sep = if is_var_base && (is_std_module || is_known_type) {
                        // 类型名上的调用一律关联路径：Cell::new / Option::None /
                        // Ordering::Less（小写 field 也是关联函数，否则 Cell.new → E0423）
                        "::"
                    } else {
                        "."
                    };
                    if sep == "::" {
                        // 检查变体字段类型，为递归字段自动包裹 Box::new()
                        let field_types = self
                            .enum_variant_fields
                            .get(&(base_s.clone(), field.clone()));
                        let wrapped_args: Vec<String> = args_s
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                let needs_box = field_types.as_ref().map_or(false, |types| {
                                    types.get(i).map_or(false, |ty| type_refers_to(ty, &base_s))
                                });
                                if needs_box {
                                    format!("Box::new({})", a)
                                } else {
                                    // 非 Copy 类型字段 + 变量参数 → 自动 clone 避免 move 后借用报错
                                    let field_is_noncopy = field_types
                                        .as_ref()
                                        .and_then(|types| types.get(i))
                                        .map_or(false, |ty| match ty {
                                            IrType::Str => true,
                                            IrType::Named { path, args: _ } => matches!(
                                                path.as_str(),
                                                "String" | "Vec" | "HashMap" | "Box"
                                            ),
                                            _ => false,
                                        });
                                    if field_is_noncopy
                                        && matches!(
                                            args.get(i).map(|a| &a.kind),
                                            Some(ExprKind::Var(_) | ExprKind::FieldAccess { .. })
                                        )
                                    {
                                        format!("{}.clone()", a)
                                    } else {
                                        a.clone()
                                    }
                                }
                            })
                            .collect();
                        // Option::None（无参变体）：注入类型参数，避免闭包返回位置
                        // 无法推断 T（E0282，如 opt.and_then(|x| Option.None)）
                        if field == "None" && wrapped_args.is_empty() && base_s == "Option" {
                            // 泛型函数（如 map<R> 内 `case Option.None => Option.None`）中
                            // 硬编码 i64 错误：Option::None 让 Rust 从 match 臂配对推断（combo-struct-method.lz）；
                            // 且用户自定义 `enum Option<T>` 会遮蔽 std Option（enum.lz），
                            // 裸 None 是 std 变体类型不匹配（E0308），必须带 Option:: 前缀
                            if self.in_generic_fn {
                                return "Option::None".to_string();
                            }
                            let elem = self.option_none_elem(&expr.ty);
                            return format!("Option::<{}>::None", elem);
                        }
                        // 命名字段变体（Type.Variant(field: v) → Type::Variant { field: v }）：
                        // gen_enum_def 对命名字段变体生成 struct variant，tuple 构造会 E0533
                        let named_fields = self
                            .enum_variant_named_fields
                            .get(&(base_s.clone(), field.clone()))
                            .cloned()
                            .unwrap_or_default();
                        if !named_fields.is_empty() && named_fields.len() == wrapped_args.len() {
                            let pairs: Vec<String> = named_fields
                                .iter()
                                .zip(wrapped_args.iter())
                                .map(|(f, a)| format!("{}: {}", f, a))
                                .collect();
                            return format!("{}::{} {{ {} }}", base_s, field, pairs.join(", "));
                        }
                        // 无数据变体的调用式构造 `Shape.Point()` → `Shape::Point`。
                        // Rust 的单元变体不是函数，带 `()` 即 E0618（2026-10-02 实测：
                        // CY/TESTS/07_data_structures/enum_data.lz 走 rust 后端 rustc 失败，
                        // 同一份 .lz 在 cy 后端能跑 ⇒ 跨后端行为分叉）。
                        // 限定在「field 确是 base 的变体」且「该变体零字段」，
                        // 否则 Cell::new() / Vec::new() 这类零参关联函数会被摘掉括号。
                        let unit_variant_ctor = wrapped_args.is_empty()
                            && self
                                .enum_variants
                                .get(field)
                                .map_or(false, |en| en == &base_s)
                            && self
                                .enum_variant_fields
                                .get(&(base_s.clone(), field.clone()))
                                .map_or(true, |v| v.is_empty());
                        if unit_variant_ctor {
                            return format!("{}::{}", base_s, field);
                        }
                        return format!("{}::{}({})", base_s, field, wrapped_args.join(", "));
                    }
                    // else: normal field access call, fall through
                }

                // 检测 enum variant 构造器调用: Circle(0,0,5) → Shape::Circle(0, 0, 5)
                // 注：callee 为 Var("Some") 时上方 gen_expr 已展开为 "Option::Some"，
                // 因此需同时按 callee 原始 Var 名匹配（裸简写 `Some(42)` / `Ok(5)`）。
                let (variant_name, enum_name): (String, String) =
                    if let Some(en) = self.enum_variants.get(&callee_s) {
                        (callee_s.clone(), en.clone())
                    } else if let ExprKind::Var(n) = &callee.kind {
                        match self.enum_variants.get(n) {
                            Some(en) => (n.clone(), en.clone()),
                            None => (String::new(), String::new()),
                        }
                    } else {
                        (String::new(), String::new())
                    };
                if !enum_name.is_empty() {
                    return if args_s.is_empty() {
                        format!("{}::{}", enum_name, variant_name)
                    } else {
                        // 命名字段变体（`Some(value: 42)` / 裸简写 `Some(42)`）→ 结构体形式：
                        // `Enum::Variant { field: arg, ... }`；否则元组形式 `Enum::Variant(a, b)`
                        // （enum.lz 自定义 `enum Option<T>: Some(value: T)` 后 `Some(42)`
                        //   生成 `Option::Some(42)` 会报 E0533 expected value, found struct variant）
                        let named_fields = self
                            .enum_variant_named_fields
                            .get(&(enum_name.clone(), variant_name.clone()))
                            .cloned()
                            .unwrap_or_default();
                        if !named_fields.is_empty() {
                            let args_c: Vec<String> = args_s
                                .iter()
                                .zip(args.iter())
                                .map(|(s, a)| {
                                    if matches!(&a.kind, ExprKind::Var(n) if n == "self" || n == "self_")
                                        && !s.contains(".clone()")
                                    {
                                        format!("{}.clone()", s)
                                    } else {
                                        s.clone()
                                    }
                                })
                                .collect();
                            let pairs: Vec<String> = named_fields
                                .iter()
                                .zip(args_c.iter())
                                .map(|(f, s)| format!("{}: {}", f, s))
                                .collect();
                            return format!(
                                "{}::{} {{ {} }}",
                                enum_name,
                                variant_name,
                                pairs.join(", ")
                            );
                        }
                        // `Err(self)`：self 是 &Self（&Rc<T>），Err 需要 owned Rc<T>，
                        // 自动 clone（box.lz try_unwrap → E0277 cannot move out of self）
                        let args_c: Vec<String> = args_s
                            .iter()
                            .zip(args.iter())
                            .map(|(s, a)| {
                                if matches!(&a.kind, ExprKind::Var(n) if n == "self" || n == "self_")
                                    && !s.contains(".clone()")
                                {
                                    format!("{}.clone()", s)
                                } else {
                                    s.clone()
                                }
                            })
                            .collect();
                        format!("{}::{}({})", enum_name, variant_name, args_c.join(", "))
                    };
                }

                // 类型转换: int(x) → x as i64, str(x) → format!("{}", x), f64(x) → x as f64
                if matches!(callee_s.as_str(), "int" | "str" | "f64" | "float")
                    && !args_s.is_empty()
                {
                    return match callee_s.as_str() {
                        "int" => {
                            // 检查参数表达式类型来决定转换方式
                            // 字符串（Str 或 Named String/str）→ fallible 解析，否则 E0605
                            // （p41_full_tokenize `num as int` 复现：num: String → parse）
                            if args.len() == 1 {
                                let arg_ty = &args[0].ty;
                                // 用户 struct 定义了 __int__ 缺口魔法 → 直派（06d §十六）
                                if let IrType::Named { path, .. } = arg_ty {
                                    if self.is_known_type(path)
                                        && self.struct_method_names(path).contains("__int__")
                                    {
                                        return format!("({}).__int__()", args_s[0]);
                                    }
                                }
                                let is_str = matches!(arg_ty, IrType::Str)
                                    || matches!(arg_ty, IrType::Named { path, .. }
                                        if path == "String" || path == "str");
                                if is_str {
                                    format!("({}).parse::<i64>().unwrap()", args_s[0])
                                } else {
                                    format!("({} as i64)", args_s[0])
                                }
                            } else {
                                format!("({} as i64)", args_s[0])
                            }
                        }
                        "str" => {
                            // 用户 struct 且有 __str__ → 调用 __str__()；否则用 Display
                            if args.len() == 1 {
                                if let IrType::Named { path, .. } = &args[0].ty {
                                    if self.is_known_type(path) {
                                        format!("({}).__str__()", args_s[0])
                                    } else {
                                        format!("format!(\"{{}}\", {})", args_s[0])
                                    }
                                } else {
                                    format!("format!(\"{{}}\", {})", args_s[0])
                                }
                            } else {
                                format!("format!(\"{{}}\", {})", args_s[0])
                            }
                        }
                        "f64" | "float" => {
                            if args.len() == 1 {
                                let arg_ty = &args[0].ty;
                                // 用户 struct 定义了 __float__ 缺口魔法 → 直派（06d §十六）
                                if let IrType::Named { path, .. } = arg_ty {
                                    if self.is_known_type(path)
                                        && self.struct_method_names(path).contains("__float__")
                                    {
                                        return format!("({}).__float__()", args_s[0]);
                                    }
                                }
                                if matches!(arg_ty, IrType::Str) {
                                    format!("({}).parse::<f64>().unwrap()", args_s[0])
                                } else {
                                    format!("({} as f64)", args_s[0])
                                }
                            } else {
                                format!("({} as f64)", args_s[0])
                            }
                        }
                        _ => unreachable!(),
                    };
                }

                if callee_s == "print" || callee_s == "println" {
                    let fmt_placeholders: String =
                        args_s.iter().map(|_| "{:?}").collect::<Vec<_>>().join(" ");
                    let fmt = format!("\"{}\"", fmt_placeholders);
                    // 顶层静态（LazyLock<..>）需解引用才能打印值：print(config) → print(*config)
                    // 注意：gen_expr 的 Var 分支已对 lazy_static 生成 `(*name).clone()`，
                    // 此处直接用该结果即可（若再包 (*{}) 会双重解引用，E0614）
                    let print_args: Vec<String> = args
                        .iter()
                        .zip(args_s.iter())
                        .map(|(_a, s)| s.clone())
                        .collect();
                    format!("println!({}, {})", fmt, print_args.join(", "))
                } else if callee_s == "eprintln!" {
                    // check 语句生成的 eprintln! 调用：格式宏第一个参数必须是字面量
                    // 格式串（Str 字面量不能 .to_string()，E0308/E0061），
                    // 其他参数保持占位符输出
                    let mut macro_args: Vec<String> = Vec::new();
                    for (i, (a, s)) in args.iter().zip(args_s.iter()).enumerate() {
                        if i == 0 {
                            if let ExprKind::Lit(LitKind::Str(_)) = &a.kind {
                                macro_args.push(s.trim_end_matches(".to_string()").to_string());
                                continue;
                            }
                        }
                        macro_args.push(s.clone());
                    }
                    format!("eprintln!({})", macro_args.join(", "))
                } else if callee_s == "set!" {
                    format!("std::collections::HashSet::from([{}])", args_s.join(", "))
                } else if callee_s == "panic!" || callee_s == "panic" {
                    format!("panic!(\"{{:?}}\", {})", args_s.join(", "))
                } else if callee_s == "Exception" {
                    format!("panic!(\"Exception: {{:?}}\", {})", args_s.join(", "))
                // --- Prelude free function → method/expression mappings ---
                } else if callee_s == "len" && args_s.len() == 1 {
                    // fn_ref_params 自动 & 可能把 len(self) 的实参变成 &self（&usize），
                    // 去掉多余 &（E0606 casting &usize as i64 is invalid）
                    // 自定义类型实现 __len__ 魔法（Range2.__len__）→ 调用 __len__()
                    let arg0 = args_s[0].trim_start_matches('&');
                    let has_custom_len = matches!(&args[0].ty, IrType::Named { path, .. }
                        if self.is_known_type(path)
                            && self.struct_method_names(path).contains("__len__"));
                    if has_custom_len {
                        format!("({}.__len__() as i64)", arg0)
                    } else {
                        format!("({}.len() as i64)", arg0)
                    }
                } else if callee_s == "abs" && args_s.len() == 1 {
                    // abs(x)：用户 struct 定义了 __abs__ 缺口魔法 → 直派（06d §六）；
                    // 其余走 lz_abs 内建（i64/f64 重载经类型分派）
                    let has_custom_abs = matches!(&args[0].ty, IrType::Named { path, .. }
                        if self.is_known_type(path)
                            && self.struct_method_names(path).contains("__abs__"));
                    if has_custom_abs {
                        format!("({}.__abs__())", args_s[0])
                    } else {
                        format!("lz_abs({})", args_s[0])
                    }
                } else if callee_s == "type_name" && args_s.len() == 1 {
                    // BUG-EC-006: type_name(x) 函数式内省 → 静态类型名（方案 C，与 v.type_name() 方法一致）
                    let t = self.rust_type(&args[0].ty);
                    let t = t.trim_start_matches('&').trim().to_string();
                    format!("std::any::type_name::<{}>().to_string()", t)
                } else if callee_s == "contains" && args_s.len() == 2 {
                    // HashMap/Dict → contains_key; String/Vec → contains
                    let is_dict = matches!(&args[0].ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                    if is_dict {
                        format!("({}).contains_key(&{})", args_s[0], args_s[1])
                    } else {
                        // 判断是否为 Vec/List（Vec::contains 需要 &T）
                        let is_vec = matches!(&args[0].ty, IrType::Named { path, .. } if path == "List" || path == "Vec")
                            || matches!(&args[0].ty, IrType::Generic(_));
                        // 第二个参数：String → 裸字符串字面量（String::contains 接受 Pattern，&String 不实现 Pattern）
                        let arg1 = if args_s[1].ends_with(".to_string()") {
                            args_s[1].trim_end_matches(".to_string()").to_string()
                        } else {
                            args_s[1].clone()
                        };
                        if is_vec {
                            format!("({}).contains(&{})", args_s[0], arg1)
                        } else {
                            format!("({}).contains({})", args_s[0], arg1)
                        }
                    }
                } else if callee_s == "iter" && args_s.len() == 1 {
                    format!("({}).iter()", args_s[0])
                } else if callee_s == "enumerate" && args_s.len() == 1 {
                    format!("({}).iter().enumerate()", args_s[0])
                } else if callee_s == "zip" && args_s.len() == 2 {
                    format!("({}).into_iter().zip({}.into_iter())", args_s[0], args_s[1])
                } else if callee_s == "clone" && args_s.len() == 1 {
                    format!("({}).clone()", args_s[0])
                } else if callee_s == "__go" && args_s.len() >= 1 {
                    // go/spawn expr → 根据函数上下文分派：
                    //   async 函数中 → __spawn_task(expr) 异步 Future
                    //   普通函数中 → std::thread::spawn(move || { expr }) 并行线程
                    if self.current_fn_is_async {
                        format!("__spawn_task({})", args_s.join(", "))
                    } else {
                        format!("std::thread::spawn(move || {{ {} }})", args_s.join(", "))
                    }
                } else if callee_s == "spawn" && args_s.len() >= 1 {
                    // spawn(expr) → 保持异步 Future 语义
                    // 在 async 上下文中：spawn fetch(1) 生成 __spawn_task(fetch(1))
                    // 注意：fetch 是 async fn，直接调用返回 Future
                    format!("__spawn_task({})", args_s.join(", "))
                } else if callee_s == "sort" && args_s.len() == 1 {
                    format!(
                        "{{ let mut _tmp = {0}.clone(); _tmp.sort(); _tmp }}",
                        args_s[0]
                    )
                } else if callee_s == "reverse" && args_s.len() == 1 {
                    format!(
                        "{{ let mut _tmp = {0}.clone(); _tmp.reverse(); _tmp }}",
                        args_s[0]
                    )
                } else if callee_s == "format" && !self.fn_param_types.contains_key("format") {
                    // format("fmt", args...) → format!("fmt", args...)
                    // 用户自定义 format 函数（string.lz）优先调用自定义函数，
                    // 否则 std format! 宏（Vec 等参数报 E0277 Display）
                    let fmt_str = if args.len() >= 1 {
                        if let ExprKind::Lit(LitKind::Str(s)) = &args[0].kind {
                            format!("\"{}\"", s.escape_default())
                        } else {
                            args_s[0].clone()
                        }
                    } else {
                        "\"\"".to_string()
                    };
                    let rest = if args_s.len() > 1 {
                        format!(", {}", args_s[1..].join(", "))
                    } else {
                        String::new()
                    };
                    format!("format!({}{})", fmt_str, rest)
                } else if callee_s == "hash" && args_s.len() == 1 {
                    format!("{{ let mut _hasher = std::collections::hash_map::DefaultHasher::new(); std::hash::Hash::hash(&{}, &mut _hasher); std::hash::Hasher::finish(&_hasher) as i64 }}", args_s[0])
                } else if callee_s == "bool" && args_s.len() == 1 {
                    format!("({} != 0)", args_s[0])
                } else if callee_s == "range" && args_s.len() >= 1 {
                    // range(start, end) or range(end) → start..end or 0..end
                    if args_s.len() == 1 {
                        format!("0..{}", args_s[0])
                    } else {
                        format!("{}..{}", args_s[0], args_s[1])
                    }
                // ── Iterator/collection free-function → method mappings ──
                // Pipe inserts receiver as first arg: [1,2,3] |> f(args) → f([1,2,3], args)
                // Strip type annotations from closure args for Rust iterator adapters
                } else if callee_s == "sum"
                    && args_s.len() == 1
                    && !self.fn_param_types.contains_key("sum")
                {
                    // sum(collection) → collection.iter().copied().sum::<i64>()
                    // （.iter() 产出 &i64，.copied() 转值；.sum::<i64>() 显式类型
                    // 标注，否则 E0283 cannot infer type parameter S）
                    // 守卫：用户自定义同名函数（iterator.lz `def sum(iter: Iterator)`）
                    // 优先调用自定义实现，否则被劫持为 std sum（E0599）
                    format!("({}).iter().copied().sum::<i64>()", args_s[0])
                } else if callee_s == "map"
                    && args_s.len() == 2
                    && !self.fn_param_types.contains_key("map")
                {
                    // map(collection, fn) → collection.into_iter().map(fn).collect::<Vec<_>>()
                    // LZ 自由函数 map 返回 List（与链式 .map 不同），需 collect 成 Vec
                    let lambda = strip_lambda_type(&args_s[1]);
                    format!(
                        "({}).into_iter().map({}).collect::<Vec<_>>()",
                        args_s[0], lambda
                    )
                } else if callee_s == "filter"
                    && args_s.len() == 2
                    && !self.fn_param_types.contains_key("filter")
                {
                    // filter(iterator, fn) → iterator.into_iter().filter(fn)[.copied()].collect()
                    // Vec/List 无 filter 方法（E0599），需先转迭代器；
                    // filter 闭包接收 &Item，strip_lambda_type_with_ref 给参数加 &。
                    // .copied() 仅当输入是引用（iter.lz `filter(&vec, ...)` → into_iter
                    // 产出 &i64，需转值）；owned 输入（pipe_spec 管道链 map 后的 Vec，
                    // into_iter 产出 i64）加 .copied() 报 E0271 expected &_ yields i64
                    let lambda = strip_lambda_type_with_ref(&args_s[1]);
                    let copied = if args_s[0].trim_start().starts_with('&') {
                        ".copied()"
                    } else {
                        ""
                    };
                    format!(
                        "({}).into_iter().filter({}){}.collect::<Vec<_>>()",
                        args_s[0], lambda, copied
                    )
                } else if callee_s == "fold"
                    && args_s.len() == 3
                    && !self.fn_param_types.contains_key("fold")
                {
                    // fold(collection, init, fn) → collection.into_iter().fold(init, fn)
                    // 守卫：用户自定义同名函数（tree.lz `def fold(node, acc, f)`）优先
                    let lambda = strip_lambda_type(&args_s[2]);
                    format!(
                        "({}).into_iter().fold({}, {})",
                        args_s[0], args_s[1], lambda
                    )
                } else if callee_s == "collect"
                    && args_s.len() == 1
                    && !self.fn_param_types.contains_key("collect")
                {
                    // collect(iterable)：输入可能是迭代器或已 collect 的 Vec（管道链
                    // filter 已返回 Vec，再 collect 报 E0599 no method collect on Vec）。
                    // into_iter() 对两者都有效（Iterator: IntoIterator 恒等，Vec 消费）
                    // 守卫：用户自定义同名函数（iterator.lz `def collect(iter: Iterator)`）优先
                    format!("({}).into_iter().collect::<Vec<_>>()", args_s[0])
                } else if callee_s == "max"
                    && args_s.len() == 1
                    && !self.fn_param_types.contains_key("max")
                {
                    format!("(*(&{}).iter().max().unwrap())", args_s[0])
                } else if callee_s == "min" && args_s.len() == 1 {
                    format!("(*(&{}).iter().min().unwrap())", args_s[0])
                } else if callee_s == "any" && args_s.len() == 2 {
                    let lambda = strip_lambda_type(&args_s[1]);
                    format!("({}).iter().any({})", args_s[0], lambda)
                } else if callee_s == "all" && args_s.len() == 2 {
                    let lambda = strip_lambda_type(&args_s[1]);
                    format!("({}).iter().all({})", args_s[0], lambda)
                } else if callee_s == "sorted" && args_s.len() == 1 {
                    format!(
                        "{{ let mut _tmp = {0}.clone(); _tmp.sort(); _tmp }}",
                        args_s[0]
                    )
                } else if callee_s == "reversed" && args_s.len() == 1 {
                    format!(
                        "{{ let mut _tmp = {0}.clone(); _tmp.reverse(); _tmp }}",
                        args_s[0]
                    )
                } else if (callee_s == "push" || callee_s == "append") && args_s.len() == 2 {
                    // 自由函数 push/append(list, item) → (list).push(item)
                    // （Vec 固有方法，自动 &mut 借用；recv 经 auto_mut_locals 标为 mut）
                    // item 若被多次使用则克隆，避免二次移动 E0382
                    format!(
                        "({}).push({})",
                        args_s[0],
                        self.clone_if_multiuse(args_s[1].clone(), &args[1])
                    )
                } else if callee_s == "pop" && args_s.len() == 1 {
                    format!("({}).pop()", args_s[0])
                } else if callee_s == "extend" && args_s.len() == 2 {
                    format!(
                        "({}).extend({})",
                        args_s[0],
                        self.clone_if_multiuse(args_s[1].clone(), &args[1])
                    )
                } else if (callee_s == "insert" || callee_s == "remove") && args_s.len() == 2 {
                    format!(
                        "({}).{}({})",
                        args_s[0],
                        callee_s,
                        self.clone_if_multiuse(args_s[1].clone(), &args[1])
                    )
                // 宏系统（08-宏与编译期.md）：quote(...) 是宏体 Token 包装，
                // IR 后端不展开宏，降级为参数拼接（单参直接返回，多参用 + 连接，
                // 后续参数以 &... 借用匹配 Rust String + &str）
                } else if callee_s == "quote" && !args_s.is_empty() {
                    if args_s.len() == 1 {
                        args_s[0].clone()
                    } else {
                        let mut parts = Vec::new();
                        for (idx, a) in args_s.iter().enumerate() {
                            if idx == 0 {
                                parts.push(a.clone());
                            } else {
                                parts.push(format!("&{}[..]", a));
                            }
                        }
                        parts.join(" + ")
                    }
                // --- End prelude mappings ---
                } else if !args.is_empty()
                    && !is_kwarg_call(args)
                    && self.case_structs.contains(&callee_s)
                {
                    // case struct 位置构造：Point(1, 3) → Point { x: 1, y: 3 }（按字段声明顺序）
                    let base_name = callee_s.split('<').next().unwrap_or(&callee_s).to_string();
                    if let Some(info) = self.struct_fields_info.get(&base_name) {
                        if info.len() == args_s.len() {
                            let fields: Vec<String> = info
                                .iter()
                                .zip(args_s.iter())
                                .map(|((fname, fty), a)| {
                                    // &str → String: 字段类型为 String 且值为 .clone() 时，
                                    // 将 .clone() 替换为 .to_string()（E0308）
                                    let val =
                                        if matches!(fty, IrType::Str) && a.ends_with(".clone()") {
                                            let base = a.trim_end_matches(".clone()");
                                            format!("{}.to_string()", base)
                                        } else {
                                            a.clone()
                                        };
                                    format!("{}: {}", fname, val)
                                })
                                .collect();
                            format!("{}{} {{ {} }}", callee_s, turbofish, fields.join(", "))
                        } else {
                            format!("{}{}({})", callee_s, turbofish, args_s.join(", "))
                        }
                    } else {
                        format!("{}{}({})", callee_s, turbofish, args_s.join(", "))
                    }
                } else if !args.is_empty() && is_kwarg_call(args) && self.is_known_type(&callee_s) {
                    // Struct constructor with keyword args: Point(x=3, y=4) → Point { x: 3.0, y: 4.0 }
                    let base_name = callee_s.split('<').next().unwrap_or(&callee_s).to_string();

                    // If struct has __new__, route kwarg construction through Name::__new__(...)
                    // (converts kwargs to positional args in __new__ param order, fills defaults for missing)
                    // Skip when inside __new__ body to avoid infinite recursion (N(v: ...) inside __new__ → direct field init)
                    if !self.in_new_body && self.struct_new_params_map.contains_key(&base_name) {
                        let new_params = self.struct_new_params_map.get(&base_name).unwrap();
                        // Build kwarg name → value map
                        let kwarg_map: std::collections::HashMap<String, String> = args
                            .iter()
                            .filter_map(|a| {
                                if let ExprKind::StructCtor { name, fields } = &a.kind {
                                    if name == "_KwArg" {
                                        let k = fields.iter().find(|(n, _)| n == "name").and_then(
                                            |(_, v)| match &v.kind {
                                                ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                                                _ => None,
                                            },
                                        );
                                        let v = fields
                                            .iter()
                                            .find(|(n, _)| n == "value")
                                            .map(|(_, e)| self.gen_expr(e));
                                        return Some((k?, v?));
                                    }
                                }
                                None
                            })
                            .collect();
                        // Build positional args in __new__ param order, using defaults for missing

                        // 获取 struct 字段名列表，用于映射 kwarg_map 中的字段名到参数名
                        let field_names: Vec<String> = self
                            .struct_fields_info
                            .get(&base_name)
                            .map(|info| info.iter().map(|(n, _)| n.clone()).collect())
                            .unwrap_or_default();

                        let positional: Vec<String> = new_params
                            .iter()
                            .enumerate()
                            .map(|(i, (pname, pty))| {
                                // 尝试用字段名查找 kwarg_map（struct 构造时用的是字段名，不是参数名）
                                let field_name =
                                    field_names.get(i).cloned().unwrap_or_else(|| pname.clone());
                                if let Some(val) = kwarg_map.get(&field_name) {
                                    val.clone()
                                } else {
                                    let default = self.default_value_for(pty);

                                    default
                                }
                            })
                            .collect();

                        // 构造器方法名：魔法 `__new__`（无论写在 struct 体——
                        // 由 struct_has_new 登记，如 magic_methods 的 Config——
                        // 还是 impl 块——由 struct_method_names_map 含 "__new__" 判定，
                        // 如 box.lz 的 Rc/Arc/Box）用 `__new__`；仅 impl 块里用户定义的
                        // 普通 `new` 方法（如 g6_impl 的 Point）用 `new`。
                        let is_magic_new = self.struct_has_new.contains(&base_name)
                            || self
                                .struct_method_names_map
                                .get(&base_name)
                                .map_or(false, |s| s.contains("__new__"));
                        let new_method = if is_magic_new { "__new__" } else { "new" };
                        return format!("{}::{}({})", callee_s, new_method, positional.join(", "));
                    }

                    // 递归字段集合：字段类型直接引用 struct 自身（如 next: Self?）→ 构造时自动 Box
                    // （Vec<Rc<Self>> 等已间接，不 Box）
                    let recursive_fields: std::collections::HashSet<String> = self
                        .struct_fields_info
                        .get(&base_name)
                        .map(|info| {
                            info.iter()
                                .filter(|(_, fty)| field_needs_box(fty, &base_name))
                                .map(|(fn_, _)| fn_.clone())
                                .collect()
                        })
                        .unwrap_or_default();
                    let provided: Vec<String> = args
                        .iter()
                        .map(|a| {
                            let s = gen_kwarg_field(a, self);
                            // 空列表字段（neighbors: []）：按字段类型生成 Vec::<T>::new()，
                            // 避免推断为 Vec<i64> 与字段类型（如 Vec<Rc<SharedNode>>）不匹配
                            if let Some(fname) = kwarg_field_name(a) {
                                if let Some(info) = self.struct_fields_info.get(&base_name) {
                                    if let Some((_, fty)) = info.iter().find(|(n, _)| n == &fname) {
                                        if let IrType::Named { path, args } = fty {
                                            if (path == "Vec" || path == "List")
                                                && !args.is_empty()
                                                && s.split_once(':').map(|(_, v)| v.trim()).map_or(
                                                    false,
                                                    |v| {
                                                        v == "Vec::<i64>::new()"
                                                            || v == "Vec::new()"
                                                            || v == "vec![]"
                                                    },
                                                )
                                            {
                                                let elem = self.rust_type(&args[0]);
                                                return format!(
                                                    "{}: Vec::<{}>::new()",
                                                    fname, elem
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            // 递归字段值自动 Box：根据**值表达式类型**决定包装方式：
                            // - 值本身是 Option（head 变量 / None 字面量）→ .map(Box::new)（None 直接 None）
                            // - 值不是 Option（裸 TreeNode{...} 构造）→ Some(Box::new(...))
                            let fname = kwarg_field_name(a);
                            if let Some(fname) = fname {
                                if recursive_fields.contains(&fname) {
                                    // 取值表达式及其 IR 类型
                                    let val_expr = match &a.kind {
                                        ExprKind::StructCtor { fields, .. } => fields
                                            .iter()
                                            .find(|(n, _)| n == "value")
                                            .map(|(_, v)| v),
                                        _ => None,
                                    };
                                    let val_is_option = val_expr.map_or(false, |v| {
                                        // Option<T> 有两种 IR 表示：IrType::Option(_) 与
                                        // Named{"Option",[T]}。原实现只认前者，导致
                                        // `next: self.head`（self.head 为命名 Option）误走
                                        // Some(Box::new(..)) 分支 → 二次包裹（E0308）。
                                        is_option_ty(&v.ty)
                                            || matches!(&v.kind, ExprKind::Var(n)
                                                if n == "None" || n == "Some")
                                            // 类型未解析（Any，如 `self.head` 字段访问未回填类型）：
                                            // 按表达式形态回退 —— 裸结构构造为非 Option
                                            // （next: TreeNode{..} → Some(Box::new(..))）；
                                            // 其余（变量/字段访问，如 self.head）视为 Option
                                            // → `.map(Box::new)`。对齐 FIND_BUG.md 轮次6 的既定意图。
                                            || (matches!(&v.ty, IrType::Any)
                                                && !matches!(&v.kind, ExprKind::StructCtor { .. }))
                                    });
                                    let val_s = s
                                        .split_once(':')
                                        .map(|(_, v)| v.trim().to_string())
                                        .unwrap_or(s);
                                    return if val_s == "None" {
                                        // None 字面量：类型由字段上下文推断，直接保留
                                        format!("{}: None", fname)
                                    } else if val_is_option {
                                        format!("{}: {}.map(Box::new)", fname, val_s)
                                    } else {
                                        format!("{}: Some(Box::new({}))", fname, val_s)
                                    };
                                }
                                // BUG-SG-002/003：可空字段（`host: str?` / `db: DbConfig?`）
                                // 用非 Option 值构造 → 补 `Some(..)`（E0308）。
                                // 递归字段已在上一分支处理（Box 包装），不重复。
                                let fty = self
                                    .struct_fields_info
                                    .get(&base_name)
                                    .and_then(|info| info.iter().find(|(n, _)| n == &fname))
                                    .map(|(_, t)| t.clone());
                                if let Some(fty) = fty {
                                    let val_expr = match &a.kind {
                                        ExprKind::StructCtor { fields, .. } => fields
                                            .iter()
                                            .find(|(n, _)| n == "value")
                                            .map(|(_, v)| v),
                                        _ => None,
                                    };
                                    if val_expr.map_or(false, |v| needs_some_wrap(&fty, v)) {
                                        let val_s = s
                                            .split_once(':')
                                            .map(|(_, v)| v.trim().to_string())
                                            .unwrap_or_else(|| s.clone());
                                        return format!("{}: Some({})", fname, val_s);
                                    }
                                }
                            }
                            s
                        })
                        .collect();
                    // 已提供的字段名集合
                    let provided_names: std::collections::HashSet<String> = args
                        .iter()
                        .filter_map(|a| {
                            if let ExprKind::StructCtor { name, fields } = &a.kind {
                                if name == "_KwArg" {
                                    return fields.iter().find(|(n, _)| n == "name").and_then(
                                        |(_, v)| match &v.kind {
                                            ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                                            _ => None,
                                        },
                                    );
                                }
                            }
                            None
                        })
                        .collect();
                    // __new__ 魔术构造：补齐未提供的字段为类型默认值（如 Config(host,port) → debug:false）
                    let mut all_fields = provided;
                    // &str → String: gen_kwarg_field 对 &str 变量加了 .clone()（仍是 &str），
                    // 但目标字段是 String → 替换为 .to_string()（E0308）
                    for field_str in &mut all_fields {
                        if let Some((fname, val)) = field_str.split_once(':') {
                            let val = val.trim().trim_end_matches(',');
                            if val.ends_with(".clone()") {
                                let info = self.struct_fields_info.get(&base_name);
                                if let Some(fty) = info.and_then(|i| {
                                    i.iter().find(|(n, _)| n == fname).map(|(_, t)| t)
                                }) {
                                    if matches!(fty, IrType::Str) {
                                        let base = val.trim_end_matches(".clone()");
                                        *field_str = format!("{}: {}.to_string()", fname, base);
                                    }
                                }
                            }
                        }
                    }
                    if self.struct_has_new.contains(&base_name) {
                        if let Some(info) = self.struct_fields_info.get(&base_name) {
                            for (fname, fty) in info {
                                if !provided_names.contains(fname) {
                                    all_fields.push(format!(
                                        "{}: {}",
                                        fname,
                                        self.default_value_for(&fty)
                                    ));
                                }
                            }
                        }
                    }
                    // 自动补 PhantomData 字段（box.lz `Rc(_inner: 0)` kwarg 构造 → E0063）
                    if let Some(phantoms) = self.struct_phantom_generics.get(&base_name) {
                        for g in phantoms {
                            all_fields
                                .push(format!("_lz_phantom_{}: std::marker::PhantomData,", g));
                        }
                    }
                    format!("{}{} {{ {} }}", callee_s, turbofish, all_fields.join(", "))
                } else if let Some(&_kwidx) = self.fn_kwargs.get(&callee_s) {
                    // kwargs 注入函数调用: 普通位置实参在前，命名实参打包为 &HashMap<String, V>
                    // （若同时有 args 注入，位置实参按 variadic 起始索引打包为 &[...]）
                    let mut normal: Vec<String> = Vec::new();
                    let mut pairs: Vec<String> = Vec::new();
                    for a in args {
                        if let ExprKind::StructCtor { name, fields } = &a.kind {
                            if name == "_KwArg" {
                                let k = fields
                                    .iter()
                                    .find(|(n, _)| n == "name")
                                    .and_then(|(_, v)| match &v.kind {
                                        ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                let v = fields
                                    .iter()
                                    .find(|(n, _)| n == "value")
                                    .map(|(_, e)| self.gen_expr(e))
                                    .unwrap_or_default();
                                // 键转义为 Rust 字符串字面量: "timeout".to_string()
                                pairs.push(format!(
                                    "(\"{}\".to_string(), {})",
                                    k.replace('\\', "\\\\").replace('"', "\\\""),
                                    v
                                ));
                                continue;
                            }
                        }
                        normal.push(self.gen_expr(a));
                    }
                    // args 变参打包（Both 模式: args + kwargs 双收集）
                    if let Some(&v_idx) = self.fn_variadic.get(&callee_s) {
                        let head = normal[..v_idx.min(normal.len())].to_vec();
                        let tail = if normal.len() > v_idx {
                            normal[v_idx..].join(", ")
                        } else {
                            String::new()
                        };
                        let mut all = head;
                        if normal.len() >= v_idx {
                            all.push(format!("&[{}]", tail));
                        } else {
                            all.push("&[]".to_string());
                        }
                        normal = all;
                    }
                    let map = if pairs.is_empty() {
                        "std::collections::HashMap::new()".to_string()
                    } else {
                        format!("std::collections::HashMap::from([{}])", pairs.join(", "))
                    };
                    normal.push(format!("&{}", map));
                    format!("{}{}({})", callee_s, turbofish, normal.join(", "))
                } else if !args.is_empty() && is_kwarg_call(args) {
                    // Function call with named args: func(a, b~) → func(a, b)
                    let flat_args: Vec<String> =
                        args.iter().map(|a| gen_kwarg_value(a, self)).collect();
                    format!("{}{}({})", callee_s, turbofish, flat_args.join(", "))
                } else if let Some(&variadic_idx) = self.fn_variadic.get(&callee_s) {
                    // Variadic 函数调用: 将 variadic_idx 及之后的实参打包为 &[...]
                    let normal_args = &args_s[..variadic_idx.min(args_s.len())];
                    let variadic_args = if args_s.len() > variadic_idx {
                        args_s[variadic_idx..].join(", ")
                    } else {
                        String::new()
                    };
                    // 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` 的 args 参数
                    // 类型是 IrType::Tuple(prefix) → 打包为 (T1, T2, Vec<Box<dyn Any>>)
                    // 前 N 个实参直接进元组字段，尾部 `..` 实参 Box::new 收集
                    let is_tuple_variadic = self
                        .fn_param_types
                        .get(&callee_s)
                        .and_then(|pts| pts.get(variadic_idx))
                        .map_or(false, |t| matches!(t, IrType::Tuple(_)));
                    // type-pack（..: Tuple<Ts...>，03d §2.8 方案 B）：Tuple 全部为
                    // 泛型参数 → 打包全部实参为 Rust 异质元组 (v1, v2, v3)
                    let is_typepack_variadic = is_tuple_variadic
                        && match &self
                            .fn_param_types
                            .get(&callee_s)
                            .and_then(|pts| pts.get(variadic_idx))
                        {
                            Some(IrType::Tuple(items)) => {
                                !items.is_empty()
                                    && items.iter().all(|t| matches!(t, IrType::Generic(_)))
                            }
                            _ => false,
                        };
                    let mut all_args: Vec<String> = normal_args.to_vec();
                    if is_typepack_variadic {
                        let tuple_fields: Vec<String> = args_s[variadic_idx..].to_vec();
                        all_args.push(format!("({})", tuple_fields.join(", ")));
                    } else if is_tuple_variadic {
                        let prefix_n = args_s.len().saturating_sub(variadic_idx).min(
                            match &self
                                .fn_param_types
                                .get(&callee_s)
                                .and_then(|pts| pts.get(variadic_idx))
                            {
                                Some(IrType::Tuple(items)) => items.len(),
                                _ => 0,
                            },
                        );
                        let tuple_fields: Vec<String> =
                            args_s[variadic_idx..variadic_idx + prefix_n].to_vec();
                        let tail: Vec<String> = if args_s.len() > variadic_idx + prefix_n {
                            args_s[variadic_idx + prefix_n..]
                                .iter()
                                .map(|a| format!("Box::new({})", a))
                                .collect()
                        } else {
                            vec![]
                        };
                        let mut tuple_parts: Vec<String> = tuple_fields;
                        tuple_parts.push(format!("vec![{}]", tail.join(", ")));
                        all_args.push(format!("({})", tuple_parts.join(", ")));
                    } else if args_s.len() >= variadic_idx {
                        all_args.push(format!("&[{}]", variadic_args));
                    } else {
                        all_args.push("&[]".to_string());
                    }
                    format!("{}{}({})", callee_s, turbofish, all_args.join(", "))
                } else if let Some(ptypes) = self.fn_param_types.get(&callee_s) {
                    // 隐式 variadic: 单集合参数 + 实参数量不匹配 → auto-pack
                    if ptypes.len() == 1 && args_s.len() != 1 && self.is_collection_type(&ptypes[0])
                    {
                        let packed = if args_s.is_empty() {
                            "vec![]".to_string()
                        } else {
                            format!("vec![{}]", args_s.join(", "))
                        };
                        format!("{}{}({})", callee_s, turbofish, packed)
                    } else {
                        let call_str = format!("{}{}({})", callee_s, turbofish, args_s.join(", "));
                        // ~: 元组解包：将调用包装在 { let __t = <packed>; callee(__t.0, __t.1) } 中
                        if let Some(ref packed) = unpack_packed {
                            format!("{{ let __t = {}; {} }}", packed, call_str)
                        } else {
                            call_str
                        }
                    }
                } else if args_s.is_empty()
                    && self.is_known_type(&callee_s)
                    && !matches!(
                        callee_s.as_str(),
                        "Option" | "Result" | "Some" | "None" | "Ok" | "Err"
                    )
                {
                    // 空字段 struct 构造：Text() → Text {}
                    format!("{} {{}}", callee_s)
                } else if args_s.is_empty() {
                    // type alias 空构造：List()/Vec() → Vec::new()；Set()/HashSet() →
                    // HashSet::new()；Dict()/HashMap() → HashMap::new()（type alias
                    // 不能当函数调用，E0423 expected function, found type alias）
                    match callee_s.as_str() {
                        "List" | "Vec" => "Vec::new()".to_string(),
                        "Set" | "HashSet" => "std::collections::HashSet::new()".to_string(),
                        "Dict" | "HashMap" => "std::collections::BTreeMap::new()".to_string(),
                        _ => {
                            let call_str =
                                format!("{}{}({})", callee_s, turbofish, args_s.join(", "));
                            if let Some(ref packed) = unpack_packed {
                                format!("{{ let __t = {}; {} }}", packed, call_str)
                            } else {
                                call_str
                            }
                        }
                    }
                } else {
                    // `Err(self)` / `Ok(self)` 等变体构造：self 是 &Self 引用，
                    // 但变体需 owned 值，自动 clone（box.lz try_unwrap E0277/E0308）
                    let args_c: Vec<String> = if matches!(
                        callee_s.as_str(),
                        "Ok" | "Err" | "Some" | "None"
                    ) {
                        args_s
                            .iter()
                            .zip(args.iter())
                            .map(|(s, a)| {
                                // `self` 或 `self.xxx()`（get 返回 &T）→ clone 为 owned
                                let is_self_ref = matches!(&a.kind, ExprKind::Var(n) if n == "self" || n == "self_")
                                    || matches!(&a.kind, ExprKind::MethodCall { receiver, .. }
                                        if matches!(&receiver.kind, ExprKind::Var(n) if n == "self" || n == "self_"));
                                let s = if is_self_ref && !s.contains(".clone()") {
                                    format!("{}.clone()", s)
                                } else {
                                    s.clone()
                                };
                                self.clone_if_multiuse(s, a)
                            })
                            .collect()
                    } else {
                        args_s
                            .iter()
                            .zip(args.iter())
                            .map(|(s, a)| self.clone_if_multiuse(s.clone(), a))
                            .collect()
                    };
                    // 函数指针字段调用（如 `self.f(v)`）：需生成 `(self.f)(v)` 而非 `self.f(v)`
                    // （Rust 中 fn 类型字段是 fn item，直接调用需括号包裹转为 fn pointer）

                    let callee_is_fn_field = matches!(&callee.kind, ExprKind::FieldAccess { .. })
                        && matches!(&callee.ty, IrType::Fn { .. });
                    let call_str = if callee_is_fn_field {
                        format!("({})({})", callee_s, args_c.join(", "))
                    } else {
                        format!("{}{}({})", callee_s, turbofish, args_c.join(", "))
                    };
                    // f(f(x))：FnMut 类型变量嵌套调用自身（closure_capture.lz
                    // `f(f(x))`）需拆临时变量，否则 E0499 cannot borrow f as mutable
                    // more than once（外层调用仍借用 f 时内层调用再次可变借用）
                    let call_str = if let ExprKind::Var(fname) = &callee.kind {
                        if args.len() == 1
                            && matches!(&callee.ty, IrType::Fn { .. })
                            && matches!(&args[0].kind, ExprKind::Call { callee: c, .. }
                                if matches!(&c.kind, ExprKind::Var(n) if n == fname))
                        {
                            format!("{{ let __t = {}; {}(__t) }}", args_c[0], callee_s)
                        } else {
                            call_str
                        }
                    } else {
                        call_str
                    };
                    if let Some(ref packed) = unpack_packed {
                        format!("{{ let __t = {}; {} }}", packed, call_str)
                    } else {
                        call_str
                    }
                }
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let recv = self.gen_expr(receiver);
                // std 模块链调用拦截（BUG-SB-001 修复，2026-09-03）：
                // `time.Duration.fromMillis(1500)` 在 IR 是 MethodCall {
                //   receiver: FieldAccess { base: Var("time"), field: "Duration" },
                //   method: "fromMillis" } → 生成 `std::time::Duration::from_millis(1500)`。
                // 仅拦截已知 std 模块名（非用户变量），避免误伤同名局部变量。
                if let ExprKind::FieldAccess { base, field } = &receiver.kind {
                    if let ExprKind::Var(module) = &base.kind {
                        const STD_MODULES: &[(&str, &[&str])] = &[
                            ("time", &["Duration", "Instant"]),
                            ("thread", &["Thread", "JoinHandle"]),
                            ("fs", &["File", "Dir"]),
                            ("io", &["Stdout", "Stdin", "BufReader", "BufWriter"]),
                        ];
                        if let Some((rust_mod, types)) = STD_MODULES
                            .iter()
                            .find(|(lz_name, _)| *lz_name == module.as_str())
                        {
                            if types.contains(&field.as_str()) {
                                // camelCase 方法名 → snake_case（同方法映射表）
                                let rust_method = match method.as_str() {
                                    "fromMillis" => "from_millis",
                                    "toMillis" => "as_millis",
                                    "fromSecs" | "fromSeconds" => "from_secs",
                                    "fromMicros" => "from_micros",
                                    "fromNanos" => "from_nanos",
                                    "now" => "now",
                                    "elapsed" => "elapsed",
                                    _ => method.as_str(),
                                };
                                let args_s: Vec<String> =
                                    args.iter().map(|a| self.gen_expr(a)).collect();
                                return format!(
                                    "std::{}::{}::{}({})",
                                    rust_mod,
                                    field,
                                    rust_method,
                                    args_s.join(", ")
                                );
                            }
                        }
                    }
                }
                // 静态调用：`Type::method(args)`（Cell::new / Vec::new / HashMap::new 等）。
                // receiver 为已知类型名（本文件声明的类型或外部标准类型）时生成
                // `Type::method(...)`，否则走实例方法路径 `recv.method(...)`
                // （否则 Cell::new 被生成为 Cell.new → E0423）
                let static_type_call = match &receiver.kind {
                    ExprKind::Var(n) => {
                        const KNOWN_EXT_TYPES: &[&str] = &[
                            "Cell",
                            "RefCell",
                            "Vec",
                            "HashMap",
                            "HashSet",
                            "Rc",
                            "Arc",
                            "Result",
                            "Option",
                            "String",
                            "Mutex",
                            "Box",
                            "Ref",
                            "RefMut",
                            "OnceLock",
                            "BTreeMap",
                            "BTreeSet",
                            "BinaryHeap",
                            "LinkedList",
                            "VecDeque",
                            "i8",
                            "i16",
                            "i32",
                            "i64",
                            "isize",
                            "u8",
                            "u16",
                            "u32",
                            "u64",
                            "usize",
                            "f32",
                            "f64",
                            "bool",
                            "str",
                            "char",
                        ];
                        self.known_types.contains(n.as_str())
                            || KNOWN_EXT_TYPES.contains(&n.as_str())
                            // 枚举名（ParseError::UnexpectedChar 等变体构造）也走静态调用
                            || self.enum_variants.values().any(|e| e == n)
                    }
                    _ => false,
                };
                if static_type_call {
                    // struct::new 字段类型表（new 形参顺序即字段顺序）：fn 字段实参
                    // 为 lambda 时转 fn 指针（同 Call 分支，lib_iterator MapIter.new）
                    let struct_fields = self.struct_fields_info.get(&recv).cloned();
                    // 枚举变体构造器（ParseError::UnexpectedChar(...)）：
                    // 按 (Enum, Variant) 查字段类型表，为每个实参注入期望类型，
                    // 使 String 索引（s[pos]）生成 String 而非 i64（lib_json E0308）
                    let ctor_field_types = self
                        .enum_variant_fields
                        .get(&(recv.clone(), method.clone()))
                        .cloned();
                    let mut args_s: Vec<String> = args
                        .iter()
                        .enumerate()
                        .map(|(i, a)| {
                            let prev_expected = self.current_expected_ty.borrow().clone();
                            if let Some(types) = &ctor_field_types {
                                if i < types.len() {
                                    *self.current_expected_ty.borrow_mut() = Some(types[i].clone());
                                }
                            }
                            let s = self.gen_expr(a);
                            *self.current_expected_ty.borrow_mut() = prev_expected;
                            if let Some(fields) = &struct_fields {
                                if let Some((_, IrType::Fn { params, ret })) = fields.get(i) {
                                    if matches!(&a.kind, ExprKind::Lambda { .. }) {
                                        let ps: Vec<String> =
                                            params.iter().map(|pt| self.rust_type(pt)).collect();
                                        let type_text = format!(
                                            "fn({}) -> {}",
                                            ps.join(", "),
                                            self.rust_type(ret)
                                        );
                                        // 泛型占位符（T/U 或隐式小写 a/b）不能裸转 fn
                                        // 指针（E0425）；仅检测类型签名部分。
                                        let chars: Vec<char> = type_text.chars().collect();
                                        let has_placeholder =
                                            chars.iter().enumerate().any(|(i, c)| {
                                                c.is_ascii_alphabetic()
                                                    && (i == 0
                                                        || !chars[i - 1].is_ascii_alphanumeric())
                                                    && (i + 1 >= chars.len()
                                                        || !chars[i + 1].is_ascii_alphanumeric())
                                            });
                                        if !has_placeholder {
                                            return format!("({}) as {}", s, type_text);
                                        }
                                    }
                                }
                            }
                            s
                        })
                        .collect();
                    // 默认参数处理：静态方法调用（Type::method）有默认参数时补 None
                    // 键用 Type::method 格式（与 gen_struct_def 中的登记格式一致）
                    let method_key = format!("{}::{}", recv, method);
                    if let Some(&(total_params, def_count)) = self.fn_param_info.get(&method_key) {
                        let required = total_params - def_count;
                        if args_s.len() < required {
                            while args_s.len() < required {
                                args_s.push("/* missing arg */".to_string());
                            }
                        }
                        // 补默认参数：将显式传入的后几个参数包裹在 Some() 中
                        let explicit_default_args = if args_s.len() > required {
                            args_s.len() - required
                        } else {
                            0
                        };
                        for i in required..args_s.len() {
                            let arg_idx = i - required;
                            if arg_idx < explicit_default_args {
                                args_s[i] = format!("Some({})", args_s[i]);
                            }
                        }
                        // 补 None 填充未提供的默认参数
                        while args_s.len() < total_params {
                            args_s.push("None".to_string());
                        }
                    }
                    return if args_s.is_empty() {
                        // 无数据变体的调用式构造 `Shape.Point()` → `Shape::Point`：
                        // Rust 的单元变体不是函数，带 `()` 即 E0618（2026-10-02 实测
                        // CY/TESTS/07_data_structures/enum_data.lz 走 rust 后端 rustc 失败，
                        // 同一份 .lz 在 cy 后端能跑 ⇒ 跨后端分叉）。
                        // 本分支是 `Type.method(...)` 的**静态调用**路线，早于 FieldAccess
                        // 分支返回，所以两处都要拦（只改 FieldAccess 那处实测无效）。
                        // 限定「method 确是 recv 的变体」且「该变体零字段」，
                        // 否则 Cell::new() / Vec::new() 这类零参关联函数会被摘掉括号。
                        let unit_variant_ctor = self
                            .enum_variants
                            .get(method.as_str())
                            .map_or(false, |en| en.as_str() == recv.as_str())
                            && self
                                .enum_variant_fields
                                .get(&(recv.clone(), method.clone()))
                                .map_or(true, |v| v.is_empty());
                        if unit_variant_ctor {
                            format!("{}::{}", recv, method)
                        } else {
                            format!("{}::{}()", recv, method)
                        }
                    } else {
                        // 命名字段枚举变体构造：Type.Variant(field: v) → Type::Variant { field: v }
                        // static_type_call 早于 is_enum_variant 分支返回，必须在此处理，
                        // 否则 struct variant 用 tuple 语法构造报 E0533
                        let named_fields = self
                            .enum_variant_named_fields
                            .get(&(recv.clone(), method.clone()))
                            .cloned()
                            .unwrap_or_default();
                        let field_types = self
                            .enum_variant_fields
                            .get(&(recv.clone(), method.clone()));
                        let boxed_args: Vec<String> = args_s
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                // 递归字段（enum 变体字段类型引用自身）自动 Box::new()
                                let needs_box = field_types.map_or(false, |types| {
                                    types.get(i).map_or(false, |ty| type_refers_to(ty, &recv))
                                });
                                if needs_box {
                                    format!("Box::new({})", a)
                                } else {
                                    // 非 Copy 类型字段 + 变量参数 → 自动 clone 避免 move 后借用报错
                                    let field_is_noncopy = field_types
                                        .as_ref()
                                        .and_then(|types| types.get(i))
                                        .map_or(false, |ty| match ty {
                                            IrType::Str => true,
                                            IrType::Named { path, args: _ } => matches!(
                                                path.as_str(),
                                                "String" | "Vec" | "HashMap" | "Box"
                                            ),
                                            _ => false,
                                        });
                                    if field_is_noncopy
                                        && matches!(
                                            args.get(i).map(|a| &a.kind),
                                            Some(ExprKind::Var(_) | ExprKind::FieldAccess { .. })
                                        )
                                    {
                                        format!("{}.clone()", a)
                                    } else {
                                        a.clone()
                                    }
                                }
                            })
                            .collect();
                        if !named_fields.is_empty() && named_fields.len() == args_s.len() {
                            let pairs: Vec<String> = named_fields
                                .iter()
                                .zip(boxed_args.iter())
                                .map(|(f, a)| format!("{}: {}", f, a))
                                .collect();
                            format!("{}::{} {{ {} }}", recv, method, pairs.join(", "))
                        } else {
                            format!("{}::{}({})", recv, method, boxed_args.join(", "))
                        }
                    };
                }
                // self.iter.next()（方法调用借用 self 字段推进迭代器）：gen_expr 的
                // self 字段访问生成 self.iter.clone()——clone 后 next 不推进原 iter，
                // collect 无限迭代死循环（traits.lz Enumerate 的 __next__），去掉 .clone()
                let recv = if recv.starts_with("self.") && recv.ends_with(".clone()") {
                    recv.trim_end_matches(".clone()").to_string()
                } else {
                    recv
                };
                let mut args_s: Vec<String> = args
                    .iter()
                    .map(|a| {
                        let s = self.gen_expr(a);
                        // fn 值实参（Arc 载体）→ 转发闭包：链式 `xs.map(f)`/`filter(f)` 的
                        // 形参要 FnMut，`Arc<dyn Fn … + Send + Sync>` 不满足（见 fn_value_fwd）
                        if let Some(fwd) = self.fn_value_fwd(&s, a) {
                            return fwd;
                        }
                        self.clone_if_multiuse(s, a)
                    })
                    .collect();
                // 用户导入模块（含别名）的方法调用 m.add(...) → add(...)：
                // 模块项已平铺生成到同一 Rust 文件，直接调用 method 即可
                // （避免走方法调用路径把 add 误映射成 insert）
                if matches!(&receiver.kind, ExprKind::Var(base_name)
                    if self.imported_modules.contains(base_name.as_str()))
                {
                    return if args_s.is_empty() {
                        format!("{}()", method)
                    } else {
                        format!("{}({})", method, args_s.join(", "))
                    };
                }
                // ref 参数：调用点自动 &x（DictExt::get 的 key: ref K → d.get(&key)，
                // 否则 expected &_, found String E0308）
                if let Some(ref_flags) = self.fn_ref_params.get(method.as_str()).cloned() {
                    for (i, _a) in args.iter().enumerate() {
                        // 跳过 self 参数：fn_ref_params[0] 是 self 标记，方法调用
                        // args 不含 self（Dict::remove 的 key: ref K → d.remove(&key)）
                        let fi = i + 1;
                        if fi >= ref_flags.len() || i >= args_s.len() {
                            break;
                        }
                        let (is_ref, is_mut) = ref_flags[fi];
                        if is_ref && !args_s[i].starts_with('&') {
                            args_s[i] = if is_mut {
                                format!("&mut {}", args_s[i])
                            } else {
                                format!("&{}", args_s[i])
                            };
                        }
                    }
                }

                // 关联类型路径上的方法调用（iter.lz `I::Item.default()`）：
                // receiver 是 `泛型参数.大写字段`（如 I::Item），方法应生成
                // `I::Item::default()`（关联函数），否则 E0599 no associated
                // function or constant named `Item` found for type parameter `I`
                let recv_is_assoc_path = matches!(
                    &receiver.kind,
                    ExprKind::FieldAccess { base, field }
                        if matches!(&base.kind, ExprKind::Var(n) if n != "self")
                            && field.chars().next().map_or(false, |c| c.is_uppercase())
                            && !self.known_types.contains(field.as_str())
                            // 关联类型路径（I::Item::default()）只出现在泛型函数/impl 中；
                            // 非泛型上下文里 `Ordering.Less.is_lt()` 是枚举变体方法调用，
                            // 误判会生成 `Ordering.Less::is_lt()`（E0601 语法错误）
                            && (self.in_generic_fn
                                || self.in_impl_generic
                                || !self.param_renames.is_empty())
                );
                if recv_is_assoc_path {
                    let assoc_sep = "::";
                    // 方法调用用 :: 连接（I::Item::default()）
                    return if args.is_empty() {
                        format!("{}{}{}()", recv, assoc_sep, method)
                    } else {
                        format!("{}{}{}({})", recv, assoc_sep, method, args_s.join(", "))
                    };
                }

                // 类型参数 receiver 的关联函数调用（collect 的 `C.from_iter(self)`）：
                // C 是类型参数（大写），方法调用用 ::（C::from_iter），否则 E0423
                // expected value, found type parameter C
                if let ExprKind::Var(n) = &receiver.kind {
                    let is_type_param = n != "self"
                        && n.chars().next().map_or(false, |c| c.is_uppercase())
                        && !self.known_types.contains(n.as_str())
                        && !self.emitted_types.contains(n.as_str())
                        && !self.global_vars.contains_key(n.as_str())
                        && !self.downgraded_vars.contains(n.as_str());
                    if is_type_param {
                        return if args.is_empty() {
                            format!("{}::{}({})", recv, method, args_s.join(", "))
                        } else {
                            format!("{}::{}({})", recv, method, args_s.join(", "))
                        };
                    }
                }

                // await: x.await() → x.await (Rust postfix keyword)
                if method == "await" {
                    return format!("({}).await", recv);
                }

                // clone 方法：&str 类型需要 to_string() 而非 clone()
                if method == "clone" {
                    let recv_is_str = matches!(&receiver.ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str))
                        || matches!(&receiver.ty, IrType::Str);
                    if recv_is_str {
                        return format!("{}.to_string()", recv);
                    }
                }

                // parse_f64: String.parse_f64() -> parse::<f64>().unwrap()
                if method == "parse_f64" {
                    return format!("({}).parse::<f64>().unwrap()", recv);
                }

                // size_hint：std Iterator 返回 (usize, Option<usize>)，LZ 语义是
                // (int, Option<int>)（iter.lz Zip::size_hint 中 `self.a.size_hint()`），
                // 解包后转 i64（LZ int 语义，供解构/运算）
                if (method == "size_hint" || method == "__size_hint__")
                    && self.current_fn_is_size_hint
                {
                    let call = format!("{}.size_hint()", recv);
                    return format!(
                        "{{ let __t = {}; (__t.0 as i64, __t.1.map(|v| v as i64)) }}",
                        call
                    );
                }

                // null coalesce: a ?? b → .or() 或 .unwrap_or()
                if method == "__null_coalesce" && !args.is_empty() {
                    let arg_is_option = matches!(&args[0].ty, IrType::Option(_))
                        || matches!(&args[0].ty, IrType::Named { path, .. } if path == "Option");
                    return if arg_is_option {
                        format!("{}.or({})", recv, args_s[0])
                    } else {
                        format!("{}.unwrap_or({})", recv, args_s[0])
                    };
                }

                // try_into (the ? operator): convert to Result::unwrap() for now
                // In the future, this should emit ? operator when in a Result-returning context
                if method == "try_into" {
                    // 自定义传播类型（实现 __is_ok__/__unwrap__/__err__ 的 struct，
                    // 如 spread_protocol.lz 的 HttpResult）：生成 is_ok 判定 + 失败
                    // panic(err) + 成功解包，语义与 Result.unwrap 等价
                    let recv_is_custom = matches!(&receiver.ty, IrType::Named { path, .. }
                        if self.is_known_type(path)
                            && self.struct_method_names(path).contains("__is_ok__"));
                    if recv_is_custom {
                        return format!(
                            "{{ if !{0}.__is_ok__() {{ panic!(\"{{:?}}\", {0}.__err__()); }} {0}.__unwrap__() }}",
                            recv
                        );
                    }
                    // 当前函数返回 Option/Result 时，`?` 用 Rust 原生传播语义：
                    // `a?` → `a?`（None/Err 直接 return 传播），而非 .unwrap() panic。
                    // （combo-error-control.lz unwrap_add: let va = a? → va = a?）
                    // 也包括 raises 函数（非 try/catch），让 ? 在纯函数中生效
                    let ret_is_result_like = self
                        .current_ret_ty
                        .as_ref()
                        .map(|rt| {
                            matches!(rt, IrType::Option(_) | IrType::Result { .. })
                                || matches!(rt, IrType::Named { path, .. }
                                    if path == "Option" || path == "Result")
                        })
                        .unwrap_or(false);
                    if ret_is_result_like || self.current_fn_raises.is_some() {
                        // BUG-spread：返回 Result/raises 的上下文中对 Option 用 `?`
                        // 必须先转 Result（E0277：? 不能作用于 Option）。
                        // 但若函数本身返回 Option，`Option?` 直接传播 None，合法，不转换。
                        let recv_is_option = matches!(&receiver.ty, IrType::Option(_))
                            || matches!(&receiver.ty, IrType::Named { path, .. }
                                if path == "Option");
                        let ret_is_result = self.current_fn_raises.is_some()
                            || self.current_ret_ty.as_ref().map_or(false, |rt| {
                                matches!(rt, IrType::Result { .. })
                                    || matches!(rt, IrType::Named { path, .. } if path == "Result")
                            });
                        if recv_is_option && ret_is_result {
                            return format!(
                                "({}).ok_or_else(|| LzError::new(\"None\".to_string()))?",
                                recv
                            );
                        }
                        return format!("({})?", recv);
                    }
                    return format!("{}.unwrap()", recv);
                }

                // Enum variant 构造: Type.Variant(kwargs...) → Type::Variant(val1, val2, ...)
                // 生成位置参数构造（与 tuple variant 定义一致）
                let is_enum_variant = (self.is_known_type_or_enum(&recv)
                    || matches!(recv.as_str(), "Option" | "Result"))
                    && is_kwarg_call(args);
                if is_enum_variant {
                    let field_types = self
                        .enum_variant_fields
                        .get(&(recv.clone(), method.clone()));
                    let named = self
                        .enum_variant_named_fields
                        .get(&(recv.clone(), method.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if named.len() == args.len() && !named.is_empty() {
                        // 命名字段变体: E.X(v: 42) → E::X { v: 42 }
                        let pairs: Vec<String> = args
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                let val = gen_kwarg_value(a, self);
                                let needs_box = field_types.as_ref().map_or(false, |types| {
                                    types.get(i).map_or(false, |ty| type_refers_to(ty, &recv))
                                });
                                let val = if needs_box {
                                    format!("Box::new({})", val)
                                } else {
                                    val
                                };
                                format!("{}: {}", named[i], val)
                            })
                            .collect();
                        return format!("{}::{} {{ {} }}", recv, method, pairs.join(", "));
                    }
                    let values: Vec<String> = args
                        .iter()
                        .enumerate()
                        .map(|(i, a)| {
                            let val = gen_kwarg_value(a, self);
                            let needs_box = field_types.as_ref().map_or(false, |types| {
                                types.get(i).map_or(false, |ty| type_refers_to(ty, &recv))
                            });
                            if needs_box {
                                format!("Box::new({})", val)
                            } else {
                                val
                            }
                        })
                        .collect();
                    return format!("{}::{}({})", recv, method, values.join(", "));
                }
                // Enum 类型调用变体: Status.Pending("x") → Status::Pending("x")
                // Also: Option.Some(42) → Option::Some(42)
                if self.is_known_type_or_enum(&recv) || matches!(recv.as_str(), "Option" | "Result")
                {
                    let field_types = self
                        .enum_variant_fields
                        .get(&(recv.clone(), method.clone()));
                    let wrapped_args: Vec<String> = args_s
                        .iter()
                        .enumerate()
                        .map(|(i, a)| {
                            let needs_box = field_types.as_ref().map_or(false, |types| {
                                types.get(i).map_or(false, |ty| type_refers_to(ty, &recv))
                            });
                            if needs_box {
                                format!("Box::new({})", a)
                            } else {
                                a.clone()
                            }
                        })
                        .collect();
                    // Option::None（无参变体）：注入类型参数，避免闭包返回位置
                    // 无法推断 T（E0282，如 opt.and_then(|x| Option.None)）
                    if method == "None" && wrapped_args.is_empty() && recv == "Option" {
                        let elem = match &expr.ty {
                            IrType::Named { path, args } if path == "Option" && args.len() == 1 => {
                                self.rust_type(&args[0])
                            }
                            _ => "i64".to_string(),
                        };
                        // 泛型函数（如 `def map<R>(...) = Container(data: match self.data:
                        //   case Option.Some(value: v) => Option.Some(value: f(v))
                        //   case Option.None => Option.None)`）中硬编码 i64 错误：
                        // Option::None 让 Rust 从 match 臂配对推断（combo-struct-method.lz E0308）；
                        // 且用户自定义 `enum Option<T>` 遮蔽 std Option 时裸 None 类型不匹配（enum.lz）
                        if self.in_generic_fn {
                            return "Option::None".to_string();
                        }
                        return format!("Option::<{}>::None", elem);
                    }
                    // 命名字段变体的位置参数构造: Option.Some(42) → Option::Some { value: 42 }
                    // （E0533: struct variant 不能用 tuple 语法构造）
                    let named_fields = self
                        .enum_variant_named_fields
                        .get(&(recv.clone(), method.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if !named_fields.is_empty() && named_fields.len() == wrapped_args.len() {
                        let pairs: Vec<String> = named_fields
                            .iter()
                            .zip(wrapped_args.iter())
                            .map(|(f, a)| format!("{}: {}", f, a))
                            .collect();
                        return format!("{}::{} {{ {} }}", recv, method, pairs.join(", "));
                    }
                    return format!("{}::{}({})", recv, method, wrapped_args.join(", "));
                }

                // 判断 receiver 是否为用户自定义 struct（有对应魔术方法时用魔术方法名）
                let recv_is_struct =
                    matches!(&receiver.ty, IrType::Named { path, .. } if self.is_known_type(path));
                // 用户 struct 的同名普通方法优先于魔术方法映射：
                // lib_hashmap 的 HashMap.len(self) 是普通方法，映射成 __len__ 会 E0599
                let user_plain = match &receiver.ty {
                    IrType::Named { path, .. } if self.is_known_type(path) => {
                        self.struct_method_names(path)
                    }
                    _ => std::collections::HashSet::new(),
                };

                // LZ magic methods → Rust equivalents
                // plus common method name mappings
                // 注意：算术/比较魔术方法（__add__/__eq__ 等）保留原名，
                // 因为用户 struct 的 impl 方法就叫 __add__；__str__/__iter__ 用于
                // str() 转换和迭代的容器场景，继续映射
                let rust_method = match method.as_str() {
                    // 用户 struct：len/iter/next/contains 等映射到魔术方法；
                    // 但同名普通方法存在时保持原名（普通方法优先）
                    "len" if recv_is_struct && !user_plain.contains("len") => "__len__",
                    "iter" if recv_is_struct && !user_plain.contains("iter") => "__iter__",
                    "next" if recv_is_struct && !user_plain.contains("next") => "__next__",
                    "getitem" if recv_is_struct && !user_plain.contains("getitem") => "__getitem__",
                    "setitem" if recv_is_struct && !user_plain.contains("setitem") => "__setitem__",
                    "contains" if recv_is_struct && !user_plain.contains("contains") => {
                        "__contains__"
                    }
                    // impl Iterator 块内调用迭代器元素上的迭代方法：
                    // `self.a.__next__()`（A: Iterator 为 std trait，方法是 next）→ .next()
                    // 泛型 receiver（Peekable 的 self.iter.__next__()，I 非已知 struct）
                    // 也映射 next（E0599 no method __next__ on type parameter I）
                    "__next__" if self.in_iterator_impl || !recv_is_struct => "next",
                    "__size_hint__" if self.in_iterator_impl => "size_hint",
                    // 非用户 struct 的 __str__/__iter__ 用于内置容器/字符串场景
                    // self.__str__()（trait 默认方法，如 Error::description）保留方法
                    // 调用（self 实现 LZ Display trait），映射 to_string 需 std Display
                    // duck 约束接收者（x: Printable，TY-001）也保留 __str__ 调用：
                    // 接收者是泛型参数，std to_string 需 Display bound，直接 E0599
                    "__str__"
                        if !recv_is_struct
                            && recv != "self"
                            && !matches!(&receiver.ty, IrType::Named { path, .. }
                            if self.duck_defs.contains_key(path.as_str())) =>
                    {
                        "to_string"
                    }
                    "__iter__" if !recv_is_struct => "iter",
                    "length" => "len", // LZ .length() → Rust .len()
                    "to_upper" => "to_uppercase",
                    "to_lower" => "to_lowercase",
                    // string.lz to_lower/to_upper 内部调用 self.lower()/self.upper()
                    // （编译器映射标记）：lower/upper 映射到 std to_lowercase/to_uppercase
                    "lower" => "to_lowercase",
                    "upper" => "to_uppercase",
                    "push" | "append" => "push",
                    // add → insert 仅在 receiver **无自定义 add 方法**时映射：
                    // - set_tuple.lz 的 {1,2} 字面量是原生 HashSet（无 add）→ insert
                    // - lz_std/set.lz 的 Set 扩展已提供自定义 add（struct_method_names
                    //   含 add）→ 保留 add，否则破坏其调用（返回 bool 与语句级
                    //   if 的 else () 类型不兼容 E0308）
                    "add"
                        if !(matches!(&receiver.ty, IrType::Named { path, .. }
                        if self.struct_method_names(path).contains("add"))) =>
                    {
                        "insert"
                    }
                    "insert" | "insert_at" => "insert",
                    "remove" => "remove",
                    "pop" => "pop",
                    "sort" => "sort",
                    "reverse" => "reverse",
                    "contains" => {
                        // HashMap/Dict → contains_key; String/Vec → contains
                        let is_dict = matches!(&receiver.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                        // 也检查是否为 kwargs 字段（__Params 的 kwargs 是 HashMap）
                        let is_kwargs = matches!(&receiver.kind, ExprKind::FieldAccess { field, .. } if field == "kwargs");
                        if is_dict || is_kwargs {
                            "contains_key"
                        } else {
                            "contains"
                        }
                    }
                    "slice" => {
                        // LZ .slice(a, b) → StringExt/ListExt::lz_slice（E0599 修复）
                        // 仅内置 str/List 接收者映射；用户 struct 自定义 slice 保留
                        let is_str_like = matches!(&receiver.ty, IrType::Named { path, .. }
                            if path == "str" || path == "String")
                            || matches!(&receiver.ty, IrType::Str);
                        let is_list_like = matches!(&receiver.ty, IrType::Named { path, .. }
                            if path == "List" || path == "Vec" || path == "list");
                        if is_str_like || is_list_like {
                            "lz_slice"
                        } else {
                            "slice"
                        }
                    }
                    "split" => "split",
                    "join" => "join",
                    "replace" => "replace",
                    "trim" => "trim",
                    "starts_with" => "starts_with",
                    "ends_with" => "ends_with",
                    // StdBridge camelCase 别名接线（BUG-SB-001/003 修复，2026-09-03）：
                    // bridge/std.rs resolve_method 已有完整映射（含单测），
                    // 此前 IR codegen 方法表未接入 → startsWith 原样透传 E0599。
                    // 仅在 receiver 非用户 struct 时映射（用户自定义同名方法优先）。
                    "startsWith" if !recv_is_struct => "starts_with",
                    "endsWith" if !recv_is_struct => "ends_with",
                    "isEmpty" if !recv_is_struct => "is_empty",
                    "fromMillis" => "from_millis",
                    "toMillis" => "as_millis",
                    "fromSecs" | "fromSeconds" => "from_secs",
                    "fromMicros" => "from_micros",
                    "new"
                        if self.emitted_types.contains(&recv)
                            || recv == "Box"
                            || recv == "Rc"
                            || recv == "Arc" =>
                    {
                        // Static ctor on type → use :: syntax.
                        // 魔法构造器是 `__new__`（struct 体由 struct_has_new 登记，impl 块由
                        // struct_method_names_map 含 "__new__" 判定），普通 impl `new` 才是 `new`。
                        let is_magic_new = self.struct_has_new.contains(&recv)
                            || self
                                .struct_method_names_map
                                .get(&recv)
                                .map_or(false, |m| m.contains("__new__"));
                        let ctor = if is_magic_new { "__new__" } else { "new" };
                        return format!("{}::{}({})", recv, ctor, args_s.join(", "));
                    }
                    _ => method,
                };
                // String Pattern trait方法 + 集合contains等需要引用的方法
                // String Pattern trait方法 + 集合contains等需要引用的方法
                let pattern_methods = [
                    "starts_with",
                    "ends_with",
                    "find",
                    "rfind",
                    "replace",
                    "trim_start_matches",
                    "trim_end_matches",
                    "contains",
                    "contains_key",
                    "split",
                    "rsplit",
                    "splitn",
                    "rsplitn",
                    "get",
                    "remove",
                ];
                // BUG-SB-003（2026-09-03）：判定用映射后的 rust_method——
                // camelCase 别名（startsWith → starts_with）也需走 &str 改写。
                if (pattern_methods.contains(&method.as_str())
                    || pattern_methods.contains(&rust_method))
                    && !args_s.is_empty()
                {
                    // 仅对 str/String receiver 应用（String Pattern trait 方法）：
                    // 自定义类型的 get（list.lz `lst.get(1)` 参数是 i64）不受影响
                    let recv_is_str = matches!(&receiver.ty, IrType::Str)
                        || matches!(&receiver.ty, IrType::Named { path, .. }
                            if path == "str" || path == "String")
                        || matches!(&receiver.ty, IrType::Ref(inner) | IrType::MutRef(inner)
                            if matches!(inner.as_ref(), IrType::Str)
                                || matches!(inner.as_ref(), IrType::Named { path, .. }
                                    if path == "str" || path == "String"));
                    // HashMap/Dict 的 contains_key：key 需 &（HashMap::contains_key 参数 &Q）
                    // Set/HashSet 的 contains：参数 &Q（containers.lz `tags.contains("rust")`，
                    // E0308 expected &_, found String）；kwargs 字段（checker 的 __Params.kwargs
                    // 是 HashMap）同 dict；HashMap/Dict 的 get 也需 &Q（operators.lz
                    // `__sn.get("key")`，E0308 expected &_, found String）
                    let recv_is_dict = matches!(&receiver.ty, IrType::Named { path, .. }
                        if path == "Dict" || path == "HashMap");
                    let recv_is_set = matches!(&receiver.ty, IrType::Named { path, .. }
                        if path == "Set" || path == "HashSet");
                    // Vec/List 的 contains：参数需 &T（Vec::contains 签名 &T，
                    // `[1,2,3].contains(2)` → E0308 expected &i64, found i64）。
                    // 注意：仅当 receiver **无自定义 contains 方法**时才加 &——
                    // lz_std/list.lz 自定义 contains(ref self, value: T) 参数为值
                    // （T 非引用），误加 & 会 E0308 expected i64, found &i64
                    let recv_is_vec = matches!(&receiver.ty, IrType::Named { path, .. }
                        if path == "List" || path == "Vec");
                    // 自定义 contains 判定：直接查该类型名下已收集的方法集合（struct
                    // 方法 + impl 块方法均已并入 struct_method_names_map）。
                    // 原实现额外要求 is_known_type(path)，但 `impl List<T>` 这类扩展
                    // trait 类型既不进 known_types/emitted_types 也不进 impl_types，
                    // 导致误判为「无自定义 contains」而加 & → E0308
                    // （lz_std/list.lz `lst.contains(20)` → `lst.contains(&20i64)`）。
                    let recv_has_custom_contains = matches!(&receiver.ty, IrType::Named { path, .. }
                        if self.struct_method_names(path).contains("contains"));
                    let is_kwargs = matches!(&receiver.kind, ExprKind::FieldAccess { field, .. }
                        if field == "kwargs");
                    let need_ref = recv_is_str
                        // Vec contains：receiver 类型为 List/Vec 且无自定义 contains
                        // 时加 &（std Vec::contains 需 &T）。此前仅限定 ListLit
                        // 字面量 receiver（BUG-SB-002 修复，2026-09-03）：变量
                        // receiver `v.contains(2)` 同样走 std Vec::contains，E0308
                        // expected &i64, found i64；lz_std 自定义 contains 的场景
                        // 由 recv_has_custom_contains 守卫排除。
                        || (recv_is_vec && method == "contains" && !recv_has_custom_contains)
                        || (recv_is_dict && (rust_method == "contains_key" || method == "get" || method == "remove"))
                        || (recv_is_set && (method == "contains" || method == "remove"))
                        || (is_kwargs && (rust_method == "contains_key" || method == "get"))
                        // closure 参数 receiver（operators.lz `__sn.get("key")`，__sn 类型
                        // 推断为 Any）：get 的实参是 String 时必为 HashMap::get（需 &Q），
                        // Vec::get 的实参是 int（走 usize 转换），不会误伤
                        || (method == "get"
                            && (matches!(&args[0].ty, IrType::Str)
                                || matches!(&args[0].ty, IrType::Named { path, .. }
                                    if path == "str" || path == "String")))
                        // 链式调用 receiver 类型为 Any 时（如 s.replace(...).replace(...)），
                        // 第一次 replace 返回 String（IR 推断为 Any），第二次 replace 的
                        // receiver 是 Any 导致 need_ref 为 false → 参数未改写为 &str → E0277/E0308。
                        // 当 method 是 Pattern 方法且参数含字符串字面量时，强制改写。
                        || (matches!(&receiver.ty, IrType::Any)
                            && (pattern_methods.contains(&method.as_str())
                                || pattern_methods.contains(&rust_method))
                            && args.iter().any(|a| matches!(&a.kind, ExprKind::Lit(LitKind::Str(_)))));
                    if need_ref {
                        // String Pattern 方法（replace/starts_with/split 等）的**所有**
                        // 字符串参数都需 &str：`s.replace("-", "+")` 的 from/to 两个参数。
                        // 旧实现只处理 args_s[0]，第二个字符串字面量参数被包装成
                        // "+".to_string()（String）→ E0308 expected &str。
                        // Vec/List 的 contains 参数需 &T（含 int：`[1,2,3].contains(2)`
                        // → E0308 expected &i64）。只处理字符串字面量（直接用 &str）、
                        // 字符串类型值（加 &）与 Vec contains 的任意参数（加 &），
                        // 不误伤 Vec::get 的 usize 索引参数。
                        // BUG-SB-002（2026-09-03）：变量 receiver（v.contains(2)）同样
                        // 走 std Vec::contains，去掉 ListLit 限定；自定义 contains 由
                        // 上方 need_ref 的 recv_has_custom_contains 守卫排除。
                        let is_vec_contains = recv_is_vec && method == "contains";
                        // Set/HashSet 的 remove 参数需 &Q（HashSet::remove 签名 &Q，
                        // `ms.remove(2)` → E0308 expected &i64, found i64）
                        let is_set_remove = recv_is_set && method == "remove";
                        // Set/HashSet 的 contains 参数也需 &Q（containers.lz
                        // `numbers.contains(3)` → E0308 expected &i64, found i64）
                        let is_set_contains = recv_is_set && method == "contains";
                        // Dict/HashMap 的 contains_key 参数需 &Q（list.lz unique
                        // `seen.contains_key(item)` → E0308 expected &_, found T）
                        let is_dict_contains_key = recv_is_dict && method == "contains_key";
                        for (idx, arg_expr) in args.iter().enumerate() {
                            // 用户 struct 的同名普通方法（如 lib_hashmap 的 get(key: str)
                            // → String 形参）：不做 &str/& 实参改写，保持原 String 语义。
                            // 但 str/String 接收者除外：扩展 trait（StrExt）的同名方法是
                            // trait 方法，压不过原生 inherent `str::starts_with`（参数需
                            // Pattern），解析仍落到原生方法 → 仍需改写实参加 &。
                            // （string.lz `self.starts_with(other.clone())`，
                            //  否则 E0277 `String: Pattern` 不满足）
                            if user_plain.contains(method.as_str()) && !recv_is_str {
                                continue;
                            }
                            if let Some(slot) = args_s.get_mut(idx) {
                                // 实参是否「字符串类」：取 &String/&str 解引用后的内层类型，
                                // 并覆盖 IR 类型退化但其为 str/String 上 .clone() 的情形
                                let inner_ty: &IrType = match &arg_expr.ty {
                                    IrType::Ref(i) | IrType::MutRef(i) => i,
                                    t => t,
                                };
                                let arg_is_str_like = matches!(inner_ty, IrType::Str)
                                    || matches!(inner_ty, IrType::Named { path, .. }
                                        if path == "str" || path == "String")
                                    || Self::expr_is_clone_of_string(arg_expr);
                                if let ExprKind::Lit(LitKind::Str(s)) = &arg_expr.kind {
                                    // 字符串字面量参数直接生成 &str 字面量（避免 .to_string()
                                    // 与 E0308），但必须 escape_default：`"\\"` 若直接
                                    // format!("\"{}\"", s) 会生成 `"\`（json.lz E0308/语法错误）
                                    *slot = format!("\"{}\"", s.escape_default());
                                } else if !slot.starts_with('&')
                                    && (is_vec_contains
                                        || is_set_remove
                                        || is_set_contains
                                        || is_dict_contains_key
                                        || arg_is_str_like)
                                    && (!matches!(&arg_expr.ty, IrType::Ref(_) | IrType::MutRef(_))
                                        // 引用实参被 codegen 追加 .clone() 后已变 owned
                                        // （&String → String），仍需补 & 才能满足 Pattern：
                                        // string.lz `self.starts_with(other.clone())`
                                        // 否则 E0277 `String: Pattern` 不满足
                                        || slot.ends_with(".clone()"))
                                {
                                    *slot = format!("&{}", slot);
                                }
                            }
                        }
                    }
                }
                // 算术/比较魔术方法（__add__/__eq__ 等）取 &self + owned 参数，
                // 调用方需 clone 以避免 move 复用的变量。
                // 注意：__eq__/__ne__ 等比较方法（签名 `fn __eq__(&self, other: &Self)`）
                // 参数已是引用（box.lz `assert a == b` → a.__eq__(&b)），不能 clone
                // 参数（`(&b).clone()` 会调用 Box::clone 返回 owned Box，E0308）
                let non_consuming_magic = [
                    "__add__", "__sub__", "__mul__", "__div__", "__lt__", "__gt__", "__le__",
                    "__ge__", "__eq__", "__ne__",
                ];
                let is_compare_magic = matches!(method.as_str(), "__eq__" | "__ne__");
                // __eq__/__ne__ 参数为 ref（box.lz `ref other: Box<T>`）时不 clone
                // （`(&b).clone()` 调用 Box::clone 返回 owned，E0308）；参数为 owned
                // （magic_methods.lz `other: Vector`）时需 clone（`v1 == v1` → E0505
                // cannot move out of v1 because it is borrowed）
                let compare_arg_is_ref = is_compare_magic
                    && self
                        .fn_ref_params
                        .get(method.as_str())
                        .and_then(|f| f.get(1))
                        .map_or(false, |(is_ref, _)| *is_ref);
                if non_consuming_magic.contains(&method.as_str())
                    && recv_is_struct
                    && !compare_arg_is_ref
                {
                    let recv_c = format!("({}).clone()", recv);
                    // 参数生成：ref 参数需传引用（`&(b).clone()` 借用临时克隆，避免 move
                    // 原变量，修复 vector.lz `__add__(ref self, ref other)` 的 E0308）；
                    // owned 参数 clone；数值标量直接传值
                    let mut args_c: Vec<String> = Vec::with_capacity(args_s.len());
                    for (i, (s, a)) in args_s.iter().zip(args.iter()).enumerate() {
                        let is_ref_arg = self
                            .fn_ref_params
                            .get(method.as_str())
                            .and_then(|f| f.get(i + 1))
                            .map_or(false, |(is_ref, _)| *is_ref);
                        let is_scalar = matches!(&a.ty, IrType::Int | IrType::F64 | IrType::Bool);
                        if is_ref_arg {
                            if is_scalar {
                                args_c.push(format!("&{}", s));
                            } else if s.starts_with('&') {
                                // 前置 fn_ref_params 自动 & 已加 &（&b），直接复用借用，
                                // 避免 `&(&b).clone()` 双重引用
                                args_c.push(s.clone());
                            } else {
                                args_c.push(format!("&({}).clone()", s));
                            }
                        } else if is_scalar {
                            args_c.push(s.clone());
                        } else {
                            args_c.push(format!("({}).clone()", s));
                        }
                    }
                    // 默认参数处理：检查方法是否有默认参数，如有则补 None
                    let recv_ty_name = match &receiver.ty {
                        IrType::Named { path, .. } => path.as_str(),
                        _ => "",
                    };

                    if let Some(&(total_params, def_count)) = self
                        .struct_method_names_map
                        .get(recv_ty_name)
                        .and_then(|_| self.fn_param_info.get(method.as_str()))
                    {
                        let required = total_params - def_count;
                        if args_c.len() < required {
                            while args_c.len() < required {
                                args_c.push("/* missing arg */".to_string());
                            }
                        }
                        while args_c.len() < total_params {
                            args_c.push("None".to_string());
                        }
                    }
                    let call = format!("{}.{}({})", recv_c, rust_method, args_c.join(", "));
                    return call;
                }
                // Vec::insert/remove/get 需要 usize 索引（LZ int 是 i64）：
                // `self.insert_at(index, value)`（编译器映射标记，list.lz insert 方法）、
                // remove_at 的 `self.remove(index)`（std Vec::remove 语义）与
                // parts.get(i)（Vec::get）首个 int 参数需转 usize（E0308/E0277）。
                // 已自动 & 的参数（ref 参数，如 set.lz remove 的 value: ref T → &1i64）
                // 是元素值而非索引，不转（E0606 casting &i64 as usize）；
                // 自定义 get（list.lz ListExt::get 参数 i64 值）也不转（E0308
                // expected i64, found usize）
                let recv_ty_name = match &receiver.ty {
                    IrType::Named { path, .. } => path.clone(),
                    _ => String::new(),
                };
                let has_custom_get = method == "get"
                    && self
                        .struct_method_names_map
                        .get(&recv_ty_name)
                        .map_or(false, |s| s.contains("get"));
                // Set/HashSet 的 insert/remove：参数是元素值 i64（HashSet::insert(value)），
                // 非 Vec 索引，转 usize 报 E0308（containers.lz `numbers.insert(6)`）
                let recv_is_set_ty = matches!(&receiver.ty, IrType::Named { path, .. }
                    if path == "Set" || path == "HashSet");
                // List 自定义 remove（list.lz `remove_at` 内部调 `self.remove(index)`）：
                // 该 remove 是 std Vec::remove 语义（index 需 usize），仍要转换；
                // 仅当 receiver 是 Set/HashSet 时跳过（值语义）
                if (method == "insert"
                    || method == "insert_at"
                    || method == "remove"
                    || method == "get")
                    && !args_s.is_empty()
                    && matches!(&args[0].ty, IrType::Int)
                    && !args_s[0].starts_with('&')
                    && !has_custom_get
                    && !recv_is_set_ty
                {
                    args_s[0] = format!("({} as usize)", args_s[0]);
                }
                // 字符串字面量接收者的 join：LZ `", ".join(parts)` 语义是
                // 用分隔符拼接集合，即 Rust 的 `parts.join(sep)`（Vec::join）。
                // 直接生成 `sep.join(parts)` 会 E0599（String 无 join 方法）。
                // 反转接收者与参数：`{args}.join({sep})`，sep 用 &str 字面量。
                let is_sep_join = method == "join"
                    && args_s.len() == 1
                    && matches!(&receiver.kind, ExprKind::Lit(LitKind::Str(_)));
                let call = if is_sep_join {
                    let sep = if let ExprKind::Lit(LitKind::Str(s)) = &receiver.kind {
                        format!("\"{}\"", s.escape_default())
                    } else {
                        recv.clone()
                    };
                    format!("{}.join({})", args_s[0], sep)
                } else if recv == "self"
                    && rust_method.starts_with("lz_")
                    && (self.in_ext_trait
                        || matches!(&receiver.ty, IrType::Named { path, .. } if path == "str")
                        || matches!(&receiver.ty, IrType::Str))
                {
                    // 扩展 trait 方法体内 `self.lz_*(...)` 调用：显式 UFCS 到本地
                    // trait（StrExt/ListExt/...）。否则与 lz_builtins::StringExt/
                    // ListExt（`use lz_builtins::*` 也在作用域，且 StringExt 同时
                    // impl 于 str/String）的同名方法冲突 → E0034 multiple applicable
                    // items in scope（string.lz starts_with/ends_with/find/rfind/
                    // char_at 复现）。UFCS 传 self（已是 &Self 引用）匹配 &self。
                    // 非扩展 trait 上下文（in_ext=false）但 receiver 是 str 的
                    // `self.lz_*` 调用同样冲突（string.lz split 的 if 块内
                    // self.slice 复现 E0034），一并 UFCS 到 StrExt。
                    let ext_name = if self.in_ext_trait {
                        self.current_ext_trait.clone().unwrap_or_default()
                    } else {
                        "StrExt".to_string()
                    };
                    format!("{}::{}(self, {})", ext_name, rust_method, args_s.join(", "))
                } else if recv.starts_with('<') && recv.contains(">::") && !recv.ends_with(')') {
                    // 关联类型路径 receiver（`<Self as std::iter::Iterator>::Item.default()`）：
                    // Item 是关联类型，方法调用用 ::（`<Self as std::iter::Iterator>::Item::default()`），
                    // 否则 E0575 expected method, found associated type Iterator::Item。
                    // 注意：StrExt 强制调用 `<str as StrExt>::find(self, substr)` 以 ) 结尾
                    // （函数调用而非关联路径），后续 .is_some() 必须用 .（E0308 语法错误）
                    format!("{}::{}({})", recv, rust_method, args_s.join(", "))
                } else {
                    // 函数指针字段调用（如 `self.f(v)`）：需生成 `(self.f)(v)` 而非 `self.f(v)`
                    // （Rust 中 fn 类型字段是 fn item，直接调用需括号包裹转为 fn pointer）
                    // 检查 receiver 类型是否有 fn 类型的字段名为 method
                    let recv_is_fn_field = if let IrType::Named { path, .. } = &receiver.ty {
                        self.struct_fields_info
                            .get(path)
                            .map(|fields| fields.iter().find(|(n, _)| n == method.as_str()))
                            .flatten()
                            .map(|(_, f)| matches!(f, IrType::Fn { .. }))
                            .unwrap_or(false)
                    } else {
                        false
                    };
                    if recv_is_fn_field {
                        format!("({}.{})({})", recv, method, args_s.join(", "))
                    } else {
                        format!("{}.{}({})", recv, rust_method, args_s.join(", "))
                    }
                };
                // LZ 值语义：List.reverse() 返回新列表（Rust Vec::reverse 原地返回 ()，
                // `ll.to_list().reverse()` E0308 expected Vec<i64>, found ()）。
                // receiver 为 List/Vec/Array 且调用点是临时表达式（非简单变量，
                // 如方法调用链 to_list().reverse()）时，生成 克隆+reverse+返回 块。
                // 注意：简单变量 receiver 的 `xs.reverse()` 语句级调用保留原地语义
                // （sort.lz 等已通过库依赖），仅临时表达式才需值语义包装。
                if std::env::var("LZ_DBG_REVERSE").is_ok() {
                    eprintln!(
                        "REVDBG method={:?} recv={:?} recv_ty={:?} kind={:?} recv_is_struct={:?}",
                        method, recv, receiver.ty, receiver.kind, recv_is_struct
                    );
                }
                // 仅临时表达式 receiver（方法调用链结果）时做值语义包装；receiver.ty
                // 在链式调用中常推断为 Any（ll.to_list() 类型未知），故以 recv_is_struct
                // + kind 判断：非用户 struct 且非简单变量的 reverse 一律克隆反转返回
                if method == "reverse"
                    && !recv_is_struct
                    && !matches!(&receiver.kind, ExprKind::Var(_))
                {
                    return format!(
                        "{{ let mut __lz_rev = {}; __lz_rev.reverse(); __lz_rev }}",
                        recv
                    );
                }
                // rev 方法值语义（06d §九）：rev() 在 Iterator trait 上（不在 Vec 上），
                // `vec![1,2,3].rev()` → `vec![1,2,3].into_iter().rev().collect::<Vec<_>>()`
                if method == "rev" && !recv_is_struct {
                    return format!("{}.into_iter().rev().collect::<Vec<_>>()", recv);
                }
                // StrExt trait 方法强制调用：str/String 的 find/trim_start/trim_end/
                // split/lines 与 std str 固有方法同名（固有优先调用 std 版本，返回
                // usize/&str 而非 LZ 的 i64/String，E0277/E0308）——显式 StrExt:: 调用
                let recv_is_str = matches!(&receiver.ty, IrType::Str)
                    || matches!(&receiver.ty, IrType::Named { path, .. }
                        if path == "str" || path == "String")
                    // 方法调用链（format 的 `tpl.slice_from(pos)` ty 推断为 Any）：
                    // base 是 str/String 参数时按字符串处理，确保 find 等强制 StrExt
                    || matches!(&receiver.kind, ExprKind::MethodCall { receiver: base, .. }
                        if matches!(&base.ty, IrType::Str)
                            || matches!(&base.ty, IrType::Named { path, .. }
                                if path == "str" || path == "String"));
                if recv_is_str
                    && self.trait_names.contains("StrExt")
                    && matches!(
                        method.as_str(),
                        "chars"
                            | "find"
                            | "rfind"
                            | "replace"
                            | "repeat"
                            | "trim_start"
                            | "trim_end"
                            | "split"
                            | "lines"
                            | "starts_with"
                            | "ends_with"
                            | "contains"
                    )
                {
                    // StrExt 的这些方法参数是 String（owned），去掉 pattern_methods
                    // 误加的 &（例如 __eq__ 内 `self.starts_with(other)` 的 other 是
                    // &String，被 need_ref 逻辑加 & 变成 &String，与 StrExt 签名冲突
                    // → E0308；恢复为 owned 才能匹配 StrExt::starts_with(&self, prefix: String)）
                    // 并恢复字符串字面量的 to_string（pattern_methods 曾去掉）
                    let args_owned: Vec<String> = args_s
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            // StrExt 这些方法（find/starts_with/ends_with/contains/split/
                            // rfind/replace）的字符串形态实参均归一到 Owned String：裸 &str
                            // 字面量（如 `&"hello"` 剥 & 后仍是 &str）与 &str/&String 变量
                            // 必须 .to_string()，否则 E0308（expected String, found &str）。
                            // 非字符串形态实参（如 repeat 的 i64）由下方 arg_is_str 守卫排除。
                            let needs_own = method == "find"
                                || method == "starts_with"
                                || method == "ends_with"
                                || method == "contains"
                                || method == "split"
                                || method == "rfind"
                                || method == "replace";
                            // 字符串形态实参归一到 Owned String：裸 &str 字面量（如
                            // `&"hello"` 剥 & 后仍是 &str）与 &str/&String 变量必须
                            // .to_string()，否则 E0308（expected String, found &str）。
                            let arg_is_str = args
                                .get(i)
                                .map(|a| {
                                    matches!(&a.ty, IrType::Str)
                                        || matches!(
                                            &a.ty,
                                            IrType::Ref(inner)
                                                if matches!(inner.as_ref(), IrType::Str)
                                        )
                                        || matches!(
                                            &a.ty,
                                            IrType::Ref(inner)
                                                if matches!(
                                                    inner.as_ref(),
                                                    IrType::Named { path, .. }
                                                        if path == "str" || path == "String"
                                                )
                                        )
                                        || matches!(
                                            &a.ty,
                                            IrType::Named { path, .. } if path == "String"
                                        )
                                })
                                .unwrap_or(false);
                            let stripped = if s.starts_with('&') && !s.starts_with("&&") {
                                &s[1..]
                            } else {
                                s.as_str()
                            };
                            if needs_own
                                && arg_is_str
                                && !stripped.ends_with(".to_string()")
                                && !stripped.ends_with(".clone()")
                            {
                                format!("{}.to_string()", stripped)
                            } else if needs_own && s.starts_with('&') && !s.starts_with("&&") {
                                s[1..].to_string()
                            } else {
                                s.clone()
                            }
                        })
                        .collect();
                    // recv 已是引用（self 是 &str）时直接传；值是 String 时 & 取引用
                    // （&String → &str deref coercion）
                    let recv_ref = if recv == "self" || recv.starts_with('&') {
                        recv.clone()
                    } else if recv.starts_with('(') {
                        format!("&{}", recv)
                    } else {
                        format!("&({})", recv)
                    };
                    return format!(
                        "<str as StrExt>::{}({}, {})",
                        rust_method,
                        recv_ref,
                        args_owned.join(", ")
                    );
                }
                // DictExt trait 方法强制调用：HashMap 的 keys/values/items/iter 与
                // std HashMap 固有方法同名（固有优先返回 Keys/Values 迭代器而非
                // LZ 的 Vec/List，E0308 expected Vec<K>, found Keys）——显式 DictExt::
                let recv_is_dict = matches!(&receiver.ty, IrType::Named { path, .. }
                    if path == "Dict" || path == "HashMap");
                // 用户 struct 同名普通方法（如 lib_hashmap 的 keys()）优先于 DictExt
                if recv_is_dict
                    && !user_plain.contains(method.as_str())
                    && matches!(
                        method.as_str(),
                        "keys" | "values" | "items" | "iter" | "iter_keys" | "iter_values"
                    )
                {
                    let recv_ref = if recv == "self" || recv.starts_with('&') {
                        recv.clone()
                    } else if recv.starts_with('(') {
                        format!("&{}", recv)
                    } else {
                        format!("&({})", recv)
                    };
                    // DictExt 方法名映射：keys→lz_keys, values→lz_values, items→lz_items
                    let dict_method = match method.as_str() {
                        "keys" => "lz_keys",
                        "values" => "lz_values",
                        "items" => "lz_items",
                        _ => rust_method,
                    };
                    return format!(
                        "<dyn DictExt<_, _>>::{}({}, {})",
                        dict_method,
                        recv_ref,
                        args_s.join(", ")
                    );
                }
                // SetExt trait 方法强制调用：HashSet 的 union/intersection/difference/
                // symmetric_difference 与 std HashSet 固有方法同名（固有优先返回
                // Union/Intersection 迭代器而非 LZ 的 Set，E0599/E0308）——显式 SetExt::
                let recv_is_set = matches!(&receiver.ty, IrType::Named { path, .. }
                    if path == "Set" || path == "HashSet");
                // 用户 struct 同名普通方法优先于 SetExt（同 DictExt 守卫）
                if recv_is_set
                    && !user_plain.contains(method.as_str())
                    && matches!(
                        method.as_str(),
                        "union" | "intersection" | "difference" | "symmetric_difference" | "iter"
                    )
                {
                    let recv_ref = if recv == "self" || recv.starts_with('&') {
                        recv.clone()
                    } else if recv.starts_with('(') {
                        format!("&{}", recv)
                    } else {
                        format!("&({})", recv)
                    };
                    return format!(
                        "SetExt::{}({}, {})",
                        rust_method,
                        recv_ref,
                        args_s.join(", ")
                    );
                }
                // 比较魔术方法调用（`self.get().__eq__(other.get())`，receiver 非用户
                // struct 时为泛型 T）：转为 Rust 运算符（==/!=/</>/<=/>=），
                // 依赖 T: PartialEq 约束（box.lz `where T: Eq` → E0599 __eq__ not found）。
                // duck 约束泛型接收者（TY-001：DuckParam0: Printable）不转换——
                // Rust 运算符要求 T: PartialOrd（E0369），duck trait 只提供 __lt__，
                // 走常规 trait 方法调用 `a.__lt__(b.clone())`
                let recv_is_duck_constrained = matches!(&receiver.ty, IrType::Named { path, .. }
                    if self.duck_defs.contains_key(path.as_str()));
                if !recv_is_struct
                    && !recv_is_duck_constrained
                    && !matches!(&receiver.kind, ExprKind::Var(n) if n == "self")
                {
                    // `self.get() == other.get()`（__eq__ 的 body）：self.get() 返回
                    // &T（Ref），比较需解引用（*self.get() == *other.get()），否则
                    // E0277/E0308 can't compare T with &T（box.lz）
                    let deref_expr = |cg: &Self, e: &Expr| -> String {
                        let s = cg.gen_expr(e);
                        if matches!(e.ty, IrType::Ref(_) | IrType::MutRef(_)) {
                            format!("*{}", s)
                        } else {
                            s
                        }
                    };
                    let deref_str = |s: &str| -> String {
                        // `&other.get()`（other.get() 已是 &T，& 前缀 → &&T）：
                        // 去掉多余 & 并解引用 → *other.get()（T）
                        if let Some(rest) = s.strip_prefix('&') {
                            format!("*{}", rest)
                        } else {
                            s.to_string()
                        }
                    };
                    match method.as_str() {
                        "__eq__" => {
                            return format!(
                                "{} == {}",
                                deref_expr(self, receiver),
                                args_s
                                    .iter()
                                    .map(|a| deref_str(a))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                        "__ne__" => {
                            return format!(
                                "{} != {}",
                                deref_expr(self, receiver),
                                args_s
                                    .iter()
                                    .map(|a| deref_str(a))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                        "__lt__" => {
                            return format!(
                                "{} < {}",
                                deref_expr(self, receiver),
                                args_s
                                    .iter()
                                    .map(|a| deref_str(a))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                        "__gt__" => {
                            return format!(
                                "{} > {}",
                                deref_expr(self, receiver),
                                args_s
                                    .iter()
                                    .map(|a| deref_str(a))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                        "__le__" => {
                            return format!(
                                "{} <= {}",
                                deref_expr(self, receiver),
                                args_s
                                    .iter()
                                    .map(|a| deref_str(a))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                        "__ge__" => {
                            return format!(
                                "{} >= {}",
                                deref_expr(self, receiver),
                                args_s
                                    .iter()
                                    .map(|a| deref_str(a))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                        _ => {}
                    }
                }
                // ── 迭代器适配器链特殊处理 ──
                // LZ 值语义：.iter() 产出 owned 元素（.iter().cloned()），供 filter/map 闭包
                // 直接按值使用（E0308：xs.iter().filter(|x| x > 0) 闭包参数是 &&i64）；
                // filter 闭包接收 &Item → |&x| 模式（strip_lambda_type_with_ref）；
                // take/skip 参数需 usize（LZ int 是 i64）
                let recv_is_option =
                    matches!(
                        &receiver.ty,
                        IrType::Named { path, .. } if path == "Option" || path == "Result"
                    ) || matches!(&receiver.ty, IrType::Option(_) | IrType::Result { .. });
                if !recv_is_option {
                    if method == "iter" && self.is_collection_type(&receiver.ty) {
                        // 区分自定义 iter（ListExt::iter，item=T 值语义，list.lz 有
                        // ListExt trait）与 std Vec::iter（item=&T，traits.lz 无自定义
                        // iter → .cloned() 转 T，否则 filter 闭包 E0631 expected fn(&&_))
                        let recv_ty_name = match &receiver.ty {
                            IrType::Named { path, .. } => path.clone(),
                            _ => String::new(),
                        };
                        let has_custom_iter = self
                            .struct_method_names_map
                            .get(&recv_ty_name)
                            .map_or(false, |s| s.contains("iter"));
                        if has_custom_iter {
                            return format!("({}).iter()", recv);
                        }
                        return format!("({}).iter().cloned()", recv);
                    }
                    // filter 特判仅适用于 List/Vec（Rust Iterator::filter 单参闭包）：
                    // Dict/HashMap/Set/HashSet 通过扩展 trait（DictExt::filter 等）提供
                    // 自定义 filter（双参闭包 |k, v|），走特判会生成 (d).filter(|&k| ...)
                    // 丢失第二参数（E0425 cannot find value `v`）。
                    // 注意：receiver 类型推断为 Any 时（如 dict.lz `d.filter(...)`），
                    // 也走普通方法调用（解析到扩展 trait 方法），不命中本特判。
                    let recv_is_list = matches!(&receiver.ty, IrType::Named { path, .. }
                        if path == "List" || path == "Vec" || path == "Array");
                    if (method == "filter" || method == "map") && args_s.len() == 1 && recv_is_list
                    {
                        // 区分「类型自带扩展方法」（ListExt::filter/map，闭包参数 &T，
                        // list.lz 同模块 impl）与「裸 Vec 字面量」（build-block，无扩展
                        // trait，Vec 不是 Iterator，裸 .filter/.map 报 E0599）。
                        // 前者必须普通方法调用（into_iter 产出 owned，闭包 &T → E0631）；
                        // 后者必须迭代器链（into_iter().filter/map().collect()）。
                        let recv_ty_name = match &receiver.ty {
                            IrType::Named { path, .. } => path.clone(),
                            _ => String::new(),
                        };
                        let has_ext_method = self
                            .struct_method_names_map
                            .get(&recv_ty_name)
                            .map_or(false, |s| s.contains(method));
                        if has_ext_method {
                            return format!("({}).{}({})", recv, method, args_s[0]);
                        }
                        return format!(
                            "({}).into_iter().{}({}).collect::<Vec<_>>()",
                            recv, method, args_s[0]
                        );
                    }
                    // std 迭代器链的 filter（nesting-expressions `xs.iter().cloned()
                    // .filter(|x| x > 0)`）：闭包参数是 &Item（&&T 因 cloned 后是
                    // &T），需 |&x| 模式（否则 E0308 expected &i64, found i64）。
                    // 注意：带类型注解的闭包（|x: ref T|，DictExt/SetExt/ListExt 的
                    // filter）body 已用 *x 解引用（E0614 type i64 cannot be
                    // dereferenced），不能再加 & 模式——仅无注解闭包需 strip
                    if method == "filter" && args_s.len() == 1 && !recv_is_list {
                        if !args_s[0].contains(": &") {
                            return format!(
                                "({}).filter({})",
                                recv,
                                strip_lambda_type_with_ref(&args_s[0])
                            );
                        }
                    }
                    if method == "take" && args_s.len() == 1 {
                        return format!("({}).take({} as usize)", recv, args_s[0]);
                    }
                    if method == "skip" && args_s.len() == 1 {
                        return format!("({}).skip({} as usize)", recv, args_s[0]);
                    }
                    if method == "sum" && args_s.is_empty() {
                        return format!("({}).sum()", recv);
                    }
                }
                // Option/Result 消费型方法（map/and_then/unwrap_or 等）：receiver 按值
                // 消费，非 Copy 类型（内部含 String 等）的变量需 clone 才能复用（E0382 修复，
                // 如 ok.map(...) 后再用 ok）。借用型方法（as_ref/len 等）不受影响。
                let call = if matches!(&receiver.kind, ExprKind::Var(_))
                    && (matches!(
                        &receiver.ty,
                        IrType::Named { path, .. } if path == "Result" || path == "Option"
                    ) || matches!(&receiver.ty, IrType::Result { .. } | IrType::Option(_)))
                    && matches!(
                        method.as_str(),
                        "map"
                            | "and_then"
                            | "unwrap_or"
                            | "unwrap"
                            | "unwrap_or_else"
                            | "ok"
                            | "err"
                            | "flatten"
                            | "expect"
                            | "filter"
                            | "or"
                            | "and"
                    )
                    && !matches!(&receiver.ty, IrType::Named { args, .. }
                        if args.iter().all(|a| matches!(a, IrType::Int | IrType::F64 | IrType::Bool)))
                {
                    format!("({}).clone().{}({})", recv, rust_method, args_s.join(", "))
                } else {
                    call
                };
                // .len()/.length() on collections → cast usize to i64
                if method == "len" || method == "length" {
                    // 某些路径会生成 &self.len()（&usize），去掉多余的 &（E0606
                    // casting &usize as i64 is invalid）
                    let call_clean = call.trim_start_matches('&').to_string();
                    format!("({} as i64)", call_clean)
                } else if method == "first"
                    || method == "last"
                    || (method == "get"
                        && self.is_collection_type(&receiver.ty)
                        // 用户 struct 自带 get（返回 owned）：不走内置集合的
                        // Option<&T> → .cloned() 包装（lib_hashmap 实测 E0599）
                        && !user_plain.contains("get"))
                {
                    // .first()/.last()/.get() 返回 Option<&T>（LZ ref 语义），需 .cloned()
                    // 转 Option<T>（.copied() 对非 Copy 元素如 String 报 E0277 String: Copy）
                    // get 仅 List/Vec 集合（ListExt::get 返回 Option<&T>）——box.lz 的
                    // Box::get 返回 &T（非 Option），.cloned() 报 E0599 &T is not an iterator
                    format!("({}).cloned()", call)
                } else if method == "type_name" && args_s.is_empty() {
                    // 运行时类型自省（03d §2.8 方案 C）：v.type_name() →
                    // std::any::type_name::<T>()（T 为 receiver 静态类型，去掉引用层级）
                    let t = self.rust_type(&receiver.ty);
                    let t = t.trim_start_matches('&').trim().to_string();
                    format!("std::any::type_name::<{}>()", t)
                } else {
                    call
                }
            }
            ExprKind::FieldAccess { base, field } => {
                if std::env::var("LZ_DBG_FIELD").is_ok() {
                    eprintln!(
                        "FIELDBG base_ty={:?} base_kind={:?} field={:?}",
                        base.ty, base.kind, field
                    );
                }
                // Enum variant: Color.Red → Color::Red (field 大写开头)
                // Module path: std.io.print → std::io::print
                // Method/field access: config.get() -> config.get (field 小写开头)
                // duck 约束泛型参数的字段访问：a.field → a.__field_field()（trait accessor）
                // type-pack 异质元组索引（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的 args
                // 编译为切片 &[Ts]，`args.0` 映射为 `args[0]`（Rust 切片索引）
                let is_numeric_field =
                    !field.is_empty() && field.chars().all(|c| c.is_ascii_digit());
                if is_numeric_field
                    && matches!(
                        &base.ty,
                        IrType::Named { path, .. } if path == "List" || path == "Vec" || path == "Tuple"
                    )
                {
                    return format!("{}[{}]", self.gen_expr(base), field);
                }
                if let ExprKind::Var(name) = &base.kind {
                    if let Some(fields) = self.duck_field_members.get(name) {
                        if fields.contains(field) {
                            let base_s = self.gen_expr(base);
                            // trait accessor 返回 &String / &i64：clone 为 owned 值
                            return format!("{}.__field_{}().clone()", base_s, field);
                        }
                    }
                }
                let base_s = self.gen_expr(base);
                // 用户导入模块的命名空间访问（services.service_name）：
                // 模块项已平铺生成到同一 Rust 文件，直接引用 field 即可
                if matches!(&base.kind, ExprKind::Var(base_name)
                    if self.imported_modules.contains(base_name.as_str()))
                {
                    return field.clone();
                }
                // self 在 impl 方法中始终是 receiver，用 `.` 访问字段
                // `self.Item`（trait Iterator 方法里的关联类型路径，如 sum 的
                // self.Item）→ <Self as Iterator>::Item（字段访问报 E0609 no
                // field Item on &mut Self）
                if base_s == "self" && field.chars().next().map_or(false, |c| c.is_uppercase()) {
                    // 与 where 约束（Self: std::iter::Iterator）一致：<Self as
                    // std::iter::Iterator>::Item（否则 default 等方法 E0599）
                    return format!("<Self as std::iter::Iterator>::{}", field);
                }
                if base_s == "self" {
                    // self.field 从 &self 共享引用中需要 .clone() 来获取所有权
                    // 除非字段类型是 Copy 标量（Int/F64/Bool）
                    let is_scalar = matches!(&base.ty, IrType::Int | IrType::F64 | IrType::Bool);
                    if is_scalar {
                        return format!("{}.{}", base_s, field);
                    }
                    return format!("{}.{}.clone()", base_s, field);
                }
                let known_modules = ["std", "core", "alloc", "crate", "self", "super"];
                let is_var_base = matches!(&base.kind, ExprKind::Var(_));
                let root = base_s.split("::").next().unwrap_or("");
                let is_root_known = known_modules.contains(&root) && root != base_s;
                let is_known_type = is_var_base && self.is_known_type_or_enum(&base_s);
                // 关联类型路径（06c-trait定义.md §五）：`I.Item` → `I::Item`
                // （泛型参数上的关联类型用 ::，E0423 expected value, found type parameter）。
                // 判断：base 是裸泛型参数名（非 self/已知类型/变量），field 大写开头（Item）
                let base_is_generic_param = matches!(&base.kind, ExprKind::Var(n)
                    if n != "self"
                        && !self.downgraded_vars.contains(n.as_str())
                        && !self.global_vars.contains_key(n.as_str())
                        && !self.known_types.contains(n.as_str())
                        && !self.emitted_types.contains(n.as_str())
                        && !self.struct_method_names_map.contains_key(n.as_str())
                        && (self.in_generic_fn
                            || self.in_impl_generic
                            || self.current_variadic_params.contains(n.as_str())));
                // 仅当 field 是大写开头（枚举变体/模块/关联类型）时才用 ::；小写开头为方法/字段，用 .
                let field_is_uppercase = field.chars().next().map_or(false, |c| c.is_uppercase());
                // 未声明的"类型风格"标识符（prelude.lz 引用 lz_builtins 的 Ordering）：
                // 大写开头、非局部变量/常量/已声明类型 → 视为外部枚举变体访问 Ordering::Less，
                // 否则生成 `Ordering.Less` 报 E0423 expected value, found enum
                let base_is_unresolved_type = matches!(&base.kind, ExprKind::Var(n)
                    if n != "self"
                        && !self.downgraded_vars.contains(n.as_str())
                        && !self.global_vars.contains_key(n.as_str())
                        && !self.known_types.contains(n.as_str())
                        && !self.emitted_types.contains(n.as_str())
                        && !self.impl_types.contains(n.as_str())
                        && n.chars().next().map_or(false, |c| c.is_uppercase()));
                let sep = if (is_root_known
                    || is_known_type
                    || base_is_generic_param
                    || base_is_unresolved_type)
                    && field_is_uppercase
                {
                    "::"
                } else {
                    "."
                };
                let mut access_s = format!("{}{}{}", base_s, sep, field);
                // 命名字段枚举字段访问: enum E: X(v: i64) 中 `x.v`
                // → match &x { E::X { v, .. } => v.clone(), _ => unreachable!() }
                // （Rust 不允许对枚举值直接 .v 访问，需解构；仅小写字段走此路径）
                if sep == "." && !field.chars().next().map_or(false, |c| c.is_uppercase()) {
                    if let IrType::Named { path, .. } = &base.ty {
                        if let Some((variant, fields)) = self
                            .enum_variant_named_fields
                            .iter()
                            .find(|((e, _), _)| e == path)
                            .map(|((_, v), f)| (v.clone(), f.clone()))
                        {
                            if fields.iter().any(|f| f == field) {
                                access_s = format!(
                                    "match &{} {{ {}::{} {{ {}, .. }} => {}.clone(), _ => unreachable!() }}",
                                    base_s, path, variant, field, field
                                );
                            }
                        }
                    }
                }
                // Option::None（无参变体）：注入类型参数，避免闭包返回位置
                // 无法推断 T（E0282，如 opt.and_then(|x| Option.None)）
                let access_s = if sep == "::" && field == "None" && base_s == "Option" {
                    // 泛型函数（如 map<R> 内 `case Option.None => Option.None`）中
                    // 硬编码 i64 错误：Option::None 让 Rust 从 match 臂配对推断（combo-struct-method.lz）；
                    // 用户自定义 `enum Option<T>` 遮蔽 std Option 时裸 None 类型不匹配（enum.lz）
                    if self.in_generic_fn {
                        "Option::None".to_string()
                    } else {
                        let elem = self.option_none_elem(&expr.ty);
                        format!("Option::<{}>::None", elem)
                    }
                } else {
                    access_s
                };
                // 递归字段透明解 Box：字段类型是 struct 自身的 Option<Box<Self>>，
                // 读取时映射为 Option<Self>（n.next → n.next.map(|__b| *__b)）。
                // 按 base 的静态类型名（而非变量名）查字段信息。
                let base_type_name = match &base.ty {
                    IrType::Named { path, .. } => Some(path.clone()),
                    IrType::Option(inner) => match inner.as_ref() {
                        IrType::Named { path, .. } => Some(path.clone()),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(bt_name) = base_type_name {
                    if let Some(info) = self.struct_fields_info.get(&bt_name) {
                        let is_recursive = info
                            .iter()
                            .find(|(n, _)| n == field)
                            .map_or(false, |(_, fty)| type_refers_to(fty, &bt_name));
                        if is_recursive {
                            let field_ty = info
                                .iter()
                                .find(|(n, _)| n == field)
                                .map(|(_, t)| t.clone());
                            let is_option = matches!(&field_ty, Some(IrType::Option(_)));
                            return if is_option {
                                format!("{}.map(|__b| *__b)", access_s)
                            } else {
                                format!("(*{})", access_s)
                            };
                        }
                    }
                }
                access_s
            }
            ExprKind::IndexGet { base, key } => {
                let base_s = self.gen_expr(base);
                // Box/Rc/Arc dereference: x[0] on Box<i64> → *x
                if matches!(&base.ty, IrType::Named { path, .. } if path == "Box" || path == "Rc" || path == "Arc")
                {
                    let key_s = self.gen_expr(key);
                    // x[0]（下标 0）→ 解引用 (*x)；其他下标按索引处理
                    let is_zero = matches!(&key.kind, ExprKind::Lit(LitKind::Int(0)))
                        || key_s.trim_end_matches("i64") == "0";
                    if is_zero {
                        // Box<dyn FnOnce(...)>：boxed[0] 语义是解引用调用（03e §六）
                        // → (boxed)()（FnOnce 需 move Box 整体调用，(*boxed)() 不合法）
                        let inner_fn = matches!(&base.ty, IrType::Named { args, .. } if args.first().map_or(false, |a| matches!(a, IrType::Fn { .. })));
                        if inner_fn {
                            format!("({})()", base_s)
                        } else {
                            format!("(*{})", base_s)
                        }
                    } else {
                        format!("{}[{}]", base_s, key_s)
                    }
                } else {
                    let key_s = self.gen_index_key(key, base);
                    // 元组索引 t[0] → Rust 元组字段访问 t.0（Rust 元组不支持 [] 索引，
                    // E0608 cannot index into tuple）——生成 .0/.1/.2
                    if let IrType::Tuple(_) = &base.ty {
                        let idx = match &key.kind {
                            ExprKind::Lit(LitKind::Int(n)) => n.to_string(),
                            _ => key_s.trim_end_matches("i64").trim().to_string(),
                        };
                        return format!("{}.{}", base_s, idx);
                    }
                    // HashMap/Dict 索引: map["key"] → map.get(&"key").cloned()
                    // Rust HashMap 不实现 Index trait
                    let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                    let is_kwargs = matches!(&base.kind, ExprKind::FieldAccess { field, .. } if field == "kwargs");

                    // 用户 struct：ml[0] → ml.__getitem__(0)（key 保持 i64，内部 self.items[i] 再转 usize）
                    let is_struct = !is_dict
                        && matches!(&base.ty, IrType::Named { path, .. } if self.is_known_type(path));
                    if is_struct {
                        format!("({}).__getitem__({})", base_s, self.gen_expr(key))
                    } else if is_kwargs {
                        // __Params.kwargs 值是 Box<dyn Any>：索引取值需 downcast
                        // （.cloned() 会要求 Box<dyn Any>: Clone，E0277）
                        let val_ty = self.rust_type(&expr.ty);
                        format!(
                            "(*(({base}).get(&{key}).unwrap())).downcast_ref::<{val_ty}>().expect(\"kwargs cast failed\").clone()",
                            base = base_s,
                            key = key_s,
                            val_ty = val_ty,
                        )
                    } else if is_dict {
                        // HashMap 索引：dict[key] 返回 &V（LZ ref 语义）或 V 值
                        // ref 返回上下文（get/set_default 返回 Option<ref V>/ref V）用
                        // .get(&key).unwrap()（&V），否则 .cloned().unwrap()（V 值）
                        let ret_is_ref_like = matches!(&self.current_ret_ty, Some(IrType::Ref(_) | IrType::MutRef(_)))
                                || matches!(&self.current_ret_ty, Some(IrType::Option(inner))
                                    if matches!(&**inner, IrType::Ref(_) | IrType::MutRef(_)))
                                || matches!(&self.current_ret_ty, Some(IrType::Named { path, args })
                                    if path == "Option"
                                        && args.first().map_or(false, |a| matches!(a, IrType::Ref(_) | IrType::MutRef(_))))
                                // current_ret_ty 在 if 块内可能为 None：回退到函数
                                // 签名返回类型（get/set_default 返回 Option<ref V>）
                                || matches!(&self.current_fn_ret_ty, Some(IrType::Ref(_) | IrType::MutRef(_)))
                                || matches!(&self.current_fn_ret_ty, Some(IrType::Option(inner))
                                    if matches!(&**inner, IrType::Ref(_) | IrType::MutRef(_)))
                                || matches!(&self.current_fn_ret_ty, Some(IrType::Named { path, args })
                                    if path == "Option"
                                        && args.first().map_or(false, |a| matches!(a, IrType::Ref(_) | IrType::MutRef(_))));
                        if ret_is_ref_like {
                            format!("({}).get(&{}).unwrap()", base_s, key_s)
                        } else {
                            format!("({}).get(&{}).cloned().unwrap()", base_s, key_s)
                        }
                    } else {
                        // 值上下文取 self.字段[...]：需 .clone() 避免从容器 move（self.items[idx] 返回 T）
                        // 赋值目标走 gen_target_expr，不会进入此分支
                        // 字符串单字符索引 s[i]（s 是参数/局部变量，非 self）：Rust 不支持
                        // str 按 usize 索引（E0277 SliceIndex），映射为字节索引取字符码
                        // （simple_hash `let c = s[i]`，comptime 焊死场景）
                        let is_range_key_any = matches!(&key.kind,
                            ExprKind::StructCtor { name, .. } if name == "Range");
                        // base 是 String/str：包括 IR 类型明确为 Str/Named(String)，
                        // 以及 base 是 MethodCall（如 self.s.clone()）时类型被推断为 Any 的情况
                        let base_is_str_any = matches!(&base.ty, IrType::Str)
                            || matches!(&base.ty, IrType::Named { path, .. }
                                if path == "str" || path == "String")
                            || (matches!(&base.ty, IrType::Any)
                                && matches!(&base.kind,
                                    ExprKind::MethodCall { method, receiver, .. }
                                    if method == "clone" && matches!(&receiver.kind,
                                        ExprKind::FieldAccess { field, .. }
                                        if field == "s" || field == "source" || field == "input")))
                            || (matches!(&base.ty, IrType::Any)
                                && matches!(&base.kind,
                                    ExprKind::FieldAccess { field, .. }
                                    if field == "s" || field == "source" || field == "input"));

                        if base_is_str_any && is_range_key_any {
                            return format!("{}[{}].to_string()", base_s, key_s);
                        }
                        if base_is_str_any {
                            // 字符串单字符索引：char 安全；根据期望返回类型决定 char -> String 或 char -> i64
                            let idx_wants_string = matches!(
                                &self.current_ret_ty,
                                Some(IrType::Str)
                            ) || matches!(&self.current_ret_ty, Some(IrType::Named { path, .. })
                                if path == "str" || path == "String")
                                || matches!(&self.current_fn_ret_ty, Some(IrType::Str))
                                || matches!(&self.current_fn_ret_ty, Some(IrType::Named { path, .. })
                                if path == "str" || path == "String")

                                // 调用参数/构造器上下文：期望类型在 current_expected_ty
                                // （如 ParseError::UnexpectedChar(pos, ch: str)）
                                || matches!(&*self.current_expected_ty.borrow(), Some(IrType::Str))
                                || matches!(&*self.current_expected_ty.borrow(), Some(IrType::Named { path, .. })
                                if path == "str" || path == "String");
                            if idx_wants_string {
                                return format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = (({}) as usize); if __i >= __cs.len() {{ '\\0'.to_string() }} else {{ __cs[__i].to_string() }}}}", base_s, key_s);
                            } else {
                                return format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = (({}) as usize); if __i >= __cs.len() {{ '\\0' as i64 }} else {{ __cs[__i] as i64 }}}}", base_s, key_s);
                            }
                        }
                        let is_self_field = matches!(&base.kind, ExprKind::FieldAccess { base: b, .. } if matches!(&b.kind, ExprKind::Var(n) if n == "self"));
                        let is_self_base = matches!(&base.kind, ExprKind::Var(n) if n == "self");
                        // 值上下文取 self 的索引：Rust 的 a[i] 是 *index()（T 值），
                        // move 出容器报 E0507——clone 为 owned（T: Clone，pop/remove_at）
                        // ref 返回上下文（__getitem__/Some(self[i])）用 &self[i]，不 clone
                        let ret_is_ref_like = matches!(
                            &self.current_ret_ty,
                            Some(IrType::Ref(_) | IrType::MutRef(_))
                        ) || matches!(&self.current_ret_ty, Some(IrType::Option(inner))
                                    if matches!(&**inner, IrType::Ref(_) | IrType::MutRef(_)))
                            || matches!(&self.current_ret_ty, Some(IrType::Named { path, args })
                                    if path == "Option"
                                        && args.first().map_or(false, |a| matches!(a, IrType::Ref(_) | IrType::MutRef(_))));
                        if ret_is_ref_like && is_self_base {
                            format!("&{}[{}]", base_s, key_s)
                        } else if is_self_field || is_self_base {
                            // str 的 Range 切片（self[start..end]）返回 &str，
                            // 需 to_string 转 String（string.lz slice，E0599）
                            let base_is_str = matches!(&base.ty, IrType::Str)
                                || matches!(&base.ty, IrType::Named { path, .. }
                                    if path == "str" || path == "String");
                            let is_range_key = matches!(&key.kind,
                                ExprKind::StructCtor { name, .. } if name == "Range");
                            if base_is_str && is_range_key {
                                format!("{}[{}].to_string()", base_s, key_s)
                            } else if base_is_str {
                                // 字符串单字符索引：char 安全；函数返回 str/String 时生成 String
                                let idx_wants_string2 = matches!(
                                    &self.current_ret_ty,
                                    Some(IrType::Str)
                                ) || matches!(&self.current_ret_ty, Some(IrType::Named { path, .. })
                                        if path == "str" || path == "String")
                                    || matches!(&self.current_fn_ret_ty, Some(IrType::Str))
                                    || matches!(&self.current_fn_ret_ty, Some(IrType::Named { path, .. })
                                        if path == "str" || path == "String")
                                    // 调用参数/构造器上下文：期望类型在 current_expected_ty
                                    // （如 ParseError::UnexpectedChar(pos, ch: str)）
                                    || matches!(&*self.current_expected_ty.borrow(), Some(IrType::Str))
                                    || matches!(&*self.current_expected_ty.borrow(), Some(IrType::Named { path, .. })
                                        if path == "str" || path == "String");
                                if idx_wants_string2 {
                                    format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = (({}) as usize); if __i >= __cs.len() {{ '\\0'.to_string() }} else {{ __cs[__i].to_string() }}}}", base_s, key_s)
                                } else {
                                    format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = (({}) as usize); if __i >= __cs.len() {{ '\\0' as i64 }} else {{ __cs[__i] as i64 }}}}", base_s, key_s)
                                }
                            } else {
                                format!("{}[{}].clone()", base_s, key_s)
                            }
                        } else if self.in_generic_fn {
                            // 泛型函数内普通容器索引：T 非 Copy，直接 move 出容器报 E0507
                            // （sort_by<T>/bubble_sort 冒泡 result[j]），自动 clone 为 owned
                            format!("{}[{}].clone()", base_s, key_s)
                        } else {
                            // Range 键落在**非 self 基底**上时，`xs[1..3]` 的类型是 unsized
                            // `[i64]`：作「值」使用（print 实参 / let 绑定 / for 迭代）rustc 直接
                            // E0277 `the size for values of type [i64] cannot be known`
                            // （实测 TEMP/probe10/a_slice_as_value.lz，同件在 cy 面出 `[1, 2]`），
                            // 而规范明写切片返回 `List<int>`（SYNTAX/01-类型系统.md:442）。
                            // 取 owned Vec 而不是 `&[..]`：`{:?}` 可打、与数组字面量可比
                            // （`impl PartialEq<[U;N]> for Vec<T>`）、可按值迭代，
                            // 三种上下文都不引入引用生命周期。比较位原本就能过，改成 Vec 后照样过。
                            if matches!(&key.kind, ExprKind::StructCtor { name, .. } if name == "Range")
                            {
                                // 基底是文本时切片出 `&str`，owned 化走 `to_string()`；
                                // 走 `to_vec()` 会 E0599 `no method named to_vec found for
                                // type str`（实测 2026-10-02：自举语料 lz_lex_flag/rich.rs:261
                                // 的 `word[0..2]`，`word` 的 IR 类型是 Any ⇒ 类型判定不够，
                                // 还要查 str 登记表，见 base_is_str_like）。
                                if self.base_is_str_like(base) {
                                    format!("{}[{}].to_string()", base_s, key_s)
                                } else {
                                    format!("{}[{}].to_vec()", base_s, key_s)
                                }
                            } else {
                                format!("{}[{}]", base_s, key_s)
                            }
                        }
                    }
                }
            }
            ExprKind::IndexSet { base, key, value } => {
                let base_s = self.gen_expr(base);
                let key_s = self.gen_index_key(key, base);
                let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                // checker 块的 __Params.args[k] = v：元素为 Box<dyn Any>，需 Box::new 包装
                let is_params_args = matches!(&base.kind,
                    ExprKind::FieldAccess { field, .. } if field == "args");
                // 用户 struct：ml[1] = v → ml.__setitem__(1, v)
                let is_struct =
                    matches!(&base.ty, IrType::Named { path, .. } if self.is_known_type(path));
                if is_struct {
                    format!(
                        "({}).__setitem__({}, {})",
                        base_s,
                        key_s,
                        self.gen_expr(value)
                    )
                } else if is_dict {
                    // HashMap 不支持 IndexMut，使用 .insert() 代替
                    format!("{}.insert(&{}, {})", base_s, key_s, self.gen_expr(value))
                } else if is_params_args {
                    format!("{}[{}] = Box::new({})", base_s, key_s, self.gen_expr(value))
                } else {
                    format!("{}[{}] = {}", base_s, key_s, self.gen_expr(value))
                }
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                // 字符串比较对齐：任一侧为 &String/&str 引用（str 参数）时，
                // 两侧统一 to_string() 做值比较（&String == String 未实现 → E0277）。
                // 覆盖 Eq/Neq 及顺序比较 Lt/Gt/Le/Ge：str 参数与字面量比较
                // `c >= "0"` 生成 &str >= String（E0308，自举 lz_parser.lz
                // is_digit/is_alpha 复现）
                if matches!(
                    op,
                    BinOpKind::Eq
                        | BinOpKind::Neq
                        | BinOpKind::Lt
                        | BinOpKind::Gt
                        | BinOpKind::Le
                        | BinOpKind::Ge
                ) {
                    let is_str_ty = |t: &IrType| match t {
                        IrType::Str => true,
                        IrType::Ref(inner) | IrType::MutRef(inner) => {
                            matches!(inner.as_ref(), IrType::Str)
                        }
                        _ => false,
                    };
                    if is_str_ty(&lhs.ty) && is_str_ty(&rhs.ty) {
                        let o = match op {
                            BinOpKind::Eq => "==",
                            BinOpKind::Neq => "!=",
                            BinOpKind::Lt => "<",
                            BinOpKind::Gt => ">",
                            BinOpKind::Le => "<=",
                            _ => ">=",
                        };
                        // 单字符索引操作数直出 String 形态（否则 i64 码点块被
                        // .to_string() 成十进制数字串，比较运行期永不相等——lib_json）
                        let lhs_s = self
                            .gen_str_char_index_tostring(lhs)
                            .unwrap_or_else(|| self.gen_expr(lhs));
                        let rhs_s = self
                            .gen_str_char_index_tostring(rhs)
                            .unwrap_or_else(|| self.gen_expr(rhs));
                        return format!("({}).to_string() {} ({}).to_string()", lhs_s, o, rhs_s);
                    }
                }
                // Pow: ** → .pow() 方法调用 (a ** b → a.pow(b))
                if matches!(op, BinOpKind::Pow) {
                    // a ** b → a.pow(b)。gen_lit 已为整数字面量附加 i64 后缀
                    //（如 2i64），直接使用 lhs_s，避免重复追加产生 2i64_i64。
                    // Rust 的 .pow() 指数参数为 u32：整数字面量用 {n}u32，否则 as u32。
                    let lhs_s = self.gen_expr(lhs);
                    let rhs_s = match &rhs.kind {
                        ExprKind::Lit(LitKind::Int(n)) => format!("{}u32", n),
                        _ => format!("{} as u32", self.gen_expr(rhs)),
                    };
                    return format!("{}.pow({})", lhs_s, rhs_s);
                }
                // In / NotIn: 成员测试 → .contains() 方法 (elem in container → container.contains(&elem))
                if matches!(op, BinOpKind::In | BinOpKind::NotIn) {
                    let not_prefix = if matches!(op, BinOpKind::NotIn) {
                        "!"
                    } else {
                        ""
                    };
                    let elem_s = self.gen_expr(lhs);
                    let cont_s = self.gen_expr(rhs);
                    // 字符串包含: "llo" in "hello" → "hello".contains("llo")
                    // 用不带 & 的 contains：对 char / &str / String 都有效（均实现 Pattern）
                    if matches!(&rhs.ty, IrType::Str) {
                        // String::contains 的 Pattern 参数需为 &str：
                        //  - 字符串字面量 "a" 直接使用（已是 &str）
                        //  - String 值（"a".to_string()）用 &* 解引用为 &str
                        //  - char / 其他则原样
                        let elem_arg = if let ExprKind::Lit(LitKind::Str(s)) = &lhs.kind {
                            format!("\"{}\"", s.escape_default())
                        } else if elem_s.ends_with(".to_string()") || elem_s.starts_with('&') {
                            if elem_s.starts_with('&') {
                                format!("*({})", elem_s)
                            } else {
                                format!("&*({})", elem_s)
                            }
                        } else {
                            elem_s.clone()
                        };
                        return format!("{}{}.contains({})", not_prefix, cont_s, elem_arg);
                    }
                    // Dict/HashMap: key in map → map.contains_key(&key)
                    if matches!(&rhs.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap")
                    {
                        return format!("{}{}.contains_key(&{})", not_prefix, cont_s, elem_s);
                    }
                    // 用户 struct 定义了 __contains__ 魔术方法 → 直接调用它
                    // （06d §八：`a in b` → `b.__contains__(a)`），否则 struct 无
                    // contains 方法报 E0599
                    if let IrType::Named { path, .. } = &rhs.ty {
                        if self.is_known_type(path)
                            && self.struct_method_names(path).contains("__contains__")
                        {
                            return format!("{}{}.__contains__({})", not_prefix, cont_s, elem_s);
                        }
                    }
                    // List/Set/其他集合: elem in container → container.contains(&elem)
                    return format!("{}{}.contains(&{})", not_prefix, cont_s, elem_s);
                }
                // 泛型闭包/函数内的 `+`：操作数类型为 Generic（a/b 等隐式泛型形参），
                // 同一份闭包体需对 i64 与 String 等同时合法，而 Rust 原生 `+` 无法兼顾
                // （String 未实现 Add<String>）。经 LZ 多态运算符 trait LzAdd 分派——
                // i64/f64/String 均有 impl，返回自身类型。@math 函数走原生 +（带
                // T: Add 约束），不在此路径，避免改变其语义。
                // List/List 拼接（回归修复）：`+` 两侧任一侧为列表字面量时，语义是
                // Vec 拼接，而非 LzAdd（Vec 未实现 LzAdd）。必须在下方 LzAdd 分派**之前**——
                // `self.data + [val]` 中 self.data 的类型可能为 Any/Generic，会误入
                // LzAdd 分派（lib_vector 回归 E0277）。生成 `{ let mut v = lhs.clone();
                // v.extend(rhs); v }`，clone 避免移动被借用的接收者（E0507）。
                let add_is_list_concat = *op == BinOpKind::Add
                    && (matches!(&lhs.kind, ExprKind::ListLit(_))
                        || matches!(&rhs.kind, ExprKind::ListLit(_))
                        || matches!(&lhs.ty, IrType::Named { path, .. } if path == "List" || path == "Vec")
                        || matches!(&rhs.ty, IrType::Named { path, .. } if path == "List" || path == "Vec"));
                if add_is_list_concat {
                    let lhs_s = self.gen_expr(lhs);
                    let rhs_s = self.gen_expr(rhs);
                    return format!(
                        "{{ let mut __cat = {}.clone(); __cat.extend({}); __cat }}",
                        lhs_s, rhs_s
                    );
                }
                if *op == BinOpKind::Add
                    && !self.in_math_fn
                    && (matches!(&lhs.ty, IrType::Generic(_) | IrType::Any)
                        || matches!(&rhs.ty, IrType::Generic(_) | IrType::Any))
                {
                    let lhs_s = self.gen_expr(lhs);
                    let rhs_s = self.gen_expr(rhs);
                    return format!("LzAdd::__add__({}, {})", lhs_s, rhs_s);
                }
                // String + 拼接: 右侧需借用 & 以匹配 Rust Add<&str>
                // 但如果 rhs 是 variadic 参数（类型已是 &[T]），不应再加 &
                let str_concat = matches!(&rhs.ty, IrType::Str)
                    || matches!(&lhs.ty, IrType::Str)
                    || matches!(&rhs.ty, IrType::Named { path, .. } if path == "String" || path == "str")
                    || matches!(&lhs.ty, IrType::Named { path, .. } if path == "String" || path == "str");
                if *op == BinOpKind::Add && str_concat {
                    let lhs_s = self.gen_expr(lhs);
                    let rhs_s = self.gen_expr(rhs);
                    // const str（&str 静态引用，如 __init__.lz 的 `STDLIB_NAME + " v"`）
                    // 或字符串字面量作 lhs 时 `&str + &str` 非法（E0369）：
                    // 需先 `.to_string()` 变为 String 再拼 &str
                    let lhs_is_ref_str = matches!(&lhs.kind, ExprKind::Lit(LitKind::Str(_)))
                        || matches!(
                            &lhs.kind,
                            ExprKind::Var(name)
                                if self.top_level_static_names.contains(name.as_str())
                        )
                        // &str 引用参数（str 引用语义：builder 标 is_ref 但 ty 保持
                        // Str，参数名登记于 str_typed_vars，生成 &str）作 lhs：
                        // `&str + &str` 非法（E0369，自举 lz_parser.lz
                        // `let key: str = c1 + c2` 复现），同样需先转 String
                        || matches!(
                            &lhs.kind,
                            ExprKind::Var(name) if self.str_typed_vars.contains(name)
                        )
                        || matches!(
                            &lhs.ty,
                            IrType::Ref(inner) | IrType::MutRef(inner)
                                if matches!(inner.as_ref(), IrType::Str)
                        );
                    let lhs_base = if lhs_is_ref_str {
                        format!("{}.to_string()", lhs_s)
                    } else if matches!(&lhs.kind, ExprKind::IndexGet { .. }) {
                        // lhs 从容器取元素（`chars[0] + "x"`，自举试点 p23 复现）：
                        // IndexGet 生成 `chars[0]`（move 出 String，E0507），
                        // str 拼接要求 lhs 为 String 值——需 .clone()（与 v162
                        // 实参 clone 条件 Var|IndexGet 同族）
                        format!("{}.clone()", lhs_s)
                    } else {
                        lhs_s
                    };
                    let rhs_is_variadic = matches!(&rhs.kind, ExprKind::Var(name) if self.current_variadic_params.contains(name));
                    // rhs 为字符串单字符索引（`result + s[pos]`，lib_json 复现）：
                    // 拼接语境直出 to_string 形态（通用路径会落 i64 码点变体，
                    // 拼进 String 语境报 E0308 expected &str found &i64）。
                    if let Some(idx_s) = self.gen_str_char_index_tostring(rhs) {
                        return format!("{} + &{}[..]", lhs_base, idx_s);
                    }
                    if rhs_is_variadic {
                        return format!("{} + {}", lhs_base, rhs_s);
                    }
                    // lhs 非字符串类型（int + str / float + str 等）：
                    // Rust i64 未实现 Add<&str>，需整体用 format! 拼接
                    if !matches!(&lhs.ty, IrType::Str)
                        && !matches!(&lhs.ty, IrType::Named { path, .. } if path == "String" || path == "str")
                    {
                        return format!("format!(\"{{}}{{}}\", {}, {})", lhs_base, rhs_s);
                    }
                    // rhs 非字符串类型（str + int / str + float / str + bool 等）：
                    // Rust String 未实现 Add<i64> 等，需将 rhs 转为 &str 再拼接
                    if !matches!(&rhs.ty, IrType::Str)
                        && !matches!(&rhs.ty, IrType::Named { path, .. } if path == "String" || path == "str")
                    {
                        return format!("{} + format!(\"{{}}\", {}).as_str()", lhs_base, rhs_s);
                    }
                    // String + String → String + &str（Rust Add<&str>）
                    // 对临时 String（format! 等）用 &{}[..] 切为 &str
                    if matches!(&rhs.kind, ExprKind::Call { .. })
                        || rhs_s.ends_with(".to_string()")
                        || matches!(&rhs.kind, ExprKind::Var(_))
                    {
                        return format!("{} + &{}[..]", lhs_base, rhs_s);
                    }
                    return format!("{} + &{}", lhs_base, rhs_s);
                }
                // List/Vec + List/Vec 拼接：Rust Vec 未实现 Add（E0369，
                // `out + [x]` 会生成 `out + vec![...]` 编译失败）。生成
                // extend 语义：`a + b` → `{ let mut v = a; v.extend(b); v }`
                // （b 为 Vec<T>，实现 IntoIterator；拼接返回新 Vec，语义一致）
                let lhs_is_list = matches!(&lhs.ty, IrType::Named { path, .. }
                    if path == "List" || path == "Vec");
                let rhs_is_list = matches!(&rhs.ty, IrType::Named { path, .. }
                    if path == "List" || path == "Vec");
                if *op == BinOpKind::Add && (lhs_is_list || rhs_is_list) {
                    let lhs_s = self.gen_expr(lhs);
                    let rhs_s = self.gen_expr(rhs);
                    // LZ 值语义：lhs 为变量或引用时先克隆再 extend（否则遮蔽场景
                    // `let result = result + [x]` E0382 use of moved value；引用则
                    // E0507 move 借用值）
                    let lhs_base = if lhs_s.trim_start().starts_with('&')
                        || matches!(&lhs.kind, ExprKind::Var(_))
                    {
                        format!("({}).clone()", lhs_s)
                    } else {
                        lhs_s
                    };
                    return format!(
                        "{{ let mut __lz_cat = {}; __lz_cat.extend({}); __lz_cat }}",
                        lhs_base, rhs_s
                    );
                }
                let op_s = self.binop_str(op);
                // 用户 struct 比较运算符 → 自定义魔术方法（box.lz `a == b` 调用
                // `impl Box<T> { def __eq__ }`，否则 Box 无 PartialEq 报 E0369）：
                // lhs.__eq__(&rhs) / __ne__ / __lt__ / __gt__ / __le__ / __ge__
                if op.is_comparison() {
                    // 闭包 ref 参数比较（iter.lz find `|x: ref int| x > 2`，x 是 &i64）：
                    // 任一侧为 Ref 类型时自动解引用（*lhs == rhs / lhs == *rhs /
                    // *lhs == *rhs），否则 E0308 expected &i64 found i64 或 expected T found &T
                    let lhs_is_ref = matches!(lhs.ty, IrType::Ref(_) | IrType::MutRef(_))
                        || matches!(&lhs.kind, ExprKind::Var(n) if n == "self");
                    let rhs_is_ref = matches!(rhs.ty, IrType::Ref(_) | IrType::MutRef(_))
                        || matches!(&rhs.kind, ExprKind::Var(n) if n == "self");
                    if lhs_is_ref || rhs_is_ref {
                        let lhs_s = self.gen_expr(lhs);
                        let rhs_s = self.gen_expr(rhs);
                        let l = if lhs_is_ref {
                            format!("*{}", lhs_s)
                        } else {
                            lhs_s
                        };
                        let r = if rhs_is_ref {
                            format!("*{}", rhs_s)
                        } else {
                            rhs_s
                        };
                        return format!("{} {} {}", l, op_s, r);
                    }
                    if let IrType::Named { path, .. } = &lhs.ty {
                        if self.is_known_type(path) {
                            let methods = self.struct_method_names(path);
                            let magic = match op {
                                BinOpKind::Eq => "__eq__",
                                BinOpKind::Neq => "__ne__",
                                BinOpKind::Lt => "__lt__",
                                BinOpKind::Gt => "__gt__",
                                BinOpKind::Le => "__le__",
                                BinOpKind::Ge => "__ge__",
                                _ => "",
                            };
                            if !magic.is_empty() && methods.contains(magic) {
                                let lhs_s = self.gen_expr(lhs);
                                let rhs_s = self.gen_expr(rhs);
                                // __eq__/__ne__ 参数可能是 owned（magic_methods.lz
                                // `def __eq__(ref self, other: Vector)`）或 ref（box.lz
                                // `def __eq__(ref self, ref other: Box<T>)`）。owned 参数
                                // 传入变量会 move（E0505 cannot move out of v1 because
                                // it is borrowed，`v1 == v1`），需 clone。
                                let other_is_ref = self
                                    .fn_ref_params
                                    .get(magic)
                                    .and_then(|f| f.get(1))
                                    .map_or(false, |(is_ref, _)| *is_ref);
                                let rhs_final = if other_is_ref {
                                    rhs_s
                                } else {
                                    format!("({}).clone()", rhs_s)
                                };
                                return format!("{}.{}({})", lhs_s, magic, rhs_final);
                            }
                        }
                    }
                }
                // `x == None` / `x != None` → `x.is_none()` / `x.is_some()`：
                // 泛型 Option<a> 与 None 比较会因 a 缺 PartialEq 而 E0369；
                // is_none/is_some 无需约束且语义等价（== None ⇔ is_none）。
                if *op == BinOpKind::Eq || *op == BinOpKind::Neq {
                    let is_none_expr = |e: &Expr| -> bool {
                        matches!(&e.kind, ExprKind::Var(n) if n == "None")
                            || matches!(&e.kind, ExprKind::Lit(LitKind::None_))
                            || matches!(&e.kind, ExprKind::StructCtor { name, .. } if name == "None")
                    };
                    let rhs_none = is_none_expr(rhs);
                    let lhs_none = is_none_expr(lhs);
                    if rhs_none != lhs_none {
                        let other = if rhs_none {
                            self.gen_expr(lhs)
                        } else {
                            self.gen_expr(rhs)
                        };
                        return if *op == BinOpKind::Eq {
                            format!("{}.is_none()", other)
                        } else {
                            format!("{}.is_some()", other)
                        };
                    }
                }
                // 链式比较分解: a < b < c → (a < b) && (b < c)
                // 检测：LHS 是比较表达式 且 当前操作符也是比较
                if op.is_comparison()
                    && matches!(&lhs.kind, ExprKind::BinOp { op: lhs_op, .. } if lhs_op.is_comparison())
                {
                    if let ExprKind::BinOp {
                        op: inner_op,
                        lhs: inner_lhs,
                        rhs: inner_rhs,
                    } = &lhs.kind
                    {
                        let inner_lhs_s = self.gen_expr(inner_lhs);
                        let inner_rhs_s = self.gen_expr(inner_rhs);
                        let rhs_s = self.gen_expr(rhs);
                        return format!(
                            "({} {} {}) && ({} {} {})",
                            inner_lhs_s,
                            self.binop_str(inner_op),
                            inner_rhs_s,
                            inner_rhs_s,
                            op_s,
                            rhs_s
                        );
                    }
                }
                // 二元操作的操作数若为 unsafe 块（全局变量访问），需加括号：
                // unsafe { a } + unsafe { b } → (unsafe { a }) + (unsafe { b })
                // float×int 混合算术：int 侧自动提升为 f64（如 3.14 * r）
                let arith = matches!(
                    op,
                    BinOpKind::Add
                        | BinOpKind::Sub
                        | BinOpKind::Mul
                        | BinOpKind::Div
                        | BinOpKind::Mod
                );
                let lhs_ty = &lhs.ty;
                let rhs_ty = &rhs.ty;
                // 操作数是 `as f64` 转换（Cast 目标为 F64）时也视为 f64 侧：
                // `(x as f64) + y` 中 lhs 的 IR 类型可能是 i64（Cast 类型未传播），
                // 但生成代码已是 f64，需提升另一侧避免 E0277（f64 + i64）
                let lhs_is_f64 = matches!(lhs_ty, IrType::F64)
                    || matches!(&lhs.kind, ExprKind::Cast { target, .. } if matches!(target, IrType::F64));
                let rhs_is_f64 = matches!(rhs_ty, IrType::F64)
                    || matches!(&rhs.kind, ExprKind::Cast { target, .. } if matches!(target, IrType::F64));
                // rhs 是数值或未知（Any fallback 为 i64）时，f64 侧混合算术需提升
                let rhs_is_numeric = matches!(rhs_ty, IrType::Int | IrType::F64 | IrType::Any);
                let lhs_is_numeric = matches!(lhs_ty, IrType::Int | IrType::F64 | IrType::Any);
                // 操作数期望类型注入：比较运算互相传递对方类型；任一侧为
                // str/String 时给另一侧传 Str 期望（s[pos] == "-" 生成 String，
                // lib_json skip_ws/parse_number E0308）
                let either_is_str = matches!(lhs_ty, IrType::Str)
                    || matches!(rhs_ty, IrType::Str)
                    || matches!(lhs_ty, IrType::Named { path, .. }
                        if path == "str" || path == "String")
                    || matches!(rhs_ty, IrType::Named { path, .. }
                        if path == "str" || path == "String");
                let lhs_s = if op.is_comparison() || (arith && either_is_str) {
                    let prev_expected = self.current_expected_ty.borrow().clone();
                    if either_is_str && matches!(lhs_ty, IrType::Any | IrType::Int) {
                        *self.current_expected_ty.borrow_mut() = Some(IrType::Str);
                    } else if op.is_comparison() {
                        *self.current_expected_ty.borrow_mut() = Some(rhs_ty.clone());
                    }
                    let s = self.wrap_bin_operand(self.gen_expr(lhs));
                    *self.current_expected_ty.borrow_mut() = prev_expected;
                    s
                } else {
                    self.wrap_bin_operand(self.gen_expr(lhs))
                };
                let rhs_s = if op.is_comparison() || (arith && either_is_str) {
                    let prev_expected = self.current_expected_ty.borrow().clone();
                    if either_is_str && matches!(rhs_ty, IrType::Any | IrType::Int) {
                        *self.current_expected_ty.borrow_mut() = Some(IrType::Str);
                    } else if op.is_comparison() {
                        *self.current_expected_ty.borrow_mut() = Some(lhs_ty.clone());
                    }
                    let s = self.wrap_bin_operand(self.gen_expr(rhs));
                    *self.current_expected_ty.borrow_mut() = prev_expected;
                    s
                } else {
                    self.wrap_bin_operand(self.gen_expr(rhs))
                };
                let lhs_is_complex = is_complex_ty(lhs_ty);
                let rhs_is_complex = is_complex_ty(rhs_ty);
                // 复数提升：算术运算中一侧是 Complex、另一侧是数值 → 将数值侧提升为 Complex::new(x, 0.0)
                // （如 `3.0 + 4.0i` → `Complex64::new(3.0, 0.0) + Complex64::new(0, 4)`）
                if arith && lhs_is_complex && (rhs_is_numeric || rhs_is_f64) && !rhs_is_complex {
                    format!("{lhs_s} {op_s} {COMPLEX_RS}::new({rhs_s}, 0.0)")
                } else if arith
                    && rhs_is_complex
                    && (lhs_is_numeric || lhs_is_f64)
                    && !lhs_is_complex
                {
                    format!("{COMPLEX_RS}::new({lhs_s}, 0.0) {op_s} {rhs_s}")
                } else if arith && lhs_is_f64 && rhs_is_numeric && !rhs_is_f64 {
                    format!("{} {} ({} as f64)", lhs_s, op_s, rhs_s)
                } else if arith && rhs_is_f64 && lhs_is_numeric && !lhs_is_f64 {
                    format!("({} as f64) {} {}", lhs_s, op_s, rhs_s)
                } else {
                    format!("{} {} {}", lhs_s, op_s, rhs_s)
                }
            }
            ExprKind::UnOp { op, operand } => {
                // P1: i64::MIN 特判 — -(-9223372036854775808) → i64::MIN
                if *op == UnOpKind::Neg {
                    if let ExprKind::Lit(LitKind::Int(v)) = &operand.kind {
                        if *v == i64::MIN {
                            return "i64::MIN".to_string();
                        }
                    }
                }
                let op_s = self.unop_str(op);
                let inner = self.gen_expr(operand);
                // P1: ! 运算符高优先级 — 操作数是 BinOp 时需要括号；
                // 且 `not self.__eq__(other)` 生成 `!self == other`（inner 含比较
                // 运算符）时 ! 只应用到 self（E0600 cannot apply ! to &Self），
                // 需 `!(self == other)` 括号包裹
                if *op == UnOpKind::Not {
                    let has_cmp = inner.contains(" == ")
                        || inner.contains(" != ")
                        || inner.contains(" < ")
                        || inner.contains(" > ")
                        || inner.contains(" <= ")
                        || inner.contains(" >= ");
                    if matches!(operand.kind, ExprKind::BinOp { .. }) || has_cmp {
                        format!("{}({})", op_s, inner)
                    } else {
                        format!("{}{}", op_s, inner)
                    }
                } else {
                    format!("{}{}", op_s, inner)
                }
            }
            ExprKind::IfExpr { cond, then, els } => {
                let then_s = self.gen_expr(then);
                let mut els_s = self.gen_expr(els);
                // if/else 类型统一：else 为 `()` 而 then 返回非 Unit 值时，
                // then 分支需 `let _ = expr` 丢弃值，使两分支类型均为 `()`
                // （否则 E0308 if and else have incompatible types）
                let then_is_method = matches!(
                    &then.kind,
                    ExprKind::MethodCall { .. } | ExprKind::Call { .. }
                );
                let els_is_unit = els_s.trim().is_empty() || els_s.trim() == "()";
                let then_needs_discard =
                    then_is_method && els_is_unit && !matches!(&then.ty, IrType::Unit);
                let then_final = if then_needs_discard {
                    format!("let _ = {};", then_s)
                } else {
                    then_s
                };
                // 三元 then/else 类型统一：then 是 bool 而 else 是数值时，
                // else 按 LZ 真值语义转 bool（非零为真），如
                // `(n := compute()) > 5 if n * 10 else 0`（combo_ternary_walrus.lz）
                if matches!(&then.ty, IrType::Bool) && matches!(&els.ty, IrType::Int | IrType::F64)
                {
                    els_s = format!("({}) != 0", els_s);
                }
                // 如果 then 或 els 包含多行 BlockExpr，使用多行格式确保缩进正确
                if then_final.contains('\n') || els_s.contains('\n') {
                    // emit_line 会在字符串前添加 self.indent 级别的缩进，
                    // 所以这里的内容缩进只需 self.indent + 1（相对于 if 行再缩进一层）
                    let close_indent = "    ".repeat(self.indent);
                    let inner_indent = "    ".repeat(self.indent + 1);
                    let then_body = if then_final.starts_with("{\n") {
                        // BlockExpr: 重新格式化内容，使用正确的缩进级别
                        let inner = &then_final[2..then_final.len() - 1]; // 去掉 { 和 }
                        let inner = inner.trim();
                        let lines: Vec<&str> = inner.lines().collect();
                        if lines.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "\n{}{}\n{}",
                                inner_indent,
                                lines.join(&format!("\n{}", inner_indent)),
                                close_indent
                            )
                        }
                    } else {
                        format!(" {}", then_final)
                    };
                    let else_body = if els_s.starts_with("{\n") {
                        let inner = &els_s[2..els_s.len() - 1];
                        let inner = inner.trim();
                        let lines: Vec<&str> = inner.lines().collect();
                        if lines.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "\n{}{}\n{}",
                                inner_indent,
                                lines.join(&format!("\n{}", inner_indent)),
                                close_indent
                            )
                        }
                    } else {
                        format!(" {}", els_s)
                    };
                    format!(
                        "if {} {{{}}} else {{{}}}",
                        self.gen_bool_cond(cond),
                        then_body,
                        else_body
                    )
                } else {
                    format!(
                        "if {} {{ {} }} else {{ {} }}",
                        self.gen_bool_cond(cond),
                        then_final,
                        els_s
                    )
                }
            }
            ExprKind::Lambda {
                params,
                body,
                ret_ty,
                ..
            } => {
                // 嵌套 Fn 返回（fn -> fn -> T）：内层闭包作为外层返回值需 Arc::new 包装
                // （factory_chain: |a| => |b| => x + a + b → move |a| { Arc::new(move |b| {...}) }）
                let nested = self.nested_fn_ret && matches!(&body.kind, ExprKind::Lambda { .. });
                // 闭包返回类型注解（`|x| -> T = ...`）：生成 `-> T` 让 Rust 闭包显式标注，
                // 否则 `or_else(b, |e: str| -> Result<int,int> = Ok(100))` 无法从 Ok(100)
                // 推断 Err 泛型（E0283）
                let ret_ann = ret_ty
                    .as_ref()
                    .map(|t| format!(" -> {}", self.rust_type(t)))
                    .unwrap_or_default();
                // 未使用的闭包参数（Any 类型）无法从上下文推断 → 加 i64 标注
                // （如 Option.None.and_then(|x| Option.None)，E0282）
                let mut body_s = self.gen_expr(body);
                if nested {
                    body_s = format!("Arc::new({})", body_s);
                }
                let params: Vec<String> = params
                    .iter()
                    .map(|p| {
                        let ps = self.gen_param(p);
                        if matches!(&p.ty, IrType::Any) && !body_s.contains(&p.name) {
                            format!("{}: i64", p.name)
                        } else {
                            ps
                        }
                    })
                    .collect();
                // Use move for all closures - LZ doesn't have Rust borrow semantics
                // 当 body 是 BlockExpr 时，抑制 return 关键字让尾表达式正常工作
                let lam = if let ExprKind::BlockExpr { block } = &body.kind {
                    let mut child = CodeGen::new();
                    child.current_fn_raises = self.current_fn_raises.clone();
                    child.current_ret_ty = self.current_ret_ty.clone();
                    child.current_fn_ret_ty = self.current_fn_ret_ty.clone();
                    child.enum_variant_named_fields = self.enum_variant_named_fields.clone();
                    child.emitted_types = self.emitted_types.clone();
                    child.enum_variants = self.enum_variants.clone();
                    child.enum_variant_fields = self.enum_variant_fields.clone();
                    child.fn_param_info = self.fn_param_info.clone();
                    child.current_variadic_params = self.current_variadic_params.clone();
                    // 继承 struct 方法名集合（闭包体内调用用户 struct 方法时
                    // user_plain 判定需要，避免误映射 __next__）
                    child.struct_method_names_map = self.struct_method_names_map.clone();
                    child.struct_init_params_map = self.struct_init_params_map.clone();
                    child.struct_new_params_map = self.struct_new_params_map.clone();
                    child.in_new_body = self.in_new_body;
                    // 传递 static/global 变量名集合（用于 E0530 冲突检测）
                    child.global_vars = self.global_vars.clone();
                    child.top_level_static_names = self.top_level_static_names.clone();
                    child.downgraded_vars = self.downgraded_vars.clone();
                    child.mutated_consts = self.mutated_consts.clone();
                    // 延迟惰性绑定（`@lazy`）需传递给 child：闭包体引用外层 @lazy 变量时
                    // 同样改写为 `*cfg.get_or_init(..)`（T04/AC4）
                    child.lazy_bindings = self.lazy_bindings.clone();
                    child.in_generator = self.in_generator;
                    // 泛型函数标志需传递给 child（match 臂内 Option.None 的裸 None 推断，
                    // combo-struct-method.lz map<R> 泛型方法）
                    child.in_generic_fn = self.in_generic_fn;
                    // Lambda 体内不生成 return，让尾表达式成为闭包返回值
                    child.suppress_tail_return = true;
                    // 嵌套 Fn 返回：内层闭包尾表达式需 Arc::new 包装（E0562）
                    child.nested_fn_ret = self.nested_fn_ret;
                    child.in_lambda_block = true;
                    // 块内含无值 return（return;）→ 尾表达式丢弃值（生成 expr;），
                    // 使闭包返回类型为 ()（var_call_block.lz demo_return_no_value）
                    child.force_unit_tail = block_has_bare_return(block);
                    // move 语义修复：继承当前函数名与引用计数表（同 BlockExpr 处理）
                    child.cur_fn_name = self.cur_fn_name.clone();
                    child.fn_use_count = self.fn_use_count.clone();
                    child.gen_block_inner(block);
                    // 闭包体内赋值外部捕获变量（iter.lz for_each `|x| total = total + x`）：
                    // 用借用捕获（非 move），否则 move 复制 total 副本，外部变量不更新
                    let uses_move = !block_has_external_assign(block, &params);
                    let move_kw = if uses_move { "move " } else { "" };
                    format!(
                        "{move_kw}|{}|{} {{\n{}        }}",
                        params.join(", "),
                        ret_ann,
                        child.buf.trim()
                    )
                } else {
                    // 闭包体内赋值外部捕获变量 → 借用捕获（非 move）
                    let uses_move = !expr_has_external_assign(body, &params);
                    let move_kw = if uses_move { "move " } else { "" };
                    format!(
                        "{move_kw}|{}|{} {{ {} }}",
                        params.join(", "),
                        ret_ann,
                        body_s
                    )
                };
                // IR-003：值位置（let = Lambda / return Lambda）的闭包需 Arc::new 包装为
                // Arc<dyn Fn … + Send + Sync>（可 clone）；参数位置（传给 impl FnMut，如 map/filter/for_each）不包装，
                // 保留借用捕获（for_each |x| total += x 修改外层变量）
                if self.box_lambda {
                    format!("Arc::new({})", lam)
                } else {
                    lam
                }
            }
            ExprKind::StructCtor { name, fields } => {
                // Special handling for built-in types
                match name.as_str() {
                    "_KwArg" => {
                        // 关键字参数 → 提取 value（builder 层暂未完全降级）
                        fields
                            .iter()
                            .find(|(n, _)| n == "value")
                            .map(|(_, v)| self.gen_expr(v))
                            .unwrap_or_else(|| "()".into())
                    }
                    "_Walrus" => {
                        // := walrus 运算符：变量已在 emit_walrus_predecls 中预声明
                        // 这里做赋值（非 let 绑定）并返回变量值
                        let bind = fields.iter().find(|(n, _)| n == "_bind");
                        let val = fields.iter().find(|(n, _)| n == "_val");
                        let bind_s = bind.map(|(_, v)| self.gen_expr(v)).unwrap_or_default();
                        let val_s = val.map(|(_, v)| self.gen_expr(v)).unwrap_or_default();
                        format!("{{ {} = {}; {} }}", bind_s, val_s, bind_s)
                    }
                    "Dict" => {
                        if fields.is_empty() {
                            "std::collections::BTreeMap::new()".to_string()
                        } else {
                            // 带条目的 Dict: HashMap::from([(k, v), ...])
                            let mut pairs = Vec::new();
                            let mut i = 0;
                            while i < fields.len() {
                                let key = fields.iter().find(|(n, _)| n == &format!("_k{}", i));
                                let val = fields.iter().find(|(n, _)| n == &format!("_v{}", i));
                                if let (Some((_, k)), Some((_, v))) = (key, val) {
                                    pairs.push(format!(
                                        "({}, {})",
                                        self.gen_expr(k),
                                        self.gen_expr(v)
                                    ));
                                }
                                i += 1;
                            }
                            format!("std::collections::BTreeMap::from([{}])", pairs.join(", "))
                        }
                    }
                    "Range" => {
                        let start = fields.iter().find(|(n, _)| n == "start");
                        let end = fields.iter().find(|(n, _)| n == "end");
                        let inclusive = fields.iter().any(|(n, v)| {
                            n == "inclusive"
                                && matches!(&v.kind, ExprKind::Lit(LitKind::Bool(true)))
                        });
                        match (start, end) {
                            (Some((_, s)), Some((_, e))) if inclusive => {
                                format!("{}..={}", self.gen_expr(s), self.gen_expr(e))
                            }
                            (Some((_, s)), Some((_, e))) => {
                                format!("{}..{}", self.gen_expr(s), self.gen_expr(e))
                            }
                            (Some((_, s)), None) => format!("{}..", self.gen_expr(s)),
                            (None, Some((_, e))) => format!("..{}", self.gen_expr(e)),
                            _ => "0..0".to_string(),
                        }
                    }
                    "List" | "Vec" => {
                        // List() 空构造 → Vec::new()（List 是 type alias，不能当函数调用 E0423）
                        if fields.is_empty() {
                            "Vec::new()".to_string()
                        } else {
                            let items: Vec<String> =
                                fields.iter().map(|(_, v)| self.gen_expr(v)).collect();
                            format!("vec![{}]", items.join(", "))
                        }
                    }
                    "Set" | "HashSet" => {
                        // Set() 空构造 → HashSet::new()（Set 是 type alias，不能当函数调用 E0423）
                        if fields.is_empty() {
                            "std::collections::HashSet::new()".to_string()
                        } else {
                            let items: Vec<String> =
                                fields.iter().map(|(_, v)| self.gen_expr(v)).collect();
                            format!("std::collections::HashSet::from([{}])", items.join(", "))
                        }
                    }
                    _ => {
                        // 有 magic __new__ 的 struct（box.lz `Rc([1,2,3])`）：
                        // 位置参数构造应分派到 `Name::__new__(value)`（magic __new__
                        // 写在 impl 块中，body 返回 `Rc(_inner: 0)`），而不是把参数
                        // 直接映射到字段（_inner 是 int 占位，E0308）。
                        // 注：box.lz 的 __new__ 在 impl 块里（struct_has_new 不含），
                        // 需检查 struct 方法集合
                        let has_new_method = self
                            .struct_method_names_map
                            .get(name.as_str())
                            .map_or(false, |m| m.contains("__new__"));
                        if !self.in_new_body
                            && (self.struct_has_new.contains(name.as_str()) || has_new_method)
                        {
                            let values: Vec<String> =
                                fields.iter().map(|(_, v)| self.gen_expr(v)).collect();
                            // 同 kwarg 路径：魔法 `__new__`（struct 体由 struct_has_new 登记，
                            // impl 块由方法集合含 "__new__" 判定）用 `__new__`，普通 impl `new` 用 `new`。
                            let is_magic_new = self.struct_has_new.contains(name.as_str())
                                || self
                                    .struct_method_names_map
                                    .get(name.as_str())
                                    .map_or(false, |m| m.contains("__new__"));
                            let new_method = if is_magic_new { "__new__" } else { "new" };
                            return format!("{}::{}({})", name, new_method, values.join(", "));
                        }
                        // 自动补 PhantomData 字段（box.lz `Box(_ptr: 0)` → `Box { _ptr: 0, _lz_phantom_T: PhantomData }`，
                        // 否则 E0063 missing field `_lz_phantom_T`）
                        // BUG-SG-002/003：可空字段（`host: str?`、`db: DbConfig?`）
                        // 用非 Option 值初始化时补 `Some(..)`（E0308）。字段类型表
                        // 来自 struct_fields_info；泛型 struct 的字段类型可能含未绑定
                        // 参数，此时按不行包装处理（保守，避免误包）。
                        let field_tys = self
                            .struct_fields_info
                            .get(name.as_str())
                            .cloned()
                            .unwrap_or_default();
                        let mut fields: Vec<String> = fields
                            .iter()
                            .map(|(n, v)| {
                                // &str → String: 字段类型为 String 且值为 .clone() 时，
                                // 将 .clone() 替换为 .to_string()（E0308）
                                let field_ty =
                                    field_tys.iter().find(|(fn_, _)| fn_ == n).map(|(_, ft)| ft);
                                if n == "s" {}
                                let v_s = if let Some(ft) = field_ty {
                                    if matches!(ft, IrType::Str) {
                                        if let ExprKind::MethodCall {
                                            method, receiver, ..
                                        } = &v.kind
                                        {
                                            if method == "clone" {
                                                let recv_s = self.gen_expr(receiver);
                                                format!("{}.to_string()", recv_s)
                                            } else {
                                                self.gen_expr(v)
                                            }
                                        } else {
                                            self.gen_expr(v)
                                        }
                                    } else {
                                        self.gen_expr(v)
                                    }
                                } else {
                                    self.gen_expr(v)
                                };
                                let wrap = field_tys
                                    .iter()
                                    .find(|(fn_, _)| fn_ == n)
                                    .map_or(false, |(_, ft)| needs_some_wrap(ft, v));
                                if wrap {
                                    format!("{}: Some({})", n, v_s)
                                } else {
                                    format!("{}: {}", n, v_s)
                                }
                            })
                            .collect();
                        if let Some(phantoms) = self.struct_phantom_generics.get(name.as_str()) {
                            for g in phantoms {
                                // PhantomData 不带显式类型参数：T 在调用点（如 main）未绑定，
                                // 让 Rust 从 `_lz_phantom_T: PhantomData<T>` 字段类型推断（E0425）
                                fields
                                    .push(format!("_lz_phantom_{}: std::marker::PhantomData,", g));
                            }
                        }
                        format!("{} {{ {} }}", name, fields.join(", "))
                    }
                }
            }
            ExprKind::EnumCtor {
                enum_name,
                variant,
                args,
            } => {
                // 查找该变体的字段类型，为递归字段自动包裹 Box::new()
                let field_types = self
                    .enum_variant_fields
                    .get(&(enum_name.clone(), variant.clone()));
                // `Some(self[i])`：Rust 的 a[i] 是 *index()（T 值），但 LZ 的
                // __getitem__ 返回 ref T——在返回 Option<ref T> 的方法里（list.lz
                // first/last/get）需 & 取引用（E0308 expected &T, found T）
                let ret_is_ref_option = matches!(&self.current_ret_ty,
                    Some(IrType::Option(inner)) if matches!(&**inner, IrType::Ref(_) | IrType::MutRef(_)))
                    || matches!(&self.current_ret_ty, Some(IrType::Named { path, args })
                        if path == "Option"
                            && args.first().map_or(false, |a| matches!(a, IrType::Ref(_) | IrType::MutRef(_))));
                let args_s: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        let mut expr_s = self.gen_expr(a);
                        if ret_is_ref_option
                            && matches!(variant.as_str(), "Some" | "Ok")
                            && matches!(&a.kind, ExprKind::IndexGet { base, .. }
                                if matches!(&base.kind, ExprKind::Var(n) if n == "self"))
                        {
                            expr_s = format!("&{}", expr_s);
                        }
                        // 检查该位置是否需要 Box::new() 包装
                        let needs_box = field_types.map_or(false, |types| {
                            types
                                .get(i)
                                .map_or(false, |ty| type_refers_to(ty, enum_name))
                        });
                        if needs_box {
                            format!("Box::new({})", expr_s)
                        } else {
                            // 非 Copy 类型字段 + 变量参数 → 自动 clone 避免 move 后借用报错
                            let field_is_noncopy = field_types
                                .as_ref()
                                .and_then(|types| types.get(i))
                                .map_or(false, |ty| match ty {
                                    IrType::Str => true,
                                    IrType::Named { path, args: _ } => matches!(
                                        path.as_str(),
                                        "String" | "Vec" | "HashMap" | "Box"
                                    ),
                                    _ => false,
                                });
                            if field_is_noncopy
                                && matches!(a.kind, ExprKind::Var(_) | ExprKind::FieldAccess { .. })
                            {
                                format!("{}.clone()", expr_s)
                            } else {
                                expr_s
                            }
                        }
                    })
                    .collect();
                // `Err(self)`：self 是 &Self 引用（&Rc<T>），但 Err 需要 owned Rc<T>，
                // 自动 clone（box.lz try_unwrap → E0277/E0308）
                let args_s: Vec<String> = args_s
                    .iter()
                    .zip(args.iter())
                    .map(|(s, a)| {
                        if matches!(&a.kind, ExprKind::Var(n) if n == "self" || n == "self_")
                            && !s.starts_with("(*")
                            && !s.contains(".clone()")
                        {
                            format!("{}.clone()", s)
                        } else {
                            s.clone()
                        }
                    })
                    .collect();
                if args_s.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    let named = self
                        .enum_variant_named_fields
                        .get(&(enum_name.clone(), variant.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if named.len() == args_s.len() && !named.is_empty() {
                        let pairs: Vec<String> = named
                            .iter()
                            .zip(args_s.iter())
                            .map(|(f, v)| format!("{}: {}", f, v))
                            .collect();
                        format!("{}::{} {{ {} }}", enum_name, variant, pairs.join(", "))
                    } else {
                        format!("{}::{}({})", enum_name, variant, args_s.join(", "))
                    }
                }
            }
            ExprKind::Cast { expr, target } => {
                // Special cases: as bool → != 0, as str → format/to_string
                if std::env::var("LZ_DBG_CAST").is_ok() {
                    eprintln!(
                        "CASTDBG target={:?} expr_ty={:?} expr_kind={:?}",
                        target, expr.ty, expr.kind
                    );
                }
                if *target == IrType::Bool {
                    return format!("{} != 0", self.gen_expr(expr));
                }
                if *target == IrType::Str {
                    return format!("format!(\"{{}}\", {})", self.gen_expr(expr));
                }
                // __cast__/__try_cast__ 缺口魔法直派（06d §六）：用户 struct 定义了
                // __cast__ → `x as T` → `x.__cast__()`；仅定义 __try_cast__ →
                // `x.__try_cast__().unwrap()`（可失败转换的断言语义，失败 panic
                // 与 Rust `as` 截断/panic 兜底一致）；两者均无 → 裸 `as`（E0605 兜底）
                if let IrType::Named { path, .. } = &expr.ty {
                    if self.is_known_type(path) {
                        let names = self.struct_method_names(path);
                        if names.contains("__cast__") {
                            return format!("({}.__cast__())", self.gen_expr(expr));
                        }
                        if names.contains("__try_cast__") {
                            return format!("({}.__try_cast__().unwrap())", self.gen_expr(expr));
                        }
                    }
                }
                // __Params.args[i]（Box<dyn Any>）→ 数值：downcast 而非 `as` 强转
                // checker 块体内 `ps.args[i] as int` 的取值路径。
                // 必须在 src_is_string（ty=Any 也走 parse）之前判断，
                // 否则 args 元素 ty=Any 会被 parse 分支截胡，生成 `.parse()` 导致 E0599。
                let tgt_is_numeric = matches!(target, IrType::Int | IrType::F64);
                if tgt_is_numeric
                    && matches!(&expr.kind, ExprKind::IndexGet { base, .. }
                        if matches!(&base.kind, ExprKind::FieldAccess { field, .. } if field == "args"))
                {
                    let tgt = self.rust_type(target);
                    let idx_s = self.gen_expr(expr);
                    return format!(
                        "(*{}.downcast_ref::<{}>().expect(\"checker arg cast failed\"))",
                        idx_s, tgt
                    );
                }
                // String/str → 数值：fallible 解析（str→int 按 09-错误处理.md §2.4）
                // Rust 的 `as` 不允许 String→数值，必须用 .parse()
                // 类型 Any（?）：可能是 String（如元组 field 类型丢失），走 parse 安全
                let src_is_string = matches!(expr.ty, IrType::Str)
                    || matches!(&expr.ty, IrType::Named { path, .. } if path == "String")
                    || matches!(expr.ty, IrType::Any);
                // 字符串切片/索引（s[a..b] / s[i]）→ 数值：base 是 String 但
                // 切片表达式 ty 常推断为 Any（自举试点 parser.lz 复现 E0605
                // `s[0..2] as i64` 非原生 cast），需同样走 .parse()
                let expr_is_str_index = matches!(
                    &expr.kind,
                    ExprKind::IndexGet { base, .. }
                        if matches!(&base.ty, IrType::Str)
                            || matches!(&base.ty, IrType::Named { path, .. }
                                if path == "String" || path == "str")
                );
                if (src_is_string || expr_is_str_index) && tgt_is_numeric {
                    let tgt = self.rust_type(target);
                    return format!("({}).parse::<{}>().unwrap()", self.gen_expr(expr), tgt);
                }
                // __Params.args[i]（Box<dyn Any>）→ 数值：downcast 而非 `as` 强转
                // checker 块体内 `ps.args[i] as int` 的取值路径
                if tgt_is_numeric
                    && matches!(&expr.kind, ExprKind::IndexGet { base, .. }
                        if matches!(&base.kind, ExprKind::FieldAccess { field, .. } if field == "args"))
                {
                    let tgt = self.rust_type(target);
                    let idx_s = self.gen_expr(expr);
                    return format!(
                        "(*{}.downcast_ref::<{}>().expect(\"checker arg cast failed\"))",
                        idx_s, tgt
                    );
                }
                // int → f64: implicit widening
                // Non-primitive casts: as String → .to_string()
                if let IrType::Named { path, .. } = target {
                    if path == "String" {
                        return format!("({}).to_string()", self.gen_expr(expr));
                    }
                }
                format!("{} as {}", self.gen_expr(expr), self.rust_type(target))
            }
            ExprKind::GenExpr { yield_of } => {
                format!("gen {{ yield {}; }}", self.gen_expr(yield_of))
            }
            ExprKind::MagicCall { kind, args } => {
                // 特殊 magic: UnpackBuildCall → ~: 构建块元组解包
                // args[0] = 闭包立即调用表达式, args[1] = 元素索引
                if *kind == MagicKind::UnpackBuildCall && args.len() >= 2 {
                    let packed = self.gen_expr(&args[0]);
                    // 元组字段索引必须是裸整数（无类型后缀），否则 __t.0i64 非法。
                    // args[1] 是索引字面量，直接从 IR 提取，避免 gen_expr 附加的 i64 后缀。
                    let idx = match &args[1].kind {
                        ExprKind::Lit(LitKind::Int(n)) => n.to_string(),
                        _ => self.gen_expr(&args[1]),
                    };
                    // 使用临时变量访问元组字段: { let __t = packed; __t.<idx> }
                    return format!("{{ let __t = {}; __t.{} }}", packed, idx);
                }
                // 魔法方法 → Rust 方法/运算符降级
                self.gen_magic_call(kind, args)
            }
            ExprKind::Pipe {
                receiver,
                callee,
                args,
            } => {
                // 管道兜底展开：receiver 预填充为首参调用 callee
                // （函数/构造/闭包等通用路径；__call__ 实例与 __rpipe__ 由 builder 决策）
                let recv = self.gen_expr(receiver);
                let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                let mut all = vec![recv];
                all.extend(args_s);
                let callee_s = self.gen_expr(callee);
                // 闭包作为 callee 需括号包裹：(|x| ...)(recv)
                if matches!(&callee.kind, ExprKind::Lambda { .. }) {
                    format!("({})({})", callee_s, all.join(", "))
                } else {
                    format!("{}({})", callee_s, all.join(", "))
                }
            }
            ExprKind::BlockExpr { block } => {
                let mut child = CodeGen::new();
                // 复制父 CodeGen 的枚举/类型映射到子实例
                child.current_fn_raises = self.current_fn_raises.clone();
                child.current_ret_ty = self.current_ret_ty.clone();
                child.current_fn_ret_ty = self.current_fn_ret_ty.clone();
                child.enum_variant_named_fields = self.enum_variant_named_fields.clone();
                child.emitted_types = self.emitted_types.clone();
                child.enum_variants = self.enum_variants.clone();
                child.enum_variant_fields = self.enum_variant_fields.clone();
                child.fn_param_info = self.fn_param_info.clone();
                child.in_generator = self.in_generator;
                // 生成器构建块闭包内（func *:）嵌套块表达式（if 分支等）需继承
                // in_gen_build，否则块内 yield 会误走 __gen_vec 函数级路径（E0425）
                child.in_gen_build = self.in_gen_build;
                // 继承 struct 方法名集合（块内调用用户 struct 方法时 user_plain 判定
                // 需要；缺失会导致 self.inner.next() 误映射为 __next__）
                child.struct_method_names_map = self.struct_method_names_map.clone();
                child.struct_init_params_map = self.struct_init_params_map.clone();
                child.struct_new_params_map = self.struct_new_params_map.clone();
                child.in_new_body = self.in_new_body;
                // 泛型函数标志需传递给 child（match 表达式内 Option.None 的裸 None
                // 推断，combo-struct-method.lz map<R> 泛型方法）
                child.in_generic_fn = self.in_generic_fn;
                // 继承父级已声明变量集合：块内 `x = v`（is_mut let）对外层变量的
                // 赋值应生成 `x = ...` 而非 `let mut x = ...` 遮蔽（edge-walrus-operator
                // walrus_if 的 result = first；t_seq 同块顺序赋值正常因 declared 共享）
                child.declared = self.declared.clone();
                // 块表达式尾值应为块尾表达式（非 return）
                child.suppress_tail_return = true;
                // 复制变量重命名表：math.lz `let sign` 遮蔽模块级 fn sign 时，
                // 声明被改名 sign_ 并登记 param_renames；块表达式内（如 while 循环体的
                // `return sign * next_guess`）若不复制，引用 sign 会解析到模块级函数
                // （E0369 cannot multiply fn by f64）
                child.param_renames = self.param_renames.clone();
                child.downgraded_vars = self.downgraded_vars.clone();
                child.global_vars = self.global_vars.clone();
                child.mutated_consts = self.mutated_consts.clone();
                child.slice_clone_bindings = self.slice_clone_bindings.clone();
                child.lazy_static_names = self.lazy_static_names.clone();
                // 延迟惰性绑定（`@lazy`）需传递给 child：多语句 if/while 块体走子
                // CodeGen，块内引用外层 @lazy 变量时同样改写为 `*cfg.get_or_init(..)`（T04/AC4）
                child.lazy_bindings = self.lazy_bindings.clone();
                child.top_level_static_names = self.top_level_static_names.clone();
                child.struct_phantom_generics = self.struct_phantom_generics.clone();
                // size_hint 标志需传递给 child：if 分支体内的 `(0, Some(0))` 元组
                // 走子 CodeGen，若不复制则元组元素不会转 usize（E0308）
                child.current_fn_is_size_hint = self.current_fn_is_size_hint;
                child.in_iterator_impl = self.in_iterator_impl;
                // 返回引用标志（`-> &Self`）：BlockExpr 内 `return self` 判断是否
                // clone 时需继承（inspect 等返回引用的方法，E0308）
                child.current_fn_ret_is_ref = self.current_fn_ret_is_ref;
                // 函数级返回类型：BlockExpr（if 块）内 dict 索引 ref 判断需继承
                child.current_fn_ret_ty = self.current_fn_ret_ty.clone();
                // 函数参数类型表需继承：块内 Call 的实参 clone 注入（非 Copy 变量/
                // 索引元素自动 .clone()，E0382/E0507）依赖 fn_param_types 查询，
                // 否则 if 分支块内的 `f(list[i])` 不 clone（bootstrap/lz_ir 试点暴露）
                child.fn_param_types = self.fn_param_types.clone();
                child.fn_value_carriers = self.fn_value_carriers.clone();
                child.fn_ref_params = self.fn_ref_params.clone();
                child.overload_sigs = self.overload_sigs.clone();
                child.fn_variadic = self.fn_variadic.clone();
                child.fn_kwargs = self.fn_kwargs.clone();
                child.current_variadic_params = self.current_variadic_params.clone();
                // move 语义修复：继承当前函数名与引用计数表，使块表达式内（if 分支等）
                // 的 push/extend 等调用实参能按正确函数作用域查 fn_use_count 自动 .clone()
                // （否则块走子 CodeGen 时 cur_fn_name 为空，clone_if_multiuse 失效，E0382）
                child.cur_fn_name = self.cur_fn_name.clone();
                child.fn_use_count = self.fn_use_count.clone();
                // __gen_vec 已在函数级别声明，BlockExpr 中只需 push 不需要重新声明
                child.gen_block_inner(block);
                format!("{{\n{}    }}", child.buf)
            }
            ExprKind::GenBuild { callee, block } => {
                // 生成器构建块 func *: { yield ... }
                // 有 callee：收集 yield 参数包（闭包收集器 __bb），逐包调用 callee → collect Vec
                // 无 callee：仅收集参数包返回 Vec（迭代器）；yield 类型即收集器元素类型
                let elem_ty = first_yield_type(block).unwrap_or(IrType::Unit);
                let elem_rust = self.rust_type(&elem_ty);
                let mut child = CodeGen::new();
                child.current_fn_raises = self.current_fn_raises.clone();
                child.current_ret_ty = self.current_ret_ty.clone();
                child.current_fn_ret_ty = self.current_fn_ret_ty.clone();
                child.enum_variant_named_fields = self.enum_variant_named_fields.clone();
                child.emitted_types = self.emitted_types.clone();
                child.enum_variants = self.enum_variants.clone();
                child.enum_variant_fields = self.enum_variant_fields.clone();
                child.fn_param_info = self.fn_param_info.clone();
                child.in_generator = false;
                child.in_gen_build = true;
                child.in_generic_fn = self.in_generic_fn;
                child.in_new_body = self.in_new_body;
                child.declared = self.declared.clone();
                child.suppress_tail_return = true;
                child.param_renames = self.param_renames.clone();
                child.downgraded_vars = self.downgraded_vars.clone();
                child.global_vars = self.global_vars.clone();
                child.mutated_consts = self.mutated_consts.clone();
                child.slice_clone_bindings = self.slice_clone_bindings.clone();
                child.lazy_static_names = self.lazy_static_names.clone();
                // 延迟惰性绑定（`@lazy`）需传递给 child：生成器构建块体内引用外层
                // @lazy 变量时同样改写为 `*cfg.get_or_init(..)`（T04/AC4）
                child.lazy_bindings = self.lazy_bindings.clone();
                child.top_level_static_names = self.top_level_static_names.clone();
                child.struct_phantom_generics = self.struct_phantom_generics.clone();
                child.current_fn_is_size_hint = self.current_fn_is_size_hint;
                child.in_iterator_impl = self.in_iterator_impl;
                child.current_fn_ret_is_ref = self.current_fn_ret_is_ref;
                child.current_fn_ret_ty = self.current_fn_ret_ty.clone();
                child.fn_param_types = self.fn_param_types.clone();
                child.fn_value_carriers = self.fn_value_carriers.clone();
                child.fn_ref_params = self.fn_ref_params.clone();
                child.overload_sigs = self.overload_sigs.clone();
                child.fn_variadic = self.fn_variadic.clone();
                child.fn_kwargs = self.fn_kwargs.clone();
                child.current_variadic_params = self.current_variadic_params.clone();
                child.gen_block_inner(block);
                let body_s = child.buf;
                let tail = match callee {
                    Some(callee_expr) => {
                        let callee_s = self.gen_expr(callee_expr);
                        match &elem_ty {
                            IrType::Tuple(elems) => {
                                let binds: Vec<String> =
                                    (0..elems.len()).map(|i| format!("__a{}", i)).collect();
                                let pat = if binds.len() == 1 {
                                    format!("({},)", binds.join(", "))
                                } else {
                                    format!("({})", binds.join(", "))
                                };
                                format!(
                                    "__bb.into_iter().map(move |__p| {{ let {} = __p; {}({}) }}).collect::<Vec<_>>()",
                                    pat,
                                    callee_s,
                                    binds.join(", ")
                                )
                            }
                            IrType::Unit => format!(
                                "__bb.into_iter().map(move |_| {}()).collect::<Vec<_>>()",
                                callee_s
                            ),
                            _ => format!(
                                "__bb.into_iter().map(move |__p| {}(__p)).collect::<Vec<_>>()",
                                callee_s
                            ),
                        }
                    }
                    None => "__bb".to_string(),
                };
                format!(
                    "{{\n    let mut __bb: Vec<{}> = Vec::new();\n    (|| unsafe {{\n{}    }})();\n    {}\n}}",
                    elem_rust, body_s, tail
                )
            }
            ExprKind::ImplicitConvert { source, target_ty } => {
                // `return self`（self 是 &Self 引用）→ 直接 self.clone()：
                // 生成 <Ordering as ImplicitFrom<Self>>::__implicit_from__(self) 会把
                // &Ordering 传给需要 owned Self 的参数（E0308 expected Ordering, found &Ordering）
                if matches!(&source.kind, ExprKind::Var(n) if n == "self" || n == "self_") {
                    // `-> &Self`（inspect 等方法）返回引用：保持 self 引用不 clone，
                    // 否则 `return self` 生成 self.clone() 报 E0308 expected &Result, found Result
                    if self.current_fn_ret_is_ref {
                        return format!("self");
                    }
                    // ref str 的 self.clone() 返回 &str（&str: Clone），需 to_string 转
                    // String（string.lz replace `return self`，E0308）
                    let ret_is_string = matches!(target_ty, IrType::Named { path, .. } if path == "String" || path == "str")
                        || matches!(target_ty, IrType::Str);
                    if ret_is_string {
                        return format!("self.to_string()");
                    }
                    return format!("self.clone()");
                }
                let src = self.gen_expr(source);
                // `Some(item)`（item 是借用绑定 &I::Item，match &self.peeked）实际已是
                // Option<&T>，但 builder 推断 Option<T>（值）插入转换——跳过
                // （E0277 Option<&I::Item>: ImplicitFrom<Option<I::Item>>，Peekable peek）
                let skip_opt_ref = if let IrType::Option(t_inner) = target_ty {
                    if let IrType::Ref(ir) = t_inner.as_ref() {
                        matches!(&source.ty, IrType::Option(s_inner)
                            if s_inner.as_ref() == ir.as_ref())
                    } else {
                        false
                    }
                } else {
                    false
                };
                if skip_opt_ref {
                    return src;
                }
                let tgt = self.rust_type(target_ty);
                let src_ty = self.rust_type(&source.ty);
                format!(
                    "<{} as ImplicitFrom<{}>>::__implicit_from__({})",
                    tgt, src_ty, src
                )
            }
            ExprKind::Paren(inner) => {
                // 剥离不必要括号: (*expr) → *expr
                // 注意：BinOp 子表达式不能剥离——`(a + b) / c` 剥成
                // `a + b / c` 会改变运算优先级（math.lz sqrt 断言失败）。
                // 一元运算符自身优先级最高可安全剥离。
                match &inner.kind {
                    ExprKind::UnOp { .. } => {
                        self.gen_expr(inner) // 一元运算符自身优先级足够
                    }
                    ExprKind::BinOp { .. } => format!("({})", self.gen_expr(inner)),
                    _ => format!("({})", self.gen_expr(inner)),
                }
            }
            ExprKind::TupleLit(elems) => {
                // impl Iterator 的 size_hint 方法体：std 要求返回 (usize, Option<usize>)，
                // 元组元素（i64 字面量/表达式，含 Some(0) 内部与 Option<i64> 变量）需转 usize
                if self.current_fn_is_size_hint {
                    let elems: Vec<String> = elems
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            let s = self.gen_expr(e);
                            // Some(0) → Some(0 as usize)：Option<Int> 元素内部转 usize
                            if matches!(&e.ty, IrType::Option(_)) && s.contains("Some(") {
                                if let ExprKind::Call { args, .. } = &e.kind {
                                    if args.len() == 1 && matches!(args[0].ty, IrType::Int) {
                                        let inner = self.gen_expr(&args[0]);
                                        return format!("Some({} as usize)", inner);
                                    }
                                }
                            }
                            // Option 变量（如 `hi`/`new_upper`，类型 Option<Int> 或
                            // Option<Any>）→ map 转 usize：hi.map(|v| v as usize)
                            if matches!(&e.ty, IrType::Option(_))
                                && !s.contains("map(")
                                && !s.contains("Some(")
                            {
                                return format!("({}).map(|v| v as usize)", s);
                            }
                            if matches!(e.ty, IrType::Int) && !s.contains(" as usize") {
                                format!("({} as usize)", s)
                            } else if matches!(e.ty, IrType::Any)
                                && !s.contains(" as usize")
                                && !s.contains("map(")
                            {
                                // 变量元素类型 Any（LetTuple 解构未传播到表达式）：
                                // size_hint 元组按位置兜底，第 0 个（usize）as usize，
                                // 第 1 个（Option<usize>）map 转 usize（E0308 expected
                                // usize, found i64——Map/Filter/Take 的 size_hint）
                                if i == 0 {
                                    format!("({} as usize)", s)
                                } else if i == 1 {
                                    format!("({}).map(|v| v as usize)", s)
                                } else {
                                    s
                                }
                            } else {
                                s
                            }
                        })
                        .collect();
                    format!("({})", elems.join(", "))
                } else {
                    // BUG-11：元组槽位的期望类型下推，形状对齐 ListLit 元素位
                    // （save/restore 约定同 Call 实参位）。改前 TupleLit 从不下推，
                    // `let t: (List<List<int>>, List<int>) = ([], [1, 2])` 的第 0 槽
                    // 空列表借不到声明类型 `List<List<int>>`，只能按自身默认推断
                    // 发成 `Vec::<i64>::new()` ⇒ E0308 expected Vec<Vec<i64>>,
                    // found Vec<i64>（静态定义与内联引用两侧同错）。
                    let tuple_expected = self.current_expected_ty.borrow().clone();
                    let elems: Vec<String> = elems
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            let pushed = match &tuple_expected {
                                Some(IrType::Tuple(tys))
                                    if matches!(
                                        &e.kind,
                                        ExprKind::ListLit(_) | ExprKind::TupleLit(_)
                                    ) =>
                                {
                                    tys.get(i).cloned()
                                }
                                _ => None,
                            };
                            let prev_expected = if pushed.is_some() {
                                let prev = self.current_expected_ty.borrow().clone();
                                *self.current_expected_ty.borrow_mut() = pushed;
                                Some(prev)
                            } else {
                                None
                            };
                            let s = self.gen_expr(e);
                            if let Some(prev) = prev_expected {
                                *self.current_expected_ty.borrow_mut() = prev;
                            }
                            s
                        })
                        .collect();
                    format!("({})", elems.join(", "))
                }
            }
            ExprKind::ListLit(elems) => {
                // BUG-SG-005: 含展开元素 → 降级为 { let mut v = Vec::new(); v.extend(..); v.push(..); v }
                // （vec![] 字面量无法内联 extend，必须用块构造）
                if elems.iter().any(|e| matches!(e.kind, ExprKind::Spread(_))) {
                    // 每个展开列表降级为独立 { } 块，__spread_v 块级作用域天然不冲突
                    let v = "__spread_v";
                    let mut lines = vec![format!("let mut {} = Vec::new();", v)];
                    for e in elems {
                        match &e.kind {
                            ExprKind::Spread(inner) => {
                                let s = self.gen_expr(inner);
                                // inner 通常为 Vec<T>/&[T]；iter().cloned() 产出 T，extend 据此推导元素类型
                                lines.push(format!("{}.extend({}.iter().cloned());", v, s));
                            }
                            _ => {
                                let s = self.gen_expr(e);
                                // 与非展开元素一致的 .clone() 规则（E0507/E0382）
                                let is_copy =
                                    matches!(&e.ty, IrType::Int | IrType::F64 | IrType::Bool);
                                let is_moveable =
                                    matches!(&e.kind, ExprKind::Var(_) | ExprKind::IndexGet { .. });
                                let s = if !is_copy
                                    && is_moveable
                                    && !s.starts_with('&')
                                    && !s.ends_with(".clone()")
                                    && !s.contains("::")
                                {
                                    format!("{}.clone()", s)
                                } else {
                                    s
                                };
                                lines.push(format!("{}.push({});", v, s));
                            }
                        }
                    }
                    lines.push(v.to_string());
                    return format!("{{\n{}\n}}", lines.join("\n"));
                }
                // 空列表：Nil/Unit/Any → ()，否则 → Vec::new() 或 vec![...]
                let is_nil = elems.is_empty()
                    && (matches!(expr.ty, IrType::Unit | IrType::Any)
                        || matches!(self.rust_type(&expr.ty).as_str(), "()"));
                if is_nil {
                    "()".to_string()
                } else {
                    // BUG-5：元素位期望类型下推（save/restore 约定同 Call 实参位）。
                    // 只对「元素本身又是列表/元组字面量」的位置下推，其余元素保持原环境。
                    let elem_expected = self.list_lit_elem_ty(&expr.ty);
                    let elems_s: Vec<String> = elems
                        .iter()
                        .map(|e| {
                            let pushed = if matches!(
                                &e.kind,
                                ExprKind::ListLit(_) | ExprKind::TupleLit(_)
                            ) {
                                elem_expected.clone()
                            } else {
                                None
                            };
                            let prev_expected = if pushed.is_some() {
                                let prev = self.current_expected_ty.borrow().clone();
                                *self.current_expected_ty.borrow_mut() = pushed;
                                Some(prev)
                            } else {
                                None
                            };
                            let s = self.gen_expr(e);
                            if let Some(prev) = prev_expected {
                                *self.current_expected_ty.borrow_mut() = prev;
                            }
                            // 列表字面量元素：IndexGet/Var 且元素类型非 Copy 时
                            // 自动 .clone()（E0507 cannot move out of index /
                            // E0382 moved value，如 `out + [ts[i]]` 的 vec![ts[i]]）
                            let is_copy = matches!(&e.ty, IrType::Int | IrType::F64 | IrType::Bool);
                            let is_moveable =
                                matches!(&e.kind, ExprKind::Var(_) | ExprKind::IndexGet { .. });
                            // &str 类型需要 to_string() 而非 clone()
                            let is_str = matches!(&e.ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str))
                                || matches!(&e.ty, IrType::Str);
                            if !is_copy
                                && is_moveable
                                && !s.starts_with('&')
                                && !s.ends_with(".clone()")
                                && !s.contains("::")
                            {
                                if is_str {
                                    format!("{}.to_string()", s)
                                } else {
                                    format!("{}.clone()", s)
                                }
                            } else {
                                s
                            }
                        })
                        .collect();
                    if elems_s.is_empty() {
                        // 空列表：元素类型推断。字面量 `[]` 在 IR 中常默认推断为
                        // List<i64>，但函数返回/调用上下文可能要求 List<a>（泛型）。
                        // 优先用期望类型/返回类型覆盖，否则误生成 Vec::<i64>::new()（E0308）。
                        if let IrType::Named { path, args } = &expr.ty {
                            if (path == "List" || path == "Vec") && !args.is_empty() {
                                let elem = self.empty_list_elem(&expr.ty);
                                format!("Vec::<{}>::new()", elem)
                            } else if path == "List" || path == "Vec" {
                                "vec![]".to_string()
                            } else {
                                "vec![]".to_string()
                            }
                        } else {
                            "vec![]".to_string()
                        }
                    } else {
                        format!("vec![{}]", elems_s.join(", "))
                    }
                }
            }
            ExprKind::AssignExpr { target, value } => {
                // 纯赋值表达式（闭包体 `total = total + x`）：渲染 `target = value`
                format!("{} = {}", self.gen_expr(target), self.gen_expr(value))
            }
            _ => unreachable!("unsupported expr kind: {:?}", expr.kind),
        }
    }

    /// 判断 IrType 是否完全具体（不含泛型类型参数如 K, V）
    pub(crate) fn is_fully_concrete(&self, ty: &IrType) -> bool {
        match ty {
            IrType::Any
            | IrType::Unit
            | IrType::Int
            | IrType::Int128
            | IrType::BigInt
            | IrType::Complex
            | IrType::F64
            | IrType::Bool
            | IrType::Str
            | IrType::Never
            | IrType::Self_
            | IrType::Ext => true,
            IrType::Named { path, args } => {
                // 单字母类型名（K, V, T 等）视为泛型类型参数，非具体
                if path.len() == 1 && path.chars().next().unwrap().is_ascii_uppercase() {
                    return false;
                }
                // 必须是已知具体类型
                if !self.is_known_type(path) {
                    return false;
                }
                args.iter().all(|a| self.is_fully_concrete(a))
            }
            IrType::Option(inner)
            | IrType::Result { ok: inner, .. }
            | IrType::Ref(inner)
            | IrType::MutRef(inner) => self.is_fully_concrete(inner),
            IrType::Tuple(elems) => elems.iter().all(|e| self.is_fully_concrete(e)),
            IrType::Fn { params, ret } => {
                params.iter().all(|p| self.is_fully_concrete(p)) && self.is_fully_concrete(ret)
            }
            IrType::Duck { .. } => false,
            IrType::Generic(_) => false,
        }
    }

    pub(crate) fn gen_bool_cond(&self, cond: &Expr) -> String {
        // 处理 Not 包裹：not expr → !(expr 转 bool)
        if let ExprKind::UnOp {
            op: UnOpKind::Not,
            operand,
        } = &cond.kind
        {
            let inner = self.gen_bool_cond(operand);
            return format!("!({})", inner);
        }
        // 用户 struct 类型 → 真值判定链（规范 06g §9）：
        //   ① __bool__()        显式布尔判定
        //   ② __len__() != 0    长度回退（HasLen）
        //   ③ 默认 true          非空对象视为 true
        // 之前无条件生成 `(x).__bool__()`，未实现 __bool__ 的类型会 E0599。
        if let IrType::Named { path, .. } = &cond.ty {
            if self.is_known_type(path) {
                let base = path.split('<').next().unwrap_or(path);
                let methods = self.struct_method_names(base);
                let s = self.gen_expr(cond);
                // 若表达式是赋值等复合，直接调用
                if methods.contains("__bool__") {
                    return format!("({}).__bool__()", s);
                }
                if methods.contains("__len__") {
                    return format!("(({}).__len__() != 0)", s);
                }
                // 两者皆未实现 → 规范 §9.3：任何非空对象视为 true
                return "true".to_string();
            }
        }
        // 数值条件：LZ 真值语义非零为真（如 `a if n * 10 else 0`，combo_ternary_walrus.lz）。
        // i64/f64 条件需转 bool 比较，否则 if 条件类型不匹配（E0308）
        if matches!(&cond.ty, IrType::Int | IrType::F64) {
            let s = self.gen_expr(cond);
            return format!("({}) != 0", s);
        }
        // 内建容器/字符串真值（06d §十二）：非空为真——
        //   String/&str/Vec/HashMap/HashSet → !x.is_empty()
        // 否则直接透传类型作条件，E0308 expected bool / E0600 !String
        if matches!(&cond.ty, IrType::Str)
            || matches!(&cond.ty, IrType::Named { path, .. }
                if matches!(path.as_str(), "String" | "Vec" | "List" | "Dict" | "HashMap" | "Set" | "HashSet"))
        {
            let s = self.gen_expr(cond);
            return format!("!({}).is_empty()", s);
        }
        self.gen_expr(cond)
    }

    /// 生成 f-string: 提取 {expr} 插值，转成 format!("literal", expr, ...)
    /// {{ / }} 转义为字面量大括号；单个 {expr} 为插值占位符
    pub(crate) fn gen_fstring(&self, s: &str) -> String {
        let mut format_str = String::new();
        let mut args: Vec<String> = Vec::new();
        let mut str_specs: Vec<bool> = Vec::new();
        let mut arg_idx = 0usize;
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        // {{ → 显示字面 {（format! 中需要 {{）
                        chars.next();
                        format_str.push_str("{{");
                    } else {
                        // 提取插值表达式 {expr}
                        let mut expr = String::new();
                        let mut depth = 0usize;
                        while let Some(&ec) = chars.peek() {
                            match ec {
                                '}' if depth == 0 => {
                                    chars.next();
                                    break;
                                }
                                '{' => {
                                    depth += 1;
                                    expr.push(ec);
                                    chars.next();
                                }
                                '}' => {
                                    depth -= 1;
                                    expr.push(ec);
                                    chars.next();
                                }
                                _ => {
                                    expr.push(ec);
                                    chars.next();
                                }
                            }
                        }
                        // 用唯一标记占位，最后替换为 {} 占位符
                        format_str.push_str(&format!("__LZ_FMT_{}__", arg_idx));
                        arg_idx += 1;
                        // 插值若是单个变量名且为降级变量（line 等宏名冲突）→ 用重命名后的名字
                        let expr_trim = expr.trim();
                        let is_str_var = !self.unbound_fstring_vars.contains(expr_trim)
                            && (self.str_typed_vars.contains(expr_trim)
                                || expr_trim.starts_with("len(")
                                || self.is_str_expr(&expr));
                        if self.unbound_fstring_vars.contains(expr_trim) {
                            // 探测用例中未绑定的插值变量 → 空字符串，保证 rustc 编译通过
                            args.push("\"\"".to_string());
                        } else if self.downgraded_vars.contains(expr_trim) {
                            args.push(format!("{}_", expr_trim));
                        } else if let Some(inner) = expr_trim
                            .strip_prefix("len(")
                            .and_then(|s| s.strip_suffix(')'))
                        {
                            // f-string 插值中的 len(x) → (x.len() as i64)
                            args.push(format!("({}.len() as i64)", inner));
                        } else {
                            args.push(self.gen_expr_str(&expr));
                        }
                        // 记录该插值是否字符串语义（用 {} 而非 {:?}，避免 String Debug 引号）
                        str_specs.push(is_str_var);
                    }
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        format_str.push_str("}}");
                    } else {
                        format_str.push('}');
                    }
                }
                _ => format_str.push(c),
            }
        }
        // 先转义文本中的 { / }，再恢复插值占位符为 {}，避免占位符被误转义
        let escaped = escape_format_braces(&format_str);
        let mut fmt_quoted = escaped;
        for i in 0..arg_idx {
            // 字符串语义插值（String/str 变量、字面量等）用 {}（Display，无引号）；
            // 其余（容器/Option/HashMap 等只有 Debug）用 {:?}（E0277 规避）。
            // 口径出处是规范而不是实现：SYNTAX/00-词法基础.md:188 明确
            // `f"x={x}"` → `format!("x={}", x)`，即 str 插值**不带引号**；
            // 该断言由 tests/str_boundary.rs 的 c7_fstring 用例实跑锁死
            // （`assert msg == "name=LZ, age=30"`，2026-10-02 复测为绿）。
            // 注：DEMO/01_basics/strings.lz:43 与 DEMO/01_basics/polish_28_strings.lz:28
            // 曾断言**带引号**形态，与该规范行冲突（台账 BUG-22）——DEMO 闸门只做
            // 转译+rustc 编译，从不执行这些 assert，故其文案一直不构成判据。
            let spec = if str_specs.get(i).copied().unwrap_or(false) {
                "{}"
            } else {
                "{:?}"
            };
            fmt_quoted = fmt_quoted.replace(&format!("__LZ_FMT_{}__", i), spec);
        }
        let fmt_quoted = fmt_quoted.replace('"', "\\\"");
        if args.is_empty() {
            format!("format!(\"{}\")", fmt_quoted)
        } else {
            format!("format!(\"{}\", {})", fmt_quoted, args.join(", "))
        }
    }

    /// 将 IR 表达式字符串化（用于 f-string 插值）。简单提取：若为 Var/字段则直接用名字
    pub(crate) fn gen_expr_str(&self, expr: &str) -> String {
        expr.trim().to_string()
    }

    /// 基底是否**文本**：决定索引/切片的 owned 化走 `to_string()` 还是 `to_vec()`。
    /// 不能只看 `base.ty`——builder 对逐字符累加的变量（自举词法器里的 `word`）推断成
    /// `Any`，只看类型会把 str 切片发成 `to_vec()`（E0599）。故再查两张 str 登记表与
    /// 字段名启发式（与既有 `base_is_str_any` 同源，收紧成一处）。
    pub(crate) fn base_is_str_like(&self, base: &Expr) -> bool {
        let named_str = |t: &IrType| match t {
            IrType::Str => true,
            IrType::Named { path, .. } => path == "str" || path == "String",
            IrType::Ref(inner) | IrType::MutRef(inner) => matches!(&**inner, IrType::Str),
            _ => false,
        };
        if named_str(&base.ty) {
            return true;
        }
        if let ExprKind::FieldAccess { field, .. } = &base.kind {
            if matches!(field.as_str(), "s" | "source" | "input") {
                return true;
            }
        }
        if matches!(&base.ty, IrType::Any) {
            if let ExprKind::Var(n) = &base.kind {
                return self.str_typed_vars.contains(n)
                    || self.fstr_var_types.get(n).map_or(false, |t| named_str(t));
            }
        }
        false
    }

    /// f-string 插值表达式是否为字符串语义（生成 {} 而非 {:?}）：
    /// 已登记的字符串变量或字符串字面量。其余（int/容器/属性等）用 Debug {:?}
    pub(crate) fn is_str_expr(&self, expr: &str) -> bool {
        let t = expr.trim();
        let strish = |ty: &IrType| {
            matches!(ty, IrType::Str)
                || matches!(ty, IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str))
                || matches!(ty, IrType::MutRef(inner) if matches!(inner.as_ref(), IrType::Str))
        };
        if t.starts_with('"') || t.starts_with('\'') {
            return true;
        }
        if self.str_typed_vars.contains(t) {
            return true;
        }
        // 按 IR 类型复核：`str_typed_vars` 存的是**降级后的安全名**（`file` → `file_`），
        // 而插值文本永远是源码名 ⇒ 降级变量在那里必然查不到，会被当成非字符串走
        // `{:?}` 加引号（违反 SYNTAX/00-词法基础.md:188）。fstr_var_types 两个名字都登记。
        if self.fstr_var_types.get(t).map_or(false, strish) {
            return true;
        }
        // 字符串方法链（如 x.to_upper()）— 已登记变量首段
        if let Some(dot) = t.find('.') {
            let base = &t[..dot];
            if self.str_typed_vars.contains(base) {
                return true;
            }
            // `recv.field`：recv 是 self 或本函数的 struct 形参时，按字段声明类型判定
            let field = t[dot + 1..].trim();
            if !field.is_empty() && !field.contains('.') && !field.contains('(') {
                let struct_name = if base == "self" {
                    self.fstr_self_type.clone()
                } else {
                    match self.fstr_var_types.get(base) {
                        Some(IrType::Named { path, .. }) => Some(path.clone()),
                        Some(IrType::Ref(inner)) | Some(IrType::MutRef(inner)) => {
                            match inner.as_ref() {
                                IrType::Named { path, .. } => Some(path.clone()),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                };
                if let Some(name) = struct_name {
                    let b = name.split('<').next().unwrap_or(&name).to_string();
                    if let Some(info) = self.struct_fields_info.get(&b) {
                        if info
                            .iter()
                            .any(|(fname, fty)| fname == field && strish(fty))
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// 判断表达式是否为字符串产生式（生成 Rust String）：
    /// 字符串字面量 / 字符串插值 / 接收者已知为字符串的字符串方法调用
    /// （如 `", ".join(parts)` → receiver 为 Str 字面量，join 返回 String）。
    /// 用于 let 绑定登记 str_typed_vars（builder 对 join 等方法调用推断为 Any）
    pub(crate) fn is_str_producing(&self, v: &Expr) -> bool {
        match &v.kind {
            ExprKind::Lit(LitKind::Str(_)) | ExprKind::Lit(LitKind::FStr(_)) => true,
            ExprKind::MethodCall {
                receiver, method, ..
            } => {
                // 接收者已知为字符串
                let recv_str = match &receiver.kind {
                    ExprKind::Lit(LitKind::Str(_)) | ExprKind::Lit(LitKind::FStr(_)) => true,
                    ExprKind::Var(n) => self.str_typed_vars.contains(n),
                    _ => self.is_str_producing(receiver),
                };
                if !recv_str {
                    return false;
                }
                // 字符串接收者上常见返回 String 的方法（join 返回拼接串，
                // to_upper/to_lower/replace/trim 等均返回 String）
                matches!(
                    method.as_str(),
                    "join"
                        | "to_upper"
                        | "to_lower"
                        | "upper"
                        | "lower"
                        | "replace"
                        | "trim"
                        | "strip"
                        | "lstrip"
                        | "rstrip"
                        | "title"
                        | "capitalize"
                        | "zfill"
                        | "ljust"
                        | "rjust"
                        | "center"
                        | "format"
                        | "removeprefix"
                        | "removesuffix"
                )
            }
            ExprKind::Call { callee, .. } => {
                // 自定义函数调用：检查函数返回类型是否为字符串
                let fname = match &callee.kind {
                    ExprKind::Var(n) => Some(n.as_str()),
                    _ => None,
                };
                if let Some(fname) = fname {
                    self.fn_returns.get(fname).map_or(false, |ret_ty| {
                        matches!(ret_ty, IrType::Str)
                            || matches!(ret_ty, IrType::Named { path, args: _ } if path == "String")
                    })
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// BUG-4：当前「期望类型」是否为 BigInt。
    /// Let 注解位（gen_stmt Let 在生成值前把声明类型写入 `current_expected_ty`）、
    /// 实参位、比较运算对方类型都走这条通道；字面量发射点只有 expr.ty 时
    /// 拿不到它（IR 里 `12345678901234567890` 的 expr.ty 是 int128，注解 bigint
    /// 挂在 let/const 上），于是 bigint 变量在函数体内静默坍塌成 i128。
    pub(crate) fn expected_is_bigint(&self) -> bool {
        matches!(&*self.current_expected_ty.borrow(), Some(t) if is_bigint_ty(t))
    }

    pub(crate) fn gen_lit(&self, lit: &LitKind, _ty: &IrType) -> String {
        match lit {
            LitKind::Int(n) => {
                // BigInt 槽位（let x: bigint = 100 / 期望类型来自 bigint 注解）：
                // 裸 `100i64` 与 BigInt 目标类型不符（E0308），需 from 包装
                if self.expected_is_bigint() {
                    return format!("{BIGINT_RS}::from({n})");
                }
                // @math 函数体内整数字面量经 T::from(2i32) 转换，
                // 使 `x * 2` 中 2 可推断为 T（裸 2 默认 i64，E0308；
                // f64 无 From<i64>，需 From<i32> 约束）。
                // 普通泛型函数（如 sum_measures 的 `total = 0`）不转换，
                // 否则 T::from(0i32) 返回 T 与 i64 变量冲突（E0308）
                if self.in_math_fn {
                    format!("T::from({}i32)", n)
                } else {
                    format!("{}i64", n)
                }
            }
            LitKind::Int128(n) => {
                // 目标类型为 BigInt（字面量自身类型 or Let/实参注入的期望类型）
                // 时生成 BigInt::from(...)，否则生成 i128 字面量
                if is_bigint_ty(_ty) || self.expected_is_bigint() {
                    return bigint_from_code(*n);
                }
                if self.in_math_fn {
                    format!("T::from({}i32)", *n as i32)
                } else {
                    format!("{}i128", *n)
                }
            }
            LitKind::BigInt(s) => bigint_lit_code(s),
            LitKind::Complex(re, im) => {
                format!("{COMPLEX_RS}::new({re:?}, {im:?})")
            }
            LitKind::F64(f) => {
                // NaN / 无穷：f64 文本化后为 "NaN"/"inf"/"-inf"，若走下方后缀逻辑
                // 会变成 `NaN.0f64` 等非法元组下标（math.lz 的 `const NAN = 0.0/0.0`
                // 被 comptime 内联为字面量时触发 E0658 / 非法 tuple index）。
                let s = f.to_string();
                let lower = s.to_ascii_lowercase();
                if lower == "nan" {
                    return "f64::NAN".to_string();
                } else if lower == "inf" {
                    return "f64::INFINITY".to_string();
                } else if lower == "-inf" {
                    return "(-f64::INFINITY)".to_string();
                }
                // 加 f64 后缀固定类型：泛型调用（@math）推断参数时，
                // 无后缀浮点字面量会探索 f32/f64/f128，触发 unstable f128（E0658）
                if s.contains('.') || s.contains('e') {
                    format!("{}f64", s)
                } else {
                    format!("{}.0f64", s)
                }
            }
            LitKind::Str(s) => {
                let escaped = s.escape_default().to_string();
                // 如果期望类型是 &str，生成裸字符串字面量而非 .to_string()
                let expected_ty = self.current_expected_ty.borrow();
                if let Some(IrType::Ref(inner) | IrType::MutRef(inner)) = expected_ty.as_ref() {
                    if matches!(inner.as_ref(), IrType::Str) {
                        return format!("\"{}\"", escaped);
                    }
                }
                format!("\"{}\".to_string()", escaped)
            }
            LitKind::FStr(s) => self.gen_fstring(s),
            LitKind::Bool(b) => b.to_string(),
            LitKind::Unit => "()".to_string(),
            LitKind::None_ => {
                // 自定义 `enum Option<T>`（lz_std/option.lz）场景：裸 None 需生成
                // Option::None（自定义枚举变体），否则 Rust 把 None 解析为 std
                // Option 变体 → E0308 expected Option<i64>, found Option<_>
                if self.known_types.contains("Option") {
                    "Option::None".to_string()
                } else {
                    "None".to_string()
                }
            }
        }
    }

    /// 二元操作的操作数包装：若生成的表达式是 unsafe 块（全局变量访问），
    /// 需加括号，否则 `unsafe { a } + unsafe { b }` 无法解析。
    pub(crate) fn wrap_bin_operand(&self, s: String) -> String {
        let trimmed = s.trim_start();
        if trimmed.starts_with("unsafe {") || trimmed.starts_with("unsafe{") {
            format!("({})", s)
        } else {
            s
        }
    }
    /// 字符串单字符索引的 String 形态直出（拼接/比较等字符串语境专用）：
    /// `s[i]` 的 IndexGet 通用路径按 current_ret_ty/current_expected_ty 判定，
    /// 落不到字符串语境时生成 i64 码点块——该块再被字符串比较/拼接约定包一层
    /// .to_string() 会变成**码点十进制数字串**（如 104 → "104"），运行期比较
    /// 永不相等（lib_json `while s[pos] != "\""` 跑飞 UnexpectedEnd 实测）。
    /// 对「str 基底 + 非 Range 键」的单字符索引直出 to_string 形态；
    /// 其余形态返回 None，调用方回退通用生成。
    pub(crate) fn gen_str_char_index_tostring(&self, e: &Expr) -> Option<String> {
        let (base, key) = match &e.kind {
            ExprKind::IndexGet { base, key } => (base, key),
            _ => return None,
        };
        let base_is_str = matches!(&base.ty, IrType::Str)
            || matches!(&base.ty, IrType::Named { path, .. }
                if path == "str" || path == "String")
            || (matches!(&base.ty, IrType::Any)
                && matches!(&base.kind,
                    ExprKind::MethodCall { method, receiver, .. }
                    if method == "clone" && matches!(&receiver.kind,
                        ExprKind::FieldAccess { field, .. }
                        if field == "s" || field == "source" || field == "input")))
            || (matches!(&base.ty, IrType::Any)
                && matches!(&base.kind,
                    ExprKind::FieldAccess { field, .. }
                    if field == "s" || field == "source" || field == "input"));
        if !base_is_str {
            return None;
        }
        if matches!(&key.kind, ExprKind::StructCtor { name, .. } if name == "Range") {
            return None; // 切片已有 .to_string() 形态，走通用路径
        }
        let base_s = self.gen_expr(base);
        let key_s = self.gen_expr(key);
        Some(format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = (({}) as usize); if __i >= __cs.len() {{ '\\0'.to_string() }} else {{ __cs[__i].to_string() }}}}", base_s, key_s))
    }

    pub(crate) fn binop_str(&self, op: &BinOpKind) -> &'static str {
        match op {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            BinOpKind::Div => "/",
            BinOpKind::Mod => "%",
            BinOpKind::Pow => "**", // 不应直接输出，由 gen_expr 特殊处理
            BinOpKind::Eq => "==",
            BinOpKind::Neq => "!=",
            BinOpKind::Lt => "<",
            BinOpKind::Gt => ">",
            BinOpKind::Le => "<=",
            BinOpKind::Ge => ">=",
            BinOpKind::And => "&&",
            BinOpKind::Or => "||",
            BinOpKind::BitAnd => "&",
            BinOpKind::BitOr => "|",
            BinOpKind::Xor => "^",
            BinOpKind::Shl => "<<",
            BinOpKind::Shr => ">>",
            BinOpKind::In => "in",
            BinOpKind::NotIn => "not_in", // 不应直接输出，由 gen_expr 特殊处理
        }
    }

    pub(crate) fn unop_str(&self, op: &UnOpKind) -> &'static str {
        match op {
            UnOpKind::Neg => "-",
            UnOpKind::Not => "!",
            UnOpKind::Ref => "&",
            UnOpKind::MutRef => "&mut ",
            UnOpKind::Deref => "*",
        }
    }
}
