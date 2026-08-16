//! 推断核心
//!
//! 扫描 `.lz` 源文件 → 解析 → 局部推断 → 输出 `LziFile`。
//!
//! ## 两阶段跨模块推断
//!
//! - **Phase 1**: 所有模块独立推断（逐文件解析 + 推断）
//! - **Phase 2**: 使用 Phase 1 结果作为跨模块上下文重新推断每个模块
//!   - 预注入其他模块的 struct 定义到当前模块
//!   - 预注入其他模块的 type_alias 到当前模块
//!   - 通过 `LziRegistry` 提供函数签名（仅限模块内函数补充签名）

use crate::lzi::{LziFile, LziFunction, LziModule, LziParam, LziStruct};
use lang_zone::ast::{ConstDef, Function, Module, StructDef, TypeAlias};
use lang_zone::parser::parse_module_from_source;
use lang_zone::typer::Typer;
use lang_zone::types::Type;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// 从文件或目录收集 `.lz` 文件的类型签名。
pub fn infer_path(input: &Path) -> Result<LziFile, String> {
    let mut file = LziFile::new();

    let entries = collect_lz_files(input)?;
    for path in entries {
        match infer_file(&path) {
            Ok((module_name, module, mut errors)) => {
                let lzi_module = module_to_lzi(&module, &mut errors);
                file.modules.insert(module_name, lzi_module);
                file.unresolved.extend(errors);
            }
            Err(e) => {
                file.unresolved.push(format!("{}: {}", path.display(), e));
            }
        }
    }

    Ok(file)
}

/// 推断单个 `.lz` 文件，返回 (模块名, 推断后的 Module, 错误列表)。
fn infer_file(path: &Path) -> Result<(String, Module, Vec<String>), String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("read error: {}", e))?;
    let mut module = parse_module_from_source(&source)
        .map_err(|e| format!("parse error: {}", e))?;

    let module_name = derive_module_name(path);
    module.name = Some(module_name.clone());
    module.file_path = Some(path.to_string_lossy().to_string());
    module.package = derive_package(path);

    let errors = Typer::infer_module(&mut module);
    Ok((module_name, module, errors))
}

/// 将推断后的 Module 转换为 LziModule。
fn module_to_lzi(module: &Module, unresolved: &mut Vec<String>) -> LziModule {
    let mut lzi = LziModule::default();

    // 类型别名
    for TypeAlias { name, ty, .. } in &module.type_aliases {
        lzi.type_aliases.insert(name.clone(), type_to_lz_string(ty));
    }

    // 常量
    for ConstDef { name, ty, value, .. } in &module.consts {
        if let Some(t) = ty {
            let evaluated = crate::eval::eval_const_expr(value);
            lzi.consts.insert(name.clone(), crate::lzi::LziConst { ty: type_to_lz_string(t), value: evaluated });
        } else {
            unresolved.push(format!("const '{}': type could not be inferred", name));
        }
    }

    // 结构体
    for s in &module.structs {
        lzi.structs.insert(s.name.clone(), struct_to_lzi(s, unresolved));
    }

    // 顶层函数
    for f in &module.functions {
        if let Some(sig) = function_to_lzi(f, unresolved) {
            lzi.functions.insert(f.name.clone(), sig);
        }
    }

    // impl 方法
    for imp in &module.impls {
        for m in &imp.methods {
            if let Some(sig) = function_to_lzi(m, unresolved) {
                let qualified = format!("{}.{}", imp.type_name, m.name);
                lzi.functions.insert(qualified, sig);
            }
        }
    }

    lzi
}

fn struct_to_lzi(s: &StructDef, unresolved: &mut Vec<String>) -> LziStruct {
    let mut fields = HashMap::new();
    for f in &s.fields {
        fields.insert(f.name.clone(), type_to_lz_string(&f.ty));
    }

    let mut methods = HashMap::new();
    for m in &s.methods {
        if let Some(sig) = function_to_lzi(m, unresolved) {
            methods.insert(m.name.clone(), sig);
        }
    }

    LziStruct { fields, methods }
}

