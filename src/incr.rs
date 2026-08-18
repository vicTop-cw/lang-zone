//! 增量编译管线（FIST T4.3 / LZ_UPGRADE_PLAN 第4章 方向B）
//!
//! 目标：
//! 1. 模块级缓存：`源码哈希 → IR → 生成代码`，未变更模块直接读缓存产物；
//! 2. 依赖图：import 关系构建模块依赖图，变更模块 + 依赖它的下游级联失效；
//! 3. 缓存失效：直接失效（源码哈希变化）+ 传播失效（下游级联）；
//! 4. 并行编译：`feature = "parallel"`（rayon）下，独立模块并行编译；
//! 5. 增量诊断：只报告变更模块的错误，命中模块不参与诊断。
//!
//! 缓存布局（cache_dir，默认 `.lzcache_incr`）：
//! - `<rel_key>.lzcache`：模块元数据（源哈希 / 依赖哈希 / 产物文件名），复用 cache::CacheEntry 格式；
//! - `<rel_key>.rs`：该模块独立生成的 Rust 代码片段；
//! - 缓存键 `<rel_key>` = 模块文件相对 base_dir 的路径（`/` → `_`）。
//!
//! 输出：按依赖拓扑序拼接各模块代码片段，与全量合并编译保持行为一致；
//! 缓存命中输出与增量全量输出逐字符一致（golden 校验见 tests/incremental_golden.rs）。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast;
use crate::cache::{self, CacheEntry};
use crate::ir::{builder::build_ir, codegen::CodeGen as IrCodeGen};
use crate::lexer::{Lexer, Token};
use crate::macros::expand::{
    contains_pending_call, extract_macro_defs, extract_template_defs, MacroExpander,
    TemplateExpander,
};
use crate::parser::Parser;
use crate::util::import::ImportResolver;

/// crate import 白名单（与 project.rs load_import 保持一致）
const CRATE_IMPORTS: &[&str] = &[
    "serde",
    "tokio",
    "regex",
    "chrono",
    "rand",
    "itertools",
    "serde_json",
    "once_cell",
];

/// 增量编译统计
#[derive(Debug, Clone, Default)]
pub struct IncrStats {
    pub total: usize,
    pub hits: usize,
    pub misses: usize,
    pub elapsed_ms: u128,
}

/// 增量编译结果
#[derive(Debug)]
pub struct IncrOutcome {
    /// 拼接后的完整 Rust 源码
    pub code: String,
    pub stats: IncrStats,
    /// 本次重编译的模块（相对路径）
    pub rebuilt: Vec<String>,
    /// 本次命中缓存的模块（相对路径）
    pub cached: Vec<String>,
}

/// 单个模块信息（收集阶段构建）
#[derive(Debug, Clone)]
struct IncrModule {
    /// 模块绝对路径
    path: PathBuf,
    /// 相对 base_dir 的路径（缓存键 + 展示）
    rel: String,
    /// import 依赖的模块绝对路径（非 std/crate）
    deps: Vec<PathBuf>,
    /// 跨模块宏导入文件（import macro X）——编译期依赖，不传播
    macro_files: Vec<PathBuf>,
}

/// 增量编译器
#[derive(Debug)]
pub struct IncrCompiler {
    base_dir: PathBuf,
    cache_dir: PathBuf,
    modules: Vec<IncrModule>,
    /// 模块路径 → 模块索引（依赖图反向查询用）
    index: HashMap<PathBuf, usize>,
}

