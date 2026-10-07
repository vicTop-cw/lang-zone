// Lang-Zone 编译器 — ir/builder.rs
// AST → LZIR-H 构造器：将 AST Module 转换为 IrModule
//
// 职责：
// 1. 逐节点 AST → LZIR 转换
// 2. 构建块脱糖（=:→Let, ^:→IndexGet, ~:→Call, *:→GenExpr）
// 3. 类型推导（从标注 + 简单传播 + 字面量推断）
// 4. 魔法方法归一化（MagicCall / MethodCall）
//
// 大文件拆分（move-only，2026-10-06）：单文件按职责切分为子模块，逻辑零改动。
// 对外路径 `builder::build_ir` / `builder::IrBuildError` / `builder::TypeCtx` 等
// 保持原样 —— 子模块符号在此 re-export，`src/ir/mod.rs` 与 `src/main.rs` 零改动。

mod capture;
mod comptime_conv;
mod convert_decl;
mod convert_expr;
mod convert_stmt;
mod extended_check;
mod generics;
mod infer;
mod ops_map;
mod pattern;
mod specialize;
mod tail_call;
mod util;

pub(crate) use capture::*;
pub(crate) use comptime_conv::*;
pub(crate) use convert_decl::*;
pub(crate) use convert_expr::*;
pub(crate) use convert_stmt::*;
pub(crate) use extended_check::*;
pub(crate) use generics::*;
pub(crate) use infer::*;
pub(crate) use ops_map::*;
pub(crate) use pattern::*;
pub(crate) use specialize::*;
pub(crate) use tail_call::*;
// TCO 开关与提示通道需由二进制（main.rs / cli.rs）调用，不能只走 pub(crate) 重导出
pub use tail_call::{set_tco_enabled, tco_enabled, take_tco_warnings};
pub(crate) use util::*;

use crate::ast::{
    self, AssignOp, BinOp, BuildKind, Expr as AstExpr, Pattern as AstPattern, Stmt as AstStmt,
    UnaryOp,
};

use crate::types::Type as AstType;


use super::codegen::collect_var_refs;

use super::node::*;

use super::types::{from_ast_type, from_ast_type_with_generics, IrType};

use super::IrModule;

use std::cell::RefCell;

use std::collections::{HashMap, HashSet};

use std::rc::Rc;


/// 类型推导上下文
///
/// 注：拆分后子模块函数（pub(crate)）签名中出现该类型，故提升为 `pub(crate)`
/// （仅可见性调整，字段与逻辑零改动）。
#[derive(Clone)]
pub(crate) struct TypeCtx {
    /// 变量名 → 类型
    vars: HashMap<String, IrType>,
    /// 函数名 → 返回类型
    fn_returns: HashMap<String, IrType>,
    /// 函数名 → raises 异常类型（BUG-CG-004 收口：try 体末尾调用 raises 函数时，
    /// 其 Rust 实际返回 Result<ok, err>，需据此将 try body 块类型标注为 Result，
    /// 使 codegen 走 use_result_try（match）而非 catch_unwind（panic 基），
    /// 否则 match 臂类型不兼容 E0308）
    fn_raises: HashMap<String, IrType>,
    /// 函数参数类型（用于泛型实例化）
    fn_params: HashMap<String, Vec<IrType>>,
    /// struct 名称集合（用于区分构造调用与普通函数调用）
    struct_names: HashSet<String>,
    /// struct 字段类型：struct_name → field_name → type
    struct_fields: HashMap<String, HashMap<String, IrType>>,
    /// struct 字段声明顺序：struct_name → [field_name, ...]（位置参数构造用）
    struct_field_order: HashMap<String, Vec<String>>,
    /// struct 方法名集合：struct_name → 方法名集合（含魔术方法）
    struct_methods: HashMap<String, HashSet<String>>,
    /// struct 方法非 self 参数个数：struct_name → (method_name → 参数个数)
    /// 用于 __call__ 单参校验与 __rpipe__/__lpipe__ 分派
    struct_method_arity: HashMap<String, HashMap<String, usize>>,
    /// 顶层 const/static 类型：name → type
    top_level_consts: HashMap<String, IrType>,
    /// 被重新赋值的顶层 const 名称（可变全局）：这些不能内联为字面量，
    /// 否则 `count = 0; count += 1` 会生成 `0i64 = 0i64 + 1i64`（BUG-7/walrus 簇）
    mutated_top_level_consts: std::collections::HashSet<String>,
    /// enum variant → enum name 映射
    enum_variants: HashMap<String, String>,
    /// enum 变体字段类型：variant → [类型]（有序，match 臂绑定用）
    enum_variant_field_types: HashMap<String, Vec<IrType>>,
    /// enum 泛型形参列表：enum_name → [T, E, ...]（match 臂绑定实例化用）
    enum_generics: HashMap<String, Vec<String>>,
    /// 本块（convert_block 循环）内首次声明的变量：区分「首次绑定」与「重新赋值」
    /// （闭包体内 `total = total + x` 写外部变量 → 应转 Assign，而非新 let 绑定）
    block_declared: std::collections::HashSet<String>,
    /// 当前函数泛型参数
    current_generics: Vec<String>,
    /// 当前函数返回类型
    current_ret_ty: Option<IrType>,
    /// impl 方法中 self 的具体类型（如 Dict<K,V> / HashMap<K,V>）：
    /// 未设置时 self 推断为 Self_，导致 `self[key]`/`key in self` 无法解析
    /// 容器方法（codegen 需 Named Dict/HashMap 分支判断）
    self_ty: Option<IrType>,
    /// 当前是否在 iterator（生成器）函数内：return 等价 raise，不做隐式类型转换
    current_is_iterator: bool,
    /// 当前函数名（用于嵌套函数命名）
    current_fn_name: Option<String>,
    /// 提升出的待处理顶级 Items（嵌套函数等）
    pending_items: Rc<RefCell<Vec<Item>>>,
    /// 语义错误收集（不可变重赋值 E0384 / 空列表类型不可推断 E0282）
    errors: Rc<RefCell<Vec<String>>>,
    /// 顶层 const 的编译期求值结果（name → ComptimeValue）：comptime 块/表达式
    /// 内解析 const 引用（`comptime LIMIT / 2`），否则 Ident 查不到报未定义
    comptime_consts: std::collections::HashMap<String, crate::comptime::ComptimeValue>,
    /// 跨模块类型签名（lz-infer 生成的 .lzi）：函数返回类型回退查询源
    /// （本地函数查不到时，从 .lzi 模块签名补全，接通跨模块推断管线）
    #[cfg(feature = "infer")]
    lzi_signatures: Option<std::rc::Rc<crate::infer::LziRegistry>>,
    /// 当前模块 AST（Rc 共享）：comptime 求值需访问模块函数定义
    /// （`comptime gen_primes(8)` 查 module.functions 编译期执行）
    comptime_module: Option<std::rc::Rc<ast::Module>>,
    /// M3 单态化：已生成特化函数名的集合（去重，避免同一 (函数, comptime值) 组合
    /// 重复发射 pending item）。与 ctx 一同克隆，全模块共享同一份。
    specialization_set: std::rc::Rc<std::cell::RefCell<std::collections::HashSet<String>>>,
}
impl TypeCtx {


