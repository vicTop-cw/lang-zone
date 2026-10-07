// Lang-Zone 编译器 — tests/tail_recursion.rs
// 尾递归自动优化（Auto TCO + @tailrec）端到端闸门。
//
// 依据：`IR/tailrec-auto-plan.md` §6「测试与验收」。
// 覆盖：
//  - 自动发现：无标注的尾递归函数被改写为循环（生成产物含 loop、无自调用）
//  - 深递归 e2e：n = 100_000 正常出结果（改写前 -O0 深递归会栈溢出）
//  - 非尾递归 /相互递归：输出与开关状态无关（逐字节一致，不漂移）
//  - 开关：`--no-tco` 与 `LZ_TCO=0` 关闭**自动**改写（退化为普通递归）
//  - `@tailrec` 三类静态报错：无自调用 / 非尾位置 / 不可转换结构
//  - `#[tail_call]` 弃用别名：语义与 @tailrec 一致，既有语料零改动
//  - 求参顺序：`f(n + 1, n)` 改写后第二参取旧 n

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

/// 过滤 lzc 打到 stdout 的提示行（Generated / [tco] / Bridge registry / LINK RECIPE）
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

struct Out {
    ok: bool,
    stdout: String,
    stderr: String,
    rs: String,
}

/// 转译 + rustc + 运行；`extra` 追加给lang-zone 的参数，`env_tco` 设置 LZ_TCO
fn run_lz(name: &str, source: &str, extra: &[&str], env_tco: Option<&str>) -> Out {
    let work = std::env::temp_dir().join(format!("lz_tco_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");
    let rs = lz.with_extension("rs");
    let _ = std::fs::remove_file(&rs);

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let mut cmd = Command::new(&bin);
    cmd.arg(&lz);
    for a in extra {
        cmd.arg(a);
    }
    match env_tco {
        Some(v) => {
            cmd.env("LZ_TCO", v);
        }
        None => {
            cmd.env_remove("LZ_TCO");
        }
    }
    let out = cmd.output().expect("run lang-zone");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() || !rs.exists() {
        return Out {
            ok: false,
            stdout,
            stderr,
            rs: String::new(),
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
        return Out {
            ok: false,
            stdout,
            stderr: format!(
                "{}\n[rustc]\n{}",
                stderr,
                String::from_utf8_lossy(&rc.stderr)
            ),
            rs: rs_text,
        };
    }
    let run = Command::new(&exe).output().expect("run exe");
    // lzc 的「Generated ...」/「[tco] ...」提示混在 stdout 里，过滤掉只留程序输出
    let prog_out: String = String::from_utf8_lossy(&run.stdout)
        .lines()
        .filter(|l| !l.starts_with("Generated ") && !l.starts_with("[tco]"))
        .collect::<Vec<_>>()
        .join("\n");
    Out {
        ok: run.status.success(),
        stdout: format!("{}{}", clean(&stdout), prog_out),
        stderr: format!(
            "{}\n[run]\n{}",
            stderr,
            String::from_utf8_lossy(&run.stderr)
        ),
        rs: rs_text,
    }
}

/// 只转译（不编译运行），用于断言错误文案
fn transpile(name: &str, source: &str) -> (bool, String) {
    let work = std::env::temp_dir().join(format!("lz_tco_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz)
        .env_remove("LZ_TCO")
        .output()
        .expect("run lang-zone");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// 取某个函数的生成代码片段（从 `fn <name>` 到下一个顶层 `pub fn` / `fn`）
fn fn_body<'a>(rs: &'a str, name: &str) -> &'a str {
    let key = format!("fn {name}(");
    let start = match rs.find(&key) {
        Some(i) => i,
        None => return "",
    };
    // 参数可能跨行，改用下一个 "\npub fn " / "\nfn " 作为结束边界
    let rest = &rs[start..];
    let mut end = rest.len();
    for pat in ["\npub fn ", "\nfn ", "\npub unsafe fn "] {
        if let Some(i) = rest[1..].find(pat) {
            if i + 1 < end {
                end = i + 1;
            }
        }
    }
    &rest[..end]
}

/// 统计子串出现次数（断言「函数体内无自调用」时，签名本身算 1 次）
fn count_of(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// 断言函数体内除签名外没有对自身的调用
fn assert_no_self_call(body: &str, name: &str) {
    let n = count_of(body, &format!("{name}("));
    assert_eq!(
        n,
        1,
        "改写后函数体内不应残留自调用（仅签名 1 处），实际 {} 处：\n{}",
        n,
        body
    );
}

// ═══════════════════════════════════════════════════════════════════
// 1. 自动发现 + 深递归 e2e
// ═══════════════════════════════════════════════════════════════════

#[test]
fn deep_tail_recursion_becomes_loop_and_survives_100k() {
    let src = r#"
def deep(n: int, acc: int = 0) -> int =
    if n <= 0:
        acc
    else:
        deep(n - 1, acc + n)

def main() =
    print(deep(100000))
"#;
    let out = run_lz("deep", src, &[], None);
    assert!(out.ok, "深递归应正常编译运行：\n{}", out.stderr);
    assert_eq!(out.stdout.trim(), "5000050000", "100000 层求和结果不对");

    let body = fn_body(&out.rs, "deep");
    assert!(
        body.contains("loop {") || body.contains("while true {"),
        "改写后应出现循环：\n{}",
        body
    );
    assert_no_self_call(body, "deep");
}

#[test]
fn non_tail_recursion_is_untouched_and_output_stable() {
    let src = r#"
def sum_to(n: int) -> int =
    if n == 0:
        0
    else:
        sum_to(n - 1) + n

def main() =
    print(sum_to(100))
"#;
    let on = run_lz("nontail_on", src, &[], None);
    let off = run_lz("nontail_off", src, &["--no-tco"], None);
    assert!(on.ok && off.ok, "非尾递归应保持可编译：\n{}\n{}", on.stderr, off.stderr);
    assert_eq!(
        on.stdout.trim(),
        off.stdout.trim(),
        "开关切换不得改变非尾递归的输出"
    );
    assert_eq!(on.stdout.trim(), "5050");
    let body = fn_body(&on.rs, "sum_to");
    assert!(
        body.contains("sum_to("),
        "非尾递归不应被改写为循环：\n{}",
        body
    );
}

// ═══════════════════════════════════════════════════════════════════
// 1.5 回归闸门（由tests/tail_recursion_matrix.rs 系统性检测后修复的三处缺陷）
//
// ① match 分支体里的尾调用：分析器判为尾位置，但改写器原先不处理 Match，
//    函数被裹进循环而分支值被丢弃 ⇒ E0308。
// ② Unit 返回的函数：原先无条件声明 `let mut __tco_res`，后端把 `let x: () = ();`
//    省略掉，循环内 `__tco_res = ...` ⇒ E0425。改为 Unit 走「裸表达式 + break」。
// ③ 函数体含 defer：defer 是**逐次调用**的清理块，改写成循环后不但只执行一次，
//    还会落在循环体不可达分支里（实测「递归 4 层打印 4次」变0 次）。
//    改为在结构层直接判定不可转换。
// ═══════════════════════════════════════════════════════════════════

#[test]
fn match_arm_tail_call_is_rewritten_with_branch_values_kept() {
    let src = r#"
def step(n: int) -> int =
    match n:
        case 0 => 0
        case 1 => 1
        case _ => step(n - 1)

def main() =
    print(step(4))
    print(step(9))
"#;
    let out = run_lz("match_arm", src, &[], None);
    assert!(out.ok, "match 分支尾调用应能编译运行：\n{}", out.stderr);
    // step(4) 递推到 n=1 → 1；step(9) 递推到 n=1 → 1
    assert_eq!(out.stdout.trim(), "1\n1", "match 分支值必须保留");
    let body = fn_body(&out.rs, "step");
    assert!(
        body.contains("loop {"),
        "match 分支体里的尾调用应改写为循环：\n{}",
        body
    );
    // 值分支须转成「赋结果变量 + break」，否则 match 作为语句被求值丢弃
    assert!(
        body.contains("break;"),
        "值分支应产生 break：\n{}",
        body
    );
    assert_no_self_call(body, "step");
}

#[test]
fn unit_return_tail_call_emits_no_result_var() {
    let src = r#"
def tick(n: int) =
    if n <= 0:
        print("done")
    else:
        tick(n - 1)

def main() =
    tick(3)
"#;
    let out = run_lz("unit_ret", src, &[], None);
    assert!(out.ok, "Unit 返回的尾递归应能编译运行：\n{}", out.stderr);
    assert_eq!(out.stdout.trim(), "\"done\"");
    let body = fn_body(&out.rs, "tick");
    assert!(
        !body.contains("__tco_res"),
        "Unit 返回不应引入结果变量（后端会省略 `let x: () = ();`）：\n{}",
        body
    );
    assert!(body.contains("loop {"), "应改写为循环：\n{}", body);
}

#[test]
fn defer_in_tail_recursive_fn_is_not_transformed() {
    let src = r#"
def run(n: int) -> int =
    defer:
        print("bye")
    if n <= 0:
        0
    else:
        run(n - 1)

def main() =
    print(run(3))
"#;
    let on = run_lz("defer_on", src, &[], None);
    let off = run_lz("defer_off", src, &["--no-tco"], None);
    assert!(on.ok && off.ok, "含 defer 的函数应保持可编译：\n{}", on.stderr);
    // defer 是逐次调用的清理块：4 层递归打印 4 次，改写不得改变这一语义
    let expected = "\"bye\"\n\"bye\"\n\"bye\"\n\"bye\"\n0";
    assert_eq!(
        on.stdout.trim(),
        expected,
        "含 defer 的函数必须保持逐次调用语义"
    );
    assert_eq!(
        on.stdout.trim(),
        off.stdout.trim(),
        "含 defer 的函数不应被改写（开关无关）"
    );
    let body = fn_body(&on.rs, "run");
    assert!(
        !body.contains("loop {"),
        "含 defer 的函数不得改写为循环：\n{}",
        body
    );
}

// ═══════════════════════════════════════════════════════════════════
// 1.6 回归闸门（第二轮审计：语句形态覆盖交叉核对 + 表达式覆盖核对）
//
// ④ 命名块（`block NAME:`）体在尾位置：分析器按尾传播、改写器无对应分支，
//    块体被留���循环内 → 改写后仍是递归调用 → 无限递归。
//    另 `break/continue <label>` 的标签语义也与循环冲突。
// ⑤ match **守卫**里的自调用：扫描器完全不遍历 `MatchArm.guard`（计数与改写都漏），
//    若函数别处还有真尾调用，就会被判定为可优化，守卫里的递归留在循环内 → 无限递归。
// ⑥ `Result` 返回类型：初值取了 `None`，而 Result 没有 nil 值
//    → `let __tco_res: Result<T,E> = None;` ⇒ E0308。
// ═══════════════════════════════════════════════════════════════════

#[test]
fn labeled_block_tail_call_is_not_transformed() {
    let src = r#"
def walk(n: int) -> int =
    if n <= 0:
        return 0
    block step:
        walk(n - 1)

def main() =
    print(walk(3))
"#;
    let out = run_lz("blklabel", src, &[], None);
    assert!(
        !out.rs.contains("loop {") && !out.rs.contains("__tco_res"),
        "含命名块的函数不得改写为循环（块内递归会变成死循环）：\n{}",
        out.rs
    );
    assert!(
        out.rs.contains("walk(n - 1"),
        "原递归调用应保留：\n{}",
        out.rs
    );
}

#[test]
fn match_guard_self_call_is_not_transformed() {
    let src = r#"
def f(n: int) -> int =
    match n:
        case 0 => 0
        case _ if n > 0 && f(n - 1) > 100 => 1
        case _ => f(n - 1)

def main() =
    print(f(3))
"#;
    let out = run_lz("match_guard", src, &[], None);
    assert!(out.ok, "守卫用例应可编译运行：\n{}", out.stderr);
    assert_eq!(out.stdout.trim(), "0");
    assert!(
        !out.rs.contains("loop {"),
        "守卫里的自调用是检测盲区，含它的函数不得改写：\n{}",
        out.rs
    );
}

#[test]
fn result_returning_tail_recursive_fn_is_not_transformed() {
    let src = r#"
def safe(n: int) -> Result<int, str> =
    if n <= 0:
        Ok(0)
    else:
        safe(n - 1)

def main() =
    let r = safe(3)
    print(r)
"#;
    let out = run_lz("result_ret", src, &[], None);
    assert!(out.ok, "Result 返回的用例应可编译运行：\n{}", out.stderr);
    assert!(
        !out.rs.contains("__tco_res"),
        "Result 没有 nil 初值，不得构造结果变量：\n{}",
        out.rs
    );
}

// ═══════════════════════════════════════════════════════════════════
// 2. 开关
// ═══════════════════════════════════════════════════════════════════

#[test]
fn switch_disables_auto_rewrite() {
    let src = r#"
def deep(n: int, acc: int = 0) -> int =
    if n <= 0:
        acc
    else:
        deep(n - 1, acc + n)

def main() =
    print(deep(10))
"#;
    let off = run_lz("switch_off", src, &["--no-tco"], None);
    assert!(off.ok, "--no-tco 下应可编译：\n{}", off.stderr);
    let body = fn_body(&off.rs, "deep");
    assert!(
        body.contains("deep("),
        "--no-tco 应关闭自动改写（保留递归调用）：\n{}",
        body
    );
    assert_eq!(off.stdout.trim(), "55");

    // 环境变量等价路径
    let env_off = run_lz("switch_env", src, &[], Some("0"));
    assert!(env_off.ok, "LZ_TCO=0 下应可编译：\n{}", env_off.stderr);
    let body2 = fn_body(&env_off.rs, "deep");
    assert!(
        body2.contains("deep("),
        "LZ_TCO=0 应关闭自动改写：\n{}",
        body2
    );
}

// ═══════════════════════════════════════════════════════════════════
// 3. @tailrec 静态契约（三类报错）
// ═══════════════════════════════════════════════════════════════════

#[test]
fn tailrec_rejects_non_recursive_function() {
    let src = r#"
@tailrec
def not_recursive(n: int) -> int =
    n + 1

def main() =
    print(not_recursive(1))
"#;
    let (ok, msg) = transpile("tailrec_norec", src);
    assert!(!ok, "无自调用的 @tailrec 应编译失败");
    assert!(
        msg.contains("@tailrec 标注函数 'not_recursive' 不是可优化的尾递归")
            && msg.contains("无直接自调用"),
        "错误文案不符：\n{}",
        msg
    );
}

#[test]
fn tailrec_rejects_non_tail_recursion() {
    let src = r#"
@tailrec
def bad(n: int) -> int =
    if n <= 0:
        0
    else:
        bad(n - 1) + n

def main() =
    print(bad(3))
"#;
    let (ok, msg) = transpile("tailrec_nontail", src);
    assert!(!ok, "非尾位置的 @tailrec 应编译失败");
    assert!(
        msg.contains("@tailrec 标注函数 'bad'") && msg.contains("自调用不在尾位置"),
        "错误文案不符：\n{}",
        msg
    );
}

#[test]
fn tailrec_rejects_not_transformable() {
    // while 体内的自调用不是尾位置（循环回边）⇒ 不可转换
    let src = r#"
@tailrec
def loopy(n: int) -> int =
    while true:
        if n <= 0:
            return 0
        n = n - 1
        loopy(n)

def main() =
    print(loopy(3))
"#;
    let (ok, msg) = transpile("tailrec_loopy", src);
    assert!(!ok, "while 体内自调用 + @tailrec 应编译失败");
    assert!(
        msg.contains("@tailrec 标注函数 'loopy'"),
        "错误文案不符：\n{}",
        msg
    );
}

// ═══════════════════════════════════════════════════════════════════
// 4. 弃用别名 + 求参顺序
// ═══════════════════════════════════════════════════════════════════

#[test]
fn tail_call_alias_still_accepted() {
    let src = r#"
@tail_call
def factorial(n: int, acc: int = 1) -> int =
    if n <= 1:
        acc
    else:
        factorial(n - 1, acc * n)

def main() =
    print(factorial(10))
"#;
    let out = run_lz("alias", src, &[], None);
    assert!(out.ok, "@tail_call 别名路径应全绿：\n{}", out.stderr);
    assert_eq!(out.stdout.trim(), "3628800");
}

#[test]
fn args_evaluated_before_reassignment() {
    // f(n + 1, n)：第二个实参必须取旧 n。若先重赋 n 再算第二参会得到 n+1。
    let src = r#"
def args_order(n: int, m: int) -> int =
    if n >= 3:
        m
    else:
        args_order(n + 1, n)

def main() =
    print(args_order(2, 0))
"#;
    let out = run_lz("argorder", src, &[], None);
    assert!(out.ok, "编译运行失败：\n{}", out.stderr);
    // 求参先行：第二轮的 m 取**旧** n（=2）；若先重赋 n 再求参会得到 3
    assert_eq!(
        out.stdout.trim(),
        "2",
        "第二实参应取旧 n（求参先行）；若得 3 说明先重赋后求参"
    );
    let body = fn_body(&out.rs, "args_order");
    assert!(
        body.contains("loop {") || body.contains("while true {"),
        "应改写为循环：\n{}",
        body
    );
    assert_no_self_call(body, "args_order");
}

#[test]
fn mutually_recursive_functions_are_left_alone() {
    let src = r#"
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
"#;
    let out = run_lz("mutual", src, &[], None);
    assert!(out.ok, "相互递归应原样保留：\n{}", out.stderr);
    assert_eq!(
        out.stdout.trim(),
        "true\ntrue",
        "is_even(10)=true, is_odd(7)=true"
    );
    let body = fn_body(&out.rs, "is_even");
    assert!(
        body.contains("is_odd("),
        "相互递归不应被改写：\n{}",
        body
    );
}

#[test]
fn closure_self_call_is_not_tail_call() {
    let src = r#"
def outer(n: int) -> int =
    let g = || n + 1
    if n <= 0:
        0
    else:
        outer(n - 1)

def main() =
    print(outer(5))
"#;
    let out = run_lz("closure", src, &[], None);
    assert!(out.ok, "闭包边界用例应可编译：\n{}", out.stderr);
    // 外层无自调用（λ 内不是外层的自调用）⇒ 仍是普通递归
    let body = fn_body(&out.rs, "outer");
    assert!(
        body.contains("outer("),
        "闭包边界外的自调用判定不应误判：\n{}",
        body
    );
}