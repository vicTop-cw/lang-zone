// Lang-Zone 编译器 — tests/tail_recursion_matrix.rs
// 尾递归 / 非尾递归**系统性**检测矩阵（Auto TCO 能力普查 + 语义回归闸门）。
//
// 目的：
//   1. 用一张数据表覆盖尽可能多的递归形态：直接尾递归、累加器风格、互递归、树形/
//      分叉递归、类方法、高阶函数、λ 闭包、生成器、异常处理、defer、match 分支与
//      守卫、短路运算、默认形参、深递归等。
//   2. 每例同时记录两件事：
//        · **优化是否生效** —— 生成产物是否出现改写标记 `__tco_res`；
//        · **语义是否保持** —— 产物真编译真运行，输出与期望逐行比对。
//   3. 「未优化」本身**不是**失败（保守边界是设计的一部分），失败条件只有：
//      期望优化却没优化 / 期望不改写却改了 / 语义漂移 / 编译或运行失败。
//   4. 与TCO 无关的**既有**缺陷（try+enum 异常、raises+enum、递归生成器 yield from）
//      用 `TranspileOnly` / `TranspileBlocked` 显式记录，而不是把它们算作 TCO 的锅，
//      也不因此放宽 TCO 自身的判定。
//
// 运行：cargo test -j 1 --test tail_recursion_matrix -- --nocapture

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tco {
    /// 期望被自动改写为循环
    Optimized,
    /// 期望保持原样（保守边界 / 非尾位置 / 非直接自调用）
    Untouched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    /// 期望正常退出且输出匹配
    Ok,
    /// 期望运行期失败（典型：非尾深递归栈溢出）——记录「优化未覆盖」的事实
    Crash,
    /// 转译必须成功、TCO 状态必须符合预期，但**不要求**产物可运行：
    /// 该形态触发了与 TCO 无关的既有 codegen 缺陷（见用例 why 列）
    TranspileOnly,
    /// 转译本身即被既有缺陷拒绝（记录事实用，不参与成败判定）
    TranspileBlocked,
}

struct Case {
    group: &'static str,
    name: &'static str,
    src: &'static str,
    tco: Tco,
    run: Run,
    why: &'static str,
    out: &'static str,
}

// ═══════════════════════════════════════════════════════════════════
// 基础设施
// ═══════════════════════════════════════════════════════════════════

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

/// 从 lzc 的输出里取「自动改写 N 个尾递归函数」的 N。
///
/// 注意：`[tco] ...` 提示走 **stderr**（cli.rs::report_tco_warnings 用 eprintln!），
/// 所以两处输出都要扫。
///
/// 不能只用产物里的 `__tco_res` 作判据：Unit 返回的函数改写后**不引入**结果变量
/// （`let x: () = ();` 会被后端省略），此时只有计数行能反映真实改写。
fn tco_count(lzc_output: &[&str]) -> usize {
    for text in lzc_output {
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("[tco]") {
                let digits: String = rest
                    .chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = digits.parse::<usize>() {
                    return n;
                }
            }
        }
    }
    0
}