    pub(crate) fn new() -> Self {
        TypeCtx {
            vars: HashMap::new(),
            fn_returns: HashMap::new(),
            fn_raises: HashMap::new(),
            fn_params: HashMap::new(),
            struct_names: HashSet::new(),
            struct_fields: HashMap::new(),
            struct_field_order: HashMap::new(),
            struct_methods: HashMap::new(),
            struct_method_arity: HashMap::new(),
            top_level_consts: HashMap::new(),
            mutated_top_level_consts: std::collections::HashSet::new(),
            enum_variants: HashMap::new(),
            enum_variant_field_types: HashMap::new(),
            enum_generics: HashMap::new(),
            block_declared: std::collections::HashSet::new(),
            current_generics: vec![],
            current_ret_ty: None,
            self_ty: None,
            current_is_iterator: false,
            current_fn_name: None,
            pending_items: Rc::new(RefCell::new(Vec::new())),
            errors: Rc::new(RefCell::new(Vec::new())),
            comptime_consts: std::collections::HashMap::new(),
            comptime_module: None,
            specialization_set: std::rc::Rc::new(std::cell::RefCell::new(
                std::collections::HashSet::new(),
            )),
            #[cfg(feature = "infer")]
            lzi_signatures: None,
        }
    }

    /// 记录一条语义错误（自动去重），供 build_ir 在末尾统一报错
    pub(crate) fn report_error(&self, msg: String) {
        let mut errors = self.errors.borrow_mut();
        if !errors.contains(&msg) {
            errors.push(msg);
        }
    }

    pub(crate) fn collect_structs(&mut self, module: &ast::Module) {
        for s in &module.structs {
            if s.is_enum {
                self.enum_generics
                    .insert(s.name.clone(), s.generics.clone());
                for f in &s.fields {
                    self.enum_variants.insert(f.name.clone(), s.name.clone());
                }
                // 收集变体字段类型：variant → [类型]（有序，match 臂绑定用）
                // 变体字段合并存于 Field.ty：单字段 → AstType::Int；多字段 → AstType::Tuple([..])；
                // 命名字段（Circle(x: f64, y: f64)）→ AstType::Record([(name, ty)])
                for v in &s.fields {
                    let types: Vec<IrType> = match &v.ty {
                        AstType::Duck { fields } => {
                            fields.iter().map(|(_, t)| from_ast_type(t)).collect()
                        }
                        AstType::Tuple(items) => items.iter().map(from_ast_type).collect(),
                        AstType::Unit => vec![],
                        other => vec![from_ast_type(other)],
                    };
                    // 限定名键（Enum.Variant）：跨枚举同名变体（如 IrType.Tuple 与
                    // Pattern.Tuple）消歧用；裸名键保持旧语义兑底。裸变体名不含
                    // 点号，与限定键在同名 map 内不冲突。
                    self.enum_variant_field_types
                        .insert(format!("{}.{}", s.name, v.name), types.clone());
                    self.enum_variant_field_types.insert(v.name.clone(), types);
                }
            } else {
                self.struct_names.insert(s.name.clone());
                let mut fields = HashMap::new();
                let mut field_order: Vec<String> = Vec::new();
                // Self 字段（next: Self?）解析为 struct 自身类型名，供字段访问类型推断
                let self_ty = IrType::Named {
                    path: s.name.clone(),
                    args: s
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                };
                for f in &s.fields {
                    fields.insert(
                        f.name.clone(),
                        replace_self(&from_ast_type(&f.ty), &self_ty),
                    );
                    field_order.push(f.name.clone());
                }
                self.struct_fields.insert(s.name.clone(), fields);
                self.struct_field_order.insert(s.name.clone(), field_order);
                // 收集 struct 方法名（含魔术方法）
                let mut mset: HashSet<String> = HashSet::new();
                let mut arity_map: HashMap<String, usize> = HashMap::new();
                for m in s.methods.iter() {
                    mset.insert(m.name.clone());
                    arity_map.insert(
                        m.name.clone(),
                        m.params.iter().filter(|p| p.name != "self").count(),
                    );
                }
                for m in s.magic_methods.iter() {
                    mset.insert(m.name.clone());
                    arity_map.insert(
                        m.name.clone(),
                        m.params.iter().filter(|p| p.name != "self").count(),
                    );
                }
                self.struct_methods.insert(s.name.clone(), mset);
                self.struct_method_arity.insert(s.name.clone(), arity_map);
                // 登记 struct 内联方法返回类型（如 json.lz `parse_object` 返回 `JsonValue raises ParseError`）：
                // 否则 `self.parse_object()?` 方法调用推断为 Any，`?` 无法触发 try_into 传播（E0277）。
                // 用 `类型名.方法名` 作 key（与 impl 方法一致），避免不同 struct 同名方法互相覆盖。
                let special_magic = ["__new__", "__init__", "__implicit_from__"];
                for m in s.methods.iter().filter(|m| !special_magic.contains(&m.name.as_str())) {
                    let key = format!("{}.{}", s.name, m.name);
                    if m.return_type.is_some() {
                        self.fn_returns.insert(
                            key,
                            Self::fn_return_ir(&m.return_type, &m.raises, &s.generics),
                        );
                    } else {
                        // 无返回类型注解的方法默认登记为 Unit（避免 IR 返回类型
                        // 为 Any 导致 wrap_ok 误包成 Result::Ok(...)）
                        self.fn_returns.insert(key, IrType::Unit);
                    }
                }
                for m in s.magic_methods.iter().filter(|m| !special_magic.contains(&m.name.as_str())) {
                    let key = format!("{}.{}", s.name, m.name);
                    if m.return_type.is_some() {
                        self.fn_returns.insert(
                            key,
                            Self::fn_return_ir(&m.return_type, &m.raises, &s.generics),
                        );
                    } else {
                        self.fn_returns.insert(key, IrType::Unit);
                    }
                }
            }
        }
    }

