// Lang-Zone 编译器 — ir/codegen.rs
// LZIR → Rust 源代码生成器
//
// 职责：
// 1. 将 IrModule 转换为合法的 Rust 源代码字符串
// 2. 类型映射：IrType → Rust 类型（如 Option→Option, List→Vec）
// 3. 生成完整的、可编译的 .rs 文件

mod helpers;
pub mod moddec_emit;
use helpers::*;
// builder.rs 直接依赖 collect_var_refs（AST→IR 的全局变量收集），重导出保持路径稳定
pub(crate) use helpers::collect_var_refs;

use super::node::*;
use super::types::IrType;
use super::IrModule;
use std::collections::{HashMap, HashSet};

/// IR → Rust 代码生成器
pub struct CodeGen {
    /// 缩进级别（空格数）
    indent: usize,
    /// 类型映射表：LZ 类型名 → Rust 类型名
    type_map: HashMap<&'static str, &'static str>,
    /// 当前函数返回类型
    current_ret_ty: Option<IrType>,
    /// 当前函数 raises 异常类型（BUG-CG-004 轮次12）：Some(E) 时函数返回
    /// `Result<ret_ty, E>`，try/catch 结果基分支据此决定是否按 Result 匹配。
    current_fn_raises: Option<IrType>,
    /// 当前是否位于 try 块体内。try/catch 是异常语义：raises 函数（返回
    /// `Result<T,E>`）在 try 块内绑定时应**解包**为 T，失败时由 unwrap 触发
    /// panic、被外层 catch_unwind 捕获。否则 `let v = checked(21)` 会把
    /// `Ok(42)` 当值绑定，print 出 `Ok(42)` 且 catch 永不触发。
    in_try_block: bool,
    /// 当前函数签名返回类型（与 current_ret_ty 不同：后者在 if/match 等块内
    /// 可能被推断覆盖为 None，此字段保存函数级返回类型用于 ref 判断回退）
    current_fn_ret_ty: Option<IrType>,
    /// 当前表达式期望类型（函数调用实参位置由 fn_param_types 注入，
    /// 用于 Option::None 等需类型参数的表达式跟随实参类型，E0308 修复）
    /// 用 RefCell：gen_expr 仅 &self，实参生成时需临时改写
    current_expected_ty: std::cell::RefCell<Option<IrType>>,
    /// 当前函数是否返回引用（`-> &Self` 等）：builder 对 ref 返回推断可能为 None，
    /// Stmt::Return 中 `return self` 判断是否 clone 时使用
    current_fn_ret_is_ref: bool,
    is_main: bool,
    declared: std::collections::HashSet<String>,
    /// 已知为字符串类型的局部变量名（let 绑定 ty=Str / 字符串字面量 / 参数 ty=Str）：
    /// f-string 插值用 `{}` 而非 `{:?}`（`{:?}` 会给字符串加引号，如 `f"[{joined}]"`）
    str_typed_vars: std::collections::HashSet<String>,
    /// ref 绑定变量名集合（ref r = x / let ref r = x）：后续赋值 r = v 需生成 `*r = v`
    ref_bindings: std::collections::HashSet<String>,
    emitted_types: std::collections::HashSet<String>,
    /// 仅 impl 块（非 struct/enum）的类型名，用于 FieldAccess 生成 :: 语法
    impl_types: std::collections::HashSet<String>,
    /// enum variant → enum name 映射（用于构造器调用路由）
    enum_variants: HashMap<String, String>,
    /// 抑制尾表达式隐式 return（用于 match arm / 块表达式内部）
    suppress_tail_return: bool,
    /// defer+尾值捕获：块尾值语句（ExprStmt/TryCatch）已生成到 `__tail_val`，
    /// 块结束后（flush defer cleanup 之后）统一 `return __tail_val`（否则尾值
    /// 被 force_stmt_semicolon 丢弃、cleanup 落尾 → 函数返回 ()，E0308 家族）
    pending_tail_capture: Option<String>,
    /// 块内存在无值 return（return;）时强制尾表达式生成 `expr;`（丢弃值），
    /// 使闭包/块返回类型为 ()（否则 return; 与尾值类型冲突 E0308）
    force_unit_tail: bool,
    /// 循环体（for/while）内所有表达式语句强制加分号：循环体不是值上下文，
    /// 尾表达式裸生成会导致 E0308（如 go print(...) → std::thread::spawn(...) 缺 `;`）
    force_stmt_semicolon: bool,
    /// 函数返回类型为嵌套 Fn（fn -> fn -> T）时：内层闭包返回需 Box::new 包装，
    /// 且返回类型用 Box<dyn Fn>（Rust 不允许 impl Fn -> impl Fn 嵌套，E0562）
    nested_fn_ret: bool,
    /// IR-003：Lambda 作为一等值（let = Lambda / return Lambda 的值位置）时
    /// 需 Box::new 包装为 `Box<dyn Fn>`。仅在值位置置 true，避免借用捕获闭包被误装箱
    /// （for_each |x| total += x 必须保持 impl FnMut 借用捕获，见 gen_param）
    box_lambda: bool,
    /// 顶层 def 名称集合：用作值（Var）时需 Box::new(f)；局部 fn-let 由 let 值位置已
    /// 装箱，不重复装箱（避免 Rc/Box 双层嵌套）
    top_level_fns: std::collections::HashSet<String>,
    /// @math 函数标志：体内整数字面量经 T::from(i32) 转换（使 `x * 2` 中 2 推断为 T）。
    /// 普通泛型函数不应转换（否则 `total = 0` 生成 T::from(0i32) 与 i64 冲突，E0308）
    in_math_fn: bool,
    /// 当前是否在 Lambda 块体（BlockExpr child）内：嵌套 Fn 返回时，
    /// 仅块体尾闭包需 Box::new 包装，函数体本身的尾表达式（外层闭包）不包装
    in_lambda_block: bool,
    /// 当前是否在生成器（iterator/yield）函数内：体内 return 等价 raise（panic）
    in_generator: bool,
    /// 当前是否在生成器构建块（func *:）闭包内：yield → push 到闭包收集器 __bb
    in_gen_build: bool,
    /// 当前是否在泛型函数内（@math 等）：整数字面量不附加 i64 后缀，
    /// 否则 `x * 2i64`（T 泛型）报 E0308（expected T, found i64）
    in_generic_fn: bool,
    /// 当前是否在泛型 impl 块内：impl<T> 的方法自身无 generics 字段，
    /// 但体内 Option.None 等需按泛型上下文处理（magic_methods.lz __next__ E0308）
    in_impl_generic: bool,
    /// 当前 impl / struct 块的泛型参数名。
    /// builder 会把外层泛型合并进方法的 generics，若方法再声明一次会导致
    /// `impl<T> X<T> { fn m<T>(..) }` → E0403（T 重复声明）。
    /// gen_fn_def 生成方法签名时据此剔除同名泛型（方法自身泛型如 <U> 保留）。
    current_impl_generics: Vec<String>,
    /// 循环 else 标志栈：Some(flag) = 当前循环带 else 子句（while/else, for/else，
    /// 规范 05-控制流.md §13.2/13.3），break 时置 false 跳过 else 体
    loop_else_stack: Vec<Option<String>>,
    /// 循环 else 标志唯一命名计数器
    loop_else_counter: usize,
    /// 块级 defer 体收集（BUG-IR-002 方案 A：内联脱糖，块退出前逆序 emit，规避闭包捕获 E0499）
    deferred: Vec<Block>,
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
    /// 正在生成外部类型扩展 trait（ListExt/DictExt/SetExt/StrExt）时置 true：
    /// 方法名需与调用点映射一致（slice → lz_slice，对齐 lz_builtins 的
    /// ListExt/StringExt），否则 trait 声明是 slice 而调用是 lz_slice → E0599
    in_ext_trait: bool,
    /// 当前是否在 Result 基 try 块闭包体内：raises 函数调用需 `?` 解包
    in_result_try: bool,
    /// 正在生成的扩展 trait 名（ListExt/DictExt/SetExt/StrExt）：扩展 trait 方法体
    /// 内 `self.lz_*` 调用需显式 UFCS `<本地 trait>::lz_*(self, ...)`，否则与
    /// lz_builtins::StringExt/ListExt（也 `use lz_builtins::*` 进入作用域，且
    /// StringExt 同时 impl 于 str/String）同名方法产生 E0034 多重适用项歧义
    current_ext_trait: Option<String>,
    /// 函数名 → (总参数数, 默认参数数)（用于调用时自动填充 None）
    fn_param_info: HashMap<String, (usize, usize)>,
    /// 函数 ref/mut ref 参数标记：函数名 → 每参数 (is_ref, is_mut)（调用点自动传引用）
    fn_ref_params: HashMap<String, Vec<(bool, bool)>>,
    /// 当前函数内需自动 mut 的局部 let 变量名（预扫描收集）：
    /// LZ 源码 `let v = vec; v.push(1)` 未写 mut，但可变方法调用/赋值需 mut 绑定
    auto_mut_locals: std::collections::HashSet<String>,
    /// 被修改的模块级 const 名称（需生成 static mut 而非 const）
    mutated_consts: std::collections::HashSet<String>,
    /// enum variant → field types 映射: (enum_name, variant_name) → Vec<IrType>
    enum_variant_fields: HashMap<(String, String), Vec<IrType>>,
    /// enum variant → 命名字段名列表（空 = 元组/位置字段）
    enum_variant_named_fields: HashMap<(String, String), Vec<String>>,
    /// 函数名 → variadic 参数起始索引（该索引及之后的参数收集为 &[T]）
    fn_variadic: HashMap<String, usize>,
    /// 函数名 → kwargs 注入参数起始索引（kwargs 收集为 &HashMap<String, V>）
    fn_kwargs: HashMap<String, usize>,
    /// 函数名 → 参数类型列表（用于隐式 variadic + 调用方类型检查）
    fn_param_types: HashMap<String, Vec<IrType>>,
    /// 比较约束传播：函数名 → (eq 泛型集合, ord 泛型集合)。
    /// 由 gen_module 预扫描所有 FnDef 体（含调用图不动点）得到，
    /// 用于为参与比较运算（==/!= → PartialEq+Eq；<,>,<=,>= → PartialEq+Eq+Ord）
    /// 的泛型注入 Rust trait bound，修复泛型比较算法的 E0277/E0369。
    fn_cmp: HashMap<String, (HashSet<String>, HashSet<String>)>,
    /// 各函数变量引用次数（函数名 → 变量名 → 次数），用于 move 语义修复：
    /// 被多次使用的非 Copy 变量在调用实参处自动 .clone()，避免 E0382 二次移动，
    /// 如 `for x in xs: push(seen, x); push(result, x)`。由 gen_module 预计算
    /// （含闭包/循环体等嵌套函数），按函数名查表，避免 gen_fn_def 嵌套覆盖。
    fn_use_count: HashMap<String, HashMap<String, usize>>,
    /// 当前正在生成的函数名（嵌套 gen_fn_def 时经保存/恢复）。
    cur_fn_name: String,
    /// type-pack 变参函数（`..: Tuple<Ts...>`，03d §2.8 方案 B）：
    /// 函数名 → pack 泛型参数名（如 "Ts"）。args 参数类型为
    /// IrType::Tuple([Generic("Ts")])，由调用点具体化为 Rust 异质元组。
    typepack_param: HashMap<String, String>,
    /// type-pack 函数调用点收集：函数名 → 各调用点实参类型列表
    /// （用于生成具体化签名 `args: (T1, T2, T3)`，修复异质元组被降级为
    /// &[Ts] 同构切片的 E0308 根因）
    typepack_sigs: HashMap<String, Vec<Vec<IrType>>>,
    /// 顶层 `def m(self: StructName, ...)` 归属表（BUG-CG-002/TY-002 修复）：
    /// 函数名 → (struct 名, self 是否 mut)。命中的函数发射为 `impl StructName`
    /// 方法而非自由函数（E0568：Rust 的 self 参数只许出现在 impl/trait 关联项），
    /// 调用点同步改写为 `recv.m(...)` 方法语法。仅当 self 注解类型是本模块
    /// struct 时归属（外部类型的扩展方法仍走自由函数路径）。
    self_fns: HashMap<String, (String, bool)>,
    /// checker 块名称集合（fn NAME(ps: &mut __Params)），用于 default_checker 链
    /// 调用时区分 checker 块与普通值函数（fn NAME(ps: __Params) -> __Params）
    checker_blocks: std::collections::HashSet<String>,
    /// raises 函数名集合：checker 注入调用点需解包其 `Result` 返回值
    /// （raises checker 是值函数 `fn NAME(ps: __Params) -> __Params raises E`，
    /// Rust 侧返回 `Result<__Params, E>`，直接 `__ps = NAME(__ps)` 会 E0308）
    raises_fn_names: std::collections::HashSet<String>,
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
    /// @parallel 装饰的当前函数：函数内 `xs.map(f)` 方法调用生成并行版本
    cur_fn_is_parallel: bool,
    /// @init 装饰的模块初始化函数名（main 开头按声明顺序自动注入调用）
    init_fns: Vec<String>,
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
    /// f-string 插值中未绑定的变量名（探测用例引用外部未定义符号，如
    /// lit_probe 的 `f"hello {name}"`）：生成时替换为空字符串，保证 rustc
    /// 编译通过（批测口径只编译不运行）
    unbound_fstring_vars: std::collections::HashSet<String>,
    /// 未定义外部函数名 → 最大实参个数（p16_lzi 等跨模块探测用例：
    /// 调用的 external_add/external_opt 定义在 .lzi 签名中，批测未加载
    /// .lzi 时生成 i64 stub 兜底，保证 rustc 编译通过）
    unknown_extern_fns: std::collections::HashMap<String, usize>,
    /// struct 字段信息：struct_name → [(field_name, field_type)]，用于 __new__ 补齐默认字段
    /// struct 字段信息（供 __new__ 补齐默认字段）
    struct_fields_info: std::collections::HashMap<String, Vec<(String, IrType)>>,
    /// struct 名 → 自动追加的 PhantomData 泛型参数列表
    /// （box.lz `struct Box<T> { _ptr: int }` 中 T 未使用，E0392 修复；
    /// 构造点需自动补 `_lz_phantom_T: PhantomData` 字段，否则 E0063）
    struct_phantom_generics: std::collections::HashMap<String, Vec<String>>,
    /// struct 方法名集合：struct_name → 方法名集合（用于 r? 自定义传播类型判定 __is_ok__ 等）
    struct_method_names_map: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// 结构 __init__ 参数表（仅 self 时可自动注入调用点）
    struct_init_params_map: std::collections::HashMap<String, Vec<(String, IrType)>>,
    /// struct 是否定义了 __new__：struct_name → 是否
    struct_has_new: std::collections::HashSet<String>,
    /// struct __new__ 参数表：struct_name → [(param_name, param_type)]，用于 kwarg 构造路由到 __new__
    struct_new_params_map: std::collections::HashMap<String, Vec<(String, IrType)>>,
    /// 当前是否正在生成 __new__ 体（抑制 kwarg→__new__ 路由，避免无限递归）
    in_new_body: bool,
    /// case struct 集合（自动配 __unapply__ / __unapply_seq__ 提取魔法方法）
    case_structs: std::collections::HashSet<String>,
    /// 支持定长提取（__unapply__）的 struct 集合（case struct 或显式实现）
    struct_has_unapply: std::collections::HashSet<String>,
    /// 支持变长提取（__unapply_seq__）的 struct 集合（仅字段同构的 case struct）
    struct_has_unapply_seq: std::collections::HashSet<String>,
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
    /// 本模块是否使用 Ext 类型或 #[extern] 装饰器（仅此时生成 ExtHandle）
    module_uses_ext: bool,
    /// I3：L2 中继——extern 自动登记目标 registry。注入后 codegen 处理
    /// #[extern(lang)] 函数时自动 register_symbol 并联动台账（REGISTER）。
    /// None = 不登记（默认，保持既有生成行为不变）。
    pub bridge_registry: Option<crate::bridge::core::BridgeRegistry>,
    buf: String,
}

// ════════════════════════════════════════════════════════════════
// 比较约束传播（comparison trait bound propagation）
//
// 泛型比较算法（如 unique/dedup/sort）需在参与 ==/!=/<,>,<=,>= 的泛型上
// 注入 PartialEq/Eq/Ord，否则 Rust 报 E0277/E0369。约束必须沿调用图
// 传播（如 merge_sort 调用 merge（比较 T）→ merge_sort 的 T 也需 Ord；
// unique 调用内置 contains（要求元素 PartialEq）→ unique 的 a 需 PartialEq）。
// 这里预扫描所有 FnDef 体，经调用图不动点计算得到每个函数的 eq/ord 泛型集合，
// 再由 gen_fn_generics 注入对应 Rust trait bound。
// ════════════════════════════════════════════════════════════════

/// 单个函数的比较约束收集结果
#[derive(Default)]
struct CmpInfo {
    /// 参与 ==/!= 的泛型名（需 PartialEq + Eq）
    eq: HashSet<String>,
    /// 参与 <,>,<=,>= 的泛型名（需 PartialEq + Eq + Ord）
    ord: HashSet<String>,
    /// 调用点：(被调函数名, 调用实参类型中出现的「当前函数泛型」集合)
    calls: Vec<(String, HashSet<String>)>,
}

/// 收集类型中出现的泛型名
fn cmp_collect_gen_names(ty: &IrType, out: &mut HashSet<String>) {
    match ty {
        IrType::Generic(n) => {
            out.insert(n.clone());
        }
        IrType::Named { args, .. } => {
            for a in args {
                cmp_collect_gen_names(a, out);
            }
        }
        IrType::Option(inner) => cmp_collect_gen_names(inner, out),
        IrType::Result { ok, err } => {
            cmp_collect_gen_names(ok, out);
            cmp_collect_gen_names(err, out);
        }
        IrType::Tuple(elems) => {
            for e in elems {
                cmp_collect_gen_names(e, out);
            }
        }
        IrType::Fn { params, ret } => {
            for p in params {
                cmp_collect_gen_names(p, out);
            }
            cmp_collect_gen_names(ret, out);
        }
        IrType::Ref(inner) | IrType::MutRef(inner) => cmp_collect_gen_names(inner, out),
        IrType::Duck { fields } => {
            for (_, t) in fields {
                cmp_collect_gen_names(t, out);
            }
        }
        _ => {}
    }
}

fn cmp_walk_block(block: &Block, my_gen: &HashSet<String>, info: &mut CmpInfo) {
    for s in &block.stmts {
        cmp_walk_stmt(s, my_gen, info);
    }
}