impl IncrCompiler {
    pub fn new(base_dir: PathBuf, cache_dir: PathBuf) -> Self {
        IncrCompiler {
            base_dir,
            cache_dir,
            modules: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// 默认缓存目录名（对齐 cache.rs 的 `.lzcache` 风格）
    pub fn default_cache_dir() -> &'static str {
        ".lzcache_incr"
    }

    /// 增量编译入口：收集模块 → 变更判定 → 传播失效 → 重编/复用 → 拼接输出
    pub fn compile(&mut self, entry_file: &Path) -> Result<IncrOutcome, String> {
        let started = std::time::Instant::now();
        // base_dir 规范化（Windows verbatim 前缀 `\\?\`），保证与 canonicalize 后的模块路径可 strip_prefix
        if let Ok(cb) = self.base_dir.canonicalize() {
            self.base_dir = cb;
        }
        let entry_abs = self.resolve_entry(entry_file)?;

        // 1. 收集模块（拓扑序：依赖在前）
        let mut loaded: HashSet<PathBuf> = HashSet::new();
        self.collect_module(&entry_abs, &mut loaded)?;
        if self.modules.is_empty() {
            return Err(format!(
                "Incremental compile: no modules collected from {}",
                entry_abs.display()
            ));
        }

        // 2. 构建反向索引（依赖图）
        for (i, m) in self.modules.iter().enumerate() {
            self.index.insert(m.path.clone(), i);
        }

        // 3. 直接失效判定
        let mut dirty: HashSet<PathBuf> = HashSet::new();
        for m in &self.modules {
            if !self.cache_fresh(m) {
                dirty.insert(m.path.clone());
            }
        }

        // 4. 传播失效：变更模块的下游（依赖它的模块）级联重编
        //    依赖方向：A.deps 含 B → A 依赖 B → B 变更时 A 重编
        let mut queue: Vec<PathBuf> = dirty.iter().cloned().collect();
        while let Some(changed) = queue.pop() {
            for m in &self.modules {
                if m.deps.iter().any(|d| *d == changed) && !dirty.contains(&m.path) {
                    dirty.insert(m.path.clone());
                    queue.push(m.path.clone());
                }
            }
        }

        // 5. 编译 dirty / 复用 hit
        let mut rebuilt = Vec::new();
        let mut cached = Vec::new();
        let mut code_parts: Vec<(usize, String)> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        #[cfg(feature = "parallel")]
        let dirty_results: Vec<(PathBuf, Result<String, String>, Option<String>)> = {
            use rayon::prelude::*;
            let dirty_vec: Vec<IncrModule> = self
                .modules
                .iter()
                .filter(|m| dirty.contains(&m.path))
                .cloned()
                .collect();
            dirty_vec
                .par_iter()
                .map(|m| {
                    let r = compile_module(&m.path);
                    let cache_err = r.as_ref().ok().and_then(|code| self.save_cache(m, code));
                    (m.path.clone(), r, cache_err)
                })
                .collect()
        };

        #[cfg(not(feature = "parallel"))]
        let dirty_results: Vec<(PathBuf, Result<String, String>, Option<String>)> = self
            .modules
            .iter()
            .filter(|m| dirty.contains(&m.path))
            .map(|m| {
                let r = compile_module(&m.path);
                let cache_err = r.as_ref().ok().and_then(|code| self.save_cache(m, code));
                (m.path.clone(), r, cache_err)
            })
            .collect();

        for (path, result, cache_err) in dirty_results {
            if let Some(e) = cache_err {
                errors.push(e);
            }
            match result {
                Ok(code) => {
                    if let Some(idx) = self.index.get(&path) {
                        code_parts.push((*idx, code));
                    }
                    rebuilt.push(self.rel_of(&path));
                }
                Err(e) => errors.push(e),
            }
        }

        // 命中缓存：直接读产物片段
        for m in &self.modules {
            if !dirty.contains(&m.path) {
                match self.load_cached_code(m) {
                    Ok(code) => {
                        code_parts.push((self.index[&m.path], code));
                        cached.push(m.rel.clone());
                    }
                    Err(e) => errors.push(format!("Cache read error in {}: {}", m.rel, e)),
                }
            }
        }

        // 6. 增量诊断：有错误立即失败（只含变更模块错误）
        if !errors.is_empty() {
            return Err(format!("Incremental compile failed:\n{}", errors.join("\n")));
        }

        // 7. 按拓扑序拼接
        //    每个模块的独立 codegen 产物含完整头块（allow 属性 / prelude use）与
        //    模块元数据常量（__name__ 等），直接拼接会重复定义（E0252/E0428）。
        //    拼接规则：
        //    - prelude use（std::* / lz_builtins::*）：只保留拓扑序首个模块；
        //    - 模块间 use（use lib_math; 等）：按行文本去重保留（多个模块 import 同一依赖时防 E0252）；
        //    - allow 属性：只保留首个模块；
        //    - 模块元数据常量（__name__/__file__/__package__/__path__/__doc__/__is_macro__）：只保留 entry；
        //    - 非 entry 模块的占位 main（auto-generated 空 main）：整体跳过，仅保留 entry 的 main。
        code_parts.sort_by_key(|(idx, _)| *idx);
        let mut code = String::new();
        let mut emitted_uses: HashSet<String> = HashSet::new();
        for (idx, part) in &code_parts {
            let is_entry = *idx == self.modules.len() - 1;
            let mut in_placeholder_main = false;
            for line in part.lines() {
                let t = line.trim_start();
                // 非 entry 的占位 main：从 `pub fn main()` 到闭合大括号整体跳过
                if !is_entry && t.starts_with("pub fn main()") {
                    in_placeholder_main = true;
                    continue;
                }
                if in_placeholder_main {
                    if line.trim_end().ends_with('}') {
                        in_placeholder_main = false;
                    }
                    continue;
                }
                // use 导入：prelude 只保留首模块；其余按文本去重
                if t.starts_with("use ") {
                    let key = t.to_string();
                    let is_prelude = t.starts_with("use std::") || t.starts_with("use lz_builtins::");
                    if (is_prelude && *idx != 0) || emitted_uses.contains(&key) {
                        continue;
                    }
                    emitted_uses.insert(key);
                    code.push_str(line);
                    code.push('\n');
                    continue;
                }
                // allow 属性：只保留首模块
                if t.starts_with("#[allow(") {
                    if *idx == 0 {
                        code.push_str(line);
                        code.push('\n');
                    }
                    continue;
                }
                // 模块元数据常量：只保留 entry
                if t.starts_with("const __name__:")
                    || t.starts_with("const __file__:")
                    || t.starts_with("const __package__:")
                    || t.starts_with("const __path__:")
                    || t.starts_with("const __doc__:")
                    || t.starts_with("const __is_macro__:")
                {
                    if is_entry {
                        code.push_str(line);
                        code.push('\n');
                    }
                    continue;
                }
                code.push_str(line);
                code.push('\n');
            }
        }

        Ok(IncrOutcome {
            code,
            stats: IncrStats {
                total: self.modules.len(),
                hits: cached.len(),
                misses: rebuilt.len(),
                elapsed_ms: started.elapsed().as_millis(),
            },
            rebuilt,
            cached,
        })
    }

    /// 解析入口文件绝对路径
    fn resolve_entry(&self, entry_file: &Path) -> Result<PathBuf, String> {
        let entry_abs = if entry_file.is_absolute() {
            entry_file.to_path_buf()
        } else {
            // CLI 传的是相对当前工作目录的路径（如 DEMO/08_modules/use_services.lz）
            std::env::current_dir()
                .map_err(|e| format!("Cannot get current dir: {}", e))?
                .join(entry_file)
        };
        entry_abs
            .canonicalize()
            .map_err(|e| format!("Cannot find entry file {:?}: {}", entry_file, e))
    }

    /// 递归收集模块（DFS 后序 → 拓扑序）
    fn collect_module(&mut self, abs_path: &Path, loaded: &mut HashSet<PathBuf>) -> Result<(), String> {
        if loaded.contains(abs_path) {
            return Ok(());
        }
        loaded.insert(abs_path.to_path_buf());

        // 读取源码
        let source = fs::read_to_string(abs_path)
            .map_err(|e| format!("Cannot read {}: {}", abs_path.display(), e))?;

        // 解析出 AST（获取 imports），不生成代码
        let module = parse_source(&source, abs_path)?;

        let rel = self.rel_of(abs_path);
        let mut deps: Vec<PathBuf> = Vec::new();
        let mut macro_files: Vec<PathBuf> = Vec::new();

        // 宏导入依赖（import macro X / from macro X import Y）
        collect_macro_import_files(&source, abs_path, &mut macro_files);

        // 递归处理 import（非 std/crate）
        for imp in &module.imports {
            if imp.path.first().map_or(false, |p| p == "std") {
                continue;
            }
            if imp.path.first().map_or(false, |p| CRATE_IMPORTS.contains(&p.as_str())) {
                continue;
            }
            let base_dir = abs_path.parent().unwrap_or(&self.base_dir);
            let candidates = ImportResolver::resolve_path(&imp.path, base_dir);
            let mut found = false;
            for candidate in &candidates {
                if candidate.exists() {
                    let dep_abs = candidate
                        .canonicalize()
                        .map_err(|e| format!("Canonicalize error: {}", e))?;
                    deps.push(dep_abs.clone());
                    self.collect_module(&dep_abs, loaded)?;
                    found = true;
                    break;
                }
            }
            // 未找到 .lz 文件 — 不报错（可能是 crate import，后续由 StdBridge 处理）
            let _ = found;
        }

        self.modules.push(IncrModule {
            path: abs_path.to_path_buf(),
            rel,
            deps,
            macro_files,
        });
        Ok(())
    }

    /// 相对 base_dir 的展示/缓存键路径（字符串前缀剥离，避免 verbatim 路径组件比较差异）
    fn rel_of(&self, abs: &Path) -> String {
        let abs_s = abs.to_string_lossy().replace('\\', "/");
        let base_s = self.base_dir.to_string_lossy().replace('\\', "/");
        if base_s.is_empty() || !abs_s.starts_with(&base_s) {
            // 回退：仅文件名（极少见，仍保证缓存键唯一性由路径哈希兜底）
            return abs
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| abs_s.clone());
        }
        abs_s[base_s.len()..]
            .trim_start_matches('/')
            .to_string()
    }

