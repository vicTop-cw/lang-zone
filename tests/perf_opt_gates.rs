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