    /// 计算函数泛型形参列表：仅取显式声明的 `func.generics`。
    /// 泛型必须显式声明；签名中未声明的类型名（如顶层 `def fold(xs: List<a>)` 的 `a`）
    /// 不再隐式当作泛型，而是报「未知类型」。外层 impl 泛型由 `convert_fn_def` 合并
    /// `current_generics` 注入，无需在此处理。
    pub(crate) fn fn_generics(&self, func: &crate::ast::Function) -> Vec<String> {
        crate::ast::augmented_fn_generics(func, &[])
    }

    pub(crate) fn collect_functions(&mut self, module: &ast::Module) {
        // 工作栈：先入顶层函数，处理时再压入函数体内的嵌套 def（FnDef），
        // 使嵌套 raises 函数也能登记 fn_returns / fn_raises（BUG-CG-004 收口：
        // try 体末尾调用嵌套 raises 函数需据此将 body 块类型标注为 Result）
        let mut stack: Vec<&ast::Function> = module.functions.iter().collect();
        while let Some(f) = stack.pop() {
            let generics: Vec<String> = self.fn_generics(f);
            let has_ret_annot = f.return_type.is_some();
            // raises/async/iterator 均经 fn_return_ir 计算（raises 时升级为 Result<ok, err>）
            let ret = Self::fn_return_ir(&f.return_type, &f.raises, &generics);
            if has_ret_annot {
                // async 函数调用返回 Future<T>（Rust async fn 调用产生 Future），
                // 登记为 Future<T> 供 await / let 标注使用（E0308/E0277 修复）
                if f.is_async {
                    self.fn_returns.insert(
                        f.name.clone(),
                        IrType::Named {
                            path: "Future".into(),
                            args: vec![ret.clone()],
                        },
                    );
                } else if f.is_iterator {
                    // iterator 生成器函数（iterator repeat<T>(val, n) -> T /
                    // count_from(...) -> Iterator<int>）：调用返回**迭代器集合**
                    // Vec<元素类型>（急切收集）。登记为
                    // Vec<int>——若声明返回 Iterator<int> 则取元素 int 登记
                    // Vec<int>，否则 Vec<ret>（iterator_demo `for x in
                    // repeat("hi", 3)` 中 T=String 误当字符串迭代生成 .chars()，
                    // E0599；while_let `Vec<impl Iterator>` 非法 E0562）
                    let elem = match &ret {
                        IrType::Named { path, args } if path == "Iterator" && args.len() == 1 => {
                            args[0].clone()
                        }
                        _ => ret.clone(),
                    };
                    self.fn_returns.insert(
                        f.name.clone(),
                        IrType::Named {
                            path: "Vec".into(),
                            args: vec![elem],
                        },
                    );
                } else {
                    self.fn_returns.insert(f.name.clone(), ret.clone());
                }
            } else {
                // 无返回注解：从函数体最后语句推断并登记。
                // 否则调用点 lookup_fn_return 回退 Any→i64，
                // 导致 `main 末尾调用 closure_in_box()` 被误推为 i64（E0308）
                let mut ret = f
                    .body
                    .last()
                    .map(|s| infer_stmt_type(s, self))
                    .unwrap_or(IrType::Unit);
                // BUG-CG-004 收口：raises 函数即使无返回注解，其 Rust 返回类型也是
                // Result<ok, err>（ok 为推断返回值，err 为 raises 异常类型），
                // 否则 fn_returns 漏 raises 包裹 → try 体末尾调用该函数时 body 块类型
                // 非 Result，use_result_try 误判走 catch_unwind（panic 基）导致 E0308
                if let Some(r) = &f.raises {
                    ret = IrType::Result {
                        ok: Box::new(ret),
                        err: Box::new(from_ast_type_with_generics(r, &generics)),
                    };
                }
                self.fn_returns.insert(f.name.clone(), ret);
            }
            let params: Vec<IrType> = f
                .params
                .iter()
                .map(|p| from_ast_type_with_generics(&p.ty, &generics))
                .collect();
            self.fn_params.insert(f.name.clone(), params.clone());
            // 登记顶层函数类型到 vars：使函数名作为值传递时（如 `fold(..., add)`）
            // lookup_var 能返回 Fn 类型，否则回退 Any 导致 codegen 无法生成 `as fn(...)` 强转。
            let fn_ty = IrType::Fn {
                    params,
                    ret: Box::new(ret.clone()),
                };
            
            self.vars.insert(f.name.clone(), fn_ty);
            
            // BUG-CG-004 收口：登记 raises 异常类型，供 try 体末尾调用 raises 函数时
            // 将 body 块类型标注为 Result<ok, err>（见 convert_expr 的 TryCatch 分支）。
            if let Some(r) = &f.raises {
                self.fn_raises
                    .insert(f.name.clone(), from_ast_type_with_generics(r, &generics));
            }
            // 递归收集函数体内的嵌套 def（FnDef），使其 raises 也能登记
            for s in &f.body {
                if let AstStmt::FnDef { func: nested } = s {
                    stack.push(nested);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn begin_fn(&mut self, generics: &[String], ret_ty: Option<&AstType>) {
        self.current_generics = generics.to_vec();
        self.current_ret_ty = ret_ty.map(|t| from_ast_type(t));
    }

    pub(crate) fn add_param(&mut self, name: &str, ty: IrType) {
        self.vars.insert(name.to_string(), ty);
    }

    pub(crate) fn add_var(&mut self, name: &str, ty: IrType) {
        self.vars.insert(name.to_string(), ty);
    }

    pub(crate) fn lookup_var(&self, name: &str) -> IrType {
        self.vars
            .get(name)
            .cloned()
            .or_else(|| {
                self.current_generics
                    .iter()
                    .find(|g| g.as_str() == name)
                    .map(|g| IrType::Generic(g.clone()))
            })
            .or_else(|| self.top_level_consts.get(name).cloned())
            .unwrap_or(IrType::Any)
    }

    pub(crate) fn lookup_fn_return(&self, name: &str) -> IrType {
        if let Some(t) = self.fn_returns.get(name) {
            return t.clone();
        }
        // 跨模块回退：本地函数查不到时，从 .lzi 签名补全（lz-infer 生成的
        // 跨模块类型签名，main.rs --lzi 加载后经 build_ir_with_lzi 注入）
        #[cfg(feature = "infer")]
        if let Some(reg) = &self.lzi_signatures {
            for file in &reg.files {
                for m in file.modules.values() {
                    if let Some(f) = m.functions.get(name) {
                        if let Some(rt) = &f.return_type {
                            return lzi_type_to_ir(rt);
                        }
                    }
                }
            }
        }
        IrType::Any
    }

    /// BUG-CG-004（轮次12）：根据返回类型与 raises 注解计算 IR 返回类型。
    /// raises 非空时返回 `Result<ok, err>`，与 convert_fn_def 的 ret_ty 升级保持一致，
    /// 使 `?` 错误传播运算符与函数调用类型推断一致（json.lz 的 self.parse_object()? 等）。
    pub(crate) fn fn_return_ir(
        ret_ty: &Option<AstType>,
        raises: &Option<AstType>,
        generics: &[String],
    ) -> IrType {
        let base = ret_ty
            .as_ref()
            .map(|t| from_ast_type_with_generics(t, generics))
            .unwrap_or(IrType::Unit);
        match raises {
            Some(e) => IrType::Result {
                ok: Box::new(base),
                err: Box::new(from_ast_type_with_generics(e, generics)),
            },
            None => base,
        }
    }

    pub(crate) fn is_struct(&self, name: &str) -> bool {
        self.struct_names.contains(name)
    }

    pub(crate) fn lookup_field(&self, struct_name: &str, field: &str) -> IrType {
        self.struct_fields
            .get(struct_name)
            .and_then(|fields| fields.get(field))
            .cloned()
            .unwrap_or(IrType::Any)
    }

    pub(crate) fn is_builtin_function(&self, name: &str) -> bool {
        matches!(
            name,
            "print"
                | "println"
                | "panic"
                | "len"
                | "contains"
                | "iter"
                | "enumerate"
                | "zip"
                | "sum"
                | "map"
                | "filter"
                | "collect"
                | "max"
                | "min"
                | "any"
                | "all"
                | "sorted"
                | "reversed"
                | "set!"
                | "format"
                | "hash"
                | "bool"
                | "range"
                | "clone"
                | "sort"
                | "reverse"
                | "spawn"
                | "go"
                | "__go"
                | "Exception"
                | "panic!"
        )
    }

    pub(crate) fn is_struct_type(&self, ty: &IrType) -> bool {
        match ty {
            IrType::Named { path, .. } => self.struct_names.contains(path),
            _ => false,
        }
    }}


// ══════════════════════════════════════════════════════════════
// 公开 API
// ══════════════════════════════════════════════════════════════

/// 错误类型
#[derive(Debug)]
pub enum IrBuildError {
    Generic(String),
}
impl std::fmt::Display for IrBuildError {


    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IrBuildError::Generic(msg) => write!(f, "IR build error: {msg}"),
        }
    }}


/// 主入口：AST Module → IrModule
/// 将 .lzi 签名中的 LZ 类型字符串转换为 IrType（跨模块推断回退用）。
/// 支持基本类型与常见泛型容器；未知类型降级为 Named（与 from_ast_type 语义一致）。
#[cfg(feature = "infer")]
fn lzi_type_to_ir(s: &str) -> IrType {
    match s.trim() {
        "int" | "i64" => IrType::Int,
        "bigint" | "BigInt" => IrType::BigInt,
        "complex" | "Complex" | "Complex64" => IrType::Complex,
        "f64" => IrType::F64,
        "str" | "String" => IrType::Str,
        "bool" => IrType::Bool,
        "()" | "Unit" => IrType::Unit,
        other => {
            // Option<T> / List<T> / Vec<T> / Dict<K,V> 等泛型容器
            if let Some(inner) = other
                .strip_prefix("Option<")
                .and_then(|t| t.strip_suffix('>'))
            {
                IrType::Option(Box::new(lzi_type_to_ir(inner)))
            } else if let Some(inner) = other
                .strip_prefix("List<")
                .or_else(|| other.strip_prefix("Vec<"))
                .and_then(|t| t.strip_suffix('>'))
            {
                IrType::Named {
                    path: "Vec".into(),
                    args: vec![lzi_type_to_ir(inner)],
                }
            } else if let Some(inner) = other
                .strip_prefix("Dict<")
                .or_else(|| other.strip_prefix("HashMap<"))
                .and_then(|t| t.strip_suffix('>'))
            {
                // Dict<K,V> → HashMap<K,V>（IR Named 表达）
                let parts: Vec<&str> = inner.splitn(2, ',').map(|t| t.trim()).collect();
                IrType::Named {
                    path: "HashMap".into(),
                    args: vec![
                        lzi_type_to_ir(parts.first().copied().unwrap_or("Any")),
                        lzi_type_to_ir(parts.get(1).copied().unwrap_or("Any")),
                    ],
                }
            } else {
                IrType::Named {
                    path: other.to_string(),
                    args: vec![],
                }
            }
        }
    }
}


/// 默认入口：不带跨模块签名（等价 build_ir_with_lzi(_, None)）
pub fn build_ir(ast_module: &ast::Module) -> Result<IrModule, IrBuildError> {
    build_ir_inner(ast_module, |_ctx| {})
}


/// 带 .lzi 跨模块类型签名的入口：main.rs `--lzi <file>` 加载后传入，
/// 供 IR builder 在本地函数查不到时回退查询外部模块函数返回类型。
#[cfg(feature = "infer")]
pub fn build_ir_with_lzi(
    ast_module: &ast::Module,
    lzi: std::rc::Rc<crate::infer::LziRegistry>,
) -> Result<IrModule, IrBuildError> {
    build_ir_inner(ast_module, |ctx| ctx.lzi_signatures = Some(lzi))
}


/// BUG-7：扫描模块，收集被重新赋值的顶层 const 名称（可变全局）。
/// 顶层 `count = 0` 经 comptime 求值进入 `comptime_consts`，普通引用会被内联为字面量；
/// 但若该 const 在函数体内被重新赋值（`count += 1` / `count = x`），内联会破坏可变性，
/// 故将其排除出内联集合，交给 codegen 走 `mutated_consts` → `static mut` + `unsafe {}`。
fn collect_mutated_top_level_consts(
    module: &ast::Module,
    top_level: &std::collections::HashMap<String, IrType>,
    out: &mut std::collections::HashSet<String>,
) {
    for f in &module.functions {
        walk_top_level_stmts(&f.body, top_level, out);
    }
    walk_top_level_stmts(&module.tests, top_level, out);
    for (_, body) in &module.top_level_builds {
        walk_top_level_stmts(body, top_level, out);
    }
}


fn walk_top_level_stmts(
    stmts: &[AstStmt],
    top_level: &std::collections::HashMap<String, IrType>,
    out: &mut std::collections::HashSet<String>,
) {
    for s in stmts {
        match s {
            AstStmt::Let { name, mutable, .. } => {
                if *mutable && top_level.contains_key(name) {
                    out.insert(name.clone());
                }
            }
            AstStmt::Assign { target, .. } => {
                if let AstExpr::Ident(name) = &target {
                    if top_level.contains_key(name) {
                        out.insert(name.clone());
                    }
                }
            }
            AstStmt::Block { body, .. } => walk_top_level_stmts(body, top_level, out),
            AstStmt::While { body, else_body, .. } => {
                walk_top_level_stmts(body, top_level, out);
                if let Some(b) = else_body {
                    walk_top_level_stmts(b, top_level, out);
                }
            }
            AstStmt::WhileLet { body, else_body, .. } => {
                walk_top_level_stmts(body, top_level, out);
                if let Some(b) = else_body {
                    walk_top_level_stmts(b, top_level, out);
                }
            }
            AstStmt::For { body, else_body, .. } => {
                walk_top_level_stmts(body, top_level, out);
                if let Some(b) = else_body {
                    walk_top_level_stmts(b, top_level, out);
                }
            }
            AstStmt::Loop(body) => walk_top_level_stmts(body, top_level, out),
            AstStmt::CheckerBlock { body, .. } => walk_top_level_stmts(body, top_level, out),
            AstStmt::Defer(body) => walk_top_level_stmts(body, top_level, out),
            AstStmt::With { body, .. } => walk_top_level_stmts(body, top_level, out),
            AstStmt::Guard { else_body, .. } => walk_top_level_stmts(else_body, top_level, out),
            AstStmt::Comptime { body } => walk_top_level_stmts(body, top_level, out),
            AstStmt::Test { body, .. } => walk_top_level_stmts(body, top_level, out),
            AstStmt::Suite { setup, teardown, tests, .. } => {
                if let Some(b) = setup {
                    walk_top_level_stmts(b, top_level, out);
                }
                if let Some(b) = teardown {
                    walk_top_level_stmts(b, top_level, out);
                }
                walk_top_level_stmts(tests, top_level, out);
            }
            AstStmt::Expr(e) => walk_top_level_expr(e, top_level, out),
            AstStmt::Return(e) | AstStmt::Yield(e) => {
                if let Some(e) = e {
                    walk_top_level_expr(e, top_level, out);
                }
            }
            AstStmt::Raise(e)
            | AstStmt::YieldFrom(e)
            | AstStmt::Assert { expr: e, .. }
            | AstStmt::Check { expr: e, .. } => {
                walk_top_level_expr(e, top_level, out);
            }
            AstStmt::FnDef { func } => walk_top_level_stmts(&func.body, top_level, out),
            // 其余变体不含语句级赋值，无需递归
            AstStmt::Const { .. }
            | AstStmt::EnumDef(_)
            | AstStmt::TypeAlias { .. }
            | AstStmt::Pass
            | AstStmt::Continue
            | AstStmt::Break(_)
            | AstStmt::BreakLabel { .. }
            | AstStmt::LetTuple { .. }
            | AstStmt::BlockCall { .. }
            | AstStmt::EmbedBlock { .. } => {}
        }
    }
}


fn walk_top_level_expr(
    e: &AstExpr,
    top_level: &std::collections::HashMap<String, IrType>,
    out: &mut std::collections::HashSet<String>,
) {
    match e {
        AstExpr::Match { arms, .. } => {
            for arm in arms {
                walk_top_level_stmts(&arm.body, top_level, out);
            }
        }
        AstExpr::If {
            then_body,
            elif_clauses,
            else_body,
            ..
        } => {
            walk_top_level_stmts(then_body, top_level, out);
            for (_, b) in elif_clauses {
                walk_top_level_stmts(b, top_level, out);
            }
            if let Some(b) = else_body {
                walk_top_level_stmts(b, top_level, out);
            }
        }
        _ => {}
    }
}


fn build_ir_inner(
    ast_module: &ast::Module,
    init_ctx: impl FnOnce(&mut TypeCtx),
) -> Result<IrModule, IrBuildError> {
    let mut ctx = TypeCtx::new();
    let pending_items = Rc::new(RefCell::new(Vec::new()));
    ctx.pending_items = pending_items.clone();
    // 跨模块签名注入（.lzi）：lookup_fn_return 回退查询源；
    // 闭包在 build_ir 传空（无 infer 时等价）、build_ir_with_lzi 注入 registry
    init_ctx(&mut ctx);
    // comptime 求值需要访问模块函数定义（`comptime gen_primes(8)` 编译期执行）
    ctx.comptime_module = Some(Rc::new(ast_module.clone()));

    // 1. 收集类型信息
    ctx.collect_structs(ast_module);
    ctx.collect_functions(ast_module);
    // 预收集顶层 const 类型（供函数体内引用查询，如生成器集合迭代）
    for c in &ast_module.consts {
        let ty =
            c.ty.as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(&c.value, &ctx));
        ctx.top_level_consts.insert(c.name.clone(), ty);
        // 顶层 const 编译期求值（comptime 块/表达式内解析 const 引用）
        let empty_module = ast::Module::default();
        let module_ref = ctx
            .comptime_module
            .as_ref()
            .map(|m| m.as_ref())
            .unwrap_or(&empty_module);
        let mut cctx = crate::comptime::ComptimeContext::new(module_ref);
        // 注入源码文本（inspect.getsource/getsourcelines 数据源，main.rs 已填）
        if let Some(src) = &module_ref.source_text {
            cctx = cctx.with_source(src.clone());
        }
        if let Ok(v) = crate::comptime::ComptimeEvaluator::eval_expr(&c.value, &mut cctx) {
            ctx.comptime_consts.insert(c.name.clone(), v);
        }
    }

    // 收集被重新赋值的顶层 const（可变全局）：这些不能内联为字面量
    // （walrus.lz `count = 0` + `count_up` 内 `count += 1` → 内联会生成
    // `0i64 = 0i64 + 1i64` 的 E0070；保留 Var 引用交给 codegen 的
    // `mutated_consts` → `static mut` + `unsafe {}` 路径）。
    collect_mutated_top_level_consts(ast_module, &ctx.top_level_consts, &mut ctx.mutated_top_level_consts);

    // 2. 构建 IR 模块
    let name = ast_module
        .name
        .clone()
        .unwrap_or_else(|| "main".to_string());
    let mut ir_mod = IrModule::new(name);

    // 3. 默认 prelude（lz.std 内建）
    ir_mod.prelude = vec![
        "Option".into(),
        "Result".into(),
        "Ordering".into(),
        "Box".into(),
        "Rc".into(),
        "Arc".into(),
        "Itor".into(),
        "Strategy".into(),
    ];

    // 4. 转换 imports → Use 项
    for imp in &ast_module.imports {
        ir_mod.items.push(Item::Use(UseStmt {
            path: imp.path.clone(),
            alias: imp.alias.clone(),
            items: imp.items.clone(),
            is_from: imp.is_from,
        }));
    }

    // 5. 转换 structs（enum 同名方法声明需合并）
    // 收集所有 enum 的额外方法声明（is_enum=true 且 fields 非空的是主声明，其余是方法声明）
    let mut enum_extra_methods: HashMap<String, Vec<&ast::StructDef>> = HashMap::new();
    let mut is_enum_main_decl: HashSet<String> = HashSet::new();
    for s in &ast_module.structs {
        if s.is_enum && !s.fields.is_empty() {
            is_enum_main_decl.insert(s.name.clone());
        }
    }
    for s in &ast_module.structs {
        if s.is_enum && s.fields.is_empty() && is_enum_main_decl.contains(&s.name) {
            // 这是附加方法声明，收集起来
            enum_extra_methods
                .entry(s.name.clone())
                .or_default()
                .push(s);
        }
    }
    // 只转换主声明和方法声明（跳过纯方法声明）
    for s in &ast_module.structs {
        if s.is_enum && s.fields.is_empty() && is_enum_main_decl.contains(&s.name) {
            continue; // 额外方法声明已收集，稍后合并
        }
        let mut item = convert_struct(s, &ctx);
        // 合并额外方法到 EnumDef
        if s.is_enum {
            if let Item::EnumDef(ref mut ed) = item {
                if let Some(extras) = enum_extra_methods.get(&s.name) {
                    for extra in extras {
                        for m in &extra.methods {
                            let mut method_ctx = TypeCtx::new();
                            method_ctx.pending_items = ctx.pending_items.clone();
                            method_ctx.struct_names = ctx.struct_names.clone();
                            method_ctx.struct_fields = ctx.struct_fields.clone();
                            method_ctx.struct_methods = ctx.struct_methods.clone();
                            ed.methods.push(convert_fn_def(m, &method_ctx));
                        }
                    }
                }
            }
        }
        // BUG-CG-004（轮次12）：注册 struct 内联方法返回类型（含 raises→Result），
        // 供方法调用与 ? 错误传播类型推断一致（json.lz 的 self.parse_object()? 等）。
        for m in &s.methods {
            if m.return_type.is_some() || m.raises.is_some() {
                ctx.fn_returns.insert(
                    format!("{}.{}", s.name, m.name),
                    TypeCtx::fn_return_ir(&m.return_type, &m.raises, &s.generics),
                );
            }
        }
        ir_mod.items.push(item);
    }

    // 6. 转换 traits
    for t in &ast_module.traits {
        ir_mod.items.push(convert_trait(t, &ctx));
    }

    // 7. 转换 impls
    for imp in &ast_module.impls {
        // 注册 impl 方法名到 struct_methods（如 HttpResult 的 __is_ok__/__unwrap__），
        // 供 AstExpr::Try（r?）自定义传播类型判定使用
        ctx.struct_methods
            .entry(imp.type_name.clone())
            .or_default()
            .extend(imp.methods.iter().map(|m| m.name.clone()));
        // 注册 impl 方法 arity（非 self 参数个数）：供扩展语义检查
        // `Counter.inc(c, 5)` 关联调用（E0061 参数数量）检测使用
        for m in &imp.methods {
            let arity = m.params.iter().filter(|p| p.name != "self").count();
            ctx.struct_method_arity
                .entry(imp.type_name.clone())
                .or_default()
                .insert(m.name.clone(), arity);
        }
        // 登记 impl 方法返回类型（如 box.lz `def get(ref self) -> ref T` 返回 &T）：
        // 否则 `b.get()` 方法调用推断为 Any，`assert b.get() == 42` 无法解引用（E0277）。
        // 用 `类型名.方法名` 作 key：Rc/Arc 都有 try_unwrap，裸方法名会互相覆盖
        // （`rc.try_unwrap()` 误推断为 Arc 的签名 → E0425 cannot find type `T`）
        for m in &imp.methods {
            if m.return_type.is_some() {
                ctx.fn_returns.insert(
                    format!("{}.{}", imp.type_name, m.name),
                    TypeCtx::fn_return_ir(&m.return_type, &m.raises, &imp.generics),
                );
            }
        }
        ir_mod.items.push(convert_impl(imp, &ctx));
    }

    // 7.5 独立 magic 块: magic __str__: def __str__(self: MyStruct) → impl MyStruct
    for mb in &ast_module.magic_blocks {
        // 从 self 参数类型确定目标类型
        let target = mb
            .function
            .params
            .iter()
            .find(|p| p.name == "self" || p.name == "self_")
            .map(|p| from_ast_type(&p.ty))
            .and_then(|ty| {
                if let IrType::Named { path, .. } = ty {
                    Some(path)
                } else {
                    None
                }
            })
            .unwrap_or_default();
        if target.is_empty() {
            continue;
        }
        // 注册魔术方法名到 struct_methods，供运算符/调用分发
        ctx.struct_methods
            .entry(target.clone())
            .or_default()
            .insert(mb.method_name.clone());
        let mut impl_ctx = ctx.clone();
        impl_ctx.current_generics = mb.function.generics.clone();
        let method = convert_fn_def(&mb.function, &impl_ctx);
        ir_mod.items.push(Item::Impl(ImplDef {
            trait_: None,
            for_type: IrType::named(&target),
            generics: vec![],
            methods: vec![method],
            assoc_type_bindings: vec![],
            where_clause: vec![],
        }));
    }

    // 8. 转换 functions
    for f in &ast_module.functions {
        // comptime def：仅编译期存在（供 comptime 求值器调用），不生成运行时代码
        if f.is_comptime {
            continue;
        }
        ir_mod.items.push(Item::FnDef(convert_fn_def(f, &ctx)));
    }

    // 9. 转换 consts
    // 9.0. 模块级魔法属性（06e-模块级魔法属性.md）：__name__/__file__/__package__/
    // __path__/__doc__/__is_macro__ 等自动填充
    // __file__/__package__/__path__ 从模块名派生（跨机器可复现，不含本机绝对路径）
    // 取源文件路径的"文件名.lz"作为 __file__（与 codegen_cython.rs 一致）
    let src_full = ast_module.file_path.clone().unwrap_or_default();
    let src_file = std::path::Path::new(&src_full)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // 若文件名不含 .lz 后缀则补上，保证与 CY 后端 __name__/__file__ 一致
    let file_const = if src_file.ends_with(".lz") {
        src_file
    } else {
        format!("{}.lz", src_file)
    };
    let path_const = std::path::Path::new(&src_full)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let package_const = std::path::Path::new(&src_full)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let magic_consts: Vec<(String, IrType, ExprKind)> = vec![
        (
            "__name__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(ir_mod.name.clone())),
        ),
        (
            "__file__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(file_const)),
        ),
        (
            "__package__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(package_const)),
        ),
        (
            "__path__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(path_const)),
        ),
        (
            "__doc__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(String::new())),
        ),
        (
            "__is_macro__".to_string(),
            IrType::Bool,
            // 宏模块（首行 #!bin macro）为 true，普通模块为 false（06e 规范）
            ExprKind::Lit(LitKind::Bool(ast_module.is_macro)),
        ),
    ];
    for (mc_name, mc_ty, mc_kind) in magic_consts {
        ctx.top_level_consts.insert(mc_name.clone(), mc_ty.clone());
        ir_mod.items.push(Item::Const(ConstDef {
            name: mc_name,
            ty: mc_ty,
            value: Expr::new(mc_kind, IrType::Any, Span::unknown()),
            mods: IrMods::default(),
        }));
    }

    for c in &ast_module.consts {
        let ty =
            c.ty.as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(&c.value, &ctx));
        // 记录顶层 const 类型，供函数内 lookup_var 查询（如生成器集合迭代）
        ctx.top_level_consts.insert(c.name.clone(), ty.clone());
        // 若声明类型为 BigInt/Complex，将字面量值类型也设为对应类型，使 codegen 正确生成
        let mut val_expr = convert_expr(&c.value, &ctx);
        if matches!(ty, IrType::BigInt | IrType::Complex) && matches!(val_expr.kind, ExprKind::Lit(_)) {
            val_expr.ty = ty.clone();
        }
        ir_mod.items.push(Item::Const(ConstDef {
            name: c.name.clone(),
            ty,
            value: val_expr,
            mods: IrMods::from_ast(&c.mods),
        }));
    }