fn function_to_lzi(f: &Function, unresolved: &mut Vec<String>) -> Option<LziFunction> {
    let mut params = Vec::new();
    let mut all_resolved = true;

    for p in &f.params {
        match &p.ty {
            Some(t) => params.push(LziParam {
                name: p.name.clone(),
                ty: type_to_lz_string(t),
            }),
            None => {
                all_resolved = false;
                params.push(LziParam {
                    name: p.name.clone(),
                    ty: "?".into(),
                });
            }
        }
    }

    let return_type = f.return_type.as_ref().map(type_to_lz_string);
    if return_type.is_none() && f.name != "main" {
        // main 默认 Unit，其他缺失返回类型则标记
        if !f.body.is_empty() {
            all_resolved = false;
        }
    }

    if !all_resolved {
        unresolved.push(format!(
            "{}: some parameter/return types could not be inferred",
            f.name
        ));
        // 仍然输出已解析的部分
    }

    let mut generic_bounds = HashMap::new();
    for (name, bounds) in &f.generic_bounds {
        generic_bounds.insert(
            name.clone(),
            bounds.iter().map(type_to_lz_string).collect(),
        );
    }

    let mut where_clause = HashMap::new();
    for wb in &f.where_clause {
        where_clause.insert(
            wb.type_param.clone(),
            wb.bounds.iter().map(type_to_lz_string).collect(),
        );
    }

    Some(LziFunction {
        params,
        return_type,
        raises: f.raises.as_ref().map(type_to_lz_string),
        generics: f.generics.clone(),
        generic_bounds,
        where_clause,
    })
}

/// 将 `lang_zone::types::Type` 转换为 LZ 语法字符串。
pub fn type_to_lz_string(ty: &Type) -> String {
    match ty {
        Type::Int => "int".into(),
        Type::F64 | Type::Float => "f64".into(),
        Type::Str => "str".into(),
        Type::Bool => "bool".into(),
        Type::Unit => "Unit".into(),
        Type::Never => "Never".into(),
        Type::Any => "Any".into(),
        Type::None_ => "None".into(),
        Type::Self_ => "Self".into(),
        Type::Named(name) => name.clone(),
        Type::Var(_) => "?".into(),
        Type::Constructor { name, .. } => name.clone(),
        Type::Apply { constructor, args } => {
            let base_s = type_to_lz_string(constructor);
            let args_s: Vec<String> = args.iter().map(type_to_lz_string).collect();
            format!("{}<{}>", base_s, args_s.join(", "))
        }
        Type::Generic { base, args } => {
            let base_s = type_to_lz_string(base);
            let args_s: Vec<String> = args.iter().map(type_to_lz_string).collect();
            format!("{}<{}>", base_s, args_s.join(", "))
        }
        Type::Option(inner) => format!("Option<{}>", type_to_lz_string(inner)),
        Type::Result { ok, err } => format!(
            "Result<{}, {}>",
            type_to_lz_string(ok),
            type_to_lz_string(err)
        ),
        Type::Optional(inner) => format!("{}?", type_to_lz_string(inner)),
        Type::Ref(inner) => format!("&{}", type_to_lz_string(inner)),
        Type::MutRef(inner) => format!("&mut {}", type_to_lz_string(inner)),
        Type::Fn { params, ret } => {
            let params_s: Vec<String> = params.iter().map(type_to_lz_string).collect();
            format!("fn({}) -> {}", params_s.join(", "), type_to_lz_string(ret))
        }
        Type::Tuple(elems) => {
            let elems_s: Vec<String> = elems.iter().map(type_to_lz_string).collect();
            format!("({})", elems_s.join(", "))
        }
        Type::Record(fields) => {
            let fields_s: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{}: {}", n, type_to_lz_string(t)))
                .collect();
            format!("{{ {} }}", fields_s.join(", "))
        }
        Type::Simd { elem, width } => format!("Simd[{}, {}]", type_to_lz_string(elem), width),
        Type::Future(inner) => format!("Future<{}>", type_to_lz_string(inner)),
        Type::Futures(types) => {
            let s: Vec<String> = types.iter().map(type_to_lz_string).collect();
            format!("Futures<{}>", s.join(", "))
        }
        Type::Intersection(members) => {
            let members_s: Vec<String> = members.iter().map(type_to_lz_string).collect();
            members_s.join(" & ")
        }
        Type::Union(members) => {
            let members_s: Vec<String> = members.iter().map(type_to_lz_string).collect();
            members_s.join(" | ")
        }
        Type::PathDependent { path, member } => format!("{}.{}", path, member),
        Type::Wildcard => "_".into(),
    }
}

