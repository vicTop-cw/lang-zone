// FIND_BUG 36 编号回归守护套件：lz → (rs) → rustc → run 全链路
// 约定（对齐 tests/find_bug_libs.rs）：
//   - ✅ 无 bug 用例：直接转正（任一回归即红）
//   - ❌ 确认 bug 用例：#[ignore] 分级挂起，ignore reason 记录卡点阶段，
//     修复后移除 #[ignore] 转正（任一用例被放行即红的负向守护用 *Negative 命名）
//   - 判定证据：FIND_BUG.md「实测记录（2026-09-03）」章节
// 全量基线：39 非库用例 = 12 无bug / 21 确认bug / 1 部分问题 / 2 待单测（已补测全绿）
// ↑ 那是 2026-09-03 的口径。此后每轮往里加锁，对照请以**实测当轮数**为准：
//   2026-10-01 起本套件 45 收集 / 45 passed / 0 ignored（D6 三条 + 本轮 BUG-11 一条，
//   另 BUG-5、BUG-9 两条 #[ignore] 转正）。

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
                let n = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
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
    let out = Command::new(&bin)
        .arg(&lz)
        .output()
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
                .arg("-L")
                .arg(format!("dependency={}", builtins_rlib().parent().unwrap().join("deps").display()))
                .arg("-A")
                .arg("warnings")
                .arg("-o")
                .arg(&exe)
                .output()
                .map_err(|e| format!("rustc err: {}", e))?;
            if !rc.status.success() {
                return Err(format!(
                    "RUSTC_FAIL {}: {}",
                    rs.display(),
                    String::from_utf8_lossy(&rc.stderr)
                ));
            }
            let run = Command::new(&exe)
                .output()
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
    full(
        "FIND_BUG/lexer/bug-escape-unicode.lz",
        "bug-escape-unicode.lz done",
    )
    .unwrap();
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
    full(
        "FIND_BUG/parser/bug-typealias-magic.lz",
        "typealias-magic.lz done",
    )
    .unwrap();
}

#[test]
fn pr004_typealias_magic_rejected_negative() {
    // 探针 p2 复验：type MyAdder = __add__ → Parse error: Expected type, got MagicMethod
    let dir = manifest().join("target/tmp_negative_pr004");
    let _ = std::fs::create_dir_all(&dir);
    let lz = dir.join("ta_magic.lz");
    std::fs::write(
        &lz,
        "type MyAdder = __add__\ndef main() =\n  print(\"no\")\n",
    )
    .unwrap();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().unwrap();
    assert!(!out.status.success(), "type = 魔法方法 应被 lzc 拒绝");
}

// BUG-IR-005: comptime: 块解析 + const 提升（Rust 编译期折叠）
#[test]
fn ir005_comptime_block_ok() {
    full(
        "FIND_BUG/ir/bug-ir-comptime.lz",
        "comptime block parsed successfully",
    )
    .unwrap();
}

// BUG-CG-001: ..: int 变参全链路
#[test]
fn cg001_varargs_full_ok() {
    full(
        "FIND_BUG/codegen/bug-codegen-varargs.lz",
        "bug-codegen-varargs.lz done",
    )
    .unwrap();
}

// BUG-CG-003: #!export 编译运行
#[test]
fn cg003_export_ok() {
    full(
        "FIND_BUG/codegen/bug-codegen-export.lz",
        "bug-codegen-export.lz done",
    )
    .unwrap();
}

// BUG-SB-004: kebab-case 透传
#[test]
fn sb004_kebab_ok() {
    full(
        "FIND_BUG/stdbridge/bug-stdbridge-kebab.lz",
        "my-lib-utils::helper::do_work",
    )
    .unwrap();
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
    full(
        "FIND_BUG/edge/bug-edge-underscore.lz",
        "underscore test done",
    )
    .unwrap();
}

// ══════════════════════════════════════════════════════════════
// ❌ 确认 bug 用例（#[ignore] 挂起，修复后转正）
// 卡点阶段编码：LZ_REJECT = 解析/词法拒绝；RUSTC_FAIL = rustc 段；
// RUN_WRONG = 运行语义错；SILENT_PASS = 负向用例被放行
// ══════════════════════════════════════════════════════════════

