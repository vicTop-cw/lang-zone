// Lang-Zone 编译器 — ir/codegen.rs
// LZIR → Rust 源代码生成器
//
// 职责：
// 1. 将 IrModule 转换为合法的 Rust 源代码字符串
// 2. 类型映射：IrType → Rust 类型（如 Option→Option, List→Vec）
// 3. 生成完整的、可编译的 .rs 文件

use super::node::*;
use super::types::IrType;
use super::IrModule;
use std::collections::HashMap;

/// IR → Rust 代码生成器
pub struct CodeGen {
    /// 缩进级别（空格数）
    indent: usize,
    /// 类型映射表：LZ 类型名 → Rust 类型名
    type_map: HashMap<&'static str, &'static str>,
    /// 当前函数返回类型
    current_ret_ty: Option<IrType>,
    /// 当前函数签名返回类型（与 current_ret_ty 不同：后者在 if/match 等块内
    /// 可能被推断覆盖为 None，此字段保存函数级返回类型用于 ref 判断回退）
    current_fn_ret_ty: Option<IrType>,
    /// 当前函数是否返回引用（`-> &Self` 等）：builder 对 ref 返回推断可能为 None，
    /// Stmt::Return 中 `return self` 判断是否 clone 时使用
    current_fn_ret_is_ref: bool,
    is_main: bool,
    declared: std::collections::HashSet<String>,
    /// ref 绑定变量名集合（ref r = x / let ref r = x）：后续赋值 r = v 需生成 `*r = v`
    ref_bindings: std::collections::HashSet<String>,
    emitted_types: std::collections::HashSet<String>,
    /// 仅 impl 块（非 struct/enum）的类型名，用于 FieldAccess 生成 :: 语法
    impl_types: std::collections::HashSet<String>,
    /// enum variant → enum name 映射（用于构造器调用路由）
    enum_variants: HashMap<String, String>,
    /// 抑制尾表达式隐式 return（用于 match arm / 块表达式内部）
    suppress_tail_return: bool,
    /// 块内存在无值 return（return;）时强制尾表达式生成 `expr;`（丢弃值），
    /// 使闭包/块返回类型为 ()（否则 return; 与尾值类型冲突 E0308）
    force_unit_tail: bool,
    /// 循环体（for/while）内所有表达式语句强制加分号：循环体不是值上下文，
    /// 尾表达式裸生成会导致 E0308（如 go print(...) → std::thread::spawn(...) 缺 `;`）
    force_stmt_semicolon: bool,
    /// 函数返回类型为嵌套 Fn（fn -> fn -> T）时：内层闭包返回需 Box::new 包装，
    /// 且返回类型用 Box<dyn Fn>（Rust 不允许 impl Fn -> impl Fn 嵌套，E0562）
    nested_fn_ret: bool,
    /// @math 函数标志：体内整数字面量经 T::from(i32) 转换（使 `x * 2` 中 2 推断为 T）。
    /// 普通泛型函数不应转换（否则 `total = 0` 生成 T::from(0i32) 与 i64 冲突，E0308）
    in_math_fn: bool,
    /// 当前是否在 Lambda 块体（BlockExpr child）内：嵌套 Fn 返回时，
    /// 仅块体尾闭包需 Box::new 包装，函数体本身的尾表达式（外层闭包）不包装
    in_lambda_block: bool,
    /// 当前是否在生成器（iterator/yield）函数内：体内 return 等价 raise（panic）
    in_generator: bool,
    /// 当前是否在泛型函数内（@math 等）：整数字面量不附加 i64 后缀，
    /// 否则 `x * 2i64`（T 泛型）报 E0308（expected T, found i64）
    in_generic_fn: bool,
    /// 当前是否在泛型 impl 块内：impl<T> 的方法自身无 generics 字段，
    /// 但体内 Option.None 等需按泛型上下文处理（magic_methods.lz __next__ E0308）
    in_impl_generic: bool,
    /// 循环 else 标志栈：Some(flag) = 当前循环带 else 子句（while/else, for/else，
    /// 规范 05-控制流.md §13.2/13.3），break 时置 false 跳过 else 体
    loop_else_stack: Vec<Option<String>>,
    /// 循环 else 标志唯一命名计数器
    loop_else_counter: usize,
    /// plain block（block NAME: → (|| { ... })() 闭包）嵌套深度：
    /// 闭包内顶层（非循环内）break 应生成 return 退出闭包而非裸 break（E0267）
    plain_block_depth: usize,
    /// 当前循环嵌套深度（for/while/while let/loop）：
    /// plain block 内循环中的 break 仍跳出循环，块顶层的 break 退出闭包
    loop_depth: usize,
    /// type-pack 切片模式绑定变量名集合（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的
    /// args 编译为 &[Ts]，`[a]` / `[a, ..]` 模式中 a 绑定 &Ts（引用），臂体内
    /// 引用 a 需生成 a.clone()（否则 E0308 expected Ts, found &Ts）
    slice_clone_bindings: std::collections::HashSet<String>,
    /// 当前是否在 `impl Iterator for X` 内（LZ 迭代协议，规范 06d-内置魔法trait和
    /// 全局函数.md §五）：LZ 用 `__next__`/`__size_hint__` 魔术方法实现迭代协议，
    /// 生成 std::iter::Iterator impl 时需映射为 `next`/`size_hint`（否则 E0407）
    in_iterator_impl: bool,
    /// 函数名 → (总参数数, 默认参数数)（用于调用时自动填充 None）
    fn_param_info: HashMap<String, (usize, usize)>,
    /// 函数 ref/mut ref 参数标记：函数名 → 每参数 (is_ref, is_mut)（调用点自动传引用）
    fn_ref_params: HashMap<String, Vec<(bool, bool)>>,
    /// 被修改的模块级 const 名称（需生成 static mut 而非 const）
    mutated_consts: std::collections::HashSet<String>,
    /// enum variant → field types 映射: (enum_name, variant_name) → Vec<IrType>
    enum_variant_fields: HashMap<(String, String), Vec<IrType>>,
    /// 函数名 → variadic 参数起始索引（该索引及之后的参数收集为 &[T]）
    fn_variadic: HashMap<String, usize>,
    /// 函数名 → kwargs 注入参数起始索引（kwargs 收集为 &HashMap<String, V>）
    fn_kwargs: HashMap<String, usize>,
    /// 函数名 → 参数类型列表（用于隐式 variadic + 调用方类型检查）
    fn_param_types: HashMap<String, Vec<IrType>>,
    /// checker 块名称集合（fn NAME(ps: &mut __Params)），用于 default_checker 链
    /// 调用时区分 checker 块与普通值函数（fn NAME(ps: __Params) -> __Params）
    checker_blocks: std::collections::HashSet<String>,
    /// checker 块捕获的外层局部变量：checker 名 → [(变量名, 类型)]
    /// （block 闭包语义，规范 05b-block命名块.md §三）：生成
    /// fn NAME(ps: &mut __Params, out: &mut Vec<i64>, ...)，调用点传 &mut out
    checker_captures: std::collections::HashMap<String, Vec<(String, IrType)>>,
    /// 当前正在生成的 checker fn 捕获参数名集合（递归调用时捕获变量已是
    /// fn 参数 &mut 引用，直接传名而非 &mut 名）
    current_checker_captures: std::collections::HashSet<String>,
    /// match 臂 `ref mut` 模式绑定名集合（case Some(ref mut c)：臂体内
    /// c = c + 1 需生成 *c = *c + 1 解引用赋值，E0384 修复）
    ref_mut_bindings: std::collections::HashSet<String>,
    /// 重载函数签名集合：函数名 → 多个参数类型签名（同名函数 >1 个时启用 mangling）
    overload_sigs: HashMap<String, Vec<Vec<IrType>>>,
    /// 重载签名是否带 `..` 变参：函数名 → 每个签名是否 variadic（03d §2.7 兜底）
    overload_variadic: HashMap<String, Vec<bool>>,
    /// 重载签名的显式参数类型（排除注入的 args/kwargs）：函数名 → 每签名显式参数
    overload_explicit: HashMap<String, Vec<Vec<IrType>>>,
    /// 当前正在生成的函数的 variadic 参数名集合
    current_variadic_params: std::collections::HashSet<String>,
    /// 模块级 const/static 名称（用于参数重命名避免 E0530 冲突）
    top_level_static_names: std::collections::HashSet<String>,
    /// 当前函数的参数重命名映射（原名 → 新名），用于 E0530 冲突解决
    param_renames: HashMap<String, String>,
    /// 所有用户自定义类型名（struct/enum），预收集用于判断表达式是否为自定义类型
    known_types: std::collections::HashSet<String>,
    /// 所有用户自定义 trait 名（TraitDef），用于 trait 对象引用生成 `dyn Trait`
    /// （error.lz `Option<ref Error>` → Option<&dyn Error>，E0782 expected a type, found a trait）
    trait_names: std::collections::HashSet<String>,
    /// 模块自定义 `trait Iterator` 是否为 LZ 迭代协议（含 __next__/next 魔术方法）：
    /// 是 → impl 映射 std::iter::Iterator（iter.lz/traits.lz）；否（trait_assoc.lz 的
    /// get/peek 自定义 trait）→ 使用本地 trait 名（E0407 method get is not a member）
    custom_iterator_is_protocol: bool,
    /// 当前方法是否以共享引用（&self）接收，用于对 self.字段 值表达式自动 .clone()
    borrow_self: bool,
    /// 模块级全局可变变量：name → type（跨函数共享，生成 static mut + unsafe 访问）
    global_vars: std::collections::HashMap<String, IrType>,
    /// 关键字降级变量（Ok/Some/None/Err 用作变量名时重命名为 name_ 避免 E0530）
    downgraded_vars: std::collections::HashSet<String>,
    /// struct 字段信息：struct_name → [(field_name, field_type)]，用于 __new__ 补齐默认字段
    /// struct 字段信息（供 __new__ 补齐默认字段）
    struct_fields_info: std::collections::HashMap<String, Vec<(String, IrType)>>,
    /// struct 名 → 自动追加的 PhantomData 泛型参数列表
    /// （box.lz `struct Box<T> { _ptr: int }` 中 T 未使用，E0392 修复；
    /// 构造点需自动补 `_lz_phantom_T: PhantomData` 字段，否则 E0063）
    struct_phantom_generics: std::collections::HashMap<String, Vec<String>>,
    /// struct 方法名集合：struct_name → 方法名集合（用于 r? 自定义传播类型判定 __is_ok__ 等）
    struct_method_names_map: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// struct 是否定义了 __new__：struct_name → 是否
    struct_has_new: std::collections::HashSet<String>,
    /// 使用 LazyLock 的顶层静态集合名（需解引用访问）
    lazy_static_names: std::collections::HashSet<String>,
    /// 已导入的用户模块名（import services → "services"）：模块命名空间访问
    /// （services.service_name）降级为直接引用（模块项已平铺生成到同一 Rust 文件）
    imported_modules: std::collections::HashSet<String>,
    /// 当前正在生成的函数是否为 async（用于 __go 的异步/同步分派）
    current_fn_is_async: bool,
    /// 当前是否在生成 `impl Iterator` 的 size_hint 方法体：
    /// std Iterator 要求返回 (usize, Option<usize>)，方法体内 `(0, Some(0))`
    /// 元组元素需转 usize（否则 E0308 expected usize, found i64）
    current_fn_is_size_hint: bool,
    /// duck 约束字段成员：泛型参数名 → 该泛型 duck 约束声明的字段名集合
    /// （用于泛型函数体内 `a.field` → trait accessor `a.__field_field()`）
    duck_field_members: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// duck 定义索引（name → DuckDef），供 gen_fn_def 填充 duck_field_members 使用
    duck_defs: std::collections::HashMap<String, DuckDef>,
    buf: String,
}

impl CodeGen {
    pub fn new() -> Self {
        let mut type_map = HashMap::new();
        type_map.insert("List", "Vec");
        type_map.insert("Dict", "HashMap");
        type_map.insert("Set", "HashSet");
        type_map.insert("String", "String");
        type_map.insert("Nil", "()");
        type_map.insert("Unit", "()");
        type_map.insert("Range", "std::ops::Range<i64>");
        type_map.insert("RangeInclusive", "std::ops::RangeInclusive<i64>");
        // 基础类型保持原样
        CodeGen {
            indent: 0,
            type_map,
            current_ret_ty: None,
            current_fn_ret_ty: None,
            is_main: false,
            declared: std::collections::HashSet::new(),
            ref_bindings: std::collections::HashSet::new(),
            emitted_types: std::collections::HashSet::new(),
            impl_types: std::collections::HashSet::new(),
            enum_variants: HashMap::new(),
            suppress_tail_return: false,
            force_unit_tail: false,
            force_stmt_semicolon: false,
            nested_fn_ret: false,
            in_lambda_block: false,
            in_math_fn: false,
            in_generator: false,
            in_generic_fn: false,
            in_impl_generic: false,
            loop_else_stack: Vec::new(),
            loop_else_counter: 0,
            plain_block_depth: 0,
            loop_depth: 0,
            slice_clone_bindings: std::collections::HashSet::new(),
            in_iterator_impl: false,
            fn_param_info: HashMap::new(),
            fn_ref_params: HashMap::new(),
            mutated_consts: std::collections::HashSet::new(),
            enum_variant_fields: HashMap::new(),
            current_variadic_params: std::collections::HashSet::new(),
            fn_variadic: HashMap::new(),
            fn_kwargs: HashMap::new(),
            fn_param_types: HashMap::new(),
            checker_blocks: std::collections::HashSet::new(),
            checker_captures: std::collections::HashMap::new(),
            current_checker_captures: std::collections::HashSet::new(),
            ref_mut_bindings: std::collections::HashSet::new(),
            overload_sigs: HashMap::new(),
            overload_variadic: HashMap::new(),
            overload_explicit: HashMap::new(),
            top_level_static_names: std::collections::HashSet::new(),
            param_renames: HashMap::new(),
            known_types: std::collections::HashSet::new(),
            trait_names: std::collections::HashSet::new(),
            custom_iterator_is_protocol: true,
            borrow_self: false,
            struct_fields_info: std::collections::HashMap::new(),
            struct_phantom_generics: std::collections::HashMap::new(),
            struct_method_names_map: std::collections::HashMap::new(),
            struct_has_new: std::collections::HashSet::new(),
            lazy_static_names: std::collections::HashSet::new(),
            imported_modules: std::collections::HashSet::new(),
            current_fn_is_async: false,
            current_fn_is_size_hint: false,
            current_fn_ret_is_ref: false,
            duck_field_members: std::collections::HashMap::new(),
            duck_defs: std::collections::HashMap::new(),
            global_vars: std::collections::HashMap::new(),
            downgraded_vars: std::collections::HashSet::new(),
            buf: String::new(),
        }
    }

    // ── 入口 ──