/// 递归收集目录下所有 `.lz` 文件（排除 target/ 与点开头的目录）。
fn collect_lz_files(input: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    if input.is_file() {
        if input.extension().and_then(|s| s.to_str()) == Some("lz") {
            result.push(input.to_path_buf());
        }
        return Ok(result);
    }
    if !input.is_dir() {
        return Err(format!("'{}' is not a file or directory", input.display()));
    }

    for entry in fs::read_dir(input).map_err(|e| format!("read dir error: {}", e))? {
        let entry = entry.map_err(|e| format!("dir entry error: {}", e))?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if path.is_dir() {
            result.extend(collect_lz_files(&path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("lz") {
            result.push(path);
        }
    }
    Ok(result)
}

/// 从文件路径推导模块名（相对于指定 base 目录）。
///
/// 例如 base=`src/`, path=`src/utils/math.lz` → `"utils::math"`.
fn derive_module_name_relative(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    derive_module_name(rel)
}

/// 从文件路径推导包名（相对于指定 base 目录）。
fn derive_package_relative(path: &Path, base: Option<&Path>) -> Option<String> {
    if let Some(base) = base {
        let rel = path.strip_prefix(base).unwrap_or(path);
        derive_package(rel)
    } else {
        derive_package(path)
    }
}

/// 从文件路径推导模块名（src/utils/math.lz → "src::utils::math"）。
fn derive_module_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let parts: Vec<String> = path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        stem
    } else {
        format!("{}::{}", parts.join("::"), stem)
    }
}

/// 从文件路径推导包名（src/utils/math.lz → Some("src::utils")）。
fn derive_package(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let parts: Vec<String> = parent
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("::"))
    }
}

// ===========================================================================
// 两阶段跨模块推断
// ===========================================================================

/// Phase 1 结果：一个模块独立推断后的完整信息
struct Phase1Module {
    lzi: LziModule,
    ast: Module,
}

/// 两阶段跨模块类型推断
///
/// Phase 1: 所有模块独立推断
/// Phase 2: 使用共享上下文重新推断每个模块
///
/// 输出中标记了跨模块解析的类型（`#[cross_module]` 注记在 unresolved 中）。
pub fn infer_path_cross_module(input: &Path) -> Result<LziFile, String> {
    let entries = collect_lz_files(input)?;
    if entries.is_empty() {
        return Ok(LziFile::new());
    }

    // ---- Phase 1: 独立推断所有模块 ----
    let mut phase1: HashMap<String, Phase1Module> = HashMap::new();
    let mut phase1_errors: Vec<String> = Vec::new();

    // 如果输入是目录，模块名应相对于该目录推导
    let base_dir: Option<&Path> = if input.is_dir() {
        Some(input)
    } else {
        None
    };

    for path in &entries {
        match infer_file(path) {
            Ok((name, module, errors)) => {
                // 如果是目录输入，重写模块名为相对路径
                let module_name = if let Some(base) = base_dir {
                    derive_module_name_relative(path, base)
                } else {
                    name
                };
                let mut errs = errors.clone();
                let lzi = module_to_lzi(&module, &mut errs);
                phase1.insert(module_name, Phase1Module { lzi, ast: module });
                phase1_errors.extend(errs);
            }
            Err(e) => {
                phase1_errors.push(format!("{}: Phase1 failed: {}", path.display(), e));
            }
        }
    }

    // ---- Phase 2: 带跨模块上下文重新推断 ----
    let mut result = LziFile::new();
    let mut cross_module_count: usize = 0;

    for path in &entries {
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                result.unresolved.push(format!("{}: Phase2 read error: {}", path.display(), e));
                continue;
            }
        };

        let mut module = match parse_module_from_source(&source) {
            Ok(m) => m,
            Err(e) => {
                result.unresolved.push(format!("{}: Phase2 parse error: {}", path.display(), e));
                continue;
            }
        };

        let module_name = if let Some(base) = base_dir {
            derive_module_name_relative(path, base)
        } else {
            derive_module_name(path)
        };
        module.name = Some(module_name.clone());
        module.file_path = Some(path.to_string_lossy().to_string());
        module.package = derive_package_relative(path, base_dir);

        // 收集当前模块的导入依赖
        let imported_modules = collect_imported_modules(&module);

        // 预注入跨模块 struct 定义和 type_alias
        let injected_structs = inject_cross_module_defs(&mut module, &phase1, &imported_modules, &module_name);
        let injected_aliases = inject_cross_module_aliases(&mut module, &phase1, &imported_modules, &module_name);

        // 构建 LziRegistry（从 Phase 1 其他模块的签名）
        let registry = build_phase1_registry(&phase1, &module_name);

        // 使用跨模块上下文推断
        let errors = Typer::infer_module_with(&mut module, registry.as_ref());
        let mut errs = errors.clone();
        let lzi_module = module_to_lzi(&module, &mut errs);

        // 标记跨模块解析的项
        if injected_structs > 0 || injected_aliases > 0 {
            cross_module_count += 1;
            if injected_structs > 0 {
                errs.push(format!(
                    "[cross_module] {} struct(s) injected from other modules",
                    injected_structs
                ));
            }
            if injected_aliases > 0 {
                errs.push(format!(
                    "[cross_module] {} type alias(es) injected from other modules",
                    injected_aliases
                ));
            }
        }

        result.modules.insert(module_name, lzi_module);
        result.unresolved.extend(errs);
    }

    // 合并 Phase 1 的未解决问题（去重后的）
    result.unresolved.extend(phase1_errors);

    if cross_module_count > 0 {
        eprintln!(
            "Cross-module inference: {} / {} module(s) received cross-module context",
            cross_module_count,
            result.modules.len()
        );
    }

    Ok(result)
}

