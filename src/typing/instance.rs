//! Type Class / Trait 实例注册表与隐式推导
//!
//! 本模块提供最小可用的 trait 实例解析：
//! - 从模块的 `trait`/`impl` 定义构建 [`InstanceRegistry`]。
//! - 通过 [`resolve_instance`] 对具体类型查找匹配实例：
//!   1. 精确实例（`impl Show for int`）
//!   2. 泛型实例替换（`impl Show[T] for List[T] where T: Show`）
//!   3. 容器递归派生（`List[int]: Show` 由 `int: Show` + 泛型实例推导）

use std::collections::HashMap;

use crate::ast::{ImplDef, Module, TraitDef, WhereBound};
use crate::types::def::Type;

/// 实例在注册表中的键：trait 名 + 实现目标类型名
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub trait_name: String,
    pub type_name: String,
}

/// 实例匹配结果分类
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceKind {
    /// 精确实例，无泛型参数
    Exact,
    /// 泛型实例，携带泛型参数到具体类型的替换映射
    Generic { subst: HashMap<String, Type> },
    /// 通过标准容器 auto-derive 规则递归推导得到
    Derived,
}

/// 一个 trait 实例条目
#[derive(Debug, Clone)]
pub struct Instance {
    pub trait_name: String,
    pub type_name: String,
    /// impl 头声明的泛型参数名（如 `["T"]`）
    pub generics: Vec<String>,
    /// impl 头的泛型约束（如 `T: Show`）
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// impl 的 where 子句
    pub where_clause: Vec<WhereBound>,
    /// 实现的方法名列表
    pub methods: Vec<String>,
    /// 本次解析得到的匹配方式
    pub kind: InstanceKind,
}

/// Trait 实例注册表
#[derive(Debug, Clone, Default)]
pub struct InstanceRegistry {
    /// (trait, type) → 该键下注册的所有实例（通常一个精确 + 若干泛型）
    instances: HashMap<InstanceKey, Vec<Instance>>,
    /// trait 名 → trait 声明的方法名列表（辅助信息）
    traits: HashMap<String, Vec<String>>,
}

impl InstanceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从整个模块构建注册表
    pub fn from_module(module: &Module) -> Self {
        let mut reg = Self::new();
        for tr in &module.traits {
            reg.register_trait(tr);
        }
        for imp in &module.impls {
            reg.register_impl(imp);
        }
        reg
    }

    /// 注册一个 trait 定义（仅记录方法名，用于后续调试/校验）
    pub fn register_trait(&mut self, tr: &TraitDef) {
        let methods: Vec<String> = tr.methods.iter().map(|m| m.name.clone()).collect();
        self.traits.insert(tr.name.clone(), methods);
    }

    /// 注册一个 impl 定义
    pub fn register_impl(&mut self, imp: &ImplDef) {
        // 只关心 `impl Trait for Type`；inherent impl 不参与实例推导
        let trait_name = match imp.trait_name.as_ref() {
            Some(n) => n.clone(),
            None => return,
        };
        let type_name = imp.type_name.clone();
        let methods: Vec<String> = imp.methods.iter().map(|m| m.name.clone()).collect();
        let kind = if imp.generics.is_empty() {
            InstanceKind::Exact
        } else {
            InstanceKind::Generic {
                subst: HashMap::new(),
            }
        };
        let instance = Instance {
            trait_name: trait_name.clone(),
            type_name: type_name.clone(),
            generics: imp.generics.clone(),
            generic_bounds: imp.generic_bounds.clone(),
            where_clause: imp.where_clause.clone(),
            methods,
            kind,
        };
        self.instances
            .entry(InstanceKey {
                trait_name,
                type_name,
            })
            .or_default()
            .push(instance);
    }

    /// 按精确键查询已注册实例列表
    pub fn get(&self, trait_name: &str, type_name: &str) -> Option<&Vec<Instance>> {
        self.instances.get(&InstanceKey {
            trait_name: trait_name.to_string(),
            type_name: type_name.to_string(),
        })
    }

    /// 查询某 trait 声明的方法名列表
    pub fn trait_methods(&self, trait_name: &str) -> Option<&Vec<String>> {
        self.traits.get(trait_name)
    }
}