// BUG-LX-002: 嵌套块注释不支持（P3）
#[test]
fn lx002_nested_comment() {
    full(
        "FIND_BUG/lexer/bug-comment-nested.lz",
        "bug-comment-nested.lz done",
    )
    .unwrap();
}

// BUG-LX-005: 内联 x =: expr 拒绝（仅支持换行块形态）
#[test]
fn lx005_inline_build_assign() {
    full(
        "FIND_BUG/lexer/bug-equals-colon-ambiguity.lz",
        "bug-equals-colon-ambiguity.lz done",
    )
    .unwrap();
}

// BUG-PR-001: 顶层 =: 构建块
#[test]
fn pr001_top_level_build() {
    full("FIND_BUG/parser/bug-top-level-build.lz", "greet result:").unwrap();
}

// BUG-PR-002: raises 与 -> 返回类型同行共存，两种顺序都应接受并正确返回
//   顺序1: def f() -> str raises IOError
//   顺序2: def f() raises IOError -> str
#[test]
fn pr002_raises_with_return() {
    // BUG-CG-004（收口）：raises 函数返回 Rust Result<T, E>（E 为标准错误名 → LzError），
    // 调用点打印经 Debug 显示为 Ok("...")。两种声明顺序（-> 在前 / raises 在前）都应接受。
    full(
        "FIND_BUG/parser/bug-raises-return-type.lz",
        "\"raises+return test:\" Ok(\"config_v1\") Ok(\"config_v2\")",
    )
    .unwrap();
}

// BUG-PR-005: @decorator 用于变量应被拒绝（负向护城河）
#[test]
fn pr005_decorator_on_var_negative() {
    // 修复后 lzc 在解析阶段拒绝「装饰器修饰变量」，退出码非 0
    reject("FIND_BUG/parser/bug-decorator-on-var.lz").unwrap();
}

// BUG-TY-001: duck 自引用参数 E0391
#[test]
fn ty001_duck_generic() {
    // main 只打印标记串（LZ print 走 Debug 渲染，外层带引号）；
    // 回归点：递归 duck `Comparable.__lt__(&self, other: Self)` 能过类型检查+rustc
    full(
        "FIND_BUG/typer/bug-duck-generic.lz",
        "duck-generic.lz: type system check",
    )
    .unwrap();
}

// BUG-TY-002: 已修（2026-09-03）——顶层 self-def 挂 impl + 调用点方法语法 + mut self 透传
#[test]
fn ty002_self_underscore() {
    full(
        "FIND_BUG/typer/bug-self-underscore.lz",
        "bug-self-underscore.lz done",
    )
    .unwrap();
}

// BUG-TY-004: __Params.new() 点调用错编
#[test]
fn ty004_params_type_erase() {
    full(
        "FIND_BUG/typer/bug-params-type-erase.lz",
        "params-type-erase.lz done",
    )
    .unwrap();
}

// BUG-TY-005: 泛型默认值 + 空泛型实参 `Container<>`（已修复：parser 空泛型解析）
#[test]
fn ty005_generic_default() {
    full(
        "FIND_BUG/typer/bug-generic-default-conflict.lz",
        "generic-default-conflict.lz done",
    )
    .unwrap();
}

// BUG-IR-001: ~: 参数位 BuildCall → 闭包脱糖（已修复：parser 层 ~: 闭包形态转 Closure）
#[test]
fn ir001_build_block_expr() {
    full(
        "FIND_BUG/ir/bug-ir-build-block.lz",
        "bug-ir-build-block.lz done",
    )
    .unwrap();
}

// BUG-IR-002: defer guard 内联脱糖（方案 A）
#[test]
fn ir002_defer_guard() {
    full("FIND_BUG/ir/bug-ir-defer.lz", "bug-ir-defer.lz done").unwrap();
}