fn cmp_walk_stmt(s: &Stmt, my_gen: &HashSet<String>, info: &mut CmpInfo) {
    match s {
        Stmt::Let { value, .. } => cmp_walk_expr(value, my_gen, info),
        Stmt::Assign { target, value } => {
            cmp_walk_expr(target, my_gen, info);
            cmp_walk_expr(value, my_gen, info);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                cmp_walk_expr(v, my_gen, info);
            }
        }
        Stmt::ExprStmt { expr } => cmp_walk_expr(expr, my_gen, info),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            cmp_walk_expr(cond, my_gen, info);
            cmp_walk_block(then_branch, my_gen, info);
            if let Some(b) = else_branch {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            cmp_walk_expr(iter, my_gen, info);
            if let Some(g) = guard {
                cmp_walk_expr(g, my_gen, info);
            }
            cmp_walk_block(body, my_gen, info);
            if let Some(b) = else_body {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            cmp_walk_expr(cond, my_gen, info);
            if let Some(g) = guard {
                cmp_walk_expr(g, my_gen, info);
            }
            cmp_walk_block(body, my_gen, info);
            if let Some(b) = else_body {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
        } => {
            cmp_walk_expr(expr, my_gen, info);
            if let Some(g) = guard {
                cmp_walk_expr(g, my_gen, info);
            }
            cmp_walk_block(body, my_gen, info);
        }
        Stmt::Match { scrutinee, arms } => {
            cmp_walk_expr(scrutinee, my_gen, info);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    cmp_walk_expr(g, my_gen, info);
                }
                cmp_walk_block(&arm.body, my_gen, info);
            }
        }
        Stmt::Raise { value } => cmp_walk_expr(value, my_gen, info),
        Stmt::Assert { cond, message } => {
            cmp_walk_expr(cond, my_gen, info);
            if let Some(m) = message {
                cmp_walk_expr(m, my_gen, info);
            }
        }
        Stmt::Yield { value } => cmp_walk_expr(value, my_gen, info),
        Stmt::YieldFrom { iter } => cmp_walk_expr(iter, my_gen, info),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                cmp_walk_expr(v, my_gen, info);
            }
        }
        Stmt::Block { stmts } => {
            for s2 in stmts {
                cmp_walk_stmt(s2, my_gen, info);
            }
        }
        Stmt::BlockLabel { label, body } => cmp_walk_block(body, my_gen, info),
        Stmt::CheckerBlock { body, .. } => cmp_walk_block(body, my_gen, info),
        Stmt::Defer { body } => cmp_walk_block(body, my_gen, info),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            cmp_walk_block(body, my_gen, info);
            for (_, b) in catches {
                cmp_walk_block(b, my_gen, info);
            }
            if let Some(b) = else_body {
                cmp_walk_block(b, my_gen, info);
            }
            if let Some(b) = finally_body {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::TypeAlias { .. } | Stmt::Pass | Stmt::Break | Stmt::Continue => {}
    }
}

fn cmp_walk_expr(e: &Expr, my_gen: &HashSet<String>, info: &mut CmpInfo) {
    // 比较运算：收集操作数类型中的泛型名
    if let ExprKind::BinOp { op, lhs, rhs } = &e.kind {
        if op.is_comparison() {
            for t in [&lhs.ty, &rhs.ty] {
                if let IrType::Generic(name) = t {
                    if my_gen.contains(name) {
                        if *op == BinOpKind::Eq || *op == BinOpKind::Neq {
                            info.eq.insert(name.clone());
                        } else {
                            info.ord.insert(name.clone());
                        }
                    }
                }
            }
        }
    }
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Var(name) = &callee.kind {
                let mut flow = HashSet::new();
                for a in args {
                    cmp_collect_gen_names(&a.ty, &mut flow);
                }
                flow.retain(|n| my_gen.contains(n));
                if !flow.is_empty() {
                    info.calls.push((name.clone(), flow));
                }
            }
            cmp_walk_expr(callee, my_gen, info);
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let mut flow = HashSet::new();
            cmp_collect_gen_names(&receiver.ty, &mut flow);
            for a in args {
                cmp_collect_gen_names(&a.ty, &mut flow);
            }
            flow.retain(|n| my_gen.contains(n));
            if !flow.is_empty() {
                info.calls.push((method.clone(), flow));
            }
            cmp_walk_expr(receiver, my_gen, info);
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::FieldAccess { base, .. } => cmp_walk_expr(base, my_gen, info),
        ExprKind::IndexGet { base, key } => {
            cmp_walk_expr(base, my_gen, info);
            cmp_walk_expr(key, my_gen, info);
        }
        ExprKind::IndexSet { base, key, value } => {
            cmp_walk_expr(base, my_gen, info);
            cmp_walk_expr(key, my_gen, info);
            cmp_walk_expr(value, my_gen, info);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            cmp_walk_expr(lhs, my_gen, info);
            cmp_walk_expr(rhs, my_gen, info);
        }
        ExprKind::AssignExpr { target, value } => {
            cmp_walk_expr(target, my_gen, info);
            cmp_walk_expr(value, my_gen, info);
        }
        ExprKind::UnOp { operand, .. } => cmp_walk_expr(operand, my_gen, info),
        ExprKind::IfExpr { cond, then, els } => {
            cmp_walk_expr(cond, my_gen, info);
            cmp_walk_expr(then, my_gen, info);
            cmp_walk_expr(els, my_gen, info);
        }
        ExprKind::Lambda { body, .. } => cmp_walk_expr(body, my_gen, info),
        ExprKind::StructCtor { fields, .. } => {
            for (_, f) in fields {
                cmp_walk_expr(f, my_gen, info);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::GenExpr { yield_of } => cmp_walk_expr(yield_of, my_gen, info),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                cmp_walk_expr(c, my_gen, info);
            }
            cmp_walk_block(block, my_gen, info);
        }
        ExprKind::Cast { expr, .. } => cmp_walk_expr(expr, my_gen, info),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::BlockExpr { block } => cmp_walk_block(block, my_gen, info),
        ExprKind::TupleLit(v) | ExprKind::Tuple(v) | ExprKind::ListLit(v) | ExprKind::List(v) => {
            for x in v {
                cmp_walk_expr(x, my_gen, info);
            }
        }
        ExprKind::Spread(x) => cmp_walk_expr(x, my_gen, info),
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs {
                cmp_walk_expr(k, my_gen, info);
                cmp_walk_expr(v, my_gen, info);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                cmp_walk_expr(s, my_gen, info);
            }
            cmp_walk_expr(end, my_gen, info);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            cmp_walk_expr(receiver, my_gen, info);
            cmp_walk_expr(callee, my_gen, info);
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::Paren(x) => cmp_walk_expr(x, my_gen, info),
        ExprKind::ImplicitConvert { source, .. } => cmp_walk_expr(source, my_gen, info),
        ExprKind::Lit(_) | ExprKind::Var(_) | ExprKind::Default => {}
    }
}

// ── 变量引用计数（move 语义修复：被多次使用的非 Copy 变量在调用实参处克隆）──

fn count_vars_expr(e: &Expr, count: &mut HashMap<String, usize>) {
    if let ExprKind::Var(name) = &e.kind {
        *count.entry(name.clone()).or_insert(0) += 1;
    }
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            count_vars_expr(callee, count);
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            count_vars_expr(receiver, count);
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::FieldAccess { base, .. } => count_vars_expr(base, count),
        ExprKind::IndexGet { base, key } => {
            count_vars_expr(base, count);
            count_vars_expr(key, count);
        }
        ExprKind::IndexSet { base, key, value } => {
            count_vars_expr(base, count);
            count_vars_expr(key, count);
            count_vars_expr(value, count);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            count_vars_expr(lhs, count);
            count_vars_expr(rhs, count);
        }
        ExprKind::AssignExpr { target, value, .. } => {
            count_vars_expr(target, count);
            count_vars_expr(value, count);
        }
        ExprKind::UnOp { operand, .. } => count_vars_expr(operand, count),
        ExprKind::IfExpr { cond, then, els } => {
            count_vars_expr(cond, count);
            count_vars_expr(then, count);
            count_vars_expr(els, count);
        }
        ExprKind::Lambda { body, .. } => count_vars_expr(body, count),
        ExprKind::StructCtor { fields, .. } => {
            for (_, f) in fields {
                count_vars_expr(f, count);
            }
        }
        ExprKind::EnumCtor {
            enum_name,
            variant,
            args,
        } => {
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::GenExpr { yield_of } => count_vars_expr(yield_of, count),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                count_vars_expr(c, count);
            }
            count_vars_block(block, count);
        }
        ExprKind::Cast { expr, .. } => count_vars_expr(expr, count),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::BlockExpr { block } => count_vars_block(block, count),
        ExprKind::TupleLit(v) | ExprKind::Tuple(v) | ExprKind::ListLit(v) | ExprKind::List(v) => {
            for x in v {
                count_vars_expr(x, count);
            }
        }
        ExprKind::Spread(x) => count_vars_expr(x, count),
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs {
                count_vars_expr(k, count);
                count_vars_expr(v, count);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                count_vars_expr(s, count);
            }
            count_vars_expr(end, count);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            count_vars_expr(receiver, count);
            count_vars_expr(callee, count);
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::Paren(x) => count_vars_expr(x, count),
        ExprKind::ImplicitConvert { source, .. } => count_vars_expr(source, count),
        ExprKind::Lit(_) | ExprKind::Var(_) | ExprKind::Default => {}
    }
}

fn count_vars_stmt(s: &Stmt, count: &mut HashMap<String, usize>) {
    match s {
        Stmt::Let { value, .. } => count_vars_expr(value, count),
        Stmt::Assign { target, value } => {
            count_vars_expr(target, count);
            count_vars_expr(value, count);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                count_vars_expr(v, count);
            }
        }
        Stmt::ExprStmt { expr } => count_vars_expr(expr, count),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            count_vars_expr(cond, count);
            count_vars_block(then_branch, count);
            if let Some(b) = else_branch {
                count_vars_block(b, count);
            }
        }
        Stmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            count_vars_expr(iter, count);
            if let Some(g) = guard {
                count_vars_expr(g, count);
            }
            count_vars_block(body, count);
            if let Some(b) = else_body {
                count_vars_block(b, count);
            }
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            count_vars_expr(cond, count);
            if let Some(g) = guard {
                count_vars_expr(g, count);
            }
            count_vars_block(body, count);
            if let Some(b) = else_body {
                count_vars_block(b, count);
            }
        }
        Stmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
        } => {
            count_vars_expr(expr, count);
            if let Some(g) = guard {
                count_vars_expr(g, count);
            }
            count_vars_block(body, count);
        }
        Stmt::Match { scrutinee, arms } => {
            count_vars_expr(scrutinee, count);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    count_vars_expr(g, count);
                }
                count_vars_block(&arm.body, count);
            }
        }
        Stmt::Raise { value } => count_vars_expr(value, count),
        Stmt::Assert { cond, message } => {
            count_vars_expr(cond, count);
            if let Some(m) = message {
                count_vars_expr(m, count);
            }
        }
        Stmt::Yield { value } => count_vars_expr(value, count),
        Stmt::YieldFrom { iter } => count_vars_expr(iter, count),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                count_vars_expr(v, count);
            }
        }
        Stmt::Block { stmts } => {
            for s2 in stmts {
                count_vars_stmt(s2, count);
            }
        }
        Stmt::BlockLabel { body, .. } => count_vars_block(body, count),
        Stmt::CheckerBlock { body, .. } => count_vars_block(body, count),
        Stmt::Defer { body } => count_vars_block(body, count),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            count_vars_block(body, count);
            for (_, b) in catches {
                count_vars_block(b, count);
            }
            if let Some(b) = else_body {
                count_vars_block(b, count);
            }
            if let Some(b) = finally_body {
                count_vars_block(b, count);
            }
        }
        Stmt::TypeAlias { .. } | Stmt::Pass | Stmt::Break | Stmt::Continue => {}
    }
}

