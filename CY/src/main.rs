// lzcyc — LZ → Cython 子编译器
//
// 架构约束（长期项目决策）：
// - lzc 是主编译器（默认 Rust 后端），本子编译器不修改 src/ 下任何文件；
//   主编译器未来会融入子编译器，届时以本 crate 为主干。
// - lzcyc 复用主编译器 lib 公开层（L1 lexer → L2 parser/ast → L3 semantic_check
//   → L3.5 ir/builder → Cython 后端 codegen_cython），全部走 `lang_zone::` 库路径。
//
// 管线：
//   input.lz → lex → parse → semantic_check → build_ir → CythonCodeGen → output.pyx
//           → (可选) cythonize → .c → C 编译 → .pyd

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_help();
        std::process::exit(0);
    }
    match args[0].as_str() {
        "transpile" => cmd_transpile(&args[1..]),
        "compile" => cmd_compile(&args[1..]),
        "run" => cmd_run(&args[1..]),
        "--version" | "-V" => println!("lzcyc {VERSION}"),
        "--help" | "-h" => print_help(),
        other => {
            eprintln!("未知命令: {other}（查看帮助: lzcyc --help）");
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "lzcyc {VERSION} — LZ → Cython 编译器（lzc 的子编译器）\n\
         \n\
         用法:\n\
         \x20 lzcyc transpile <file.lz> [-o <dir>]   转译为 .pyx\n\
         \x20 lzcyc compile  <file.lz> [-o <dir>]    转译 + cythonize（→ .c/.pyd）\n\
         \x20 lzcyc run      <file.lz> [func]        编译并运行（默认入口 main）\n\
         \x20 lzcyc --version                        版本号\n\
         \x20 lzcyc --help                           本帮助\n\
         \n\
         说明:\n\
         \x20 前端（lexer/parser/semantic_check/IR）与 lzc 共享同一实现，\n\
         \x20 后端输出 Cython (.pyx)，可编译为 Python C 扩展 (.pyd)。"
    );
}

// ─────────────────────────── 核心管线 ───────────────────────────

/// 读取 --macro-check=loose|light|strict（默认 light，对齐主编译器）
fn macro_check_mode() -> lang_zone::macros::expand::CheckMode {
    match std::env::args().find_map(|a| a.strip_prefix("--macro-check=").map(String::from)) {
        Some(s) if s == "loose" => lang_zone::macros::expand::CheckMode::Loose,
        Some(s) if s == "strict" => lang_zone::macros::expand::CheckMode::Strict,
        _ => lang_zone::macros::expand::CheckMode::Light,
    }
}