// BUG-IR-003: 嵌套 def 捕获外层变量 → 本地闭包返回（一等值，支持捕获）
#[test]
fn ir003_nested_function() {
    full("FIND_BUG/ir/bug-ir-nested-function.lz", "outer(5)(10): 15").unwrap();
}

// BUG-CG-002: 已修（2026-09-03）——__call__/__init__ 挂 impl + add5(10) → add5.__call__(10) 接线
#[test]
fn cg002_call_magic() {
    full(
        "FIND_BUG/codegen/bug-codegen-call-magic.lz",
        "bug-codegen-call-magic.lz done",
    )
    .unwrap();
}

// BUG-CG-004: raises → Result + try/catch 解包（已修复）
#[test]
fn cg004_raises_result() {
    full("FIND_BUG/codegen/bug-codegen-raises.lz", "raises test done").unwrap();
}

// BUG-SB-001: fromMillis 已接线（三轮复验 2026-09-03：codegen camelCase 表 + Duration 静态调用）
#[test]
fn sb001_time_method() {
    full(
        "FIND_BUG/stdbridge/bug-stdbridge-time-method.lz",
        "Duration fromMillis:",
    )
    .unwrap();
}

// BUG-SB-002: contains & 已修（变量 receiver + recv_has_custom_contains 守卫）
#[test]
fn sb002_vec_contains() {
    full(
        "FIND_BUG/stdbridge/bug-stdbridge-vec-contains.lz",
        "contains 2:",
    )
    .unwrap();
}

// BUG-SB-003: startsWith 已接线（camelCase 方法表）
#[test]
fn sb003_starts_with() {
    // print 逐参输出带引号格式：`"startsWith hello:" true`
    full(
        "FIND_BUG/stdbridge/bug-stdbridge-startswith.lz",
        "startsWith hello:",
    )
    .unwrap();
}

// BUG-SG-002: 已修（2026-09-03）——T? 位置自动 Some 包装（let 绑定 + struct 构造）
#[test]
fn sg002_null_coalesce() {
    // print 多参逐项带 Debug 引号：实际输出 `"None ?? 42:" 42`
    full(
        "FIND_BUG/syntax/bug-syntax-null-coalesce.lz",
        "\"None ?? 42:\" 42",
    )
    .unwrap();
}

// BUG-SG-003: 已修（2026-09-03）——?. 链可空字段走 and_then 扁平化（非 map）
#[test]
fn sg003_safe_nav() {
    // 实际输出 `"safe nav host:" "localhost"`
    full(
        "FIND_BUG/syntax/bug-syntax-safe-nav.lz",
        "\"safe nav host:\" \"localhost\"",
    )
    .unwrap();
}

// BUG-SG-005: ... 展开运算符（已修 轮次9）
#[test]
fn sg005_spread() {
    full("FIND_BUG/syntax/bug-syntax-spread.lz", "spread [0,...a,4]:").unwrap();
}

// BUG-EC-002: i128 支持后，9223372036854775808 透传为 i128（转正）
#[test]
fn ec002_int_overflow() {
    full("FIND_BUG/edge/bug-edge-int-overflow.lz", "i64::MAX:").unwrap();
}

// BUG-EC-006: type_name 内省（已修 轮次10）
#[test]
fn ec006_type_name() {
    full("FIND_BUG/edge/bug-edge-type-name.lz", "type_name(42):").unwrap();
}

// core 组：fn 类型注解参数已修复 → 转正为正向测试
#[test]
fn core_fold_ok() {
    full("FIND_BUG/core/fold.lz", "fold sum:").unwrap();
}

#[test]
fn core_compose_ok() {
    full("FIND_BUG/core/compose.lz", "compose (5+3)*2:").unwrap();
}

#[test]
fn core_unique_ok() {
    full("FIND_BUG/core/unique.lz", "unique [1,2,2,3,1,4,3]:").unwrap();
}

// ══════════════════════════════════════════════════════════════
// ✅ 已转正：fn 值载体可跨线程（BUG-9，2026-10-01 T0r5 续轮）
// ══════════════════════════════════════════════════════════════