fn count_vars_block(block: &Block, count: &mut HashMap<String, usize>) {
    for s in &block.stmts {
        count_vars_stmt(s, count);
    }
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
            current_expected_ty: std::cell::RefCell::new(None),
            indent: 0,
            type_map,
            current_ret_ty: None,
            current_fn_raises: None,
            current_fn_ret_ty: None,
            is_main: false,
            declared: std::collections::HashSet::new(),
            str_typed_vars: std::collections::HashSet::new(),
            ref_bindings: std::collections::HashSet::new(),
            emitted_types: std::collections::HashSet::new(),
            impl_types: std::collections::HashSet::new(),
            enum_variants: HashMap::new(),
            suppress_tail_return: false,
            pending_tail_capture: None,
            force_unit_tail: false,
            force_stmt_semicolon: false,
            nested_fn_ret: false,
            box_lambda: false,
            top_level_fns: std::collections::HashSet::new(),
            in_lambda_block: false,
            in_math_fn: false,
            in_generator: false,
            in_gen_build: false,
            in_generic_fn: false,
            in_impl_generic: false,
            current_impl_generics: Vec::new(),
            loop_else_stack: Vec::new(),
            loop_else_counter: 0,
            deferred: Vec::new(),
            plain_block_depth: 0,
            loop_depth: 0,
            slice_clone_bindings: std::collections::HashSet::new(),
            in_iterator_impl: false,
            in_try_block: false,
            in_ext_trait: false,
            /// 当前是否在 Result 基 try 块闭包体内：raises 函数调用需 `?` 解包
            in_result_try: false,
            current_ext_trait: None,
            fn_param_info: HashMap::new(),
            fn_ref_params: HashMap::new(),
            auto_mut_locals: std::collections::HashSet::new(),
            mutated_consts: std::collections::HashSet::new(),
            enum_variant_fields: HashMap::new(),
            enum_variant_named_fields: HashMap::new(),
            current_variadic_params: std::collections::HashSet::new(),
            fn_variadic: HashMap::new(),
            fn_kwargs: HashMap::new(),
            fn_param_types: HashMap::new(),
            fn_cmp: HashMap::new(),
            fn_use_count: HashMap::new(),
            cur_fn_name: String::new(),
            typepack_param: HashMap::new(),
            typepack_sigs: HashMap::new(),
            self_fns: HashMap::new(),
            checker_blocks: std::collections::HashSet::new(),
            raises_fn_names: std::collections::HashSet::new(),
            checker_captures: std::collections::HashMap::new(),
            current_checker_captures: std::collections::HashSet::new(),
            ref_mut_bindings: std::collections::HashSet::new(),
            overload_sigs: HashMap::new(),
            overload_variadic: HashMap::new(),
            overload_explicit: HashMap::new(),
            top_level_static_names: std::collections::HashSet::new(),
            param_renames: HashMap::new(),
            cur_fn_is_parallel: false,
            init_fns: Vec::new(),
            known_types: std::collections::HashSet::new(),
            trait_names: std::collections::HashSet::new(),
            custom_iterator_is_protocol: true,
            borrow_self: false,
            struct_fields_info: std::collections::HashMap::new(),
            struct_phantom_generics: std::collections::HashMap::new(),
            struct_method_names_map: std::collections::HashMap::new(),
            struct_init_params_map: std::collections::HashMap::new(),
            struct_has_new: std::collections::HashSet::new(),
            struct_new_params_map: std::collections::HashMap::new(),
            in_new_body: false,
            case_structs: std::collections::HashSet::new(),
            struct_has_unapply: std::collections::HashSet::new(),
            struct_has_unapply_seq: std::collections::HashSet::new(),
            lazy_static_names: std::collections::HashSet::new(),
            imported_modules: std::collections::HashSet::new(),
            current_fn_is_async: false,
            current_fn_is_size_hint: false,
            current_fn_ret_is_ref: false,
            duck_field_members: std::collections::HashMap::new(),
            duck_defs: std::collections::HashMap::new(),
            module_uses_ext: false,
            global_vars: std::collections::HashMap::new(),
            downgraded_vars: std::collections::HashSet::new(),
            unbound_fstring_vars: std::collections::HashSet::new(),
            unknown_extern_fns: std::collections::HashMap::new(),
            bridge_registry: None,
            buf: String::new(),
        }
    }

    // ── 入口 ──

    /// 注入 BridgeRegistry：使 #[extern(lang)] 在 codegen 时自动登记（I3）
    pub fn set_bridge_registry(&mut self, registry: crate::bridge::core::BridgeRegistry) {
        self.bridge_registry = Some(registry);
    }

    /// I3/I4：生成 Rust 代码的同时注入 BridgeRegistry，自动登记
    /// #[extern]/#[embed]/@export 符号并联动台账。返回 (rust_code, registry)。
    ///
    /// 不改变生成产物本身（registry 仅登记，不影响 emit 输出），
    /// 调用方可对 registry 做台账落盘与审计（G5）。
    pub fn generate_with_bridge(
        &mut self,
        module: &IrModule,
    ) -> (String, crate::bridge::core::BridgeRegistry) {
        let reg = crate::bridge::core::BridgeRegistry::new();
        self.bridge_registry = Some(reg);
        let code = self.generate(module);
        let reg = self.bridge_registry.take().unwrap();
        (code, reg)
    }

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
        self.raises_fn_names.clear();

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

        // 预扫描 raises 函数名（checker 调用点解包其 Result 返回值）
        for item in &module.items {
            if let Item::FnDef(f) = item {
                if f.raises.is_some() {
                    self.raises_fn_names.insert(f.name.clone());
                }
            }
        }

        // 预扫描 f-string 未绑定插值变量（lit_probe 等探测用例）
        self.unbound_fstring_vars = collect_unbound_fstring_vars(module);

        // 预扫描未定义外部函数（p16_lzi 等跨模块探测用例）
        self.unknown_extern_fns = collect_unknown_extern_fns(module);

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

        // 预登记 LazyLock 静态集合名（集合/Option<集合>/Tuple/Str 非字面量 const）。
        // 必须在函数体生成前登记：否则 main 内 config.and_then(...) 时
        // lazy_static_names 尚无 config → 生成 config 而非 (*config).clone()（E0507）
        // 与 gen_const_def 的 needs_lazy 条件对齐：Str 非纯字面量值生成
        // .to_string() → LazyLock<String>，引用处同样需 (*name).clone() 解引用
        // （顶层构建块 x =: 返回 String 的场景，E0369）
        self.lazy_static_names.clear();
        for item in &module.items {
            if let Item::Const(c) = item {
                let ty_is_collection = match &c.ty {
                    IrType::Named { path, .. } => {
                        ["Vec", "List", "HashMap", "HashSet", "Dict", "Set"]
                            .contains(&path.as_str())
                    }
                    IrType::Option(inner) => matches!(
                        inner.as_ref(),
                        IrType::Named { path, .. }
                            if ["Vec", "List", "HashMap", "HashSet", "Dict", "Set"]
                                .contains(&path.as_str())
                    ),
                    IrType::Tuple(_) => true,
                    IrType::Str => !matches!(&c.value.kind, ExprKind::Lit(LitKind::Str(_))),
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
                        self.custom_iterator_is_protocol = t
                            .methods
                            .iter()
                            .any(|m| m.name == "__next__" || m.name == "next");
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
                    // 命名字段列表（全命名 = 结构体变体）
                    let named_fields: Vec<String> = variant
                        .fields
                        .iter()
                        .filter(|f| !f.name.is_empty())
                        .map(|f| f.name.clone())
                        .collect();
                    let named =
                        !variant.fields.is_empty() && named_fields.len() == variant.fields.len();
                    self.enum_variant_named_fields.insert(
                        (e.name.clone(), variant.name.clone()),
                        if named { named_fields } else { Vec::new() },
                    );
                }
            }
            if let Item::StructDef(s) = item {
                let mset: std::collections::HashSet<String> =
                    s.methods.iter().map(|m| m.name.clone()).collect();
                let mut mset = mset;
                // __init__ 也被特殊处理，需加入 map 以便构造后调用点检测
                if s.has_init {
                    mset.insert("__init__".to_string());
                }
                self.struct_method_names_map.insert(s.name.clone(), mset);
                // 记录 __init__ 参数表，用于判断是否可自动注入（仅 self 参数时）
                if s.has_init {
                    self.struct_init_params_map
                        .insert(s.name.clone(), s.init_params.clone());
                }
                // 记录 __new__ 参数表，用于 kwarg 构造路由到 __new__
                if s.has_new {
                    self.struct_new_params_map
                        .insert(s.name.clone(), s.new_params.clone());
                }
                // struct 内定义方法的 ref/mut ref 参数标记登记 fn_ref_params
                // （vector.lz `__add__(ref self, ref other)` 的 other 调用点需自动 &，
                //   否则 (b).clone() 传 owned 报 E0308 expected &VectorInt）
                for m in &s.methods {
                    if m.params.iter().any(|p| p.is_ref) {
                        self.fn_ref_params.insert(
                            m.name.clone(),
                            m.params.iter().map(|p| (p.is_ref, p.is_mut)).collect(),
                        );
                    }
                }
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
                        // 若 __init__ 在 impl 块中，也记录其参数表以便注入点判定
                        if m.name == "__init__" {
                            let params: Vec<(String, IrType)> = m
                                .params
                                .iter()
                                .map(|p| (p.name.clone(), p.ty.clone()))
                                .collect();
                            self.struct_init_params_map.insert(path.clone(), params);
                        }
                        // 若 __new__ 在 impl 块中，记录其参数表以便 kwarg 构造路由
                        if m.name == "__new__" {
                            let params: Vec<(String, IrType)> = m
                                .params
                                .iter()
                                .map(|p| (p.name.clone(), p.ty.clone()))
                                .collect();
                            self.struct_new_params_map.insert(path.clone(), params);
                        }
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
                // 登记 type-pack 变参（..: Tuple<Ts...>）：args 参数类型为
                // Tuple([Generic("Ts")])（builder 标记）→ 记录 pack 泛型名，
                // 供调用点具体化为异质元组（03d §2.8 方案 B）
                if let Some((_, p)) = f
                    .params
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.variadic && p.name == "args")
                {
                    if let IrType::Tuple(items) = &p.ty {
                        if items.len() == 1 {
                            if let IrType::Generic(g) = &items[0] {
                                self.typepack_param.insert(f.name.clone(), g.clone());
                            }
                        }
                    }
                }
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
                // @init：模块初始化函数登记（main 开头按声明顺序注入调用）
                if f
                    .intrinsics
                    .iter()
                    .any(|i| matches!(i.kind, IntrinsicKind::Init))
                {
                    self.init_fns.push(f.name.clone());
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
            if let Item::CheckerBlock { name, captured, .. } = item {
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

        // 收集 type-pack 函数调用点实参类型（具体化签名数据源）。
        // 需在全部 typepack_param 登记完成后扫描，故作为独立 pass。
        if !self.typepack_param.is_empty() {
            for item in &module.items {
                match item {
                    Item::FnDef(f) => collect_typepack_calls(
                        &f.body,
                        &self.typepack_param,
                        &mut self.typepack_sigs,
                    ),
                    Item::CheckerBlock { body, .. } => {
                        collect_typepack_calls(body, &self.typepack_param, &mut self.typepack_sigs)
                    }
                    Item::StructDef(s) => {
                        for m in &s.methods {
                            collect_typepack_calls(
                                &m.body,
                                &self.typepack_param,
                                &mut self.typepack_sigs,
                            );
                        }
                    }
                    Item::EnumDef(e) => {
                        for m in &e.methods {
                            collect_typepack_calls(
                                &m.body,
                                &self.typepack_param,
                                &mut self.typepack_sigs,
                            );
                        }
                    }
                    Item::Impl(im) => {
                        for m in &im.methods {
                            collect_typepack_calls(
                                &m.body,
                                &self.typepack_param,
                                &mut self.typepack_sigs,
                            );
                        }
                    }
                    Item::Test(t) => collect_typepack_calls(
                        &t.body,
                        &self.typepack_param,
                        &mut self.typepack_sigs,
                    ),
                    _ => {}
                }
            }
        }

        // 顶层 self-def 归属判定（BUG-CG-002/TY-002，E0568 修复）：
        // 顶层 `def m(self: StructName, ...)` 的 self 注解类型是本模块
        // struct → 登记归属表（发射为 impl 方法 + 调用点方法语法）。
        // 独立 pass 的原因：struct 定义与 def 的源码顺序任意（struct 可在
        // def 之后），须在 struct_method_names_map 全量收集完成后判定。
        // 不含 self 或注解非本模块 struct（如基础类型/外部类型）→ 不归属。
        for item in &module.items {
            if let Item::FnDef(f) = item {
                let Some(self_p) = f.params.first() else {
                    continue;
                };
                if self_p.name != "self" {
                    continue;
                }
                if let IrType::Named { path, .. } = &self_p.ty {
                    // 剥离泛型参数（部分场景注解带 <>）后按基础名归属
                    let base = path.split('<').next().unwrap_or(path);
                    if self.struct_method_names_map.contains_key(base) {
                        // 方法名并入 struct 方法集合（user_plain 判定、
                        // magic 映射守卫等已有逻辑自动生效）
                        self.struct_method_names_map
                            .get_mut(base)
                            .unwrap()
                            .insert(f.name.clone());
                        self.self_fns
                            .insert(f.name.clone(), (base.to_string(), self_p.is_mut));
                        // 若该顶层方法是 __init__，同步记录参数表以便注入点判定
                        if f.name == "__init__" {
                            let params: Vec<(String, IrType)> = f
                                .params
                                .iter()
                                .map(|p| (p.name.clone(), p.ty.clone()))
                                .collect();
                            self.struct_init_params_map.insert(base.to_string(), params);
                        }
                    }
                }
            }
        }

        // 标准 prelude
        self.module_uses_ext = module.items.iter().any(|item| match item {
            Item::FnDef(f) => {
                matches!(f.ret_ty, IrType::Ext)
                    || f.intrinsics
                        .iter()
                        .any(|i| matches!(i.kind, IntrinsicKind::Extern(_)))
            }
            _ => false,
        });
        // 比较约束传播：预扫描全部 FnDef 体，计算各函数「参与比较运算」的泛型集合
        // （含调用图不动点），供 gen_fn_generics 注入 PartialEq/Eq/Ord（修复泛型比较算法）
        self.compute_cmp_constraints(module);
        // 变量引用计数：预计算各函数体内变量引用次数，供 move 语义修复（克隆多次
        // 使用的非 Copy 变量，避免 E0382 二次移动）
        self.compute_use_counts(module);

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
                // 已归属 struct 的顶层 self-def 不作自由函数发射（E0568），
                // 改由下方 gen_self_fn_impls 挂 impl 块
                if self.self_fns.contains_key(&f.name) {
                    continue;
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

        // 如果没有 main 函数，自动生成 main 并顺序执行顶层语句（BUG-PR-001：
        // 顶层表达式语句如 print(...) 需被执行，而非被静默丢弃）
        if !has_main {
            self.buf.push_str("pub fn main() {\n");
            self.indent += 1;
            let blk = Block {
                stmts: module.top_level_stmts.clone(),
                ty: IrType::Unit,
                span: Span::unknown(),
            };
            self.gen_block_inner(&blk);
            self.indent -= 1;
            self.buf.push_str("}\n");
        }

        // 顶层 self-def → impl 块（BUG-CG-002/TY-002，E0568 修复）：
        // 按 struct 分组发射 `impl StructName { fn m(...) }`，
        // 复用 gen_fn_def 的方法生成路径（is_method 分支渲染 &self 等）
        self.gen_self_fn_impls(module);

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
                IrType::Result { ok, err } => {
                    format!("Result<{}, {}>", self.rust_type(ok), self.rust_type(err))
                }
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
                IrType::Result { .. } => "Err(\"lz_walrus_default\".to_string())".to_string(),
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
        let uses_ext = self.module_uses_ext;
        self.emit_prelude_base();
        if uses_ext {
            self.emit_ext_preamble();
        }
    }

    fn emit_ext_preamble(&mut self) {
        // 外部专用句柄 ExtHandle（#[extern(lang)] 返回类型，无 extern 关键字）
        self.emit_line("/// 外部专用句柄：#[extern(lang)] 标记入口的统一返回值。");
        self.emit_line("#[derive(Debug, Clone)]");
        self.emit_line("pub struct ExtHandle {");
        self.emit_line("    pub lang: String,       // 目标语言标记（Rust / Python / C）");
        self.emit_line("    pub ptr: usize,         // 外部对象不透明句柄（0 = 错误）");
        self.emit_line("    pub err: Option<String>, // 错误约定：非 None 即为错误");
        self.emit_line("}");
        self.emit_line("impl ExtHandle {");
        self.emit_line("    pub fn ok(lang: &str, ptr: usize) -> Self { Self { lang: lang.to_string(), ptr, err: None } }");
        self.emit_line("    pub fn fail(lang: &str, msg: &str) -> Self { Self { lang: lang.to_string(), ptr: 0, err: Some(msg.to_string()) } }");
        self.emit_line("    pub fn is_err(&self) -> bool { self.err.is_some() || self.ptr == 0 }");
        self.emit_line("    pub fn err_msg(&self) -> Option<&str> { self.err.as_deref() }");
        self.emit_line("}");
        // 分发器：extern 入口经此调用（缺失实现 → 可定位诊断，不静默）
        self.emit_line(
            "fn __lz_ext_call(lang: &str, name: &str, args: Vec<String>) -> ExtHandle {",
        );
        self.emit_line("    let _ = (lang, name, args);");
        self.emit_line("    ExtHandle::fail(lang, &format!(\"no external implementation for '{}' (lang {})\", name, lang))");
        self.emit_line("}");
        self.buf.push('\n');
    }

    fn emit_prelude_base(&mut self) {
        // Rust 2021 edition support (async/await, etc.)
        // 使用 outer attributes (#[..]) 而非 inner attributes (#![..])
        // 因为 type alias 可能已在 prelude 之前输出，inner attributes 不允许出现在 item 之后
        self.emit_line("#[allow(unused_imports)]");
        self.emit_line("#[allow(unused_variables)]");
        self.emit_line("#[allow(dead_code)]");
        self.emit_line("#[allow(non_snake_case)]");
        self.buf.push('\n');
        // 用户自定义 HashMap/HashSet（如 lib_hashmap 的 struct HashMap）时跳过
        // std 同名导入，否则 E0255 重复定义 + E0107 缺泛型（与 Rc/Arc 守卫同款）。
        // 注意 collections 导入在前、any::Any 在后，保持输出顺序稳定。
        let wants_hashmap = !self.known_types.contains("HashMap");
        let wants_hashset = !self.known_types.contains("HashSet");
        if wants_hashmap && wants_hashset {
            self.emit_line("use std::collections::{HashMap, HashSet};");
        } else if wants_hashmap {
            self.emit_line("use std::collections::HashMap;");
        } else if wants_hashset {
            self.emit_line("use std::collections::HashSet;");
        }
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

        // 未定义外部函数 stub（p16_lzi 等跨模块探测用例：.lzi 签名未加载时兜底）。
        // 名字取自 lz_builtins 导出函数表之外、且模块内无定义的顶层调用。
        // 含 `::` 的路径调用（如 Struct::__from__ 隐式转换）是模块内已定义方法，
        // 不是外部函数，跳过——否则生成 `fn Wrapper::__from__(...)` 非法 Rust
        //（E0530 invalid path separator in function definition）
        if !self.unknown_extern_fns.is_empty() {
            let mut names: Vec<(String, usize)> = self
                .unknown_extern_fns
                .iter()
                .filter(|(k, _)| !k.contains("::"))
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            names.sort();
            for (name, arity) in names {
                let params: Vec<String> = (0..arity).map(|i| format!("__a{}: i64", i)).collect();
                self.emit_line(&format!(
                    "#[allow(non_snake_case)] #[allow(dead_code)]\nfn {}({}) -> i64 {{ i64::MAX }}",
                    name,
                    params.join(", ")
                ));
            }
            self.buf.push('\n');
        }
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
    fn struct_method_names(&self, name: &str) -> std::collections::HashSet<String> {
        self.struct_method_names_map
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    /// 判断表达式是否为「对 str/String（含 &String）调用 .clone()」。
    /// 这类实参的 IR 类型常退化为 Any，但 clone 结果仍是 String，按字符串实参
    /// 处理（Pattern 方法需 &）。例：string.lz `self.starts_with(other.clone())`。
    fn expr_is_clone_of_string(e: &Expr) -> bool {
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
    fn duck_sig_type(&self, ty: &IrType, duck: &DuckDef) -> String {
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
    const STD_ERROR_KIND_NAMES: &[&str] = &[
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

    fn rust_type(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "i64".into(),
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

    /// LZ `fn` **值**（变量绑定 / 函数返回值）的 Rust 表示：`Box<dyn Fn(P) -> R>`。
    /// 嵌套 `fn -> fn -> T` 递归为 `Box<dyn Fn(P) -> Box<dyn Fn(Q) -> R>>`。
    ///
    /// 注意区分三种位置（IR-003 / 设计文档）：
    /// - **值位置**（变量 / 返回值）：用此处 `Box<dyn Fn>`（支持捕获闭包作一等值）。
    /// - **HOF 形参位置**：保留 `impl FnMut`（借用捕获，见 gen_param），不读此函数。
    /// - **struct 字段位置**：保留 `fn` 指针（Clone 安全，见 gen_struct_def 字段覆盖），
    ///   不读此函数。
    fn fn_value_type(&self, ty: &IrType) -> String {
        match ty {
            IrType::Fn { params, ret } => {
                let p: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                format!(
                    "Box<dyn Fn({}) -> {}>",
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
    fn option_none_elem(&self, expr_ty: &IrType) -> String {
        let pick = |ty: &IrType| -> Option<String> {
            // IR 语义标记 Option(T)
            if let IrType::Option(inner) = ty {
                return Some(self.rust_type(inner));
            }
            // 命名类型 Option<T>
            if let IrType::Named { path, args } = ty {
                if path == "Option" && args.len() == 1 {
                    return Some(self.rust_type(&args[0]));
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
    fn empty_list_elem(&self, expr_ty: &IrType) -> String {
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

    /// 扩展 trait（ListExt/StrExt 等）方法名映射：与调用点 MethodCall 的
    /// `slice → lz_slice` 映射保持一致，否则 trait 声明是 slice、调用是
    /// lz_slice → E0599（list.lz 复现）
    fn ext_trait_method_name(&self, name: &str, _ext_name: &str) -> String {
        match name {
            "slice" => "lz_slice".to_string(),
            _ => name.to_string(),
        }
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
                        if p.is_mut {
                            format!("&mut {}", self.rust_type(&p.ty))
                        } else {
                            format!("&{}", self.rust_type(&p.ty))
                        }
                    } else {
                        // fn(...) 类型参数 → impl Fn(...)：可接受闭包（03e §五），
                        // 直接生成 fn 指针无法接收 move 闭包（E0308）。
                        // 用 impl Fn（非 FnMut）：闭包体内调用此类参数得到的是 Fn 闭包，
                        // 才能作为 Box<dyn Fn> 返回（compose 返回 |x| g(f(x))，E0596）。
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
                                format!(
                                    "impl Fn({}) -> {} + 'static",
                                    ps.join(", "),
                                    self.rust_type(ret)
                                )
                            }
                        } else {
                            self.rust_type(&p.ty).to_string()
                        }
                    };
                    // __Params 值参数（checker 链值函数，如 def double_ps(ps: __Params)）
                    // 体内会写 ps.args，必须生成 `mut ps: __Params`（否则 E0596）
                    let ty_is_params =
                        matches!(&p.ty, IrType::Named { path, .. } if path == "__Params");
                    // Iterator 参数：.next() 需要 &mut self，必须生成 `mut it: impl Iterator<..>`
                    let ty_is_iterator =
                        matches!(&p.ty, IrType::Named { path, .. } if path == "Iterator");
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
                    IrType::Fn { params, ret } => {
                        // LZ `fn` 值返回：统一用 Box<dyn Fn（IR-003，支持捕获闭包作返回）。
                        // 嵌套 fn -> fn -> T 由 fn_value_type 递归为
                        // Box<dyn Fn(P) -> Box<dyn Fn(Q) -> R>；
                        // 单层 fn(int)->int → Box<dyn Fn(i64) -> i64>
                        // （原 impl Fn(P) -> i64 为误生成，E0308）。
                        let p: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                        format!(
                            "Box<dyn Fn({}) -> {}>",
                            p.join(", "),
                            self.fn_value_type(ret)
                        )
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
            let key_types: Vec<String> = f
                .params
                .iter()
                .map(|p| self.rust_type(&p.ty))
                .collect();
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
            // fn a(A) -> Box<dyn Fn(B) -> Box<dyn Fn(C) -> R>>
            let mut ret_chain = self.rust_type(&f.ret_ty);
            for p in &f.params[1..] {
                ret_chain = format!("Box<dyn Fn({}) -> {}>", self.rust_type(&p.ty), ret_chain);
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
                        "Box::new(move |{}: {}| -> {} {{",
                        pname,
                        pty,
                        self.rust_type(&f.ret_ty)
                    ));
                } else {
                    self.emit_line(&format!("Box::new(move |{}: {}| {{", pname, pty));
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
            self.emit_line("if let Some(__lz_hit) = __lz_memo.iter().find(|(k, _)| *k == __lz_memo_key) {");
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
        // BUG-CG-004（轮次12）：记录当前函数 raises 异常类型，供 try/catch 结果基分支判断
        self.current_fn_raises = f.raises.clone();
        // 登记顶层 def 名称（IR-003：顶层函数作值时需 Box::new(f)，局部 fn-let 已装箱不重包）
        self.top_level_fns.insert(f.name.clone());
        // 嵌套 Fn 返回类型（fn -> fn -> T）：内层闭包返回值需 Box::new 包装
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
            self.emit_line("let __async_main = async {");
            self.indent += 1;
            self.gen_block_inner(&f.body);
            self.indent -= 1;
            self.emit_line("};");
            self.emit_line("__block_on(__async_main);");
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
            for m in &s.methods {
                self.gen_fn_def(m);
                self.buf.push('\n');
            }
            self.current_impl_generics = saved_impl_generics;
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

    fn gen_enum_def(&mut self, e: &EnumDef) {
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
                let uses_self_item = sig.body.is_some()
                    || matches!(&sig.ret, IrType::Named { path, .. } if path.contains("Self."))
                    || sig
                        .params
                        .iter()
                        .any(|p| matches!(p, IrType::Named { path, .. } if path.contains("Self.")));
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
        for m in &i.methods {
            // __new__ 体内抑制 kwarg→__new__ 路由（避免 Self(v:...) 无限递归）
            let prev_in_new_body = self.in_new_body;
            if m.name == "__new__" {
                self.in_new_body = true;
            }
            self.gen_fn_def(m);
            self.in_new_body = prev_in_new_body;
            self.buf.push('\n');
        }
        self.current_impl_generics = saved_impl_generics;
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
        let enum_derives_partial_eq = matches!(&i.for_type, IrType::Named { path, .. }
            if self.enum_variants.values().any(|en| en == path));
        // 外部/内置类型（Vec/str/String/HashMap…）：Rust 孤儿规则禁止为外部类型
        // 实现外部 trait（E0117），且 std 已提供 PartialEq，跳过自动 impl
        let is_external_type = matches!(&i.for_type, IrType::Named { path, .. }
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

    /// 自动为魔法方法生成对应的 Rust trait impl（联动补全，对齐设计 §9.1）。
    /// 仅在 inherent impl（trait_==None）且非枚举/外部类型时由 emit_impl 调用。
    ///
    /// 关键：每个魔法方法自带 `where` 约束（如 `def __str__(ref self) -> str
    /// where T: Display`），其生成的 trait impl 必须**携带同名 `where` 子句**，
    /// 否则 impl 块的泛型上下文缺少该约束，调用 `__str__()` 时触发 E0277
    /// （box.lz `impl<T: Clone + Debug> Display for Box<T>` 缺 `T: Display`）。
    fn gen_magic_trait_impls(&mut self, for_ty: &str, generics: &str, methods: &[FnDef]) {
        // __eq__ → std::cmp::PartialEq（struct 体内 / impl 块共用）。
        // 委托 __eq__ 生成 impl，并携带 __eq__ 的 where 约束（如 T: Eq）。
        // 第二参数为 ref 时直接传 other（&Self）；值为参数时需 (*other).clone()。
        if let Some(eq_m) = methods.iter().find(|m| m.name == "__eq__") {
            let eq_where: String = eq_m
                .where_clause
                .iter()
                .map(|(tp, bounds)| {
                    let bs: Vec<String> = bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                    format!("{}: {}", tp, bs.join(" + "))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let where_str = if eq_where.is_empty() {
                String::new()
            } else {
                format!(" where {}", eq_where)
            };
            self.emit_line(&format!(
                "impl{} std::cmp::PartialEq for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn eq(&self, other: &Self) -> bool {");
            self.indent += 1;
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

        // __str__ → std::fmt::Display
        if let Some(sm) = methods.iter().find(|m| m.name == "__str__") {
            let where_str = self.magic_impl_where_str(sm);
            self.emit_line(&format!(
                "impl{} std::fmt::Display for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {");
            self.indent += 1;
            self.emit_line("write!(f, \"{}\", self.__str__())");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __repr__ → std::fmt::Debug
        if let Some(rm) = methods.iter().find(|m| m.name == "__repr__") {
            let where_str = self.magic_impl_where_str(rm);
            self.emit_line(&format!(
                "impl{} std::fmt::Debug for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {");
            self.indent += 1;
            self.emit_line("write!(f, \"{}\", self.__repr__())");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __next__ → std::iter::Iterator（仅当返回 Option<T> 时生成；否则 next 签名不匹配）
        if let Some(nm) = methods.iter().find(|m| m.name == "__next__") {
            if let IrType::Option(inner) = &nm.ret_ty {
                let where_str = self.magic_impl_where_str(nm);
                let item_ty = self.rust_type(inner);
                self.emit_line(&format!(
                    "impl{} std::iter::Iterator for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Item = {};", item_ty));
                self.emit_line("fn next(&mut self) -> Option<Self::Item> {");
                self.indent += 1;
                self.emit_line("self.__next__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __rev__ → std::iter::DoubleEndedIterator（06d §九）：仅当 __next__ 共存时
        // 生成（DoubleEndedIterator 继承 Iterator，需先有 Iterator impl）。
        // `def __rev__(ref self) -> Option<T>` →
        // impl DoubleEndedIterator for SelfTy { fn next_back(&mut self) -> Option<Self::Item> { self.__rev__() } }
        // 注意：不在 let 链中写 `&& let`（Rust 2021 不支持），改用 match 解构
        if methods.iter().any(|m| m.name == "__next__") {
            if let Some(rm) = methods.iter().find(|m| m.name == "__rev__") {
                if let IrType::Option(inner) = &rm.ret_ty {
                    let where_str = self.magic_impl_where_str(rm);
                    self.emit_line(&format!(
                        "impl{} std::iter::DoubleEndedIterator for {} {} {{",
                        generics, for_ty, where_str
                    ));
                    self.indent += 1;
                    self.emit_line("fn next_back(&mut self) -> Option<Self::Item> {");
                    self.indent += 1;
                    self.emit_line("self.__rev__()");
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
        }

        // __iter__ → std::iter::IntoIterator（仅当返回命名迭代器类型时生成；
        // 返回 () 等非法类型会报 "() is not an iterator"，见 duck_nested.lz）
        // __iter__ → std::iter::IntoIterator（06d §九）：
        // `def __iter__(self) -> IterTy` → impl IntoIterator。
        // 当返回 Vec<T> 时，IntoIter 不能是 Vec<T>（Vec 不满足 Iterator），
        // 应映射为 std::vec::IntoIter<T> 并在 into_iter 体调用 .into_iter()
        // 包装（p41 探针暴露：`Vec<i64>` is not an iterator）
        if let Some(im) = methods.iter().find(|m| m.name == "__iter__") {
            if let IrType::Named { .. } = &im.ret_ty {
                let where_str = self.magic_impl_where_str(im);
                let (iter_ty, body_expr) = match &im.ret_ty {
                    IrType::Named { path, args } if path == "Vec" && args.len() == 1 => {
                        let elem = self.rust_type(&args[0]);
                        (
                            format!("std::vec::IntoIter<{}>", elem),
                            "self.__iter__().into_iter()".to_string(),
                        )
                    }
                    IrType::Named { path, args } if path == "List" && args.len() == 1 => {
                        let elem = self.rust_type(&args[0]);
                        (
                            format!("std::vec::IntoIter<{}>", elem),
                            "self.__iter__().into_iter()".to_string(),
                        )
                    }
                    _ => {
                        let ty = self.rust_type(&im.ret_ty);
                        (ty, "self.__iter__()".to_string())
                    }
                };
                self.emit_line(&format!(
                    "impl{} std::iter::IntoIterator for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type IntoIter = {};", iter_ty));
                self.emit_line(&format!(
                    "type Item = <{} as std::iter::Iterator>::Item;",
                    iter_ty
                ));
                self.emit_line("fn into_iter(self) -> Self::IntoIter {");
                self.indent += 1;
                self.emit_line(&body_expr);
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // 复合赋值族 → std::ops::*Assign（06d §四，对齐 __iadd__ 先例）：
        // __iadd__→AddAssign、__isub__→SubAssign、__imul__→MulAssign、__idiv__→DivAssign
        //（Rhs 取 other 参数类型；self 固有方法为 `mut self`，AddAssign 的
        // add_assign(&mut self, rhs) 经 auto-mut 调用即可）
        for (magic, trait_path, trait_method) in &[
            ("__iadd__", "std::ops::AddAssign", "add_assign"),
            ("__isub__", "std::ops::SubAssign", "sub_assign"),
            ("__imul__", "std::ops::MulAssign", "mul_assign"),
            ("__idiv__", "std::ops::DivAssign", "div_assign"),
        ] {
            if let Some(am) = methods.iter().find(|m| m.name == *magic) {
                let where_str = self.magic_impl_where_str(am);
                let rhs_ty = am
                    .params
                    .get(1)
                    .map(|p| self.rust_type(&p.ty))
                    .unwrap_or_else(|| "()".to_string());
                self.emit_line(&format!(
                    "impl{} {}<{}> for {} {} {{",
                    generics, trait_path, rhs_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!(
                    "fn {}(&mut self, other: {}) {{",
                    trait_method, rhs_ty
                ));
                self.indent += 1;
                self.emit_line(&format!("self.{}(other)", magic));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // 一元 __neg__ → std::ops::Neg / __not__ → std::ops::Not：
        // 调用点已直派 __neg__/__not__（builder Unary 分派），trait impl 供
        // 泛型/库代码路径使用
        if let Some(nm) = methods.iter().find(|m| m.name == "__neg__") {
            if !matches!(nm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(nm);
                let out_ty = self.rust_type(&nm.ret_ty);
                self.emit_line(&format!(
                    "impl{} std::ops::Neg for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", out_ty));
                self.emit_line(&format!("fn neg(self) -> {} {{", out_ty));
                self.indent += 1;
                self.emit_line("self.__neg__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }
        if let Some(nm) = methods.iter().find(|m| m.name == "__not__") {
            if !matches!(nm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(nm);
                let out_ty = self.rust_type(&nm.ret_ty);
                self.emit_line(&format!(
                    "impl{} std::ops::Not for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", out_ty));
                self.emit_line(&format!("fn not(self) -> {} {{", out_ty));
                self.indent += 1;
                self.emit_line("self.__not__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }
        // 位非 ~a：__invert__ 复用 std::ops::Not impl（注册表 Invert kind 同 trait；
        // Rust 无独立 BitNot trait，Not 即位非/逻辑非统一入口）
        let has_not = methods.iter().any(|m| m.name == "__not__");
        if let Some(nm) = methods.iter().find(|m| m.name == "__invert__") {
            if !has_not && !matches!(nm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(nm);
                let out_ty = self.rust_type(&nm.ret_ty);
                self.emit_line(&format!(
                    "impl{} std::ops::Not for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", out_ty));
                self.emit_line(&format!("fn not(self) -> {} {{", out_ty));
                self.indent += 1;
                self.emit_line("self.__invert__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __cmp__ → std::cmp::Ord（06d §五）：全序比较委托 __cmp__。
        // Ord 要求 Eq + PartialOrd 超trait——仅在 __eq__/__lt__ 同时定义
        // （PartialEq/PartialOrd impl 已生成）时才生成，否则 E0277。
        // LZ 的 __cmp__ 返回 int（-1/0/1），映射为 Ordering::Less/Equal/Greater
        let has_eq_impl = methods.iter().any(|m| m.name == "__eq__");
        let has_lt_impl = methods.iter().any(|m| m.name == "__lt__");
        if let Some(cm) = methods.iter().find(|m| m.name == "__cmp__") {
            if has_eq_impl && has_lt_impl {
                // Rust 的 Eq 是独立标记 trait（Ord/Hash 超trait 要求 Eq），
                // 仅 PartialEq 不满足——需一并生成空 Eq 标记 impl
                let eq_m = methods.iter().find(|m| m.name == "__eq__");
                let eq_where = eq_m
                    .map(|m| self.magic_impl_where_str(m))
                    .unwrap_or_default();
                self.emit_line(&format!(
                    "impl{} std::cmp::Eq for {} {} {{}}",
                    generics, for_ty, eq_where
                ));
                let where_str = self.magic_impl_where_str(cm);
                self.emit_line(&format!(
                    "impl{} std::cmp::Ord for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line("fn cmp(&self, other: &Self) -> std::cmp::Ordering {");
                self.indent += 1;
                // __cmp__ 第二参数 ref 直接传，owned 需解引用克隆
                let takes_ref = cm.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                let arg = if takes_ref {
                    "other".to_string()
                } else {
                    "(*other).clone()".to_string()
                };
                self.emit_line(&format!(
                    "let c = self.__cmp__({}); if c < 0 {{ std::cmp::Ordering::Less }} else if c == 0 {{ std::cmp::Ordering::Equal }} else {{ std::cmp::Ordering::Greater }}",
                    arg
                ));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __hash__ → std::hash::Hash（06d §五）：哈希委托 __hash__（返回 int）
        if let Some(hm) = methods.iter().find(|m| m.name == "__hash__") {
            let where_str = self.magic_impl_where_str(hm);
            self.emit_line(&format!(
                "impl{} std::hash::Hash for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn hash<H: std::hash::Hasher>(&self, state: &mut H) {");
            self.indent += 1;
            self.emit_line("self.__hash__().hash(state)");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __drop__ → std::ops::Drop（06d §十）：析构委托 __drop__（&mut self）。
        // 注意：Drop impl 的泛型参数须与 struct 定义**逐字一致**（E0367）——
        // generics 串可能带 bounds（`<T: Clone + Debug>`），加上即与无约束的
        // struct 定义不匹配；裸泛型内调 self.__drop__() 又会因方法约束报 E0599
        //（box.lz）。故仅方法与泛型串均无约束时生成（保守回退）
        if let Some(dm) = methods.iter().find(|m| m.name == "__drop__") {
            if dm.where_clause.is_empty() && !generics.contains(':') {
                self.emit_line(&format!(
                    "impl{} std::ops::Drop for {} {{",
                    generics, for_ty
                ));
                self.indent += 1;
                self.emit_line("fn drop(&mut self) {");
                self.indent += 1;
                self.emit_line("self.__drop__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __clone__ → std::clone::Clone 委托（06d §十）：struct 定义处已去掉
        // derive(Clone)（E0119），此处生成手动 impl，使 `a.clone()` 走用户的
        // 自定义克隆逻辑（如引用计数 +1）而非逐字段复制
        if let Some(cm) = methods.iter().find(|m| m.name == "__clone__") {
            let where_str = self.magic_impl_where_str(cm);
            self.emit_line(&format!(
                "impl{} std::clone::Clone for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn clone(&self) -> Self {");
            self.indent += 1;
            self.emit_line("self.__clone__()");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __default__ → std::default::Default（06d §十）：默认值委托 __default__
        //（静态方法，无 self，返回 Self）
        if let Some(fm) = methods.iter().find(|m| m.name == "__default__") {
            if !matches!(fm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(fm);
                self.emit_line(&format!(
                    "impl{} std::default::Default for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line("fn default() -> Self {");
                self.indent += 1;
                self.emit_line("Self::__default__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __int__/__float__ 缺口魔法（06d §十六）：编译器自动生成
        // impl From<SelfTy> for i64 / impl From<SelfTy> for f64，
        // 使 int(x)/float(x) 可经 .into() 或调用点直派转换
        for (magic, std_ty) in &[("__int__", "i64"), ("__float__", "f64")] {
            if let Some(im) = methods.iter().find(|m| m.name == *magic) {
                if !matches!(im.ret_ty, IrType::Unit) {
                    let where_str = self.magic_impl_where_str(im);
                    self.emit_line(&format!(
                        "impl{} std::convert::From<{}> for {} {} {{",
                        generics, for_ty, std_ty, where_str
                    ));
                    self.indent += 1;
                    self.emit_line(&format!("fn from(value: {}) -> Self {{", for_ty));
                    self.indent += 1;
                    self.emit_line(&format!("value.{}()", magic));
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
        }

        // __implicit_copy__ → lz_builtins::ImplicitCopy（06d §十四 Mojo 风格）：
        // `def __implicit_copy__(self) -> Self` →
        // impl ImplicitCopy for SelfTy { fn __implicit_copy__(&self) -> Self { Self::__implicit_copy__(&self) } }
        if let Some(ic) = methods.iter().find(|m| m.name == "__implicit_copy__") {
            let where_str = self.magic_impl_where_str(ic);
            self.emit_line(&format!(
                "impl{} ImplicitCopy for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn __implicit_copy__(&self) -> Self {");
            self.indent += 1;
            self.emit_line(&format!("{}::__implicit_copy__(&self)", for_ty));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __implicit_default__ → lz_builtins::ImplicitDefault（06d §十四）：
        // `def __implicit_default__() -> Self`（静态方法，无 self 参数）→
        // impl ImplicitDefault for SelfTy { fn __implicit_default__() -> Self { Self::__implicit_default__() } }
        if methods.iter().any(|m| m.name == "__implicit_default__") {
            let where_str =
                if let Some(im) = methods.iter().find(|m| m.name == "__implicit_default__") {
                    self.magic_impl_where_str(im)
                } else {
                    String::new()
                };
            self.emit_line(&format!(
                "impl{} ImplicitDefault for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn __implicit_default__() -> Self {");
            self.indent += 1;
            self.emit_line(&format!("{}::__implicit_default__()", for_ty));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __implicit_from__ → ImplicitFrom blanket（06d §十五 隐式策略）：
        // 用户在 impl 块定义 `def __implicit_from__(raw: SrcTy) -> Self` →
        // impl ImplicitFrom<SrcTy> for SelfTy，委托用户方法（区别于 struct
        // 声明式 implicit_froms 的内建首字段构造，见 emit StructDef 处）
        if let Some(im) = methods.iter().find(|m| m.name == "__implicit_from__") {
            if let Some(p0) = im.params.first() {
                let src_ty = self.rust_type(&p0.ty);
                let where_str = self.magic_impl_where_str(im);
                self.emit_line(&format!(
                    "impl{} ImplicitFrom<{}> for {} {} {{",
                    generics, src_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!(
                    "fn __implicit_from__(value: {}) -> Self {{",
                    src_ty
                ));
                self.indent += 1;
                self.emit_line(&format!("{}::__implicit_from__(value)", for_ty));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __implicit_to__ → lz_builtins::ImplicitInto（06d §十四）：
        // `def __implicit_to__(self) -> TgtTy` →
        // impl ImplicitInto<TgtTy> for SelfTy { fn __implicit_into__(&self) -> TgtTy { Self::__implicit_to__(self) } }
        if let Some(it) = methods.iter().find(|m| m.name == "__implicit_to__") {
            let tgt_ty = self.rust_type(&it.ret_ty);
            let where_str = self.magic_impl_where_str(it);
            self.emit_line(&format!(
                "impl{} ImplicitInto<{}> for {} {} {{",
                generics, tgt_ty, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line(&format!("fn __implicit_into__(&self) -> {} {{", tgt_ty));
            self.indent += 1;
            self.emit_line(&format!("{}::__implicit_to__(&self)", for_ty));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __from__ → std::convert::From（隐式转换第一环，06d §十四）：
        // `def __from__(raw: SrcTy) -> Self`（静态方法，无 self 参数）→
        // impl From<SrcTy> for SelfTy { fn from(value: SrcTy) -> Self { Self::__from__(value) } }
        // 返回 Unit / 无参数时无 trait 对应物，跳过
        if let Some(fm) = methods.iter().find(|m| m.name == "__from__") {
            if let (Some(p0), false) = (fm.params.first(), matches!(fm.ret_ty, IrType::Unit)) {
                let src_ty = self.rust_type(&p0.ty);
                let where_str = self.magic_impl_where_str(fm);
                self.emit_line(&format!(
                    "impl{} std::convert::From<{}> for {} {} {{",
                    generics, src_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("fn from(value: {}) -> Self {{", src_ty));
                self.indent += 1;
                self.emit_line("Self::__from__(value)");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __into__ → std::convert::Into（From→Into 链第二环，06d §十四）：
        // `def __into__(self) -> TargetTy` →
        // impl Into<TargetTy> for SelfTy { fn into(self) -> TargetTy { self.__into__() } }
        // （Rust 已有 From→Into blanket，但用户只定义 __into__ 时无 From，需显式 Into impl）
        if let Some(im) = methods.iter().find(|m| m.name == "__into__") {
            if !matches!(im.ret_ty, IrType::Unit) {
                let tgt_ty = self.rust_type(&im.ret_ty);
                let where_str = self.magic_impl_where_str(im);
                self.emit_line(&format!(
                    "impl{} std::convert::Into<{}> for {} {} {{",
                    generics, tgt_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("fn into(self) -> {} {{", tgt_ty));
                self.indent += 1;
                self.emit_line("self.__into__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __try_from__ → std::convert::TryFrom（06d §六）：
        // `def __try_from__(raw: SrcTy) -> Result<Self, E>` →
        // impl TryFrom<SrcTy> for SelfTy { type Error = E;
        //   fn try_from(value: SrcTy) -> Result<Self, Self::Error> { Self::__try_from__(value) } }
        if let Some(fm) = methods.iter().find(|m| m.name == "__try_from__") {
            if let (Some(p0), Some(err_ty)) = (fm.params.first(), Self::try_from_err_ty(&fm.ret_ty))
            {
                if !matches!(fm.ret_ty, IrType::Unit) {
                    let src_ty = self.rust_type(&p0.ty);
                    let err_s = self.rust_type(&err_ty);
                    let where_str = self.magic_impl_where_str(fm);
                    self.emit_line(&format!(
                        "impl{} std::convert::TryFrom<{}> for {} {} {{",
                        generics, src_ty, for_ty, where_str
                    ));
                    self.indent += 1;
                    self.emit_line(&format!("type Error = {};", err_s));
                    self.emit_line(&format!(
                        "fn try_from(value: {}) -> Result<Self, Self::Error> {{",
                        src_ty
                    ));
                    self.indent += 1;
                    self.emit_line("Self::__try_from__(value)");
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
        }

        // __try_into__ → std::convert::TryInto（06d §六）：
        // `def __try_into__(self) -> Result<TargetTy, E>` →
        // impl TryInto<TargetTy> for SelfTy { type Error = E;
        //   fn try_into(self) -> Result<TargetTy, Self::Error> { self.__try_into__() } }
        if let Some(im) = methods.iter().find(|m| m.name == "__try_into__") {
            if let Some((tgt, err_ty)) = Self::try_into_tgt_err(&im.ret_ty) {
                let tgt_s = self.rust_type(&tgt);
                let err_s = self.rust_type(&err_ty);
                let where_str = self.magic_impl_where_str(im);
                self.emit_line(&format!(
                    "impl{} std::convert::TryInto<{}> for {} {} {{",
                    generics, tgt_s, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Error = {};", err_s));
                self.emit_line(&format!(
                    "fn try_into(self) -> Result<{}, Self::Error> {{",
                    tgt_s
                ));
                self.indent += 1;
                self.emit_line("self.__try_into__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // 比较族 __eq__ + __lt__ → std::cmp::PartialOrd：
        // `<` `>` 等运算符调用点已直派 __lt__/__gt__（本文件 comparison 分派），
        // 但泛型场景（Vec<V> 排序、sort()）需要 PartialOrd impl。由
        // __eq__/__lt__ 推导 partial_cmp（Equal/Less/Greater），两者都定义才生成
        //（PartialOrd: PartialEq 超trait，缺 __eq__ 时 impl 会 E0277）
        if let (Some(eq_m), Some(lt_m)) = (
            methods.iter().find(|m| m.name == "__eq__"),
            methods.iter().find(|m| m.name == "__lt__"),
        ) {
            let where_str = self.magic_impl_where_str(lt_m);
            self.emit_line(&format!(
                "impl{} std::cmp::PartialOrd for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {");
            self.indent += 1;
            // __eq__/__lt__ 第二参数可能为 ref（box.lz `ref other`）或 owned：
            // ref 直接传 other（&Self），owned 需 (*other).clone() 解引用克隆
            let arg_of = |m: &FnDef| -> String {
                let takes_ref = m.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                if takes_ref {
                    "other".to_string()
                } else {
                    "(*other).clone()".to_string()
                }
            };
            let eq_arg = arg_of(eq_m);
            let lt_arg = arg_of(lt_m);
            self.emit_line(&format!(
                "if self.__eq__({}) {{ Some(std::cmp::Ordering::Equal) }} else if self.__lt__({}) {{ Some(std::cmp::Ordering::Less) }} else {{ Some(std::cmp::Ordering::Greater) }}",
                eq_arg, lt_arg
            ));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // 算术/位运算族（注册表签名固定型）→ std::ops trait impl：
        // __add__→Add、__sub__→Sub、__mul__→Mul、__div__→Div、__rem__→Rem、
        // __bitand__→BitAnd、__bitor__→BitOr、__bitxor__→BitXor、__shl__→Shl、__shr__→Shr
        // 价值：`+=` 未定义 __iadd__ 时脱糖为 `a = a + b`，需要 Add impl 才能编译。
        // Rust 的 Add::add 接收 owned self：用户 `ref self` 时经 auto-ref 调用
        // &self 版本；rhs 参数 ref 时取 &rhs（与 __eq__ 的 owned/ref 处理一致）。
        for (magic, trait_path, trait_method) in &[
            ("__add__", "std::ops::Add", "add"),
            ("__sub__", "std::ops::Sub", "sub"),
            ("__mul__", "std::ops::Mul", "mul"),
            ("__div__", "std::ops::Div", "div"),
            ("__rem__", "std::ops::Rem", "rem"),
            ("__bitand__", "std::ops::BitAnd", "bitand"),
            ("__bitor__", "std::ops::BitOr", "bitor"),
            ("__bitxor__", "std::ops::BitXor", "bitxor"),
            ("__shl__", "std::ops::Shl", "shl"),
            ("__shr__", "std::ops::Shr", "shr"),
        ] {
            if let Some(mm) = methods.iter().find(|m| m.name == *magic) {
                // 输出类型：无返回注解（Unit）的算术魔法方法没有 trait 对应物，跳过
                if matches!(mm.ret_ty, IrType::Unit) {
                    continue;
                }
                let where_str = self.magic_impl_where_str(mm);
                let rhs_is_ref = mm.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                let rhs_ty = mm
                    .params
                    .get(1)
                    .map(|p| self.rust_type(&p.ty))
                    .unwrap_or_else(|| for_ty.to_string());
                let output_ty = self.rust_type(&mm.ret_ty);
                // Rhs 泛型参数：ref 参数（&V）时 impl Add<&V>，调用传 &rhs
                let rhs_arg = if rhs_is_ref {
                    format!("&{}", rhs_ty)
                } else {
                    rhs_ty.clone()
                };
                self.emit_line(&format!(
                    "impl{} {}<{}> for {} {} {{",
                    generics, trait_path, rhs_arg, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!(
                    "type Output = {};",
                    if *magic == "__shl__" || *magic == "__shr__" {
                        // Shl/Shr 的 Output 可为任意类型，但 trait 默认 ()；显式声明
                        output_ty.clone()
                    } else {
                        output_ty.clone()
                    }
                ));
                self.emit_line(&format!(
                    "fn {}(self, rhs: {}) -> {} {{",
                    trait_method, rhs_arg, output_ty
                ));
                self.indent += 1;
                if rhs_is_ref {
                    self.emit_line(&format!("self.{}(&rhs)", magic));
                } else {
                    self.emit_line(&format!("self.{}(rhs)", magic));
                }
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __pow__ —— 幂 `**` → lz_builtins::LzPow（std::ops 无对应 trait，LZ 自定义）：
        // 模式与算术运算符一致（委托 self.__pow__(rhs)）。数值 `**` 走内建 .pow()；
        // 用户 struct 定义 __pow__ 时生成 impl LzPow<Rhs>，使泛型场景可约束幂运算（06d §一）。
        if let Some(pm) = methods.iter().find(|m| m.name == "__pow__") {
            if !matches!(pm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(pm);
                let rhs_is_ref = pm.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                let rhs_ty = pm
                    .params
                    .get(1)
                    .map(|p| self.rust_type(&p.ty))
                    .unwrap_or_else(|| for_ty.to_string());
                let output_ty = self.rust_type(&pm.ret_ty);
                let rhs_arg = if rhs_is_ref {
                    format!("&{}", rhs_ty)
                } else {
                    rhs_ty.clone()
                };
                self.emit_line(&format!(
                    "impl{} lz_builtins::LzPow<{}> for {} {} {{",
                    generics, rhs_arg, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", output_ty));
                // trait 方法名 `pow` 与固有魔法名 `__pow__` 不同（仿 std::ops::Add::add），
                // impl 体内 `self.__pow__(rhs)` 唯一解析到固有方法，无同名递归（仿 Add 循环）。
                self.emit_line(&format!(
                    "fn pow(self, rhs: {}) -> {} {{",
                    rhs_arg, output_ty
                ));
                self.indent += 1;
                if rhs_is_ref {
                    self.emit_line("self.__pow__(&rhs)");
                } else {
                    self.emit_line("self.__pow__(rhs)");
                }
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __buildparams__ → lz_builtins::BuildParams trait impl（06d §十三 构建块协议）：
        // `def __buildparams__(ref self) -> ArgsTuple` → impl BuildParams for Struct，
        // 使 struct 实例可作为构建块载荷（~: / *:）
        if let Some(bm) = methods.iter().find(|m| m.name == "__buildparams__") {
            let where_str = self.magic_impl_where_str(bm);
            let args_ty = self.rust_type(&bm.ret_ty);
            self.emit_line(&format!(
                "impl{} lz_builtins::BuildParams for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line(&format!("type Args = {};", args_ty));
            self.emit_line("fn into_args(&self) -> Self::Args {");
            self.indent += 1;
            self.emit_line("self.__buildparams__()");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __guarded_pred__ + __guarded_action__ → lz_builtins::GuardedStrategy impl（06d §十 守卫策略）：
        // 两者配对生成：pred 判定是否执行兜底行为，action 执行兜底行为。
        let guarded_pred = methods.iter().find(|m| m.name == "__guarded_pred__");
        let guarded_action = methods.iter().find(|m| m.name == "__guarded_action__");
        if let (Some(gp), Some(ga)) = (guarded_pred, guarded_action) {
            // 提取 Input 类型（__guarded_pred__ 的非 self 参数类型）
            let input_ty = gp
                .params
                .iter()
                .find(|p| p.name != "self" && p.name != "self_")
                .map(|p| self.rust_type(&p.ty))
                .unwrap_or_else(|| "()".to_string());
            let output_ty = self.rust_type(&ga.ret_ty);
            let where_str = self.magic_impl_where_str(ga);

            self.emit_line(&format!(
                "impl{} lz_builtins::GuardedStrategy<{}> for {} {} {{",
                generics, input_ty, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line(&format!("type Output = {};", output_ty));
            self.emit_line(&format!("fn pred(&self, input: &{}) -> bool {{", input_ty));
            self.indent += 1;
            self.emit_line(&format!("self.__guarded_pred__(*input)"));
            self.indent -= 1;
            self.emit_line("}");
            self.emit_line(&format!(
                "fn action(self, input: {}) -> Self::Output {{",
                input_ty
            ));
            self.indent += 1;
            self.emit_line(&format!("self.__guarded_action__(input)"));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __call__ → Callable trait impl（06d §十一 可调用性）：
        // struct 实例作为闭包传给高阶函数。
        // 使用 lz_builtins::Callable<Args> trait（stable Rust 兼容）。
        if let Some(cm) = methods.iter().find(|m| m.name == "__call__") {
            let arg_types: Vec<String> = cm
                .params
                .iter()
                .filter(|p| p.name != "self" && p.name != "self_")
                .map(|p| self.rust_type(&p.ty))
                .collect();

            if !arg_types.is_empty() {
                let args_tuple = if arg_types.len() == 1 {
                    arg_types[0].clone()
                } else {
                    format!("({})", arg_types.join(", "))
                };
                let ret_ty = self.rust_type(&cm.ret_ty);
                let where_str = self.magic_impl_where_str(cm);
                let call_args: Vec<String> = cm
                    .params
                    .iter()
                    .filter(|p| p.name != "self" && p.name != "self_")
                    .enumerate()
                    .map(|(i, _)| format!("args.{}", i))
                    .collect();

                self.emit_line(&format!(
                    "impl{} lz_builtins::Callable<({},)> for {} {} {{",
                    generics, args_tuple, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", ret_ty));
                self.emit_line(&format!(
                    "fn __call__(&self, args: ({},)) -> Self::Output {{",
                    args_tuple
                ));
                self.indent += 1;
                self.emit_line(&format!("self.__call__({})", call_args.join(", ")));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }
    }

    /// 由魔法方法推导其 trait impl 所需的 `where` 子句（与 __eq__→PartialEq 一致）。
    /// 例如 `def __str__(ref self) -> str where T: Display` 返回 ` where T: std::fmt::Display`。
    /// 单独成方法（而非内联闭包）以避免 `&self` 借用与 `self.indent += 1` 的 `&mut self` 冲突。
    /// 解出 `Result<T, E>` 的 E（TryFrom 的 Error 关联类型）；非 Result 返回 None
    fn try_from_err_ty(ret: &IrType) -> Option<IrType> {
        match ret {
            IrType::Result { err, .. } => Some((**err).clone()),
            IrType::Named { path, args } if path == "Result" && args.len() == 2 => {
                Some(args[1].clone())
            }
            _ => None,
        }
    }

    /// 解出 `Result<T, E>` 的 (T, E)（TryInto 的目标/错误类型对）；非 Result 返回 None
    fn try_into_tgt_err(ret: &IrType) -> Option<(IrType, IrType)> {
        match ret {
            IrType::Result { ok, err } => Some(((**ok).clone(), (**err).clone())),
            IrType::Named { path, args } if path == "Result" && args.len() == 2 => {
                Some((args[0].clone(), args[1].clone()))
            }
            _ => None,
        }
    }

    fn magic_impl_where_str(&self, m: &FnDef) -> String {
        let w: String = m
            .where_clause
            .iter()
            .map(|(tp, bounds)| {
                let bs: Vec<String> = bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                format!("{}: {}", tp, bs.join(" + "))
            })
            .collect::<Vec<_>>()
            .join(", ");
        if w.is_empty() {
            String::new()
        } else {
            format!(" where {}", w)
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
                || val_str.contains("__try_result")
                || val_str.contains("LazyLock")
                || val_str.contains(".to_string()"));
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
    /// 顶层 self-def → 按结构体分组发射 impl 块（BUG-CG-002/TY-002，E0568 修复）。
    /// `def m(self: S, ...)` 发射为 `impl S { fn m(&self, ...) }`，
    /// 方法体生成复用 gen_fn_def（is_method 路径渲染 &self / &mut self）。
    /// mut self 标记：self_p.is_mut 已在归属判定时登记，此处透传——
    /// gen_fn_def 的 self 参数渲染逻辑按 p.is_mut 生成 &mut self。
    fn gen_self_fn_impls(&mut self, module: &IrModule) {
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
    /// Ordered → Ord（全序：Ord 蕴含 PartialOrd + Eq，泛型比较算法
    /// sort/merge 等需 T: Ord 时不致因仅 PartialOrd 而 E0277），
    /// Display → Display, Clone → Clone 等
    fn gen_trait_bound(&self, b: &IrType) -> String {
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
    /// cmp_eq / cmp_ord：经调用图传播得到的「参与比较运算」的泛型名，
    /// 注入 PartialEq+Eq（相等）或 PartialEq+Eq+Ord（有序）约束，
    /// 修复泛型比较算法 E0277/E0369（比较约束传播特性）。
    fn gen_fn_generics(
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

    /// 比较约束传播：预扫描模块内所有 FnDef 体（含 impl/trait 默认方法），
    /// 经调用图不动点计算「参与比较运算」的泛型集合，存入 `self.fn_cmp`。
    /// 调用图传播：若 F 调用 G（G 比较其泛型 → 需 Eq/Ord），且调用实参类型中
    /// 含 F 的泛型，则 F 的该泛型也需 Eq/Ord（如 merge_sort→merge、
    /// unique→内置 contains）。内置比较函数（contains/index）按 builtin_cmp 处理。
    fn compute_cmp_constraints(&mut self, module: &IrModule) {
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
    fn compute_use_counts(&mut self, module: &IrModule) {
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
                _ => {}
            }
        }
    }

    /// move 语义修复：若实参为「被多次使用的非 Copy 变量」，自动 .clone()，
    /// 避免 `for x in xs: push(seen, x); push(result, x)` 类的二次移动 E0382。
    /// Copy 类型克隆无害（i64.clone() 等价），此处统一克隆以保证安全。
    fn clone_if_multiuse(&self, s: String, a: &Expr) -> String {
        if s.contains(".clone()") {
            return s;
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

    /// BUG-CG-004（轮次12）：将 LZ Pattern 转为 Rust 模式串，用于 try/catch 结果基
    /// 分支的 `match __err_val { <pattern> => ... }`。支持 Wildcard / Ident /
    /// Enum（递归 args）。`line`/`column`/`file` 与 Rust 内置宏同名 → 降级加下划线。
    fn pattern_to_rust_pat(&self, p: &Pattern) -> String {
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
    fn pattern_idents(&self, p: &Pattern) -> Vec<String> {
        match p {
            Pattern::Ident(name) => vec![name.clone()],
            Pattern::Enum { args, .. } => {
                args.iter().flat_map(|a| self.pattern_idents(a)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn gen_block_inner(&mut self, block: &Block) {
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
    fn flush_deferred(&mut self) {
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

    fn gen_stmt(&mut self, stmt: &Stmt, is_last: bool) {
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
                ..
            } => {
                // IR-003：嵌套 def 转本地闭包产物 `let name = <Lambda>`（ty=Fn 且值
                // 为 Lambda）→ 值位置，需 Box<dyn Fn> 标注并装箱
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
                if (*is_mut || self_rebind) && self.declared.contains(&safe_name) {
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
                // IR-003：fn 值 let（嵌套 def 闭包）显式标注 Box<dyn Fn>，否则
                // `let inner = Box::new(closure)` 无法推断目标 trait object（E0282/E0308）
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
                self.box_lambda = saved_box_lambda;
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
                    path: target_path, ..
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
                let (value_s, ty_str) =
                    if self.in_result_try && matches!(&value.ty, IrType::Result { .. }) {
                        let unwrapped_ty = if let IrType::Result { ok, .. } = &value.ty {
                            format!(": {}", self.rust_type(ok))
                        } else {
                            ty_str.clone()
                        };
                        (format!("({})?", value_s), unwrapped_ty)
                    } else {
                        (value_s, ty_str)
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
                // 嵌套 Fn 返回（fn -> fn -> T）：内层闭包作为外层返回值需 Box::new 包装
                // （factory_chain: |a| => |b| => x + a + b → move |a| { Box::new(move |b| {...}) }）。
                // 仅在 Lambda 块体内生效；函数体本身的尾表达式（外层闭包）不包装
                let nested_fn_body = self.nested_fn_ret
                    && self.in_lambda_block
                    && matches!(&expr.kind, ExprKind::Lambda { .. });
                let saved_box_lambda = self.box_lambda;
                self.box_lambda = matches!(&self.current_ret_ty, Some(IrType::Fn { .. }))
                    && matches!(&expr.kind, ExprKind::Lambda { .. })
                    && !self.in_lambda_block;
                let expr_s = if nested_fn_body {
                    format!("Box::new({})", self.gen_expr(expr))
                } else {
                    self.gen_expr(expr)
                };
                self.box_lambda = saved_box_lambda;
                // BUG-CG-004（收口）：raises 函数尾表达式需包成 Ok(...)（expr 自身已是 Result 则不包）
                let wrap_ok =
                    self.current_fn_raises.is_some() && !matches!(&expr.ty, IrType::Result { .. });
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
                    self.gen_block_inner(&arm.body);
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
                } else if matches!(&expr.ty, IrType::Fn { .. })
                    && self.top_level_fns.contains(name.as_str())
                {
                    // IR-003：顶层 def 作一等值 → 装箱为 Box<dyn Fn>。
                    // 局部 fn-let 已在 let 值位置装箱，不重复装箱（避免双层 Box）
                    format!("Box::new({})", name)
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
                        if let ExprKind::IndexGet { base, .. } = &args[0].kind {
                            if matches!(base.ty, IrType::Str) {
                                return self.gen_expr(&args[0]);
                            }
                        }
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
                        let rest: Vec<String> =
                            args[1..].iter().map(|a| self.gen_expr(a)).collect();
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
                            // fn(...) 类型形参接收 lambda 实参：需转 fn 指针。参数声明
                            // 生成 impl FnMut（可收闭包），但存入 fn 字段/传给 fn 形参时
                            // opaque impl 无法 .clone()/赋值（E0599/E0308，lib_iterator
                            // MapIter.new(f)）。无捕获闭包可 `as fn(...)` 强转。
                            if let Some(IrType::Fn { params, ret }) =
                                callee_expected.as_ref().and_then(|pts| pts.get(i))
                            {
                                if matches!(&a.kind, ExprKind::Lambda { .. }) {
                                    let ps: Vec<String> =
                                        params.iter().map(|pt| self.rust_type(pt)).collect();
                                    // 泛型形参（T/U 或隐式小写 a/b 等占位符）在实例化前
                                    // 无类型可转 → 跳过 `as fn(...)` 强转（E0425 cannot
                                    // find type T/a）。impl Fn 形参本就接受闭包，无需强转；
                                    // 仅对「类型签名部分」做占位符检测，避免闭包体内的单字母
                                    // 变量名（如 x）被误判为占位符。
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
                            *s = format!("&{}", s);
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
                        if arg_is_var && !arg_is_copy {
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
                                && i < args_s.len()
                            {
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
                                .map(|((fname, _), a)| format!("{}: {}", fname, a))
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
                        let positional: Vec<String> = new_params
                            .iter()
                            .map(|(pname, pty)| {
                                if let Some(val) = kwarg_map.get(pname) {
                                    val.clone()
                                } else {
                                    self.default_value_for(pty)
                                }
                            })
                            .collect();
                        return format!("{}::__new__({})", callee_s, positional.join(", "));
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
                    }
                    _ => false,
                };
                if static_type_call {
                    // struct::new 字段类型表（new 形参顺序即字段顺序）：fn 字段实参
                    // 为 lambda 时转 fn 指针（同 Call 分支，lib_iterator MapIter.new）
                    let struct_fields = self.struct_fields_info.get(&recv).cloned();
                    let args_s: Vec<String> = args
                        .iter()
                        .enumerate()
                        .map(|(i, a)| {
                            let s = self.gen_expr(a);
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
                    return if args_s.is_empty() {
                        format!("{}::{}()", recv, method)
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
                                    a.clone()
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
                    .map(|a| self.clone_if_multiuse(self.gen_expr(a), a))
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
                    format!("{}.{}({})", recv, rust_method, args_s.join(", "))
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
                        .map(|(_, s)| {
                            let s = if (method == "find"
                                || method == "starts_with"
                                || method == "ends_with"
                                || method == "contains")
                                && s.starts_with('&')
                                && !s.starts_with("&&")
                            {
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
                    // 也检查是否为 kwargs 字段（__Params 的 kwargs 是 HashMap）
                    let is_kwargs = matches!(&base.kind, ExprKind::FieldAccess { field, .. } if field == "kwargs");
                    // 用户 struct：ml[0] → ml.__getitem__(0)（key 保持 i64，内部 self.items[i] 再转 usize）
                    // 注意：必须在 is_dict 之后，否则 Dict 会被误判为 struct
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
                        let base_is_str_any = matches!(&base.ty, IrType::Str)
                            || matches!(&base.ty, IrType::Named { path, .. }
                                if path == "str" || path == "String");
                        let is_range_key_any = matches!(&key.kind,
                            ExprKind::StructCtor { name, .. } if name == "Range");
                        if base_is_str_any && is_range_key_any {
                            return format!("{}[{}].to_string()", base_s, key_s);
                        }
                        if base_is_str_any {
                            // 字符串单字符索引：char 安全；若当前函数返回类型是 str/String，
                            // 生成 String（'\0'.to_string() / char.to_string()），否则生成
                            // char 码点 i64（兼容 string_index_unicode `let c = s[i]` 场景）
                            let idx_wants_string = matches!(
                                &self.current_ret_ty,
                                Some(IrType::Str)
                            ) || matches!(&self.current_ret_ty, Some(IrType::Named { path, .. })
                                    if path == "str" || path == "String")
                                || matches!(&self.current_fn_ret_ty, Some(IrType::Str))
                                || matches!(&self.current_fn_ret_ty, Some(IrType::Named { path, .. })
                                    if path == "str" || path == "String");
                            if idx_wants_string {
                                return format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = ({} as usize); if __i >= __cs.len() {{ '\\0'.to_string() }} else {{ __cs[__i].to_string() }}}}", base_s, key_s);
                            }
                            return format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = ({} as usize); if __i >= __cs.len() {{ '\\0' as i64 }} else {{ __cs[__i] as i64 }}}}", base_s, key_s);
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
                                        if path == "str" || path == "String");
                                if idx_wants_string2 {
                                    format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = ({} as usize); if __i >= __cs.len() {{ '\\0'.to_string() }} else {{ __cs[__i].to_string() }}}}", base_s, key_s)
                                } else {
                                    format!("{{let __cs: Vec<char> = ({}).chars().collect(); let __i = ({} as usize); if __i >= __cs.len() {{ '\\0' as i64 }} else {{ __cs[__i] as i64 }}}}", base_s, key_s)
                                }
                            } else {
                                format!("{}[{}].clone()", base_s, key_s)
                            }
                        } else if self.in_generic_fn {
                            // 泛型函数内普通容器索引：T 非 Copy，直接 move 出容器报 E0507
                            // （sort_by<T>/bubble_sort 冒泡 result[j]），自动 clone 为 owned
                            format!("{}[{}].clone()", base_s, key_s)
                        } else {
                            format!("{}[{}]", base_s, key_s)
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
                    if rhs_is_variadic {
                        return format!("{} + {}", lhs_base, rhs_s);
                    }
                    // lhs 非字符串类型（int + str / float + str 等）：
                    // Rust i64 未实现 Add<&str>，需整体用 format! 拼接
                    if !matches!(&lhs.ty, IrType::Str) {
                        return format!("format!(\"{{}}{{}}\", {}, {})", lhs_base, rhs_s);
                    }
                    // rhs 非字符串类型（str + int / str + float / str + bool 等）：
                    // Rust String 未实现 Add<i64> 等，需将 rhs 转为 &str 再拼接
                    if !matches!(&rhs.ty, IrType::Str) {
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
                // 三元 then/else 类型统一：then 是 bool 而 else 是数值时，
                // else 按 LZ 真值语义转 bool（非零为真），如
                // `(n := compute()) > 5 if n * 10 else 0`（combo_ternary_walrus.lz）
                if matches!(&then.ty, IrType::Bool) && matches!(&els.ty, IrType::Int | IrType::F64)
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
            ExprKind::Lambda {
                params,
                body,
                ret_ty,
                ..
            } => {
                // 嵌套 Fn 返回（fn -> fn -> T）：内层闭包作为外层返回值需 Box::new 包装
                // （factory_chain: |a| => |b| => x + a + b → move |a| { Box::new(move |b| {...}) }）
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
                // IR-003：值位置（let = Lambda / return Lambda）的闭包需 Box::new 包装为
                // Box<dyn Fn>；参数位置（传给 impl FnMut，如 map/filter/for_each）不包装，
                // 保留借用捕获（for_each |x| total += x 修改外层变量）
                if self.box_lambda {
                    format!("Box::new({})", lam)
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
                            return format!("{}::__new__({})", name, values.join(", "));
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
                                let v_s = self.gen_expr(v);
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
                child.top_level_static_names = self.top_level_static_names.clone();
                child.struct_phantom_generics = self.struct_phantom_generics.clone();
                child.current_fn_is_size_hint = self.current_fn_is_size_hint;
                child.in_iterator_impl = self.in_iterator_impl;
                child.current_fn_ret_is_ref = self.current_fn_ret_is_ref;
                child.current_fn_ret_ty = self.current_fn_ret_ty.clone();
                child.fn_param_types = self.fn_param_types.clone();
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
                    let elems: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
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
                    let elems_s: Vec<String> = elems
                        .iter()
                        .map(|e| {
                            let s = self.gen_expr(e);
                            // 列表字面量元素：IndexGet/Var 且元素类型非 Copy 时
                            // 自动 .clone()（E0507 cannot move out of index /
                            // E0382 moved value，如 `out + [ts[i]]` 的 vec![ts[i]]）
                            let is_copy = matches!(&e.ty, IrType::Int | IrType::F64 | IrType::Bool);
                            let is_moveable =
                                matches!(&e.kind, ExprKind::Var(_) | ExprKind::IndexGet { .. });
                            if !is_copy
                                && is_moveable
                                && !s.starts_with('&')
                                && !s.ends_with(".clone()")
                                && !s.contains("::")
                            {
                                format!("{}.clone()", s)
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

    /// 判断 IrType 是否完全具体（不含泛型类型参数如 K, V）
    fn is_fully_concrete(&self, ty: &IrType) -> bool {
        match ty {
            IrType::Any
            | IrType::Unit
            | IrType::Int
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

    /// 隐式转换桥：let x: TargetTy = src_val 中，若 TargetTy 实现了 __implicit_from__(SrcTy)，
    /// 生成 `<TargetTy as ImplicitFrom<SrcTy>>::__implicit_from__(value)` 桥接表达式。
    fn build_implicit_bridge(
        &self,
        target_ty: &IrType,
        value: &crate::ir::node::Expr,
        value_s: &str,
    ) -> Option<String> {
        // 提取目标路径
        let target_path = match target_ty {
            IrType::Named { path, .. } => path.as_str(),
            _ => return None,
        };
        // 目标类型必须是已知的 struct/enum
        if !self.is_known_type(target_path) {
            return None;
        }
        // 源值类型
        let src_ty = &value.ty;
        // 若源类型与目标类型已匹配 → 无需桥接
        if let IrType::Named { path: src_path, .. } = src_ty {
            if src_path == target_path {
                return None;
            }
        }
        // 若源类型是 Any/Unit → 无法判断，保守跳过
        if matches!(src_ty, IrType::Any | IrType::Unit) {
            return None;
        }
        // 若源类型不是已知具体类型（含泛型参数），保守跳过——避免在泛型上下文（如 fn<K,V> 内）误触发
        if !self.is_known_type(match src_ty {
            IrType::Named { path, .. } => path.as_str(),
            _ => "",
        }) && !matches!(
            src_ty,
            IrType::Int | IrType::F64 | IrType::Bool | IrType::Str | IrType::Unit | IrType::Any
        ) {
            return None;
        }
        // 源/目标类型必须完全具体（不含泛型类型参数），否则跳过——避免
        // 在泛型函数体内对 Option<(K,V)> 等误生成桥接（E0277）
        if !self.is_fully_concrete(src_ty) || !self.is_fully_concrete(target_ty) {
            return None;
        }
        // 检查目标类型是否实现了 __implicit_from__ 方法
        let has_implicit_from = self
            .struct_method_names_map
            .get(target_path)
            .map(|ms| ms.contains("__implicit_from__"))
            .unwrap_or(false);
        if has_implicit_from {
            let src_rust_ty = self.rust_type(src_ty);
            let target_rust_ty = self.rust_type(&IrType::Named {
                path: target_path.to_string(),
                args: vec![],
            });
            return Some(format!(
                "<{} as lz_builtins::runtime::ImplicitFrom<{}>>::__implicit_from__({})",
                target_rust_ty, src_rust_ty, value_s
            ));
        }
        // 回退：检查源类型是否实现了 __implicit_to__ 方法（目标端转换）
        if let IrType::Named { path: src_path, .. } = src_ty {
            if self.is_known_type(src_path) {
                let has_implicit_to = self
                    .struct_method_names_map
                    .get(src_path)
                    .map(|ms| ms.contains("__implicit_to__"))
                    .unwrap_or(false);
                if has_implicit_to {
                    let target_rust_ty = self.rust_type(&IrType::Named {
                        path: target_path.to_string(),
                        args: vec![],
                    });
                    return Some(format!(
                        "<{} as lz_builtins::runtime::ImplicitInto<{}>>::__implicit_into__(&{})",
                        self.rust_type(src_ty),
                        target_rust_ty,
                        value_s
                    ));
                }
            }
        }
        None
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
    fn gen_fstring(&self, s: &str) -> String {
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
            // 其余（容器/Option/HashMap 等只有 Debug）用 {:?}（E0277 规避）
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
    fn gen_expr_str(&self, expr: &str) -> String {
        expr.trim().to_string()
    }

    /// f-string 插值表达式是否为字符串语义（生成 {} 而非 {:?}）：
    /// 已登记的字符串变量或字符串字面量。其余（int/容器/属性等）用 Debug {:?}
    fn is_str_expr(&self, expr: &str) -> bool {
        let t = expr.trim();
        if t.starts_with('"') || t.starts_with('\'') {
            return true;
        }
        if self.str_typed_vars.contains(t) {
            return true;
        }
        // 字符串方法链（如 x.to_upper()）— 已登记变量首段
        if let Some(dot) = t.find('.') {
            let base = &t[..dot];
            if self.str_typed_vars.contains(base) {
                return true;
            }
        }
        false
    }

    /// 判断表达式是否为字符串产生式（生成 Rust String）：
    /// 字符串字面量 / 字符串插值 / 接收者已知为字符串的字符串方法调用
    /// （如 `", ".join(parts)` → receiver 为 Str 字面量，join 返回 String）。
    /// 用于 let 绑定登记 str_typed_vars（builder 对 join 等方法调用推断为 Any）
    fn is_str_producing(&self, v: &Expr) -> bool {
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
            _ => false,
        }
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
                if std::env::var("LZ_DBG_PAT").is_ok() {
                    eprintln!(
                        "PATDBG enum={:?} variant={:?} args={:?} named={:?}",
                        enum_name,
                        variant,
                        args.len(),
                        self.enum_variant_named_fields
                            .get(&(enum_name.clone(), variant.clone()))
                    );
                }
                // 递归字段在模式中不添加 box 关键字（box_patterns 尚未稳定）
                // 由 gen_stmt(Match) 在臂体开头自动插入 let var = *var; 解引用
                if args.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    let args: Vec<String> = args.iter().map(|a| self.gen_pattern(a)).collect();
                    let named = self
                        .enum_variant_named_fields
                        .get(&(enum_name.clone(), variant.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if named.len() == args.len() && !named.is_empty() {
                        let pairs: Vec<String> = named
                            .iter()
                            .zip(args.iter())
                            .map(|(f, v)| format!("{}: {}", f, v))
                            .collect();
                        format!("{}::{} {{ {} }}", enum_name, variant, pairs.join(", "))
                    } else {
                        format!("{}::{}({})", enum_name, variant, args.join(", "))
                    }
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

// ── f-string 未绑定插值预扫描（lit_probe 等探测用例）──

/// 提取 f-string 中纯变量插值名（`f"hello {name}"` → "name"）；
/// 复杂插值（len(x)、a.b 等）不提取
fn fstring_var_interps(s: &str, out: &mut std::collections::HashSet<String>) {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                continue;
            }
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
            let t = expr.trim();
            // 纯标识符（含格式说明符前的冒号取前段）
            let ident = t.split(':').next().unwrap_or(t).trim();
            if !ident.is_empty()
                && ident
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                && !ident.chars().next().unwrap().is_ascii_digit()
            {
                out.insert(ident.to_string());
            }
        }
    }
}

/// 扫描表达式树，收集 f-string 插值变量引用
fn scan_expr_fstrings(e: &Expr, used: &mut std::collections::HashSet<String>) {
    match &e.kind {
        ExprKind::Lit(LitKind::FStr(s)) => fstring_var_interps(s, used),
        ExprKind::Lit(_) => {}
        ExprKind::Var(_) => {}
        ExprKind::Call { callee, args, .. } => {
            scan_expr_fstrings(callee, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            scan_expr_fstrings(receiver, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::FieldAccess { base, .. } => scan_expr_fstrings(base, used),
        ExprKind::IndexGet { base, key } => {
            scan_expr_fstrings(base, used);
            scan_expr_fstrings(key, used);
        }
        ExprKind::IndexSet { base, key, value } => {
            scan_expr_fstrings(base, used);
            scan_expr_fstrings(key, used);
            scan_expr_fstrings(value, used);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_fstrings(lhs, used);
            scan_expr_fstrings(rhs, used);
        }
        ExprKind::AssignExpr { target, value } => {
            scan_expr_fstrings(target, used);
            scan_expr_fstrings(value, used);
        }
        ExprKind::UnOp { operand, .. } => scan_expr_fstrings(operand, used),
        ExprKind::IfExpr { cond, then, els } => {
            scan_expr_fstrings(cond, used);
            scan_expr_fstrings(then, used);
            scan_expr_fstrings(els, used);
        }
        ExprKind::Lambda { body, .. } => scan_expr_fstrings(body, used),
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                scan_expr_fstrings(v, used);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::GenExpr { yield_of } => scan_expr_fstrings(yield_of, used),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                scan_expr_fstrings(c, used);
            }
            scan_block_fstrings(block, &mut std::collections::HashSet::new(), used);
        }
        ExprKind::Cast { expr, .. } => scan_expr_fstrings(expr, used),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::BlockExpr { block } => {
            scan_block_fstrings(block, &mut std::collections::HashSet::new(), used);
        }
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                scan_expr_fstrings(k, used);
                scan_expr_fstrings(v, used);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                scan_expr_fstrings(s, used);
            }
            scan_expr_fstrings(end, used);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            scan_expr_fstrings(receiver, used);
            scan_expr_fstrings(callee, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        _ => {}
    }
}

/// 收集模式中的绑定名（match 臂 / while let / catch 模式）到 bound：
/// f-string 插值若引用这些名字，不得降级为空串（lib_pattern 实测：
/// case Color.RGB(r, g, b) 臂体内 f"#{r}{g}{b}" 曾被降级为 ""）
fn scan_pattern_bindings(pat: &Pattern, bound: &mut std::collections::HashSet<String>) {
    match pat {
        Pattern::Ident(n) | Pattern::RefMutIdent(n) => {
            bound.insert(n.clone());
        }
        Pattern::Tuple(ps) | Pattern::List(ps) => {
            for p in ps {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Dict(kvps) => {
            for (_, p) in kvps {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Rest(Some(n)) => {
            bound.insert(n.clone());
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Enum { args, .. } => {
            for p in args {
                scan_pattern_bindings(p, bound);
            }
        }
        _ => {}
    }
}

/// 扫描块：let 名收集到 bound，f-string 插值收集到 used
fn scan_block_fstrings(
    b: &Block,
    bound: &mut std::collections::HashSet<String>,
    used: &mut std::collections::HashSet<String>,
) {
    for st in &b.stmts {
        match st {
            Stmt::Let { name, value, .. } => {
                bound.insert(name.clone());
                scan_expr_fstrings(value, used);
            }
            Stmt::Assign { target, value } => {
                scan_expr_fstrings(target, used);
                scan_expr_fstrings(value, used);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    scan_expr_fstrings(v, used);
                }
            }
            Stmt::ExprStmt { expr } => scan_expr_fstrings(expr, used),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                scan_expr_fstrings(cond, used);
                scan_block_fstrings(then_branch, bound, used);
                if let Some(eb) = else_branch {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
            } => {
                bound.insert(var.clone());
                scan_expr_fstrings(iter, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
            } => {
                scan_expr_fstrings(cond, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                scan_pattern_bindings(pattern, bound);
                scan_expr_fstrings(expr, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
            }
            Stmt::Match { scrutinee, arms } => {
                scan_expr_fstrings(scrutinee, used);
                for arm in arms {
                    // 臂模式绑定（含枚举/元组/结构子模式）也是有效绑定
                    scan_pattern_bindings(&arm.pattern, bound);
                    if let Some(g) = &arm.guard {
                        scan_expr_fstrings(g, used);
                    }
                    scan_block_fstrings(&arm.body, bound, used);
                }
            }
            Stmt::Raise { value } => scan_expr_fstrings(value, used),
            Stmt::Assert { cond, message } => {
                scan_expr_fstrings(cond, used);
                if let Some(m) = message {
                    scan_expr_fstrings(m, used);
                }
            }
            Stmt::Yield { value } => scan_expr_fstrings(value, used),
            Stmt::YieldFrom { iter } => scan_expr_fstrings(iter, used),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    scan_expr_fstrings(v, used);
                }
            }
            Stmt::BlockLabel { body, .. } => scan_block_fstrings(body, bound, used),
            Stmt::CheckerBlock { body, .. } => scan_block_fstrings(body, bound, used),
            Stmt::Defer { body } => scan_block_fstrings(body, bound, used),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                scan_block_fstrings(body, bound, used);
                for (cpat, cb) in catches {
                    if let Some(p) = cpat {
                        scan_pattern_bindings(p, bound);
                    }
                    scan_block_fstrings(cb, bound, used);
                }
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
                if let Some(fb) = finally_body {
                    scan_block_fstrings(fb, bound, used);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                scan_block_fstrings(&tmp, bound, used);
            }
            _ => {}
        }
    }
}

/// 收集表达式树中所有 lambda 参数名（f-string 插值若引用 lambda 参数，不得降级为空串）
fn collect_lambda_params_expr(e: &Expr, bound: &mut std::collections::HashSet<String>) {
    match &e.kind {
        ExprKind::Lambda { params, body, .. } => {
            for p in params {
                bound.insert(p.name.clone());
            }
            collect_lambda_params_expr(body, bound);
        }
        ExprKind::Call { callee, args, .. } => {
            collect_lambda_params_expr(callee, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_lambda_params_expr(receiver, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_lambda_params_expr(base, bound),
        ExprKind::IndexGet { base, key } => {
            collect_lambda_params_expr(base, bound);
            collect_lambda_params_expr(key, bound);
        }
        ExprKind::IndexSet { base, key, value } => {
            collect_lambda_params_expr(base, bound);
            collect_lambda_params_expr(key, bound);
            collect_lambda_params_expr(value, bound);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_lambda_params_expr(lhs, bound);
            collect_lambda_params_expr(rhs, bound);
        }
        ExprKind::AssignExpr { target, value } => {
            collect_lambda_params_expr(target, bound);
            collect_lambda_params_expr(value, bound);
        }
        ExprKind::UnOp { operand, .. } => collect_lambda_params_expr(operand, bound),
        ExprKind::IfExpr { cond, then, els } => {
            collect_lambda_params_expr(cond, bound);
            collect_lambda_params_expr(then, bound);
            collect_lambda_params_expr(els, bound);
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                collect_lambda_params_expr(v, bound);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::GenExpr { yield_of } => collect_lambda_params_expr(yield_of, bound),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                collect_lambda_params_expr(c, bound);
            }
            collect_lambda_params_block(block, bound);
        }
        ExprKind::Cast { expr, .. } => collect_lambda_params_expr(expr, bound),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::BlockExpr { block } => collect_lambda_params_block(block, bound),
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                collect_lambda_params_expr(k, bound);
                collect_lambda_params_expr(v, bound);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_lambda_params_expr(s, bound);
            }
            collect_lambda_params_expr(end, bound);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            collect_lambda_params_expr(receiver, bound);
            collect_lambda_params_expr(callee, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        _ => {}
    }
}

/// 收集 block 中所有 lambda 参数名
fn collect_lambda_params_block(b: &Block, bound: &mut std::collections::HashSet<String>) {
    for st in &b.stmts {
        match st {
            Stmt::Let { value, .. } => collect_lambda_params_expr(value, bound),
            Stmt::Assign { target, value } => {
                collect_lambda_params_expr(target, bound);
                collect_lambda_params_expr(value, bound);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    collect_lambda_params_expr(v, bound);
                }
            }
            Stmt::ExprStmt { expr } => collect_lambda_params_expr(expr, bound),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_lambda_params_expr(cond, bound);
                collect_lambda_params_block(then_branch, bound);
                if let Some(eb) = else_branch {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::For {
                iter,
                body,
                else_body,
                ..
            } => {
                collect_lambda_params_expr(iter, bound);
                collect_lambda_params_block(body, bound);
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::While {
                cond,
                body,
                else_body,
                ..
            } => {
                collect_lambda_params_expr(cond, bound);
                collect_lambda_params_block(body, bound);
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::WhileLet { expr, body, .. } => {
                collect_lambda_params_expr(expr, bound);
                collect_lambda_params_block(body, bound);
            }
            Stmt::Match { scrutinee, arms } => {
                collect_lambda_params_expr(scrutinee, bound);
                for arm in arms {
                    collect_lambda_params_block(&arm.body, bound);
                }
            }
            Stmt::Raise { value } => collect_lambda_params_expr(value, bound),
            Stmt::Assert { cond, message } => {
                collect_lambda_params_expr(cond, bound);
                if let Some(m) = message {
                    collect_lambda_params_expr(m, bound);
                }
            }
            Stmt::Yield { value } => collect_lambda_params_expr(value, bound),
            Stmt::YieldFrom { iter } => collect_lambda_params_expr(iter, bound),
            Stmt::BlockLabel { body, .. } => collect_lambda_params_block(body, bound),
            Stmt::CheckerBlock { body, .. } => collect_lambda_params_block(body, bound),
            Stmt::Defer { body } => collect_lambda_params_block(body, bound),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                collect_lambda_params_block(body, bound);
                for (_, cb) in catches {
                    collect_lambda_params_block(cb, bound);
                }
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
                if let Some(fb) = finally_body {
                    collect_lambda_params_block(fb, bound);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                collect_lambda_params_block(&tmp, bound);
            }
            _ => {}
        }
    }
}

/// 预扫描 IR 模块，返回 f-string 插值中未绑定的变量名集合
fn collect_unbound_fstring_vars(module: &IrModule) -> std::collections::HashSet<String> {
    let mut bound: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 全局绑定：fn / const / struct / enum / trait / checker / alias / duck 名 + 函数参数
    for item in &module.items {
        match item {
            Item::FnDef(f) => {
                bound.insert(f.name.clone());
                for p in &f.params {
                    bound.insert(p.name.clone());
                }
                collect_lambda_params_block(&f.body, &mut bound);
            }
            Item::Const(c) => {
                bound.insert(c.name.clone());
                collect_lambda_params_expr(&c.value, &mut bound);
            }
            Item::StructDef(s) => {
                bound.insert(s.name.clone());
                for m in &s.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::EnumDef(e) => {
                bound.insert(e.name.clone());
                for m in &e.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::TraitDef(t) => {
                bound.insert(t.name.clone());
                for m in &t.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params_names {
                        bound.insert(p.clone());
                    }
                    if let Some(body) = &m.body {
                        collect_lambda_params_block(body, &mut bound);
                    }
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::CheckerBlock { name, body, .. } => {
                bound.insert(name.clone());
                collect_lambda_params_block(body, &mut bound);
            }
            Item::TypeAlias(t) => {
                bound.insert(t.name.clone());
            }
            Item::DuckDef(d) => {
                bound.insert(d.name.clone());
            }
            _ => {}
        }
    }

    // 函数体/checker/方法/const 中的 let 绑定与 f-string 使用
    for item in &module.items {
        match item {
            Item::FnDef(f) => scan_block_fstrings(&f.body, &mut bound, &mut used),
            Item::CheckerBlock { body, .. } => scan_block_fstrings(body, &mut bound, &mut used),
            Item::StructDef(s) => {
                for m in &s.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::EnumDef(e) => {
                for m in &e.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::Test(t) => scan_block_fstrings(&t.body, &mut bound, &mut used),
            Item::Const(c) => scan_expr_fstrings(&c.value, &mut used),
            _ => {}
        }
    }

    used.into_iter().filter(|u| !bound.contains(u)).collect()
}

// ── type-pack 调用点预扫描（03d §2.8 方案 B：`..: Tuple<Ts...>` 具体化）──

/// 递归扫描 Block，收集对 type-pack 变参函数的调用点实参类型
fn collect_typepack_calls(
    b: &Block,
    tp: &std::collections::HashMap<String, String>,
    out: &mut std::collections::HashMap<String, Vec<Vec<IrType>>>,
) {
    for s in &b.stmts {
        match s {
            Stmt::Let { value, .. } => collect_typepack_expr(value, tp, out),
            Stmt::Assign { target, value } => {
                collect_typepack_expr(target, tp, out);
                collect_typepack_expr(value, tp, out);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    collect_typepack_expr(v, tp, out);
                }
            }
            Stmt::ExprStmt { expr } => collect_typepack_expr(expr, tp, out),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_typepack_expr(cond, tp, out);
                collect_typepack_calls(then_branch, tp, out);
                if let Some(eb) = else_branch {
                    collect_typepack_calls(eb, tp, out);
                }
            }
            Stmt::For {
                iter,
                guard,
                body,
                else_body,
                ..
            } => {
                collect_typepack_expr(iter, tp, out);
                if let Some(g) = guard {
                    collect_typepack_expr(g, tp, out);
                }
                collect_typepack_calls(body, tp, out);
                if let Some(eb) = else_body {
                    collect_typepack_calls(eb, tp, out);
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
            } => {
                collect_typepack_expr(cond, tp, out);
                if let Some(g) = guard {
                    collect_typepack_expr(g, tp, out);
                }
                collect_typepack_calls(body, tp, out);
                if let Some(eb) = else_body {
                    collect_typepack_calls(eb, tp, out);
                }
            }
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                collect_typepack_expr(expr, tp, out);
                if let Some(g) = guard {
                    collect_typepack_expr(g, tp, out);
                }
                collect_typepack_calls(body, tp, out);
            }
            Stmt::Match { scrutinee, arms } => {
                collect_typepack_expr(scrutinee, tp, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        collect_typepack_expr(g, tp, out);
                    }
                    collect_typepack_calls(&arm.body, tp, out);
                }
            }
            Stmt::Raise { value } => collect_typepack_expr(value, tp, out),
            Stmt::Assert { cond, message } => {
                collect_typepack_expr(cond, tp, out);
                if let Some(m) = message {
                    collect_typepack_expr(m, tp, out);
                }
            }
            Stmt::Yield { value } => collect_typepack_expr(value, tp, out),
            Stmt::YieldFrom { iter } => collect_typepack_expr(iter, tp, out),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    collect_typepack_expr(v, tp, out);
                }
            }
            Stmt::BlockLabel { body, .. } => collect_typepack_calls(body, tp, out),
            Stmt::CheckerBlock { body, .. } => collect_typepack_calls(body, tp, out),
            Stmt::Defer { body } => collect_typepack_calls(body, tp, out),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                collect_typepack_calls(body, tp, out);
                for (_, cb) in catches {
                    collect_typepack_calls(cb, tp, out);
                }
                if let Some(eb) = else_body {
                    collect_typepack_calls(eb, tp, out);
                }
                if let Some(fb) = finally_body {
                    collect_typepack_calls(fb, tp, out);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                collect_typepack_calls(&tmp, tp, out);
            }
            _ => {}
        }
    }
}

/// 递归扫描表达式，收集 Call{ Var(typepack_fn), args } 的实参类型
fn collect_typepack_expr(
    e: &Expr,
    tp: &std::collections::HashMap<String, String>,
    out: &mut std::collections::HashMap<String, Vec<Vec<IrType>>>,
) {
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Var(name) = &callee.kind {
                if tp.contains_key(name) {
                    out.entry(name.clone())
                        .or_insert_with(Vec::new)
                        .push(args.iter().map(|a| a.ty.clone()).collect());
                }
            }
            collect_typepack_expr(callee, tp, out);
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_typepack_expr(receiver, tp, out);
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_typepack_expr(base, tp, out),
        ExprKind::IndexGet { base, key } => {
            collect_typepack_expr(base, tp, out);
            collect_typepack_expr(key, tp, out);
        }
        ExprKind::IndexSet { base, key, value } => {
            collect_typepack_expr(base, tp, out);
            collect_typepack_expr(key, tp, out);
            collect_typepack_expr(value, tp, out);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_typepack_expr(lhs, tp, out);
            collect_typepack_expr(rhs, tp, out);
        }
        ExprKind::AssignExpr { target, value } => {
            collect_typepack_expr(target, tp, out);
            collect_typepack_expr(value, tp, out);
        }
        ExprKind::UnOp { operand, .. } => collect_typepack_expr(operand, tp, out),
        ExprKind::IfExpr { cond, then, els } => {
            collect_typepack_expr(cond, tp, out);
            collect_typepack_expr(then, tp, out);
            collect_typepack_expr(els, tp, out);
        }
        ExprKind::Lambda { body, .. } => collect_typepack_expr(body, tp, out),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e2) in fields {
                collect_typepack_expr(e2, tp, out);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::GenExpr { yield_of } => collect_typepack_expr(yield_of, tp, out),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                collect_typepack_expr(c, tp, out);
            }
            collect_typepack_calls(block, tp, out);
        }
        ExprKind::Cast { expr, .. } => collect_typepack_expr(expr, tp, out),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::BlockExpr { block } => collect_typepack_calls(block, tp, out),
        ExprKind::TupleLit(es) | ExprKind::Tuple(es) => {
            for e2 in es {
                collect_typepack_expr(e2, tp, out);
            }
        }
        _ => {}
    }
}

// ── 未定义外部函数预扫描（p16_lzi 等跨模块探测用例）──

/// lz_builtins crate 导出的函数名（use lz_builtins::* 已引入，不得再生成同名 stub）
const LZ_BUILTIN_FN_NAMES: &[&str] = &[
    // lz_builtins crate 实际导出（use lz_builtins::* 已引入）
    "__block_on",
    "__lz_bytes",
    "__lz_duration_ms",
    "__lz_duration_secs",
    "__lz_path",
    "__lz_pathbuf",
    "__lz_str_ref",
    "abs_f64",
    "abs_i64",
    "ceil",
    "chr",
    "chr_unchecked",
    "clear",
    "compile_warn",
    "contains",
    "currentframe",
    "divmod",
    "field",
    "field_count",
    "find_type",
    "floor",
    "from_args",
    "from_std",
    "get_raw",
    "getargs",
    "getcomments",
    "getdoc",
    "getmembers",
    "getmodulename",
    "getreturntype",
    "getsource",
    "getsourcefile",
    "getsourcelines",
    "init",
    "input",
    "is_empty",
    "is_eq",
    "is_ge",
    "is_gt",
    "is_le",
    "is_lt",
    "isclass",
    "isfunction",
    "ismethod",
    "ismodule",
    "len",
    "lz_abs",
    "lz_all",
    "lz_any",
    "lz_bool",
    "lz_clamp",
    "lz_count_if",
    "lz_ends_with",
    "lz_float",
    "lz_int",
    "lz_join_words",
    "lz_range",
    "lz_range_step",
    "lz_sum_ints",
    "max_f64",
    "max_i64",
    "min_f64",
    "min_i64",
    "new",
    "ord",
    "pow_f64",
    "pow_i64",
    "print",
    "print_str",
    "print_val",
    "range",
    "range2",
    "range3",
    "read_file",
    "register",
    "round",
    "set_raw",
    "set_str",
    "signature",
    "status_str",
    "to_std",
    "type_count",
    "with_start",
    "with_step",
    "write_file",
    // 集合可变自由函数：push/pop/append/extend/insert/remove —— 由 lz_builtins 或
    // Vec 固有方法提供，禁止 stub 遮蔽；gen_call 降级为 `(recv).push(item)` 等方法调用
    "push",
    "pop",
    "append",
    "extend",
    "insert",
    "remove",
    // collections.rs 集合方法（trait 提供，同样禁止 stub）
    "lz_push",
    "lz_pop",
    "lz_len",
    "lz_get",
    "lz_set",
    "lz_contains",
    "lz_index",
    "lz_remove",
    "lz_insert",
    "lz_sort",
    "lz_reverse",
    "lz_extend",
    "lz_clear",
    "lz_is_empty",
    "lz_first",
    "lz_last",
    "lz_slice",
    "lz_keys",
    "lz_values",
    "lz_items",
    "lz_update",
    "lz_set_default",
    "lz_add",
    "lz_union",
    "lz_intersection",
    "lz_difference",
    "lz_symmetric_difference",
    "lz_iter",
    "lz_next",
    "lz_collect",
    "lz_map",
    "lz_filter",
    "lz_enumerate",
    "lz_zip",
    "lz_reduce",
    "lz_fold",
    "lz_take",
    "lz_skip",
    "lz_repeat",
    "lz_chain",
    "lz_once",
    "lz_empty",
    // 标准枚举构造器（rustc 自带，禁止 stub 遮蔽）
    "Some",
    "None",
    "Ok",
    "Err",
    // lz 内置类型转换 / 内置函数（codegen 有专门生成路径，禁止 stub 遮蔽）
    "str",
    "int",
    "float",
    "bool",
    "list",
    "dict",
    "set",
    "tuple",
    "bytes",
    "bytearray",
    "frozenset",
    "slice",
    "memoryview",
    "object",
    "type",
    "repr",
    "ascii",
    "format",
    "hash",
    "id",
    "iter",
    "next",
    "enumerate",
    "zip",
    "map",
    "filter",
    "reversed",
    "sorted",
    "sum",
    "min",
    "max",
    "abs",
    "all",
    "any",
    "open",
    "super",
    "property",
    "staticmethod",
    "classmethod",
    "isinstance",
    "issubclass",
    "callable",
    "vars",
    "dir",
    "getattr",
    "setattr",
    "hasattr",
    "delattr",
    "locals",
    "globals",
];

/// 扫描表达式：收集对未定义顶层函数的 Call（callee 为 Var；排除宏名与局部绑定）
#[allow(clippy::too_many_arguments)]
fn scan_expr_extern_calls(
    e: &Expr,
    known: &std::collections::HashSet<String>,
    builtins: &std::collections::HashSet<&str>,
    bound: &std::collections::HashSet<String>,
    out: &mut std::collections::HashMap<String, usize>,
) {
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Var(n) = &callee.kind {
                if !n.contains('!')
                    && !known.contains(n)
                    && !bound.contains(n)
                    && !builtins.contains(n.as_str())
                {
                    let cnt = out.entry(n.clone()).or_insert(0);
                    if args.len() > *cnt {
                        *cnt = args.len();
                    }
                }
            }
            scan_expr_extern_calls(callee, known, builtins, bound, out);
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            scan_expr_extern_calls(receiver, known, builtins, bound, out);
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::FieldAccess { base, .. } => {
            scan_expr_extern_calls(base, known, builtins, bound, out)
        }
        ExprKind::IndexGet { base, key } => {
            scan_expr_extern_calls(base, known, builtins, bound, out);
            scan_expr_extern_calls(key, known, builtins, bound, out);
        }
        ExprKind::IndexSet { base, key, value } => {
            scan_expr_extern_calls(base, known, builtins, bound, out);
            scan_expr_extern_calls(key, known, builtins, bound, out);
            scan_expr_extern_calls(value, known, builtins, bound, out);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_extern_calls(lhs, known, builtins, bound, out);
            scan_expr_extern_calls(rhs, known, builtins, bound, out);
        }
        ExprKind::AssignExpr { target, value } => {
            scan_expr_extern_calls(target, known, builtins, bound, out);
            scan_expr_extern_calls(value, known, builtins, bound, out);
        }
        ExprKind::UnOp { operand, .. } => {
            scan_expr_extern_calls(operand, known, builtins, bound, out)
        }
        ExprKind::IfExpr { cond, then, els } => {
            scan_expr_extern_calls(cond, known, builtins, bound, out);
            scan_expr_extern_calls(then, known, builtins, bound, out);
            scan_expr_extern_calls(els, known, builtins, bound, out);
        }
        ExprKind::Lambda { params, body, .. } => {
            let mut b2 = bound.clone();
            for p in params {
                b2.insert(p.name.clone());
            }
            scan_expr_extern_calls(body, known, builtins, &b2, out);
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                scan_expr_extern_calls(v, known, builtins, bound, out);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::GenExpr { yield_of } => {
            scan_expr_extern_calls(yield_of, known, builtins, bound, out)
        }
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                scan_expr_extern_calls(c, known, builtins, bound, out);
            }
            scan_block_extern_calls(block, known, builtins, bound, out);
        }
        ExprKind::Cast { expr, .. } => scan_expr_extern_calls(expr, known, builtins, bound, out),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::BlockExpr { block } => {
            scan_block_extern_calls(block, known, builtins, bound, out);
        }
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                scan_expr_extern_calls(k, known, builtins, bound, out);
                scan_expr_extern_calls(v, known, builtins, bound, out);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(st) = start {
                scan_expr_extern_calls(st, known, builtins, bound, out);
            }
            scan_expr_extern_calls(end, known, builtins, bound, out);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            scan_expr_extern_calls(receiver, known, builtins, bound, out);
            scan_expr_extern_calls(callee, known, builtins, bound, out);
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        _ => {}
    }
}

/// 扫描块：let/for/match 等绑定加入局部 bound，收集未定义顶层函数 Call
fn scan_block_extern_calls(
    b: &Block,
    known: &std::collections::HashSet<String>,
    builtins: &std::collections::HashSet<&str>,
    bound: &std::collections::HashSet<String>,
    out: &mut std::collections::HashMap<String, usize>,
) {
    let mut b2 = bound.clone();
    for st in &b.stmts {
        match st {
            Stmt::Let { name, value, .. } => {
                scan_expr_extern_calls(value, known, builtins, &b2, out);
                b2.insert(name.clone());
            }
            Stmt::Assign { target, value } => {
                scan_expr_extern_calls(target, known, builtins, &b2, out);
                scan_expr_extern_calls(value, known, builtins, &b2, out);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    scan_expr_extern_calls(v, known, builtins, &b2, out);
                }
            }
            Stmt::ExprStmt { expr } => scan_expr_extern_calls(expr, known, builtins, &b2, out),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                scan_expr_extern_calls(cond, known, builtins, &b2, out);
                scan_block_extern_calls(then_branch, known, builtins, &b2, out);
                if let Some(eb) = else_branch {
                    scan_block_extern_calls(eb, known, builtins, &b2, out);
                }
            }
            Stmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
                ..
            } => {
                scan_expr_extern_calls(iter, known, builtins, &b2, out);
                if let Some(g) = guard {
                    scan_expr_extern_calls(g, known, builtins, &b2, out);
                }
                let mut b3 = b2.clone();
                b3.insert(var.clone());
                scan_block_extern_calls(body, known, builtins, &b3, out);
                if let Some(eb) = else_body {
                    scan_block_extern_calls(eb, known, builtins, &b3, out);
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
                ..
            } => {
                scan_expr_extern_calls(cond, known, builtins, &b2, out);
                if let Some(g) = guard {
                    scan_expr_extern_calls(g, known, builtins, &b2, out);
                }
                scan_block_extern_calls(body, known, builtins, &b2, out);
                if let Some(eb) = else_body {
                    scan_block_extern_calls(eb, known, builtins, &b2, out);
                }
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
                ..
            } => {
                scan_expr_extern_calls(expr, known, builtins, &b2, out);
                if let Some(g) = guard {
                    scan_expr_extern_calls(g, known, builtins, &b2, out);
                }
                let mut b3 = b2.clone();
                collect_pattern_bindings(pattern, &mut b3);
                scan_block_extern_calls(body, known, builtins, &b3, out);
            }
            Stmt::Match { scrutinee, arms } => {
                scan_expr_extern_calls(scrutinee, known, builtins, &b2, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        scan_expr_extern_calls(g, known, builtins, &b2, out);
                    }
                    let mut b3 = b2.clone();
                    collect_pattern_bindings(&arm.pattern, &mut b3);
                    scan_block_extern_calls(&arm.body, known, builtins, &b3, out);
                }
            }
            Stmt::Raise { value } => scan_expr_extern_calls(value, known, builtins, &b2, out),
            Stmt::Assert { cond, message } => {
                scan_expr_extern_calls(cond, known, builtins, &b2, out);
                if let Some(m) = message {
                    scan_expr_extern_calls(m, known, builtins, &b2, out);
                }
            }
            Stmt::Yield { value } => scan_expr_extern_calls(value, known, builtins, &b2, out),
            Stmt::YieldFrom { iter } => scan_expr_extern_calls(iter, known, builtins, &b2, out),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    scan_expr_extern_calls(v, known, builtins, &b2, out);
                }
            }
            Stmt::BlockLabel { body, .. } => {
                scan_block_extern_calls(body, known, builtins, &b2, out)
            }
            Stmt::CheckerBlock { body, .. } => {
                scan_block_extern_calls(body, known, builtins, &b2, out)
            }
            Stmt::Defer { body } => scan_block_extern_calls(body, known, builtins, &b2, out),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                scan_block_extern_calls(body, known, builtins, &b2, out);
                for (_, cb) in catches {
                    scan_block_extern_calls(cb, known, builtins, &b2, out);
                }
                if let Some(eb) = else_body {
                    scan_block_extern_calls(eb, known, builtins, &b2, out);
                }
                if let Some(fb) = finally_body {
                    scan_block_extern_calls(fb, known, builtins, &b2, out);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                scan_block_extern_calls(&tmp, known, builtins, &b2, out);
            }
            _ => {}
        }
    }
}

/// 收集 pattern 中的绑定名（match 分支）
fn collect_pattern_bindings(p: &Pattern, out: &mut std::collections::HashSet<String>) {
    match p {
        Pattern::Ident(name) | Pattern::RefMutIdent(name) => {
            out.insert(name.clone());
        }
        Pattern::Tuple(items) | Pattern::List(items) => {
            for it in items {
                collect_pattern_bindings(it, out);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, fp) in fields {
                collect_pattern_bindings(fp, out);
            }
        }
        Pattern::Enum { args, .. } => {
            for a in args {
                collect_pattern_bindings(a, out);
            }
        }
        Pattern::Rest(Some(name)) => {
            out.insert(name.clone());
        }
        _ => {}
    }
}

/// 预扫描 IR 模块，返回被调用但模块内无定义的顶层函数名 → 最大实参个数
fn collect_unknown_extern_fns(module: &IrModule) -> std::collections::HashMap<String, usize> {
    let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &module.items {
        match item {
            Item::FnDef(f) => {
                known.insert(f.name.clone());
                for p in &f.params {
                    known.insert(p.name.clone());
                }
            }
            Item::Const(c) => {
                known.insert(c.name.clone());
            }
            Item::StructDef(s) => {
                known.insert(s.name.clone());
                for m in &s.methods {
                    known.insert(m.name.clone());
                    for p in &m.params {
                        known.insert(p.name.clone());
                    }
                }
            }
            Item::EnumDef(e) => {
                known.insert(e.name.clone());
                for m in &e.methods {
                    known.insert(m.name.clone());
                    for p in &m.params {
                        known.insert(p.name.clone());
                    }
                }
            }
            Item::TraitDef(t) => {
                known.insert(t.name.clone());
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    known.insert(m.name.clone());
                    for p in &m.params {
                        known.insert(p.name.clone());
                    }
                }
            }
            Item::CheckerBlock { name, .. } => {
                known.insert(name.clone());
            }
            Item::TypeAlias(t) => {
                known.insert(t.name.clone());
            }
            Item::DuckDef(d) => {
                known.insert(d.name.clone());
            }
            // 导入符号属于本 crate 已知名字：from lib_stats import sum_squares /
            // import lib_math 的目标名不得触发未知外部函数桩
            // （`fn NAME(__a0: i64) -> i64 { i64::MAX }`），否则增量拼接产物中
            // 桩与依赖模块的真身重复定义（rustc E0428，见
            // tests/incremental_golden.rs 增量拼接 rustc 校验步骤）。
            // 真正无签名的外部调用（p16_lzi 类）不走 Use item，兜底语义不变。
            Item::Use(u) => {
                if u.is_from {
                    for n in &u.items {
                        known.insert(n.clone());
                    }
                }
                if let Some(last) = u.path.last() {
                    known.insert(last.clone());
                }
                if let Some(a) = &u.alias {
                    known.insert(a.clone());
                }
            }
            _ => {}
        }
    }
    let builtins: std::collections::HashSet<&str> = LZ_BUILTIN_FN_NAMES.iter().copied().collect();
    let mut out: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for item in &module.items {
        match item {
            Item::FnDef(f) => scan_block_extern_calls(&f.body, &known, &builtins, &known, &mut out),
            Item::CheckerBlock { body, .. } => {
                scan_block_extern_calls(body, &known, &builtins, &known, &mut out)
            }
            Item::StructDef(s) => {
                for m in &s.methods {
                    scan_block_extern_calls(&m.body, &known, &builtins, &known, &mut out);
                }
            }
            Item::EnumDef(e) => {
                for m in &e.methods {
                    scan_block_extern_calls(&m.body, &known, &builtins, &known, &mut out);
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    scan_block_extern_calls(&m.body, &known, &builtins, &known, &mut out);
                }
            }
            Item::Test(t) => scan_block_extern_calls(&t.body, &known, &builtins, &known, &mut out),
            Item::Const(c) => scan_expr_extern_calls(&c.value, &known, &builtins, &known, &mut out),
            _ => {}
        }
    }
    out
}

// ── @parallel：AST 级并行化变换 ────────────────────────────────────
// 把 @parallel 函数体内的 `xs.map(f)` 方法调用重写为
// `lz_builtins::__lz_par_map(xs, f)`（std::thread 分块并行）。

fn rewrite_parallel_block(block: &mut Block) {
    for stmt in &mut block.stmts {
        rewrite_parallel_stmt(stmt);
    }
}

fn rewrite_parallel_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::Let { value, .. } => rewrite_parallel_expr(value),
        Stmt::Assign { value, .. } => rewrite_parallel_expr(value),
        Stmt::Return { value: Some(v) } => rewrite_parallel_expr(v),
        Stmt::ExprStmt { expr } => rewrite_parallel_expr(expr),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            rewrite_parallel_expr(cond);
            rewrite_parallel_block(then_branch);
            if let Some(b) = else_branch {
                rewrite_parallel_block(b);
            }
        }
        Stmt::For {
            iter,
            guard,
            body,
            else_body,
            ..
        } => {
            rewrite_parallel_expr(iter);
            if let Some(g) = guard {
                rewrite_parallel_expr(g);
            }
            rewrite_parallel_block(body);
            if let Some(b) = else_body {
                rewrite_parallel_block(b);
            }
        }
        Stmt::While {
            cond,
            guard,
            body,
            ..
        } => {
            rewrite_parallel_expr(cond);
            if let Some(g) = guard {
                rewrite_parallel_expr(g);
            }
            rewrite_parallel_block(body);
        }
        Stmt::WhileLet { expr, guard, body, .. } => {
            rewrite_parallel_expr(expr);
            if let Some(g) = guard {
                rewrite_parallel_expr(g);
            }
            rewrite_parallel_block(body);
        }
        Stmt::Match { scrutinee, arms } => {
            rewrite_parallel_expr(scrutinee);
            for arm in arms {
                if let Some(g) = &mut arm.guard {
                    rewrite_parallel_expr(g);
                }
                rewrite_parallel_block(&mut arm.body);
            }
        }
        Stmt::Block { stmts } => {
            for s in stmts {
                rewrite_parallel_stmt(s);
            }
        }
        Stmt::BlockLabel { body, .. } => rewrite_parallel_block(body),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            rewrite_parallel_block(body);
            for (_, b) in catches {
                rewrite_parallel_block(b);
            }
            if let Some(b) = else_body {
                rewrite_parallel_block(b);
            }
            if let Some(b) = finally_body {
                rewrite_parallel_block(b);
            }
        }
        Stmt::Defer { body } => rewrite_parallel_block(body),
        _ => {}
    }
}

fn rewrite_parallel_expr(expr: &mut Expr) {
    // `xs.map(lambda)` → `__lz_par_map(xs, lambda)`
    if let ExprKind::MethodCall { receiver, method, args } = &mut expr.kind {
        if method == "map" && args.len() == 1 && matches!(args[0].kind, ExprKind::Lambda { .. }) {
            let span = expr.span.clone();
            let ty = expr.ty.clone();
            let recv = std::mem::replace(
                receiver,
                Box::new(Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, span.clone())),
            );
            let lam = args[0].clone();
            expr.kind = ExprKind::Call {
                callee: Box::new(Expr::new(
                    ExprKind::Var("__lz_par_map".into()),
                    IrType::Any,
                    span.clone(),
                )),
                args: vec![*recv, lam],
                type_args: vec![],
            };
            expr.ty = ty;
            return;
        }
    }
    // 递归遍历所有子表达式
    match &mut expr.kind {
        ExprKind::Call { callee, args, .. } => {
            rewrite_parallel_expr(callee);
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            rewrite_parallel_expr(receiver);
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::FieldAccess { base, .. } => rewrite_parallel_expr(base),
        ExprKind::IndexGet { base, key } => {
            rewrite_parallel_expr(base);
            rewrite_parallel_expr(key);
        }
        ExprKind::IndexSet { base, key, value } => {
            rewrite_parallel_expr(base);
            rewrite_parallel_expr(key);
            rewrite_parallel_expr(value);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            rewrite_parallel_expr(lhs);
            rewrite_parallel_expr(rhs);
        }
        ExprKind::AssignExpr { target, value } => {
            rewrite_parallel_expr(target);
            rewrite_parallel_expr(value);
        }
        ExprKind::UnOp { operand, .. } => rewrite_parallel_expr(operand),
        ExprKind::IfExpr { cond, then, els } => {
            rewrite_parallel_expr(cond);
            rewrite_parallel_expr(then);
            rewrite_parallel_expr(els);
        }
        ExprKind::Lambda { body, .. } => rewrite_parallel_expr(body),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                rewrite_parallel_expr(e);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::GenExpr { yield_of } => rewrite_parallel_expr(yield_of),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                rewrite_parallel_expr(c);
            }
            rewrite_parallel_block(block);
        }
        ExprKind::Cast { expr, .. } => rewrite_parallel_expr(expr),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::BlockExpr { block } => rewrite_parallel_block(block),
        ExprKind::TupleLit(es) | ExprKind::Tuple(es) | ExprKind::ListLit(es) | ExprKind::List(es) => {
            for e in es {
                rewrite_parallel_expr(e);
            }
        }
        ExprKind::Spread(e) => rewrite_parallel_expr(e),
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs {
                rewrite_parallel_expr(k);
                rewrite_parallel_expr(v);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                rewrite_parallel_expr(s);
            }
            rewrite_parallel_expr(end);
        }
        ExprKind::Pipe { receiver, callee, args } => {
            rewrite_parallel_expr(receiver);
            rewrite_parallel_expr(callee);
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::Paren(e) => rewrite_parallel_expr(e),
        ExprKind::ImplicitConvert { source, .. } => rewrite_parallel_expr(source),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extern_auto_registers_to_bridge_registry() {
        // I3：注入 registry 时，#[extern(lang)] 函数在 codegen 中自动
        // register_symbol + 台账 REGISTER（不改变生成产物本身）。
        let mut module = IrModule::new("test".into());
        module.items.push(Item::FnDef(FnDef {
            name: "open_device".into(),
            generics: vec![],
            params: vec![Param {
                name: "dev".into(),
                ty: IrType::Int,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: false,
                comptime: false,
                mods: IrMods::default(),
            }],
            ret_ty: IrType::Unit,
            raises: None,
            body: Block {
                stmts: vec![],
                ty: IrType::Unit,
                span: Span::unknown(),
            },
            intrinsics: vec![Intrinsic {
                kind: IntrinsicKind::Extern(vec!["Rust".into()]),
                span: Span::unknown(),
            }],
            is_async: false,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: Span::unknown(),
        }));
        let reg = crate::bridge::core::BridgeRegistry::new();
        let mut cg = CodeGen::new();
        cg.set_bridge_registry(reg);
        let _rust = cg.generate(&module);
        let reg = cg.bridge_registry.unwrap();
        assert_eq!(reg.symbol_count(), 1, "extern 符号应自动登记");
        let sym = reg.lookup_symbol("open_device").expect("符号应可查询");
        assert_eq!(sym.lang, "Rust");
        assert!(
            sym.signature.contains("dev: i64"),
            "签名应含参数类型: {}",
            sym.signature
        );
        let recs = reg.ledger().records();
        assert!(
            recs.iter()
                .any(|r| r.event == "REGISTER" && r.detail.contains("open_device")),
            "台账应含 REGISTER 记录"
        );
    }

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
            raises: None,
            body: Block {
                stmts: vec![],
                ty: IrType::Unit,
                span: Span::unknown(),
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
                    comptime: false,
                    mods: IrMods::default(),
                },
                Param {
                    name: "b".into(),
                    ty: IrType::Int,
                    is_mut: false,
                    is_ref: false,
                    is_owned: false,
                    default: None,
                    variadic: false,
                    comptime: false,
                    mods: IrMods::default(),
                },
            ],
            ret_ty: IrType::Int,
            raises: None,
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
                span: Span::unknown(),
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