    /// 缓存键（相对路径 → 安全文件名）
    fn cache_key(&self, m: &IncrModule) -> String {
        m.rel.replace('/', "_").replace(".lz", ".lzcache")
    }

    /// 缓存元数据文件路径
    fn meta_path(&self, m: &IncrModule) -> PathBuf {
        self.cache_dir.join(self.cache_key(m))
    }

    /// 代码片段产物文件路径
    fn code_path(&self, m: &IncrModule) -> PathBuf {
        self.cache_dir.join(format!(
            "{}_code.rs",
            m.rel.replace('/', "_").replace(".lz", "")
        ))
    }

    /// 检查模块缓存是否新鲜（源哈希 + 依赖哈希 + 产物存在）
    fn cache_fresh(&self, m: &IncrModule) -> bool {
        let meta = match CacheEntry::load_from(&self.meta_path(m)) {
            Ok(Some(e)) => e,
            _ => return false,
        };
        // 源哈希
        let cur_src = cache::content_hash(&m.path).unwrap_or_default();
        if cur_src != meta.hash {
            return false;
        }
        // 依赖哈希（AST imports 模块 + 宏导入文件）
        let mut deps: Vec<(String, String)> = Vec::new();
        for d in &m.deps {
            deps.push((d.to_string_lossy().to_string(), cache::content_hash(d).unwrap_or_default()));
        }
        for f in &m.macro_files {
            deps.push((f.to_string_lossy().to_string(), cache::content_hash(f).unwrap_or_default()));
        }
        if deps.len() != meta.deps.len() {
            return false;
        }
        for (p, h) in &deps {
            if !meta.deps.iter().any(|(mp, mh)| mp == p && mh == h) {
                return false;
            }
        }
        // 产物存在
        self.code_path(m).exists()
    }