/// 对具体类型 `concrete_type` 解析 trait `trait_name` 的可用实例。
///
/// 返回 `None` 表示无法找到或推导所需实例。
pub fn resolve_instance(
    registry: &InstanceRegistry,
    trait_name: &str,
    concrete_type: &Type,
) -> Option<Instance> {
    let mut seen = Vec::new();
    resolve_core(registry, trait_name, concrete_type, &mut seen)
}

fn resolve_core(
    registry: &InstanceRegistry,
    trait_name: &str,
    concrete_type: &Type,
    seen: &mut Vec<(String, Type)>,
) -> Option<Instance> {
    // 防止递归循环（如 T: Foo 需要 T: Foo）
    if seen
        .iter()
        .any(|(t, ty)| t == trait_name && ty == concrete_type)
    {
        return None;
    }
    seen.push((trait_name.to_string(), concrete_type.clone()));
    let result = resolve_once(registry, trait_name, concrete_type, seen);
    seen.pop();
    result
}

fn resolve_once(
    registry: &InstanceRegistry,
    trait_name: &str,
    concrete_type: &Type,
    seen: &mut Vec<(String, Type)>,
) -> Option<Instance> {
    let base_name = canonical_type_name(concrete_type);

    // 1. 精确实例匹配
    if let Some(name) = base_name.as_ref() {
        if let Some(instances) = registry.get(trait_name, name) {
            for inst in instances {
                if inst.generics.is_empty() {
                    return Some(Instance {
                        kind: InstanceKind::Exact,
                        ..inst.clone()
                    });
                }
            }
        }
    }

    // 2. 泛型实例替换匹配
    let (container_name, args) = decompose_type(concrete_type);
    if let Some(name) = container_name.as_ref() {
        if let Some(instances) = registry.get(trait_name, name) {
            for inst in instances {
                if !inst.generics.is_empty() && inst.generics.len() == args.len() {
                    let subst: HashMap<String, Type> = inst
                        .generics
                        .iter()
                        .cloned()
                        .zip(args.iter().cloned())
                        .collect();
                    if check_instance_bounds(registry, inst, &subst, seen) {
                        return Some(Instance {
                            kind: InstanceKind::Generic { subst },
                            ..inst.clone()
                        });
                    }
                }
            }
        }
    }

    // 3. 标准容器递归派生
    if let Some(name) = container_name.as_ref() {
        if crate::typing::bounds::auto_derive_containers(name, trait_name) && !args.is_empty() {
            let mut all_ok = true;
            for arg in &args {
                if resolve_core(registry, trait_name, arg, seen).is_none() {
                    all_ok = false;
                    break;
                }
            }
            if all_ok {
                return Some(Instance {
                    trait_name: trait_name.to_string(),
                    type_name: name.clone(),
                    generics: vec![],
                    generic_bounds: vec![],
                    where_clause: vec![],
                    methods: vec![],
                    kind: InstanceKind::Derived,
                });
            }
        }
    }

    None
}

/// 检查泛型实例在替换后的所有约束是否都能满足
fn check_instance_bounds(
    registry: &InstanceRegistry,
    inst: &Instance,
    subst: &HashMap<String, Type>,
    seen: &mut Vec<(String, Type)>,
) -> bool {
    // generic_bounds: T: Show + Debug
    for (param, bounds) in &inst.generic_bounds {
        let concrete = match subst.get(param) {
            Some(t) => t,
            None => return false,
        };
        for bound in bounds {
            let bound_trait = match trait_name_from_bound(bound) {
                Some(n) => n,
                None => continue,
            };
            if resolve_core(registry, bound_trait, concrete, seen).is_none() {
                return false;
            }
        }
    }

    // where_clause 同样处理
    for wb in &inst.where_clause {
        let concrete = match subst.get(&wb.type_param) {
            Some(t) => t,
            None => continue,
        };
        for bound in &wb.bounds {
            let bound_trait = match trait_name_from_bound(bound) {
                Some(n) => n,
                None => continue,
            };
            if resolve_core(registry, bound_trait, concrete, seen).is_none() {
                return false;
            }
        }
    }

    true
}