// BUG-RC-SEND: fn 值被 go/spawn 捕获时载体必须 `Send + Sync`，否则 E0277。
// 归因校正（本轮实测，推翻 2026-09-30 卡片里的「T0r3 引入」）：把生成码的载体
// 逐一手改跑同一组 rustc 参数——
//   `Box<dyn Fn(i64) -> i64>`（HEAD 旧载体）⇒ E0277 cannot be sent between threads safely
//   `Rc<dyn Fn(i64) -> i64>`（T0r3 载体）   ⇒ E0277 同错
//   `Arc<dyn Fn(i64) -> i64>`（裸换指针）    ⇒ E0277 send + shared 两条
//   `Arc<dyn Fn(i64) -> i64 + Send + Sync>` ⇒ 编译通过、运行输出 "rc-send-marker ok"
// 即：这条缺口在 Box 时代同样存在，换 Arc 单独也不够，必须带 `+ Send + Sync` 上界。
// 落点：src/ir/codegen/mod.rs 的 fn_value_type / 柯里链 / 6 处闭包装箱统一发
// `Arc<dyn Fn(…) -> … + Send + Sync>` + `Arc::new(…)`。
#[test]
fn cg_fn_value_carrier_send_sync_in_spawn() {
    full(
        "FIND_BUG/codegen/bug-rc-fn-not-send-in-spawn.lz",
        "rc-send-marker ok",
    )
    .unwrap();
}

// ══════════════════════════════════════════════════════════════
// ✅ 已转正：元组槽位的空列表期望类型下推（BUG-11，2026-10-01 同轮）
// 由 BUG-5 的内联侧修复暴露：引用位修好后，定义位仍按字面量自身默认推断。
// ══════════════════════════════════════════════════════════════

// `let t: (List<List<int>>, List<int>) = ([], [1, 2])` 的第 0 槽：
// ListLit 元素位有下推（BUG-5），TupleLit 槽位改前没有 ⇒
// 发成 `Vec::<i64>::new()` 而槽位要 `Vec<Vec<i64>>`，E0308 expected Vec<Vec<i64>>, found Vec<i64>。
// 落点：src/ir/codegen/mod.rs 的 ExprKind::TupleLit 非 size_hint 臂（按位下推
// `IrType::Tuple` 的元素类型），并把 ListLit 的下推目标从「仅列表字面量」
// 扩到「列表/元组字面量」，使嵌套元组也能接力。
#[test]
fn cg_tuple_slot_empty_list_elem_ty_pushdown() {
    full(
        "FIND_BUG/codegen/bug-tuple-slot-empty-list-pushdown.lz",
        "([], [1, 2])",
    )
    .unwrap();
}

// ══════════════════════════════════════════════════════════════
// D6「BigInt 类型系统对齐」批次（2026-09-30，ns lz-explore / T0r4）
// 语料源：FIND_BUG/hunt-20260926/bug-{A,B,C,D}-*/（台账 BUG-1~4）
// 此前这 6 个复现件不在任何门禁里（grep tests/ = 0 命中）⇒ 闸门全绿也测不到它们。
// ══════════════════════════════════════════════════════════════

#[test]
fn d6_bigint_import_and_const_and_fnbody() {
    // BUG-1 + BUG-2 + BUG-3：顶层 `let x: bigint` 走 const 内联路径
    //   （改前：裸发 use num_bigint::BigInt ⇒ E0432；无 BigInt::from ⇒ E0308；const 上下文调用非 const ⇒ E0015）
    full("FIND_BUG/hunt-20260926/bug-A-numcrate-import-e0432/bigint_min.lz", "1234567890123456789000").unwrap();
    // BUG-4：函数体内 `let x: bigint = <huge>` 改前坍塌为 i128 且连 import 都不发
    full("FIND_BUG/hunt-20260926/bug-D-bigint-fnbody-type-collapse/bigint_fnbody.lz", "1234567890123456789000").unwrap();
}