    /// 将整个 IrModule 生成为 Rust 源代码
    pub fn generate(&mut self, module: &IrModule) -> String {
        self.buf.clear();
        self.indent = 0;

        // 预扫描：收集 enum variant → enum name 映射 + 函数参数信息 + impl-only 类型名
        // 注意：不能插入 emitted_types（会阻断 gen_enum_def / gen_struct_def 的去重逻辑）
        self.enum_variants.clear();
        self.fn_param_info.clear();
        self.emitted_types.clear();
        self.impl_types.clear();
        self.mutated_consts.clear();
        self.enum_variant_fields.clear();
        self.overload_sigs.clear();

        // 预收集所有用户自定义类型名（struct/enum），供函数体/表达式中判断
        self.known_types.clear();
        for item in &module.items {
            match item {
                Item::StructDef(s) => {
                    self.known_types.insert(s.name.clone());
                }
                Item::EnumDef(e) => {
                    self.known_types.insert(e.name.clone());
                }
                _ => {}
            }
        }

        // 收集所有模块级 const 名称
        let const_names: std::collections::HashSet<String> = module
            .items
            .iter()
            .filter_map(|item| {
                if let Item::Const(c) = item {
                    Some(c.name.clone())
                } else {
                    None
                }
            })
            .collect();

        // 预登记 LazyLock 静态集合名（集合/Option<集合>/Tuple 类型 const）。
        // 必须在函数体生成前登记：否则 main 内 config.and_then(...) 时
        // lazy_static_names 尚无 config → 生成 config 而非 (*config).clone()（E0507）
        self.lazy_static_names.clear();
        for item in &module.items {
            if let Item::Const(c) = item {
                let ty_is_collection = match &c.ty {
                    IrType::Named { path, .. } => ["Vec", "List", "HashMap", "HashSet", "Dict", "Set"]
                        .contains(&path.as_str()),
                    IrType::Option(inner) => matches!(
                        inner.as_ref(),
                        IrType::Named { path, .. }
                            if ["Vec", "List", "HashMap", "HashSet", "Dict", "Set"]
                                .contains(&path.as_str())
                    ),
                    IrType::Tuple(_) => true,
                    _ => false,
                };
                if ty_is_collection {
                    self.lazy_static_names.insert(c.name.clone());
                }
            }
        }

        // 收集所有模块级顶层名称（const + 函数名）以避免 E0530 参数冲突
        self.top_level_static_names.clear();
        for item in &module.items {
            match item {
                Item::Const(c) => {
                    self.top_level_static_names.insert(c.name.clone());
                }
                Item::FnDef(f) => {
                    self.top_level_static_names.insert(f.name.clone());
                }
                Item::StructDef(s) => {
                    self.top_level_static_names.insert(s.name.clone());
                }
                Item::EnumDef(e) => {
                    self.top_level_static_names.insert(e.name.clone());
                }
                Item::TraitDef(t) => {
                    self.top_level_static_names.insert(t.name.clone());
                    self.trait_names.insert(t.name.clone());
                    if t.name == "Iterator" {
                        self.custom_iterator_is_protocol =
                            t.methods.iter().any(|m| m.name == "__next__" || m.name == "next");
                    }
                }
                Item::Impl(i) => {
                    // 外部类型扩展 trait（impl str → StrExt 等）：预登记到 trait_names，
                    // 供 StrExt 强制调用判断（string.lz `self.slice_from(pos).find(...)`
                    // 需 `<str as StrExt>::find`；prelude_demo 无 StrExt 则走普通方法）
                    if let IrType::Named { path, .. } = &i.for_type {
                        let ext = match path.as_str() {
                            "Dict" | "HashMap" => Some("DictExt".to_string()),
                            "Set" | "HashSet" => Some("SetExt".to_string()),
                            "List" | "Vec" => Some("ListExt".to_string()),
                            "str" | "String" => Some("StrExt".to_string()),
                            _ => None,
                        };
                        if let Some(ext_name) = ext {
                            self.trait_names.insert(ext_name);
                        }
                    }
                }
                _ => {}
            }
        }

        // 分析跨函数全局可变变量（如 count，在 next() 中使用但 main() 中声明）
        self.analyze_global_vars(module, &const_names);
        // 索引 duck 定义（用于泛型函数体内 duck 字段访问 → trait accessor 转换）
        self.duck_defs.clear();
        let mut duck_defs_idx: HashMap<&str, &DuckDef> = HashMap::new();
        for item in &module.items {
            if let Item::DuckDef(d) = item {
                duck_defs_idx.insert(d.name.as_str(), d);
                self.duck_defs.insert(d.name.clone(), d.clone());
            }
        }
        // 生成 static mut 全局变量声明
        if !self.global_vars.is_empty() {
            let gv: Vec<(String, String, String)> = self
                .global_vars
                .iter()
                .map(|(n, t)| (n.clone(), self.rust_type(t), self.const_default_value(t)))
                .collect();
            for (name, rust_ty, init) in &gv {
                self.emit_line(&format!("static mut {}: {} = {};", name, rust_ty, init));
            }
            self.buf.push('\n');
        }

        // 提前生成所有类型别名（必须在使用前声明）
        for item in &module.items {
            if let Item::TypeAlias(ta) = item {
                self.gen_type_alias_def(ta);
            }
        }
        self.buf.push('\n');

        for item in &module.items {
            if let Item::EnumDef(e) = item {
                for variant in &e.variants {
                    self.enum_variants
                        .insert(variant.name.clone(), e.name.clone());
                    // 收集变体字段类型（用于构造时 Box::new() 包装判断）
                    let field_types: Vec<IrType> =
                        variant.fields.iter().map(|f| f.ty.clone()).collect();
                    self.enum_variant_fields
                        .insert((e.name.clone(), variant.name.clone()), field_types);
                }
            }
            if let Item::StructDef(s) = item {
                let mset: std::collections::HashSet<String> =
                    s.methods.iter().map(|m| m.name.clone()).collect();
                self.struct_method_names_map.insert(s.name.clone(), mset);
            }
            // impl 块方法也并入 struct 方法名集合（如 HttpResult 的 __is_ok__/__unwrap__
            // 定义在 `impl HttpResult<T>` 中，r? 自定义传播类型判定需要）
            if let Item::Impl(i) = item {
                if let IrType::Named { path, .. } = &i.for_type {
                    let entry = self
                        .struct_method_names_map
                        .entry(path.clone())
                        .or_default();
                    for m in &i.methods {
                        entry.insert(m.name.clone());
                        // 收集 impl 方法的 ref/mut ref 参数标记（DictExt::get 的
                        // key: ref K 调用点自动 &，否则 d.get("a") 报 E0308
                        // expected &K, found String）
                        if m.params.iter().any(|p| p.is_ref) {
                            self.fn_ref_params.insert(
                                m.name.clone(),
                                m.params.iter().map(|p| (p.is_ref, p.is_mut)).collect(),
                            );
                        }
                    }
                }
            }
            if let Item::FnDef(f) = item {
                let default_count = f.params.iter().filter(|p| p.default.is_some()).count();
                if default_count > 0 {
                    self.fn_param_info
                        .insert(f.name.clone(), (f.params.len(), default_count));
                }
                // 收集 ref/mut ref 参数标记（函数名 → 每参数 (is_ref, is_mut)）
                if f.params.iter().any(|p| p.is_ref) {
                    self.fn_ref_params.insert(
                        f.name.clone(),
                        f.params.iter().map(|p| (p.is_ref, p.is_mut)).collect(),
                    );
                }
                // 收集所有参数类型（用于隐式 variadic 检测）
                self.fn_param_types.insert(
                    f.name.clone(),
                    f.params.iter().map(|p| p.ty.clone()).collect(),
                );
                // 收集重载签名：同名非方法函数出现多次 → 记录各签名（用于函数重载 mangling）
                if !f.params.iter().any(|p| p.name == "self") {
                    let sig: Vec<IrType> = f.params.iter().map(|p| p.ty.clone()).collect();
                    self.overload_sigs
                        .entry(f.name.clone())
                        .or_insert_with(Vec::new)
                        .push(sig);
                    // 03d §2.7：`..` 变长签名作为兜底候选，记录 variadic 标记与显式参数
                    let is_var = f.params.iter().any(|p| p.variadic);
                    self.overload_variadic
                        .entry(f.name.clone())
                        .or_insert_with(Vec::new)
                        .push(is_var);
                    let explicit: Vec<IrType> = f
                        .params
                        .iter()
                        .filter(|p| !p.variadic)
                        .map(|p| p.ty.clone())
                        .collect();
                    self.overload_explicit
                        .entry(f.name.clone())
                        .or_insert_with(Vec::new)
                        .push(explicit);
                }
                // 收集 variadic 参数信息（函数名 → variadic 参数起始索引）
                // 注意：kwargs 注入参数单独记录在 fn_kwargs，不参与位置变参打包
                if let Some((idx, _)) = f
                    .params
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.variadic && p.name != "kwargs")
                {
                    self.fn_variadic.insert(f.name.clone(), idx);
                }
                // 收集 kwargs 注入参数（函数名 → kwargs 参数起始索引）
                if let Some((idx, _)) = f
                    .params
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.variadic && p.name == "kwargs")
                {
                    self.fn_kwargs.insert(f.name.clone(), idx);
                }
                // 方法定义语法 fn Type.method() → Type 是 impl-only 类型名
                if let Some((ty_name, _)) = f.name.split_once('.') {
                    self.impl_types.insert(ty_name.to_string());
                }
                // 扫描函数体中的 const 修改
                if !const_names.is_empty() {
                    scan_const_mutations(&f.body, &const_names, &mut self.mutated_consts);
                }
                // 检测函数参数名与模块级 static 的冲突（E0530）
                // 冲突解决在 gen_fn_def 中通过 param_renames 处理
            }
            if let Item::CheckerBlock { name, .. } = item {
                self.fn_param_types.insert(
                    name.clone(),
                    vec![IrType::Named {
                        path: "__Params".into(),
                        args: vec![],
                    }],
                );
                // 登记 checker 块名称：default_checker 链调用时区分
                // checker 块（fn NAME(ps: &mut __Params) → NAME(ps);）
                // 与普通值函数（fn NAME(ps: __Params) -> __Params → *ps = NAME(ps.clone());）
                self.checker_blocks.insert(name.clone());
            }
            if let Item::CheckerBlock {
                name, captured, ..
            } = item
            {
                // 登记 checker 块捕获的外层局部变量（block 闭包语义，规范 05b-block命名块.md §三）
                if !captured.is_empty() {
                    self.checker_captures.insert(name.clone(), captured.clone());
                }
            }
        }

        // 补登记：为 mangled 重载名登记 variadic/kwargs/param_types，
        // 使调用点按 mangled 名（如 show__Any）能查到打包信息（03d §2.7）
        for item in &module.items {
            if let Item::FnDef(f) = item {
                if f.params.iter().any(|p| p.name == "self") {
                    continue;
                }
                let sig: Vec<IrType> = f.params.iter().map(|p| p.ty.clone()).collect();
                let mangled = self.mangled_fn_name(f.name.clone(), &sig);
                if mangled == f.name {
                    continue;
                }
                // variadic args 起始索引（排除 kwargs）
                if let Some((idx, _)) = f
                    .params
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.variadic && p.name != "kwargs")
                {
                    self.fn_variadic.insert(mangled.clone(), idx);
                }
                // kwargs 起始索引
                if let Some((idx, _)) = f
                    .params
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.variadic && p.name == "kwargs")
                {
                    self.fn_kwargs.insert(mangled.clone(), idx);
                }
                // 参数类型（含注入参数，供隐式 variadic 检测）
                self.fn_param_types.insert(mangled.clone(), sig);
                // 默认参数信息
                let default_count = f.params.iter().filter(|p| p.default.is_some()).count();
                if default_count > 0 {
                    self.fn_param_info
                        .insert(mangled, (f.params.len(), default_count));
                }
            }
        }

        // 标准 prelude
        self.emit_prelude();

        // 每个顶层 item — 先发射 checker 块供后续函数引用
        let mut has_main = false;
        // 追踪已生成的 use 语句（去重 prelude imports）
        let mut emitted_uses: std::collections::HashSet<String> = std::collections::HashSet::new();
        // prelude 已自动导入的模块/类型
        let prelude_imports: std::collections::HashSet<&str> =
            ["std::collections::HashMap", "std::collections::HashSet"]
                .iter()
                .cloned()
                .collect();

        // 第一遍：仅发射 CheckerBlock（必须先于引用它的函数）
        for item in &module.items {
            if matches!(item, Item::CheckerBlock { .. }) {
                self.gen_item(item);
                self.buf.push('\n');
            }
            // 收集用户导入模块名（import services → "services"）供命名空间降级
            if let Item::Use(u) = item {
                if let Some(first) = u.path.first() {
                    self.imported_modules.insert(first.clone());
                }
                // import moda as m → 别名 m 同样登记为模块命名空间前缀
                if let Some(alias) = &u.alias {
                    self.imported_modules.insert(alias.clone());
                }
            }
        }

        // 第二遍：发射所有其他 item
        for item in &module.items {
            if matches!(item, Item::CheckerBlock { .. }) {
                continue;
            }
            if let Item::FnDef(f) = item {
                if f.name == "main" {
                    has_main = true;
                }
            }
            // 跳过已在 prelude 中导入的重复 use 语句
            if let Item::Use(u) = item {
                let key = u.path.join("::");
                if prelude_imports.contains(key.as_str()) {
                    continue;
                }
                if u.is_from && u.items.len() == 1 {
                    let full = format!("{}::{}", key, u.items[0]);
                    if prelude_imports.contains(full.as_str()) {
                        continue;
                    }
                }
                if emitted_uses.contains(&key) && u.items.is_empty() {
                    continue; // 完全重复的 use path;
                }
                emitted_uses.insert(key);
            }
            self.gen_item(item);
            self.buf.push('\n');
        }

        // 如果没有 main 函数，自动生成空 main（避免 E0601）
        if !has_main {
            self.buf.push_str(
                "pub fn main() {\n    // auto-generated: LZ module has no main entry point\n}\n",
            );
        }

        // duck 结构匹配自动 impl：结构满足 duck 的具体类型 → impl Duck for Type
        self.gen_duck_auto_impls(module);

        std::mem::take(&mut self.buf)
    }

    /// 分析跨函数全局可变变量：在函数 A 中引用但未在 A 声明的变量，
    /// 若在另一个函数中作为局部变量声明，则视为模块级全局变量
    fn analyze_global_vars(
        &mut self,
        module: &IrModule,
        const_names: &std::collections::HashSet<String>,
    ) {
        // 收集每个函数声明的局部变量名（参数 + 局部 let + 闭包参数）
        let mut fn_locals: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        let mut fn_refs: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for item in &module.items {
            if let Item::FnDef(f) = item {
                let mut locals: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for p in &f.params {
                    locals.insert(p.name.clone());
                }
                // 递归收集局部 let 绑定名（含闭包参数遮蔽）
                collect_local_lets(&f.body, &mut locals);
                fn_locals.insert(f.name.clone(), locals);

                let mut refs: Vec<String> = Vec::new();
                // 收集引用的自由变量（排除闭包参数遮蔽）
                collect_var_refs(&f.body, &mut std::collections::HashSet::new(), &mut refs);
                fn_refs.insert(f.name.clone(), refs);
            }
        }

        let known: std::collections::HashSet<String> = self.top_level_static_names.clone();
        // 枚举变体名（Less/Equal/Greater 等）不是变量——排除，否则被误判为
        // 跨函数全局变量生成 `static mut Less`，导致模式匹配重命名冲突（E0416）
        // 及枚举变体引用错误（E0423 expected value, found enum）
        let mut enum_variant_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for item in &module.items {
            if let Item::EnumDef(e) = item {
                for v in &e.variants {
                    enum_variant_names.insert(v.name.clone());
                }
            }
        }
        let mut candidates: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (fname, refs) in &fn_refs {
            let locals = fn_locals.get(fname).cloned().unwrap_or_default();
            for rname in refs {
                if locals.contains(rname.as_str()) {
                    continue;
                }
                if const_names.contains(rname.as_str()) {
                    continue;
                }
                if known.contains(rname.as_str()) {
                    continue;
                }
                if enum_variant_names.contains(rname.as_str()) {
                    continue;
                }
                if rname == "self" || rname == "self_" || rname == "pass" || rname == "_" {
                    continue;
                }
                if rname.starts_with('_') && rname != "_" {
                    continue;
                }
                // 排除枚举变体/字面量名（None/Some/Ok/Err/true/false）—— 非变量
                if matches!(
                    rname.as_str(),
                    "None" | "Some" | "Ok" | "Err" | "true" | "false" | "pass"
                ) {
                    continue;
                }
                candidates.insert(rname.clone());
            }
        }

        // 仅当变量在另一个函数中作为局部变量声明时，才视为全局
        for c in candidates {
            let declared_elsewhere = fn_locals.values().any(|l| l.contains(c.as_str()));
            if declared_elsewhere {
                let ty = self.infer_global_type(module, &c);
                self.global_vars.insert(c, ty);
            }
        }
    }

    /// 生成全局变量的 const 兼容默认值（不能调用 Default::default，非 const-stable）
    fn const_default_value(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "0".into(),
            IrType::F64 => "0.0".into(),
            IrType::Bool => "false".into(),
            IrType::Str => "String::new()".into(),
            IrType::Named { path, .. } => match path.as_str() {
                "String" => "String::new()".into(),
                "Vec" | "List" => "Vec::new()".into(),
                "HashMap" | "Dict" => "std::collections::HashMap::new()".into(),
                "HashSet" | "Set" => "std::collections::HashSet::new()".into(),
                _ => "0".into(),
            },
            _ => "0".into(),
        }
    }

    /// 从模块中推断全局变量类型
    fn infer_global_type(&self, module: &IrModule, name: &str) -> IrType {
        for item in &module.items {
            if let Item::FnDef(f) = item {
                let t = infer_global_type(&f.body, name, &f.params);
                if t != IrType::Any {
                    return t;
                }
            }
        }
        IrType::Int
    }

    // ── 辅助方法 ──

    fn pad(&self) -> String {
        "    ".repeat(self.indent)
    }

    #[allow(dead_code)]
    fn emit(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    fn emit_line(&mut self, s: &str) {
        self.buf.push_str(&self.pad());
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    /// 返回最后发射的一行（不含缩进和前导空白）
    fn last_emitted_line(&self) -> &str {
        let trimmed = self.buf.trim_end();
        trimmed.rsplit('\n').next().unwrap_or("").trim_start()
    }

    /// 在最后发射的一行末尾追加文本
    fn append_to_last_line(&mut self, s: &str) {
        let len = self.buf.trim_end().len();
        self.buf.insert_str(len, s);
    }

    /// 从表达式中收集 walrus 变量名（用于预声明）
    fn collect_walrus_vars(expr: &Expr, vars: &mut Vec<(String, IrType)>) {
        match &expr.kind {
            ExprKind::StructCtor { name, fields } if name == "_Walrus" => {
                if let Some((_, bind_expr)) = fields.iter().find(|(n, _)| n == "_bind") {
                    if let ExprKind::Var(v) = &bind_expr.kind {
                        // walrus 绑定变量类型来自值表达式（first := values.first()
                        // → Option<i64>，非 i64）；否则硬编码 i64 会 E0308
                        let val_ty = fields
                            .iter()
                            .find(|(n, _)| n == "_val")
                            .map(|(_, v)| v.ty.clone())
                            .unwrap_or(IrType::Int);
                        if !vars.iter().any(|(n, _)| n == v) {
                            vars.push((v.clone(), val_ty));
                        }
                    }
                }
            }
            ExprKind::BinOp { lhs, rhs, .. } => {
                Self::collect_walrus_vars(lhs, vars);
                Self::collect_walrus_vars(rhs, vars);
            }
            ExprKind::UnOp { operand, .. } => {
                Self::collect_walrus_vars(operand, vars);
            }
            ExprKind::Call { callee, args, .. } => {
                Self::collect_walrus_vars(callee, vars);
                for a in args {
                    Self::collect_walrus_vars(a, vars);
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                Self::collect_walrus_vars(receiver, vars);
                for a in args {
                    Self::collect_walrus_vars(a, vars);
                }
            }
            ExprKind::IfExpr { cond, then, els } => {
                Self::collect_walrus_vars(cond, vars);
                Self::collect_walrus_vars(then, vars);
                Self::collect_walrus_vars(els, vars);
            }
            ExprKind::Paren(inner) | ExprKind::ImplicitConvert { source: inner, .. } => {
                Self::collect_walrus_vars(inner, vars);
            }
            _ => {}
        }
    }

    /// 为 walrus 变量生成预声明: let mut n: i64;
    fn emit_walrus_predecls(&mut self, cond: &Expr) {
        let mut vars = Vec::new();
        Self::collect_walrus_vars(cond, &mut vars);
        for (v, ty) in &vars {
            // 用 walrus 绑定值的实际类型声明（first := values.first() → Option<i64>），
            // 否则硬编码 i64 与后续 Option 值比较时 E0308
            let ty_str = match ty {
                IrType::Int => "i64".to_string(),
                IrType::F64 => "f64".to_string(),
                IrType::Bool => "bool".to_string(),
                IrType::Str => "String".to_string(),
                IrType::Option(inner) => {
                    format!("Option<{}>", self.rust_type(inner))
                }
                IrType::Result { ok, err } => format!(
                    "Result<{}, {}>",
                    self.rust_type(ok),
                    self.rust_type(err)
                ),
                IrType::Any => "i64".to_string(),
                other => self.rust_type(other),
            };
            // walrus 变量带默认值初始化：`(n := compute()) > 5 if n * 10 else 0` 中
            // 三元条件 `n * 10` 在 walrus 赋值（then 分支内）之前求值，未初始化变量
            // 报 E0381（combo_ternary_walrus.lz）。默认值仅为让 Rust 编译通过，
            // 实际值在 walrus 表达式求值时被覆盖。
            let default = match ty {
                IrType::Int => "0i64".to_string(),
                IrType::F64 => "0.0".to_string(),
                IrType::Bool => "false".to_string(),
                IrType::Str => "String::new()".to_string(),
                IrType::Option(_) => "None".to_string(),
                IrType::Result { .. } => {
                    "Err(\"lz_walrus_default\".to_string())".to_string()
                }
                _ => String::new(),
            };
            if default.is_empty() {
                self.emit_line(&format!("let mut {}: {};", v, ty_str));
            } else {
                self.emit_line(&format!("let mut {}: {} = {};", v, ty_str, default));
            }
        }
    }

    fn emit_prelude(&mut self) {
        // Rust 2021 edition support (async/await, etc.)
        // 使用 outer attributes (#[..]) 而非 inner attributes (#![..])
        // 因为 type alias 可能已在 prelude 之前输出，inner attributes 不允许出现在 item 之后
        self.emit_line("#[allow(unused_imports)]");
        self.emit_line("#[allow(unused_variables)]");
        self.emit_line("#[allow(dead_code)]");
        self.emit_line("#[allow(non_snake_case)]");
        self.buf.push('\n');
        self.emit_line("use std::collections::{HashMap, HashSet};");
        // 多类型变参位置约束（03d §2.3 `..: Tuple<T1,T2,..>`）的尾部收集
        // args: (T1, T2, Vec<Box<dyn Any>>) 需要 std::any::Any
        self.emit_line("use std::any::Any;");
        // 若模块自定义了 Rc/Arc 类型（如 lz_std/box.lz 的 `struct Rc<T>`/`struct Arc<T>`，
        // LZ 自举标准库自行实现智能指针），跳过 std 同名导入，否则 E0255 重复定义
        if !self.known_types.contains("Rc") {
            self.emit_line("use std::rc::Rc;");
        }
        if !self.known_types.contains("Arc") {
            self.emit_line("use std::sync::Arc;");
        }
        // traits.lz 定义了自定义 trait Debug/Display（LZ 语义）时，不 import
        // std::fmt 的同名 trait，否则 E0255 the name is defined multiple times
        if !self.trait_names.contains("Debug") {
            self.emit_line("use std::fmt::Debug;");
        }
        if !self.trait_names.contains("Display") {
            self.emit_line("use std::fmt::Display;");
        }
        self.buf.push('\n');

        // ── Lang-Zone 运行时 builtins（内部子库导入，避免重复内联）──
        // __Params / __spawn_task / __block_on 及 print/len/range 等全部
        // 由 lz_builtins crate 提供；生成代码仅导入 API。
        self.emit_line("use lz_builtins::*;");
        self.buf.push('\n');
    }

    // ── 类型映射 ──

    fn rust_type_name(&self, name: &str) -> String {
        match name {
            "int" => "i64".into(),
            "float" | "f64" => "f64".into(),
            "str" => "String".into(),
            "bool" => "bool".into(),
            "List" => "Vec".into(),
            "Dict" => "HashMap".into(),
            "Set" => "HashSet".into(),
            other => other.to_string(),
        }
    }

    fn is_collection_type(&self, ty: &IrType) -> bool {
        matches!(ty, IrType::Named { path, .. }
            if ["Vec","List","HashMap","HashSet","Dict","Set"].contains(&path.as_str()))
    }

    /// 检查名称是否为已知的类型名（内置枚举 + 用户定义的 enum/impl 类型）
    fn is_known_type_or_enum(&self, name: &str) -> bool {
        self.emitted_types.contains(name)
            || self.impl_types.contains(name)
            || self.known_types.contains(name)
            || matches!(name, "Option" | "Result" | "Some" | "None" | "Ok" | "Err")
    }

    /// 查询 struct 的方法名集合（用于 r? 自定义传播类型判定 __is_ok__ 等）
    fn struct_method_names(&self, name: &str) -> std::collections::HashSet<String> {
        self.struct_method_names_map
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    /// 生成 duck 方法签名中的类型：关联类型引用（I.Item，§2.3）→ `Self::Item`，
    /// 其余类型走 rust_type。仅用于 duck trait / impl 方法签名。
    fn duck_sig_type(&self, ty: &IrType, duck: &DuckDef) -> String {
        match ty {
            IrType::Named { path, args } => {
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

    fn rust_type(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "i64".into(),
            IrType::F64 => "f64".into(),
            IrType::Str => "String".into(),
            IrType::Bool => "bool".into(),
            IrType::Unit => "()".into(),
            IrType::Never => "!".into(),
            IrType::Any => "i64".into(),
            IrType::Self_ => "Self".into(),
            IrType::Duck { .. } => "()".into(), // Duck types: cannot determine Rust type, use unit
            IrType::Named { path, args } => {
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
                    // 常见容器类型需要默认泛型参数，否则 Rust 无法推断
                    // 使用 i64（LZ 默认数值类型）作为默认元素类型，避免 Vec::new() 无法推断
                    if path == "List" || path == "Vec" {
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
                    } else if self.trait_names.contains(path.as_str()) {
                        // trait 对象引用：`ref Error` → &dyn Error（E0782 expected a
                        // type, found a trait；trait 名裸引用不合法，需 dyn 前缀）
                        format!("dyn {}", path)
                    } else {
                        mapped
                    }
                } else {
                    // Iterator<T> 是 trait：仅允许出现在函数参数位置（impl Trait），
                    // 变量声明处由 skip_ty 逻辑跳过标注，由调用方推断具体类型
                    if path == "Iterator" {
                        let inner: Vec<String> = args.iter().map(|a| self.rust_type(a)).collect();
                        return format!("impl Iterator<Item = {}>", inner.join(", "));
                    }
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
                format!("Option<{}>", self.rust_type(inner))
            }
            IrType::Result { ok, err } => {
                format!("Result<{}, {}>", self.rust_type(ok), self.rust_type(err))
            }
            IrType::Tuple(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.rust_type(e)).collect();
                format!("({})", elems.join(", "))
            }
            IrType::Fn { params, ret } => {
                let params: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                format!("fn({}) -> {}", params.join(", "), self.rust_type(ret))
            }
            IrType::Ref(inner) => format!("&{}", self.rust_type(inner)),
            IrType::MutRef(inner) => format!("&mut {}", self.rust_type(inner)),
            IrType::Generic(name) => name.clone(),
        }
    }

    // ── Item 生成 ──

    fn gen_item(&mut self, item: &Item) {
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
                    format!("fn {name}(ps: &mut __Params, {}) {{", captured_params.join(", "))
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
        }
    }

    /// 查询 checker 块捕获变量在调用点的实参列表（block 闭包语义，规范 05b-block命名块.md §三）。
    /// - 模块级/函数级调用：捕获变量是局部变量 → 传 `&mut out`
    /// - checker fn 体内递归调用：捕获变量已是 fn 的 &mut 参数 → 直接传 `out`
    fn checker_extra_args(&self, name: &str) -> Vec<String> {
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

    /// 计算重载函数的 mangled 名称。仅当函数名有多个重载签名时返回 mangled 名，
    /// 否则返回原名。用于函数定义处。
    fn mangled_fn_name(&self, name: String, sig: &[IrType]) -> String {
        // 清理测试名：空格 → 下划线，确保生成合法的 Rust 标识符
        let name = name.replace(' ', "_");
        if let Some(sigs) = self.overload_sigs.get(&name) {
            if sigs.len() > 1 {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return format!("{}__{}", name, suffix.join("_"));
            }
        }
        name
    }

    /// 根据实参 IR 类型匹配重载签名，返回选择的 mangled 函数名。
    /// 找不到匹配时返回 None（调用方保留原名）。
    /// 分派原则（03d §2.7）：先用不带 `..` 的固定签名匹配；无命中时
    /// 再按声明顺序对带 `..` 的变长签名做兜底匹配（只看显式参数）。
    fn match_overload(&self, name: &str, sigs: &[Vec<IrType>], args: &[Expr]) -> Option<String> {
        // 参数类型兼容：实参类型与签名参数类型匹配（含 Any 通配）
        let compatible = |arg_ty: &IrType, param_ty: &IrType| -> bool {
            if matches!(param_ty, IrType::Any) {
                return true;
            }
            if matches!(arg_ty, IrType::Any) {
                return true;
            }
            arg_ty == param_ty
        };
        let variadic_flags = self.overload_variadic.get(name);
        let explicit_sigs = self.overload_explicit.get(name);
        // 阶段 1：固定签名（无 `..`）精确匹配
        for (i, sig) in sigs.iter().enumerate() {
            let is_var = variadic_flags.map_or(false, |v| v.get(i).copied().unwrap_or(false));
            if is_var {
                continue;
            }
            if sig.len() == args.len()
                && args
                    .iter()
                    .zip(sig.iter())
                    .all(|(a, p)| compatible(&a.ty, p))
            {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return Some(format!("{}__{}", name, suffix.join("_")));
            }
        }
        // 阶段 2：变长签名（带 `..`）兜底：显式参数全部兼容且数量不超出即可
        for (i, sig) in sigs.iter().enumerate() {
            let is_var = variadic_flags.map_or(false, |v| v.get(i).copied().unwrap_or(false));
            if !is_var {
                continue;
            }
            let explicit = explicit_sigs
                .and_then(|e| e.get(i))
                .cloned()
                .unwrap_or_default();
            if args.len() < explicit.len() {
                continue;
            }
            if explicit
                .iter()
                .zip(args.iter())
                .all(|(p, a)| compatible(&a.ty, p))
            {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return Some(format!("{}__{}", name, suffix.join("_")));
            }
        }
        None
    }

    /// 将 IrType 编码为 mangled 后缀（简短稳定编码）
    fn type_mangle_suffix(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "i64".to_string(),
            IrType::F64 => "f64".to_string(),
            IrType::Bool => "bool".to_string(),
            IrType::Str => "String".to_string(),
            IrType::Named { path, args } => {
                if args.is_empty() {
                    path.replace("::", "_")
                } else {
                    let inner: Vec<String> =
                        args.iter().map(|a| self.type_mangle_suffix(a)).collect();
                    format!("{}_{}", path.replace("::", "_"), inner.join("_"))
                }
            }
            other => format!("{:?}", other).replace(['<', '>', ' ', '(', ')', ',', '{', '}'], "_"),
        }
    }

    fn gen_fn_def(&mut self, f: &FnDef) {
        self.declared.clear();
        // 记录当前函数是否为 async（用于 __go 的异步/同步分派）
        self.current_fn_is_async = f.is_async || (f.name == "main" && block_has_await(&f.body));
        // 记录当前是否在生成 impl Iterator 的 size_hint 方法体（返回元组需 usize）
        self.current_fn_is_size_hint = self.in_iterator_impl
            && (f.name == "size_hint" || f.name == "__size_hint__");
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
        let duck_params: Vec<String> = f
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| is_duck_ty(&p.ty))
            .map(|(i, _)| format!("DuckParam{}", i))
            .collect();
        let duck_indices: Vec<usize> = f
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| is_duck_ty(&p.ty))
            .map(|(i, _)| i)
            .collect();
        // duck 参数 → 泛型参数名 + trait bound（DuckParam0: Pet）
        let duck_bounds: Vec<String> = f
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| is_duck_ty(&p.ty))
            .filter_map(|(i, p)| {
                if let IrType::Named { path, .. } = &p.ty {
                    Some(format!("DuckParam{}: {}", i, path))
                } else {
                    None
                }
            })
            .collect();

        let has_ducks = !duck_params.is_empty();
        let is_math = f.intrinsics.iter().any(|intr| matches!(&intr.kind, IntrinsicKind::Export(targets) if targets.iter().any(|t| t == "Math")));

        let generics = if has_ducks {
            let base = self.gen_fn_generics(&f.generics);
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
            self.gen_fn_generics(&f.generics)
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
                    let idx = duck_indices.iter().position(|&d| d == i).unwrap();
                    format!("{}: {}", pname, duck_params[idx])
                } else if p.name == "self" {
                    // self 参数修饰：ref self → &self；mut self → &mut self；owned self → self
                    // 消耗型魔术方法（__enter__/__iter__）以 owned self 接收以便 move 字段
                    // 算术运算符保留 &self 以避免调用方多次复用实例时发生 move
                    let consumes_self = f.name == "__enter__" || f.name == "__iter__";
                    // impl Iterator 中 __next__/next 必须为 &mut self（std Iterator trait
                    // 要求，否则 E0053 types differ in mutability）
                    if self.in_iterator_impl
                        && (f.name == "next" || f.name == "__next__")
                    {
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
                            // 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` →
                            // args: (T1, T2, Vec<Box<dyn Any>>)（前 N 位置精确类型，
                            // 尾部 `..` 通配收集为 Box<dyn Any>）
                            let prefix: Vec<String> =
                                items.iter().map(|t| self.rust_type(t)).collect();
                            format!("({}, Vec<Box<dyn Any>>)", prefix.join(", "))
                        } else {
                            format!("&[{}]", self.rust_type(&p.ty))
                        }
                    } else if p.default.is_some() {
                        format!("Option<{}>", self.rust_type(&p.ty))
                    } else if p.is_ref {
                        // ref x: T → &T；mut ref x: T → &mut T
                        if p.is_mut {
                            format!("&mut {}", self.rust_type(&p.ty))
                        } else {
                            format!("&{}", self.rust_type(&p.ty))
                        }
                    } else {
                        // fn(...) 类型参数 → impl FnMut(...)：可接受闭包（03e §五），
                        // 直接生成 fn 指针无法接收 move 闭包（E0308）；
                        // 用 FnMut 而非 Fn：闭包体可修改外部捕获变量
                        // （iter.lz for_each `|x| total = total + x`，E0594 cannot assign）
                        if let IrType::Fn { params, ret } = &p.ty {
                            let ps: Vec<String> =
                                params.iter().map(|pt| self.rust_type(pt)).collect();
                            format!("impl FnMut({}) -> {}", ps.join(", "), self.rust_type(ret))
                        } else {
                            self.rust_type(&p.ty).to_string()
                        }
                    };
                    // __Params 值参数（checker 链值函数，如 def double_ps(ps: __Params)）
                    // 体内会写 ps.args，必须生成 `mut ps: __Params`（否则 E0596）
                    let ty_is_params = matches!(&p.ty, IrType::Named { path, .. } if path == "__Params");
                    // Iterator 参数：.next() 需要 &mut self，必须生成 `mut it: impl Iterator<..>`
                    let ty_is_iterator = matches!(&p.ty, IrType::Named { path, .. } if path == "Iterator");
                    // fn 类型参数生成 impl FnMut：调用需可变借用，必须 `mut f`（E0596）
                    let ty_is_fn = matches!(&p.ty, IrType::Fn { .. });
                    if p.is_mut || ty_is_params || ty_is_iterator || ty_is_fn {
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
        self.in_generic_fn = !f.generics.is_empty() || self.in_impl_generic;
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
            if self.in_iterator_impl
                && (f.name == "size_hint" || f.name == "__size_hint__")
            {
                format!(" -> (usize, Option<usize>)")
            } else if self.in_iterator_impl
                && (f.name == "next" || f.name == "__next__")
            {
                // `impl Iterator` 的 next：必须返回 `std::option::Option<Item>`。
                // 自定义 `enum Option<T>`（lz_std/option.lz）与 std Option 同名，
                // 裸 `Option<T>` 会解析到自定义枚举（E0053 类型不兼容）
                let item = match &f.ret_ty {
                    IrType::Named { path, args } if path == "Option" || path == "std::option::Option" => {
                        args.first().cloned().unwrap_or(IrType::Any)
                    }
                    IrType::Option(inner) => (**inner).clone(),
                    other => other.clone(),
                };
                format!(" -> std::option::Option<{}>", self.rust_type(&item))
            } else {
                let ret_ty_str = match &f.ret_ty {
                    IrType::Fn { params, ret } => {
                        // 递归生成嵌套 Fn 返回类型：
                        // fn(int) -> fn(int) -> int → impl Fn(i64) -> Box<dyn Fn(i64) -> i64>
                        // （Rust 不允许 impl Fn -> impl Fn 嵌套，E0562；内层用 Box<dyn Fn>）
                        fn rust_fn_ret(ty: &IrType, cg: &CodeGen) -> String {
                            match ty {
                                IrType::Fn { params, ret } => {
                                    let p: Vec<String> =
                                        params.iter().map(|p| cg.rust_type(p)).collect();
                                    format!(
                                        "Box<dyn Fn({}) -> {}>",
                                        p.join(", "),
                                        rust_fn_ret(ret, cg)
                                    )
                                }
                                other => cg.rust_type(other),
                            }
                        }
                        let p: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                        // 函数含 fn 类型参数（如 compose 的 f/g）且返回 fn(...)：闭包体
                        // 会捕获这些参数（FnMut），返回 impl Fn 报 E0596 cannot borrow as
                        // mutable in Fn closure——需生成 impl FnMut（nesting-closure-lambda.lz）
                        let has_fn_param = f.params.iter().any(|p| matches!(&p.ty, IrType::Fn { .. }));
                        let fn_kw = if has_fn_param { "FnMut" } else { "Fn" };
                        format!(
                            "impl {}({}) -> {}",
                            fn_kw,
                            p.join(", "),
                            rust_fn_ret(ret, self)
                        )
                    }
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
        if matches!(f.name.as_str(), "inspect" | "iter" | "as_ref" | "or" | "filter") {
            eprintln!(
                "DBG fn sig: name={} ret_str={:?} is_ref={}",
                f.name, ret, self.current_fn_ret_is_ref
            );
        }
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
            } else {
                raw.clone()
            };
            self.mangled_fn_name(
                mapped,
                &f.params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
            )
        };
        let sig = format!(
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
        );

        self.emit_line(&format!("{} {{", sig));
        self.indent += 1;

        // 生成器：body 包含 Yield → prepend __gen_vec
        if has_yield {
            self.emit_line("let mut __gen_vec = Vec::new();");
        }

        // checker 注入：有 default_checker 时打包参数→调checker→拆包
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
            if self.checker_blocks.contains(checker_name) {
                self.emit_line(&format!("{}(&mut __ps);", checker_name));
            } else {
                self.emit_line(&format!("__ps = {}(__ps);", checker_name));
            }
            for (i, (pname, pty)) in user_params.iter().enumerate() {
                let line = format!("let {0}: {1} = (*__ps.args[{2}usize].downcast_ref::<{1}>().expect(\"checker arg cast failed\"));", pname, pty, i);
                self.emit_line(&line);
            }
        }

        // 默认参数 unwrap: greet(name: str = "World") → let name = name.unwrap_or_else(|| "World".to_string());
        for p in &f.params {
            if let Some(ref default_val) = p.default {
                let pname = self
                    .param_renames
                    .get(&p.name)
                    .cloned()
                    .unwrap_or_else(|| p.name.clone());
                let def_s = self.gen_expr(default_val);
                self.emit_line(&format!(
                    "let {} = {}.unwrap_or_else(|| {});",
                    p.name, pname, def_s
                ));
            }
        }

        // 函数体
        self.current_ret_ty = Some(f.ret_ty.clone());
        self.current_fn_ret_ty = Some(f.ret_ty.clone());
        // 嵌套 Fn 返回类型（fn -> fn -> T）：内层闭包返回值需 Box::new 包装
        let saved_nested_fn_ret = self.nested_fn_ret;
        self.nested_fn_ret = matches!(&f.ret_ty, IrType::Fn { ret, .. }
            if matches!(ret.as_ref(), IrType::Fn { .. }));
        // typed main（def main() -> int）走 __lz_main 内部函数，尾表达式需 return
        self.is_main = f.name == "main" && !is_typed_main;
        if is_async_main {
            // async main → 使用 block_on 包装：fn main() { __block_on(async { body }) }
            self.emit_line("let __async_main = async {");
            self.indent += 1;
            self.gen_block_inner(&f.body);
            self.indent -= 1;
            self.emit_line("};");
            self.emit_line("__block_on(__async_main);");
        } else {
            self.gen_block_inner(&f.body);
        }
        self.nested_fn_ret = saved_nested_fn_ret;
        self.current_ret_ty = None;
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
    }

    fn gen_struct_def(&mut self, s: &StructDef) {
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

        let generics = self.gen_generics(&s.generics);
        self.emit_line("#[derive(Debug, Clone)]");
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

        // 如果 struct 有 __new__，生成 __lz_new 构造器函数
        if s.has_new {
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
            // 生成 __lz_new 函数签名
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
                "pub fn __lz_new({}) -> {} {{",
                params.join(", "),
                ret_ty
            ));
            // body: 通过关键字构造
            self.indent += 1;
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
            self.indent -= 1;
            self.emit_line("}");
            // 如果 struct 有 __init__，在同一个 impl 块中生成 __lz_init 方法
            if s.has_init {
                let init_params: Vec<String> = s
                    .init_params
                    .iter()
                    .map(|(n, t)| format!("{}: {}", n, self.rust_type(t)))
                    .collect();
                self.emit_line(&format!(
                    "pub fn __lz_init(&mut self, {}) {{",
                    init_params.join(", ")
                ));
                self.indent += 1;
                // body: 初始化字段赋值（简单实现：无操作占位）
                self.emit_line("// __init__ body (user-defined initialization)");
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
            for m in &s.methods {
                self.gen_fn_def(m);
                self.buf.push('\n');
            }
            self.in_impl_generic = saved_impl_generic;
            self.indent -= 1;
            self.emit_line("}");
        }
    }

    fn gen_enum_def(&mut self, e: &EnumDef) {
        // 去重：同名 enum 已生成则跳过
        if self.emitted_types.contains(&e.name) {
            return;
        }
        self.emitted_types.insert(e.name.clone());

        let generics = self.gen_generics(&e.generics);
        self.emit_line(&format!("#[derive(Debug, Clone, PartialEq)]"));
        self.emit_line(&format!("pub enum {}{} {{", e.name, generics));
        self.indent += 1;
        for variant in &e.variants {
            if variant.fields.is_empty() {
                self.emit_line(&format!("{},", variant.name));
            } else {
                let types: Vec<String> = variant
                    .fields
                    .iter()
                    .map(|f| {
                        let mut rust_ty = self.rust_type(&f.ty);
                        // 递归枚举字段自动 Box
                        if type_refers_to(&f.ty, &e.name) {
                            rust_ty = format!("Box<{}>", rust_ty);
                        }
                        rust_ty
                    })
                    .collect();
                self.emit_line(&format!("{}({}),", variant.name, types.join(", ")));
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
    }

    fn gen_trait_def(&mut self, t: &TraitDef) {
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
                    IrType::Named { path, args } if args.is_empty() => {
                        self.rust_type_name(path)
                    }
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
        // 关联类型声明（§五 `type Item`）→ Rust trait 关联类型
        for a in &t.assoc_types {
            self.emit_line(&format!("type {};", a));
        }
        for sig in &t.methods {
            // trait 默认方法（有 body）需真实参数名（body 引用），抽象声明用 _pN
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
            // trait 方法自身泛型（collect<C: FromIterator<Self.Item>>）需声明
            let m_gen = if sig.generics.is_empty() {
                String::new()
            } else {
                self.gen_fn_generics(&sig.generics)
            };
            // trait 方法 where 约束（try_from ... where Self: Sized / map ... where
            // Self: Iterator）：生成到方法签名（E0277 Self is not Sized / an iterator）
            let m_where = if sig.where_clause.is_empty() {
                // trait Iterator（自定义，LZ 迭代协议）的方法：完全限定
                // <Self as std::iter::Iterator>::Item 需 Self: std::iter::Iterator
                // 约束（E0277 Self is not an iterator）
                if t.name == "Iterator" && self.custom_iterator_is_protocol {
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
                                    bs.replace(
                                        "Self::",
                                        "<Self as std::iter::Iterator>::",
                                    )
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
                if is_iter_trait
                    && !sig
                        .where_clause
                        .iter()
                        .any(|(tp, _)| tp == "Self")
                {
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
                child.emitted_types = self.emitted_types.clone();
                child.enum_variants = self.enum_variants.clone();
                child.fn_param_info = self.fn_param_info.clone();
                child.in_generator = self.in_generator;
                child.suppress_tail_return = true;
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
    fn trait_sig_type(&self, ty: &IrType, current_trait: &str) -> String {
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
                                return format!(
                                    "<Self as std::iter::Iterator>::{}",
                                    member
                                );
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

    fn gen_impl_def(&mut self, i: &ImplDef) {
        // Rust impl 泛型不允许默认类型参数（E0741），剥离默认值仅保留 bounds；
        // 追加 Clone + Debug bound（LZ 值语义自动 .clone()，泛型需可 Clone）
        let stripped: Vec<GenericParam> = i
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
            // 扩展 trait 声明：方法签名（无 body）
            let trait_gen_names: Vec<String> = stripped.iter().map(|g| g.name.clone()).collect();
            let trait_gen_str = if trait_gen_names.is_empty() {
                String::new()
            } else {
                format!("<{}>", trait_gen_names.join(", "))
            };
            self.emit_line(&format!("trait {}{} {{", ext_name, trait_gen_str));
            self.indent += 1;
            for m in &i.methods {
                // 方法自身泛型参数（map<U>/map<K2,V2> 等）需在 trait 签名中声明，
                // 否则 E0425 cannot find type `U`
                let m_gen = self.gen_fn_generics(&m.generics);
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
                            let ps: Vec<String> =
                                fp.iter().map(|pt| self.rust_type(pt)).collect();
                            format!(
                                "{}: impl Fn({}) -> {}",
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
                    m.name, m_gen, params.join(", "), ret, m_where
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
                format!("{}{}", ext_name, trait_gen_str),
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
        for m in &i.methods {
            self.gen_fn_def(m);
            self.buf.push('\n');
        }
        self.in_impl_generic = saved_impl_generic;
        self.in_iterator_impl = saved_iterator_impl;
        self.indent -= 1;
        self.emit_line("}");
        // 自动生成 PartialEq：struct 定义 `__eq__` 魔术方法时（box.lz `Box/Rc/Arc`
        // 的 `def __eq__(ref self, ref other: Box<T>) where T: Eq`），
        // `assert_eq!(result, Ok(100))` 需 Result<T, Rc<T>>: PartialEq（E0369）——
        // 委托 __eq__ 生成 impl，并携带 __eq__ 的 where 约束（T: Eq）。
        // 枚举已有 #[derive(PartialEq)]（codegen 自动），跳过避免 E0119 冲突
        let enum_derives_partial_eq = matches!(&i.for_type, IrType::Named { path, .. }
            if self.enum_variants.values().any(|en| en == path));
        // 外部/内置类型（Vec/str/String/HashMap…）：Rust 孤儿规则禁止为外部类型
        // 实现外部 trait（E0117），且 std 已提供 PartialEq，跳过自动 impl
        let is_external_type = matches!(&i.for_type, IrType::Named { path, .. }
            if matches!(path.as_str(),
                "List" | "Vec" | "Dict" | "HashMap" | "Set" | "HashSet" | "String" | "str"));
        if i.trait_.is_none() && !enum_derives_partial_eq && !is_external_type {
            let eq_method = i.methods.iter().find(|m| m.name == "__eq__");
            if let Some(eq_m) = eq_method {
                let eq_where: String = eq_m
                    .where_clause
                    .iter()
                    .map(|(tp, bounds)| {
                        let bs: Vec<String> =
                            bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                        format!("{}: {}", tp, bs.join(" + "))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let where_str = if eq_where.is_empty() {
                    String::new()
                } else {
                    format!(" where {}", eq_where)
                };
                let for_ty = self.rust_type(&i.for_type);
                self.emit_line(&format!(
                    "impl{} std::cmp::PartialEq for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line("fn eq(&self, other: &Self) -> bool {");
                self.indent += 1;
                // __eq__ 第二参数为 ref（box.lz `ref other: Box<T>`）时直接传
                // other（&Self）；值为参数（polish_14 `other: Money`）时需
                // (*other).clone()（&Self 解引用克隆为值，E0308 类型不匹配）
                let eq_takes_ref = eq_m.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                if eq_takes_ref {
                    self.emit_line("self.__eq__(other)");
                } else {
                    self.emit_line("self.__eq__((*other).clone())");
                }
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }
    }

    fn gen_use_stmt(&mut self, u: &UseStmt) {
        // 映射 LZ 类型名 → Rust 类型名（仅在 import 路径中使用）
        // 以及相对路径前缀映射：. → self, .. → super
        let lz_to_rust: HashMap<&str, &str> = [
            ("List", "Vec"),
            ("Dict", "HashMap"),
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

        // 非相对路径：检查是否为已知模块或已知模块的子路径
        let is_known = known_module_paths.contains(path_str.as_str());
        let parent_path = path_str.rsplitn(2, "::").nth(1).unwrap_or("");
        let parent_is_known = known_module_paths.contains(parent_path);
        let is_std_root = path_str == "std";
        if !is_known && !parent_is_known && !is_std_root {
            // 未知模块路径，跳过（如 std::math, std::bridge.rust.serde_json）
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
            if !known_module_paths.contains(path_str.as_str()) && path_str != "std" {
                return;
            }
            self.emit_line(&format!("use {};", path_str));
        }
    }

    fn gen_const_def(&mut self, c: &ConstDef) {
        let is_mutated = self.mutated_consts.contains(&c.name);
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
            _ => (self.rust_type(&c.ty), self.gen_expr(&c.value)),
        };
        let kw = if is_mutated { "static mut" } else { "const" };
        // 需要使用 lhs!() 惰性初始化的情况：
        // 1. 集合类型（Vec, HashMap, HashSet）— 不能 const 初始化（需要 .to_string() 等）
        // 2. 包含 catch_unwind 等非 const 调用的值
        let needs_lazy = !is_mutated
            && (matches!(&c.ty,
                IrType::Named { path, .. }
                if ["Vec","List","HashMap","HashSet","Dict","Set"].contains(&path.as_str())
            ) || matches!(&c.ty, IrType::Tuple(_))
                || val_str.contains("catch_unwind")
                || val_str.contains("LazyLock")
                || val_str.contains(".to_string()"));
        if needs_lazy {
            self.lazy_static_names.insert(c.name.clone());
            let lazy_ty = self.rust_type(&c.ty);
            // 生成器构建块（*:）会在闭包体内 push __gen_vec，需先声明并在末尾返回
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

    fn gen_type_alias_def(&mut self, ta: &TypeAliasDef) {
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

    fn gen_test_def(&mut self, t: &TestDef) {
        self.emit_line("#[test]");
        // 测试名可能含空格（如 "string concat"），需转换为合法 Rust 标识符
        let safe_name = sanitize_ident(&t.name);
        self.emit_line(&format!("fn {}() {{", safe_name));
        self.indent += 1;
        self.gen_block_inner(&t.body);
        self.indent -= 1;
        self.emit_line("}");
    }

    /// duck 类型约束 → Rust trait
    fn gen_duck_def(&mut self, d: &DuckDef) {
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
    fn gen_duck_auto_impls(&mut self, module: &IrModule) {
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

    /// 将 LZ trait 约束名映射为 Rust trait 名
    /// Ordered → PartialOrd, Display → Display, Clone → Clone 等
    fn gen_trait_bound(&self, b: &IrType) -> String {
        if let IrType::Named { path, args } = b {
            let mapped = match path.as_str() {
                "Ordered" => "PartialOrd",
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
    fn gen_type_args(&self, args: &[IrType]) -> String {
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
                            return format!(
                                "{}<Self as std::iter::Iterator>::{}",
                                prefix, member
                            );
                        }
                    }
                    if let Some((owner, member)) = path.split_once('.') {
                        if owner == "Self" {
                            return format!(
                                "<Self as std::iter::Iterator>::{}",
                                member
                            );
                        }
                    }
                }
                self.rust_type(a)
            })
            .collect();
        format!("<{}>", inner.join(", "))
    }

    fn gen_generics(&self, g: &[GenericParam]) -> String {
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
    fn gen_fn_generics(&self, g: &[GenericParam]) -> String {
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
                // @math 函数体内整数字面量经 T::from(2i32) 转换（gen_lit 的
                // in_math_fn 分支），需 T: From<i32> 约束，否则 E0308
                // （@math 泛型函数，如 `x * 2` 中 2 推断为 T）
                if self.in_math_fn
                    && !all_bounds.iter().any(|b| b.contains("From<i32>"))
                {
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

    /// 类型中是否含未解析的关联类型路径（`Vec<I::Item>` 中 I 不在当前作用域，
    /// 如 main 里引用 collect_list 的泛型参数 I → E0433 cannot find type `I`）。
    /// 有此类路径时跳过变量类型标注，让 Rust 从右侧推断。
    /// 类型中是否含未解析的关联类型路径（`Vec<I::Item>` 中 I 不在当前作用域，
    /// 如 main 里引用 collect_list 的泛型参数 I → E0433 cannot find type `I`）。
    /// 有此类路径时跳过变量类型标注，让 Rust 从右侧推断。
    fn has_unbound_named(&self, ty: &IrType) -> bool {
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

    fn has_unresolved_dotted_assoc(&self, ty: &IrType) -> bool {
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

    fn gen_param(&self, p: &Param) -> String {
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
                if p.is_mut {
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

    fn gen_block_inner(&mut self, block: &Block) {
        let n = block.stmts.len();
        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == n - 1;
            self.gen_stmt(stmt, is_last);
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt, is_last: bool) {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                is_mut,
                is_ref,
            } => {
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
                if *is_mut && self.declared.contains(name) {
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
                // （Rust E0573），且不可变绑定不能有 mut 关键字（E0596）
                let mut_kw = if safe_name == "_" || !*is_mut { "" } else { "mut " };
                let skip_ty = *ty == IrType::Any
                    || *ty == IrType::Unit
                    || matches!(ty, IrType::Duck { .. })
                    || matches!(ty, IrType::Generic(_))
                    || matches!(ty, IrType::Fn { .. })
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
                let ty_str = if is_empty_container {
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
                // walrus 变量预声明（let 绑定中的 := 需要先声明变量再赋值）
                self.emit_walrus_predecls(value);
                // 元组解构（let (a,b,c) = tuple 或 __destruct_ 临时）→ 对源元组 clone 避免 move（LZ 元组可重复解构）
                let is_tuple_destr = (safe_name.starts_with('(') && safe_name.contains(','))
                    || safe_name.starts_with("__destruct_");
                let value_s = if is_tuple_destr && matches!(ty, IrType::Tuple(_)) {
                    format!("({}).clone()", self.gen_expr(value))
                } else if is_empty_container {
                    match ty {
                        IrType::Named { path, .. }
                            if path == "Dict"
                                || path == "Set"
                                || path == "HashMap"
                                || path == "HashSet" =>
                        {
                            "std::collections::HashMap::new()".to_string()
                        }
                        _ => "Vec::new()".to_string(),
                    }
                } else {
                    self.gen_expr(value)
                };
                self.emit_line(&format!(
                    "let {}{}{} = {};",
                    mut_kw, safe_name, ty_str, value_s
                ));
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
                        if let ExprKind::IndexGet { base: base2, key: key2 } = &base.kind {
                            let is_dict2 = matches!(&base2.ty, IrType::Named { path, .. }
                                if path == "Dict" || path == "HashMap");
                            if is_dict2 {
                                let base2_s = self.gen_expr(base2);
                                let key2_s = self.gen_expr(key2);
                                let inner =
                                    format!("({}).get_mut(&{}).unwrap()", base2_s, key2_s);
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
                if self.in_generator {
                    // iterator 体内 return 等价 raise：终止迭代并抛出
                    // （return expr 为错误信息；return 无值 → 空 panic）
                    if let Some(v) = value {
                        self.emit_line(&format!("panic!(\"{{:?}}\", {});", self.gen_expr(v)));
                    } else {
                        self.emit_line("panic!(\"generator return\");");
                    }
                } else if let Some(v) = value {
                    // `return self`：self 是 &self 引用。
                    // 返回类型是引用（`-> ref Self`，如 inspect）时直接 return self；
                    // 返回 owned 值时需 clone（`fn or(&self) -> Option<T>` 中
                    // `return self` → `return self.clone()`，E0308 expected Option<T>）
                    let ret_is_ref = matches!(&self.current_ret_ty, Some(IrType::Ref(_) | IrType::MutRef(_)))
                        || matches!(&self.current_ret_ty, Some(IrType::Named { path, .. }) if path == "Self")
                        // `-> &Self`（inspect 等方法）返回引用：current_ret_ty 可能为 None
                        // （builder 对 ref Self 推断失败），按函数签名判断
                        || self.current_fn_ret_is_ref;
                    if matches!(&v.kind, ExprKind::Var(n) if n == "self" || n == "self_") {
                        eprintln!(
                            "DBG retself: cur_ret={:?} fn_ref={} ret_is_ref={}",
                            self.current_ret_ty, self.current_fn_ret_is_ref, ret_is_ref
                        );
                    }
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
                            self.emit_line("return self.to_string();");
                        } else {
                            self.emit_line("return self.clone();");
                        }
                    } else {
                        // Iterator impl 的 next：自定义 `enum Option<T>`（lz_std/option.lz）
                        // 与 std Option 同名冲突——签名强制 std::option::Option<T>（E0053），
                        // body 返回的自定义 Option 需 match 转换（E0308 expected
                        // std::option::Option<T>, found Option<T>）
                        let ret_s = self.gen_expr(v);
                        // `return self`（&str）返回 String（__str__ 尾表达式 `= self`）：
                        // 需 to_string（&str: Clone 返回 &str，E0308 expected String）
                        let ret_is_string = matches!(&self.current_ret_ty,
                            Some(IrType::Named { path, .. }) if path == "String" || path == "str")
                            || matches!(&self.current_ret_ty, Some(IrType::Str));
                        if ret_is_string && (ret_s == "self" || ret_s == "(self)") {
                            self.emit_line("return self.to_string();");
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
                                self.emit_line("return self.to_string();");
                            } else {
                                self.emit_line(&format!("return {};", ret_s));
                            }
                        }
                    }
                } else {
                    self.emit_line("return;");
                }
            }
            Stmt::ExprStmt { expr } => {
                self.emit_walrus_predecls(expr);
                // 嵌套 Fn 返回（fn -> fn -> T）：内层闭包作为外层返回值需 Box::new 包装
                // （factory_chain: |a| => |b| => x + a + b → move |a| { Box::new(move |b| {...}) }）。
                // 仅在 Lambda 块体内生效；函数体本身的尾表达式（外层闭包）不包装
                let nested_fn_body = self.nested_fn_ret
                    && self.in_lambda_block
                    && matches!(&expr.kind, ExprKind::Lambda { .. });
                let expr_s = if nested_fn_body {
                    format!("Box::new({})", self.gen_expr(expr))
                } else {
                    self.gen_expr(expr)
                };
                if is_last && !self.is_main && !self.suppress_tail_return {
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
                        self.emit_line(&format!("return {}{}.{};", prefix, "self", field));
                    } else {
                        self.emit_line(&format!("return {};", expr_s));
                    }
                } else if is_last && self.suppress_tail_return && self.force_stmt_semicolon {
                    // 循环体尾表达式：非值上下文，需加分号（否则 E0308）
                    self.emit_line(&format!("{};", expr_s));
                } else if is_last && self.suppress_tail_return && self.force_unit_tail {
                    // 块内含无值 return（return;）→ 尾表达式丢弃值（expr;），
                    // 使闭包返回类型为 ()，避免与 return; 冲突（E0308）
                    self.emit_line(&format!("{};", expr_s));
                } else if is_last && self.suppress_tail_return {
                    // match arm / 块表达式尾值 → 裸表达式（无分号，作为块值）
                    self.emit_line(&format!("{}", expr_s));
                } else if is_last {
                    // main 函数尾表达式 → expr;
                    self.emit_line(&format!("{};", expr_s));
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
                    self.gen_block_inner(then_branch);
                    self.indent -= 1;
                    self.emit_line("} else {");
                    self.indent += 1;
                    self.gen_block_inner(else_blk);
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
                // self（&Vec<T>）上的 for 循环：.into_iter() 有 &Vec/Vec 双 IntoIterator
                // 歧义（E0034 multiple into_iter found），用 .iter() 明确（item=&T）
                let iter_is_self_borrow = matches!(&iter.kind, ExprKind::Var(n) if n == "self")
                    && self.borrow_self;
                let iter_expr = |cg: &Self| -> String {
                    let s = cg.gen_expr(iter);
                    // 字符串 for 迭代：String 不实现 IntoIterator（E0599），
                    // 需用 .chars() 逐字符迭代（`for c in "abcd"`）
                    let iter_is_str = matches!(&iter.ty, IrType::Str)
                        || matches!(&iter.ty, IrType::Named { path, .. }
                            if path == "str" || path == "String");
                    if iter_is_str {
                        format!("({}).chars()", s)
                    } else if iter_is_self_borrow {
                        format!("({}).iter()", s)
                    } else if iter_is_ref {
                        format!("(*{}).clone().into_iter()", s)
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
                        let guard_owned = replace_ident_boundary(&guard_s, var, &format!("{}_owned", var));
                        format!(
                            "{}.filter(|{}| {{ let {}_owned = (*{}).clone(); {} }})",
                            base,
                            var,
                            var,
                            var,
                            guard_owned,
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
                self.gen_block_inner(body);
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
                self.gen_block_inner(body);
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
                self.gen_block_inner(body);
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
                    let has_ref_mut = arms.iter().any(|a| {
                        !self.collect_ref_mut_bindings(&a.pattern).is_empty()
                    });
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
                for arm in arms {
                    // 字典模式（{"k": p}）：Rust 无原生 HashMap 模式 → 生成
                    // `_ if <scrut>.contains_key("k")` 守卫 + 臂体内 `let p = <scrut>["k"];`
                    let dict_entries = match &arm.pattern {
                        Pattern::Dict(entries) => Some(entries.clone()),
                        _ => None,
                    };
                    let pat_s = if dict_entries.is_some() {
                        "_".to_string()
                    } else if is_slice_scrutinee {
                        // 切片上下文：元组模式 (a,) / (a, ..) → 切片模式 [a] / [a, ..]
                        self.gen_slice_pattern(&arm.pattern)
                    } else {
                        self.gen_pattern(&arm.pattern)
                    };
                    let guard_s = if let Some(entries) = &dict_entries {
                        let conds: Vec<String> = entries
                            .iter()
                            .map(|(k, _)| {
                                format!("{}.contains_key(\"{}\")", scrut_orig, k.replace('"', "\\\""))
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
                    self.gen_block_inner(&arm.body);
                    self.suppress_tail_return = saved;
                    self.ref_mut_bindings = saved_ref_mut;
                    self.slice_clone_bindings = saved_slice_clone;
                    self.indent -= 1;
                    self.emit_line("}");
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
                self.emit_line(&format!("(|| {{ // block '{}", label));
                self.indent += 1;
                self.plain_block_depth += 1;
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.gen_block_inner(body);
                self.suppress_tail_return = saved;
                self.plain_block_depth -= 1;
                self.indent -= 1;
                self.emit_line("})();");
            }
            Stmt::CheckerBlock { .. } => {
                // checker 块已提升为模块级 Item::CheckerBlock（惰性登记）
                // 此处为占位语句，不生成内联代码
                self.emit_line("();  // checker block (defined at module level)");
            }
            Stmt::Pass => {
                // pass 占位：非 Unit 返回函数中（如 box.lz `fn get(&self) -> &T` 的
                // 内建占位方法）生成 unimplemented!()，否则 `()` 与返回类型不匹配（E0308）
                let ret_is_unit = matches!(
                    self.current_ret_ty,
                    None | Some(IrType::Unit)
                );
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
                self.emit_line(&format!("panic!(\"{{}}\", {});", self.gen_expr(value)));
            }
            Stmt::Assert { cond, message: _ } => {
                self.emit_line(&format!("assert!({});", self.gen_expr(cond)));
            }
            Stmt::Yield { value } => {
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
                self.emit_line(&format!("// yield from {}", self.gen_expr(iter)));
                self.emit_line(&format!(
                    "__gen_vec.extend({}.into_iter());",
                    self.gen_expr(iter)
                ));
            }
            Stmt::Defer { body: _ } => {
                // Defer 在 Rust 中使用 Drop trait 或 defer-lite crate
                self.emit_line("// defer");
            }
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                // try/catch → std::panic::catch_unwind pattern
                let has_catch = !catches.is_empty();
                let has_else = else_body.is_some();
                let has_finally = finally_body.is_some();

                // ── catch_unwind wrapping ──
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
                                    let safe =
                                        if self.downgraded_vars.contains(var_name.as_str()) {
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
                let n = stmts.len();
                for (i, s) in stmts.iter().enumerate() {
                    self.gen_stmt(s, i == n - 1);
                }
                self.declared = saved_declared;
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}");
            }
            #[allow(unreachable_patterns)]
            _ => self.emit_line("// TODO: Stmt variant not yet supported"),
        }
    }

    // ── Expr 生成 ──

    /// 生成索引 key：Rust 的 Vec/切片/字符串索引需要 usize，
    /// 而 LZ 的 int 是 i64，因此对整数索引自动转换为 usize。
    /// 对 HashMap/Dict 保持引用语义（contains_key/get 需要 &K）。
    fn gen_index_key(&self, key: &Expr, base: &Expr) -> String {
        // Range 切片 key（string.lz slice `self[start..end]`）：AST Range →
        // StructCtor{name:"Range"}，start/end 需转 usize（str/Vec 索引要求 usize）
        if let ExprKind::StructCtor { name, fields } = &key.kind {
            if name == "Range" {
                let start = fields
                    .iter()
                    .find(|(n, _)| n == "start")
                    .map(|(_, v)| format!("({} as usize)", self.gen_expr(v)));
                let end = fields
                    .iter()
                    .find(|(n, _)| n == "end")
                    .map(|(_, v)| format!("({} as usize)", self.gen_expr(v)));
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
            || (matches!(&key.ty, IrType::Any)
                && matches!(&key.kind, ExprKind::Var(_)));
        // 对整数 key（i64）转换为 usize，除非目标是 dict（其 key 不是数值索引）
        if !is_dict && key_is_numeric {
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
            if !is_dict && is_self_field_key && (is_self_field_base || is_container_base) {
                format!("({} as usize)", key_s)
            } else {
                key_s
            }
        }
    }

    /// 生成赋值目标表达式（不放 unsafe 包装，用于 Stmt::Assign 等）
    fn gen_target_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Var(name) => name.clone(),
            ExprKind::FieldAccess { base, field } => {
                // type-pack 异质元组索引（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的 args
                // 编译为切片 &[Ts]，`args.0` 映射为 `args[0]`（Rust 切片索引）；
                // 数字字段名仅在 base 是集合/切片类型时按索引处理
                let is_numeric_field = !field.is_empty() && field.chars().all(|c| c.is_ascii_digit());
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
                    let field_is_upper =
                        field.chars().next().map_or(false, |c| c.is_uppercase());
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
                        format!("{}.{}", base_s, field)
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
                let ret_is_ref_like =
                    matches!(&self.current_ret_ty, Some(IrType::Ref(_) | IrType::MutRef(_)))
                        || matches!(&self.current_ret_ty, Some(IrType::Option(inner))
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

    fn gen_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Lit(lit) => self.gen_lit(lit, &expr.ty),
            ExprKind::Var(name) => {
                if name == "pass" {
                    "()".into()
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
                } else {
                    name.clone()
                }
            }
            ExprKind::Call {
                callee,
                args,
                type_args,
            } => {
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
                    args.iter().map(|a| self.gen_expr(a)).collect()
                };
                // 函数参数调用（predicate: fn(ref X) -> bool）：callee 是 Fn 变量且
                // 参数为 ref 时，实参自动取引用（&item），否则 E0308 expected &
                if let Some(ref_flags) = &callee_fn_refs {
                    for (i, s) in args_s.iter_mut().enumerate() {
                        if i < ref_flags.len() && ref_flags[i] && !s.starts_with('&') {
                            *s = format!("&{}", s);
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
                            let packed_args: Vec<String> = args_s
                                .iter()
                                .map(|a| format!("Box::new({})", a))
                                .collect();
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
                            let is_ref_param = ref_flags
                                .get(i)
                                .map_or(false, |(r, _)| *r);
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
                            let arg_is_var = matches!(&a.kind, ExprKind::Var(_));
                            let arg_is_copy = matches!(
                                &a.ty,
                                IrType::Int | IrType::F64 | IrType::Bool
                            );
                            // Fn 类型参数（impl Fn(...) opaque）不可 clone（E0599）：
                            // 实参是闭包变量时直接传引用即可，不自动 .clone()
                            let param_is_fn = matches!(&callee_ptypes[i], IrType::Fn { .. });
                            if !param_is_copy && arg_is_var && !arg_is_copy && !param_is_fn && i < args_s.len() {
                                let s = &args_s[i];
                                let is_none_lit = s.trim_end() == "None";
                                if !s.starts_with('&')
                                    && !s.ends_with(".clone()")
                                    && !s.contains("::")
                                    && !is_none_lit
                                {
                                    args_s[i] = format!("{}.clone()", s);
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
                if let Some(&(total_params, def_count)) = self.fn_param_info.get(&callee_s) {
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
                        // 第三个参数存在 → 过滤条件（filter 闭包接收 &Item，用 & 解引用参数）
                        if let Some(cond) = args_s.get(2) {
                            let cond = strip_lambda_type_with_ref(cond);
                            return format!(
                                "({}).into_iter().filter({}).map({}).collect::<Vec<_>>()",
                                iter, cond, lambda
                            );
                        }
                        return format!(
                            "({}).into_iter().map({}).collect::<Vec<_>>()",
                            iter, lambda
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
                    let field_is_uppercase =
                        field.chars().next().map_or(false, |c| c.is_uppercase());
                    let sep =
                        if is_var_base && (is_std_module || is_known_type) && field_is_uppercase {
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
                                    a.clone()
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
                            let elem = match &expr.ty {
                                IrType::Named { path, args }
                                    if path == "Option" && args.len() == 1 =>
                                {
                                    self.rust_type(&args[0])
                                }
                                _ => "i64".to_string(),
                            };
                            return format!("Option::<{}>::None", elem);
                        }
                        return format!("{}::{}({})", base_s, field, wrapped_args.join(", "));
                    }
                    // else: normal field access call, fall through
                }

                // 检测 enum variant 构造器调用: Circle(0,0,5) → Shape::Circle(0, 0, 5)
                if let Some(enum_name) = self.enum_variants.get(&callee_s) {
                    return if args_s.is_empty() {
                        format!("{}::{}", enum_name, callee_s)
                    } else {
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
                        format!("{}::{}({})", enum_name, callee_s, args_c.join(", "))
                    };
                }

                // 类型转换: int(x) → x as i64, str(x) → format!("{}", x), f64(x) → x as f64
                if matches!(callee_s.as_str(), "int" | "str" | "f64" | "float")
                    && !args_s.is_empty()
                {
                    return match callee_s.as_str() {
                        "int" => {
                            // 检查参数表达式类型来决定转换方式
                            if args.len() == 1 {
                                let arg_ty = &args[0].ty;
                                if matches!(arg_ty, IrType::Str) {
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
                } else if callee_s == "contains" && args_s.len() == 2 {
                    // HashMap/Dict → contains_key; String/Vec → contains
                    let is_dict = matches!(&args[0].ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                    if is_dict {
                        format!("({}).contains_key(&{})", args_s[0], args_s[1])
                    } else {
                        format!("({}).contains(&{})", args_s[0], args_s[1])
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
                            format!("\"{}\"", s)
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
                } else if callee_s == "sum" && args_s.len() == 1 {
                    // sum(collection) → collection.iter().copied().sum::<i64>()
                    // （.iter() 产出 &i64，.copied() 转值；.sum::<i64>() 显式类型
                    // 标注，否则 E0283 cannot infer type parameter S）
                    format!("({}).iter().copied().sum::<i64>()", args_s[0])
                } else if callee_s == "map" && args_s.len() == 2 {
                    // map(collection, fn) → collection.into_iter().map(fn).collect::<Vec<_>>()
                    // LZ 自由函数 map 返回 List（与链式 .map 不同），需 collect 成 Vec
                    let lambda = strip_lambda_type(&args_s[1]);
                    format!(
                        "({}).into_iter().map({}).collect::<Vec<_>>()",
                        args_s[0], lambda
                    )
                } else if callee_s == "filter" && args_s.len() == 2 {
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
                } else if callee_s == "fold" && args_s.len() == 3 {
                    // fold(collection, init, fn) → collection.into_iter().fold(init, fn)
                    let lambda = strip_lambda_type(&args_s[2]);
                    format!(
                        "({}).into_iter().fold({}, {})",
                        args_s[0], args_s[1], lambda
                    )
                } else if callee_s == "collect" && args_s.len() == 1 {
                    // collect(iterable)：输入可能是迭代器或已 collect 的 Vec（管道链
                    // filter 已返回 Vec，再 collect 报 E0599 no method collect on Vec）。
                    // into_iter() 对两者都有效（Iterator: IntoIterator 恒等，Vec 消费）
                    format!("({}).into_iter().collect::<Vec<_>>()", args_s[0])
                } else if callee_s == "max" && args_s.len() == 1 {
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
                } else if !args.is_empty() && is_kwarg_call(args) && self.is_known_type(&callee_s) {
                    // Struct constructor with keyword args: Point(x=3, y=4) → Point { x: 3.0, y: 4.0 }
                    let base_name = callee_s.split('<').next().unwrap_or(&callee_s).to_string();
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
                                    if let Some((_, fty)) =
                                        info.iter().find(|(n, _)| n == &fname)
                                    {
                                        if let IrType::Named { path, args } = fty {
                                            if (path == "Vec" || path == "List")
                                                && !args.is_empty()
                                                && s.split_once(':')
                                                    .map(|(_, v)| v.trim())
                                                    .map_or(false, |v| {
                                                        v == "Vec::<i64>::new()"
                                                            || v == "Vec::new()"
                                                            || v == "vec![]"
                                                    })
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
                                        matches!(&v.ty, IrType::Option(_))
                                            || matches!(&v.kind, ExprKind::Var(n)
                                                if n == "None" || n == "Some")
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
                            all_fields.push(format!(
                                "_lz_phantom_{}: std::marker::PhantomData,",
                                g
                            ));
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
                    let mut all_args: Vec<String> = normal_args.to_vec();
                    if is_tuple_variadic {
                        let prefix_n = args_s
                            .len()
                            .saturating_sub(variadic_idx)
                            .min(
                                match &self.fn_param_types.get(&callee_s).and_then(|pts| pts.get(variadic_idx)) {
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
                        "Dict" | "HashMap" => "std::collections::HashMap::new()".to_string(),
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
                                if is_self_ref && !s.contains(".clone()") {
                                    format!("{}.clone()", s)
                                } else {
                                    s.clone()
                                }
                            })
                            .collect()
                    } else {
                        args_s.clone()
                    };
                    let call_str = format!("{}{}({})", callee_s, turbofish, args_c.join(", "));
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
                // self.iter.next()（方法调用借用 self 字段推进迭代器）：gen_expr 的
                // self 字段访问生成 self.iter.clone()——clone 后 next 不推进原 iter，
                // collect 无限迭代死循环（traits.lz Enumerate 的 __next__），去掉 .clone()
                let recv = if recv.starts_with("self.") && recv.ends_with(".clone()") {
                    recv.trim_end_matches(".clone()").to_string()
                } else {
                    recv
                };
                let mut args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
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
                    let ret_is_result_like = self
                        .current_ret_ty
                        .as_ref()
                        .map(|rt| {
                            matches!(rt, IrType::Option(_) | IrType::Result { .. })
                                || matches!(rt, IrType::Named { path, .. }
                                    if path == "Option" || path == "Result")
                        })
                        .unwrap_or(false);
                    if ret_is_result_like {
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
                if self.is_known_type_or_enum(&recv)
                    || matches!(recv.as_str(), "Option" | "Result")
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
                    return format!("{}::{}({})", recv, method, wrapped_args.join(", "));
                }

                // 判断 receiver 是否为用户自定义 struct（有对应魔术方法时用魔术方法名）
                let recv_is_struct =
                    matches!(&receiver.ty, IrType::Named { path, .. } if self.is_known_type(path));

                // LZ magic methods → Rust equivalents
                // plus common method name mappings
                // 注意：算术/比较魔术方法（__add__/__eq__ 等）保留原名，
                // 因为用户 struct 的 impl 方法就叫 __add__；__str__/__iter__ 用于
                // str() 转换和迭代的容器场景，继续映射
                let rust_method = match method.as_str() {
                    // 用户 struct：len/iter/next/contains 等映射到魔术方法
                    "len" if recv_is_struct => "__len__",
                    "iter" if recv_is_struct => "__iter__",
                    "next" if recv_is_struct => "__next__",
                    "getitem" if recv_is_struct => "__getitem__",
                    "setitem" if recv_is_struct => "__setitem__",
                    "contains" if recv_is_struct => "__contains__",
                    // impl Iterator 块内调用迭代器元素上的迭代方法：
                    // `self.a.__next__()`（A: Iterator 为 std trait，方法是 next）→ .next()
                    // 泛型 receiver（Peekable 的 self.iter.__next__()，I 非已知 struct）
                    // 也映射 next（E0599 no method __next__ on type parameter I）
                    "__next__" if self.in_iterator_impl || !recv_is_struct => "next",
                    "__size_hint__" if self.in_iterator_impl => "size_hint",
                    // 非用户 struct 的 __str__/__iter__ 用于内置容器/字符串场景
                    // self.__str__()（trait 默认方法，如 Error::description）保留方法
                    // 调用（self 实现 LZ Display trait），映射 to_string 需 std Display
                    "__str__" if !recv_is_struct && recv != "self" => "to_string",
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
                    "add" if !(matches!(&receiver.ty, IrType::Named { path, .. }
                        if self.struct_method_names(path).contains("add"))) => "insert",
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
                    "split" => "split",
                    "join" => "join",
                    "replace" => "replace",
                    "trim" => "trim",
                    "starts_with" => "starts_with",
                    "ends_with" => "ends_with",
                    "new"
                        if self.emitted_types.contains(&recv)
                            || recv == "Box"
                            || recv == "Rc"
                            || recv == "Arc" =>
                    {
                        // Static method on type → use :: syntax
                        return format!("{}::new({})", recv, args_s.join(", "));
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
                if pattern_methods.contains(&method.as_str()) && !args_s.is_empty() {
                    // 仅对 str/String receiver 应用（String Pattern trait 方法）：
                    // 自定义类型的 get（list.lz `lst.get(1)` 参数是 i64）不受影响
                    let recv_is_str = matches!(&receiver.ty, IrType::Str)
                        || matches!(&receiver.ty, IrType::Named { path, .. }
                            if path == "str" || path == "String");
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
                    let recv_has_custom_contains = matches!(&receiver.ty, IrType::Named { path, .. }
                        if self.is_known_type(path)
                            && self.struct_method_names(path).contains("contains"));
                    let is_kwargs = matches!(&receiver.kind, ExprKind::FieldAccess { field, .. }
                        if field == "kwargs");
                    let need_ref = recv_is_str
                        // Vec contains：仅对**字面量列表** receiver（[1,2,3].contains(2)）
                        // 加 &（std Vec::contains 需 &T）；变量 receiver 的 contains
                        // 走 lz_std 自定义 ListExt::contains（参数值语义 T），
                        // 误加 & 会 E0308 expected i64, found &i64（list.lz）
                        || (recv_is_vec && method == "contains" && !recv_has_custom_contains
                            && matches!(&receiver.kind, ExprKind::ListLit(_)))
                        || (recv_is_dict && (rust_method == "contains_key" || method == "get" || method == "remove"))
                        || (recv_is_set && (method == "contains" || method == "remove"))
                        || (is_kwargs && (rust_method == "contains_key" || method == "get"))
                        // closure 参数 receiver（operators.lz `__sn.get("key")`，__sn 类型
                        // 推断为 Any）：get 的实参是 String 时必为 HashMap::get（需 &Q），
                        // Vec::get 的实参是 int（走 usize 转换），不会误伤
                        || (method == "get"
                            && (matches!(&args[0].ty, IrType::Str)
                                || matches!(&args[0].ty, IrType::Named { path, .. }
                                    if path == "str" || path == "String")));
                    if need_ref {
                        // String Pattern 方法（replace/starts_with/split 等）的**所有**
                        // 字符串参数都需 &str：`s.replace("-", "+")` 的 from/to 两个参数。
                        // 旧实现只处理 args_s[0]，第二个字符串字面量参数被包装成
                        // "+".to_string()（String）→ E0308 expected &str。
                        // Vec/List 的 contains 参数需 &T（含 int：`[1,2,3].contains(2)`
                        // → E0308 expected &i64）。只处理字符串字面量（直接用 &str）、
                        // 字符串类型值（加 &）与 Vec contains 的任意参数（加 &），
                        // 不误伤 Vec::get 的 usize 索引参数。
                        let is_vec_contains = recv_is_vec && method == "contains"
                            && matches!(&receiver.kind, ExprKind::ListLit(_));
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
                            if let Some(slot) = args_s.get_mut(idx) {
                                if let ExprKind::Lit(LitKind::Str(s)) = &arg_expr.kind {
                                    *slot = format!("\"{}\"", s);
                                } else if !slot.starts_with('&')
                                    && !matches!(&arg_expr.ty, IrType::Ref(_) | IrType::MutRef(_))
                                    && (is_vec_contains
                                        || is_set_remove
                                        || is_set_contains
                                        || is_dict_contains_key
                                        || matches!(&arg_expr.ty, IrType::Str)
                                        || matches!(&arg_expr.ty, IrType::Named { path, .. }
                                            if path == "str" || path == "String"))
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
                    "__add__", "__sub__", "__mul__", "__div__", "__lt__", "__gt__",
                    "__le__", "__ge__", "__eq__", "__ne__",
                ];
                let is_compare_magic = matches!(
                    method.as_str(),
                    "__eq__" | "__ne__"
                );
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
                if non_consuming_magic.contains(&method.as_str()) && recv_is_struct && !compare_arg_is_ref {
                    let recv_c = format!("({}).clone()", recv);
                    let args_c: Vec<String> = args_s
                        .iter()
                        .zip(args.iter())
                        .map(|(s, a)| {
                            // 数值标量（如 5.0）无需 clone
                            let is_scalar =
                                matches!(&a.ty, IrType::Int | IrType::F64 | IrType::Bool);
                            if is_scalar {
                                s.clone()
                            } else {
                                format!("({}).clone()", s)
                            }
                        })
                        .collect();
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
                if (method == "insert" || method == "insert_at" || method == "remove" || method == "get")
                    && !args_s.is_empty()
                    && matches!(&args[0].ty, IrType::Int)
                    && !args_s[0].starts_with('&')
                    && !has_custom_get
                    && !recv_is_set_ty
                {
                    args_s[0] = format!("({} as usize)", args_s[0]);
                }
                let call = if recv.starts_with('<') && recv.contains(">::") && !recv.ends_with(')') {
                    // 关联类型路径 receiver（`<Self as std::iter::Iterator>::Item.default()`）：
                    // Item 是关联类型，方法调用用 ::（`<Self as std::iter::Iterator>::Item::default()`），
                    // 否则 E0575 expected method, found associated type Iterator::Item。
                    // 注意：StrExt 强制调用 `<str as StrExt>::find(self, substr)` 以 ) 结尾
                    // （函数调用而非关联路径），后续 .is_some() 必须用 .（E0308 语法错误）
                    format!("{}::{}({})", recv, rust_method, args_s.join(", "))
                } else {
                    format!("{}.{}({})", recv, rust_method, args_s.join(", "))
                };
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
                    && matches!(method.as_str(), "chars" | "find" | "rfind" | "replace" | "repeat" | "trim_start" | "trim_end" | "split" | "lines")
                {
                    if method == "find" {
                        eprintln!(
                            "DBG strext: method={} recv_ty={:?} recv_str={} recv={}",
                            method, receiver.ty, recv_is_str, recv
                        );
                    }
                    // StrExt::find 参数是 String（owned），去掉 pattern_methods 加的 &
                    // 并恢复字符串字面量的 to_string（pattern_methods 曾去掉）
                    let args_owned: Vec<String> = args_s
                        .iter()
                        .enumerate()
                        .map(|(_, s)| {
                            let s = if method == "find" && s.starts_with('&') && !s.starts_with("&&") {
                                s[1..].to_string()
                            } else {
                                s.clone()
                            };
                            // 字符串字面量（pattern_methods 已把 "...".to_string() 简化为
                            // "..."）恢复 to_string：StrExt 方法参数是 String（E0308）
                            if s.starts_with('"') && !s.contains(".to_string()") {
                                format!("{}.to_string()", s)
                            } else {
                                s
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
                if recv_is_dict
                    && matches!(method.as_str(), "keys" | "values" | "items" | "iter" | "iter_keys" | "iter_values")
                {
                    let recv_ref = if recv == "self" || recv.starts_with('&') {
                        recv.clone()
                    } else if recv.starts_with('(') {
                        format!("&{}", recv)
                    } else {
                        format!("&({})", recv)
                    };
                    return format!(
                        "DictExt::{}({}, {})",
                        rust_method,
                        recv_ref,
                        args_s.join(", ")
                    );
                }
                // SetExt trait 方法强制调用：HashSet 的 union/intersection/difference/
                // symmetric_difference 与 std HashSet 固有方法同名（固有优先返回
                // Union/Intersection 迭代器而非 LZ 的 Set，E0599/E0308）——显式 SetExt::
                let recv_is_set = matches!(&receiver.ty, IrType::Named { path, .. }
                    if path == "Set" || path == "HashSet");
                if recv_is_set
                    && matches!(method.as_str(), "union" | "intersection" | "difference" | "symmetric_difference" | "iter")
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
                // 依赖 T: PartialEq 约束（box.lz `where T: Eq` → E0599 __eq__ not found）
                if !recv_is_struct
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
                                args_s.iter().map(|a| deref_str(a)).collect::<Vec<_>>().join(", ")
                            )
                        }
                        "__ne__" => {
                            return format!(
                                "{} != {}",
                                deref_expr(self, receiver),
                                args_s.iter().map(|a| deref_str(a)).collect::<Vec<_>>().join(", ")
                            )
                        }
                        "__lt__" => {
                            return format!(
                                "{} < {}",
                                deref_expr(self, receiver),
                                args_s.iter().map(|a| deref_str(a)).collect::<Vec<_>>().join(", ")
                            )
                        }
                        "__gt__" => {
                            return format!(
                                "{} > {}",
                                deref_expr(self, receiver),
                                args_s.iter().map(|a| deref_str(a)).collect::<Vec<_>>().join(", ")
                            )
                        }
                        "__le__" => {
                            return format!(
                                "{} <= {}",
                                deref_expr(self, receiver),
                                args_s.iter().map(|a| deref_str(a)).collect::<Vec<_>>().join(", ")
                            )
                        }
                        "__ge__" => {
                            return format!(
                                "{} >= {}",
                                deref_expr(self, receiver),
                                args_s.iter().map(|a| deref_str(a)).collect::<Vec<_>>().join(", ")
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
                let recv_is_option = matches!(
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
                    if method == "filter"
                        && args_s.len() == 1
                        && recv_is_list
                    {
                        // ListExt::filter 的闭包参数是 &T（fn(ref T) -> bool），
                        // 用 |x: &T| 直接绑定（strip_lambda_type_with_ref 的 |&x| 模式
                        // 是为 std Iterator::filter 的 &&T 参数设计的，此处会多解一层
                        // 引用 → E0614 type i64 cannot be dereferenced）
                        return format!("({}).filter({})", recv, args_s[0]);
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
                    ) || matches!(
                        &receiver.ty,
                        IrType::Result { .. } | IrType::Option(_)
                    ))
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
                } else if method == "first" || method == "last"
                    || (method == "get" && self.is_collection_type(&receiver.ty))
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
                // Enum variant: Color.Red → Color::Red (field 大写开头)
                // Module path: std.io.print → std::io::print
                // Method/field access: config.get() -> config.get (field 小写开头)
                // duck 约束泛型参数的字段访问：a.field → a.__field_field()（trait accessor）
                // type-pack 异质元组索引（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的 args
                // 编译为切片 &[Ts]，`args.0` 映射为 `args[0]`（Rust 切片索引）
                let is_numeric_field = !field.is_empty() && field.chars().all(|c| c.is_ascii_digit());
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
                if base_s == "self"
                    && field.chars().next().map_or(false, |c| c.is_uppercase())
                {
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
                let access_s = format!("{}{}{}", base_s, sep, field);
                // Option::None（无参变体）：注入类型参数，避免闭包返回位置
                // 无法推断 T（E0282，如 opt.and_then(|x| Option.None)）
                let access_s = if sep == "::" && field == "None" && base_s == "Option" {
                    // 泛型函数（如 map<R> 内 `case Option.None => Option.None`）中
                    // 硬编码 i64 错误：Option::None 让 Rust 从 match 臂配对推断（combo-struct-method.lz）；
                    // 用户自定义 `enum Option<T>` 遮蔽 std Option 时裸 None 类型不匹配（enum.lz）
                    if self.in_generic_fn {
                        "Option::None".to_string()
                    } else {
                        let elem = match &expr.ty {
                            IrType::Named { path, args } if path == "Option" && args.len() == 1 => {
                                self.rust_type(&args[0])
                            }
                            _ => "i64".to_string(),
                        };
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
                    // 也检查是否为 kwargs 字段（__Params 的 kwargs 是 HashMap）
                    let is_kwargs = matches!(&base.kind, ExprKind::FieldAccess { field, .. } if field == "kwargs");
                    // 用户 struct：ml[0] → ml.__getitem__(0)（key 保持 i64，内部 self.items[i] 再转 usize）
                    let is_struct =
                        matches!(&base.ty, IrType::Named { path, .. } if self.is_known_type(path));
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
                        let ret_is_ref_like =
                            matches!(&self.current_ret_ty, Some(IrType::Ref(_) | IrType::MutRef(_)))
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
                        let base_is_str_any = matches!(&base.ty, IrType::Str)
                            || matches!(&base.ty, IrType::Named { path, .. }
                                if path == "str" || path == "String");
                        let is_range_key_any = matches!(&key.kind,
                            ExprKind::StructCtor { name, .. } if name == "Range");
                        if base_is_str_any && is_range_key_any {
                            return format!("{}[{}].to_string()", base_s, key_s);
                        }
                        if base_is_str_any {
                            return format!("(({}).as_bytes()[{}] as i64)", base_s, key_s);
                        }
                        let is_self_field = matches!(&base.kind, ExprKind::FieldAccess { base: b, .. } if matches!(&b.kind, ExprKind::Var(n) if n == "self"));
                        let is_self_base = matches!(&base.kind, ExprKind::Var(n) if n == "self");
                        // 值上下文取 self 的索引：Rust 的 a[i] 是 *index()（T 值），
                        // move 出容器报 E0507——clone 为 owned（T: Clone，pop/remove_at）
                        // ref 返回上下文（__getitem__/Some(self[i])）用 &self[i]，不 clone
                        let ret_is_ref_like =
                            matches!(&self.current_ret_ty, Some(IrType::Ref(_) | IrType::MutRef(_)))
                                || matches!(&self.current_ret_ty, Some(IrType::Option(inner))
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
                                // 字符串单字符索引 s[i]：Rust 不支持 str 按 usize
                                // 索引（E0277 SliceIndex），映射为字节索引取字符码
                                // （simple_hash `let c = s[i]`，comptime 焊死场景）
                                format!("(({}).as_bytes()[{}] as i64)", base_s, key_s)
                            } else {
                                format!("{}[{}].clone()", base_s, key_s)
                            }
                        } else {
                            format!("{}[{}]", base_s, key_s)
                        }
                    }
                }
            }
            ExprKind::IndexSet { base, key, value } => {
                let base_s = self.gen_expr(base);
                let key_s = self.gen_index_key(key, base);
                eprintln!("DBG IndexSet base_kind={:?} base_ty={:?}", base.kind, base.ty);
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
                } else {                    format!("{}[{}] = {}", base_s, key_s, self.gen_expr(value))
                }
            }
            ExprKind::BinOp { op, lhs, rhs } => {
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
                            format!("\"{}\"", s)
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
                    // List/Set/其他集合: elem in container → container.contains(&elem)
                    return format!("{}{}.contains(&{})", not_prefix, cont_s, elem_s);
                }
                // String + 拼接: 右侧需借用 & 以匹配 Rust Add<&str>
                // 但如果 rhs 是 variadic 参数（类型已是 &[T]），不应再加 &
                let str_concat = matches!(&rhs.ty, IrType::Str) || matches!(&lhs.ty, IrType::Str);
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
                        );
                    let lhs_base = if lhs_is_ref_str {
                        format!("{}.to_string()", lhs_s)
                    } else {
                        lhs_s
                    };
                    let rhs_is_variadic = matches!(&rhs.kind, ExprKind::Var(name) if self.current_variadic_params.contains(name));
                    if rhs_is_variadic {
                        return format!("{} + {}", lhs_base, rhs_s);
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
                        let l = if lhs_is_ref { format!("*{}", lhs_s) } else { lhs_s };
                        let r = if rhs_is_ref { format!("*{}", rhs_s) } else { rhs_s };
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
                let arith = matches!(op, BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul | BinOpKind::Div | BinOpKind::Mod);
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
                let lhs_s = self.wrap_bin_operand(self.gen_expr(lhs));
                let rhs_s = self.wrap_bin_operand(self.gen_expr(rhs));
                if arith && lhs_is_f64 && rhs_is_numeric && !rhs_is_f64 {
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
                    let has_cmp = inner.contains(" == ") || inner.contains(" != ")
                        || inner.contains(" < ") || inner.contains(" > ")
                        || inner.contains(" <= ") || inner.contains(" >= ");
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
                // 三元 then/else 类型统一：then 是 bool 而 else 是数值时，
                // else 按 LZ 真值语义转 bool（非零为真），如
                // `(n := compute()) > 5 if n * 10 else 0`（combo_ternary_walrus.lz）
                if matches!(&then.ty, IrType::Bool)
                    && matches!(&els.ty, IrType::Int | IrType::F64)
                {
                    els_s = format!("({}) != 0", els_s);
                }
                // 如果 then 或 els 包含多行 BlockExpr，使用多行格式确保缩进正确
                if then_s.contains('\n') || els_s.contains('\n') {
                    // emit_line 会在字符串前添加 self.indent 级别的缩进，
                    // 所以这里的内容缩进只需 self.indent + 1（相对于 if 行再缩进一层）
                    let close_indent = "    ".repeat(self.indent);
                    let inner_indent = "    ".repeat(self.indent + 1);
                    let then_body = if then_s.starts_with("{\n") {
                        // BlockExpr: 重新格式化内容，使用正确的缩进级别
                        let inner = &then_s[2..then_s.len() - 1]; // 去掉 { 和 }
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
                        format!(" {}", then_s)
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
                        then_s,
                        els_s
                    )
                }
            }
            ExprKind::Lambda { params, body, .. } => {
                // 嵌套 Fn 返回（fn -> fn -> T）：内层闭包作为外层返回值需 Box::new 包装
                // （factory_chain: |a| => |b| => x + a + b → move |a| { Box::new(move |b| {...}) }）
                let nested = self.nested_fn_ret
                    && matches!(&body.kind, ExprKind::Lambda { .. });
                // 未使用的闭包参数（Any 类型）无法从上下文推断 → 加 i64 标注
                // （如 Option.None.and_then(|x| Option.None)，E0282）
                let mut body_s = self.gen_expr(body);
                if nested {
                    body_s = format!("Box::new({})", body_s);
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
                if let ExprKind::BlockExpr { block } = &body.kind {
                    let mut child = CodeGen::new();
                    child.emitted_types = self.emitted_types.clone();
                    child.enum_variants = self.enum_variants.clone();
                    child.fn_param_info = self.fn_param_info.clone();
                    child.current_variadic_params = self.current_variadic_params.clone();
                    // 传递 static/global 变量名集合（用于 E0530 冲突检测）
                    child.global_vars = self.global_vars.clone();
                    child.top_level_static_names = self.top_level_static_names.clone();
                    child.downgraded_vars = self.downgraded_vars.clone();
                    child.mutated_consts = self.mutated_consts.clone();
                    child.in_generator = self.in_generator;
                    // 泛型函数标志需传递给 child（match 臂内 Option.None 的裸 None 推断，
                    // combo-struct-method.lz map<R> 泛型方法）
                    child.in_generic_fn = self.in_generic_fn;
                    // Lambda 体内不生成 return，让尾表达式成为闭包返回值
                    child.suppress_tail_return = true;
                    // 嵌套 Fn 返回：内层闭包尾表达式需 Box::new 包装（E0562）
                    child.nested_fn_ret = self.nested_fn_ret;
                    child.in_lambda_block = true;
                    // 块内含无值 return（return;）→ 尾表达式丢弃值（生成 expr;），
                    // 使闭包返回类型为 ()（var_call_block.lz demo_return_no_value）
                    child.force_unit_tail = block_has_bare_return(block);
                    child.gen_block_inner(block);
                    // 闭包体内赋值外部捕获变量（iter.lz for_each `|x| total = total + x`）：
                    // 用借用捕获（非 move），否则 move 复制 total 副本，外部变量不更新
                    let uses_move = !block_has_external_assign(block, &params);
                    let move_kw = if uses_move { "move " } else { "" };
                    format!(
                        "{move_kw}|{}| {{\n{}        }}",
                        params.join(", "),
                        child.buf.trim()
                    )
                } else {
                    // 闭包体内赋值外部捕获变量 → 借用捕获（非 move）
                    let uses_move = !expr_has_external_assign(body, &params);
                    let move_kw = if uses_move { "move " } else { "" };
                    format!("{move_kw}|{}| {{ {} }}", params.join(", "), body_s)
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
                            "std::collections::HashMap::new()".to_string()
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
                            format!("std::collections::HashMap::from([{}])", pairs.join(", "))
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
                            format!(
                                "std::collections::HashSet::from([{}])",
                                items.join(", ")
                            )
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
                        if self.struct_has_new.contains(name.as_str()) || has_new_method {
                            let values: Vec<String> =
                                fields.iter().map(|(_, v)| self.gen_expr(v)).collect();
                            return format!("{}::__new__({})", name, values.join(", "));
                        }
                        // 自动补 PhantomData 字段（box.lz `Box(_ptr: 0)` → `Box { _ptr: 0, _lz_phantom_T: PhantomData }`，
                        // 否则 E0063 missing field `_lz_phantom_T`）
                        let mut fields: Vec<String> = fields
                            .iter()
                            .map(|(n, v)| format!("{}: {}", n, self.gen_expr(v)))
                            .collect();
                        if let Some(phantoms) = self.struct_phantom_generics.get(name.as_str()) {
                            for g in phantoms {
                                // PhantomData 不带显式类型参数：T 在调用点（如 main）未绑定，
                                // 让 Rust 从 `_lz_phantom_T: PhantomData<T>` 字段类型推断（E0425）
                                fields.push(format!(
                                    "_lz_phantom_{}: std::marker::PhantomData,",
                                    g
                                ));
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
                            expr_s
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
                    format!("{}::{}({})", enum_name, variant, args_s.join(", "))
                }
            }
            ExprKind::Cast { expr, target } => {
                // Special cases: as bool → != 0, as str → format/to_string
                if *target == IrType::Bool {
                    return format!("{} != 0", self.gen_expr(expr));
                }
                if *target == IrType::Str {
                    return format!("format!(\"{{}}\", {})", self.gen_expr(expr));
                }
                // String/str → 数值：fallible 解析（str→int 按 09-错误处理.md §2.4）
                // Rust 的 `as` 不允许 String→数值，必须用 .parse()
                let src_is_string = matches!(expr.ty, IrType::Str)
                    || matches!(&expr.ty, IrType::Named { path, .. } if path == "String");
                let tgt_is_numeric = matches!(target, IrType::Int | IrType::F64);
                if src_is_string && tgt_is_numeric {
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
                child.emitted_types = self.emitted_types.clone();
                child.enum_variants = self.enum_variants.clone();
                child.fn_param_info = self.fn_param_info.clone();
                child.in_generator = self.in_generator;
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
                // __gen_vec 已在函数级别声明，BlockExpr 中只需 push 不需要重新声明
                child.gen_block_inner(block);
                format!("{{\n{}    }}", child.buf)
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
                    let elems: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                    format!("({})", elems.join(", "))
                }
            }
            ExprKind::ListLit(elems) => {
                // 空列表：Nil/Unit/Any → ()，否则 → Vec::new() 或 vec![...]
                let is_nil = elems.is_empty()
                    && (matches!(expr.ty, IrType::Unit | IrType::Any)
                        || matches!(self.rust_type(&expr.ty).as_str(), "()"));
                if is_nil {
                    "()".to_string()
                } else {
                    let elems_s: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                    if elems_s.is_empty() {
                        // 空列表：尝试从类型获取元素类型用于 Vec 标注
                        if let IrType::Named { path, args } = &expr.ty {
                            if (path == "List" || path == "Vec") && !args.is_empty() {
                                // 如果元素类型是泛型参数，使用 Vec::new() 让 Rust 推断
                                let elem_is_generic = matches!(&args[0], IrType::Generic(_));
                                if elem_is_generic {
                                    "Vec::new()".to_string()
                                } else {
                                    format!("Vec::<{}>::new()", self.rust_type(&args[0]))
                                }
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
            _ => format!("/* TODO: unsupported expr */"),
        }
    }

    /// 生成布尔条件：用户 struct 类型用 __bool__() 方法
    /// if acc → if acc.__bool__()；if not acc → if !(acc.__bool__())
    /// 判断某类型名是否为用户自定义类型（struct/enum），支持泛型名剥离
    fn is_known_type(&self, name: &str) -> bool {
        // 剥离泛型参数：MyList<i64> → MyList
        let base = name.split('<').next().unwrap_or(name);
        self.known_types.contains(base) || self.emitted_types.contains(base)
    }

    /// 生成字段类型的默认值（用于 __new__ 补齐）
    fn default_value_for(&self, ty: &IrType) -> String {
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

    fn gen_bool_cond(&self, cond: &Expr) -> String {
        // 处理 Not 包裹：not expr → !(expr 转 bool)
        if let ExprKind::UnOp {
            op: UnOpKind::Not,
            operand,
        } = &cond.kind
        {
            let inner = self.gen_bool_cond(operand);
            return format!("!({})", inner);
        }
        // 用户 struct 类型 → 调用 __bool__()
        if let IrType::Named { path, .. } = &cond.ty {
            if self.is_known_type(path) {
                let s = self.gen_expr(cond);
                // 若表达式是赋值等复合，直接调用
                return format!("({}).__bool__()", s);
            }
        }
        // 数值条件：LZ 真值语义非零为真（如 `a if n * 10 else 0`，combo_ternary_walrus.lz）。
        // i64/f64 条件需转 bool 比较，否则 if 条件类型不匹配（E0308）
        if matches!(&cond.ty, IrType::Int | IrType::F64) {
            let s = self.gen_expr(cond);
            return format!("({}) != 0", s);
        }
        self.gen_expr(cond)
    }

    /// 生成 f-string: 提取 {expr} 插值，转成 format!("literal", expr, ...)
    /// {{ / }} 转义为字面量大括号；单个 {expr} 为插值占位符
    fn gen_fstring(&self, s: &str) -> String {
        let mut format_str = String::new();
        let mut args: Vec<String> = Vec::new();
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
                        if self.downgraded_vars.contains(expr_trim) {
                            args.push(format!("{}_", expr_trim));
                        } else if let Some(inner) = expr_trim.strip_prefix("len(").and_then(|s| s.strip_suffix(')')) {
                            // f-string 插值中的 len(x) → (x.len() as i64)
                            args.push(format!("({}.len() as i64)", inner));
                        } else {
                            args.push(self.gen_expr_str(&expr));
                        }
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
            // 使用 {:?} (Debug)：容器（Vec/Option/HashMap）与关联类型只有 Debug
            // 无 Display，统一 {} 会 E0277（containers/duck_assoc/enum 等 7 个回归
            // 失败）。文档示例 `f"x={x}"` 的 x 为 int（{} 与 {:?} 输出相同），
            // 字符串插值按 Debug 语义输出带引号
            fmt_quoted = fmt_quoted.replace(&format!("__LZ_FMT_{}__", i), "{:?}");
        }
        let fmt_quoted = fmt_quoted.replace('"', "\\\"");
        if args.is_empty() {
            format!("format!(\"{}\")", fmt_quoted)
        } else {
            format!("format!(\"{}\", {})", fmt_quoted, args.join(", "))
        }
    }

    /// 将 IR 表达式字符串化（用于 f-string 插值）。简单提取：若为 Var/字段则直接用名字
    fn gen_expr_str(&self, expr: &str) -> String {
        expr.trim().to_string()
    }

    fn gen_lit(&self, lit: &LitKind, _ty: &IrType) -> String {
        match lit {
            LitKind::Int(n) => {
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
            LitKind::F64(f) => {
                // 加 f64 后缀固定类型：泛型调用（@math）推断参数时，
                // 无后缀浮点字面量会探索 f32/f64/f128，触发 unstable f128（E0658）
                let s = f.to_string();
                if s.contains('.') || s.contains('e') {
                    format!("{}f64", s)
                } else {
                    format!("{}.0f64", s)
                }
            }
            LitKind::Str(s) => {
                let escaped = s.escape_default().to_string();
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
    fn wrap_bin_operand(&self, s: String) -> String {
        let trimmed = s.trim_start();
        if trimmed.starts_with("unsafe {") || trimmed.starts_with("unsafe{") {
            format!("({})", s)
        } else {
            s
        }
    }
    fn binop_str(&self, op: &BinOpKind) -> &'static str {
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

    fn unop_str(&self, op: &UnOpKind) -> &'static str {
        match op {
            UnOpKind::Neg => "-",
            UnOpKind::Not => "!",
            UnOpKind::Ref => "&",
            UnOpKind::MutRef => "&mut ",
            UnOpKind::Deref => "*",
        }
    }

    // ── Pattern 生成 ──

    /// 切片上下文模式生成（type-pack 异质元组 03d §2.8 方案 B）：
    /// `..: Tuple<Ts...>` 的 args 编译为切片 &[Ts]，元组模式 `(a,)` / `(a, ..)`
    /// 需转为 Rust 切片模式 `[a]` / `[a, ..]`（否则 E0308 expected slice, found tuple）。
    /// 臂体绑定 a 为 &Ts（切片元素引用），臂体内自动 .clone() 取值。
    fn gen_slice_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Tuple(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_slice_pattern(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Pattern::Rest(name) => match name {
                Some(n) => format!("{} @ ..", n),
                None => "..".into(),
            },
            Pattern::Ident(name) => name.clone(),
            Pattern::Wildcard => "_".into(),
            other => self.gen_pattern(other),
        }
    }

    /// 收集切片模式绑定名（type-pack 异质元组 03d §2.8 方案 B）：
    /// `[a]` / `[a, ..]` 模式中 a 绑定 &Ts（切片元素引用）
    fn collect_slice_bindings(&self, pat: &Pattern, out: &mut Vec<String>) {
        match pat {
            Pattern::Ident(name) => out.push(name.clone()),
            Pattern::Tuple(elems) | Pattern::List(elems) => {
                for e in elems {
                    self.collect_slice_bindings(e, out);
                }
            }
            Pattern::Rest(name) => {
                if let Some(n) = name {
                    out.push(n.clone());
                }
            }
            _ => {}
        }
    }

    fn gen_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Wildcard => "_".into(),
            Pattern::RefMutIdent(name) => {
                // `ref mut c` 模式：c 绑定为 &mut 引用（case Some(ref mut c)）
                // Rust 模式语法为 `Some(ref mut c)`
                if let Some(dot_pos) = name.rfind('.') {
                    let type_name = &name[..dot_pos];
                    let variant = &name[dot_pos + 1..];
                    if self.emitted_types.contains(type_name)
                        || type_name == "Option"
                        || type_name == "Result"
                        || type_name == "Some"
                        || type_name == "None"
                        || type_name == "Ok"
                        || type_name == "Err"
                        || self.enum_variants.contains_key(variant)
                    {
                        format!("{}::{}", type_name, variant)
                    } else {
                        format!("ref mut {}", name)
                    }
                } else {
                    format!("ref mut {}", name)
                }
            }
            Pattern::Ident(name) => {
                // Handle dotted patterns like "Color.Red" → Rust enum pattern "Color::Red"
                if let Some(dot_pos) = name.rfind('.') {
                    let type_name = &name[..dot_pos];
                    let variant = &name[dot_pos + 1..];
                    if self.emitted_types.contains(type_name)
                        || type_name == "Option"
                        || type_name == "Result"
                        || type_name == "Some"
                        || type_name == "None"
                        || type_name == "Ok"
                        || type_name == "Err"
                        || self.enum_variants.contains_key(variant)
                    {
                        format!("{}::{}", type_name, variant)
                    } else {
                        // 检测 pattern 绑定名与模块级 static/global 冲突（E0530）
                        if self.global_vars.contains_key(name.as_str())
                            || self.top_level_static_names.contains(name.as_str())
                        {
                            format!("{}_", name)
                        } else {
                            name.clone()
                        }
                    }
                } else if let Some(enum_name) = self.enum_variants.get(name.as_str()) {
                    // 裸枚举变体名（无点号）：`case Less:` → `Ordering::Less`
                    // （否则 Rust 将 Less 当作标识符绑定，元组模式 `(Less, Less)`
                    // 报 E0416 bound more than once）
                    format!("{}::{}", enum_name, name)
                } else {
                    // 检测 pattern 绑定名与模块级 static/global 冲突（E0530）
                    if self.global_vars.contains_key(name.as_str())
                        || self.top_level_static_names.contains(name.as_str())
                    {
                        format!("{}_", name)
                    } else {
                        name.clone()
                    }
                }
            }
            Pattern::Lit(lit) => {
                // Pattern literals: no .to_string() wrapper
                match lit {
                    LitKind::Int(n) => format!("{}i64", n),
                    LitKind::Str(s) => format!("\"{}\"", s.escape_default()),
                    LitKind::Bool(b) => b.to_string(),
                    _ => self.gen_lit(lit, &IrType::Any),
                }
            }
            Pattern::Tuple(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_pattern(e)).collect();
                format!("({})", elems.join(", "))
            }
            Pattern::Struct { name, fields } => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(n, p)| format!("{}: {}", n, self.gen_pattern(p)))
                    .collect();
                format!("{} {{ {} }}", name, fields.join(", "))
            }
            Pattern::Enum {
                enum_name,
                variant,
                args,
            } => {
                // 递归字段在模式中不添加 box 关键字（box_patterns 尚未稳定）
                // 由 gen_stmt(Match) 在臂体开头自动插入 let var = *var; 解引用
                if args.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    let args: Vec<String> = args.iter().map(|a| self.gen_pattern(a)).collect();
                    format!("{}::{}({})", enum_name, variant, args.join(", "))
                }
            }
            Pattern::List(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_pattern(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Pattern::Rest(name) => match name {
                Some(n) => format!("{} @ ..", n),
                None => "..".into(),
            },
            Pattern::Dict(entries) => {
                // 字典模式：Rust 无原生 HashMap 模式，由 Match 语句层
                // 生成 contains_key 守卫 + 值绑定；此处仅作占位
                let _ = entries;
                "_".into()
            }
            Pattern::Range {
                start,
                end,
                inclusive,
            } => {
                if *inclusive {
                    format!("{}i64..={}i64", start, end)
                } else {
                    format!("{}i64..{}i64", start, end)
                }
            }
        }
    }

    /// 收集模式中的所有 Ident 绑定名（用于 catch 模式参数提取等场景）
    fn collect_pattern_idents(&self, pat: &Pattern) -> Vec<String> {
        let mut out = Vec::new();
        match pat {
            Pattern::Ident(name) => out.push(name.clone()),
            Pattern::Tuple(elems) => {
                for e in elems {
                    out.extend(self.collect_pattern_idents(e));
                }
            }
            Pattern::Struct { fields, .. } => {
                for (_, p) in fields {
                    out.extend(self.collect_pattern_idents(p));
                }
            }
            Pattern::Enum { args, .. } => {
                for a in args {
                    out.extend(self.collect_pattern_idents(a));
                }
            }
            _ => {}
        }
        out
    }

    /// 收集 match 臂 `ref mut` 模式绑定名（case Some(ref mut c) → ["c"]），
    /// 供臂体内 c = c + 1 生成 *c = *c + 1 解引用赋值（E0384 修复）
    fn collect_ref_mut_bindings(&self, pat: &Pattern) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        self.collect_ref_mut_inner(pat, &mut out);
        out
    }

    fn collect_ref_mut_inner(&self, pat: &Pattern, out: &mut std::collections::HashSet<String>) {
        match pat {
            Pattern::RefMutIdent(name) => {
                out.insert(name.clone());
            }
            Pattern::Tuple(elems) | Pattern::List(elems) => {
                for e in elems {
                    self.collect_ref_mut_inner(e, out);
                }
            }
            Pattern::Dict(entries) => {
                for (_, p) in entries {
                    self.collect_ref_mut_inner(p, out);
                }
            }
            Pattern::Enum { args, .. } => {
                for a in args {
                    self.collect_ref_mut_inner(a, out);
                }
            }
            Pattern::Struct { fields, .. } => {
                for (_, p) in fields {
                    self.collect_ref_mut_inner(p, out);
                }
            }
            _ => {}
        }
    }

    /// 收集 Enum 模式中需要 Box 解引用的绑定名（用于插入 let name = *name;）
    fn collect_box_pattern_bindings(&self, pat: &Pattern) -> Vec<String> {
        let mut bindings = Vec::new();
        if let Pattern::Enum {
            enum_name,
            variant,
            args,
        } = pat
        {
            if let Some(field_types) = self
                .enum_variant_fields
                .get(&(enum_name.clone(), variant.clone()))
            {
                for (i, arg_pat) in args.iter().enumerate() {
                    if field_types
                        .get(i)
                        .map_or(false, |ty| type_refers_to(ty, enum_name))
                    {
                        bindings.extend(self.collect_pattern_idents(arg_pat));
                    }
                }
            }
        }
        bindings
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}

/// 判断表达式是否为 _KwArg（关键字参数）
fn is_kwarg_call(args: &[Expr]) -> bool {
    args.iter()
        .any(|a| matches!(&a.kind, ExprKind::StructCtor { name, .. } if name == "_KwArg"))
}

/// 判断模式是否为列表模式（[a, b, c] / [first, ..rest]）或其子模式包含列表模式
fn pattern_is_list(pat: &Pattern) -> bool {
    match pat {
        Pattern::List(_) => true,
        Pattern::Tuple(elems) => elems.iter().any(pattern_is_list),
        Pattern::Struct { fields, .. } => fields.iter().any(|(_, p)| pattern_is_list(p)),
        Pattern::Enum { args, .. } => args.iter().any(pattern_is_list),
        _ => false,
    }
}

/// 是否为消耗型（owned self）魔术方法
fn is_consuming_self(f: &FnDef) -> bool {
    matches!(f.name.as_str(), "__enter__" | "__iter__")
}

impl CodeGen {
    /// 魔法方法 → Rust 降级映射
    fn gen_magic_call(&self, kind: &MagicKind, args: &[Expr]) -> String {
        let gen_args = |a: &[Expr]| -> Vec<String> { a.iter().map(|e| self.gen_expr(e)).collect() };
        let args_s = gen_args(args);
        match kind {
            MagicKind::Call => {
                // __call__ → receiver(args...)
                if args_s.is_empty() {
                    "()".into()
                } else {
                    format!("{}({})", args_s[0], args_s[1..].join(", "))
                }
            }
            MagicKind::GetItem => {
                if args_s.len() >= 2 {
                    format!("{}[{}]", args_s[0], args_s[1])
                } else {
                    "()".into()
                }
            }
            MagicKind::SetItem => {
                if args_s.len() >= 3 {
                    format!("{}[{}] = {}", args_s[0], args_s[1], args_s[2])
                } else {
                    "()".into()
                }
            }
            MagicKind::Iter | MagicKind::IntoIter => {
                if args_s.is_empty() {
                    "().into_iter()".into()
                } else {
                    format!("{}.into_iter()", args_s[0])
                }
            }
            MagicKind::Next => {
                if args_s.is_empty() {
                    "None".into()
                } else {
                    format!("{}.next()", args_s[0])
                }
            }
            MagicKind::Display => {
                if args_s.is_empty() {
                    "\"\"".into()
                } else {
                    format!("{}.to_string()", args_s[0])
                }
            }
            MagicKind::Eq => {
                if args_s.len() >= 2 {
                    format!("{} == {}", args_s[0], args_s[1])
                } else {
                    "true".into()
                }
            }
            MagicKind::Cmp => {
                if args_s.len() >= 2 {
                    format!("{}.cmp(&{})", args_s[0], args_s[1])
                } else {
                    "std::cmp::Ordering::Equal".into()
                }
            }
            MagicKind::Drop => {
                if args_s.is_empty() {
                    "()".into()
                } else {
                    format!("drop({})", args_s[0])
                }
            }
            MagicKind::Add => {
                if args_s.len() >= 2 {
                    format!("{} + {}", args_s[0], args_s[1])
                } else {
                    args_s.first().cloned().unwrap_or_default()
                }
            }
            MagicKind::Sub => {
                if args_s.len() >= 2 {
                    format!("{} - {}", args_s[0], args_s[1])
                } else {
                    format!("-{}", args_s.first().cloned().unwrap_or_default())
                }
            }
            MagicKind::Mul => {
                if args_s.len() >= 2 {
                    format!("{} * {}", args_s[0], args_s[1])
                } else {
                    args_s.first().cloned().unwrap_or_default()
                }
            }
            MagicKind::Neg => {
                if args_s.is_empty() {
                    "0".into()
                } else {
                    format!("-{}", args_s[0])
                }
            }
            MagicKind::Not_ => {
                if args_s.is_empty() {
                    "false".into()
                } else {
                    format!("!{}", args_s[0])
                }
            }
            MagicKind::Len => {
                if args_s.is_empty() {
                    "0".into()
                } else {
                    format!("{}.len()", args_s[0])
                }
            }
            MagicKind::Rev => {
                if args_s.is_empty() {
                    "().into_iter().rev()".into()
                } else {
                    format!("{}.into_iter().rev()", args_s[0])
                }
            }
            MagicKind::SizeHint => {
                if args_s.is_empty() {
                    "(0, None)".into()
                } else {
                    format!("{}.size_hint()", args_s[0])
                }
            }
            MagicKind::IterStrategy => args_s.first().cloned().unwrap_or_else(|| "()".into()),
            MagicKind::UnpackBuildCall => args_s.first().cloned().unwrap_or_else(|| "()".into()),
        }
    }
}

/// 检测 Block 中是否包含 yield 语句
fn block_has_yield(block: &Block) -> bool {
    for stmt in &block.stmts {
        if matches!(stmt, Stmt::Yield { .. } | Stmt::YieldFrom { .. }) {
            return true;
        }
        match stmt {
            Stmt::ExprStmt { expr } => {
                if expr_has_yield(expr) {
                    return true;
                }
            }
            Stmt::Let { value, .. } => {
                if expr_has_yield(value) {
                    return true;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_has_yield(then_branch) {
                    return true;
                }
                if let Some(ref e) = else_branch {
                    if block_has_yield(e) {
                        return true;
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                if block_has_yield(body) {
                    return true;
                }
            }
            Stmt::Block { stmts } => {
                if block_has_yield(&Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                }) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// 检测 Block 中是否包含无值 return（return;）——构建块（=:/~:/*:）内
/// return; 退出构建块自身，块值应为 ()；此时尾表达式需生成 `expr;`（丢弃值），
/// 否则闭包返回类型被推断为尾值类型，与 return; 冲突（E0308）
fn block_has_bare_return(block: &Block) -> bool {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Return { value: None } => return true,
            Stmt::ExprStmt { expr } => {
                if expr_has_bare_return(expr) {
                    return true;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_has_bare_return(then_branch) {
                    return true;
                }
                if let Some(ref e) = else_branch {
                    if block_has_bare_return(e) {
                        return true;
                    }
                }
            }
            Stmt::Block { stmts } => {
                if block_has_bare_return(&Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                }) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// 检测表达式内是否包含无值 return（return;）——覆盖 IfExpr 分支中的
/// BlockExpr 块体（构建块内 `if skip: return;` 被转换为此形态）
fn expr_has_bare_return(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::BlockExpr { block } => block_has_bare_return(block),
        ExprKind::IfExpr { then, els, .. } => {
            expr_has_bare_return(then) || expr_has_bare_return(els)
        }
        _ => false,
    }
}

/// 闭包体内是否赋值外部捕获变量（iter.lz for_each `|x| total = total + x`）：
/// 若是则用借用捕获（非 move），否则 move 复制副本导致外部变量不更新
fn block_has_external_assign(block: &Block, params: &[String]) -> bool {
    for stmt in &block.stmts {
        match stmt {
            Stmt::ExprStmt { expr } => {
                if expr_has_external_assign(expr, params) {
                    return true;
                }
            }
            Stmt::Let { value, .. } => {
                if expr_has_external_assign(value, params) {
                    return true;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_has_external_assign(then_branch, params)
                    || else_branch
                        .as_ref()
                        .map_or(false, |e| block_has_external_assign(e, params))
                {
                    return true;
                }
            }
            Stmt::Block { stmts } => {
                if block_has_external_assign(
                    &Block {
                        stmts: stmts.clone(),
                        ty: IrType::Unit,
                    },
                    params,
                ) {
                    return true;
                }
            }
            Stmt::While { body, .. } => {
                if block_has_external_assign(body, params) {
                    return true;
                }
            }
            Stmt::For { body, .. } => {
                if block_has_external_assign(body, params) {
                    return true;
                }
            }
            Stmt::Assign { target, .. } => {
                // 闭包体内赋值外部变量（`total = total + x` 是语句级 Assign）：
                // 否则漏检 → 误用 move 捕获 total 副本，外部变量不更新（输出 0）
                if let ExprKind::Var(n) = &target.kind {
                    if !params.iter().any(|p| p == n) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn expr_has_external_assign(expr: &Expr, params: &[String]) -> bool {
    match &expr.kind {
        ExprKind::AssignExpr { target, value } => {
            let is_param = if let ExprKind::Var(n) = &target.kind {
                params.iter().any(|p| p == n)
            } else {
                false
            };
            if !is_param {
                return true;
            }
            expr_has_external_assign(value, params)
        }
        ExprKind::BlockExpr { block } => block_has_external_assign(block, params),
        ExprKind::IfExpr { then, els, .. } => {
            expr_has_external_assign(then, params) || expr_has_external_assign(els, params)
        }
        ExprKind::Call { callee, args, .. } => {
            expr_has_external_assign(callee, params) || args.iter().any(|a| expr_has_external_assign(a, params))
        }
        _ => false,
    }
}

fn expr_has_yield(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::BlockExpr { block } => block_has_yield(block),
        ExprKind::IfExpr { then, els, .. } => expr_has_yield(then) || expr_has_yield(els),
        ExprKind::Call { callee, args, .. } => {
            expr_has_yield(callee) || args.iter().any(expr_has_yield)
        }
        ExprKind::Lambda { body, .. } => expr_has_yield(body),
        _ => false,
    }
}

/// 检测 Block 中是否包含 await 表达式
fn block_has_await(block: &Block) -> bool {
    for stmt in &block.stmts {
        if stmt_has_await(stmt) {
            return true;
        }
    }
    false
}

fn stmt_has_await(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::ExprStmt { expr } => expr_has_await(expr),
        Stmt::Return { value: Some(expr) } => expr_has_await(expr),
        Stmt::Yield { value } => expr_has_await(value),
        Stmt::Let { value, .. } => expr_has_await(value),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            expr_has_await(cond)
                || block_has_await(then_branch)
                || else_branch.as_ref().map_or(false, block_has_await)
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
            block_has_await(body)
        }
        Stmt::Block { stmts } => block_has_await(&Block {
            stmts: stmts.clone(),
            ty: IrType::Unit,
        }),
        _ => false,
    }
}

fn expr_has_await(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::MethodCall { method, .. } if method == "await" => true,
        ExprKind::Call { callee, args, .. } => {
            expr_has_await(callee) || args.iter().any(expr_has_await)
        }
        ExprKind::BinOp { lhs, rhs, .. } => expr_has_await(lhs) || expr_has_await(rhs),
        ExprKind::BlockExpr { block } => block_has_await(block),
        ExprKind::Lambda { body, .. } => expr_has_await(body),
        _ => false,
    }
}

/// 从 _KwArg 中提取字段值（丢弃字段名，用于位置参数构造）
fn gen_kwarg_value(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            return fields
                .iter()
                .find(|(n, _)| n == "value")
                .map(|(_, v)| cg.gen_expr(v))
                .unwrap_or_default();
        }
    }
    cg.gen_expr(arg)
}

/// 转义 format! 字符串中的独立 { / }（避免被误判为占位符）
/// 已转义的 {{ 或 }} 保持不变
fn escape_format_braces(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    // 已是 {{，保留（显示字面 {）
                    chars.next();
                    out.push_str("{{");
                } else {
                    out.push_str("{{");
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    out.push_str("}}");
                } else {
                    out.push_str("}}");
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Strip `: Type` annotations from closure params (for comprehension closures)
/// "move |x: i64| { ... }" → "move |x| { ... }"
/// "move |acc: i64, x: i64| { ... }" → "move |acc, x| { ... }"（多参数逐个剥离）
fn strip_lambda_type(lambda: &str) -> String {
    // Find `|params|` region and strip each `name: Type` down to `name`
    if let Some(pipe_open) = lambda.find('|') {
        if let Some(rel_close) = lambda[pipe_open + 1..].find('|') {
            let pipe_close = pipe_open + 1 + rel_close;
            let params_part = &lambda[pipe_open + 1..pipe_close];
            // 多参数：按逗号分割，逐个剥离类型注解
            let stripped: String = params_part
                .split(',')
                .map(|p| {
                    let trimmed = p.trim();
                    if trimmed.is_empty() {
                        String::new()
                    } else if let Some(colon) = trimmed.find(':') {
                        trimmed[..colon].trim().to_string()
                    } else {
                        trimmed.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let before = &lambda[..pipe_open + 1];
            let after = &lambda[pipe_close..];
            return format!("{}{}{}", before, stripped, after);
        }
    }
    lambda.to_string()
}

/// Strip type annotations AND add `&` before each param for filter-style closures
/// "move |x: i64| { ... }" → "move |&x| { ... }"
/// "move |x| { ... }" → "move |&x| { ... }"
fn strip_lambda_type_with_ref(lambda: &str) -> String {
    let no_types = strip_lambda_type(lambda);
    // Now add `&` before each parameter name
    // Format: "move |x, y| { ... }" or "|x| { ... }"
    if let Some(pipe_open) = no_types.find('|') {
        if let Some(pipe_close) = no_types[pipe_open + 1..].find('|') {
            let params_part = &no_types[pipe_open + 1..pipe_open + 1 + pipe_close];
            let ref_params: String = params_part
                .split(',')
                .map(|p| {
                    let trimmed = p.trim();
                    if trimmed.is_empty() {
                        String::new()
                    } else {
                        format!("&{}", trimmed)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let before = &no_types[..pipe_open + 1];
            let after = &no_types[pipe_open + 1 + pipe_close..];
            return format!("{}{}{}", before, ref_params, after);
        }
    }
    no_types
}

/// 将 _KwArg { name, value } 展开为 "field: value"
fn gen_kwarg_field(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            let name_raw = fields
                .iter()
                .find(|(n, _)| n == "name")
                .and_then(|(_, v)| match &v.kind {
                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let value = fields.iter().find(|(n, _)| n == "value")
                .map(|(_, v)| {
                    let s = cg.gen_expr(v);
                    // &self 方法内构造时 move self.字段 → 需 .clone()
                    if cg.borrow_self
                        && matches!(&v.kind, ExprKind::FieldAccess { base, .. } if matches!(&base.kind, ExprKind::Var(n) if n == "self")) {
                        format!("{}.clone()", s)
                    } else if matches!(&v.kind, ExprKind::Var(_))
                        && !matches!(&v.kind, ExprKind::Var(ref n)
                            if n == "None" || n == "None_")
                        && !matches!(&v.ty, IrType::Int | IrType::F64 | IrType::Bool)
                    {
                        // 非 Copy 变量实参（String/Vec/Option 等）：struct 构造会 move，
                        // 后续再用该变量报 E0382（combo-defer-guard.lz FileHandle{path: path}）。
                        // None 无需 clone（(None).clone() 报 E0277 Option<_>: Clone）
                        format!("({}).clone()", s)
                    } else {
                        s
                    }
                })
                .unwrap_or_default();
            return format!("{}: {}", name_raw, value);
        }
    }
    cg.gen_expr(arg)
}

/// 提取 _KwArg 的字段名（用于递归字段构造 Box 判断）
fn kwarg_field_name(arg: &Expr) -> Option<String> {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
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
}

/// 检测 IrType 是否引用了指定的类型名（用于递归枚举检测）
fn type_refers_to(ty: &IrType, name: &str) -> bool {
    match ty {
        IrType::Named { path, args } => {
            if path == name {
                return true;
            }
            args.iter().any(|a| type_refers_to(a, name))
        }
        IrType::Option(inner)
        | IrType::Result { ok: inner, err: _ }
        | IrType::Ref(inner)
        | IrType::MutRef(inner) => type_refers_to(inner, name),
        IrType::Tuple(elems) => elems.iter().any(|e| type_refers_to(e, name)),
        IrType::Fn { params, ret } => {
            params.iter().any(|p| type_refers_to(p, name)) || type_refers_to(ret, name)
        }
        _ => false,
    }
}

/// 字段是否需要自动 Box：仅当字段类型**直接**是自身（`Self` / `Self?` / `Option<Self>`），
/// 才需要 Box 打破无限大小。`Vec<Self>`、`Rc<Self>`、`Box<Self>` 等已间接，无需 Box。
fn field_needs_box(ty: &IrType, name: &str) -> bool {
    match ty {
        IrType::Self_ => true,
        IrType::Named { path, .. } => path == name,
        IrType::Option(inner) => match inner.as_ref() {
            IrType::Self_ => true,
            IrType::Named { path, .. } => path == name,
            _ => false,
        },
        _ => false,
    }
}

/// 递归替换类型中的 `Self` 引用为具体类型（struct 定义内 Self → 自身类型名）。
fn replace_self(ty: &IrType, self_ty: &IrType) -> IrType {
    match ty {
        IrType::Self_ => self_ty.clone(),
        IrType::Named { path, args } => {
            let new_args: Vec<IrType> = args.iter().map(|a| replace_self(a, self_ty)).collect();
            IrType::Named {
                path: path.clone(),
                args: new_args,
            }
        }
        IrType::Option(inner) => IrType::Option(Box::new(replace_self(inner, self_ty))),
        IrType::Result { ok, err } => IrType::Result {
            ok: Box::new(replace_self(ok, self_ty)),
            err: Box::new(replace_self(err, self_ty)),
        },
        IrType::Tuple(elems) => {
            IrType::Tuple(elems.iter().map(|e| replace_self(e, self_ty)).collect())
        }
        IrType::Ref(inner) => IrType::Ref(Box::new(replace_self(inner, self_ty))),
        IrType::MutRef(inner) => IrType::MutRef(Box::new(replace_self(inner, self_ty))),
        _ => ty.clone(),
    }
}

/// 扫描块中是否存在对 const 名称的修改
fn scan_const_mutations(
    block: &Block,
    const_names: &std::collections::HashSet<String>,
    mutated: &mut std::collections::HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, is_mut, .. } => {
                if *is_mut && const_names.contains(name) {
                    mutated.insert(name.clone());
                }
            }
            Stmt::Assign { target, .. } => {
                if let ExprKind::Var(v) = &target.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                scan_const_mutations(then_branch, const_names, mutated);
                if let Some(ref e) = else_branch {
                    scan_const_mutations(e, const_names, mutated);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                scan_const_mutations(body, const_names, mutated);
            }
            Stmt::Block { stmts } => {
                let inner_block = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                scan_const_mutations(&inner_block, const_names, mutated);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    scan_const_mutations(&arm.body, const_names, mutated);
                }
            }
            Stmt::ExprStmt { expr } => {
                scan_expr_mutations(expr, const_names, mutated);
            }
            _ => {}
        }
    }
}

/// 递归扫描表达式中对 const 名称的修改（如 +=, -= 等复合赋值）
fn scan_expr_mutations(
    expr: &Expr,
    const_names: &std::collections::HashSet<String>,
    mutated: &mut std::collections::HashSet<String>,
) {
    match &expr.kind {
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_mutations(lhs, const_names, mutated);
            scan_expr_mutations(rhs, const_names, mutated);
        }
        ExprKind::StructCtor { name, fields } if name == "_Walrus" => {
            if let Some((_, bind_expr)) = fields.iter().find(|(n, _)| n == "_bind") {
                if let ExprKind::Var(v) = &bind_expr.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

/// 收集块中的局部 let 绑定名 + 闭包参数（遮蔽名）
fn collect_local_lets(block: &Block, locals: &mut std::collections::HashSet<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, .. } => {
                locals.insert(name.clone());
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_local_lets(then_branch, locals);
                if let Some(e) = else_branch {
                    collect_local_lets(e, locals);
                }
            }
            Stmt::For { var, body, .. } => {
                // 元组解构循环变量 `for (k, v) in ...`：var 形如 "(k, v)"，
                // 需把 k、v 分别收集为局部变量，否则 analyze_global_vars 把
                // 未收集的名字误判为跨函数全局变量（E0530 static mut 冲突）
                collect_for_var_bindings(var, locals);
                collect_local_lets(body, locals);
            }
            Stmt::While { body, .. } => {
                collect_local_lets(body, locals)
            }
            Stmt::WhileLet {
                pattern, body, ..
            } => {
                // while-let 模式绑定（如 Some(item) 中的 item）也是局部变量：
                // 不收集会导致 analyze_global_vars 误判为跨函数全局变量，
                // 生成 static mut item 与 for 绑定冲突（E0530，while_let.lz）
                collect_pattern_bindings(pattern, locals);
                collect_local_lets(body, locals);
            }
            Stmt::Block { stmts } => {
                let inner = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                collect_local_lets(&inner, locals);
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    // 收集 match 模式绑定名（遮蔽外部变量）
                    collect_pattern_bindings(&a.pattern, locals);
                    collect_local_lets(&a.body, locals);
                }
            }
            _ => {}
        }
    }
}

/// 收集模式中的绑定名（match 臂的 Ident/Tuple/Struct/Enum 绑定）
fn collect_pattern_bindings(pattern: &Pattern, locals: &mut std::collections::HashSet<String>) {
    match pattern {
        Pattern::Ident(name) => {
            locals.insert(name.clone());
        }
        Pattern::Tuple(ps) => {
            for p in ps {
                collect_pattern_bindings(p, locals);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                collect_pattern_bindings(p, locals);
            }
        }
        Pattern::Enum { args, .. } => {
            for p in args {
                collect_pattern_bindings(p, locals);
            }
        }
        _ => {}
    }
}

/// 收集 for 循环变量的绑定名：`for x in ...` → x；
/// 元组解构 `for (k, v) in ...` → k、v 分别收集（否则 analyze_global_vars
/// 把未收集的名字误判为跨函数全局变量，生成 static mut 与 for 绑定冲突 E0530）
fn collect_for_var_bindings(var: &str, locals: &mut std::collections::HashSet<String>) {
    let trimmed = var.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        // 元组解构：(k, v) → 分别收集
        let inner = &trimmed[1..trimmed.len() - 1];
        for part in inner.split(',') {
            let name = part.trim();
            if !name.is_empty() && name != "_" {
                locals.insert(name.to_string());
            }
        }
    } else if !trimmed.is_empty() && trimmed != "_" {
        locals.insert(trimmed.to_string());
    }
}

/// 递归收集块中引用的自由变量名（shadow 为遮蔽名集合：闭包参数）
/// 递归收集自由变量引用。in_closure=true 表示当前处于闭包作用域，
/// 此时裸赋值 `x = v` 视为局部声明（加入遮蔽集），而不是外部变量引用。
/// 用于构建块（=: → 闭包）内的赋值，避免被误提升为全局变量。
pub(crate) fn collect_var_refs(
    block: &Block,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
) {
    collect_var_refs_inner(block, shadow, refs, false);
}

fn collect_var_refs_inner(
    block: &Block,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    for stmt in &block.stmts {
        collect_stmt_var_refs(stmt, shadow, refs, in_closure);
    }
}

fn collect_stmt_var_refs(
    stmt: &Stmt,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    match stmt {
        Stmt::Let { name, value, .. } => {
            // let 声明引入新局部变量，遮蔽同名外部引用
            shadow.insert(name.clone());
            collect_expr_var_refs(value, shadow, refs, in_closure);
        }
        Stmt::Assign { target, value } => {
            // 闭包作用域内：裸赋值视为局部声明（如构建块内 a = 10）
            if in_closure {
                if let ExprKind::Var(n) = &target.kind {
                    shadow.insert(n.clone());
                }
            }
            collect_expr_var_refs(target, shadow, refs, in_closure);
            collect_expr_var_refs(value, shadow, refs, in_closure);
        }
        Stmt::ExprStmt { expr } => collect_expr_var_refs(expr, shadow, refs, in_closure),
        Stmt::Return { value } => {
            if let Some(v) = value {
                collect_expr_var_refs(v, shadow, refs, in_closure);
            }
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            collect_var_refs_inner(then_branch, shadow, refs, in_closure);
            if let Some(e) = else_branch {
                collect_var_refs_inner(e, shadow, refs, in_closure);
            }
        }
        Stmt::For {
            iter, guard, body, ..
        } => {
            collect_expr_var_refs(iter, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
            if let Some(eb) = else_body {
                collect_var_refs_inner(eb, shadow, refs, in_closure);
            }
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_expr_var_refs(expr, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
        }
        Stmt::Block { stmts } => {
            let inner = Block {
                stmts: stmts.clone(),
                ty: IrType::Unit,
            };
            collect_var_refs_inner(&inner, shadow, refs, in_closure);
        }
        Stmt::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_var_refs(scrutinee, shadow, refs, in_closure);
            for arm in arms {
                collect_var_refs_inner(&arm.body, shadow, refs, in_closure);
            }
        }
        Stmt::Raise { value } => collect_expr_var_refs(value, shadow, refs, in_closure),
        Stmt::Assert { cond, .. } => collect_expr_var_refs(cond, shadow, refs, in_closure),
        _ => {}
    }
}

fn collect_expr_var_refs(
    expr: &Expr,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    match &expr.kind {
        ExprKind::Var(name) => {
            // 被闭包参数/局部变量遮蔽的跳过
            if !shadow.contains(name.as_str()) {
                refs.push(name.clone());
            }
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_expr_var_refs(lhs, shadow, refs, in_closure);
            collect_expr_var_refs(rhs, shadow, refs, in_closure);
        }
        ExprKind::UnOp { operand, .. } => collect_expr_var_refs(operand, shadow, refs, in_closure),
        ExprKind::Call { callee, args, .. } => {
            collect_expr_var_refs(callee, shadow, refs, in_closure);
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_expr_var_refs(receiver, shadow, refs, in_closure);
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_expr_var_refs(base, shadow, refs, in_closure),
        ExprKind::IndexGet { base, key } => {
            collect_expr_var_refs(base, shadow, refs, in_closure);
            collect_expr_var_refs(key, shadow, refs, in_closure);
        }
        ExprKind::IfExpr { cond, then, els } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            collect_expr_var_refs(then, shadow, refs, in_closure);
            collect_expr_var_refs(els, shadow, refs, in_closure);
        }
        ExprKind::BlockExpr { block } => collect_var_refs_inner(block, shadow, refs, in_closure),
        ExprKind::Lambda { params, body, .. } => {
            // 闭包参数遮蔽：进入闭包体时加入遮蔽集；闭包内裸赋值视为局部声明
            let mut inner_shadow = shadow.clone();
            for p in params {
                inner_shadow.insert(p.name.clone());
            }
            collect_expr_var_refs(body, &mut inner_shadow, refs, true);
        }
        ExprKind::ListLit(elems) => {
            for e in elems {
                collect_expr_var_refs(e, shadow, refs, in_closure);
            }
        }
        ExprKind::TupleLit(elems) => {
            for e in elems {
                collect_expr_var_refs(e, shadow, refs, in_closure);
            }
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                collect_expr_var_refs(v, shadow, refs, in_closure);
            }
        }
        ExprKind::Cast { expr, .. } => collect_expr_var_refs(expr, shadow, refs, in_closure),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        _ => {}
    }
}

/// 从函数体推断全局变量的类型（查找 name = value 赋值，从 value 推断）
fn infer_global_type(block: &Block, name: &str, params: &[Param]) -> IrType {
    for p in params {
        if p.name == name {
            return p.ty.clone();
        }
    }
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name: n, value, .. } if n == name => return value.ty.clone(),
            Stmt::Assign { target, value } => {
                if let ExprKind::Var(v) = &target.kind {
                    if v == name {
                        return value.ty.clone();
                    }
                }
            }
            _ => {}
        }
    }
    IrType::Int
}

/// 将可能包含空格/特殊字符的名称转换为合法 Rust 标识符。
/// 用于测试函数名等场景（如 "string concat" → "string_concat"）。
fn sanitize_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    // Rust 标识符不能以数字开头
    if out.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// 边界感知的标识符替换：将 s 中作为**独立标识符**出现的 `from` 替换为 `to`。
/// 用于 guard 闭包变量重命名（`i % 2 == 0` → `i_owned % 2i64 == 0i64`）：
/// 无脑 replace("i", "i_owned") 会把字面量后缀 `2i64` 里的 i 也替换成
/// i_owned64（invalid suffix `i_owned64`）。
fn replace_ident_boundary(s: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find(from) {
        let before_ok = pos == 0
            || !rest[..pos]
                .chars()
                .next_back()
                .map_or(false, |c| c.is_ascii_alphanumeric() || c == '_');
        let after = &rest[pos + from.len()..];
        let after_ok = after
            .chars()
            .next()
            .map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'));
        if before_ok && after_ok {
            out.push_str(&rest[..pos]);
            out.push_str(to);
        } else {
            out.push_str(&rest[..pos + from.len()]);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_module() {
        let module = IrModule::new("test".into());
        let mut cg = CodeGen::new();
        let rust = cg.generate(&module);
        assert!(rust.contains("use std::collections"));
    }

    #[test]
    fn test_simple_fn() {
        let mut module = IrModule::new("test".into());
        module.items.push(Item::FnDef(FnDef {
            name: "hello".into(),
            generics: vec![],
            params: vec![],
            ret_ty: IrType::Unit,
            body: Block {
                stmts: vec![],
                ty: IrType::Unit,
            },
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: Span::unknown(),
        }));
        let mut cg = CodeGen::new();
        let rust = cg.generate(&module);
        assert!(rust.contains("pub fn hello()"));
    }

    #[test]
    fn test_fn_with_params() {
        let mut module = IrModule::new("test".into());
        module.items.push(Item::FnDef(FnDef {
            name: "add".into(),
            generics: vec![],
            params: vec![
                Param {
                    name: "a".into(),
                    ty: IrType::Int,
                    is_mut: false,
                    is_ref: false,
                    is_owned: false,
                    default: None,
                    variadic: false,
                },
                Param {
                    name: "b".into(),
                    ty: IrType::Int,
                    is_mut: false,
                    is_ref: false,
                    is_owned: false,
                    default: None,
                    variadic: false,
                },
            ],
            ret_ty: IrType::Int,
            body: Block {
                stmts: vec![Stmt::ExprStmt {
                    expr: Expr::new(
                        ExprKind::BinOp {
                            op: BinOpKind::Add,
                            lhs: Box::new(Expr::new(
                                ExprKind::Var("a".into()),
                                IrType::Int,
                                Span::unknown(),
                            )),
                            rhs: Box::new(Expr::new(
                                ExprKind::Var("b".into()),
                                IrType::Int,
                                Span::unknown(),
                            )),
                        },
                        IrType::Int,
                        Span::unknown(),
                    ),
                }],
                ty: IrType::Int,
            },
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: Span::unknown(),
        }));
        let mut cg = CodeGen::new();
        let rust = cg.generate(&module);
        assert!(rust.contains("pub fn add(a: i64, b: i64) -> i64"));
        assert!(rust.contains("a + b"));
    }
}