/// 从 bound 类型中提取 trait 名
fn trait_name_from_bound(bound: &Type) -> Option<&str> {
    match bound {
        Type::Named(n) => Some(n.as_str()),
        _ => None,
    }
}

/// 把具体类型转换为期刊注册表使用的类型名
fn canonical_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Int => Some("int".to_string()),
        Type::F64 => Some("f64".to_string()),
        Type::Float => Some("float".to_string()),
        Type::Str => Some("str".to_string()),
        Type::Bool => Some("bool".to_string()),
        Type::None_ => Some("None".to_string()),
        Type::Unit => Some("Unit".to_string()),
        Type::Never => Some("Never".to_string()),
        Type::Any => Some("Any".to_string()),
        Type::Named(n) => Some(n.clone()),
        Type::Constructor { name, .. } => Some(name.clone()),
        Type::Generic { base, .. } | Type::Apply { constructor: base, .. } => match base.as_ref() {
            Type::Named(n) | Type::Constructor { name: n, .. } => Some(n.clone()),
            _ => None,
        },
        Type::Option(_) => Some("Option".to_string()),
        Type::Result { .. } => Some("Result".to_string()),
        Type::Optional(_) => Some("Option".to_string()),
        Type::Ref(inner) | Type::MutRef(inner) => canonical_type_name(inner),
        _ => None,
    }
}

