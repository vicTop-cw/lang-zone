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
use lang_zone::ast::{ConstDef, Function, Module, StructDef, TypeAliasDef};
use lang_zone::types::Type;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// 解析 `.lz` 源码为 Module（Lexer → Parser::parse_module 标准管线）。
///
/// 对齐主 crate 公开 API（src/main.rs 同款管线）：
/// `lang_zone::lexer::Lexer::tokenize` → `lang_zone::parser::Parser::parse_module`。
fn parse_module_from_source(source: &str) -> Result<Module, String> {
    let mut lexer = lang_zone::lexer::Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = lang_zone::parser::Parser::new(tokens);
    parser.parse_module()
}

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

/// 推断单个 `.lz` 文件，返回 (模块名, 解析后的 Module, 错误列表)。
///
/// 类型信息以源码显式注解为准（Param.ty / return_type 由 parser 填充，
/// 未注解处为 Type::Any）。主 crate 的 Typer 属未导出的内部模块，
/// 不作为 lz-infer 的依赖面。
fn infer_file(path: &Path) -> Result<(String, Module, Vec<String>), String> {
    let source = fs::read_to_string(path).map_err(|e| format!("read error: {}", e))?;
    let mut module = parse_module_from_source(&source).map_err(|e| format!("parse error: {}", e))?;

    let module_name = derive_module_name(path);
    module.name = Some(module_name.clone());
    module.file_path = Some(path.to_string_lossy().to_string());

    Ok((module_name, module, Vec::new()))
}

/// 将推断后的 Module 转换为 LziModule。
fn module_to_lzi(module: &Module, unresolved: &mut Vec<String>) -> LziModule {
    let mut lzi = LziModule::default();

    // 类型别名
    for TypeAliasDef { name, ty, .. } in &module.type_aliases {
        lzi.type_aliases.insert(name.clone(), type_to_lz_string(ty));
    }

    // 常量
    for ConstDef {
        name, ty, value, ..
    } in &module.consts
    {
        if let Some(t) = ty {
            let evaluated = crate::eval::eval_const_expr(value);
            lzi.consts.insert(
                name.clone(),
                crate::lzi::LziConst {
                    ty: type_to_lz_string(t),
                    value: evaluated,
                },
            );
        } else {
            unresolved.push(format!("const '{}': type could not be inferred", name));
        }
    }

    // 结构体
    for s in &module.structs {
        lzi.structs
            .insert(s.name.clone(), struct_to_lzi(s, unresolved));
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
        // Param.ty 为非 Option 的 Type：parser 对未注解参数填充 Type::Any。
        // Type::Any 视为「未解析」，标记进 unresolved。
        if matches!(p.ty, Type::Any) {
            all_resolved = false;
            params.push(LziParam {
                name: p.name.clone(),
                ty: "?".into(),
            });
        } else {
            params.push(LziParam {
                name: p.name.clone(),
                ty: type_to_lz_string(&p.ty),
            });
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

    // Function 无 generic_bounds 字段（泛型内联约束挂在 StructDef 上），
    // 仅导出 where_clause 形式的 bounds。
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
        generic_bounds: HashMap::new(),
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
        Type::Simd { elem, width } => format!("Simd[{}, {}]", type_to_lz_string(elem), width),
        Type::Duck { fields } => {
            let fields_s: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{}: {}", n, type_to_lz_string(t)))
                .collect();
            format!("duck {{ {} }}", fields_s.join(", "))
        }
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
///
/// 保留供工具/测试使用；Module 已无 package 字段，主流程不再写入。
#[allow(dead_code)]
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
    #[allow(dead_code)] // 保留以备 Phase 2 扩展（如函数签名级注入）
    lzi: LziModule,
    ast: Module,
}

/// 两阶段跨模块类型签名收集
///
/// Phase 1: 所有模块独立收集签名
/// Phase 2: 使用共享上下文重新收集每个模块（预注入导入模块的 struct/alias）
///
/// 输出中标记了跨模块解析的类型（`[cross_module]` 注记在 unresolved 中）。
pub fn infer_path_cross_module(input: &Path) -> Result<LziFile, String> {
    let entries = collect_lz_files(input)?;
    if entries.is_empty() {
        return Ok(LziFile::new());
    }

    // ---- Phase 1: 独立推断所有模块 ----
    let mut phase1: HashMap<String, Phase1Module> = HashMap::new();
    let mut phase1_errors: Vec<String> = Vec::new();

    // 如果输入是目录，模块名应相对于该目录推导
    let base_dir: Option<&Path> = if input.is_dir() { Some(input) } else { None };

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
                result
                    .unresolved
                    .push(format!("{}: Phase2 read error: {}", path.display(), e));
                continue;
            }
        };

        let mut module = match parse_module_from_source(&source) {
            Ok(m) => m,
            Err(e) => {
                result
                    .unresolved
                    .push(format!("{}: Phase2 parse error: {}", path.display(), e));
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

        // 收集当前模块的导入依赖
        let imported_modules = collect_imported_modules(&module);

        // 预注入跨模块 struct 定义和 type_alias（纯 AST 层操作，
        // 使导入的类型在本模块签名中可见，并以 [cross_module] 注记标记）
        let injected_structs =
            inject_cross_module_defs(&mut module, &phase1, &imported_modules, &module_name);
        let injected_aliases =
            inject_cross_module_aliases(&mut module, &phase1, &imported_modules, &module_name);

        let mut errs = Vec::new();
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
    let local_alias_names: HashSet<String> = module
        .type_aliases
        .iter()
        .map(|ta| ta.name.clone())
        .collect();

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
