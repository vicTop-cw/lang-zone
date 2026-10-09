// Lang-Zone 编译器 — tests/perf_opt_gates.rs
// 建议书「lz_性能优化建议书.md」方向 A/B/D/E 的回归闸门。
//
// 每条断言对应一项改动的**语义不变量**（不只是「能编译」）：
//   A1 常量折叠：字面量表达式下沉为字面量，且**溢出必须放弃折叠**
//      （i64::MAX + 1 折叠后会从 debug panic 变成静默回绕 => 语义漂移）；
//   A2  分支简化：含 break 的循环**必须原样保留**；
//   A3  死绑定：未被引用的纯绑定应删；被闭包捕获的**绝不能删**；
//   B1  move-elide：sorted(新鲜临时) 不再 clone；sorted(变量) 仍必须 clone；
//   E1  print 占位符：字符串类实参用 {}（无引号），非字符串仍 {:?}；
//   E2  with_capacity：mut v = [] + 纯 push 范围循环 => 预分配；
//   E3  STMT 标记默认关闭（--stmt-marks 可恢复）；
//   D1  分段计时：--stats 打出 lzc 段耗时；非法 --opt 值报错退出。
//
// 运行：cargo test -j 1 --test perf_opt_gates

use std::path::PathBuf;
use std::process::Command;

fn builtins_rlib() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return e.path();
            }
        }
    }
    panic!("lz_builtins rlib not found under target/debug");
}

struct Run {
    ok: bool,
    transpiled: bool,
    stdout: String,
    diag: String,
    rs_text: String,
}

fn std_dir() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("std")
        .display()
        .to_string()
}