    /// 保存模块缓存（元数据 + 代码片段）
    fn save_cache(&self, m: &IncrModule, code: &str) -> Option<String> {
        let entry = CacheEntry {
            hash: cache::content_hash(&m.path).unwrap_or_default(),
            deps: {
                let mut deps = Vec::new();
                for d in &m.deps {
                    deps.push((d.to_string_lossy().to_string(), cache::content_hash(d).unwrap_or_default()));
                }
                for f in &m.macro_files {
                    deps.push((f.to_string_lossy().to_string(), cache::content_hash(f).unwrap_or_default()));
                }
                deps
            },
            output: String::new(), // 不使用 output 字段；代码路径固定
        };
        if let Err(e) = entry.save_to(&self.meta_path(m)) {
            return Some(format!("Cache save error in {}: {}", m.rel, e));
        }
        if let Err(e) = fs::write(self.code_path(m), code) {
            return Some(format!("Code cache write error in {}: {}", m.rel, e));
        }
        None
    }

    /// 从缓存加载代码片段
    fn load_cached_code(&self, m: &IncrModule) -> Result<String, String> {
        fs::read_to_string(self.code_path(m))
            .map_err(|e| format!("read {}: {}", m.rel, e))
    }
}

/// 单模块编译（纯函数）：lexer → 宏展开 → parser → IR → codegen
fn compile_module(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
    let module = parse_source(&source, path)?;
    let ir = build_ir(&module)
        .map_err(|e| format!("IR build error in {}: {}", path.display(), e))?;
    let ir = ir
        .with_file_path(path.to_string_lossy().to_string())
        .with_source_text(source);
    let mut cg = IrCodeGen::new();
    Ok(cg.generate(&ir))
}

