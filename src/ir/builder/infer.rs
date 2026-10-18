// Lang-Zone 编译器 — ir/builder/infer.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

// ══════════════════════════════════════════════════════════════
// 类型推导：从 AST Expr 推导出 IrType
// ══════════════════════════════════════════════════════════════

pub(crate) fn infer_expr_type(ast_expr: &AstExpr, ctx: &TypeCtx) -> IrType {
    match ast_expr {
        // comptime 表达式：类型与内部表达式一致（B3 求值内联）
        AstExpr::Comptime(inner) => infer_expr_type(inner, ctx),
        AstExpr::IntLit(_) => IrType::Int,
        AstExpr::Int128Lit(_) => IrType::Int128,
        AstExpr::BigIntLit(_) => IrType::BigInt,
        AstExpr::ComplexLit(_, _) => IrType::Complex,
        AstExpr::FloatLit(_) => IrType::F64,
        AstExpr::StrLit(_) | AstExpr::FStrLit(_) | AstExpr::RawStrLit(_) => IrType::Str,
        AstExpr::BoolLit(_) => IrType::Bool,
        AstExpr::NoneLit => IrType::Any,     // None 类型取决于上下文
        AstExpr::DefaultExpr => IrType::Any, // default 类型取决于上下文
        // 展开元素：类型推导回退到内部表达式（仅在 ListLit 内被消费）
        AstExpr::Spread(inner) => infer_expr_type(inner, ctx),
        AstExpr::Ident(name) => {
            // 裸枚举变体名（Less/Equal/Greater）：类型应为枚举类型而非 Any→i64 fallback，
            // 否则 `return Less` 会插入 <Ordering as ImplicitFrom<i64>> 错误转换（E0277）
            if let Some(enum_name) = ctx.enum_variants.get(name.as_str()) {
                if !ctx.vars.contains_key(name.as_str()) {
                    IrType::Named {
                        path: enum_name.clone(),
                        args: vec![],
                    }
                } else {
                    ctx.lookup_var(name)
                }
            } else {
                ctx.lookup_var(name)
            }
        }
        AstExpr::Call {
            func,
            args,
            type_args,
        } => {
            // 检查 func 是否是枚举变体构造器（ParseError.UnexpectedChar(...)）
            if let AstExpr::FieldAccess { receiver, field } = func.as_ref() {
                if let AstExpr::Ident(enum_name) = receiver.as_ref() {
                    if ctx.enum_variants.contains_key(field)
                        && ctx.enum_variants.get(field) == Some(enum_name)
                    {
                        // 枚举变体构造器：返回枚举类型

                        return IrType::Named {
                            path: enum_name.clone(),
                            args: vec![],
                        };
                    }
                }
            }
            if let AstExpr::Ident(fname) = func.as_ref() {
                // __as__ 类型转换：返回目标类型
                if fname == "__as__" && args.len() == 2 {
                    if let AstExpr::Ident(type_name) = &args[1] {
                        return name_to_ir_type(type_name);
                    }
                    return IrType::Any;
                }
                // print/println/panic 是语言内建，返回 Unit
                if fname == "print" || fname == "println" || fname == "panic" {
                    return IrType::Unit;
                }
                // type_name(x) 内省内建：返回静态类型名字符串（与 v.type_name() 方法一致，方案 C）
                if fname == "type_name" && args.len() == 1 {
                    return IrType::Str;
                }
                // 宏系统（08-宏与编译期.md）：quote(...) 是宏体 Token 包装，
                // IR 后端不展开宏，返回 Str（宏体字符串拼接，参数数量不限）
                if fname == "quote" && !args.is_empty() {
                    return IrType::Str;
                }
                // str/int/float/bool 类型转换内建：返回对应类型
                match fname.as_str() {
                    "str" => return IrType::Str,
                    "int" => return IrType::Int,
                    "float" => return IrType::F64,
                    "bool" => return IrType::Bool,
                    "len" => return IrType::Int,
                    _ => {}
                }
                // range(start, end) / range(end) 内建：返回 Range<T>
                // 否则 `range(0,10).len()` 推断为 Any→i64，
                // codegen 走到通用 .len() 分支生成 `Range::len()` → E0599
                if fname == "range" && !args.is_empty() {
                    let elem_ty = args
                        .first()
                        .map(|a| infer_expr_type(a, ctx))
                        .unwrap_or(IrType::Int);
                    return IrType::Named {
                        path: "Range".into(),
                        args: vec![elem_ty],
                    };
                }
                // BUG-317：iter.lz 自由函数缺少类型推断 → 全部回退 Any→i64，
                // 导致 `collect_list(range(0,5)).len()` 生成 `collect_list(i64) -> i64` 存根
                // （E0308 expected i64 found Range）、`r.len()` E0599 on i64。
                // 显式按语义回传：collect_list→List<T>、range_inclusive/step_range→Range、
                // zip→List<(A,B)>、once/repeat_n/empty→对应迭代器（codegen 走 iter 适配）、
                // find→Option<T>、count_if/sum→T、product→T、for_each→Unit。
                if args.len() >= 1 || fname == "empty" || fname == "for_each" {
                    let elem_ty = args
                        .first()
                        .map(|a| infer_expr_type(a, ctx))
                        .unwrap_or(IrType::Any);
                    let elem_ty = if matches!(elem_ty, IrType::Any) {
                        // 若第一参是迭代器（Range/List），取其 Item；否则 Any→Int
                        IrType::Int
                    } else {
                        elem_ty
                    };
                    match fname.as_str() {
                        // collect_list(iterable) → List<Item>
                        // 参数若是 Iter<X> 需取出 X 作为元素类型（否则 List<Iter<X>>）
                        "collect_list" | "collect" => {
                            let list_ty = if let IrType::Named { args: inner, .. } = &elem_ty {
                                inner.first().cloned().unwrap_or(IrType::Int)
                            } else {
                                elem_ty
                            };
                            return IrType::Named {
                                path: "List".into(),
                                args: vec![list_ty],
                            }
                        }
                        // range_inclusive/step_range 同 range 语义（迭代器）
                        "range_inclusive" | "step_range" => {
                            return IrType::Named {
                                path: "Range".into(),
                                args: vec![elem_ty],
                            }
                        }
                        // once/repeat_n/empty：迭代器包装（codegen 走 iter 适配）
                        "once" | "repeat" | "repeat_n" | "empty" => {
                            return IrType::Named {
                                path: "Iter".into(),
                                args: vec![elem_ty],
                            }
                        }
                        // zip(a, b) → 元素类型为 (ItemA, ItemB) 元组
                        // 两个参数都是迭代器（Range/Iter/List），需取出各自元素类型
                        "zip" => {
                            let unwrap_elem = |t: IrType| -> IrType {
                                if let IrType::Named { args: inner, .. } = &t {
                                    inner.first().cloned().unwrap_or(IrType::Int)
                                } else {
                                    IrType::Int
                                }
                            };
                            let a_ty = unwrap_elem(elem_ty);
                            let b_ty = args
                                .get(1)
                                .map(|a| unwrap_elem(infer_expr_type(a, ctx)))
                                .unwrap_or(IrType::Int);
                            return IrType::Named {
                                path: "Iter".into(),
                                args: vec![IrType::Tuple(vec![a_ty, b_ty])],
                            }
                        }
                        // find → Option<Item>（elem_ty 是集合类型，需取出元素类型）
                        "find" | "find_iter" => {
                            let item_ty = if let IrType::Named { args: inner, .. } = &elem_ty {
                                inner.first().cloned().unwrap_or(IrType::Int)
                            } else {
                                elem_ty
                            };
                            return IrType::Option(Box::new(item_ty))
                        }
                        // map/filter 自由函数 → List<T>（闭包返回 U 时是 List<U>，这里先按
                        // 迭代器元素类型兜底，闭包推断在 codegen 层负责）
                        "map" | "filter" => {
                            let list_ty = if let IrType::Named { args: inner, .. } = &elem_ty {
                                inner.first().cloned().unwrap_or(IrType::Int)
                            } else {
                                elem_ty
                            };
                            return IrType::Named {
                                path: "List".into(),
                                args: vec![list_ty],
                            }
                        }
                        // count_if → Int（计数，与 sum/product 不同）
                        "count_if" => return IrType::Int,
                        // sum/product → Item（元素类型，不是集合类型）
                        "sum" | "product" => {
                            let item_ty = if let IrType::Named { args: inner, .. } = &elem_ty {
                                inner.first().cloned().unwrap_or(IrType::Int)
                            } else {
                                IrType::Int
                            };
                            return item_ty;
                        }
                        // for_each → Unit
                        "for_each" => return IrType::Unit,
                        _ => {}
                    }
                }
                // Ok/Err/Some/None 变体构造：返回 Result/Option 类型。
                // 否则 `let ok = Ok(10)` 推断为 Any，后续 ok.map(...) 无法
                // 触发 codegen 的消费型方法 clone 注入（E0382 moved value）
                match fname.as_str() {
                    "Ok" => {
                        let ok_ty = args
                            .first()
                            .map(|a| infer_expr_type(a, ctx))
                            .unwrap_or(IrType::Any);
                        return IrType::Result {
                            ok: Box::new(ok_ty),
                            err: Box::new(IrType::Any),
                        };
                    }
                    "Err" => {
                        let err_ty = args
                            .first()
                            .map(|a| infer_expr_type(a, ctx))
                            .unwrap_or(IrType::Any);
                        return IrType::Result {
                            ok: Box::new(IrType::Any),
                            err: Box::new(err_ty),
                        };
                    }
                    "Some" => {
                        let inner = args
                            .first()
                            .map(|a| infer_expr_type(a, ctx))
                            .unwrap_or(IrType::Any);
                        return IrType::Option(Box::new(inner));
                    }
                    "None" => return IrType::Option(Box::new(IrType::Any)),
                    _ => {}
                }
                if ctx.is_struct(fname) { return IrType::named(fname);
                }
                // 容器空构造：`List()` / `Set()` / `Dict()` 返回对应容器类型，
                // 否则推断为 Any→i64（`let mut result = List()` 后 `return result`
                // 生成 <Vec<U> as ImplicitFrom<i64>> 错误转换，E0277）
                if args.is_empty() {
                    match fname.as_str() {
                        "List" | "Vec" => {
                            return IrType::Named {
                                path: "List".into(),
                                args: vec![IrType::Any],
                            }
                        }
                        "Set" | "HashSet" => {
                            return IrType::Named {
                                path: "Set".into(),
                                args: vec![IrType::Any],
                            }
                        }
                        "Dict" | "HashMap" => {
                            return IrType::Named {
                                path: "Dict".into(),
                                args: vec![IrType::Any, IrType::Any],
                            }
                        }
                        _ => {}
                    }
                }
                // 闭包变量调用：consume() 中 consume 是局部 Fn 变量 →
                // 返回闭包体 ret 类型（否则 lookup_fn_return 回退 Any→i64，E0308）
                if let IrType::Fn { ret, .. } = ctx.lookup_var(fname) {
                    return *ret;
                }
                let ret_ty = ctx.lookup_fn_return(fname);
                // 显式 turbofish 类型参数（parse_num.<int>("42")）：
                // 直接用 type_args 替换返回类型中的泛型（否则 Result<T, String> 中 T 未绑定，E0425）
                if !type_args.is_empty() && ret_ty.contains_generics() {
                    return apply_explicit_type_args(&ret_ty, type_args);
                }
                // 泛型分辨率：如果返回类型包含 Generic，尝试从实参推断
                if ret_ty.contains_generics() {
                    if let Some(param_tys) = ctx.fn_params.get(fname) {
                        let arg_tys: Vec<IrType> =
                            args.iter().map(|a| infer_expr_type(a, ctx)).collect();
                        // 根据参数类型推断泛型变量
                        let resolved =
                            resolve_call_generics(&ret_ty, fname, param_tys, &arg_tys, ctx);
                        return resolved;
                    }
                }
                ret_ty
            } else {
                IrType::Any
            }
        }
        AstExpr::MethodCall {
            receiver,
            method,
            args: _,
            ..
        } => {
            // 枚举变体构造: Kind.A(1) → Kind 类型
            // receiver 是枚举/结构类型名时，方法名是变体
            if let AstExpr::Ident(recv_name) = receiver.as_ref() {
                let base = recv_name.split('<').next().unwrap_or(recv_name);
                if ctx.is_struct(base) || ctx.enum_variants.values().any(|e| e == base) {
                    return IrType::Named {
                        path: base.to_string(),
                        args: vec![],
                    };
                }
            }
            // 尝试从 receiver 类型推导方法返回类型

            let recv_ty = infer_expr_type(receiver, ctx);

            // 检查 clone 方法
            if method == "clone" {}
            // size_hint() 返回 (int, Option<int>)（LZ 视角 int；codegen 在
            // impl Iterator 中映射为 usize）。iter.lz Zip::size_hint 中
            // `self.a.size_hint()` 若不推断，min(lo_a, lo_b) 报 E0308
            if method == "size_hint" || method == "__size_hint__" {
                return IrType::Tuple(vec![IrType::Int, IrType::Option(Box::new(IrType::Int))]);
            }
            // 常见无返回值方法 → Unit
            if method == "push"
                || method == "insert"
                || method == "clear"
                || method == "append"
                || method == "set"
                || method == "skip_ws"
            {
                return IrType::Unit;
            }
            // 比较魔术方法（__eq__/__ne__/__lt__/__gt__/__le__/__ge__）→ Bool：
            // option.lz `a.__eq__(b)` 推断为 Bool，否则回退 i64 导致
            // `<bool as ImplicitFrom<i64>>::__implicit_from__(a == b)`（E0277）
            if method == "__eq__"
                || method == "__ne__"
                || method == "__lt__"
                || method == "__gt__"
                || method == "__le__"
                || method == "__ge__"
            {
                return IrType::Bool;
            }
            // 内置方法返回类型推断表
            if let Some(ret) = lookup_builtin_method_ret(&recv_ty, method, ctx) {
                return ret;
            }
            match &recv_ty {
                IrType::Named { path, .. } => {
                    // 简单启发式：Option::unwrap → 内部类型
                    if method == "unwrap" || method == "expect" {
                        if path == "Option" {
                            return IrType::Any; // 无法从类型名推断内部类型
                        }
                    }
                    if method == "len" {
                        return IrType::Int;
                    }
                    // clone() 返回接收者类型（Rc/Arc/Box 等）
                    if method == "clone" && (path == "Rc" || path == "Arc" || path == "Box") {
                        return recv_ty.clone();
                    }
                    // String/str.clone() 返回 String（Rust 的 Clone impl）
                    if method == "clone" && (path == "String" || path == "str") {
                        return IrType::Named {
                            path: "String".into(),
                            args: vec![],
                        };
                    }

                    // 用户 struct 方法：从登记的方法返回类型查询（box.lz `get` 返回
                    // `ref T`，否则 `b.get()` 推断为 Any，`assert b.get() == 42`
                    // 无法解引用，E0277 can't compare &i64 with i64）
                    if ctx
                        .struct_methods
                        .get(path)
                        .map(|ms| ms.contains(method))
                        .unwrap_or(false)
                    {
                        let mret = ctx.lookup_fn_return(&format!("{}.{}", path, method));
                        if !matches!(mret, IrType::Any) {
                            return mret;
                        }
                    }
                    // 用户 struct 的算术/构造魔术方法返回接收者类型
                    if ctx
                        .struct_methods
                        .get(path)
                        .map(|ms| ms.contains(method))
                        .unwrap_or(false)
                        && matches!(
                            method.as_str(),
                            "__add__"
                                | "__sub__"
                                | "__mul__"
                                | "__div__"
                                | "__new__"
                                | "__call__"
                                | "__getitem__"
                                | "__iter__"
                                | "__setitem__"
                        )
                    {
                        return recv_ty.clone();
                    }
                }
                _ => {}
            }
            IrType::Any
        }
        AstExpr::FieldAccess { receiver, field } => {
            let recv_ty = infer_expr_type(receiver, ctx);
            // self 是 &mut self / &self 时，解引用后再查字段类型
            // （skip_ws(mut self) → self 类型为 MutRef(Named("Parser"))，
            // 不解引用则 self.s 推断为 Any，str 字段索引 codegen 退化）
            let base_ty = match &recv_ty {
                IrType::Ref(inner) | IrType::MutRef(inner) => inner.as_ref().clone(),
                other => other.clone(),
            };
            match &base_ty {
                IrType::Named { path, .. } => ctx.lookup_field(path, field),
                // BUG-313：元组字段访问 `t.1` / `inner.1`（LZ 元组索引语义）。
                // 未处理时回退 Any→i64，`inner.1`（bool 元素）误生成
                // `<bool as ImplicitFrom<i64>>::__implicit_from__(inner.1)` → E0277
                IrType::Tuple(items) => field
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| items.get(i).cloned())
                    .unwrap_or(IrType::Any),
                _ => IrType::Any,
            }
        }
        AstExpr::Index { receiver, index } => {
            // `self[key]` 索引类型：从容器类型推断元素类型，而不是恒为 Any→i64。
            // Dict<K,V> → V；List<T>/Vec<T> → T；Str → Char（单字符索引）或 Str（切片）；否则 Any
            let recv_ty = infer_expr_type(receiver, ctx);

            // 检查是否为切片操作（Range 索引）
            let is_slice = matches!(&**index, AstExpr::Range { .. });
            match &recv_ty {
                IrType::Named { path, args } if path == "Dict" || path == "HashMap" => {
                    args.get(1).cloned().unwrap_or(IrType::Any)
                }
                IrType::Named { path, args } if path == "List" || path == "Vec" => {
                    args.first().cloned().unwrap_or(IrType::Any)
                }
                IrType::Str if is_slice => IrType::Str, // 字符串切片返回 Str
                IrType::Str => IrType::Str,             // 字符索引返回 Str（LZ 语义）
                IrType::Named { path, .. } if path == "String" || path == "str" => {
                    if is_slice {
                        IrType::Named {
                            path: "String".into(),
                            args: vec![],
                        }
                    } else {
                        IrType::Str // 字符索引返回 Str（LZ 语义）
                    }
                }
                // 元组索引 t[i]：从元组类型取对应元素类型
                // BUG-287：先前回退 Any→i64，`return t[1]`（t: (int, str)）误插
                // <String as ImplicitFrom<i64>>，E0277
                IrType::Tuple(items) => match &**index {
                    AstExpr::IntLit(n) => {
                        items.get(*n as usize).cloned().unwrap_or(IrType::Any)
                    }
                    _ => IrType::Any,
                },
                _ => IrType::Any,
            }
        }
        AstExpr::Binary { left, op, right } => {
            // `is` 运算符始终返回 Bool
            if matches!(op, BinOp::Is) {
                return IrType::Bool;
            }
            // 比较/布尔运算符返回 Bool
            if matches!(
                op,
                BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::And
                    | BinOp::Or
                    | BinOp::In
                    | BinOp::NotIn
            ) {
                return IrType::Bool;
            }
            // 取左侧操作数的类型（简化）；左侧未知（Any）时回退到右侧，
            // 否则 `n * 10`（n 为 walrus 变量未登记）推断为 Any，三元条件
            // 无法触发 codegen 的真值转换（combo_ternary_walrus.lz E0308）
            let lt = infer_expr_type(left, ctx);
            let lt = if matches!(&lt, IrType::Any) {
                infer_expr_type(right, ctx)
            } else {
                lt
            };
            // int 与 f64 混合算术 → f64（Rust 语义 i64 * f64 不存在，需 as 提升；
            // 否则推断为 i64，return 处包 ImplicitFrom<i64> → f64 报 E0277）
            if matches!(
                op,
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
            ) && matches!(&lt, IrType::Int)
            {
                let rt = infer_expr_type(right, ctx);
                if matches!(&rt, IrType::F64) {
                    return IrType::F64;
                }
            }
            lt
        }
        AstExpr::Unary { op, operand } => match op {
            _ => infer_expr_type(operand, ctx),
        },
        AstExpr::If { then_body, .. } => {
            // 取 then 分支最后表达式类型
            then_body
                .last()
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Match { expr, arms } => {
            // type-pack 异质元组（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的 args
            // 编译为 List<Ts> 切片，`case (a,)` / `case (a, ..)` 臂体返回 a（元素 &Ts）。
            // 从 scrutinee 元素类型 + 模式绑定变量推断臂体返回类型（否则 Any→i64 误判）
            let scrut_ty = infer_expr_type(expr, ctx);
            let elem_ty = match &scrut_ty {
                IrType::Named { args, .. } if !args.is_empty() => Some(args[0].clone()),
                _ => None,
            };
            if let (Some(arm), Some(elem)) = (arms.first(), elem_ty) {
                let mut binds = Vec::new();
                collect_ast_pattern_vars(&arm.pattern, &mut binds);
                // 臂体返回绑定变量（如 `case (a,) => a`）→ 返回元素类型
                if let Some(last) = arm.body.last() {
                    if let AstStmt::Expr(AstExpr::Ident(n)) = last {
                        if binds.iter().any(|b| b == n) {
                            return elem;
                        }
                    }
                }
            }
            arms.first()
                .and_then(|arm| arm.body.last())
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Closure {
            params,
            param_tys,
            ret_ty,
            body,
        } => {
            // 闭包有显式参数注解时构造 Fn 类型，供泛型调用推断使用：
            // 否则 `map_res(a, |x: int| -> int = x*2)` 闭包推断为 Any，导致
            // 泛型绑定零命中（lib_result and_then 链式调用 RUSTC_FAIL）
            if param_tys.iter().any(|t| t.is_some()) {
                let mut sub = ctx.clone();
                for (i, name) in params.iter().enumerate() {
                    let declared = param_tys
                        .get(i)
                        .and_then(|t| t.as_ref())
                        .map(|t| from_ast_type(t))
                        .unwrap_or(IrType::Any);
                    sub.vars.insert(name.clone(), declared);
                }
                // 返回类型：优先用显式 `-> T` 注解（如 `|x: int| -> str = ...`），
                // 否则从闭包体推断。多语句体（`|a| => let x = …; x`）需顺序登记
                // let 绑定后推断尾表达式；否则尾表达式（可能是闭包）lookup_var 失败
                // → Any → i64，触发 E0308（nesting-closure-lambda 的三层嵌套闭包：
                // outer 返回 middle、middle 返回 inner，均被误推断为 i64）。
                let ret = match ret_ty {
                    Some(t) => from_ast_type(t),
                    None => infer_closure_body_type(body, &sub),
                };
                IrType::Fn {
                    params: params
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            param_tys
                                .get(i)
                                .and_then(|t| t.as_ref())
                                .map(|t| from_ast_type(t))
                                .unwrap_or(IrType::Any)
                        })
                        .collect(),
                    ret: Box::new(ret),
                }
            } else {
                IrType::Any
            }
        }
        AstExpr::BlockExpr(_) => IrType::Any,
        AstExpr::Range { .. } => IrType::named("Range"),
        AstExpr::Walrus { value, .. } => infer_expr_type(value, ctx),
        AstExpr::Pipe { callee, .. } => {
            // 管道结果类型 = 右侧 callable 的返回类型：
            // - Ident 是已知 struct（构造调用）→ struct 类型本身
            // - Ident 是变量（__call__ 实例）→ 变量类型（__call__ 通常返回同类型）
            // - Ident 是函数 → 函数返回类型；闭包 → Any
            let mut r =
                match callee.as_ref() {
                    AstExpr::Ident(name) => {
                        if ctx.is_struct(name) {
                            // 构造调用 Point(2.0) → Point 类型（首参预填充 receiver）
                            IrType::Named {
                                path: name.clone(),
                                args: vec![],
                            }
                        } else if let IrType::Named { path, .. } = ctx.lookup_var(name) {
                            // 变量实例：若类型实现了 __call__，管道结果为 __call__ 返回类型
                            // （推断期近似为实例类型本身，Point.__call__ 返回 Point）
                            if ctx.struct_methods.get(&path).map_or(false, |m| {
                                m.contains("__call__") || m.contains("__rpipe__")
                            }) {
                                IrType::Named {
                                    path: path.clone(),
                                    args: vec![],
                                }
                            } else {
                                ctx.lookup_fn_return(name)
                            }
                        } else {
                            ctx.lookup_fn_return(name)
                        }
                    }
                    AstExpr::Closure { .. } => IrType::Any,
                    _ => infer_expr_type(callee, ctx),
                };
            // 管道应用语义：可调用的返回类型本身若是 fn（如 if_func 返回 fn(int)->int），
            // 则管道结果为最终一层返回类型
            while let IrType::Fn { ret, .. } = &r {
                r = (**ret).clone();
            }
            r
        }
        AstExpr::Try(inner) => {
            // try 表达式：Result → Ok 类型
            let inner_ty = infer_expr_type(inner, ctx);
            match &inner_ty {
                IrType::Result { ok, .. } => *ok.clone(),
                _ => IrType::Any,
            }
        }
        AstExpr::NullCoalesce { left, right } => {
            let left_ty = infer_expr_type(left, ctx);
            let right_ty = infer_expr_type(right, ctx);
            let is_right_option = matches!(&right_ty, IrType::Option(_))
                || matches!(&right_ty, IrType::Named { path, .. } if path == "Option");
            if is_right_option {
                // Option<T> ?? Option<T> → Option<T>
                left_ty
            } else {
                // Option<T> ?? T → T（解包）
                match &left_ty {
                    IrType::Option(inner) => *inner.clone(),
                    IrType::Named { path, args } if path == "Option" && !args.is_empty() => {
                        args[0].clone()
                    }
                    _ => left_ty,
                }
            }
        }
        AstExpr::ListLit(items) => {
            // 元素类型推断（BUG-SG-005）：
            // 1) 优先取首个「非展开」元素的类型；
            // 2) 若全部是展开元素，则从首个展开的「内部元素类型」推导
            //    （Spread(x) 的 x 若为 List[T]/Vec[T]，元素类型为 T，而非 List[T]）。
            let elem_ty = items
                .iter()
                .find_map(|i| match i {
                    AstExpr::Spread(_) => None,
                    other => Some(infer_expr_type(other, ctx)),
                })
                .or_else(|| {
                    items.iter().find_map(|i| match i {
                        AstExpr::Spread(inner) => match infer_expr_type(inner, ctx) {
                            IrType::Named { path, args }
                                if (path == "List" || path == "Vec" || path == "Set")
                                    && !args.is_empty() =>
                            {
                                Some(args[0].clone())
                            }
                            other => Some(other),
                        },
                        _ => None,
                    })
                })
                .unwrap_or(IrType::Any);
            IrType::Named {
                path: "List".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::DictLit(entries) => {
            let key_ty = entries
                .first()
                .and_then(|(k, _)| Some(infer_expr_type(k, ctx)))
                .unwrap_or(IrType::Any);
            let val_ty = entries
                .first()
                .and_then(|(_, v)| Some(infer_expr_type(v, ctx)))
                .unwrap_or(IrType::Any);
            IrType::Named {
                path: "Dict".into(),
                args: vec![key_ty, val_ty],
            }
        }
        AstExpr::SetLit(items) => {
            let elem_ty = items
                .first()
                .map(|i| infer_expr_type(i, ctx))
                .unwrap_or(IrType::Any);
            IrType::Named {
                path: "Set".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::TupleLit(elems) => {
            IrType::Tuple(elems.iter().map(|e| infer_expr_type(e, ctx)).collect())
        }
        AstExpr::ListComprehension { output, .. } => {
            let elem_ty = infer_expr_type(output, ctx);
            IrType::Named {
                path: "List".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::DictComprehension { key, value, .. } => {
            let k_ty = infer_expr_type(key, ctx);
            let v_ty = infer_expr_type(value, ctx);
            IrType::Named {
                path: "Dict".into(),
                args: vec![k_ty, v_ty],
            }
        }
        AstExpr::SetComprehension { elem, .. } => {
            let elem_ty = infer_expr_type(elem, ctx);
            IrType::Named {
                path: "Set".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::Assign { value, .. } => infer_expr_type(value, ctx),
        AstExpr::Spawn(inner) => {
            let it = infer_expr_type(inner, ctx);
            match &it {
                IrType::Named { path, .. } if path == "Future" => it,
                _ => IrType::Named {
                    path: "Future".into(),
                    args: vec![it],
                },
            }
        }
        AstExpr::Move(inner) => infer_expr_type(inner, ctx),
        AstExpr::Panic(_) => IrType::Never,
        AstExpr::Await(inner) => {
            // await Future<T> → T
            let inner_ty = infer_expr_type(inner, ctx);
            match &inner_ty {
                IrType::Named { path, args } if path == "Future" && !args.is_empty() => {
                    args[0].clone()
                }
                _ => IrType::Any,
            }
        }
        AstExpr::BuildBlock { kind, lhs, body } => {
            // =: / ~: / *: 构建块返回 lhs 类型（或块类型）
            // ^: 索引构建块返回 lhs 的元素类型（如 Vec<T> → T）
            let lhs_ty = infer_expr_type(lhs, ctx);
            match kind {
                BuildKind::Index => match &lhs_ty {
                    IrType::Named { args, .. } if !args.is_empty() => args[0].clone(),
                    _ => lhs_ty,
                },
                BuildKind::Call => {
                    // ~: 构建块的实际返回类型是 callee 的返回值类型
                    // body 的类型是元组（被解包为 callee 的参数），不是调用结果
                    match &lhs_ty {
                        IrType::Fn { ret, .. } => *ret.clone(),
                        _ => IrType::Any, // callee 类型未知，用 Any 避免错误类型标注
                    }
                }
                BuildKind::Gen => {
                    // *: 生成器构建块 → List<元素类型>
                    // 有 callee（函数/方法引用）时元素类型 = callee 返回类型；
                    // 否则从 body 中第一个 yield 表达式推导
                    let has_callee = matches!(
                        &**lhs,
                        AstExpr::Ident(_)
                            | AstExpr::MethodCall { .. }
                            | AstExpr::FieldAccess { .. }
                    );
                    // 优先从函数符号表取返回类型（Ident 直接引用函数时 lookup_var 回退 Any，
                    // 会导致 *: 构建块元素类型错误地取 yield 包类型）
                    let elem_ty = if let AstExpr::Ident(fname) = &**lhs {
                        if let Some(ret) = ctx.fn_returns.get(fname) {
                            ret.clone()
                        } else {
                            infer_yield_elem_ty(body, lhs_ty, ctx)
                        }
                    } else if has_callee {
                        match &lhs_ty {
                            IrType::Fn { ret, .. } => *ret.clone(),
                            _ => infer_yield_elem_ty(body, lhs_ty, ctx),
                        }
                    } else {
                        infer_yield_elem_ty(body, lhs_ty, ctx)
                    };
                    IrType::Named {
                        path: "List".into(),
                        args: vec![elem_ty],
                    }
                }
                BuildKind::Var => {
                    // =: 构建块返回块体末尾表达式类型（如 (a,b,c) 元组）。
                    // lhs 是目标变量名（此处尚未登记，lookup_var 回退 Any），
                    // 故优先用 body 末尾语句推断，否则 `multiply ~: factors`
                    // 的元组拆包会因 factors 类型为 Any 而失败（E0061）
                    let last_ty = body
                        .last()
                        .map(|s| infer_stmt_type(s, ctx))
                        .filter(|t| !matches!(t, IrType::Any))
                        .unwrap_or(lhs_ty);
                    last_ty
                }
            }
        }
        AstExpr::KwArg { .. } => IrType::Any,
        AstExpr::PathAccess { .. } => IrType::Any,
        AstExpr::SafeNav { .. } => IrType::Any,
        AstExpr::TryCatch {
            body,
            catches,
            else_body,
            ..
        } => {
            // try/catch 表达式返回类型：
            // - 有 else_body 时取 else 臂类型；
            // - 无 else 但有 catch 时，取**最后一个 catch 分支**尾表达式类型
            //   （try body 尾若是 raises 调用 Result<T,E>，catch 已解包错误，
            //   整体值类型是 T 而非 Result<T,E>；否则 try_catch_no_pattern 等
            //   无注解函数会被误推断为 Result<i64,String> → E0308）；
            // - 两者皆无（仅 finally）时取 try body 尾表达式。
            let src: &Vec<AstStmt> = match else_body {
                Some(el) => el,
                None => match catches.last() {
                    Some(last) => &last.body,
                    None => body,
                },
            };
            src.iter()
                .rev()
                .find(|s| matches!(s, AstStmt::Expr(_) | AstStmt::Return(_) | AstStmt::Yield(_)))
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Paren(inner) => infer_expr_type(inner, ctx),
    }
}

/// 闭包体返回类型推断：单表达式体直接推断；多语句体（BlockExpr）顺序登记
/// let 绑定后推断尾表达式类型，使尾表达式为闭包时能拿到真实 Fn 类型
/// （nesting-closure-lambda 的 outer/middle 嵌套闭包）。
fn infer_closure_body_type(body: &AstExpr, ctx: &TypeCtx) -> IrType {
    match body {
        AstExpr::BlockExpr(stmts) => {
            let mut cur = ctx.clone();
            let mut last_ty = IrType::Unit;
            let n = stmts.len();
            for (i, s) in stmts.iter().enumerate() {
                if let AstStmt::Let {
                    name, ty, value, ..
                } = s
                {
                    let vty = match ty {
                        Some(t) => from_ast_type(t),
                        None => infer_expr_type(value, &cur),
                    };
                    cur.vars.insert(name.clone(), vty);
                }
                if i + 1 == n {
                    last_ty = infer_stmt_type(s, &cur);
                }
            }
            last_ty
        }
        _ => infer_expr_type(body, ctx),
    }
}

pub(crate) fn infer_stmt_type(stmt: &AstStmt, ctx: &TypeCtx) -> IrType {
    match stmt {
        AstStmt::Expr(e) => {
            // go expr 作为语句使用时值被丢弃，不污染函数返回类型
            // （规范 10-并发与异步.md：`let x: Future<int> = go f()` 绑定上下文
            // 才返回 Future<T>；尾语句 `go f()` 应视为 Unit，避免无返回注解
            // 的 def main() 被推断为 Future<()> 触发 typed main 分支 E0782）
            if matches!(e, AstExpr::Spawn(_)) {
                IrType::Unit
            } else {
                infer_expr_type(e, ctx)
            }
        }
        AstStmt::Pass => IrType::Unit,
        AstStmt::TypeAlias { .. } => IrType::Unit,
        AstStmt::Check { .. } => IrType::Unit,
        // let 是声明不是值表达式：作为尾语句推断返回 Unit，
        // 否则无返回注解函数（如 def scope_defer() = ... let main_val = 30）
        // 会误推断为 Any→i64，与体实际返回 () 冲突（E0308）
        AstStmt::Let { .. } => IrType::Unit,
        AstStmt::Return(Some(e)) => infer_expr_type(e, ctx),
        AstStmt::Return(None) => IrType::Unit,
        AstStmt::Yield(Some(e)) => IrType::Named {
            path: "Itor".into(),
            args: vec![infer_expr_type(e, ctx)],
        },
        AstStmt::Yield(None) => IrType::Unit,
        AstStmt::YieldFrom(e) => IrType::Named {
            path: "Itor".into(),
            args: vec![infer_expr_type(e, ctx)],
        },
        _ => IrType::Unit,
    }
}

/// 内置方法返回类型推断表
/// 为常用内置类型提供方法返回类型推断
pub(crate) fn lookup_builtin_method_ret(
    recv_ty: &IrType,
    method: &str,
    _ctx: &TypeCtx,
) -> Option<IrType> {
    match recv_ty {
        // Iterator<T> 方法
        IrType::Named { path, args } if path == "Iterator" && args.len() == 1 => match method {
            "next" => Some(IrType::Option(Box::new(args[0].clone()))),
            "len" | "count" => Some(IrType::Int),
            "collect" => Some(IrType::Named {
                path: "List".into(),
                args: args.clone(),
            }),
            "filter" | "map" | "take" | "skip" | "cloned" | "skip_while" | "take_while" => {
                Some(recv_ty.clone())
            }
            _ => None,
        },
        // Range<T> 视作 Iterator<T>（BUG-294：Range 未登记，`r.collect()` 回退 Any→?）
        IrType::Named { path, args } if path == "Range" && args.len() == 1 => match method {
            "next" => Some(IrType::Option(Box::new(args[0].clone()))),
            "len" | "count" => Some(IrType::Int),
            "collect" => Some(IrType::Named {
                path: "List".into(),
                args: args.clone(),
            }),
            "filter" | "map" | "take" | "skip" | "cloned" | "skip_while" | "take_while" => {
                Some(recv_ty.clone())
            }
            _ => None,
        },
        // Int 内建方法（abs 等保持 Int）
        IrType::Int => match method {
            "abs" => Some(IrType::Int),
            _ => None,
        },
        // F64 内建方法（abs 等保持 F64）
        IrType::F64 => match method {
            "abs" => Some(IrType::F64),
            _ => None,
        },
        // List<T> 方法
        IrType::Named { path, args } if path == "List" && args.len() == 1 => match method {
            "len" | "size" => Some(IrType::Int),
            "clone" => Some(recv_ty.clone()),
            "iter" => Some(IrType::Named {
                path: "Iterator".into(),
                args: args.clone(),
            }),
            // get/pop → T（LZ 用 -1/"" 编码 None，代码生成 unwrap_or 平铺）
            // BUG-281：先前标 Option<T>，`let r = xs.get(1); match r { -1 => ... }`
            // 报 E0308（Option<i64> 无法匹配 -1i64）
            "get" | "pop" => Some(args[0].clone()),
            "first" | "last" => Some(args[0].clone()),
            // remove → T（Vec::remove 返回 V）
            "remove" => Some(args[0].clone()),
            // remove_at → Option<T>（list.lz:89 签名 `Option<T>`，越界 None）
            "remove_at" => Some(IrType::Option(Box::new(args[0].clone()))),
            // contains 成员测试 → Bool（否则回退 Any→i64，E0277）
            "contains" | "starts_with" | "ends_with" | "is_empty" => Some(IrType::Bool),
            // index_of → Option<Int>（list.lz:48 签名 `Option<int>`）
            "index_of" => Some(IrType::Option(Box::new(IrType::Int))),
            // count_of → Int
            "count_of" => Some(IrType::Int),
            // find/rfind → Int（LZ 用 -1 编码 None，lz_find 返回 Option<i64> 由代码生成
            // 转成 `unwrap_or(-1)` 平铺为 i64）
            "find" | "rfind" => Some(IrType::Int),
            // join → String（`",".join(parts)` 生成 `parts.join(",")`）
            "join" => Some(IrType::Str),
            // reversed/unique/slice → List<T>
            "reversed" | "unique" => Some(recv_ty.clone()),
            "slice" => Some(recv_ty.clone()),
            // extend → Unit
            "extend" => Some(IrType::Unit),
            // sort/reverse → Unit：Python 风格原地修改，返回 void（LZ 无 sorted）
            "sort" | "reverse" => Some(IrType::Unit),
            _ => None,
        },
        // Option<T> 方法
        IrType::Option(inner) => match method {
            "unwrap" | "expect" => Some((**inner).clone()),
            "map" | "and_then" => Some(recv_ty.clone()),
            "is_some" | "is_none" => Some(IrType::Bool),
            _ => None,
        },
        IrType::Named { path, args } if path == "Option" && args.len() == 1 => match method {
            "unwrap" | "expect" => Some(args[0].clone()),
            "map" | "and_then" => Some(recv_ty.clone()),
            "is_some" | "is_none" => Some(IrType::Bool),
            _ => None,
        },
        // Result<T,E>.unwrap() / expect() → T
        IrType::Result { ok, .. } => match method {
            "unwrap" | "expect" => Some((**ok).clone()),
            "map" | "and_then" => Some(recv_ty.clone()),
            // Result<T,E>.is_ok() / is_err() → bool（此前缺失导致 is_ok 返回 Any，
            // codegen 把它当 i64 用 ImplicitFrom<bool,i64> 包装 → E0277）
            "is_ok" | "is_err" => Some(IrType::Bool),
            _ => None,
        },
        // Result<T,E>.unwrap() / expect() → T
        IrType::Named { path, args } if path == "Result" && args.len() >= 1 => match method {
            "unwrap" | "expect" => Some(args[0].clone()),
            "map" | "and_then" => Some(recv_ty.clone()),
            // Result<T,E>.is_ok() / is_err() → bool（BUG-309）
            "is_ok" | "is_err" => Some(IrType::Bool),
            _ => None,
        },
        // String 方法
        IrType::Named { path, .. } if path == "str" || path == "String" => match method {
            "len" => Some(IrType::Int),
            "clone" => Some(recv_ty.clone()),
            // BUG-44：to_string() 在 str/String 上应返回 String，使 print 走 Display 而非 Debug
            "to_string" => Some(IrType::Named {
                path: "String".into(),
                args: vec![],
            }),
            // str/String 上的 find/rfind → Int（LZ 用 -1 编码 None）
            "find" | "rfind" => Some(IrType::Int),
            "contains" | "starts_with" | "ends_with" | "is_empty" => Some(IrType::Bool),
            "slice" => Some(IrType::Str),
            "join" => Some(IrType::Str),
            _ => None,
        },
        // Str 类型（ir::Str）的方法
        IrType::Str => match method {
            "len" => Some(IrType::Int),
            "clone" => Some(IrType::Named {
                path: "String".into(),
                args: vec![],
            }),
            "to_string" => Some(IrType::Named {
                path: "String".into(),
                args: vec![],
            }),
            // BUG-267：str 变换方法未登记 → `s.trim()`/`s.upper()` 等推断为 Any，
            // `result + s.trim()` 走 LzAdd 分派（需 String + String）报 E0308。
            // 与 owned_local::is_str_method 保持一致清单。
            // 仅保留 Rust std 真实存在的方法（trim/trim_start/trim_end/to_upper/to_lower/
            // replace/repeat）与代码生成器映射过的方法（upper→to_uppercase,
            // lower→to_lowercase）；reverse/capitalize/pad_*/strip 在 String 上无对应
            // Rust 方法，保留会让编译失败后无提示地生成非法 Rust 代码。
            "trim" | "trim_start" | "trim_end" | "to_upper" | "to_lower"
            | "replace" | "repeat" | "upper" | "lower" => Some(IrType::Str),
            // chars() 返回 char 迭代器（`for c in s.chars()` → c: char）
            "chars" => Some(IrType::Named {
                path: "Iterator".into(),
                args: vec![IrType::Str],
            }),
            // BUG-275：split/lines/split_whitespace 返回 &str 元素迭代器
            // （LZ 值语义 String）。此前未登记 → 循环变量 Any → `out + part`
            // 走 LzAdd 分派报 E0308。
            // BUG-308：LZ 语义下 split 返回 List<str>（codegen 收集为 Vec），
            // 标 Iterator<str> 会让 `.len()` 走 .count() → E0599 count not found on Vec。
            "split" | "lines" | "split_whitespace" | "split_inclusive"
            | "splitn" | "rsplit" | "rsplitn" | "split_terminator" => {
                Some(IrType::Named {
                    path: "List".into(),
                    args: vec![IrType::Str],
                })
            }
            "find" | "rfind" => Some(IrType::Int),
            // BUG-277：contains/starts_with/ends_with/is_empty 在 Str 上返回 Bool，
            // 但只在容器 arm 登记过 → `s.contains("x")` 推断为 Any → 代码生成
            // `ImplicitFrom<i64>` 包装报 E0277。
            "contains" | "starts_with" | "ends_with" | "is_empty" => Some(IrType::Bool),
            // slice(start, end) → Str（StringExt::lz_slice 返回 String）
            "slice" => Some(IrType::Str),
            // `",".join(parts)` 接收者 Str → 代码生成反转 `parts.join(",")` 返回 String
            "join" => Some(IrType::Str),
            _ => None,
        },
        // &Str 类型的方法
        IrType::Ref(inner) if matches!(inner.as_ref(), IrType::Str) => match method {
            "clone" => Some(IrType::Named {
                path: "String".into(),
                args: vec![],
            }),
            "to_string" => Some(IrType::Named {
                path: "String".into(),
                args: vec![],
            }),
            "trim" | "trim_start" | "trim_end" | "to_upper" | "to_lower"
            | "replace" | "repeat" | "upper" | "lower" => Some(IrType::Str),
            "chars" => Some(IrType::Named {
                path: "Iterator".into(),
                args: vec![IrType::Str],
            }),
            "split" | "lines" | "split_whitespace" | "split_inclusive"
            | "splitn" | "rsplit" | "rsplitn" | "split_terminator" => {
                Some(IrType::Named {
                    path: "List".into(),
                    args: vec![IrType::Str],
                })
            }
            "find" | "rfind" => Some(IrType::Int),
            "contains" | "starts_with" | "ends_with" | "is_empty" => Some(IrType::Bool),
            "slice" => Some(IrType::Str),
            "join" => Some(IrType::Str),
            _ => None,
        },
        // ── 字符串化：所有原语类型调用 .to_string() 应返回 String，
        //    使 print_arg_is_display 识别为字符串走 `{}`（Display）而非 `{:?}`（Debug）。
        //    BUG-44：此前 Int/F64/Bool 等臂缺失，导致 print(n.to_string()) 走 Debug 加引号。
        IrType::Int | IrType::Int128 | IrType::BigInt | IrType::F64 | IrType::Bool => {
            if method == "to_string" {
                return Some(IrType::Named { path: "String".into(), args: vec![] });
            }
            None
        }


        // Dict<K,V> / HashMap<K,V> 方法：keys→List<Ref<K>>, values→List<Ref<V>>, items→List<Ref<(K,V)>>
        IrType::Named { path, args }
            if (path == "Dict" || path == "HashMap") && args.len() == 2 =>
        {
            match method {
                // BUG-285：keys → List<K>（owned，dict.lz keys 定义）
                "keys" => Some(IrType::Named {
                    path: "List".into(),
                    args: vec![args[0].clone()],
                }),
                "values" => Some(IrType::Named {
                    path: "List".into(),
                    args: vec![args[1].clone()],
                }),
                "items" => Some(IrType::Named {
                    path: "List".into(),
                    args: vec![IrType::Tuple(vec![args[0].clone(), args[1].clone()])],
                }),
                "iter" => Some(IrType::Named {
                    path: "Iterator".into(),
                    args: vec![IrType::Tuple(vec![args[0].clone(), args[1].clone()])],
                }),
                // BUG-282：contains 成员测试 → Bool（否则回退 Any→i64，E0277）
                // BUG-313：Dict.contains_key → Bool（dict.lz contains_key 定义）
                "contains" | "contains_key" | "has_key" | "is_empty" => Some(IrType::Bool),
                "len" | "size" => Some(IrType::Int),
                // get → V（LZ 用 -1/"" 编码 None，代码生成 unwrap_or 平铺）
                "get" | "pop" => Some(args[1].clone()),
                // BUG-289：BTreeMap/HashMap::remove 返回 Option<V>，
                // LZ 视角 remove 返回 V?（Option<V>），先前回退 Any→Unit
                "remove" => Some(IrType::Option(Box::new(args[1].clone()))),
                _ => None,
            }
        }
        // Set<T> / HashSet<T> 方法
        IrType::Named { path, args } if (path == "Set" || path == "HashSet") && args.len() == 1 => {
            match method {
                // BUG-283：contains 成员测试 → Bool（否则回退 Any→i64，E0277）
                // BUG-313：is_subset/is_superset/is_disjoint 返回 bool，
                // 未登记则回退 Any→i64 → E0277 bool: ImplicitFrom<i64>
                "contains" | "is_empty" | "is_subset" | "is_superset" | "is_disjoint" => {
                    Some(IrType::Bool)
                }
                "len" | "size" => Some(IrType::Int),
                "iter" => Some(IrType::Named {
                    path: "Iterator".into(),
                    args: args.clone(),
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

/// 从 *: 构建块 body 中第一个 yield 表达式推断元素类型（无 callee 或 callee 类型不可知时）
pub(crate) fn infer_yield_elem_ty(body: &[AstStmt], _lhs_ty: IrType, ctx: &TypeCtx) -> IrType {
    body.iter()
        .find_map(|s| {
            if let AstStmt::Yield(Some(e)) = s {
                Some(infer_expr_type(e, ctx))
            } else {
                None
            }
        })
        .unwrap_or(IrType::Any)
}

/// 求 `recv.field` 中 field 的**声明类型**（无法确定时返回 None）。
///
/// 用于 SafeNav（`?.`）决定 `map` 还是 `and_then`：字段本身可空（`db: DbConfig?`）
/// 时 `map` 会得到 `Option<Option<T>>`，必须用 `and_then` 扁平化（E0609）。
///
/// 嵌套 SafeNav（`cfg?.db?.host`）需沿链回溯：内层 `cfg?.db` 在 IR 里是
/// `cfg.and_then(|__sn| __sn.db)`（表达式类型退化成 Any），所以要从 MethodCall
/// 的最内层接收者出发，逐层取字段类型。
pub(crate) fn safe_nav_field_ty(recv: &Expr, field: &str, ctx: &TypeCtx) -> Option<IrType> {
    // 嵌套 SafeNav 编译产物：MethodCall{ method: map/and_then, receiver,
    //   args: [Lambda{ FieldAccess(Var("__sn"), inner_f) }] }
    if let ExprKind::MethodCall {
        receiver,
        method,
        args,
    } = &recv.kind
    {
        if (method == "map" || method == "and_then") && args.len() == 1 {
            if let ExprKind::Lambda { body, .. } = &args[0].kind {
                if let ExprKind::FieldAccess {
                    base,
                    field: inner_f,
                } = &body.kind
                {
                    if matches!(&base.kind, ExprKind::Var(v) if v == "__sn") {
                        let inner = safe_nav_field_ty(receiver, inner_f, ctx)?;
                        if let IrType::Named { path, .. } = strip_option_ty(&inner) {
                            let t = ctx.lookup_field(&path, field);
                            if !matches!(t, IrType::Any) {
                                return Some(t);
                            }
                        }
                        return None;
                    }
                }
            }
        }
    }
    match strip_option_ty(&recv.ty) {
        IrType::Named { path, .. } => {
            let t = ctx.lookup_field(&path, field);
            if matches!(t, IrType::Any) {
                None
            } else {
                Some(t)
            }
        }
        _ => None,
    }
}
