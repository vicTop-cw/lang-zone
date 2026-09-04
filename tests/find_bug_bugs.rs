// FIND_BUG 36 编号回归守护套件：lz → (rs) → rustc → run 全链路
// 约定（对齐 tests/find_bug_libs.rs）：
//   - ✅ 无 bug 用例：直接转正（任一回归即红）
//   - ❌ 确认 bug 用例：#[ignore] 分级挂起，ignore reason 记录卡点阶段，
//     修复后移除 #[ignore] 转正（任一用例被放行即红的负向守护用 *Negative 命名）
//   - 判定证据：FIND_BUG.md「实测记录（2026-09-03）」章节
// 全量基线：39 非库用例 = 12 无bug / 21 确认bug / 1 部分问题 / 2 待单测（已补测全绿）

use std::path::PathBuf;
use std::process::Command;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn builtins_rlib() -> PathBuf {
    let dir = manifest().join("target/debug");
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        let mut cands: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let n = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                n.starts_with("liblz_builtins-") && n.ends_with(".rlib")
            })
            .collect();
        cands.sort();
        if let Some(p) = cands.pop() {
            return p;
        }
    }
    panic!("lz_builtins rlib not found under {}", dir.display());
}

/// 用例 .lz 相对路径（如 "FIND_BUG/lexer/bug-escape-unicode.lz"）
fn case_lz(rel: &str) -> (PathBuf, PathBuf) {
    let lz = manifest().join(rel);
    assert!(lz.exists(), "用例不存在: {}", lz.display());
    let dir = lz.parent().unwrap().to_path_buf();
    (lz, dir)
}