#[test]
fn d6_complex_import_route() {
    // BUG-1 的 complex 同族：改前裸发 use num_complex::Complex64 ⇒ 孤立 crate E0432
    full("FIND_BUG/hunt-20260926/bug-A-numcrate-import-e0432/complex_min.lz", "Complex { re: 1.0, im: 2.0 }").unwrap();
}

// BUG-5（收口）：三层嵌套空列表的 comptime 内联位。
// 前半段（static 定义位）由 D6 轮的 ListLit 元素位下推修好；后半段是引用位——
// `println(a)` 走 ctx.comptime_consts 内联，`comptime_value_to_lit` 原先给每个
// 子表达式一律标 `IrType::Any`，codegen 的 ListLit 臂按 `is_nil`（Any→`()`）
// 把空列表发成 `()` ⇒ `vec![(), vec![()]]` E0308。
// 落点：src/ir/builder.rs 的 comptime_value_to_lit 增加声明类型入参，沿
// `ctx.top_level_consts` 递归带出元素类型（List/Vec 取 args[0]、Tuple 按位取），
// 取不到类型时仍退 Any ⇒ 与改前逐字相同。同类点：Tuple 臂一并修；
// `comptime <expr>`（:4335）与 comptime 块（:7626）两个调用点无声明类型可传，
// 显式传 None（行为不变）。
#[test]
fn d6_list3_nested_empty_pushdown_inlined() {
    full("FIND_BUG/hunt-20260926/bug-E-list3-empty-nested-pushdown-e0308/rv_list3.lz", "[[], [[]]]").unwrap();
}

// ══════════════════════════════════════════════════════════════
// ✅ 已转正：生成产物头部链接配方 + CLI stderr 可复制命令（BUG-10，2026-10-01）
// ══════════════════════════════════════════════════════════════
// lz_builtins re-export num-bigint/num-complex ⇒ 孤立 rustc 编译必须带
// `-L dependency`，否则连不用 bigint 的程序也整体报 E0463。产品化方案 A：
// ① 每个生成 .rs 的文件头带确定性（跨机稳定、可进 golden 快照）的静态配方块；
// ② CLI 单文件生成后在 stderr 打印本机解析好的完整可复制命令（不落盘）。
// 落点：src/ir/codegen/mod.rs 的 emit_prelude_base（静态块）＋
//       src/main.rs 的 print_link_recipe（stderr，接增量/项目/常规三条写出路径）。
#[test]
fn cg_generated_rs_header_link_recipe() {
    let rel = "FIND_BUG/codegen/bug-link-recipe-header.lz";
    let (lz, _dir) = case_lz(rel);
    let rs = lz.with_extension("rs");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz)
        .output()
        .expect("lzc run");
    assert!(
        out.status.success(),
        "LZ_FAIL {}: {}",
        lz.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    let rs_text = std::fs::read_to_string(&rs)
        .unwrap_or_else(|e| panic!("no .rs generated: {}: {}", rs.display(), e));
    // ① 产物头部静态配方块（确定性文本；prelude 前既有前导空行，故 trim 后断言）
    let head = rs_text.trim_start();
    assert!(
        head.starts_with("// ── LZ rustc link recipe (BUG-10"),
        "产物头部缺链接配方块，前 2 行: {}",
        rs_text.lines().take(2).collect::<Vec<_>>().join(" | ")
    );
    assert!(
        head.contains("//   rustc --edition 2021 --extern lz_builtins=<rlib> -L dependency=<deps_dir>"),
        "配方块缺可复制命令模板"
    );
    assert!(
        head.contains("Omitting -L gives error[E0463]"),
        "配方块缺 E0463 告警行"
    );
    // ② stderr 本机解析后的完整命令（-L dependency 必在场）
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("LZ LINK RECIPE (BUG-10)"),
        "stderr 缺配方行: {}",
        stderr
    );
    assert!(
        stderr.contains("-L dependency="),
        "stderr 配方缺 -L dependency: {}",
        stderr
    );
    assert!(
        stderr.contains("--extern lz_builtins="),
        "stderr 配方缺 --extern lz_builtins: {}",
        stderr
    );
}