    // 9.5. 转换 duck 类型约束 → Item::DuckDef
    for d in &ast_module.duck_defs {
        ir_mod.items.push(Item::DuckDef(convert_duck_def(d)));
    }

    // 9.6. 转换 type aliases
    for ta in &ast_module.type_aliases {
        let ir_ty = from_ast_type(&ta.ty);
        ir_mod.items.push(Item::TypeAlias(TypeAliasDef {
            name: ta.name.clone(),
            generics: ta.generics.clone(),
            ty: ir_ty,
        }));
    }

    // 9.6. 转换顶层构建块 x =: body → let x = { ... }
    // 构建块以 BlockExpr 表示（依次执行语句，最后一个表达式为值）
    for (name, body) in &ast_module.top_level_builds {
        let mut block_ctx = TypeCtx::new();
        block_ctx.current_generics = ctx.current_generics.clone();
        block_ctx.errors = ctx.errors.clone();
        block_ctx.comptime_consts = ctx.comptime_consts.clone();
    block_ctx.mutated_top_level_consts = ctx.mutated_top_level_consts.clone();
        block_ctx.comptime_module = ctx.comptime_module.clone();
        // 预扫描：收集构建块内局部变量类型（x = value 赋值），供元组/表达式推断
        for s in body {
            match s {
                AstStmt::Expr(e) => {
                    if let AstExpr::Assign { target, value, .. } = e {
                        if let AstExpr::Ident(vname) = target.as_ref() {
                            block_ctx.add_var(vname, infer_expr_type(value, &block_ctx));
                            // 本块首次声明：否则 convert_stmt 因 vars 已含该名
                            // 且 block_declared 不含而误转 Stmt::Assign（E0425）
                            block_ctx.block_declared.insert(vname.clone());
                        }
                    }
                }
                AstStmt::Let {
                    name, ty, value, ..
                } => {
                    let ir_ty = ty
                        .as_ref()
                        .map(|t| from_ast_type(t))
                        .unwrap_or_else(|| infer_expr_type(value, &block_ctx));
                    block_ctx.add_var(name, ir_ty);
                    block_ctx.block_declared.insert(name.clone());
                }
                _ => {}
            }
        }
        let stmts: Vec<Stmt> = body.iter().map(|s| convert_stmt(s, &block_ctx)).collect();
        let blk_ty = body
            .last()
            .map(|s| infer_stmt_type(s, &block_ctx))
            .unwrap_or(IrType::Unit);
        let value = Expr::new(
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts,
                    ty: blk_ty.clone(),
                },
            },
            blk_ty.clone(),
            Span::unknown(),
        );
        ir_mod.items.push(Item::Const(ConstDef {
            name: name.clone(),
            ty: blk_ty,
            value,
            mods: IrMods::default(),
        }));
    }

    // 9.7. 转换顶层 block / checker 块语句（top_stmts）
    // checker 块（block NAME[ps: __Params]）→ Item::CheckerBlock（惰性登记，由 codegen 发射 fn NAME）
    // 其他顶层语句（如顶层赋值）→ 转为 Const/表达式，保证不丢失
    for s in &ast_module.top_stmts {
        match s {
            AstStmt::CheckerBlock {
                label,
                ps_name,
                default_checker,
                body,
            } => {
                let ir_body = convert_block(body, &ctx);
                let captured = collect_checker_captured(&ir_body, &ctx, ps_name.as_deref());
                ctx.pending_items.borrow_mut().push(Item::CheckerBlock {
                    name: label.clone(),
                    ps_name: ps_name.clone(),
                    default_checker: default_checker.clone(),
                    body: ir_body,
                    captured,
                });
            }
            AstStmt::Expr(AstExpr::Assign { target, value, .. }) => {
                if let AstExpr::Ident(name) = target.as_ref() {
                    let ty = infer_expr_type(value, &ctx);
                    // 登记顶层可变变量：函数内 `x = v`（无 let 前缀）需识别为
                    // 修改全局（guard_for_3.lz size = size - 1），否则生成局部新绑定
                    ctx.top_level_consts.insert(name.clone(), ty.clone());
                    ir_mod.items.push(Item::Const(ConstDef {
                        name: name.clone(),
                        ty,
                        value: convert_expr(value, &ctx),
                        mods: IrMods::default(),
                    }));
                }
            }
            AstStmt::EmbedBlock { lang, src, form } => {
                ir_mod.items.push(Item::EmbedBlock {
                    lang: lang.clone(),
                    src: src.clone(),
                    form: form.clone(),
                });
            }
            _ => {}
        }
    }

    // 9.7b. 收集顶层表达式语句（如顶层 `print(...)`）到 ir_mod.top_level_stmts，
    // 由 codegen 在无 `def main()` 时注入自动生成的 `main()` 顺序执行（BUG-PR-001）。
    // 注意：Assign / CheckerBlock 已在上方分别处理，这里只收集其余表达式语句。
    for s in &ast_module.top_stmts {
        if let AstStmt::Expr(e) = s {
            // 顶层赋值（AstExpr::Assign）已在 9.7 转为 Const item，避免重复执行
            if !matches!(e, AstExpr::Assign { .. }) {
                ir_mod.top_level_stmts.push(Stmt::ExprStmt {
                    expr: convert_expr(e, &ctx),
                });
            }
        }
    }

    // 10. 转换 tests（顶层 test 与 suite 内的 test 都生成 #[test]）
    //     每个 test 用独立克隆的 ctx，避免 setup 变量跨 test 泄漏（见 suite 作用域）
    for t in &ast_module.tests {
        match t {
            AstStmt::Test { name, body } => {
                // 每个 test 用独立变量作用域：新建 ctx 并仅复制模块级全局类型信息，
                // 避免 suite 的 setup 变量跨 test 泄漏（见 suite 作用域 E0425）
                let mut test_ctx = TypeCtx::new();
                test_ctx.current_generics = ctx.current_generics.clone();
                test_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                test_ctx.top_level_consts = ctx.top_level_consts.clone();
                test_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                // test 体内的嵌套 def 也要提升进模块：pending_items 必须与模块共享，
                // 否则 `convert_block` 提升出的 `fn add(...)` 随 test_ctx 一起被丢弃，
                // 只剩语句位的 Lit(Unit) 占位 —— Rust 侧得到 `fn add(..) { i64::MAX }`
                // 桩、Cython 侧 `add` 直接未定义（CY/TESTS/99_self_test 实测）。
                test_ctx.pending_items = ctx.pending_items.clone();
                let block = convert_block(body, &test_ctx);
                ir_mod.items.push(Item::Test(TestDef {
                    name: name.clone(),
                    body: block,
                }));
            }
            AstStmt::Suite {
                setup,
                teardown,
                tests,
                ..
            } => {
                // 将 suite 的 setup/teardown 内联进每个 test，逐个生成 Item::Test
                for inner in tests {
                    if let AstStmt::Test { name, body } = inner {
                        let mut combined: Vec<AstStmt> = Vec::new();
                        if let Some(s) = setup {
                            combined.extend(s.iter().cloned());
                        }
                        combined.extend(body.iter().cloned());
                        if let Some(td) = teardown {
                            combined.extend(td.iter().cloned());
                        }
                        let mut test_ctx = TypeCtx::new();
                        test_ctx.current_generics = ctx.current_generics.clone();
                        test_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                        test_ctx.top_level_consts = ctx.top_level_consts.clone();
                        test_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                        // 同 lone test 分支：suite 内 test 体的嵌套 def 也要能提升进模块
                        test_ctx.pending_items = ctx.pending_items.clone();
                        let block = convert_block(&combined, &test_ctx);
                        ir_mod.items.push(Item::Test(TestDef {
                            name: name.clone(),
                            body: block,
                        }));
                    }
                }
            }
            _ => {}
        }
    }

    // 11. 扩展语义检查（ERROR_BUG 补漏 20 例）：注入统一错误出口前，
    // 捕获 closure_param_type_mismatch / hash_key_type_mismatch / method_not_found_put /
    // return_type_mismatch2 / dict_key_type_mismatch / enum_field_type_mismatch /
    // field_not_found / method_call_arity / match_branch_type_mismatch /
    // non_exhaustive_match / duplicate_match_pattern / match_subject_type_mismatch /
    // use_result_without_unwrap / variant_type_mismatch / unconstrained_generic_cmp /
    // method_arg_type / str_plus_int / tree_insert_type_mismatch / index_type / mixed_list_type
    check_extended_semantics(ast_module, &ctx);

    // 12. 将提升出的嵌套函数等 pending items 追加到模块
    // 语义错误统一上报（不可变重赋值 E0384 / 空列表类型推断 E0282）
    {
        let errors = ctx.errors.borrow();
        if !errors.is_empty() {
            return Err(IrBuildError::Generic(errors.join("\n")));
        }
    }
    // 先 drop ctx 确保 Rc 引用计数归 1，否则 try_unwrap 静默失败
    drop(ctx);
    if let Ok(items) = Rc::try_unwrap(pending_items) {
        ir_mod.items.extend(items.into_inner());
    }

    // 13. duck 结构匹配编译期检查（具体类型 vs duck 约束）
    let duck_errors = crate::ir::duck_check::check_duck_satisfaction(&ir_mod);
    if !duck_errors.is_empty() {
        return Err(IrBuildError::Generic(duck_errors.join("\n")));
    }

    // 14. __from__ 循环检测（06d §十四）：A↔B 双向 __from__ 会在隐式转换
    // 三触发点（let/实参/返回值）互相递归展开直至栈溢出，编译期报错。
    // 例：struct A { def __from__(b: B) } + struct B { def __from__(a: A) }
    {
        // from_edges[from_ty] = [to_ty, ...]：from_ty::__from__(to_ty)
        let mut from_edges: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for item in &ir_mod.items {
            // __from__ 定义在 impl 块（`impl A = def __from__...`）→ ImplDef.methods；
            // struct 内联方法（若存在）→ StructDef.methods。两处都扫
            let (sname, methods): (Option<&String>, &Vec<crate::ir::node::FnDef>) = match item {
                crate::ir::node::Item::StructDef(sd) => (Some(&sd.name), &sd.methods),
                crate::ir::node::Item::Impl(im) => match &im.for_type {
                    IrType::Named { path, .. } => (Some(path), &im.methods),
                    _ => (None, &im.methods),
                },
                _ => continue,
            };
            let sname = match sname {
                Some(s) => s.clone(),
                None => continue,
            };
            for m in methods {
                if m.name == "__from__" {
                    if let Some(p0) = m.params.first() {
                        if let IrType::Named { path, .. } = &p0.ty {
                            from_edges
                                .entry(sname.clone())
                                .or_default()
                                .push(path.clone());
                        }
                    }
                }
            }
        }
        let mut cyc_errors: Vec<String> = Vec::new();
        let mut reported: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        for (a, targets) in &from_edges {
            for b in targets {
                if let Some(bt) = from_edges.get(b) {
                    if bt.contains(a) && reported.insert((a.clone(), b.clone())) {
                        cyc_errors.push(format!(
                            "__from__ 转换循环：{}::__from__({}) 与 {}::__from__({}) 互为源类型，隐式转换将无限递归（06d §十四）",
                            a, b, b, a
                        ));
                    }
                }
            }
        }
        if !cyc_errors.is_empty() {
            return Err(IrBuildError::Generic(cyc_errors.join("\n")));
        }
    }

    // 15. 尾递归自动优化（TCO）：verdict == TailOptimizable 的 FnDef 就地改写为
    // `while true` 循环（IR 层 desugar ⇒ Rust / Cython 两个后端同时受益，
    // codegen 零改动）。开关：`--no-tco` / `LZ_TCO=0` 关闭**自动**改写，
    // `@tailrec` / `#[tail_call]` 标注的静态校验不受开关影响（已在 convert_decl 完成）。
    rewrite_tco(&mut ir_mod);

    Ok(ir_mod)
}
// force rebuild