/// 单文件完整前端管线：读取 → 宏展开（含跨模块宏导入）→ parse → AST Module
fn parse_source(source: &str, path: &Path) -> Result<ast::Module, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();

    // 宏/template 定义提取
    let (mut registry, mut macro_ranges) =
        extract_macro_defs(&tokens).map_err(|e| format!("Macro error in {}: {}", path.display(), e))?;
    let (mut template_registry, template_ranges) = extract_template_defs(&tokens)
        .map_err(|e| format!("Template error in {}: {}", path.display(), e))?;
    macro_ranges.extend(template_ranges);

    // 跨模块宏导入：`import macro X` / `from macro X import Y`
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut i = 0usize;
    while i < tokens.len() {
        let is_import = tokens[i] == Token::Import || tokens[i] == Token::From;
        if is_import {
            let mut j = i + 1;
            let mut is_macro_import = false;
            let mut mod_name: Option<String> = None;
            while j < tokens.len() {
                match &tokens[j] {
                    Token::Macro => {
                        is_macro_import = true;
                        j += 1;
                    }
                    Token::Ident(n) if mod_name.is_none() => {
                        mod_name = Some(n.clone());
                        j += 1;
                    }
                    Token::Newline | Token::Dedent => break,
                    _ => {
                        if tokens[j] == Token::Import {
                            j += 1;
                        } else if tokens[j] == Token::As {
                            j += 2;
                        } else {
                            break;
                        }
                    }
                }
                if tokens[j] == Token::Newline {
                    break;
                }
            }
            if is_macro_import {
                if let Some(mname) = mod_name {
                    let macro_path = dir.join(format!("{}.lz", mname));
                    if let Ok(src) = fs::read_to_string(&macro_path) {
                        let mut mlexer = Lexer::new(&src);
                        let mtokens = mlexer.tokenize();
                        if let Ok((mreg, _mranges)) = extract_macro_defs(&mtokens) {
                            registry.merge(mreg);
                        }
                        if let Ok((treg, _tranges)) = extract_template_defs(&mtokens) {
                            template_registry.merge(treg);
                        }
                    }
                }
            }
            while i < tokens.len() && tokens[i] != Token::Newline {
                i += 1;
            }
        }
        i += 1;
    }

    // 从 token 流移除宏/模板定义占用 token
    let filtered: Vec<Token> = tokens
        .iter()
        .enumerate()
        .filter(|(idx, _)| {
            let mut skip = false;
            for chunk in macro_ranges.chunks(2) {
                if chunk.len() == 2 && *idx >= chunk[0] && *idx < chunk[1] {
                    skip = true;
                    break;
                }
            }
            !skip
        })
        .map(|(_, t)| t.clone())
        .collect();

    let expander = MacroExpander::new(registry);
    let template_expander = TemplateExpander::new(template_registry);

    // 宏↔模板混合嵌套交替展开（与 project.rs/main.rs 一致）
    let mut expanded = expander
        .expand(&filtered)
        .map_err(|e| format!("Expand error in {}: {}", path.display(), e))?;
    let max_passes = 16;
    let mut stable = false;
    for _pass in 0..max_passes {
        let before = expanded.clone();
        let after_tpl = template_expander
            .expand(&expanded)
            .map_err(|e| format!("Template error in {}: {}", path.display(), e))?;
        let after_mac = expander
            .expand(&after_tpl)
            .map_err(|e| format!("Expand error in {}: {}", path.display(), e))?;
        expanded = after_mac;
        if expanded == before && !contains_pending_call(&expanded) {
            stable = true;
            break;
        }
    }
    if !stable {
        return Err(format!(
            "Macro/template 交替展开未稳定 in {}（可能循环嵌套，超过 {} 轮）",
            path.display(),
            max_passes
        ));
    }

    let mut parser = Parser::new(expanded);
    parser
        .parse_module()
        .map_err(|e| format!("Parse error in {}: {}", path.display(), e))
}

