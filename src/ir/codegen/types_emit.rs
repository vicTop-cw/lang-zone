// Lang-Zone 编译器 — ir/codegen/types_emit.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::*;

/// 检查类型是否为 BigInt（IrType::BigInt 或 Named "bigint"/"BigInt"）
pub(crate) fn is_bigint_ty(ty: &IrType) -> bool {
    match ty {
        IrType::BigInt => true,
        IrType::Named { path, .. } => path == "bigint" || path == "BigInt",
        _ => false,
    }
}

/// bigint / complex 在生成码里的 Rust 类型路径（BUG-1 根因站点）。
/// 用 `lz_builtins::` 而非 `num_bigint::` / `num_complex::`：num-* 只是
/// lz_builtins 的传递依赖，孤立 crate（闸门同款 `--extern lz_builtins=…`）里
/// 裸 crate 名路径解析不到 ⇒ E0432/E0433。lz_builtins 侧已 `pub use` 这两个
/// 类型，产物里的所有 bigint/complex 路径都经它联结，不留未联结的 crate 名。
pub(crate) const BIGINT_RS: &str = "lz_builtins::BigInt";

pub(crate) const COMPLEX_RS: &str = "lz_builtins::Complex64";

/// const 语境可用的零值（`BigInt::from(0)` 是非常量函数 ⇒ const/static 位 E0015）
pub(crate) const BIGINT_ZERO: &str = "lz_builtins::BigInt::ZERO";

pub(crate) const COMPLEX_ZERO: &str = "lz_builtins::Complex64::new(0.0, 0.0)";

/// 有界整数（i128 可表示）→ BigInt 构造式；i64 域内省掉后缀便于 const 折叠。
pub(crate) fn bigint_from_code(n: i128) -> String {
    if n >= i64::MIN as i128 && n <= i64::MAX as i128 {
        format!("{BIGINT_RS}::from({n})")
    } else {
        format!("{BIGINT_RS}::from({n}i128)")
    }
}

/// 任意精度整数字面量（十进制串，可能超出 i128）→ BigInt 构造式。
/// 不发 `BigInt::from(999…)`：该字面量超出所有 Rust 整数类型范围（E0584）；
/// 也不发 `BigInt::from("999…")`：num-bigint 0.4 无 `From<&str>`（E0277）。
/// ⇒ 走 `FromStr`（num-bigint 0.4 已实现 `FromStr for BigInt`）。
pub(crate) fn bigint_lit_code(s: &str) -> String {
    match s.parse::<i128>() {
        Ok(n) => bigint_from_code(n),
        Err(_) => format!("\"{s}\".parse::<{BIGINT_RS}>().unwrap()"),
    }
}

/// 检查类型是否为 Complex（IrType::Complex 或 Named "complex"/"Complex"/"Complex64"）
pub(crate) fn is_complex_ty(ty: &IrType) -> bool {
    match ty {
        IrType::Complex => true,
        IrType::Named { path, .. } => path == "complex" || path == "Complex" || path == "Complex64",
        _ => false,
    }
}