/// 从 Module 的 import 语句中提取导入的模块名列表
fn collect_imported_modules(module: &Module) -> Vec<String> {
    module
        .imports
        .iter()
        .filter_map(|imp| {
            if imp.path.is_empty() {
                None
            } else {
                Some(imp.path.join("::"))
            }
        })
        .collect()
}

/// 预注入跨模块 struct 定义到当前 module.structs
///
/// 仅注入本模块 import 了的模块中的 struct（使用来源模块的 Phase 1 AST）。
/// 返回注入的 struct 数量。
fn inject_cross_module_defs(
    module: &mut Module,
    phase1: &HashMap<String, Phase1Module>,
    imported: &[String],
    _current_module: &str,
) -> usize {
    let local_struct_names: HashSet<String> =
        module.structs.iter().map(|s| s.name.clone()).collect();

    let mut injected = 0;
    for import_name in imported {
        if let Some(other) = phase1.get(import_name) {
            for s in &other.ast.structs {
                // 跳过同名 struct（本地定义优先）
                if local_struct_names.contains(&s.name) {
                    continue;
                }
                // 注入结构体定义（不含方法，避免引入不必要的复杂度）
                let mut s_copy = s.clone();
                s_copy.methods.clear();
                module.structs.push(s_copy);
                injected += 1;
            }
        }
    }

    injected
}

/// 预注入跨模块 type_alias 到当前 module.type_aliases
///
/// 返回注入的 type_alias 数量。
fn inject_cross_module_aliases(
    module: &mut Module,
    phase1: &HashMap<String, Phase1Module>,
    imported: &[String],
    _current_module: &str,
) -> usize {
    let local_alias_names: HashSet<String> =
        module.type_aliases.iter().map(|ta| ta.name.clone()).collect();

    let mut injected = 0;
    for import_name in imported {
        if let Some(other) = phase1.get(import_name) {
            for ta in &other.ast.type_aliases {
                // 跳过同名 type_alias（本地定义优先）
                if local_alias_names.contains(&ta.name) {
                    continue;
                }
                module.type_aliases.push(ta.clone());
                injected += 1;
            }
        }
    }

    injected
}

/// 从 Phase 1 结果构建 `lang_zone::infer::LziRegistry`
///
/// 通过 JSON 往返转换：`lz_infer::lzi::LziFile` → JSON → `lang_zone::infer::LziFile`。
/// exclude_module 为当前模块名，不包含在 registry 中。
fn build_phase1_registry(
    phase1: &HashMap<String, Phase1Module>,
    exclude_module: &str,
) -> Option<lang_zone::infer::LziRegistry> {
    // 构建仅包含其他模块的 LziFile
    let mut external = LziFile::new();
    for (name, m) in phase1 {
        if name != exclude_module {
            external.modules.insert(name.clone(), m.lzi.clone());
        }
    }

    if external.modules.is_empty() {
        return None;
    }

    // JSON 往返转换为 lang_zone::infer::LziFile
    let json = external.to_json().ok()?;
    let lz_file = lang_zone::infer::LziFile::from_json(&json).ok()?;
    let mut reg = lang_zone::infer::LziRegistry::new();
    reg.files.push(lz_file);
    Some(reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_to_lz_basic() {
        assert_eq!(type_to_lz_string(&Type::Int), "int");
        assert_eq!(type_to_lz_string(&Type::Str), "str");
        assert_eq!(
            type_to_lz_string(&Type::Generic {
                base: Box::new(Type::Named("List".into())),
                args: vec![Type::Int],
            }),
            "List<int>"
        );
    }

    #[test]
    fn derive_name_from_path() {
        let p = Path::new("src/utils/math.lz");
        assert_eq!(derive_module_name(p), "src::utils::math");
        assert_eq!(derive_package(p), Some("src::utils".into()));
    }
}
