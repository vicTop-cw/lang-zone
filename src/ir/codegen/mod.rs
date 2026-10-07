// Lang-Zone 编译器 — ir/codegen.rs
// LZIR → Rust 源代码生成器
//
// 职责：
// 1. 将 IrModule 转换为合法的 Rust 源代码字符串
// 2. 类型映射：IrType → Rust 类型（如 Option→Option, List→Vec）
// 3. 生成完整的、可编译的 .rs 文件

mod helpers;

pub mod moddec_emit;

// 大文件拆分（move-only，2026-10-06）：原单文件按职责切分为子模块，逻辑零改动
mod decl_gen;
mod emit;
mod expr_gen;
mod magic_gen;
mod pattern_gen;
mod scan;
mod stmt_gen;
mod str_boundary;
mod types_emit;

// 注：子模块内部符号不再在 mod.rs 统一 re-export —— 方法经 impl 自动可见，
// 自由函数由使用方 `use super::<mod>::<name>;` 显式引入。
use scan::{
    block_uses_dict, collect_typepack_calls, collect_unbound_fstring_vars,
    collect_unknown_extern_fns, expr_uses_dict,
};
use types_emit::{is_dict_ty, BIGINT_RS, COMPLEX_RS};

use helpers::*;

// builder.rs 直接依赖 collect_var_refs（AST→IR 的全局变量收集），重导出保持路径稳定
pub(crate) use helpers::collect_var_refs;


use super::node::*;

use super::types::IrType;

use super::IrModule;

// 修饰符装饰器新语义轴（T04）：`@shared` 族 / `@cell` 族 的轴枚举。
use crate::ast::{InteriorMode, SharedMode};

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
    /// 函数返回类型为嵌套 Fn（fn -> fn -> T）时：内层闭包返回需 Arc::new 包装，
    /// 且返回类型用 Arc<dyn Fn … + Send + Sync>（Rust 不允许 impl Fn -> impl Fn 嵌套，E0562）
    nested_fn_ret: bool,
    /// IR-003：Lambda 作为一等值（let = Lambda / return Lambda 的值位置）时
    /// 需 Arc::new 包装为 `Arc<dyn Fn … + Send + Sync>`（Arc 而非 Box：LZ fn 值是值语义，产物会 .clone()，
    /// Box<dyn Fn> 不实现 Clone）。仅在值位置置 true，避免借用捕获闭包被误装箱
    /// （for_each |x| total += x 必须保持 impl FnMut 借用捕获，见 gen_param）
    box_lambda: bool,
    /// 顶层 def 名称集合：用作值（Var）时需 Arc::new(f)；局部 fn-let 由 let 值位置已
    /// 装箱，不重复装箱（避免 Rc 双层嵌套）
    top_level_fns: std::collections::HashSet<String>,
    /// 值位置 fn 变量的名字 → 参数个数（Rust 侧载体为 `Arc<dyn Fn … + Send + Sync> …>`）：由 `let x = <闭包>`
    /// 装箱或 `let x = 返回 fn 的调用` 建立。std 未给 `Arc<dyn Fn … + Send + Sync>` 实现 `Fn`，故把这类值传给
    /// `impl Fn(…) + 'static` / 迭代器 `FnMut` 时需用转发闭包（见 fn_value_fwd）。
    /// 记个数而非类型：IR 有时把 fn 值变量标成非 Fn（偏应用 `add(_, 1)`），名字是唯一可靠线索
    fn_value_carriers: HashMap<String, usize>,
    /// @math 函数标志：体内整数字面量经 T::from(i32) 转换（使 `x * 2` 中 2 推断为 T）。
    /// 普通泛型函数不应转换（否则 `total = 0` 生成 T::from(0i32) 与 i64 冲突，E0308）
    in_math_fn: bool,
    /// 当前是否在 Lambda 块体（BlockExpr child）内：嵌套 Fn 返回时，
    /// 仅块体尾闭包需 Arc::new 包装，函数体本身的尾表达式（外层闭包）不包装
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
    /// 当前是否在 if/else 分支内：分支内方法调用返回 String 时需 `let _ = expr` 丢弃
    /// 值以统一分支类型为 `()`（否则 E0308 if and else have incompatible types）
    in_if_else_branch: bool,
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
    /// 函数返回类型表：fn_name → return_type（用于 is_str_producing 识别字符串返回函数调用，
    /// 如 `stringify(x)` 返回 String，let 绑定登记到 str_typed_vars 后 f-string 用 {} 而非 {:?}）
    fn_returns: std::collections::HashMap<String, IrType>,
    /// 未定义外部函数名 → 最大实参个数（p16_lzi 等跨模块探测用例：
    /// 调用的 external_add/external_opt 定义在 .lzi 签名中，批测未加载
    /// .lzi 时生成 i64 stub 兜底，保证 rustc 编译通过）
    unknown_extern_fns: std::collections::HashMap<String, usize>,
    /// struct 字段信息：struct_name → [(field_name, field_type)]，用于 __new__ 补齐默认字段
    /// struct 字段信息（供 __new__ 补齐默认字段）
    struct_fields_info: std::collections::HashMap<String, Vec<(String, IrType)>>,
    /// f-string 插值静态判定：当前函数可见的变量名 → IR 类型（形参在 gen_fn_def 登记，
    /// let 绑定在 gen_stmt 的 Let 分支登记）。
    /// `f"{r.name}"` 里 r 是 struct 时，需按 `struct_fields_info` 查 `name` 是否 str——
    /// 否则 str 字段被判成非字符串走 `{:?}` 带引号，既违反
    /// SYNTAX/00-词法基础.md:188（str 插值 = Display 无引号），也与 cy 后端的
    /// 运行时判定（`_lz_fs` 按值类型去引号）分叉。台账 BUG-23。
    fstr_var_types: std::collections::HashMap<String, IrType>,
    /// 当前方法所属 struct 名（`self.field` 的同款判定）
    fstr_self_type: Option<String>,
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
    /// 延迟惰性绑定（`@lazy` / `@lazy_mut`）：源码变量名 → (发射用安全名, 首次访问
    /// 求值表达式)。绑定处**不求值**，本作用域内对该变量的引用改写为
    /// `*安全名.get_or_init(|| 表达式)`，仅求值一次（AC4）。
    lazy_bindings: std::collections::HashMap<String, (String, String)>,
    /// 修饰符装饰器目标类型所需的 std 导入项：模块路径 → 项名集合
    /// （如 `std::sync` → {`Mutex`, `RwLock`}）。仅在使用相应轴时登记，
    /// 未使用时不产生任何导入，保证无装饰器模块产物逐字节不变（T04）。
    moddec_std_imports:
        std::collections::BTreeMap<&'static str, std::collections::BTreeSet<&'static str>>,
    /// `@atomic` 用到的具体原子类型名集合（如 `AtomicI64`），供 prelude 精确导入。
    moddec_atomic_types: std::collections::BTreeSet<String>,
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
    /// 本模块是否使用 Dict/HashMap 类型（仅此时注入 use std::collections::BTreeMap;）
    module_uses_dict: bool,
    /// I3：L2 中继——extern 自动登记目标 registry。注入后 codegen 处理
    /// #[extern(lang)] 函数时自动 register_symbol 并联动台账（REGISTER）。
    /// None = 不登记（默认，保持既有生成行为不变）。
    pub bridge_registry: Option<crate::bridge::core::BridgeRegistry>,
    /// embed 生成的模块名集合（如 `foo_tnr`），供 gen_use_stmt 自动路由。
    embed_modules: std::collections::HashSet<String>,
    buf: String,
}
impl CodeGen {