/// 把具体类型拆分为（容器/类型名，类型实参列表）
fn decompose_type(ty: &Type) -> (Option<String>, Vec<Type>) {
    match ty {
        Type::Generic { base, args } | Type::Apply { constructor: base, args } => {
            let name = match base.as_ref() {
                Type::Named(n) | Type::Constructor { name: n, .. } => Some(n.clone()),
                _ => None,
            };
            (name, args.clone())
        }
        Type::Option(inner) | Type::Optional(inner) => {
            (Some("Option".to_string()), vec![*inner.clone()])
        }
        Type::Result { ok, err } => {
            (Some("Result".to_string()), vec![*ok.clone(), *err.clone()])
        }
        Type::Ref(inner) | Type::MutRef(inner) => decompose_type(inner),
        _ => (canonical_type_name(ty), vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Function, Param, Stmt, Expr, TraitDef};

    fn show_trait() -> TraitDef {
        TraitDef {
            name: "Show".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            methods: vec![Function {
                name: "show".into(),
                generics: vec![],
                generic_kinds: vec![],
                generic_bounds: vec![],
                generic_defaults: vec![],
                params: vec![Param {
                    name: "self".into(),
                    ty: Some(Type::Self_),
                    default: None,
                    is_mut: false,
                    is_owned: false,
                    is_ref: false,
                    is_positional_only: false,
                }],
                return_type: Some(Type::Str),
                raises: None,
                where_clause: vec![],
                body: vec![],
                is_async: false,
                is_abstract: false,
                comptime: false,
                decorators: vec![],
                attributes: vec![],
                variadic: None,
                params_checker: None,
            }],
            fields: vec![],
            type_aliases: vec![],
        }
    }

    fn exact_int_show() -> ImplDef {
        ImplDef {
            trait_name: Some("Show".into()),
            type_name: "int".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            where_clause: vec![],
            methods: vec![Function {
                name: "show".into(),
                generics: vec![],
                generic_kinds: vec![],
                generic_bounds: vec![],
                generic_defaults: vec![],
                params: vec![Param {
                    name: "self".into(),
                    ty: Some(Type::Int),
                    default: None,
                    is_mut: false,
                    is_owned: false,
                    is_ref: false,
                    is_positional_only: false,
                }],
                return_type: Some(Type::Str),
                raises: None,
                where_clause: vec![],
                body: vec![Stmt::Return(Some(Expr::StrLit("".into())))],
                is_async: false,
                is_abstract: false,
                comptime: false,
                decorators: vec![],
                attributes: vec![],
                variadic: None,
                params_checker: None,
            }],
            type_aliases: vec![],
        }
    }

    fn generic_list_show() -> ImplDef {
        ImplDef {
            trait_name: Some("Show".into()),
            type_name: "List".into(),
            generics: vec!["T".into()],
            generic_kinds: vec![],
            generic_bounds: vec![("T".into(), vec![Type::Named("Show".into())])],
            generic_defaults: vec![],
            where_clause: vec![],
            methods: vec![Function {
                name: "show".into(),
                generics: vec![],
                generic_kinds: vec![],
                generic_bounds: vec![],
                generic_defaults: vec![],
                params: vec![Param {
                    name: "self".into(),
                    ty: Some(Type::Generic {
                        base: Box::new(Type::Named("List".into())),
                        args: vec![Type::Named("T".into())],
                    }),
                    default: None,
                    is_mut: false,
                    is_owned: false,
                    is_ref: false,
                    is_positional_only: false,
                }],
                return_type: Some(Type::Str),
                raises: None,
                where_clause: vec![],
                body: vec![Stmt::Return(Some(Expr::StrLit("".into())))],
                is_async: false,
                is_abstract: false,
                comptime: false,
                decorators: vec![],
                attributes: vec![],
                variadic: None,
                params_checker: None,
            }],
            type_aliases: vec![],
        }
    }

    fn registry() -> InstanceRegistry {
        let mut reg = InstanceRegistry::new();
        reg.register_trait(&show_trait());
        reg.register_impl(&exact_int_show());
        reg.register_impl(&generic_list_show());
        reg
    }

    fn list_of(inner: Type) -> Type {
        Type::Generic {
            base: Box::new(Type::Named("List".into())),
            args: vec![inner],
        }
    }

    #[test]
    fn exact_instance_hit() {
        let reg = registry();
        let inst = resolve_instance(&reg, "Show", &Type::Int);
        assert!(inst.is_some(), "int should have Show instance");
        assert_eq!(inst.unwrap().kind, InstanceKind::Exact);
    }

    #[test]
    fn generic_instance_substitution() {
        let reg = registry();
        let ty = list_of(Type::Int);
        let inst = resolve_instance(&reg, "Show", &ty).expect("List<int> should derive Show");
        match &inst.kind {
            InstanceKind::Generic { subst } => {
                assert_eq!(subst.get("T"), Some(&Type::Int));
            }
            other => panic!("expected Generic instance, got {:?}", other),
        }
    }

    #[test]
    fn container_recursive_derivation() {
        let reg = registry();
        let ty = list_of(list_of(Type::Int));
        let inst = resolve_instance(&reg, "Show", &ty).expect("List<List<int>> should derive Show");
        // Show is not auto-derivable; resolves via generic List<T> where T: Show
        assert!(matches!(inst.kind, InstanceKind::Generic { .. }),
            "expected Generic for List<List<int>> (Show is not auto-derivable)");
    }

    #[test]
    fn missing_instance_returns_none() {
        let reg = registry();
        // bool 没有 Show 实例，且 Show 不在 auto-derive 列表中
        assert!(resolve_instance(&reg, "Show", &Type::Bool).is_none());
        // List<bool> 因元素 bool 不满足而失败
        assert!(resolve_instance(&reg, "Show", &list_of(Type::Bool)).is_none());
    }
}