/// 宏/模板展开（对齐主编译器管线 08 §3.6）：
/// 提取宏/模板定义 → 跨模块宏导入（macro import）→ 过滤定义占用 token
/// → 首遍宏展开 → 宏↔模板交替展开至稳定（上限 16 轮）。
/// 全部复用 lib 公开层 `lang_zone::macros::*`，编排逻辑为本 crate 自有。
fn expand_macro_pipeline(
    path: &Path,
    tokens: &[lang_zone::lexer::Token],
) -> Result<Vec<lang_zone::lexer::Token>, String> {
    use lang_zone::lexer::Token;
    use lang_zone::macros::{
        contains_pending_call, extract_macro_defs, extract_template_defs, load_macro_imports,
        MacroExpander, TemplateExpander,
    };
    use lang_zone::util::import::ImportResolver;

    let mode = macro_check_mode();

    // 1) 提取本文件宏/模板定义
    let (mut registry, mut ranges) =
        extract_macro_defs(tokens).map_err(|e| format!("Macro definition error: {e}"))?;
    let (mut template_registry, tpl_ranges) =
        extract_template_defs(tokens).map_err(|e| format!("Template definition error: {e}"))?;
    ranges.extend(tpl_ranges);

    // 2) 跨模块宏导入（macro import X → 递归加载并合并宏/模板定义）
    let base_dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    let mut resolver = ImportResolver::new();
    let (imp_macros, imp_templates, imp_ranges) =
        load_macro_imports(tokens, &base_dir, &mut resolver)
            .map_err(|e| format!("Macro import error: {e}"))?;
    registry.merge(imp_macros);
    template_registry.merge(imp_templates);
    ranges.extend(imp_ranges);

    // 3) 过滤宏/模板定义占用的 token
    let expander_input: Vec<Token> = tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            !ranges
                .chunks(2)
                .any(|c| c.len() == 2 && *i >= c[0] && *i < c[1])
        })
        .map(|(_, t)| t.clone())
        .collect();

    // 4) 首遍宏展开 + 宏↔模板交替至稳定
    let mut expander = MacroExpander::new(registry);
    expander.set_check_mode(mode);
    let mut template_expander = TemplateExpander::new(template_registry);
    template_expander.set_check_mode(mode);

    let mut expanded = expander
        .expand(&expander_input)
        .map_err(|e| format!("Macro expansion error: {e}"))?;
    let max_passes = 16;
    let mut stable = false;
    for _pass in 0..max_passes {
        let before = expanded.clone();
        let after_tpl = template_expander
            .expand(&expanded)
            .map_err(|e| format!("Template expansion error: {e}"))?;
        let after_mac = expander
            .expand(&after_tpl)
            .map_err(|e| format!("Macro expansion error: {e}"))?;
        expanded = after_mac;
        if expanded == before && !contains_pending_call(&expanded) {
            stable = true;
            break;
        }
    }
    if !stable {
        return Err(
            "宏/template 交替展开未稳定（可能循环嵌套，超过 16 轮）".to_string(),
        );
    }
    Ok(expanded)
}

/// 读取 --std-dir <path>（可多次出现，累积）
fn collect_std_dirs() -> Vec<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--std-dir" && i + 1 < args.len() {
            out.push(std::path::PathBuf::from(&args[i + 1]));
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

/// LZ 源文件 → .pyx 文本
///
/// 复用主编译器 lib 公开层，管线与 lzc 对齐：
/// lex → 宏/模板展开 → parse → import 合并 → semantic_check（G2）
/// → build_ir → Cython 后端 → 生成后处理。
fn transpile_source(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Error reading {}: {}", path.display(), e))?;

    // L1 词法
    let tokens = lang_zone::lexer::Lexer::new(&source).tokenize();

    // 宏/模板展开（lexer 之后、parser 之前，对齐主编译器位置）
    let expanded_tokens = expand_macro_pipeline(path, &tokens)?;

    // L2 语法
    let mut parser = lang_zone::parser::Parser::new(expanded_tokens);
    // 宏模块检测：用原始 token 流（展开前）检测 #!bin macro 声明
    parser.is_macro = lang_zone::macros::has_bin_macro_declaration(&tokens);
    let mut module = parser
        .parse_module()
        .map_err(|e| format!("Parse error: {e}"))?;
    // 模块级魔法属性与 comptime 源码数据源（对齐主编译器编译入口行为）
    module.file_path = Some(path.to_string_lossy().to_string());
    module.source_text = Some(source);

    // import 合并（lzcyc 侧增强：源目录 + --std-dir 候选）
    let std_dirs = collect_std_dirs();
    let merged_modules = merge_imports(&mut module, path, &std_dirs)?;

    // L3 语义检查（G2，与主编译器 build_ir_opt 前置检查一致）
    let errs = lang_zone::semantic_check::check_module(&module);
    if !errs.is_empty() {
        return Err(format!("Semantic error:\n{}", errs.join("\n")));
    }

    // L3.5 IR 构建
    let ir =
        lang_zone::ir::builder::build_ir(&module).map_err(|e| format!("IR build error: {e}"))?;

    // Cython 后端
    let mut cg = lang_zone::ir::codegen_cython::CythonCodeGen::new();
    let raw = cg.generate(&ir).to_string();

    // 生成后处理（运行期语义缺口兜底，见 postprocess_pyx 注释）
    let enum_variants = collect_enum_variants(&module);
    Ok(postprocess_pyx(&raw, &enum_variants, &merged_modules))
}