enum Stage {
    /// lzc 应以非零退出（正确拒绝）
    LzReject,
    /// lz 编译 + rustc + 运行全链路通过，stdout 含 expected
    FullRun(&'static str),
}

fn run_case(rel: &str, stage: &Stage) -> Result<(), String> {
    let (lz, dir) = case_lz(rel);
    let stem = lz.file_stem().unwrap().to_string_lossy().to_string();
    let rs = lz.with_extension("rs");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output()
        .map_err(|e| format!("lz compile err: {}", e))?;

    match stage {
        Stage::LzReject => {
            if out.status.success() {
                return Err(format!("负向用例被放行：{} 应被 lzc 拒绝", lz.display()));
            }
            Ok(())
        }
        Stage::FullRun(expected) => {
            if !out.status.success() {
                return Err(format!(
                    "LZ_FAIL {}: {}",
                    lz.display(),
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            if !rs.exists() {
                return Err(format!("no .rs generated: {}", rs.display()));
            }
            let debug = dir.join("debug");
            let _ = std::fs::create_dir_all(&debug);
            let exe = debug.join(format!("{}_bugs.exe", stem));
            let rc = Command::new("rustc")
                .args(["--edition", "2021"])
                .arg(&rs)
                .arg("--extern")
                .arg(format!("lz_builtins={}", builtins_rlib().display()))
                .arg("-A").arg("warnings")
                .arg("-o").arg(&exe)
                .output()
                .map_err(|e| format!("rustc err: {}", e))?;
            if !rc.status.success() {
                return Err(format!(
                    "RUSTC_FAIL {}: {}",
                    rs.display(),
                    String::from_utf8_lossy(&rc.stderr)
                ));
            }
            let run = Command::new(&exe).output()
                .map_err(|e| format!("run err: {}", e))?;
            if !run.status.success() {
                return Err(format!(
                    "RUN_FAIL (exit {:?}): {}",
                    run.status.code(),
                    String::from_utf8_lossy(&run.stderr)
                ));
            }
            let stdout = String::from_utf8_lossy(&run.stdout).to_string();
            if !stdout.contains(expected) {
                return Err(format!(
                    "ASSERT_FAIL: stdout 应含 {:?}，实际：{:?}",
                    expected, stdout
                ));
            }
            Ok(())
        }
    }
}

fn full(rel: &str, expect: &'static str) -> Result<(), String> {
    run_case(rel, &Stage::FullRun(expect))
}

fn reject(rel: &str) -> Result<(), String> {
    run_case(rel, &Stage::LzReject)
}

// ══════════════════════════════════════════════════════════════
// ✅ 无 bug 用例（已转正，回归即红）
// ══════════════════════════════════════════════════════════════

// BUG-LX-001: emoji + \u{1F600} 正常；\u{} 空转义正确拒绝（两形态都锁）
#[test]
fn lx001_unicode_escape_ok() {
    full("FIND_BUG/lexer/bug-escape-unicode.lz", "bug-escape-unicode.lz done").unwrap();
}

#[test]
fn lx001_empty_unicode_escape_rejected_negative() {
    // \u{} 空转义应拒绝（探针 p14 复验）；直接内联最小源码验证
    let dir = manifest().join("target/tmp_negative_lx001");
    let _ = std::fs::create_dir_all(&dir);
    let lz = dir.join("u_empty.lz");
    std::fs::write(&lz, "def main() =\n  s = \"a\\u{}b\"\n  print(s)\n").unwrap();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().unwrap();
    assert!(!out.status.success(), "\\u{{}} 空转义应被 lzc 拒绝");
}

// BUG-LX-003: ~: 行尾/参数位留白违规正确拒绝
#[test]
fn lx003_tilde_colon_rejected_negative() {
    reject("FIND_BUG/lexer/bug-tilde-colon-eof.lz").unwrap();
}

// BUG-LX-004: 多行字符串缩进语义
#[test]
fn lx004_multiline_indent_ok() {
    full("FIND_BUG/lexer/bug-multiline-indent.lz", "bug-multiline").unwrap();
}

// BUG-PR-003: .. 与 / 混用互斥（探针 p28 锁定拒绝）；用例文件内的 `..: nums: int`
// 按 03d-可变参数.md 规范非法（具名收集应写 `nums: List<T>`），lzc 拒绝方向正确。
// 该用例文件整体为负向（正例 varargs_ok 用了非法形态），故 reject() 守护。
#[test]
fn pr003_varargs_mixed_rejected_negative() {
    reject("FIND_BUG/parser/bug-varargs-slash.lz").unwrap();
}

// BUG-PR-004: 正向部分（type IntPair = (int, int) 可用）；
// 负向部分（type X = __add__ 拒绝）已由探针 p2 锁定，这里跑正向全链路
#[test]
fn pr004_typealias_magic_ok() {
    full("FIND_BUG/parser/bug-typealias-magic.lz", "typealias-magic.lz done").unwrap();
}

#[test]
fn pr004_typealias_magic_rejected_negative() {
    // 探针 p2 复验：type MyAdder = __add__ → Parse error: Expected type, got MagicMethod
    let dir = manifest().join("target/tmp_negative_pr004");
    let _ = std::fs::create_dir_all(&dir);
    let lz = dir.join("ta_magic.lz");
    std::fs::write(&lz, "type MyAdder = __add__\ndef main() =\n  print(\"no\")\n").unwrap();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().unwrap();
    assert!(!out.status.success(), "type = 魔法方法 应被 lzc 拒绝");
}

// BUG-IR-005: comptime: 块解析 + const 提升（Rust 编译期折叠）
#[test]
fn ir005_comptime_block_ok() {
    full("FIND_BUG/ir/bug-ir-comptime.lz", "comptime block parsed successfully").unwrap();
}

// BUG-CG-001: ..: int 变参全链路
#[test]
fn cg001_varargs_full_ok() {
    full("FIND_BUG/codegen/bug-codegen-varargs.lz", "bug-codegen-varargs.lz done").unwrap();
}

// BUG-CG-003: #!export 编译运行
#[test]
fn cg003_export_ok() {
    full("FIND_BUG/codegen/bug-codegen-export.lz", "bug-codegen-export.lz done").unwrap();
}

// BUG-SB-004: kebab-case 透传
#[test]
fn sb004_kebab_ok() {
    full("FIND_BUG/stdbridge/bug-stdbridge-kebab.lz", "my-lib-utils::helper::do_work").unwrap();
}

// BUG-SG-004: =: 块返回值（函数内）
#[test]
fn sg004_build_block_return_ok() {
    full("FIND_BUG/syntax/bug-syntax-build-return.lz", "30").unwrap();
}

// BUG-EC-003/004/007: 空 Dict / 1e308 / _ 变量
#[test]
fn ec003_empty_dict_ok() {
    full("FIND_BUG/edge/bug-edge-empty-dict.lz", "empty-dict.lz done").unwrap();
}

#[test]
fn ec004_float_precision_ok() {
    full("FIND_BUG/edge/bug-edge-float-scientific.lz", "1e308").unwrap();
}

#[test]
fn ec007_underscore_ok() {
    full("FIND_BUG/edge/bug-edge-underscore.lz", "underscore test done").unwrap();
}

// ══════════════════════════════════════════════════════════════
// ❌ 确认 bug 用例（#[ignore] 挂起，修复后转正）
// 卡点阶段编码：LZ_REJECT = 解析/词法拒绝；RUSTC_FAIL = rustc 段；
// RUN_WRONG = 运行语义错；SILENT_PASS = 负向用例被放行
// ══════════════════════════════════════════════════════════════

// BUG-LX-002: 嵌套块注释不支持（P3）
#[test]
#[ignore = "待修 BUG-LX-002：嵌套 /* */ 注释在首个 */ 即终止（P3）"]
fn lx002_nested_comment() {
    full("FIND_BUG/lexer/bug-comment-nested.lz", "bug-comment-nested.lz done").unwrap();
}

// BUG-LX-005: 内联 x =: expr 拒绝（仅支持换行块形态）
#[test]
#[ignore = "待修 BUG-LX-005：内联 =: 拒绝，LZ_REJECT: Expected Indent（P1）"]
fn lx005_inline_build_assign() {
    full("FIND_BUG/lexer/bug-equals-colon-ambiguity.lz", "bug-equals-colon-ambiguity.lz done").unwrap();
}

// BUG-PR-001: 顶层 =: 构建块
#[test]
#[ignore = "待修 BUG-PR-001：顶层 x =: 多行 body 拒绝（P1）"]
fn pr001_top_level_build() {
    full("FIND_BUG/parser/bug-top-level-build.lz", "greet result:").unwrap();
}

// BUG-PR-002: raises + -> 同行共存
#[test]
#[ignore = "待修 BUG-PR-002：raises 与返回类型同行两种顺序均拒绝（P2）"]
fn pr002_raises_with_return() {
    full("FIND_BUG/parser/bug-raises-return-type.lz", "raises+return test:").unwrap();
}

// BUG-PR-005: @decorator 用于变量应被拒绝（负向护城河）
#[test]
fn pr005_decorator_on_var_negative() {
    // 修复后 lzc 在解析阶段拒绝「装饰器修饰变量」，退出码非 0
    reject("FIND_BUG/parser/bug-decorator-on-var.lz").unwrap();
}

// BUG-TY-001: duck 自引用参数 E0391
#[test]
#[ignore = "待修 BUG-TY-001：duck Comparable 自引用 → trait 非 dyn 兼容 RUSTC_FAIL E0391（P0）"]
fn ty001_duck_generic() {
    full("FIND_BUG/typer/bug-duck-generic", "duck-generic.lz").unwrap();
}

// BUG-TY-002: 已修（2026-09-03）——顶层 self-def 挂 impl + 调用点方法语法 + mut self 透传
#[test]
fn ty002_self_underscore() {
    full("FIND_BUG/typer/bug-self-underscore.lz", "bug-self-underscore.lz done").unwrap();
}

// BUG-TY-004: __Params.new() 点调用错编
#[test]
#[ignore = "待修 BUG-TY-004：__Params::new() → __Params.new 点调用 RUSTC_FAIL E0423（P1）"]
fn ty004_params_type_erase() {
    full("FIND_BUG/typer/bug-params-type-erase.lz", "params-type-erase.lz done").unwrap();
}

// BUG-TY-005: 泛型默认值语法（报错误导）
#[test]
#[ignore = "待修 BUG-TY-005：泛型默认值不支持且报错误导（Expected type, got Gt）（P2）"]
fn ty005_generic_default() {
    full("FIND_BUG/typer/bug-generic-default-conflict.lz", "generic-default-conflict.lz done").unwrap();
}

// BUG-IR-001: ~: 参数位 BuildCall
#[test]
#[ignore = "待修 BUG-IR-001：filter(~: ...) 参数位 ~: 拒绝 LZ_REJECT: BuildCall（P1）"]
fn ir001_build_block_expr() {
    full("FIND_BUG/ir/bug-ir-build-block.lz", "bug-ir-build-block.lz done").unwrap();
}

// BUG-IR-002: defer guard 立即执行 + push stub
#[test]
#[ignore = "待修 BUG-IR-002：defer 体立即执行非块退出 + push stub E0308 RUSTC_FAIL（P1）"]
fn ir002_defer_guard() {
    full("FIND_BUG/ir/bug-ir-defer.lz", "bug-ir-defer.lz done").unwrap();
}

// BUG-IR-003: 嵌套 def static mut 提升 E0530
#[test]
#[ignore = "待修 BUG-IR-003：嵌套 def 捕获 → static mut 全局提升 RUSTC_FAIL E0530 + 捕获语义错（P0）"]
fn ir003_nested_function() {
    full("FIND_BUG/ir/bug-ir-nested-function.lz", "outer(5)(10): 15").unwrap();
}

// BUG-CG-002: 已修（2026-09-03）——__call__/__init__ 挂 impl + add5(10) → add5.__call__(10) 接线
#[test]
fn cg002_call_magic() {
    full("FIND_BUG/codegen/bug-codegen-call-magic.lz", "bug-codegen-call-magic.lz done").unwrap();
}

// BUG-CG-004: raises 静默丢弃
#[test]
#[ignore = "待修 BUG-CG-004：raises 修饰静默丢弃 + raise→panic! + try/catch 不存在（P1）"]
fn cg004_raises_result() {
    full("FIND_BUG/codegen/bug-codegen-raises.lz", "raises test done").unwrap();
}

// BUG-SB-001: fromMillis 已接线（三轮复验 2026-09-03：codegen camelCase 表 + Duration 静态调用）
#[test]
fn sb001_time_method() {
    full("FIND_BUG/stdbridge/bug-stdbridge-time-method.lz", "Duration fromMillis:").unwrap();
}

// BUG-SB-002: contains & 已修（变量 receiver + recv_has_custom_contains 守卫）
#[test]
fn sb002_vec_contains() {
    full("FIND_BUG/stdbridge/bug-stdbridge-vec-contains.lz", "contains 2:").unwrap();
}

// BUG-SB-003: startsWith 已接线（camelCase 方法表）
#[test]
fn sb003_starts_with() {
    // print 逐参输出带引号格式：`"startsWith hello:" true`
    full("FIND_BUG/stdbridge/bug-stdbridge-startswith.lz", "startsWith hello:").unwrap();
}

// BUG-SG-002: 已修（2026-09-03）——T? 位置自动 Some 包装（let 绑定 + struct 构造）
#[test]
fn sg002_null_coalesce() {
    // print 多参逐项带 Debug 引号：实际输出 `"None ?? 42:" 42`
    full("FIND_BUG/syntax/bug-syntax-null-coalesce.lz", "\"None ?? 42:\" 42").unwrap();
}

// BUG-SG-003: 已修（2026-09-03）——?. 链可空字段走 and_then 扁平化（非 map）
#[test]
fn sg003_safe_nav() {
    // 实际输出 `"safe nav host:" "localhost"`
    full("FIND_BUG/syntax/bug-syntax-safe-nav.lz", "\"safe nav host:\" \"localhost\"").unwrap();
}

// BUG-SG-005: ... 展开运算符（已修 轮次9）
#[test]
fn sg005_spread() {
    full("FIND_BUG/syntax/bug-syntax-spread.lz", "spread [0,...a,4]:").unwrap();
}

// BUG-EC-002: i64 溢出 → 拒绝（LZ 暂不支持 i128，避免静默环绕成 i64::MIN）
#[test]
fn ec002_int_overflow() {
    reject("FIND_BUG/edge/bug-edge-int-overflow.lz").unwrap();
}

// BUG-EC-006: type_name 内省（已修 轮次10）
#[test]
fn ec006_type_name() {
    full("FIND_BUG/edge/bug-edge-type-name.lz", "type_name(42):").unwrap();
}

// core 组：fn 类型注解参数解析（fold/compose/unique 同根）
#[test]
#[ignore = "待修 core/fn-annotation：fn(b, a) -> b 参数类型解析失败 LZ_REJECT: Expected param, got LParen（P1，阻塞 std/func.lz）"]
fn core_fold() {
    full("FIND_BUG/core/fold.lz", "fold").unwrap();
}

#[test]
#[ignore = "待修 core/fn-annotation：同 core_fold（P1）"]
fn core_compose() {
    full("FIND_BUG/core/compose.lz", "compose").unwrap();
}

#[test]
#[ignore = "待修 core/fn-annotation：同 core_fold（P1）"]
fn core_unique() {
    full("FIND_BUG/core/unique.lz", "unique").unwrap();
}