/// 过滤 lzc 打到 stdout 的提示行，只留程序输出
fn clean(s: &str) -> String {
    s.lines()
        .filter(|l| {
            !l.starts_with("Generated ")
                && !l.starts_with("[tco]")
                && !l.starts_with("Bridge registry:")
                && !l.starts_with("LZ LINK RECIPE")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone)]
struct Report {
    group: &'static str,
    name: &'static str,
    why: &'static str,
    tco: Tco,
    run: Run,
    out: &'static str,
    got_tco: bool,
    transpiled: bool,
    compiled: bool,
    ran_ok: bool,
    out_match: bool,
    stdout: String,
    diag: String,
    ms: u128,
}

fn run_case(c: &Case, idx: usize) -> Report {
    let t0 = Instant::now();
    let (g, n, w, t, r, e) = (c.group, c.name, c.why, c.tco, c.run, c.out);
    let work = std::env::temp_dir().join(format!("lz_tcomx_{idx}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("input.lz");
    std::fs::write(&lz, c.src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz)
        .env_remove("LZ_TCO")
        .output()
        .expect("run lang-zone");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let lzc_stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let rewritten = tco_count(&[&lzc_stdout, &stderr]);
    let rs = lz.with_extension("rs");
    if !out.status.success() || !rs.exists() {
        return Report {
            group: g,
            name: n,
            why: w,
            tco: t,
            run: r,
            out: e,
            got_tco: false,
            transpiled: false,
            compiled: false,
            ran_ok: false,
            out_match: false,
            stdout: String::new(),
            diag: format!("转译失败:\n{stderr}"),
            ms: t0.elapsed().as_millis(),
        };
    }
    let rs_text = std::fs::read_to_string(&rs).unwrap_or_default();
    // 双重判据：编译器计数行 > 0，且产物里确有循环改写痕迹
    // （`__tco_res` 用于有返回值的函数，Unit 函数改写后无结果变量）
    let marker = rs_text.contains("__tco_res") || rs_text.contains("__tco_a");
    let got_tco = rewritten > 0 && marker;

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
        return Report {
            group: g,
            name: n,
            why: w,
            tco: t,
            run: r,
            out: e,
            got_tco,
            transpiled: true,
            compiled: false,
            ran_ok: false,
            out_match: false,
            stdout: String::new(),
            diag: format!("产物编译失败:\n{}", String::from_utf8_lossy(&rc.stderr)),
            ms: t0.elapsed().as_millis(),
        };
    }
    let run_out = Command::new(&exe).output().expect("run exe");
    let stdout = clean(&String::from_utf8_lossy(&run_out.stdout));
    let ran_ok = run_out.status.success();
    let out_match = stdout.trim() == c.out.trim();
    Report {
        group: g,
        name: n,
        why: w,
        tco: t,
        run: r,
        out: e,
        got_tco,
        transpiled: true,
        compiled: true,
        ran_ok,
        out_match,
        stdout,
        diag: String::from_utf8_lossy(&run_out.stderr).to_string(),
        ms: t0.elapsed().as_millis(),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 语料矩阵
// ═══════════════════════════════════════════════════════════════════

fn cases() -> Vec<Case> {
    vec![
        // ── A 直接尾递归：期望改写 ──────────────────────────────
        Case {
            group: "A直接尾递归",
            name: "A01_countdown_if_else",
            src: r#"
def count(n: int) -> int =
    if n <= 0:
        0
    else:
        count(n - 1)

def main() =
    print(count(5))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "if/else 两分支各自尾调用",
            out: "0",
        },
        Case {
            group: "A直接尾递归",
            name: "A02_accumulator_as_param",
            src: r#"
def sum_to(n: int, acc: int = 0) -> int =
    if n <= 0:
        acc
    else:
        sum_to(n - 1, acc + n)

def main() =
    print(sum_to(10))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "累加器经形参传递 → 改写为形参重赋（需 let mut 影子变量）",
            out: "55",
        },
        Case {
            group: "A直接尾递归",
            name: "A03_gcd_param_swap",
            src: r#"
def gcd(a: int, b: int) -> int =
    if b == 0:
        a
    else:
        gcd(b, a % b)

def main() =
    print(gcd(48, 18))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "尾调用的实参是另两个形参（重赋顺序敏感）",
            out: "6",
        },
        Case {
            group: "A直接尾递归",
            name: "A04_tail_in_match_arm",
            src: r#"
def step(n: int) -> int =
    match n:
        case 0 => 0
        case 1 => 1
        case _ => step(n - 1)

def main() =
    print(step(4))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "尾调用位于 match 分支体；分支值须转为 __tco_res + break",
            out: "1",
        },
        Case {
            group: "A直接尾递归",
            name: "A05_tail_in_match_guard_arm",
            src: r#"
def step(n: int) -> int =
    match n:
        case 0 => 0
        case _ if n < 0 => 0
        case _ => step(n - 1)

def main() =
    print(step(4))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "带守卫的 match 分支体尾调用（守卫在分支体之前求值）",
            out: "0",
        },
        Case {
            group: "A直接尾递归",
            name: "A06_tail_under_and",
            src: r#"
def walk(n: int) -> int =
    if (n > 0) and (n % 2 == 0):
        walk(n - 1)
    else:
        n

def main() =
    print(walk(6))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "尾调用位于 && 右操作数（短路之后的尾位置）",
            out: "5",
        },
        Case {
            group: "A直接尾递归",
            name: "A07_tail_under_or",
            src: r#"
def walk(n: int) -> int =
    if (n <= 0) or (n % 2 == 1):
        n
    else:
        walk(n - 1)

def main() =
    print(walk(6))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "尾调用位于 || 右操作数",
            out: "5",
        },
        Case {
            group: "A直接尾递归",
            name: "A08_tail_after_let",
            src: r#"
def loop_down(n: int) -> int =
    if n <= 0:
        0
    else:
        let m = n - 1
        loop_down(m)

def main() =
    print(loop_down(10))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "尾调用前有 let 绑定（求值顺序：先算 a0 再重赋）",
            out: "0",
        },
        Case {
            group: "A直接尾递归",
            name: "A09_early_return_then_tail",
            src: r#"
def capped(n: int) -> int =
    if n > 100:
        return 0
    if n <= 0:
        0
    else:
        capped(n - 1)

def main() =
    print(capped(5))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "函数体内多分支 + 提前 return，尾调用仍在尾位置",
            out: "0",
        },
        Case {
            group: "A直接尾递归",
            name: "A10_args_depend_on_each_other",
            src: r#"
def carry(n: int, a: int = 0, b: int = 0) -> int =
    if n <= 0:
        b
    else:
        carry(n - 1, a + 1, a + 2)

def main() =
    print(carry(2))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "两个形参互为表达式：必须先全部求值再统一重赋",
            out: "3",
        },
        Case {
            group: "A直接尾递归",
            name: "A11_unit_return_fn",
            src: r#"
def tick(n: int) =
    if n <= 0:
        print("done")
    else:
        tick(n - 1)

def main() =
    tick(3)
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "Unit 返回：改写为「裸表达式 + break」，不引入结果变量",
            out: "\"done\"",
        },
        Case {
            group: "A直接尾递归",
            name: "A12_str_return_fn",
            src: r#"
def label(n: int) -> str =
    if n <= 0:
        "end"
    else:
        label(n - 1)

def main() =
    print(label(3))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "返回 str：结果变量需 String 初值（print 用 {:?} 故带引号）",
            out: "\"end\"",
        },
        Case {
            group: "A直接尾递归",
            name: "A13_deep_100k_tail",
            src: r#"
def deep(n: int, acc: int = 0) -> int =
    if n <= 0:
        acc
    else:
        deep(n - 1, acc + n)

def main() =
    print(deep(100000))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "10 万层尾递归：改写后才能跑完（不改正必栈溢出）",
            out: "5000050000",
        },
        Case {
            group: "A直接尾递归",
            name: "A14_arg_has_side_effect",
            src: r#"
def rec(n: int) -> int =
    if n <= 0:
        0
    else:
        rec(n - 1) + tick()

def tick() -> int =
    print("t")
    1

def main() =
    print(rec(2))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "自调用结果参与加法 ⇒ 非尾位置（与副作用无关）",
            out: "\"t\"\n\"t\"\n2",
        },
        Case {
            group: "A直接尾递归",
            name: "A15_arg_contains_selfcall",
            src: r#"
def outer(n: int) -> int =
    if n <= 0:
        0
    else:
        outer(n - 1) + inner(n)

def inner(n: int) -> int =
    if n <= 0:
        0
    else:
        1 + inner(n - 1)

def main() =
    print(outer(2))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "外层调用结果参与加法 ⇒ 非尾；inner 亦非尾，两者都不改写",
            out: "3",
        },
        Case {
            group: "A直接尾递归",
            name: "A16_three_params",
            src: r#"
def tri(n: int, a: int, b: int) -> int =
    if n <= 0:
        a + b
    else:
        tri(n - 1, b, a + 1)

def main() =
    print(tri(4, 1, 2))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "三形参轮换（斐波那契式尾递归）",
            out: "7",
        },
        Case {
            group: "A直接尾递归",
            name: "A17_tail_arg_from_lambda_param",
            src: r#"
def step(n: int, k: int) -> int =
    if n <= 0:
        k
    else:
        step(n - 1, k)

def main() =
    let f = |x: int| step(3, x)
    print(f(7))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "尾调用实参来自 λ 的形参：改写只动 step，不动调用方",
            out: "7",
        },

        // ── B 非尾递归：明确不改写 ─────────────────────────────
        Case {
            group: "B非尾递归",
            name: "B01_factorial",
            src: r#"
def fact(n: int) -> int =
    if n <= 1:
        1
    else:
        n * fact(n - 1)

def main() =
    print(fact(5))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "n * fact(n-1)：结果参与乘法 ⇒ 非尾位置",
            out: "120",
        },
        Case {
            group: "B非尾递归",
            name: "B02_fib_naive",
            src: r#"
def fib(n: int) -> int =
    if n < 2:
        n
    else:
        fib(n - 1) + fib(n - 2)

def main() =
    print(fib(10))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "两次非尾调用相加",
            out: "55",
        },
        Case {
            group: "B非尾递归",
            name: "B03_local_accumulator",
            src: r#"
def sum_rec(n: int) -> int =
    if n <= 0:
        0
    else:
        n + sum_rec(n - 1)

def main() =
    print(sum_rec(10))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "累加器是局部变量而非形参：循环无法重入替换",
            out: "55",
        },
        Case {
            group: "B非尾递归",
            name: "B04_tree_double_branch",
            src: r#"
def total(d: int) -> int =
    if d <= 0:
        1
    else:
        total(d - 1) + total(d - 1)

def main() =
    print(total(3))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "树形/分叉递归：两个子调用都参与加法",
            out: "8",
        },
        Case {
            group: "B非尾递归",
            name: "B05_deep_non_tail_still_crashes",
            src: r#"
def deep_bad(n: int) -> int =
    if n <= 0:
        0
    else:
        1 + deep_bad(n - 1)

def main() =
    print(deep_bad(100000))
"#,
            tco: Tco::Untouched,
            run: Run::Crash,
            why: "10 万层**非尾**递归：明确记录 TCO 不覆盖、仍会栈溢出",
            out: "",
        },
        Case {
            group: "B非尾递归",
            name: "B06_accumulator_tail_then_postprocess",
            src: r#"
def sum_sq(n: int, acc: int = 0) -> int =
    if n <= 0:
        acc
    else:
        sum_sq(n - 1, acc + n * n)

def main() =
    print(sum_sq(4) + 1)
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "函数内自调用在尾位置；`+ 1` 在 main 里，不影响函数内改写",
            out: "31",
        },

        // ── C 相互递归 ──────────────────────────────────────────
        Case {
            group: "C互递归",
            name: "C01_mutual_tail_even_odd",
            src: r#"
def is_even(n: int) -> bool =
    if n == 0:
        true
    else:
        is_odd(n - 1)

def is_odd(n: int) -> bool =
    if n == 0:
        false
    else:
        is_even(n - 1)

def main() =
    print(is_even(10))
    print(is_odd(7))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "互递归且各自尾调用：v1 只改写直接自调用，按设计不转换",
            out: "true\ntrue",
        },
        Case {
            group: "C互递归",
            name: "C02_mutual_non_tail",
            src: r#"
def ping(n: int) -> int =
    if n <= 0:
        0
    else:
        1 + pong(n - 1)

def pong(n: int) -> int =
    if n <= 0:
        0
    else:
        ping(n - 1)

def main() =
    print(ping(4))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "互递归且调用参与运算 ⇒ 非尾",
            out: "2",
        },
        Case {
            group: "C互递归",
            name: "C03_mutual_three_functions",
            src: r#"
def a(n: int) -> int =
    if n <= 0:
        0
    else:
        b(n - 1)

def b(n: int) -> int =
    if n <= 0:
        0
    else:
        c(n - 1)

def c(n: int) -> int =
    if n <= 0:
        0
    else:
        a(n - 1)

def main() =
    print(a(7))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "三函数环形互递归：非直接自调用，不改写",
            out: "0",
        },

        // ── D 边界绕过 /作用域 ───────────────────────────────────
        Case {
            group: "D边界绕过",
            name: "D01_helper_in_while_body",
            src: r#"
def tick(n: int) -> int =
    if n <= 1:
        1
    else:
        tick(n - 1)

def drive(n: int) -> int =
    mut k = n
    mut last = 0
    while k > 0:
        last = tick(k)
        k -= 1
    last

def main() =
    print(drive(4))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "tick 自身尾递归 ⇒ 改写；drive 的 while 体只是调用方",
            out: "1",
        },
        Case {
            group: "D边界绕过",
            name: "D02_helper_in_for_body",
            src: r#"
def step(n: int) -> int =
    if n <= 1:
        1
    else:
        step(n - 1)

def total(xs: List<int>) -> int =
    mut acc = 0
    for x in xs:
        acc = acc + step(x)
    acc

def main() =
    print(total([3, 4]))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "step 自身尾递归 ⇒ 改写；for 体只是调用方",
            out: "2",
        },
        Case {
            group: "D边界绕过",
            name: "D03_recursion_inside_lambda",
            src: r#"
def helper(n: int) -> int =
    if n <= 0:
        0
    else:
        1 + helper(n - 1)

def apply(n: int) -> int =
    let f = |x: int| helper(x)
    f(n)

def main() =
    print(apply(5))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "helper 的自调用结果参与加法 ⇒ 非尾；λ 边界亦截断",
            out: "5",
        },
        Case {
            group: "D边界绕过",
            name: "D04_nested_def_self_recursion",
            src: r#"
def outer(n: int) -> int =
    def inner(k: int) -> int =
        if k <= 0:
            0
        else:
            inner(k - 1)
    inner(n)

def main() =
    print(outer(3))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "内嵌 def 的自调用是**它自己**的尾调用 ⇒ 改写（边界只截断外层）",
            out: "0",
        },
        Case {
            group: "D边界绕过",
            name: "D05_defer_in_tail_recursive_fn",
            src: r#"
def run(n: int) -> int =
    defer:
        print("bye")
    if n <= 0:
        0
    else:
        run(n - 1)

def main() =
    print(run(3))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "函数体含 defer：逐次调用的清理语义无法用循环保持 ⇒ 不改写",
            out: "\"bye\"\n\"bye\"\n\"bye\"\n\"bye\"\n0",
        },
        Case {
            group: "D边界绕过",
            name: "D06_tail_inside_try_block",
            src: r#"
enum E:
    Bad(int)

def guarded(n: int) -> int =
    let r = try:
        if n <= 0:
            0
        else:
            guarded(n - 1)
    catch E.Bad(v):
        v
    r

def main() =
    print(guarded(3))
"#,
            tco: Tco::Untouched,
            run: Run::TranspileOnly,
            why: "try 块内自调用判非尾；既有缺陷：try+enum 异常产物 E0308（与 TCO 无关）",
            out: "0",
        },
        Case {
            group: "D边界绕过",
            name: "D07_raises_function_not_transformed",
            src: r#"
enum MyErr:
    Bad(int)

def walk(n: int) -> int raises MyErr =
    if n <= 0:
        0
    else:
        if n == 2:
            raise MyErr.Bad(n)
        walk(n - 1)

def main() =
    let r = try:
        walk(4)
    catch MyErr.Bad(v):
        print(v)
        0
    print(r)
"#,
            tco: Tco::Untouched,
            run: Run::TranspileOnly,
            why: "raises 函数明确不转换；既有缺陷：raises+enum 产物 E0308（与 TCO 无关）",
            out: "2\n0",
        },
        Case {
            group: "D边界绕过",
            name: "D08_recursive_generator",
            src: r#"
iterator countdown(n: int) -> int =
    yield n
    if n > 1:
        yield from countdown(n - 1)

def main() =
    for v in countdown(3):
        print(v)
"#,
            tco: Tco::Untouched,
            run: Run::TranspileBlocked,
            why: "生成器体内自调用判非尾；既有缺陷：递归 yield from 被 IR build 拒绝",
            out: "3\n2\n1",
        },
        Case {
            group: "D边界绕过",
            name: "D09_method_self_recursion",
            src: r#"
struct Counter =
    n: int

    def down(self, k: int) -> int =
        if k <= 0:
            self.n
        else:
            self.down(k - 1)

def main() =
    let c = Counter(n: 7)
    print(c.down(3))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "方法经 self 递归：self 是引用形参，不可重入替换",
            out: "7",
        },
        Case {
            group: "D边界绕过",
            name: "D10_recursion_via_fn_param",
            src: r#"
def step(n: int) -> int =
    if n <= 0:
        0
    else:
        step(n - 1)

def apply_twice(f: fn(int) -> int, n: int) -> int =
    f(f(n))

def main() =
    print(apply_twice(step, 5))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "step 自身尾递归 ⇒ 改写；经函数形参间接调用不算自调用",
            out: "0",
        },
        Case {
            group: "D边界绕过",
            name: "D11_no_self_call_at_all",
            src: r#"
def plain(n: int) -> int =
    n + 1

def main() =
    print(plain(6))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "无自调用：自动模式静默跳过（@tailrec 下才报错）",
            out: "7",
        },
        Case {
            group: "D边界绕过",
            name: "D12_call_bound_to_let_is_not_tail",
            src: r#"
def side(n: int) -> int =
    if n <= 0:
        0
    else:
        let x = side(n - 1)
        x + 1

def main() =
    print(side(3))
"#,
            tco: Tco::Untouched,
            run: Run::Ok,
            why: "自调用值被let 绑定后再参与运算 ⇒ 非尾位置",
            out: "3",
        },
        Case {
            group: "D边界绕过",
            name: "D13_bool_return_tail",
            src: r#"
def flag_ok(n: int) -> bool =
    if n <= 0:
        true
    else:
        flag_ok(n - 1)

def main() =
    print(flag_ok(5))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "返回 bool：结果变量初值为 false",
            out: "true",
        },
        Case {
            group: "D边界绕过",
            name: "D14_option_return_tail",
            src: r#"
def maybe(n: int) -> int? =
    if n <= 0:
        None
    else:
        maybe(n - 1)

def main() =
    print(maybe(3))
"#,
            tco: Tco::Optimized,
            run: Run::Ok,
            why: "返回 Option：结果变量初值为 None",
            out: "None",
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════
// 矩阵执行
// ═══════════════════════════════════════════════════════════════════

#[test]
fn recursion_matrix() {
    let all = cases();
    let mut reports = Vec::with_capacity(all.len());

    for (i, c) in all.iter().enumerate() {
        let r = run_case(c, i);
        println!(
            "[{:>4}ms] {:<10} {:<34} tco={:<5} (期望 {:<5}) run={:?}",
            r.ms,
            r.group,
            r.name,
            if r.got_tco { "ON" } else { "off" },
            if r.tco == Tco::Optimized { "ON" } else { "off" },
            r.run,
        );
        reports.push(r);
    }

    println!("\n════════ 尾递归 / 非尾递归检测矩阵（{} 例）========", reports.len());
    println!(
        "{:<12}{:<34}{:<9}{:<9}{:<10}{}",
        "分组", "用例", "TCO", "转译", "编译", "运行/输出"
    );
    let mut failures: Vec<String> = Vec::new();
    let (mut on, mut off) = (0, 0);
    for r in &reports {
        let want_tco = r.tco == Tco::Optimized;
        let tco_ok = r.got_tco == want_tco;
        if r.got_tco {
            on += 1;
        } else {
            off += 1;
        }
        let run_ok = match r.run {
            Run::Ok => r.ran_ok && r.out_match,
            Run::Crash => !r.ran_ok || r.diag.contains("overflow") || r.diag.contains("stack"),
            // 只要求「转译成功 + TCO 状态正确」，产物可运行性由别的闸门负责
            Run::TranspileOnly => true,
            Run::TranspileBlocked => true,
        };
        let cell_tco = if tco_ok {
            if r.got_tco { "ON✓" } else { "off✓" }
        } else {
            "✗不符"
        };
        let cell_transpile = if r.transpiled { "ok" } else { "拒绝" };
        let cell_compile = match r.run {
            Run::TranspileOnly | Run::TranspileBlocked => "—",
            _ => {
                if r.compiled {
                    "ok"
                } else {
                    "✗失败"
                }
            }
        };
        let cell_run = match r.run {
            Run::Ok => {
                if r.ran_ok && r.out_match {
                    "ok"
                } else {
                    "✗不符"
                }
            }
            Run::Crash => {
                if run_ok {
                    "崩溃✓"
                } else {
                    "✗未崩"
                }
            }
            Run::TranspileOnly => "既有缺陷",
            Run::TranspileBlocked => "既有缺陷",
        };
        println!(
            "{:<12}{:<34}{:<9}{:<9}{:<10}{}",
            r.group, r.name, cell_tco, cell_transpile, cell_compile, cell_run
        );

        if !tco_ok {
            failures.push(format!(
                "[{}] {}：期望 TCO={}，实际={}（{}）",
                r.group,
                r.name,
                if want_tco { "ON" } else { "off" },
                if r.got_tco { "ON" } else { "off" },
                r.why
            ));
        }
        match r.run {
            Run::Ok | Run::Crash => {
                if !r.transpiled {
                    failures.push(format!("[{}] {}：转译失败\n{}", r.group, r.name, r.diag));
                } else if !r.compiled {
                    failures.push(format!(
                        "[{}] {}：产物编译失败\n{}",
                        r.group, r.name, r.diag
                    ));
                } else if !run_ok {
                    failures.push(format!(
                        "[{}] {}：运行结果不符（期望 {:?}）\n实际输出：\n{}\n诊断：\n{}",
                        r.group, r.name, r.run, r.stdout, r.diag
                    ));
                } else if !r.out_match {
                    failures.push(format!(
                        "[{}] {}：输出不匹配\n期望：\n{}\n实际：\n{}",
                        r.group, r.name, r.out, r.stdout
                    ));
                }
            }
            Run::TranspileOnly => {
                if !r.transpiled {
                    failures.push(format!(
                        "[{}] {}：本用例只要求转译成功，但转译失败\n{}",
                        r.group, r.name, r.diag
                    ));
                }
            }
            Run::TranspileBlocked => {}
        }
    }
    println!(
        "\n合计：优化生效 {} 例，未优化 {} 例，问题 {} 例",
        on,
        off,
        failures.len()
    );
    println!("（“既有缺陷”列表示该形态的运行期验证被与 TCO 无关的旧缺陷挡住，已在why 列注明）");

    if !failures.is_empty() {
        panic!(
            "矩阵检测到 {} 处问题：\n\n{}\n",
            failures.len(),
            failures.join("\n\n")
        );
    }
}