/// 解析 `-o/--output <dir>` 选项，返回 (剩余位置参数, 输出目录)
fn extract_out_dir(args: &[String]) -> (Vec<String>, Option<String>) {
    let mut rest = Vec::new();
    let mut out_dir = None;
    let mut i = 0;
    while i < args.len() {
        if (args[i] == "-o" || args[i] == "--output") && i + 1 < args.len() {
            out_dir = Some(args[i + 1].clone());
            i += 2;
        } else {
            rest.push(args[i].clone());
            i += 1;
        }
    }
    (rest, out_dir)
}

/// 计算输出 .pyx 路径：默认输入同目录同名；指定 -o 则落到该目录
fn pyx_output_path(input: &Path, out_dir: Option<&str>) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    match out_dir {
        Some(dir) => PathBuf::from(dir).join(format!("{stem}.pyx")),
        None => input.with_file_name(format!("{stem}.pyx")),
    }
}

// ─────────────────────────── 后处理与 import 合并 ───────────────────────────

/// 从 AST 收集 enum 变体映射（变体类名 → 声明序号）。
/// LZ 无数据枚举在 AST 中为 `StructDef { is_enum: true, fields: [变体...] }`。
fn collect_enum_variants(module: &lang_zone::ast::Module) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for s in &module.structs {
        if s.is_enum {
            for (i, f) in s.fields.iter().enumerate() {
                out.push((f.name.clone(), i));
            }
        }
    }
    out
}