/// 从源码 token 流提取跨模块宏导入文件路径（import macro X / from macro X import Y）
fn collect_macro_import_files(source: &str, path: &Path, out: &mut Vec<PathBuf>) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut i = 0usize;
    while i < tokens.len() {
        let is_import = tokens[i] == Token::Import || tokens[i] == Token::From;
        if is_import {
            let mut j = i + 1;
            let mut is_macro_import = false;
            let mut mod_name: Option<String> = None;
            while j < tokens.len() {
                match &tokens[j] {
                    Token::Macro => {
                        is_macro_import = true;
                        j += 1;
                    }
                    Token::Ident(n) if mod_name.is_none() => {
                        mod_name = Some(n.clone());
                        j += 1;
                    }
                    Token::Newline | Token::Dedent => break,
                    _ => {
                        if tokens[j] == Token::Import {
                            j += 1;
                        } else if tokens[j] == Token::As {
                            j += 2;
                        } else {
                            break;
                        }
                    }
                }
                if tokens[j] == Token::Newline {
                    break;
                }
            }
            if is_macro_import {
                if let Some(mname) = mod_name {
                    let macro_path = dir.join(format!("{}.lz", mname));
                    if macro_path.exists() {
                        out.push(macro_path);
                    }
                }
            }
            while i < tokens.len() && tokens[i] != Token::Newline {
                i += 1;
            }
        }
        i += 1;
    }
}

/// 从 AST 模块提取 import 依赖模块名（供依赖图展示/调试）
pub fn module_dep_names(module: &ast::Module) -> Vec<String> {
    module
        .imports
        .iter()
        .filter(|imp| imp.path.first().map(|s| s.as_str()) != Some("std"))
        .filter(|imp| !imp.path.first().map_or(false, |p| CRATE_IMPORTS.contains(&p.as_str())))
        .filter_map(|imp| imp.path.first().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incr_cache_key_stable() {
        let c = IncrCompiler::new(
            PathBuf::from("E:/proj"),
            PathBuf::from(".lzcache_incr"),
        );
        let m = IncrModule {
            path: PathBuf::from("E:/proj/sub/a.lz"),
            rel: "sub/a.lz".to_string(),
            deps: vec![],
            macro_files: vec![],
        };
        assert_eq!(c.cache_key(&m), "sub_a.lzcache");
        assert_eq!(c.code_path(&m), PathBuf::from(".lzcache_incr/sub_a_code.rs"));
    }
}