    pub fn new() -> Self {
        let mut type_map = HashMap::new();
        type_map.insert("List", "Vec");
        type_map.insert("Dict", "BTreeMap");
        type_map.insert("Set", "HashSet");
        type_map.insert("String", "String");
        type_map.insert("Nil", "()");
        type_map.insert("Unit", "()");
        type_map.insert("Range", "std::ops::Range<i64>");
        type_map.insert("RangeInclusive", "std::ops::RangeInclusive<i64>");
        // 基础类型保持原样
        type_map.insert("int", "i64");
        type_map.insert("int128", "i128");
        type_map.insert("bigint", BIGINT_RS);
        type_map.insert("complex", COMPLEX_RS);
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
            fn_value_carriers: HashMap::new(),
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
            in_if_else_branch: false,

            in_ext_trait: false,
            // 当前是否在 Result 基 try 块闭包体内：raises 函数调用需 `?` 解包
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
            fstr_var_types: std::collections::HashMap::new(),
            fstr_self_type: None,
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
            lazy_bindings: std::collections::HashMap::new(),
            moddec_std_imports: std::collections::BTreeMap::new(),
            moddec_atomic_types: std::collections::BTreeSet::new(),
            imported_modules: std::collections::HashSet::new(),
            current_fn_is_async: false,
            current_fn_is_size_hint: false,
            current_fn_ret_is_ref: false,
            duck_field_members: std::collections::HashMap::new(),
            duck_defs: std::collections::HashMap::new(),
            module_uses_ext: false,
            module_uses_dict: false,
            global_vars: std::collections::HashMap::new(),
            downgraded_vars: std::collections::HashSet::new(),
            unbound_fstring_vars: std::collections::HashSet::new(),
            fn_returns: std::collections::HashMap::new(),
            unknown_extern_fns: std::collections::HashMap::new(),
            bridge_registry: None,
            embed_modules: std::collections::HashSet::new(),
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
        let mut reg = crate::bridge::core::BridgeRegistry::new();
        // P3：注册默认桥接——TnrBridge 处理 tnr::/tnr_lib:: 路由
        reg.register(Box::new(crate::bridge::tnr::TnrBridge::new()));
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
        self.fn_returns.clear();
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

        // 预扫描修饰符装饰器目标类型所需的 std 导入项（T04；仅实际使用时登记）
        self.collect_moddec_imports(module);

        // P1：收集 embed 生成的模块名，供 gen_use_stmt 自动路由
        self.embed_modules.clear();
        {
            let mut ec = crate::ir::embed_collector::EmbedCollector::new();
            ec.collect(module);
            let source_name = module
                .file_path
                .as_deref()
                .unwrap_or(&module.name);
            for name in ec.module_names(source_name) {
                self.embed_modules.insert(name);
            }
        }

        // 预扫描 raises 函数名（checker 调用点解包其 Result 返回值）
        for item in &module.items {
            if let Item::FnDef(f) = item {
                if f.raises.is_some() {
                    self.raises_fn_names.insert(f.name.clone());
                }
                // 收集函数返回类型（用于 is_str_producing 识别字符串返回函数调用）
                if !matches!(f.ret_ty, IrType::Unit) {
                    self.fn_returns.insert(f.name.clone(), f.ret_ty.clone());
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
                // `@lazy_static` / `@once` 顶层静态：同样经 LazyLock/OnceLock 解引用访问
                if ty_is_collection || c.mods.lazy {
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
                        .insert((e.name.clone(), variant.name.clone()), field_types.clone());
                    // 变体字段类型（dotted + 裸名双键，与 builder.rs:156-158 一致）：
                    // MethodCall 构造器分支按 "Enum.Variant" 查表设置参数期望类型，
                    // 不填充则查表恒 None，String 索引期望类型推断失效（lib_json E0308）
                    self.enum_variant_fields
                        .insert((e.name.clone(), variant.name.clone()), field_types.clone());

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
                // str 参数不登记为 ref：参数类型映射已改为 String（按值），
                // 调用点由 9556-9648 行的值语义逻辑自动 .clone()，避免 &String vs String E0308
                for m in &s.methods {
                    let has_ref_or_str = m.params.iter().any(|p| p.is_ref);
                    if has_ref_or_str {
                        self.fn_ref_params.insert(
                            m.name.clone(),
                            m.params.iter()
                                .map(|p| {
                                    let is_ref = p.is_ref;
                                    (is_ref, p.is_mut)
                                })
                                .collect(),
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
                        // 若 __new__ 或 new 在 impl 块中，记录其参数表以便 kwarg 构造路由
                        if m.name == "__new__" || m.name == "new" {
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
                        // str 参数不登记为 ref：参数类型映射已改为 String（按值），
                        // 调用点由值语义逻辑自动 .clone()，避免 &String vs String E0308
                        let has_ref_or_str = m.params.iter().any(|p| p.is_ref);
                        if has_ref_or_str {
                            self.fn_ref_params.insert(
                                m.name.clone(),
                                m.params.iter()
                                    .map(|p| {
                                        let is_ref = p.is_ref;
                                        (is_ref, p.is_mut)
                                    })
                                    .collect(),
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
                // str 参数不登记为 ref：参数类型映射已改为 String（按值），
                // 调用点由值语义逻辑自动 .clone()，避免 &String vs String E0308
                if f.params.iter().any(|p| p.is_ref) {
                    self.fn_ref_params.insert(
                        f.name.clone(),
                        f.params.iter()
                            .map(|p| {
                                let is_ref = p.is_ref;
                                (is_ref, p.is_mut)
                            })
                            .collect(),
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
        self.module_uses_dict = module.items.iter().any(|item| match item {
            Item::FnDef(f) => {
                is_dict_ty(&f.ret_ty)
                    || f.params.iter().any(|p| is_dict_ty(&p.ty))
                    || block_uses_dict(&f.body)
            }
            Item::Const(c) => is_dict_ty(&c.ty) || expr_uses_dict(&c.value),
            Item::StructDef(s) => s.fields.iter().any(|f| is_dict_ty(&f.ty)),
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

    /// 隐式转换桥：let x: TargetTy = src_val 中，若 TargetTy 实现了 __implicit_from__(SrcTy)，
    /// 生成 `<TargetTy as ImplicitFrom<SrcTy>>::__implicit_from__(value)` 桥接表达式。
    pub(crate) fn build_implicit_bridge(
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
    }}
impl Default for CodeGen {


    fn default() -> Self {
        Self::new()
    }}


/// 判断表达式是否为 _KwArg（关键字参数）


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