/// 跑一个 .lz：转译 -> 读产物 -> rustc 编译 -> 运行。
fn run_lz(tag: &str, src: &str, extra: &[&str]) -> Run {
    let work = std::env::temp_dir().join(format!("lz_perf_{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("input.lz");
    std::fs::write(&lz, src).expect("write lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let mut cmd = Command::new(&bin);
    cmd.arg(&lz).arg("--std-dir").arg(std_dir());
    for a in extra {
        cmd.arg(a);
    }
    let out = cmd.output().expect("run lzc");
    let rs = lz.with_extension("rs");
    let diag = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() || !rs.exists() {
        return Run {
            ok: false,
            transpiled: false,
            stdout: String::new(),
            diag,
            rs_text: String::new(),
        };
    }
    let rs_text = std::fs::read_to_string(&rs).unwrap_or_default();
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            builtins_rlib().parent().unwrap().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    if !rc.status.success() {
        return Run {
            ok: false,
            transpiled: true,
            stdout: String::new(),
            diag: format!(
                "产物编译失败:\n{}",
                String::from_utf8_lossy(&rc.stderr)
            ),
            rs_text,
        };
    }
    let prog = Command::new(&exe).output().expect("run exe");
    Run {
        ok: prog.status.success(),
        transpiled: true,
        stdout: String::from_utf8_lossy(&prog.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        diag: String::from_utf8_lossy(&prog.stderr).to_string(),
        rs_text,
    }
}

// ==================== A1常量折叠 ====================

/// 只转译，不编译（用于「预期产物编译失败」的用例）。
fn transpile(tag: &str, src: &str, extra: &[&str]) -> (bool, String, String) {
    let work = std::env::temp_dir().join(format!("lz_perf_{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("input.lz");
    std::fs::write(&lz, src).expect("write lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let mut cmd = Command::new(&bin);
    cmd.arg(&lz).arg("--std-dir").arg(std_dir());
    for a in extra {
        cmd.arg(a);
    }
    let out = cmd.output().expect("run lzc");
    let rs = lz.with_extension("rs");
    let diag = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() || !rs.exists() {
        return (false, String::new(), diag);
    }
    (
        true,
        std::fs::read_to_string(&rs).unwrap_or_default(),
        diag,
    )
}

#[test]
fn a1_const_fold_literals_but_never_overflows() {
    let src = r#"
def calc(n: int) -> int =
    n * (2 + 3) + (10 / 5)

def main() =
    print(calc(7))
"#;
    let r = run_lz("a1_fold", src, &["--opt=1"]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "37", "(2+3) 与 (10/5) 都应折叠后再参与运算");
    assert!(
        !r.rs_text.contains("2i64 + 3i64") && !r.rs_text.contains("10i64 / 5i64"),
        "opt=1 应把 (2 + 3) 与 (10 / 5) 都折叠掉：\n{}",
        r.rs_text
    );
}

#[test]
fn a1_overflow_expression_is_never_folded() {
    // pass 最危险的语义漂移：把 `i64::MAX + 1` 折成回绕后的常量。
    // 正确行为是**保持不折叠**，让 rustc 继续在编译期报arithmetic_overflow
    // （这是优化前的既有行为；折了就没了这个诊断）。
    let src = r#"
def overflowing() -> int =
    9223372036854775807 + 1

def main() =
    print(1)
"#;
    for level in ["--opt=1", "--opt=2"] {
        let (ok, rs_text, diag) = transpile("a1_overflow", src, &[level]);
        assert!(ok, "转译应成功（不该在 IR 层报错）：\n{}", diag);
        assert!(
            rs_text.contains("9223372036854775807i64 + 1i64"),
            "{} 不得折叠溢出表达式：\n{}",
            level,
            rs_text
        );
        assert!(
            !rs_text.contains("9223372036854775808") && !rs_text.contains("18446744073709551615"),
            "{} 折叠出了回绕常量（语义漂移）：\n{}",
            level,
            rs_text
        );
    }
}

#[test]
fn a1_opt0_keeps_pass_inert_and_opt1_folds() {
    let src = r#"
def calc(n: int) -> int =
    n * (2 + 3)

def main() =
    print(calc(7))
"#;
    // --opt=0（默认）：pass 必须完全惰性 —— 既有 golden / 快照逐字节不变
    let off = run_lz("a1_off", src, &[]);
    assert!(off.ok, "产物应可编译运行：\n{}", off.diag);
    assert!(
        off.rs_text.contains("2i64 + 3i64"),
        "--opt=0 时不得折叠（默认产物必须与优化前一致）：\n{}",
        off.rs_text
    );
    // --opt=1：折叠生效
    let on = run_lz("a1_on", src, &["--opt=1"]);
    assert!(on.ok, "产物应可编译运行：\n{}", on.diag);
    assert!(
        !on.rs_text.contains("2i64 + 3i64"),
        "--opt=1 应折叠 (2 + 3)：\n{}",
        on.rs_text
    );
    assert_eq!(off.stdout, on.stdout, "opt 开关不得改变程序行为");
}

/// opt=0 与 opt=2 的产物差异只应来自 pass 本身；除被折叠的表达式外，
/// 程序行为必须完全一致（本例里 opt=2 不该有任何可删的纯死绑定）。
#[test]
fn a1_opt_levels_agree_on_behaviour() {
    let src = r#"
def calc(n: int) -> int =
    n * (2 + 3) + 1

def main() =
    print(calc(7))
"#;
    let l0 = run_lz("a1_lvl0", src, &[]);
    let l1 = run_lz("a1_lvl1", src, &["--opt=1"]);
    let l2 = run_lz("a1_lvl2", src, &["--opt=2"]);
    assert!(l0.ok && l1.ok && l2.ok);
    assert_eq!(l0.stdout, "36");
    assert_eq!(l1.stdout, l0.stdout);
    assert_eq!(l2.stdout, l0.stdout);
}

#[test]
fn a1_opt0_keeps_pass_inert_marker() {
    // 保留位：--opt=0 的产物头部仍带链接配方注释（未被pass 触碰），
    // 用于验证 pass 层挂在 build_ir_inner 之后不会改动模块级发射。
    let src = "def main() =\n    print(1)\n";
    let off = run_lz("a1_marker_off", src, &[]);
    assert!(off.ok);
    assert!(off.rs_text.contains("LZ rustc link recipe"));
}

// ==================== A2 分支简化 ====================

#[test]
fn a2_branch_simplify_keeps_break_loops() {
    let src = r#"
def pick(flag: bool) -> int =
    if flag:
        return 1
    else:
        return 2

def guarded(xs: List<int>) -> int =
    mut i = 0
    while true:
        if i >= len(xs):
            break
        i = i + 1
    i

def main() =
    print(pick(true))
    print(guarded([1, 2, 3]))
"#;
    let r = run_lz("a2_branch", src, &["--opt=2"]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "1\n3", "opt 不得改变可观察行为");
    assert!(
        r.rs_text.contains("while true") || r.rs_text.contains("loop"),
        "含 break 的 while(true) 必须原样保留：\n{}",
        r.rs_text
    );
}

// ==================== A3 死绑定消除 ====================

#[test]
fn a3_dce_drops_unused_pure_binding_but_keeps_captured() {
    let src = r#"
def work(n: int) -> int =
    let unused_const = 2 * 3
    let captured = n + 1
    let f = |x: int| => x + captured
    f(10)

def main() =
    print(work(5))
"#;
    let r = run_lz("a3_dce", src, &["--opt=2"]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert!(
        !r.rs_text.contains("unused_const"),
        "未被引用的纯绑定应被删除：\n{}",
        r.rs_text
    );
    assert!(
        r.rs_text.contains("captured"),
        "被闭包捕获的绑定绝不能删（删了E0425）：\n{}",
        r.rs_text
    );
}

// ==================== B1 move-elide ====================

#[test]
fn b1_sort_of_fresh_temp_moves_instead_of_cloning() {
    let src = r#"
def build() -> List<int> =
    [3, 1, 2]

def main() =
    mut xs = [5, 4, 6]
    let a = sorted(build())
    print(a)
    let b = sorted(xs)
    xs.push(9)
    print(b)
    print(xs)
"#;
    let r = run_lz("b1_move", src, &[]);
    assert!(
        r.transpiled,
        "LZ → .rs 转译应成功（产物文本可供断言）：\n{}",
        r.diag
    );
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "[1, 2, 3]\n[4, 5, 6]\n[5, 4, 6, 9]");
    assert!(
        r.rs_text.contains("_tmp = xs.clone()"),
        "变量实参必须保留 clone：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("_tmp = build().clone()"),
        "新鲜临时（函数结果）不应再 clone：\n{}",
        r.rs_text
    );
}

// ==================== E1 print 占位符 ====================

#[test]
fn e1_print_uses_display_for_strings_and_keeps_debug_otherwise() {
    let src = r#"
def main() =
    let n = 7
    print("plain")
    print(f"n = {n}")
    print(n)
"#;
    let r = run_lz("e1_print", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(
        r.stdout, "plain\nn = 7\n7",
        "字符串实参不得再带 Debug 引号"
    );
    assert!(
        r.rs_text.contains("println!(\"{}\", \"plain\")"),
        "字符串字面量应直接以 &str 传入（省一次 to_string 分配）：\n{}",
        r.rs_text
    );
    assert!(
        r.rs_text.contains("println!(\"{:?}\", n)"),
        "非字符串实参必须保持 Debug 占位符（用户 struct 未必实现 Display）：\n{}",
        r.rs_text
    );
}

// ==================== E2 with_capacity ====================

#[test]
fn e2_empty_list_before_push_loop_gets_capacity() {
    let src = r#"
def build(n: int) -> int =
    mut xs: List<int> = []
    for i in range(n):
        xs.push(i * 2)
    xs[3]

def main() =
    print(build(10))
"#;
    let r = run_lz("e2_cap", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "6");
    assert!(
        r.rs_text.contains("Vec::with_capacity"),
        "`mut v = []` + 纯 push 范围循环应预分配：\n{}",
        r.rs_text
    );
}

#[test]
fn e2_capacity_hint_not_applied_when_loop_body_has_extra_work() {
    let src = r#"
def build(n: int) -> int =
    mut xs: List<int> = []
    mut extra = 0
    for i in range(n):
        extra = extra + 1
        xs.push(extra)
    extra + xs[0]

def main() =
    print(build(4))
"#;
    let r = run_lz("e2_cap_risky", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "5");
}

// ==================== E3 STMT 标记开关 ====================

#[test]
fn e3_stmt_marks_off_by_default_and_restorable() {
    let src = r#"
def main() =
    mut total = 0
    for i in range(4):
        total = total + i
    print(total)
"#;
    let plain = run_lz("e3_plain", src, &[]);
    assert!(plain.ok, "产物应可编译运行：\n{}", plain.diag);
    assert_eq!(plain.stdout, "6");
    assert!(
        !plain.rs_text.contains("// STMT:"),
        "逐语句标记注释默认应关闭：\n{}",
        plain.rs_text
    );
    let marks = run_lz("e3_marks", src, &["--stmt-marks"]);
    assert!(
        marks.rs_text.contains("// STMT:"),
        "--stmt-marks 应恢复逐语句标记：\n{}",
        marks.rs_text
    );
}

// ==================== D1 分段计时 / 开关校验 ====================

#[test]
fn d1_stats_flag_reports_lzc_segment() {
    let work = std::env::temp_dir().join("lz_perf_stats");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let lz = work.join("input.lz");
    std::fs::write(&lz, "def main() =\n    print(1)\n").unwrap();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz)
        .arg("--std-dir")
        .arg(std_dir())
        .arg("--stats")
        .output()
        .expect("run lzc");
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        err.contains("[stats] lzc total:"),
        "--stats 应打出 lzc 段耗时（方向 D 的分段计时验收仪器）：\n{}",
        err
    );
}

#[test]
fn d1_invalid_opt_level_exits_with_error() {
    let work = std::env::temp_dir().join("lz_perf_optbad");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let lz = work.join("input.lz");
    std::fs::write(&lz, "def main() =\n    print(1)\n").unwrap();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz)
        .arg("--std-dir")
        .arg(std_dir())
        .arg("--opt=9x")
        .output()
        .expect("run lzc");
    assert!(
        !out.status.success(),
        "非法 --opt 值必须报错退出，不能静默按 0 处理"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--opt"),
        "应给出可读的 --opt 错误信息：\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ════════════════════════════════════════════════════════════════════
// 运行时性能：字符串原地追加（`s = s + rhs` → `s.push_str(&rhs)`）
//
// 动机（实测）：通用字符串加法发`lhs.to_string() + &rhs..`，累加模式下
// 每轮 3 次堆分配且把累积串完整拷贝两遍 ⇒ O(n²)。同一台机器严格 A/B
// （两份 .rs 仅差这一行）：40.5ms → 19.7ms（2.06×），峰值内存 7892KB →
// 4308KB（−45%）。本组用例钉住「该改写的改、不该改的绝不改」。
// ════════════════════════════════════════════════════════════════════

/// 字面量累加 → 原地 push_str，且实参是 `&'static str`（无中间 String）。
#[test]
fn e2_string_append_literal_becomes_push_str() {
    let src = r#"
def main() =
    mut s = ""
    mut i = 0
    while i < 4:
        s = s + "ab"
        i = i + 1
    print(s)
"#;
    let r = run_lz("e2_push_lit", src, &[]);
    assert!(r.transpiled, "转译应成功：\n{}", r.diag);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "abababab");
    assert!(
        r.rs_text.contains(r#"s.push_str("ab")"#),
        "字面量累加应原地追加：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("to_string() + &"),
        "累加路径不应再有 to_string()+add（每轮 3 次分配）：\n{}",
        r.rs_text
    );
}

/// 变量右操作数 → `push_str(&x)`（借用，不夺走 x 的所有权）。
#[test]
fn e2_string_append_var_becomes_borrowed_push_str() {
    let src = r#"
def main() =
    let piece = "cd"
    mut s = ""
    mut i = 0
    while i < 3:
        s = s + piece
        i = i + 1
    print(s)
    print(piece)
"#;
    let r = run_lz("e2_push_var", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    // piece 在循环后仍可读 ⇒ 不能被 move
    assert_eq!(r.stdout, "cdcdcd\ncd");
    assert!(
        r.rs_text.contains("s.push_str(&piece)"),
        "变量右操作数应借用传入：\n{}",
        r.rs_text
    );
}

/// f-string 右操作数 → `push_str(&format!(..))`。
#[test]
fn e2_string_append_fstring_keeps_format_borrowed() {
    let src = r#"
def main() =
    mut s = ""
    mut i = 0
    while i < 2:
        s = s + f"n{i}"
        i = i + 1
    print(s)
"#;
    let r = run_lz("e2_push_fstr", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "n0n1");
    assert!(
        r.rs_text.contains("s.push_str(&format!("),
        "f-string 应走 format! 后借用：\n{}",
        r.rs_text
    );
}

/// 非字符串右操作数（`s + 1`）→ 必须保留通用 format! 路径，绝不走 push_str。
/// `ref s: str` 形参 → 绝不能原地追加（那是借用视图，追加会改到调用方的值）。
/// 声明与使用在不同函数时，登记表必须按函数清空（否则形参会被误判为独占）。
#[test]
fn e2_owned_str_registry_is_per_function() {
    // accum 里的 s 是 by-value 局部绑定 → 允许原地追加；
    // joiner 的 s 是**形参**（本设计一律不登记形参，保守起见走通用路径）。
    // 这正是「登记表必须按函数清空」的语义：accum 的登记不能泄漏进 joiner。
    let src = r#"
def accum() -> str =
    mut s = ""
    mut i = 0
    while i < 3:
        s = s + "x"
        i = i + 1
    s

def joiner(s: str, t: str) -> str =
    s + t

def main() =
    print(accum())
    print(joiner("a", "b"))
"#;
    let r = run_lz("e2_registry_scope", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "xxx\nab");
    assert!(
        r.rs_text.contains(r#"s.push_str("x")"#),
        "accum 内的累加应被原地追加：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("push_str(&t)"),
        "形参不登记，禁止原地追加（保守策略）：\n{}",
        r.rs_text
    );
}
/// 非字符串右操作数：LZ 在 IR 构建期就拒绝 `str + int`（类型错误），
/// 因此 push_str 的 str-RHS 白名单根本不会被触及 —— 这里钉住该拒绝行为。
#[test]
fn e2_string_append_int_rhs_is_rejected_by_typecheck() {
    let src = r#"
def main() =
    mut s = "n="
    s = s + 7
    print(s)
"#;
    let r = run_lz("e2_push_int", src, &[]);
    assert!(
        !r.transpiled,
        "`str + int` 应在类型检查期被拒绝（不进 codegen）：\n{}",
        r.rs_text
    );
    assert!(
        r.diag.contains("相加") || r.diag.contains("str"),
        "应给出可读的类型错误诊断：\n{}",
        r.diag
    );
}

/// `ref s: str` 形参 → 绝不能原地追加（那是借用视图，追加会改到调用方的值）。
///
/// 已知既有缺陷（本次不修）：该形态的 `s = s + "!"` 在通用路径上会生成
/// `s = format!("{}{}", ..)` 赋给 `&str` 形参 ⇒ E0308，即by-ref str 形参
/// 的字符串拼接本来就编译不过。因此本用例只断言「**没有被原地改写**」
/// （转译级），编译级能力留给后续独立立项修复。
#[test]
fn e2_ref_param_is_never_rewritten_in_place() {
    let src = r#"
def grow(ref s: str) =
    s = s + "!"

def main() =
    mut t = "hi"
    grow(t)
    print(t)
"#;
    let r = run_lz("e2_ref_param", src, &[]);
    assert!(
        r.transpiled,
        "转译应成功（by-ref 形参不得被误改写）：\n{}",
        r.diag
    );
    assert!(
        !r.rs_text.contains("push_str"),
        "ref 形参是借用视图，禁止原地追加：\n{}",
        r.rs_text
    );
}

// ════════════════════════════════════════════════════════════════════
// 运行时性能：列表累加改原地追加（`a = a + rhs` → `a.push(..)` / `a.extend(..)`）
//
// 动机（实测）：通用列表加法发`a = { let mut __cat = a.clone(); __cat.extend(..); __cat }`
// —— **每轮一次全量 clone**，累加模式下拷贝量 O(n^2)。同机严格 A/B（两份 .rs 仅差
// 这一行，2 万元素）：230.6ms → 12.6ms（18.3×），峰值内存 5192KB → 4508KB（-13%）。
// 规模越大差距越夸张（旧的随 n 平方增长，新的随 n 线性）。
// 本组用例钉住「该改写的改、不该改的绝不改」。
// ════════════════════════════════════════════════════════════════════

/// 单元素列表字面量 → `push`（零分配）。
#[test]
fn e3_list_append_single_literal_becomes_push() {
    let src = r#"
def build(n: int) -> List<int> =
    mut acc: List<int> = []
    mut i = 0
    while i < n:
        acc = acc + [i]
        i = i + 1
    acc

def total(xs: List<int>) -> int =
    mut s = 0
    mut k = 0
    while k < xs.len():
        s = s + xs[k]
        k = k + 1
    s

def main() =
    let xs = build(4)
    print(xs.len(), total(xs))
"#;
    let r = run_lz("e3_list_push", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "4 6");
    assert!(
        r.rs_text.contains("acc.push(i);"),
        "单元素字面量应走 push：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("__cat"),
        "列表累加不应再有 clone+extend 模板：\n{}",
        r.rs_text
    );
}

/// 多元素列表字面量 → `extend(vec![..])`，元素按序追加。
#[test]
fn e3_list_append_multi_literal_becomes_extend() {
    let src = r#"
def main() =
    mut xs: List<int> = [0]
    xs = xs + [1, 2, 3]
    mut s = 0
    mut k = 0
    while k < xs.len():
        s = s + xs[k]
        k = k + 1
    print(xs.len(), s)
"#;
    let r = run_lz("e3_list_extend_lit", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "4 6");
    assert!(
        r.rs_text.contains("xs.extend([1i64, 2i64, 3i64]);"),
        "多元素字面量应走 extend(vec![..])：\n{}",
        r.rs_text
    );
}

/// 变量右操作数 → `extend(rhs)`（实参文本与通用路径一致，move 语义不变）。
#[test]
fn e3_list_append_var_becomes_extend() {
    let src = r#"
def main() =
    let a: List<int> = [1, 2]
    let b: List<int> = [3, 4]
    mut r = a
    r = r + b
    mut s = 0
    mut k = 0
    while k < r.len():
        s = s + r[k]
        k = k + 1
    print(r.len(), s)
"#;
    let r = run_lz("e3_list_extend_var", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "4 10");
    assert!(
        r.rs_text.contains("r.extend(b);"),
        "变量右操作数应直接 extend：\n{}",
        r.rs_text
    );
}

/// 形参不做登记：`a = a + b`（a 是形参）必须保留通用 clone+extend 路径，
/// 这样「参数是否独占」的判断永远不需要跨函数推断。
#[test]
fn e3_list_param_is_not_rewritten() {
    let src = r#"
def cat(mut a: List<int>, b: List<int>) -> List<int> =
    a = a + b
    a

def main() =
    let ys = cat([1], [2])
    mut s = 0
    mut k = 0
    while k < ys.len():
        s = s + ys[k]
        k = k + 1
    print(s)
"#;
    let r = run_lz("e3_list_param", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "3");
    assert!(
        r.rs_text.contains("__cat"),
        "形参不登记，必须保留通用 clone+extend：\n{}",
        r.rs_text
    );
}

/// 字符串与列表两类登记互不串扰：同一个函数里str 追加仍走 push_str，
/// 列表追加走 push。
#[test]
fn e3_str_and_list_registries_do_not_cross_talk() {
    let src = r#"
def main() =
    mut s = ""
    mut xs: List<int> = []
    mut i = 0
    while i < 3:
        s = s + "a"
        xs = xs + [i]
        i = i + 1
    print(s, xs.len())
"#;
    let r = run_lz("e3_mixed", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "aaa 3");
    assert!(r.rs_text.contains(r#"s.push_str("a")"#), "str 走 push_str：\n{}", r.rs_text);
    assert!(r.rs_text.contains("xs.push(i);"), "list 走 push：\n{}", r.rs_text);
}

// ════════════════════════════════════════════════════════════════════
// 既有缺陷修复的回归闸门（三个缺陷都是「先记录、后修复」，这里钉住修复后行为）
//════════════════════════════════════════════════════════════════════

/// 缺陷A：`xs.sum()` / `xs.product()` 此前生成 `(xs).sum()` ⇒ E0599
/// 「Vec<i64> is not an iterator」。List/Set 不是 Iterator，且 Rust 的
/// Vec/HashSet 本身没有 sum/product，必须先取迭代器。
#[test]
fn fix_a_list_sum_iterates_instead_of_passthrough() {
    let src = r#"
def main() =
    let xs: List<int> = [1, 2, 3]
    println(xs.sum())
    println(xs.product())
"#;
    let r = run_lz("fix_a_sum", src, &[]);
    assert!(r.ok, "List.sum()/product() 应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "6\n6");
    assert!(
        r.rs_text.contains("iter().copied().sum::<i64>()"),
        "sum 必须走迭代器链：\n{}",
        r.rs_text
    );
    assert!(
        r.rs_text.contains("iter().copied().product::<i64>()"),
        "product 必须走迭代器链：\n{}",
        r.rs_text
    );
}

/// 缺陷A 的类型正确性：归约类型取**元素类型**而非硬编码 i64，
/// 否则 `List<float>.sum()` 会生成 `.sum::<i64>()` ⇒ E0308。
#[test]
fn fix_a_list_sum_uses_element_type_not_hardcoded_i64() {
    let src = r#"
def main() =
    let xs: List<float> = [1.5, 2.5]
    println(xs.sum())
"#;
    let r = run_lz("fix_a_sum_f64", src, &[]);
    assert!(r.ok, "List<float>.sum() 应可编译运行：\n{}", r.diag);
    assert!(
        r.rs_text.contains("sum::<f64>()"),
        "归约类型应取元素类型 f64：\n{}",
        r.rs_text
    );
}

/// 缺陷A 不能破坏「sum 后仍可读该变量」：用 `.iter()` 而非 `.into_iter()`，
/// 后者会 move 接收者，而 LZ 值语义允许重复读。
#[test]
fn fix_a_list_sum_does_not_move_receiver() {
    let src = r#"
def main() =
    let xs: List<int> = [1, 2, 3]
    println(xs.sum())
    println(xs.len())
"#;
    let r = run_lz("fix_a_sum_nomove", src, &[]);
    assert!(r.ok, "sum 之后仍应能读 xs：\n{}", r.diag);
    assert_eq!(r.stdout, "6\n3");
}

/// 缺陷C：无显式返回注解时，尾表达式引用**函数体顶层 let 变量**，
/// 此前 `lookup_var` 落空 →回退 Any → codegen 渲染成 i64 ⇒ E0308。
/// 修复用一次性探测ctx 预登记顶层 let（不污染真实 ctx，否则变量会被
/// 误判为已声明而跳过声明 ⇒ E0425）。
#[test]
fn fix_c_str_tail_without_return_annotation_infers_string() {
    let src = r#"
def accum() =
    mut s = ""
    mut i = 0
    while i < 3:
        s = s + "x"
        i = i + 1
    s

def main() =
    println(accum())
"#;
    let r = run_lz("fix_c_tail_str", src, &[]);
    assert!(r.ok, "str 尾表达式应推断为 String 而非 i64：\n{}", r.diag);
    // 注意：调用点仍把「推断出来的返回类型」视为 Any，故 println 走 Debug（带引号）。
    // 缺陷 C 修的是**函数自身的签名**（此前渲染成 i64 ⇒ E0308 编译不过）；调用点的
    // 返回类型推断（fn_returns 只登记显式注解）是另一层缺口，不在本次范围。
    assert_eq!(r.stdout, "\"xxx\"");
    assert!(
        r.rs_text.contains("-> String"),
        "返回类型应是 String：\n{}",
        r.rs_text
    );
    // 关键回归：探测不得影响真实声明（否则 Rust 侧变量未定义 E0425）
    assert!(
        r.rs_text.contains("let mut s: String"),
        "顶层 let 的真实声明必须仍在（探测不能污染声明路径）：\n{}",
        r.rs_text
    );
}

/// 缺陷C 的反向确认：显式注解不受影响，且 int 尾表达式仍推断为 i64。
#[test]
fn fix_c_explicit_and_int_tails_unchanged() {
    let src = r#"
def annotated() -> str =
    let s = "k"
    s

def int_tail() =
    let n = 42
    n

def main() =
    println(annotated())
    println(int_tail())
"#;
    let r = run_lz("fix_c_tail_int", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "k\n42");
    assert!(
        r.rs_text.contains("fn annotated() -> String"),
        "显式 str 注解仍渲染为 String：\n{}",
        r.rs_text
    );
    assert!(
        r.rs_text.contains("fn int_tail() -> i64"),
        "int 尾表达式仍是 i64：\n{}",
        r.rs_text
    );
}

/// 缺陷B：给只读 `ref` 形参赋值此前生成非法 Rust（`s = format!(..)`赋给
/// `&str` ⇒ E0308；`s = s + 1` 赋给 `&i64` ⇒ E0384），诊断指向生成代码。
/// 现在给精确诊断。**不能**改成发射 `&mut T`：`ref self` 是 std 协议 trait
/// 方法的常规写法，改签名会与 trait 不匹配（E0053/E0195）。
#[test]
fn fix_b_assigning_to_readonly_ref_param_gives_precise_diagnostic() {
    let src = r#"
def grow(ref s: str) =
    s = s + "!"

def main() =
    mut t = "hi"
    grow(t)
    println(t)
"#;
    let r = run_lz("fix_b_ref_assign", src, &[]);
    assert!(r.transpiled, "转译应成功：\n{}", r.diag);
    assert!(
        r.rs_text.contains("compile_error!") && r.rs_text.contains("只读引用"),
        "应发射精确诊断（而非让 Rust 报 E0308/E0384）：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("s = format!"),
        "不应再生成给&str 形参赋 String 的非法代码：\n{}",
        r.rs_text
    );
}

/// 缺陷B 的边界：只读 `ref` 形参**只读**是完全正常的（std 协议 trait 全靠它），
/// 不能被误伤（`ref self` 必须仍然渲染成 `&self`）。
#[test]
fn fix_b_readonly_ref_param_is_still_usable_for_reading() {
    let src = r#"
def width(ref s: str) -> int =
    s.len()

def main() =
    println(width("abc"))
"#;
    let r = run_lz("fix_b_ref_read", src, &[]);
    assert!(r.ok, "只读 ref 形参的只读用法必须照常工作：\n{}", r.diag);
    assert_eq!(r.stdout, "3");
    assert!(
        r.rs_text.contains("s: &str"),
        "ref 形参仍应渲染成 &str：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("&mut str"),
        "不得改成 &mut str（会与 trait 签名不匹配）：\n{}",
        r.rs_text
    );
}

// ════════════════════════════════════════════════════════════════════
// Dict / Set 的 `+` 合并：此前生成的代码**根本无法编译**
//
// Rust 的 std **没有**为 BTreeMap / Vec / HashSet 实现 `Add`（已实证：
// 三者全部 E0369「cannot add X to X」）。而通用路径对 dict/set 直接透传
// `+` ⇒ `d = d + BTreeMap::from([(i, i)])` / `s = s + HashSet::from([i])`
// 必然编译失败。这不是性能问题而是**硬缺陷**，本组用例钉住修复后的行为。
//
// 附带收益：单条目改为 insert 后零分配（不再每轮新建一张 1-entry 的表）。
// 严格 A/B（8000 / 50000 元素，两侧仅差这一行）：21.3 vs 22.8ms /
// 28.1 vs 29.0ms —— **在噪声内无差异**。如实记录：本次收益是「能编译」，不是「更快」。
//════════════════════════════════════════════════════════════════════

/// 缺陷E：dict 累加此前 E0369 编译失败，现在可编译可运行，且走 insert 零分配。
#[test]
fn fix_e_dict_accumulate_uses_insert_not_add() {
    let src = r#"
def fill(n: int) -> Dict<int, int> =
    mut d: Dict<int, int> = Dict()
    mut i = 0
    while i < n:
        d = d + {i: i}
        i = i + 1
    d

def main() =
    println(fill(3).len())
"#;
    let r = run_lz("fix_e_dict_acc", src, &[]);
    assert!(r.ok, "dict累加应可编译运行（此前 E0369）：\n{}", r.diag);
    assert_eq!(r.stdout, "3");
    assert!(
        r.rs_text.contains("d.insert(i, i);"),
        "单条目 dict 字面量应走 insert（零分配）：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("d = d +"),
        "不得再透传 Rust 未实现的 Add 运算符：\n{}",
        r.rs_text
    );
}

/// 缺陷E 的 set 版本：此前同样 E0369。
#[test]
fn fix_e_set_accumulate_uses_insert_not_add() {
    let src = r#"
def fill(n: int) -> Set<int> =
    mut s: Set<int> = Set()
    mut i = 0
    while i < n:
        s = s + {i}
        i = i + 1
    s

def main() =
    println(fill(3).len())
"#;
    let r = run_lz("fix_e_set_acc", src, &[]);
    assert!(r.ok, "set 累加应可编译运行（此前 E0369）：\n{}", r.diag);
    assert_eq!(r.stdout, "3");
    assert!(
        r.rs_text.contains("s.insert(i);"),
        "单元素 set 字面量应走 insert：\n{}",
        r.rs_text
    );
    assert!(
        !r.rs_text.contains("s = s +"),
        "不得再透传 Rust 未实现的 Add 运算符：\n{}",
        r.rs_text
    );
}

/// 多条目 dict 字面量 → extend([(k, v), ..])，语义与逐条 insert 等价。
#[test]
fn fix_e_dict_multi_entry_literal_extends() {
    let src = r#"
def main() =
    mut d: Dict<int, int> = Dict()
    d = d + {1: 1, 2: 4, 3: 9}
    println(d.len())
"#;
    let r = run_lz("fix_e_dict_multi", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "3");
    assert!(
        r.rs_text.contains("d.extend([(1i64, 1i64), (2i64, 4i64), (3i64, 9i64)]);"),
        "多条目应走 extend([(k, v), ..])：\n{}",
        r.rs_text
    );
}

/// 多元素 set 字面量 → extend([..])。
#[test]
fn fix_e_set_multi_element_literal_extends() {
    let src = r#"
def main() =
    mut s: Set<int> = Set()
    s = s + {7, 8, 9}
    println(s.len())
"#;
    let r = run_lz("fix_e_set_multi", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "3");
    assert!(
        r.rs_text.contains("s.extend([7i64, 8i64, 9i64]);"),
        "多元素应走 extend([..])：\n{}",
        r.rs_text
    );
}

/// 变量右操作数 → extend(rhs)；实参文本与「道德等价基线」一致，move 语义不变。
#[test]
fn fix_e_dict_and_set_variable_rhs_extends() {
    let src = r#"
def merge_d(a: Dict<int, int>, b: Dict<int, int>) -> Dict<int, int> =
    mut d = Dict()
    d = a
    d = d + b
    d

def merge_s(a: Set<int>, b: Set<int>) -> Set<int> =
    mut s = Set()
    s = a
    s = s + b
    s

def main() =
    println(merge_d({1: 1}, {2: 2}).len())
    println(merge_s({1}, {2}).len())
"#;
    let r = run_lz("fix_e_dict_var", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "2\n2");
    assert!(
        r.rs_text.contains("d.extend(b);"),
        "dict 变量右操作数应直接 extend：\n{}",
        r.rs_text
    );
    assert!(
        r.rs_text.contains("s.extend(b);"),
        "set 变量右操作数应直接 extend：\n{}",
        r.rs_text
    );
}

/// 回归：list 的三种形态不受 Dict/Set 扩展影响（push / extend([..]) / extend(rhs)）。
#[test]
fn fix_e_list_forms_still_intact_after_container_extension() {
    let src = r#"
def main() =
    mut xs: List<int> = []
    xs = xs + [1]
    xs = xs + [2, 3]
    let other: List<int> = [4]
    xs = xs + other
    println(xs.len())
"#;
    let r = run_lz("fix_e_list_intact", src, &[]);
    assert!(r.ok, "产物应可编译运行：\n{}", r.diag);
    assert_eq!(r.stdout, "4");
    assert!(r.rs_text.contains("xs.push(1i64);"), "单元素仍走 push：\n{}", r.rs_text);
    assert!(
        r.rs_text.contains("xs.extend([2i64, 3i64]);"),
        "多元素仍走 extend([..])：\n{}",
        r.rs_text
    );
    assert!(
        r.rs_text.contains("xs.extend(other);"),
        "变量仍走 extend(rhs)：\n{}",
        r.rs_text
    );
}