/// 生成后处理：修复主编译器 Cython 后端的运行期语义缺口（挂账项，融入前由 lzcyc 兜底）。
/// 各项均以生成代码的稳定形态为锚点，匹配不上则原样保留：
/// 1. enum 无数据变体注入 `_variant = <序号>`（match 解构依赖）
/// 2. Box/Rc/Arc 补 `__getitem__`/`__setitem__`（`x[0]` 取/存内部值）
/// 3. Option 垫片：`None_()` 返回带方法的 `_LZNONE` 单例；行级 `x = None` → `x = _LZNONE`
///    （LZ `is None` 生成 `.is_none()` 调用，裸 None 无方法）
/// 4. 构建块下标：`(lambda : (...)))()(N)` → `...))()[N]`（lib 把下标生成了调用）
/// 5. 列表推导 filter 谓词补调用：`for __cv in ... if (lambda ...)` → `...(__cv)`
/// 6. 已合并 import 的限定前缀剥离：`lz_std.X` → `X`，并删除对应 `import X` 行
/// 7. checker 派发：`fn[checker](...)` 调用改写为包装器；注入 `__Params` 垫片
///    （规范接口：ps.kwargs.contains(k) / ps.kwargs[k]，kwargs 为形参名到实参映射）
fn postprocess_pyx(
    code: &str,
    enum_variants: &[(String, usize)],
    merged_modules: &[String],
) -> String {
    let mut lines: Vec<String> = code.lines().map(String::from).collect();

    // 1) _variant 注入：`class Red(Color):` 的下一行 `pass` → `_variant = N`
    for i in 0..lines.len() {
        let t = lines[i].trim_start().to_string();
        if let Some(rest) = t.strip_prefix("class ") {
            if let Some(paren) = rest.find('(') {
                let cname = rest[..paren].trim();
                if let Some((_, idx)) = enum_variants.iter().find(|(n, _)| n == cname) {
                    if i + 1 < lines.len() && lines[i + 1].trim() == "pass" {
                        let indent = lines[i + 1].len() - lines[i + 1].trim_start().len();
                        lines[i + 1] =
                            format!("{}{}", " ".repeat(indent), format!("_variant = {idx}"));
                    }
                }
            }
        }
    }
    let mut code = lines.join("\n");

    // 2) Box/Rc/Arc 下标增强（prelude 固定形态为锚点）
    for cls in ["Box", "Rc", "Arc"] {
        let anchor = format!(
            "class {cls}:\n    def __init__(self, v=None): self._v = v\n    @staticmethod\n    def new(v=None): return {cls}(v)\n    def __getattr__(self, n): return getattr(self._v, n)"
        );
        let enhanced = format!(
            "{anchor}\n    def __getitem__(self, i): return self._v\n    def __setitem__(self, i, v): self._v = v"
        );
        if code.contains(&anchor) {
            code = code.replace(&anchor, &enhanced);
        }
    }

    // 3) Option/None 方法族：已下沉 lib（is_none→is None、is_some→is not None、
    //    unwrap/expect 透传 + __lz_expect_fail），垫片方案退位删除（M1.3）

    // 4) 构建块下标修复：`))()(N)` → `))()[N]`（逐个消耗，防死循环）
    loop {
        let Some(p) = code.find("))()(") else { break };
        let after = &code[p + 5..];
        let Some(close) = after.find(')') else { break };
        let idx = &after[..close];
        if idx.is_empty() || !idx.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        code = format!("{}))()[{}]{}", &code[..p], idx, &after[close + 1..]);
    }

    // 5) 列表推导 filter 谓词补调用：已下沉 lib（filter 为 lambda 时直出 `(__cv)` 调用），
    //    postprocess 分支退位删除（M1.5——叠加生成会导致 `(__cv)(__cv)` 双调用）

    // 6) 已合并 import：删除 `import X` 行 + 剥离 `X.` 限定前缀
    for m in merged_modules {
        code = code.replace(&format!("{m}."), "");
        let mut out = Vec::with_capacity(code.lines().count());
        let import_stmt = format!("import {m}");
        for l in code.lines() {
            if l.trim() == import_stmt {
                continue;
            }
            out.push(l.to_string());
        }
        code = out.join("\n");
    }

    // 7) checker 派发：`fn[checker](...)` → 包装器调用（checker 先行检查，再透传实参）
    {
        use std::collections::BTreeSet;
        let mut wrappers: Vec<String> = Vec::new();
        let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
        // 7a) 收集调用点对并替换：`fname[checker](` → `__lz_checked_fname_checker(`
        let mut out = Vec::with_capacity(code.lines().count());
        for l in code.lines() {
            let mut fixed = l.to_string();
            if !fixed.trim_start().starts_with('#') && fixed.contains('[') {
                // 逐个识别 `标识符[标识符](` 形态
                let chars: Vec<char> = fixed.chars().collect();
                let mut i = 0;
                let mut rebuilt = String::new();
                while i < chars.len() {
                    if chars[i] == '[' {
                        // 向前找标识符
                        let head_end = i;
                        let mut fs = i;
                        while fs > 0 {
                            let c = chars[fs - 1];
                            if c.is_alphanumeric() || c == '_' {
                                fs -= 1;
                            } else {
                                break;
                            }
                        }
                        // 向后找 `](` 形态与 checker 标识符
                        let mut j = i + 1;
                        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                            j += 1;
                        }
                        if j < chars.len()
                            && chars[j] == ']'
                            && j + 1 < chars.len()
                            && chars[j + 1] == '('
                            && head_end > fs
                        {
                            let fname: String = chars[fs..head_end].iter().collect();
                            let checker: String = chars[i + 1..j].iter().collect();
                            let valid = |s: &str| {
                                !s.is_empty()
                                    && s.chars().next().map(|c| c.is_alphabetic() || c == '_')
                                        .unwrap_or(false)
                            };
                            if valid(&fname)
                                && valid(&checker)
                                && code.contains(&format!("def {checker}("))
                                && code.contains(&format!("def {fname}("))
                            {
                                let wrapper = format!("__lz_checked_{fname}_{checker}");
                                // 回退已逐字符写入 rebuilt 的 fname 字符
                                let fname_len = head_end - fs;
                                rebuilt.truncate(rebuilt.len() - fname_len);
                                rebuilt.push_str(&wrapper);
                                pairs.insert((fname.clone(), checker.clone()));
                                i = j + 1; // 跳过 `](` 中的 `]`，保留 `(` 在下一轮原样输出
                                continue;
                            }
                        }
                    }
                    rebuilt.push(chars[i]);
                    i += 1;
                }
                fixed = rebuilt;
            }
            out.push(fixed);
        }
        code = out.join("\n");

        // 7b) 生成包装器（形参名提取自 def 行，位置实参 zip 进 kwargs）
        for (fname, checker) in &pairs {
            let params = extract_def_params(&code, fname);
            let zip_list = if params.is_empty() {
                "()".to_string()
            } else {
                let names = params
                    .iter()
                    .map(|p| format!("'{p}'"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({names},)")
            };
            wrappers.push(format!(
                "def __lz_checked_{fname}_{checker}(*__args, **__kwargs):\n    __params = __Params(kwargs=dict(list(zip({zip_list}, __args)), **__kwargs))\n    {checker}(__params)\n    return {fname}(*__args, **__kwargs)"
            ));
        }
        if !wrappers.is_empty() {
            // __Params 垫片 + 包装器统一插在 def main 之前
            let shim = "__lz_params_shim__START\nclass _LzKwargsMap:\n    def __init__(self, d): self._d = dict(d)\n    def contains(self, k): return k in self._d\n    def __getitem__(self, k): return self._d[k]\n\nclass __Params:\n    def __init__(self, kwargs=None, args=None): self.kwargs = _LzKwargsMap(kwargs)\n__lz_params_shim__END";
            let block = format!(
                "{}\n{}",
                shim.replace("__lz_params_shim__START", "").replace("__lz_params_shim__END", ""),
                wrappers.join("\n\n")
            );
            let anchor = "\ndef main(";
            match code.find(anchor) {
                Some(p) => {
                    code = format!("{}\n\n{}\n{}", &code[..p], block, &code[p + 1..]);
                }
                None => code = format!("{code}\n\n{block}\n"),
            }
        }
    }

    code
}

/// 从生成代码中提取 `def fname(...)` 的形参名列表（剥 C 风格类型，跳过 self/ps）
fn extract_def_params(code: &str, fname: &str) -> Vec<String> {
    let mut out = Vec::new();
    for l in code.lines() {
        let t = l.trim_start();
        let head = format!("def {fname}(");
        if t.starts_with(&head) {
            if let Some(open) = t.find('(') {
                if let Some(close) = t.rfind(')') {
                    if close > open {
                        for seg in t[open + 1..close].split(',') {
                            let seg_t = seg.trim();
                            if seg_t.is_empty() || seg_t.starts_with('*') {
                                continue;
                            }
                            let head_t = seg_t.split('=').next().unwrap_or(seg_t).trim();
                            if head_t.contains(':') {
                                continue; // Python 标注形态：x: int → 取 x？此处保守跳过
                            }
                            if let Some(last) = head_t.split_whitespace().last() {
                                if last == "self" || last == "ps" {
                                    continue;
                                }
                                out.push(last.to_string());
                            }
                        }
                    }
                }
            }
            break;
        }
    }
    out
}

/// 简版 import 合并（对齐主编译器 merge_imports_into 语义）：
/// 对每个 `import X` 在源目录与 std 目录中查找 `X.lz` / `X/mod.lz`，
/// 找到则 parse 并将顶层项合并进主模块；import 语句保留（semantic_check
/// 需要模块名绑定，文件在同目录/std 目录可寻即放行）。
/// 返回已成功合并的模块名列表（供生成后处理剥离限定前缀）。
fn merge_imports(
    module: &mut lang_zone::ast::Module,
    entry: &Path,
    std_dirs: &[PathBuf],
) -> Result<Vec<String>, String> {
    let dir = entry.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut loaded: Vec<PathBuf> = Vec::new();
    let mut merged = Vec::new();
    merge_imports_into(module, &dir, std_dirs, &mut loaded, &mut merged)?;
    Ok(merged)
}

fn merge_imports_into(
    module: &mut lang_zone::ast::Module,
    dir: &Path,
    std_dirs: &[PathBuf],
    loaded: &mut Vec<PathBuf>,
    merged: &mut Vec<String>,
) -> Result<(), String> {
    const SKIP: &[&str] = &[
        "std", "macro", "serde", "tokio", "regex", "chrono", "rand", "itertools", "serde_json",
        "once_cell",
    ];
    let imports = module.imports.clone();
    for imp in &imports {
        if imp.path.is_empty() {
            continue;
        }
        let first = imp.path[0].as_str();
        if SKIP.contains(&first) {
            continue;
        }
        let rel = imp.path.join("/");
        let mut candidates = vec![dir.join(&rel).with_extension("lz"), dir.join(&rel).join("mod.lz")];
        for sd in std_dirs {
            candidates.push(sd.join(&rel).with_extension("lz"));
            candidates.push(sd.join(&rel).join("mod.lz"));
        }
        let Some(mp) = candidates.into_iter().find(|c| c.exists()) else {
            continue;
        };
        if loaded.contains(&mp) {
            continue;
        }
        loaded.push(mp.clone());
        let Ok(src) = std::fs::read_to_string(&mp) else {
            continue;
        };
        let tokens = lang_zone::lexer::Lexer::new(&src).tokenize();
        let mut parser = lang_zone::parser::Parser::new(tokens);
        let Ok(mut sub) = parser.parse_module() else {
            continue;
        };
        // 递归处理子模块的 import
        let sub_dir = mp.parent().unwrap_or(dir).to_path_buf();
        merge_imports_into(&mut sub, &sub_dir, std_dirs, loaded, merged)?;
        // 注入定义项（对齐主编译器合并字段）
        module.functions.extend(sub.functions);
        module.structs.extend(sub.structs);
        module.traits.extend(sub.traits);
        module.impls.extend(sub.impls);
        module.type_aliases.extend(sub.type_aliases);
        module.duck_defs.extend(sub.duck_defs);
        module.magic_blocks.extend(sub.magic_blocks);
        module.consts.extend(sub.consts);
        // 顶层 let → Const
        for s in &sub.top_stmts {
            if let lang_zone::ast::Stmt::Let {
                name,
                mutable,
                ty,
                value,
                mods,
                ..
            } = s
            {
                module.consts.push(lang_zone::ast::ConstDef {
                    name: name.clone(),
                    ty: ty.clone(),
                    value: value.clone(),
                    mutable: *mutable,
                    mods: mods.clone(),
                });
            }
        }
        // 记录已合并模块名（后处理剥限定前缀用）
        if !merged.iter().any(|m| m == &first) {
            merged.push(first.to_string());
        }
    }
    Ok(())
}

/// 剥离函数参数列表中的 C 风格类型标注：`double x, str y = "a"` → `x, y = "a"`。
/// Python 风格标注 `x: int` 与 `*args/**kwargs` 原样保留。
/// 额外处理：显式收集参数与变参重名（`args, *args` → `*args`，调用点散传语义一致）。
fn strip_param_types(params: &str) -> String {
    let stripped: Vec<String> = params
        .split(',')
        .map(|seg| {
            let seg_t = seg.trim();
            if seg_t.is_empty() || seg_t.starts_with('*') {
                return seg_t.to_string();
            }
            let (head, tail) = match seg_t.find('=') {
                Some(eq) => (&seg_t[..eq], &seg_t[eq..]),
                None => (seg_t, ""),
            };
            let head_t = head.trim();
            if head_t.contains(':') {
                // Python 类型标注，合法，保留
                format!("{head_t}{tail}")
            } else {
                match head_t.split_whitespace().last() {
                    Some(last) if head_t.contains(' ') => format!("{last}{tail}"),
                    _ => format!("{head_t}{tail}"),
                }
            }
        })
        .collect();
    // 重名消解：存在 `*args` 时删除同名显式参数（变参降级语义等价）
    let star_name = stripped.iter().find_map(|p| {
        p.trim().strip_prefix('*').map(|n| n.trim().to_string())
    });
    let out: Vec<String> = match star_name {
        Some(star) => stripped
            .into_iter()
            .filter(|p| {
                let t = p.trim();
                t.starts_with('*') || t.split('=').next().unwrap_or(t).trim() != star
            })
            .collect(),
        None => stripped,
    };
    out.join(", ")
}

/// 剥离 Cython 特有语法，使产物可作为纯 Python 运行（项目验证约定）。
/// 规则（行级保守剥离）：
/// - `cdef class X` → `class X`
/// - 函数行（def/cpdef/cdef）：剥 C 风格参数类型、`-> <ret>` 返回标注、cpdef/cdef 前缀
/// - 字段声明 `cdef ...`（无括号）→ 注释（__init__ 动态属性赋值仍生效）
/// - `cimport ...` / `from cpython ...` / `@cython.*` 装饰器 → 注释
fn strip_cython_syntax(code: &str) -> String {
    code.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let indent: String = line.chars().take(line.len() - trimmed.len()).collect();
            let is_fn_line = trimmed.starts_with("def ")
                || trimmed.starts_with("cpdef ")
                || trimmed.starts_with("cdef ");
            if trimmed.starts_with("cdef class ") {
                format!("{indent}{}", trimmed.replacen("cdef ", "", 1))
            } else if is_fn_line && trimmed.contains('(') {
                // 函数定义行：统一剥前缀/返回标注/参数 C 类型
                let body = trimmed
                    .trim_start_matches("cpdef ")
                    .trim_start_matches("cdef ");
                let name_part = match (body.find('('), body.rfind(')')) {
                    (Some(paren), Some(close)) if close > paren => {
                        let fname = body[..paren]
                            .split_whitespace()
                            .last()
                            .unwrap_or("");
                        let params = strip_param_types(&body[paren + 1..close]);
                        // 剥 `-> <ret>` 返回标注，但保留冒号后的单行函数体（如 `...`）
                        let suffix = &body[close + 1..];
                        let suffix = match suffix.find(" -> ") {
                            Some(arrow) => match suffix[arrow..].find(':') {
                                Some(colon) => format!(":{}", &suffix[arrow + colon + 1..]),
                                None => suffix[..arrow].to_string() + ":",
                            },
                            None => suffix.to_string(),
                        };
                        format!("def {fname}({params}){suffix}")
                    }
                    _ => format!("# [pyrun] {trimmed}"),
                };
                format!("{indent}{name_part}")
            } else if trimmed.starts_with("cdef ")
                || trimmed.starts_with("ctypedef ")
                || trimmed.starts_with("cimport ")
                || trimmed.starts_with("from cpython ")
                || trimmed.starts_with("@cython.")
            {
                format!("{indent}# [pyrun] {trimmed}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ─────────────────────────── 子命令 ───────────────────────────

fn cmd_transpile(args: &[String]) {
    let (rest, out_dir) = extract_out_dir(args);
    let Some(input) = rest.first() else {
        eprintln!("用法: lzcyc transpile <file.lz> [-o <dir>]");
        std::process::exit(1);
    };
    let input = Path::new(input);
    let pyx = match transpile_source(input) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let out_path = pyx_output_path(input, out_dir.as_deref());
    if let Some(dir) = out_path.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = fs::create_dir_all(dir);
        }
    }
    match fs::write(&out_path, &pyx) {
        Ok(_) => println!("transpile: {} → {}", input.display(), out_path.display()),
        Err(e) => {
            eprintln!("Error writing {}: {}", out_path.display(), e);
            std::process::exit(1);
        }
    }
}

fn cmd_compile(args: &[String]) {
    let (rest, out_dir) = extract_out_dir(args);
    let Some(input) = rest.first() else {
        eprintln!("用法: lzcyc compile <file.lz> [-o <dir>]");
        std::process::exit(1);
    };
    let input = Path::new(input);

    // 1) transpile
    let pyx = match transpile_source(input) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let out_path = pyx_output_path(input, out_dir.as_deref());
    if let Some(dir) = out_path.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = fs::create_dir_all(dir);
        }
    }
    if let Err(e) = fs::write(&out_path, &pyx) {
        eprintln!("Error writing {}: {}", out_path.display(), e);
        std::process::exit(1);
    }
    println!("transpile: {} → {}", input.display(), out_path.display());

    // 2) cythonize（调 CY/scripts/cython_build.py；.c → .pyd 需要本机 C 编译器）
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/cython_build.py");
    let status = Command::new("python")
        .arg(&script)
        .arg(&out_path)
        .arg(out_path.parent().unwrap_or(Path::new(".")))
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("compile: {} cythonize OK", out_path.display());
            println!("提示: 生成 .pyd 需要本机 C 编译器（GCC/MSVC）；");
            println!("  也可用 pyximport 直接导入 .pyx 运行。");
        }
        Ok(s) => {
            eprintln!("cythonize 失败（退出码 {:?}）", s.code());
            eprintln!("请确认 python 环境已安装 cython: pip install cython");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("无法启动 python: {e}");
            eprintln!("compile 命令需要 python + cython 环境（pip install cython）；");
            eprintln!("仅转译可使用: lzcyc transpile <file.lz>");
            std::process::exit(1);
        }
    }
}