/// 检查类型是否为 Dict/HashMap（Named "Dict"/"HashMap"）
pub(crate) fn is_dict_ty(ty: &IrType) -> bool {
    matches!(ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap")
}

impl CodeGen {
    // ── 类型映射 ──

    pub(crate) fn rust_type_name(&self, name: &str) -> String {
        match name {
            "int" => "i64".into(),
            "float" | "f64" => "f64".into(),
            "str" => "String".into(),
            "bool" => "bool".into(),
            "List" => "Vec".into(),
            "Dict" => "BTreeMap".into(),
            "Set" => "HashSet".into(),
            #[cfg(feature = "tnr-embed")]
            "Tensor" => "tnr::tensor::Tensor".into(),
            other => other.to_string(),
        }
    }

    pub(crate) fn is_collection_type(&self, ty: &IrType) -> bool {
        matches!(ty, IrType::Named { path, .. }
            if ["Vec","List","HashMap","HashSet","Dict","Set"].contains(&path.as_str()))
    }

    /// 检查名称是否为已知的类型名（内置枚举 + 用户定义的 enum/impl 类型）
    pub(crate) fn is_known_type_or_enum(&self, name: &str) -> bool {
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
            "__Params",
        ];
        self.emitted_types.contains(name)
            || self.impl_types.contains(name)
            || self.known_types.contains(name)
            || KNOWN_EXT_TYPES.contains(&name)
            || matches!(name, "Option" | "Result" | "Some" | "None" | "Ok" | "Err")
    }

    /// 查询 struct 的方法名集合（用于 r? 自定义传播类型判定 __is_ok__ 等）
    pub(crate) fn struct_method_names(&self, name: &str) -> std::collections::HashSet<String> {
        self.struct_method_names_map
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    /// 判断表达式是否为「对 str/String（含 &String）调用 .clone()」。
    /// 这类实参的 IR 类型常退化为 Any，但 clone 结果仍是 String，按字符串实参
    /// 处理（Pattern 方法需 &）。例：string.lz `self.starts_with(other.clone())`。
    pub(crate) fn expr_is_clone_of_string(e: &Expr) -> bool {
        let (receiver, method) = match &e.kind {
            ExprKind::MethodCall {
                receiver, method, ..
            } => (receiver, method),
            _ => return false,
        };
        if method != "clone" {
            return false;
        }
        fn is_str_named(t: &IrType) -> bool {
            matches!(t, IrType::Str)
                || matches!(t, IrType::Named { path, .. } if path == "str" || path == "String")
        }
        if is_str_named(&receiver.ty) {
            return true;
        }
        matches!(&receiver.ty, IrType::Ref(i) | IrType::MutRef(i) if is_str_named(i))
    }

    /// 生成 duck 方法签名中的类型：关联类型引用（I.Item，§2.3）→ `Self::Item`，
    /// 其余类型走 rust_type。仅用于 duck trait / impl 方法签名。
    pub(crate) fn duck_sig_type(&self, ty: &IrType, duck: &DuckDef) -> String {
        match ty {
            IrType::Named { path, args } => {
                // 自引用 duck 名（Comparable::__lt__(other: Comparable)）→ Self：
                // trait 方法签名引用 trait 自身类型会触发 dyn-compat 环检查
                // （E0391 cycle detected when checking if trait is dyn-compatible）
                if path == &duck.name {
                    if args.is_empty() {
                        return "Self".to_string();
                    }
                    let inner: Vec<String> =
                        args.iter().map(|a| self.duck_sig_type(a, duck)).collect();
                    return format!("Self<{}>", inner.join(", "));
                }
                // `I.Item`：path 含点号且前缀是 duck 泛型参数
                if let Some((owner, member)) = path.split_once('.') {
                    if duck.generics.iter().any(|g| g.name == owner) {
                        if args.is_empty() {
                            return format!("Self::{}", member);
                        }
                        let inner: Vec<String> =
                            args.iter().map(|a| self.duck_sig_type(a, duck)).collect();
                        return format!("Self::{}<{}>", member, inner.join(", "));
                    }
                }
                if args.is_empty() {
                    self.rust_type(ty)
                } else {
                    let inner: Vec<String> =
                        args.iter().map(|a| self.duck_sig_type(a, duck)).collect();
                    format!("{}<{}>", path, inner.join(", "))
                }
            }
            IrType::Option(inner) => format!("Option<{}>", self.duck_sig_type(inner, duck)),
            IrType::Tuple(items) => {
                let inner: Vec<String> =
                    items.iter().map(|i| self.duck_sig_type(i, duck)).collect();
                format!("({})", inner.join(", "))
            }
            IrType::Ref(inner) => format!("&{}", self.duck_sig_type(inner, duck)),
            IrType::MutRef(inner) => format!("&mut {}", self.duck_sig_type(inner, duck)),
            IrType::Result { ok, err } => format!(
                "Result<{}, {}>",
                self.duck_sig_type(ok, duck),
                self.duck_sig_type(err, duck)
            ),
            other => self.rust_type(other),
        }
    }

    /// LZ 标准错误类型名（与 lz_builtins::ErrorKind::name() 一一对应）。
    /// 作为 `raises` 错误类型出现时统一解析为 `LzError`（lz_builtins 已注入作用域），
    /// 用户自定义错误类型（如 json.lz 的 enum ParseError）不在此列，保留原名。
    pub(crate) const STD_ERROR_KIND_NAMES: &[&str] = &[
        "Error",
        "CastError",
        "IOError",
        "NotFoundError",
        "PermissionError",
        "AlreadyExistsError",
        "ValueError",
        "TypeError",
        "IndexError",
        "KeyError",
        "ParseError",
        "JSONDecodeError",
        "AssertionError",
        "NullError",
        "RecursionError",
        "ArithmeticError",
        "TimeoutError",
        "ConnectionError",
        "SerializationError",
        "DeserializationError",
        "ImportError",
        "SyntaxError",
        "CancelledError",
        "NotImplementedError",
        "InternalError",
    ];

    pub(crate) fn rust_type(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "i64".into(),
            IrType::Int128 => "i128".into(),
            IrType::BigInt => BIGINT_RS.into(),
            IrType::Complex => COMPLEX_RS.into(),
            IrType::F64 => "f64".into(),
            IrType::Str => "String".into(),
            IrType::Bool => "bool".into(),
            IrType::Unit => "()".into(),
            IrType::Never => "!".into(),
            IrType::Any => "i64".into(),
            IrType::Ext => "ExtHandle".into(),
            IrType::Self_ => "Self".into(),
            IrType::Duck { .. } => "()".into(), // Duck types: cannot determine Rust type, use unit
            IrType::Named { path, args } => {
                // BUG-CG-004（收口）：LZ 标准错误名 → 统一的 LzError 类型。
                // 用户自定义错误类型（emitted_types/impl_types 命中）保留原名，
                // 以免破坏 `raise ParseError.UnexpectedChar(...)` 等构造器调用。
                if Self::STD_ERROR_KIND_NAMES.contains(&path.as_str())
                    && !self.emitted_types.contains(path.as_str())
                    && !self.impl_types.contains(path.as_str())
                {
                    return "LzError".to_string();
                }
                // Future<T> → 保持 Future 类型用于函数签名
                // 对于变量声明，由 gen_let 等处理方决定是否省略类型标注
                if path == "Future" {
                    if let Some(inner) = args.first() {
                        let inner_ty = self.rust_type(inner);
                        return format!("std::future::Future<Output = {}>", inner_ty);
                    }
                    return "std::future::Future<Output = ()>".into();
                }
                let mapped = self
                    .type_map
                    .get(path.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        // 宏系统（08-宏与编译期.md）的 Tokens 类型：IR 后端不展开宏，
                        // 降级为 String（宏体 quote 拼接生成 Rust 字符串）
                        if path == "Tokens" {
                            "String".to_string()
                        } else if path == "Iter" {
                            // 生成器（iterator）返回类型：Iter<T> → Vec<T>
                            "Vec".to_string()
                        } else {
                            // 关联类型路径（06c-trait定义.md §五）：`I.Item` 在 Rust 中
                            // 需写为 `I::Item`（泛型参数上的关联类型用 `::` 而非 `.`）
                            path.replace('.', "::")
                        }
                    });
                // 用户自定义类型（struct/enum/impl-only）优先于内置 type_map 映射：
                // lz_std/iter.lz 自定义 `struct Range`，type_map 却映射到 std::ops::Range，
                // 导致 `impl Iterator for std::ops::Range<i64>` 与字段访问冲突（E0609）
                let mapped = if self.emitted_types.contains(path.as_str())
                    || self.impl_types.contains(path.as_str())
                {
                    path.clone()
                } else {
                    mapped
                };
                if args.is_empty() {
                    if self.emitted_types.contains(path.as_str())
                        || self.impl_types.contains(path.as_str())
                    {
                        // 用户自定义同名类型（如 lib_hashmap 的 struct HashMap）：
                        // 不加容器默认泛型（否则 E0107 struct takes 0 generic arguments）
                        mapped
                    } else if path == "List" || path == "Vec" {
                        format!("{}<i64>", mapped)
                    } else if path == "Dict" || path == "HashMap" {
                        format!("{}<i64, i64>", mapped)
                    } else if path == "Set" || path == "HashSet" {
                        format!("{}<i64>", mapped)
                    } else if path == "Option"
                        || path == "Result"
                        || path == "Rc"
                        || path == "Arc"
                        || path == "Box"
                    {
                        format!("{}<i64>", mapped)
                    } else if path == "Iterator" {
                        // Iterator<T> 是 trait：仅允许出现在函数参数位置（impl Trait），
                        // 变量声明处由 skip_ty 逻辑跳过标注，由调用方推断具体类型
                        // 注意：当前 Iterator trait 没有关联类型 Item，直接生成 impl Iterator
                        "impl Iterator".into()
                    } else if self.trait_names.contains(path.as_str()) {
                        // trait 对象引用：`ref Error` → &dyn Error（E0782 expected a
                        // type, found a trait；trait 名裸引用不合法，需 dyn 前缀）
                        format!("dyn {}", path)
                    } else {
                        mapped
                    }
                } else {
                    // Box<fn(...)> → Box<dyn FnOnce(...)>：闭包可装箱（03e §六 dyn 环境），
                    // fn 指针类型无法容纳 move 闭包（E0308）；
                    // 闭包 `|| -> str = msg` 移动捕获 → FnOnce（E0525 若标 Fn）
                    if (path == "Box" || path == "Rc" || path == "Arc") && args.len() == 1 {
                        if let IrType::Fn { params, ret } = &args[0] {
                            let ps: Vec<String> =
                                params.iter().map(|p| self.rust_type(p)).collect();
                            let trait_name = if path == "Box" { "FnOnce" } else { "Fn" };
                            return format!(
                                "{}<dyn {}({}) -> {}>",
                                mapped,
                                trait_name,
                                ps.join(", "),
                                self.rust_type(ret)
                            );
                        }
                    }
                    let args: Vec<String> = args.iter().map(|a| self.rust_type(a)).collect();
                    format!("{}<{}>", mapped, args.join(", "))
                }
            }
            IrType::Option(inner) => {
                // BUG-error-Never：Never 作为 Option 内部类型（Option<!>）不稳定，
                // 退化为 ()（纯 `-> !` 仍保留 `!`）
                let inner_s = if matches!(**inner, IrType::Never) {
                    "()".to_string()
                } else {
                    self.rust_type(inner)
                };
                format!("Option<{}>", inner_s)
            }
            IrType::Result { ok, err } => {
                // BUG-error-Never：Never 作为 Result 内部类型（Result<!, E>）不稳定，
                // 退化为 ()（纯 `-> !` 仍保留 `!`）
                let ok_s = if matches!(**ok, IrType::Never) {
                    "()".to_string()
                } else {
                    self.rust_type(ok)
                };
                format!("Result<{}, {}>", ok_s, self.rust_type(err))
            }
            IrType::Tuple(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.rust_type(e)).collect();
                format!("({})", elems.join(", "))
            }
            IrType::Fn { params, ret } => {
                let params: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                format!("fn({}) -> {}", params.join(", "), self.rust_type(ret))
            }
            IrType::Ref(inner) => {
                // 特殊处理 Str 引用：生成 &str 而非 &String
                if matches!(inner.as_ref(), IrType::Str) {
                    "&str".into()
                } else {
                    format!("&{}", self.rust_type(inner))
                }
            }
            IrType::MutRef(inner) => format!("&mut {}", self.rust_type(inner)),
            IrType::Generic(name) => name.clone(),
        }
    }

    /// LZ `fn` **值**（变量绑定 / 函数返回值）的 Rust 表示：
    /// `Arc<dyn Fn(P) -> R + Send + Sync>`。
    /// 嵌套 `fn -> fn -> T` 递归为
    /// `Arc<dyn Fn(P) -> Arc<dyn Fn(Q) -> R + Send + Sync> + Send + Sync>`。
    /// Arc 而非 Box：LZ 的 fn 值是值语义（可重复传入/调用），生成代码会对它们
    /// `.clone()`；`Box<dyn Fn>` 不实现 Clone → E0599，Arc/Rc 的 clone 是廉价共享。
    /// Arc 而非 Rc（BUG-9）：`go`/spawn 会把 fn 值捕获进 `std::thread::spawn`，
    /// 该处要求 `Send + 'static`；实测 `Box<dyn Fn>`（HEAD 旧载体）、`Rc<dyn Fn>`、
    /// 裸 `Arc<dyn Fn>` 三种载体均 E0277，只有带 `+ Send + Sync` 上界的 Arc 可编译。
    ///
    /// 注意区分三种位置（IR-003 / 设计文档）：
    /// - **值位置**（变量 / 返回值）：用此处 `Arc<dyn Fn … + Send + Sync>`（支持捕获闭包作一等值）。
    /// - **HOF 形参位置**：保留 `impl FnMut`（借用捕获，见 gen_param），不读此函数。
    /// - **struct 字段位置**：保留 `fn` 指针（Clone 安全，见 gen_struct_def 字段覆盖），
    ///   不读此函数。
    pub(crate) fn fn_value_type(&self, ty: &IrType) -> String {
        match ty {
            IrType::Fn { params, ret } => {
                let p: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                format!(
                    "Arc<dyn Fn({}) -> {} + Send + Sync>",
                    p.join(", "),
                    self.fn_value_type(ret)
                )
            }
            other => self.rust_type(other),
        }
    }

    // ── Item 生成 ──

    /// Option::None 的类型参数选择：优先调用期望类型（实参位置）→ 函数返回类型
    /// （均要求 Option<T>）→ 表达式自身类型 → 默认 i64。
    /// 修复：p27_opt_elif.lz（函数返回 Option<Token> 时 else 分支生成
    /// Option::<i64>::None 类型不匹配 E0308）、p22_option_pattern.lz
    /// （main 中 f(Option.None) 实参期望 Option<E>）。
    pub(crate) fn option_none_elem(&self, expr_ty: &IrType) -> String {
        let pick = |ty: &IrType| -> Option<String> {
            // IR 语义标记 Option(T)
            if let IrType::Option(inner) = ty {
                // inner 即 Some 的元素类型；本 None 的真实类型是 Option<inner>。
                // 发射器会包成 `Option::<pick>::None`，故 pick 必须是 inner 对应的 Rust 类型。
                // 若 inner 自身也是 Option（如 `Option<Option<X>>` 中 `None` 作 `Some`
                // 实参），再解一层避免双重 Option 包装（edge-values-boundary.lz 的
                // `Option.Some(Option.None)` 曾生成 Option::<Option<i64>>::None → E0308）。
                if let IrType::Option(x) = inner.as_ref() {
                    return Some(self.rust_type(x));
                }
                return Some(self.rust_type(inner));
            }
            // 命名类型 Option<T>
            if let IrType::Named { path, args } = ty {
                if path == "Option" && args.len() == 1 {
                    let a = &args[0];
                    if let IrType::Option(x) = a {
                        return Some(self.rust_type(x));
                    }
                    return Some(self.rust_type(a));
                }
            }
            None
        };
        if let Some(e) = self
            .current_expected_ty
            .borrow()
            .as_ref()
            .and_then(|t| pick(t))
        {
            return e;
        }
        if let Some(e) = self.current_fn_ret_ty.as_ref().and_then(|t| pick(t)) {
            return e;
        }
        pick(expr_ty).unwrap_or_else(|| "i64".to_string())
    }

    /// 空列表字面量 `[]` 的元素类型选择：优先调用点期望类型（实参位置）→
    /// 函数返回类型（均要求 List<T>/Vec<T>）→ 字面量自身类型 → 默认 i64。
    /// 修复：泛型函数内 `return []`（IR 字面量类型默认推断为 List<i64>，但函数
    /// 返回 List<a>）误生成 Vec::<i64>::new() 导致 E0308（如 dedup 的 `return []`）。
    /// 元素为泛型参数时：来自当前函数返回类型（泛型在当前作用域）→ 用具体名；
    /// 来自调用点期望类型（泛型属被调函数，调用点不在作用域）→ 用 `_` 让 Rust 推断。
    /// BUG-5：`current_expected_ty` 这一档由 ListLit 元素位下推提供，语义是
    /// **「本空列表字面量自身的类型」**（见 `list_lit_elem_ty`），所以直接取
    /// args[0] 即元素类型，任意深度都成立。`current_fn_ret_ty` 那一档仍是
    /// **外层**列表类型（`return [[], []]` 的返回类型描述整个字面量），
    /// 该来源没有下推通道，保留「剥一层」的补偿。
    pub(crate) fn empty_list_elem(&self, expr_ty: &IrType) -> String {
        let pick = |ty: &IrType, allow_generic: bool| -> Option<String> {
            if let IrType::Named { path, args } = ty {
                if (path == "List" || path == "Vec") && args.len() == 1 {
                    // 元素为默认歧义 Int 时不采用（需更精确的类型源覆盖）
                    if !matches!(&args[0], IrType::Int) {
                        // 泛型元素：allow_generic=false（调用点期望类型，泛型属被调函数）
                        // 作用域外不可见，忽略并回退到字面量类型，避免泄漏泛型名(E0425)
                        // 或 _ 推断失败(E0283)；allow_generic=true（当前函数返回类型）
                        // 泛型在当前作用域，可用具体名。
                        if matches!(&args[0], IrType::Generic(_)) && !allow_generic {
                            return None;
                        }
                        // 仅函数返回类型来源需要剥一层（理由见上方 doc 注释）：
                        // 否则 `Vec<Vec<X>>` 槽位发 Vec::<Vec<X>>::new() 双重包装
                        // （polish_30_edge.lz 的 grid_with_empty）。
                        // 期望类型来源已由 ListLit 元素位下推到本字面量自身类型，
                        // 再剥就会把第 3 层嵌套的空列表错借到外层更浅槽位（BUG-5）。
                        if allow_generic {
                            if let IrType::Named { path: ep, args: ea } = &args[0] {
                                if (ep == "List" || ep == "Vec") && ea.len() == 1 {
                                    return Some(self.rust_type(&ea[0]));
                                }
                            }
                        }
                        return Some(self.rust_type(&args[0]));
                    }
                }
            }
            None
        };
        if let Some(e) = self
            .current_expected_ty
            .borrow()
            .as_ref()
            .and_then(|t| pick(t, false))
        {
            return e;
        }
        if let Some(e) = self.current_fn_ret_ty.as_ref().and_then(|t| pick(t, true)) {
            return e;
        }
        // 回退：字面量自身类型（可能为默认 i64）
        if let IrType::Named { path, args } = expr_ty {
            if (path == "List" || path == "Vec") && args.len() == 1 {
                return self.rust_type(&args[0]);
            }
        }
        "i64".to_string()
    }

    /// BUG-5：ListLit 元素位的期望类型 = 本列表字面量的**元素类型**。
    /// 优先取 `current_expected_ty`（Let/const 注解、实参位、上层元素位下推来的
    /// 精确类型）的 args[0]，退到字面量自身 `expr.ty` 的 args[0]；两者都不是
    /// List/Vec\<_\> 时返回 None（元素沿用环境期望类型，行为同改前）。
    /// 改前 ListLit 从不下推元素期望类型，内层空列表 `[]` 只能借到外层槽位，
    /// `empty_list_elem` 用「剥一层」补丁兜住两层，第 3 层起即错位
    /// （`Vec<Vec<Vec<i64>>>` 的 `[]` 发成 `Vec::<i64>::new()`，E0308）。
    pub(crate) fn list_lit_elem_ty(&self, expr_ty: &IrType) -> Option<IrType> {
        fn arg0(ty: &IrType) -> Option<IrType> {
            match ty {
                IrType::Named { path, args }
                    if (path == "List" || path == "Vec") && args.len() == 1 =>
                {
                    Some(args[0].clone())
                }
                _ => None,
            }
        }
        let from_expected = self
            .current_expected_ty
            .borrow()
            .as_ref()
            .and_then(|t| arg0(t).filter(|e| !matches!(e, IrType::Any)));
        from_expected.or_else(|| arg0(expr_ty))
    }

    /// 由魔法方法推导其 trait impl 所需的 `where` 子句（与 __eq__→PartialEq 一致）。
    /// 例如 `def __str__(ref self) -> str where T: Display` 返回 ` where T: std::fmt::Display`。
    /// 单独成方法（而非内联闭包）以避免 `&self` 借用与 `self.indent += 1` 的 `&mut self` 冲突。
    /// 解出 `Result<T, E>` 的 E（TryFrom 的 Error 关联类型）；非 Result 返回 None
    pub(crate) fn try_from_err_ty(ret: &IrType) -> Option<IrType> {
        match ret {
            IrType::Result { err, .. } => Some((**err).clone()),
            IrType::Named { path, args } if path == "Result" && args.len() == 2 => {
                Some(args[1].clone())
            }
            _ => None,
        }
    }

    /// 解出 `Result<T, E>` 的 (T, E)（TryInto 的目标/错误类型对）；非 Result 返回 None
    pub(crate) fn try_into_tgt_err(ret: &IrType) -> Option<(IrType, IrType)> {
        match ret {
            IrType::Result { ok, err } => Some(((**ok).clone(), (**err).clone())),
            IrType::Named { path, args } if path == "Result" && args.len() == 2 => {
                Some((args[0].clone(), args[1].clone()))
            }
            _ => None,
        }
    }

    /// 将 LZ trait 约束名映射为 Rust trait 名
    /// Ordered → Ord（全序：Ord 蕴含 PartialOrd + Eq，泛型比较算法
    /// sort/merge 等需 T: Ord 时不致因仅 PartialOrd 而 E0277），
    /// Display → Display, Clone → Clone 等
    pub(crate) fn gen_trait_bound(&self, b: &IrType) -> String {
        if let IrType::Named { path, args } = b {
            let mapped = match path.as_str() {
                "Ordered" => "Ord",
                "PartialOrder" => "PartialOrd",
                "Equatable" => "PartialEq",
                "Comparable" => "PartialOrd",
                // LZ 的 Eq/Ord（traits.lz 声明）需映射到 std::cmp：
                // `==` 运算符需要 PartialEq（Eq: PartialEq 继承），HashMap<K>
                // 的 K bound 是 std::cmp::Eq（E0277 the trait Eq is not implemented）
                "Eq" => "std::cmp::Eq",
                "Ord" => "std::cmp::Ord",
                // LZ 迭代协议：`I: Iterator` 约束需 std::iter::Iterator（I::Item
                // 关联类型），traits.lz 自定义 trait Iterator 遮蔽会报 E0220
                "Iterator" => "std::iter::Iterator",
                "FromIterator" => "std::iter::FromIterator",
                "Sized" => "std::marker::Sized",
                // LZ 自定义 trait Clone（traits.lz）→ std::clone::Clone：
                // `Option<J>: std::clone::Clone` 需要 std Clone bound（E0599
                // method clone exists but trait bounds not satisfied）
                "Clone" => "std::clone::Clone",
                "Iterable" => "IntoIterator",
                "Hashable" | "Hash" => "std::hash::Hash",
                // 算术运算符 trait（iter.lz 的 `I.Item: Add<Output = I::Item>` where 约束）：
                // LZ 的 Add/Mul 等需映射到 std::ops 才能解析（E0405 cannot find trait）
                "Add" => "std::ops::Add",
                "Sub" => "std::ops::Sub",
                "Mul" => "std::ops::Mul",
                "Div" => "std::ops::Div",
                "Rem" => "std::ops::Rem",
                "Neg" => "std::ops::Neg",
                "Not" => "std::ops::Not",
                "BitAnd" => "std::ops::BitAnd",
                "BitOr" => "std::ops::BitOr",
                "BitXor" => "std::ops::BitXor",
                "Shl" => "std::ops::Shl",
                "Shr" => "std::ops::Shr",
                other => other,
            };
            if args.is_empty() {
                // `Self.Item`（where 约束的关联类型路径，sum 的 where Self.Item: Add）
                // → `Self::Item`（Rust 关联类型用 ::，否则语法错误 expected . found）
                if let Some((owner, member)) = mapped.split_once('.') {
                    if owner == "Self" {
                        return format!("Self::{}", member);
                    }
                }
                mapped.to_string()
            } else {
                format!("{}{}", mapped, self.gen_type_args(args))
            }
        } else {
            self.rust_type(b)
        }
    }

    /// 生成泛型实参 <A, B> 部分（已带 < >）
    pub(crate) fn gen_type_args(&self, args: &[IrType]) -> String {
        if args.is_empty() {
            return String::new();
        }
        let inner: Vec<String> = args
            .iter()
            .map(|a| {
                // `Self.Item`（方法泛型 bound，如 collect<C: FromIterator<Self.Item>>）
                // → <Self as std::iter::Iterator>::Item（完全限定，E0221 歧义）；
                // 关联类型绑定（`Item = Self.Item` / `Output = Self.Item`）保留
                // "Item = " 前缀（chain 的 Other: Iterator<Item = Self::Item>）
                if let IrType::Named { path, .. } = a {
                    if path.contains("Self.") {
                        if let Some(eq_pos) = path.find("= ") {
                            let prefix = &path[..eq_pos + 1];
                            let member = path.rsplit('.').next().unwrap_or("");
                            return format!("{}<Self as std::iter::Iterator>::{}", prefix, member);
                        }
                    }
                    if let Some((owner, member)) = path.split_once('.') {
                        if owner == "Self" {
                            return format!("<Self as std::iter::Iterator>::{}", member);
                        }
                    }
                }
                self.rust_type(a)
            })
            .collect();
        format!("<{}>", inner.join(", "))
    }

    pub(crate) fn gen_generics(&self, g: &[GenericParam]) -> String {
        if g.is_empty() {
            return String::new();
        }
        let params: Vec<String> = g
            .iter()
            .map(|p| {
                let mut s = p.name.clone();
                if !p.bounds.is_empty() {
                    let bounds: Vec<String> =
                        p.bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                    s.push_str(&format!(": {}", bounds.join(" + ")));
                }
                if let Some(ref def) = p.default {
                    s.push_str(&format!(" = {}", self.rust_type(def)));
                }
                s
            })
            .collect();
        format!("<{}>", params.join(", "))
    }

    /// 函数泛型参数：为未约束的泛型参数追加 Debug 约束
    /// （print/println 使用 {:?}，需保证泛型 T 可 Debug）
    /// cmp_eq / cmp_ord：经调用图传播得到的「参与比较运算」的泛型名，
    /// 注入 PartialEq+Eq（相等）或 PartialEq+Eq+Ord（有序）约束，
    /// 修复泛型比较算法 E0277/E0369（比较约束传播特性）。
    pub(crate) fn gen_fn_generics(
        &self,
        g: &[GenericParam],
        cmp_eq: &HashSet<String>,
        cmp_ord: &HashSet<String>,
    ) -> String {
        if g.is_empty() {
            return String::new();
        }
        let params: Vec<String> = g
            .iter()
            .map(|p| {
                let mut s = p.name.clone();
                let mut all_bounds: Vec<String> = Vec::new();
                for b in &p.bounds {
                    let tb = self.gen_trait_bound(b);
                    if !all_bounds.contains(&tb) {
                        all_bounds.push(tb);
                    }
                }
                // 未显式约束的泛型参数追加 Debug + Clone（LZ 值语义默认 clone，
                // 递归类型遍历（root.clone() 等）需要 T: Clone）
                if !all_bounds.iter().any(|b| b == "Debug") {
                    all_bounds.push("Debug".to_string());
                }
                if !all_bounds.iter().any(|b| b == "Clone") {
                    all_bounds.push("Clone".to_string());
                }
                // 比较约束传播：自身或调用链参与比较运算 → 注入 PartialEq/Eq/Ord
                let needs_ord = cmp_ord.contains(&p.name);
                let needs_eq = cmp_eq.contains(&p.name);
                // 显式部分序约束（PartialOrder/Comparable → PartialOrd）时跳过注入：
                // `>`/`==` 仅需 PartialOrd（其超 trait 即 PartialEq），而 f64 无
                // Ord/Eq（NaN 非全序），注入会 E0277（generics.lz
                // larger(3.14, 2.71) 回归）。无约束泛型保持 Ord 注入
                // （sort/merge 泛型算法依赖）。
                if !all_bounds.iter().any(|b| b == "PartialOrd") {
                    if needs_ord {
                        for b in ["PartialEq", "Eq", "Ord"] {
                            if !all_bounds.iter().any(|x| x == b) {
                                all_bounds.push(b.to_string());
                            }
                        }
                    } else if needs_eq {
                        for b in ["PartialEq", "Eq"] {
                            if !all_bounds.iter().any(|x| x == b) {
                                all_bounds.push(b.to_string());
                            }
                        }
                    }
                }
                // @math 函数体内整数字面量经 T::from(2i32) 转换（gen_lit 的
                // in_math_fn 分支），需 T: From<i32> 约束，否则 E0308
                // （@math 泛型函数，如 `x * 2` 中 2 推断为 T）
                if self.in_math_fn && !all_bounds.iter().any(|b| b.contains("From<i32>")) {
                    all_bounds.push("std::convert::From<i32>".to_string());
                }
                if !all_bounds.is_empty() {
                    s.push_str(&format!(": {}", all_bounds.join(" + ")));
                }
                // 函数泛型默认参数（`T = int`，03b §四）不渲染：Rust 函数泛型
                // 不允许默认类型参数（E0741），由调用点类型推断 / LZ 类型检查使用
                s
            })
            .collect();
        format!("<{}>", params.join(", "))
    }

    /// 生成布尔条件：用户 struct 类型用 __bool__() 方法
    /// if acc → if acc.__bool__()；if not acc → if !(acc.__bool__())
    /// 判断某类型名是否为用户自定义类型（struct/enum），支持泛型名剥离
    pub(crate) fn is_known_type(&self, name: &str) -> bool {
        // 剥离泛型参数：MyList<i64> → MyList
        let base = name.split('<').next().unwrap_or(name);
        self.known_types.contains(base) || self.emitted_types.contains(base)
    }

    /// 生成字段类型的默认值（用于 __new__ 补齐）
    pub(crate) fn default_value_for(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "0".into(),
            IrType::F64 => "0.0".into(),
            IrType::Bool => "false".into(),
            IrType::Str => "\"\".to_string()".into(),
            IrType::Named { path, .. } => match path.as_str() {
                "String" => "\"\".to_string()".into(),
                _ => format!("{}.new()", self.rust_type(ty)),
            },
            _ => "Default::default()".into(),
        }
    }
}

// ── @parallel：AST 级并行化变换 ────────────────────────────────────
// 把 @parallel 函数体内的 `xs.map(f)` 方法调用重写为
// `lz_builtins::__lz_par_map(xs, f)`（std::thread 分块并行）。

/// 判断 IR 类型是否为 List（@parallel 改写条件收窄用）。
pub(crate) fn is_list_type(ty: &IrType) -> bool {
    matches!(ty, IrType::Named { path, .. } if path == "List")
}
