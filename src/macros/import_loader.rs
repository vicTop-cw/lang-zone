// Lang-Zong 编译器 — macros/import_loader.rs
// macro import 机制：跨文件加载宏模块，合并宏/模板定义
//
// 设计目标：
// - `macro import std.macros` 在任意模块中引入宏能力（区别于普通 `import`）
// - 递归加载依赖宏模块（用 ImportResolver 做循环检测）
// - 仅宏模块（含 #!bin macro）可被 macro import

use std::fs;
use std::path::{Path, PathBuf};
use crate::lexer::Token;
use crate::lexer::Lexer;
use crate::macros::expand::{
    extract_macro_defs, extract_template_defs,
    MacroRegistry, TemplateRegistry,
};
use crate::util::import::ImportResolver;

/// 检测 source 首行是否为 `#!bin macro` 宏模块指令
pub fn is_macro_module_source(source: &str) -> bool {
    source.lines().next()
        .map(|line| {
            let t = line.trim();
            t == "#!bin macro" || (t.starts_with("#!bin") && t.contains("macro"))
        })
        .unwrap_or(false)
}

/// 扫描主文件 token 流中的 `macro import` 语句，递归加载所有依赖宏模块。
/// 返回 (合并的宏注册中心, 合并的模板注册中心, 主文件中 import 语句的 token 范围)
pub fn load_macro_imports(
    tokens: &[Token],
    base_dir: &Path,
    resolver: &mut ImportResolver,
) -> Result<(MacroRegistry, TemplateRegistry, Vec<usize>), String> {
    let mut registry = MacroRegistry::new();
    let mut templates = TemplateRegistry::new();
    let mut ranges = Vec::new();
    let len = tokens.len();
    let mut i = 0;
    while i < len {
        // 检测 `macro import`
        if tokens[i] == Token::Macro {
            let mut j = i + 1;
            while j < len && matches!(&tokens[j], Token::Newline | Token::Indent) { j += 1; }
            if j < len && tokens[j] == Token::Import {
                j += 1;
                let mut path = Vec::new();
                // 在同一行内收集路径段（不跨过 Newline）
                loop {
                    while j < len && tokens[j] == Token::Indent { j += 1; }
                    if let Token::Ident(n) = &tokens[j] {
                        path.push(n.clone());
                        j += 1;
                    } else { break; }
                    while j < len && tokens[j] == Token::Indent { j += 1; }
                    if j < len && tokens[j] == Token::PathSep { j += 1; } else { break; }
                }
                // 跳过 as Alias（同行内，初版忽略别名）
                while j < len && tokens[j] == Token::Indent { j += 1; }
                if j < len && tokens[j] == Token::As {
                    j += 1;
                    while j < len && tokens[j] == Token::Indent { j += 1; }
                    if j < len && matches!(&tokens[j], Token::Ident(_)) { j += 1; }
                }
                let stmt_end = find_stmt_end(tokens, j);
                ranges.push(i);
                ranges.push(stmt_end);
                load_module_recursive(&path, base_dir, resolver, &mut registry, &mut templates)?;
                i = stmt_end;
                continue;
            }
        }
        i += 1;
    }
    Ok((registry, templates, ranges))
}

/// 递归加载单个宏模块及其依赖
fn load_module_recursive(
    path: &[String],
    base_dir: &Path,
    resolver: &mut ImportResolver,
    registry: &mut MacroRegistry,
    templates: &mut TemplateRegistry,
) -> Result<(), String> {
    let candidates = ImportResolver::resolve_path(path, base_dir);
    let file: PathBuf = candidates.into_iter().find(|p| p.exists())
        .ok_or_else(|| format!("macro module not found: {}", path.join("::")))?;

    // 循环依赖检测
    resolver.push(path.to_vec(), file.clone())?;

    let source = fs::read_to_string(&file)
        .map_err(|e| format!("error reading {}: {}", file.display(), e))?;
    if !is_macro_module_source(&source) {
        resolver.pop();
        return Err(format!(
            "module '{}' is not a macro module (missing '#!bin macro')",
            path.join("::")
        ));
    }

    let mut lexer = Lexer::new(&source);
    let toks = lexer.tokenize();
    let (r, _) = extract_macro_defs(&toks)?;
    let (t, _) = extract_template_defs(&toks)?;
    registry.merge(r);
    templates.merge(t);

    // 递归加载该模块的依赖（基于该模块所在目录解析相对路径）
    let child_base = file.parent().unwrap_or(base_dir);
    let (sub_r, sub_t, _) = load_macro_imports(&toks, child_base, resolver)?;
    registry.merge(sub_r);
    templates.merge(sub_t);

    resolver.pop();
    Ok(())
}

/// 找到语句结束位置（下一个顶层 Newline 之后）
fn find_stmt_end(tokens: &[Token], start: usize) -> usize {
    let len = tokens.len();
    let mut i = start;
    let mut indent: i32 = 0;
    while i < len {
        match &tokens[i] {
            Token::Newline => { if indent <= 0 { return i + 1; } }
            Token::Indent => indent += 1,
            Token::Dedent => { if indent > 0 { indent -= 1; } }
            _ => {}
        }
        i += 1;
    }
    len
}