fn cmd_run(args: &[String]) {
    let (rest, out_dir) = extract_out_dir(args);
    let Some(input) = rest.first() else {
        eprintln!("用法: lzcyc run <file.lz> [func]");
        std::process::exit(1);
    };
    let func = rest.get(1).cloned().unwrap_or_else(|| "main".to_string());
    let input = Path::new(input);
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    // run 的 .pyx 产物固定落 output/pyrun（避免污染 TESTS 参考目录）；
    // 显式 -o 时尊重用户指定
    let pyx_path = match out_dir.as_deref() {
        Some(dir) => pyx_output_path(input, Some(dir)),
        None => {
            let pyrun_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output/pyrun");
            let stem = input
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".to_string());
            pyrun_dir.join(format!("{stem}.pyx"))
        }
    };
    let module_dir = pyx_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // 1) transpile（run 前置：确保 .pyx 为最新）
    let pyx = match transpile_source(input) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    if let Some(dir) = pyx_path.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = fs::create_dir_all(dir);
        }
    }
    if let Err(e) = fs::write(&pyx_path, &pyx) {
        eprintln!("Error writing {}: {}", pyx_path.display(), e);
        std::process::exit(1);
    }

    // 2) 探测 cython 可用性，选择运行路径
    let has_cython = Command::new("python")
        .args(["-c", "import cython"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let (code, note): (String, &str) = if has_cython {
        // Cython 可用：pyximport 即时编译导入（本机有 C 编译器时生成 .pyd 缓存）
        (
            format!(
                "import sys; sys.path.insert(0, r'{}'); \
                 import pyximport; pyximport.install(); \
                 import {stem}; \
                 getattr({stem}, '{func}')()",
                module_dir.display()
            ),
            "pyximport",
        )
    } else {
        // 纯 Python 降级运行（验证约定）：剥离 Cython 特有语法后作为 .py 模块运行。
        // struct/enum 产物含 cdef class/cpdef，经 strip_cython_syntax 保守剥离；
        // 对 `import cython` 行注入空模块 stub 以脱离 cython 包依赖。
        let pyrun_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output/pyrun");
        let _ = fs::create_dir_all(&pyrun_dir);
        let py_path = pyrun_dir.join(format!("{stem}.py"));
        let py_code = strip_cython_syntax(&pyx);
        if let Err(e) = fs::write(&py_path, &py_code) {
            eprintln!("Error writing {}: {}", py_path.display(), e);
            std::process::exit(1);
        }
        (
            format!(
                "import sys, types\n\
                 try:\n    import cython\n\
                 except ImportError:\n    sys.modules['cython'] = types.ModuleType('cython')\n\
                 sys.path.insert(0, r'{}')\n\
                 import {stem}\n\
                 getattr({stem}, '{func}')()",
                pyrun_dir.display()
            ),
            "py 降级",
        )
    };

    match Command::new("python").arg("-c").arg(&code).status() {
        Ok(s) if s.success() => println!("run: {stem}.{func}() OK（{note}）"),
        Ok(s) => {
            eprintln!("run 失败（退出码 {:?}）", s.code());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("无法启动 python: {e}");
            eprintln!("run 命令需要 python 环境（建议同时安装 cython: pip install cython）");
            std::process::exit(1);
        }
    }
}
