// Lang-Zone 编译器 — project.rs
// 跨模块项目编译器：递归加载 import 的 .lz 文件，合并多模块 AST/IR

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast;
use crate::config::paths::SearchPaths;
use crate::ir::{IrModule, builder::build_ir};
use crate::lexer::Lexer;
use crate::macros::expand::{extract_macro_defs, MacroExpander};
use crate::parser::Parser;
use crate::util::import::ImportResolver;

/// 编译单元：一个 .lz 文件的解析结果
#[derive(Debug)]
pub struct CompileUnit {
    pub path: PathBuf,
    pub module: ast::Module,
}

/// 项目级编译器：递归加载和合并多模块
pub struct ProjectCompiler {
    search_paths: SearchPaths,
    resolver: ImportResolver,
    loaded: HashSet<Vec<String>>,
    units: Vec<CompileUnit>,
    base_dir: PathBuf,
}

impl ProjectCompiler {
    pub fn new(base_dir: PathBuf, std_dir: Option<PathBuf>) -> Self {
        let mut search_paths = SearchPaths::new();
        if let Some(sd) = std_dir {
            search_paths.push(sd);
        }
        search_paths.push(base_dir.clone());
        Self {
            search_paths,
            resolver: ImportResolver::new(),
            loaded: HashSet::new(),
            units: Vec::new(),
            base_dir,
        }
    }

    /// 编译项目入口文件
    pub fn compile(&mut self, entry_file: &Path) -> Result<ast::Module, String> {
        let entry_abs = if entry_file.is_absolute() {
            entry_file.to_path_buf()
        } else {
            self.base_dir.join(entry_file)
        };
        let entry_abs = entry_abs.canonicalize()
            .map_err(|e| format!("Cannot find entry file {:?}: {}", entry_file, e))?;

        self.load_module(&entry_abs, &vec!["main".into()])?;

        self.merge_modules()
    }

    /// 编译输出 IR
    pub fn compile_to_ir(&mut self, entry_file: &Path) -> Result<IrModule, String> {
        let merged = self.compile(entry_file)?;
        build_ir(&merged).map_err(|e| format!("IR build error: {e}"))
    }

    /// 递归加载一个 .lz 模块
    fn load_module(
        &mut self,
        file_path: &Path,
        logical_path: &[String],
    ) -> Result<(), String> {
        // 循环依赖检测
        let path_vec = logical_path.to_vec();
        if self.loaded.contains(&path_vec) {
            return Ok(());
        }

        self.resolver.push(path_vec.clone(), file_path.to_path_buf())
            .map_err(|e| format!("Import error in {}: {}", file_path.display(), e))?;

        // 读取和编译
        let source = fs::read_to_string(file_path)
            .map_err(|e| format!("Cannot read {}: {}", file_path.display(), e))?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize();

        let (registry, _ranges) = extract_macro_defs(&tokens)
            .map_err(|e| format!("Macro error in {}: {}", file_path.display(), e))?;
        let expander = MacroExpander::new(registry);
        let expanded = expander.expand(&tokens)
            .map_err(|e| format!("Expand error in {}: {}", file_path.display(), e))?;

        let mut parser = Parser::new(expanded);
        let module = parser.parse_module()
            .map_err(|e| format!("Parse error in {}: {}", file_path.display(), e))?;

        self.loaded.insert(path_vec.clone());

        // 递归处理 import 语句
        for imp in &module.imports {
            self.load_import(imp, file_path)?;
        }

        self.resolver.pop();
        self.units.push(CompileUnit {
            path: file_path.to_path_buf(),
            module,
        });

        Ok(())
    }

    /// 加载 import 语句指向的模块
    fn load_import(
        &mut self,
        imp: &ast::ImportStmt,
        from_file: &Path,
    ) -> Result<(), String> {
        // 跳过 std/系统 import（由 StdBridge 处理）
        if imp.path.first().map_or(false, |p| p == "std") {
            return Ok(());
        }
        // 跳过非 .lz 的 crate import
        if imp.path.first().map_or(false, |p| {
            ["serde", "tokio", "regex", "chrono", "rand", "itertools",
             "serde_json", "once_cell"].contains(&p.as_str())
        }) {
            return Ok(());
        }

        let import_path = &imp.path;
        let base_dir = from_file.parent().unwrap_or(&self.base_dir);

        // 查找模块文件
        let candidates = ImportResolver::resolve_path(import_path, base_dir);
        let mut found = false;
        for candidate in &candidates {
            if candidate.exists() {
                let abs = candidate.canonicalize()
                    .map_err(|e| format!("Canonicalize error: {}", e))?;
                self.load_module(&abs, import_path)?;
                found = true;
                break;
            }
        }

        if !found {
            // 未找到 .lz 文件 — 不报错，由 StdBridge 后续处理
            // (可能是 crate import，如 serde_json)
        }

        Ok(())
    }

    /// 已加载的模块数
    pub fn unit_count(&self) -> usize {
        self.units.len()
    }

    /// 合并所有编译单元为一个 AST Module
    fn merge_modules(&self) -> Result<ast::Module, String> {
        if self.units.is_empty() {
            return Err("No modules to compile".into());
        }

        let mut merged = ast::Module {
            name: None,
            imports: vec![],
            functions: vec![],
            structs: vec![],
            traits: vec![],
            impls: vec![],
            consts: vec![],
            type_aliases: vec![],
            tests: vec![],
            top_level_builds: vec![],
        };

        for unit in &self.units {
            let m = &unit.module;
            merged.functions.extend(m.functions.clone());
            merged.structs.extend(m.structs.clone());
            merged.traits.extend(m.traits.clone());
            merged.impls.extend(m.impls.clone());
            merged.consts.extend(m.consts.clone());
            merged.type_aliases.extend(m.type_aliases.clone());
            merged.tests.extend(m.tests.clone());
            // imports 去重
            for imp in &m.imports {
                if !merged.imports.iter().any(|existing| {
                    existing.path == imp.path && existing.items == imp.items
                }) {
                    merged.imports.push(imp.clone());
                }
            }
        }

        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_project_compile_single_file() {
        let dir = std::env::temp_dir().join("lz_test_project");
        let _ = fs::create_dir_all(&dir);
        let main_file = dir.join("main.lz");
        fs::write(&main_file, "def hello() -> str =\n    \"world\"\n").unwrap();

        let mut pc = ProjectCompiler::new(dir.clone(), None);
        let module = pc.compile(&main_file).expect("should compile");
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].name, "hello");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cross_module_import() {
        let dir = std::env::temp_dir().join("lz_test_cross");
        let _ = fs::create_dir_all(&dir);

        fs::write(&dir.join("utils.lz"), "def add(a: int, b: int) -> int =\n    a + b\n").unwrap();
        fs::write(&dir.join("main.lz"), "import utils\n\ndef main() =\n    let x = 1\n    x\n").unwrap();

        let mut pc = ProjectCompiler::new(dir.clone(), None);
        let module = pc.compile(&dir.join("main.lz")).expect("should compile");
        // 应包含 main + utils 两个模块的函数
        let names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"add"));

        let _ = fs::remove_dir_all(&dir);
    }
}
