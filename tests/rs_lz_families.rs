//! fist-mbt T0r12 / namespace `lang-zone`：rs-lz 发射器四族回归（N2 / N3 / N4 / N5）
//!
//! 被测对象：`src/ir/lz_codegen_lib.lz`（LZ 写的 rs-lz 发射器，自举路线 B）。
//! 对应红名单 `Reports/redlist-2026-10-28.md` §4 的分族：
//!
//! | 族 | 形态 | 发射器根因 |
//! | --- | --- | --- |
//! | N2 | 切片 `s[a..b]` 两端类型不一致 | `as usize` 只拼在整串 range 末尾 |
//! | N3 | `x as int`（x: str）→ E0605 | `Expr.Cast` 无条件发 `as` |
//! | N4 | `+` 字符串拼接单侧补转换 | 要求两侧都是 str 且只补一个 `&` |
//! | N5 | 空 then 块 `if c  else {` | `gen_block` 空块返回空串 |
//!
//! 链路：`--emit=rs-lz` 只**打印**目标程序的 Rust 源码到 stdout，CLI 不编译它，
//! 所以「产物能否编过/跑通」由本文件自己 rustc + run。
//!
//! 附带（同文件同一分支）：实参 `panic(..)` 发成函数调用（Rust 里是宏，E0423）
//! 与 `def main() -> int` 尾表达式丢 `return`（E0308）的回归。

use std::path::PathBuf;
use std::process::Command;

fn lang_zone_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 生成产物需要 `--extern lz_builtins=<rlib>`（`List<T>`/`Dict<K,V>` 别名在其中）。
/// 从当前测试二进制的 target 目录出发找，避免与别的车道的 target 目录混淆。
fn builtins_rlib() -> PathBuf {
    let dir = lang_zone_bin()
        .parent()
        .expect("bin dir")
        .to_path_buf();
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("deps")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return e.path();
            }
        }
    }
    let fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    if fallback.exists() {
        return fallback;
    }
    panic!(
        "找不到 liblz_builtins.rlib（先跑 `cargo build -p lz_builtins`），查找基准目录: {}",
        dir.display()
    );
}

fn workdir(tag: &str) -> PathBuf {
    let work = std::env::temp_dir().join(format!("lz_rs_lz_families_{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    work
}

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

/// 以 wall-clock 上限运行子进程，超时即 kill 并按「未成功」返回。
///
/// T0r20 的缺陷形态正是「能编过但运行不终止」——若直接用 `.output()`，一旦回归，
/// 整个 `cargo test` 会被死循环挂死（没有超时就不可能判失败）。所以发射器、rustc、
/// 产物运行三条路径统一走这里。
fn output_with_timeout(mut cmd: Command, secs: u64) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn 子进程");
    let start = std::time::Instant::now();
    loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => {
                let mut out = Vec::new();
                let mut err = Vec::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_end(&mut out);
                }
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_end(&mut err);
                }
                return std::process::Output {
                    status,
                    stdout: out,
                    stderr: err,
                };
            }
            None => {
                if start.elapsed().as_secs() >= secs {
                    let _ = child.kill();
                    let status = child.wait().expect("wait 被 kill 的子进程");
                    return std::process::Output {
                        status,
                        stdout: Vec::new(),
                        stderr: format!("TIMEOUT: 子进程超过 {secs}s 未退出（疑似死循环）")
                            .into_bytes(),
                    };
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}

struct RsLzRun {
    /// 目标程序的 Rust 源码（`--emit=rs-lz` 的 stdout）
    rust: String,
    /// 生成期是否成功（发射器自身编译/运行失败时为 false）
    emitted: bool,
    /// 生成产物是否被 rustc 接受
    compile_ok: bool,
    /// 生成产物是否跑通
    run_ok: bool,
    /// 生成产物 stdout
    stdout: String,
    diag: String,
}

fn run_rs_lz(tag: &str, src: &str) -> RsLzRun {
    run_rs_lz_files(tag, &[("input.lz", src)])
}

/// 多文件版本：`files` 里每个 (文件名, 内容) 先落到同一个工作目录，再对 `input.lz`
/// 走完整链路。用于 `import moda as m` 这类需要**兄弟模块文件**才能解析的样本
/// （G7）。第一条必须是 input.lz。
fn run_rs_lz_files(tag: &str, files: &[(&str, &str)]) -> RsLzRun {
    let work = workdir(tag);
    for (name, body) in files {
        std::fs::write(work.join(name), body).expect("write lz file");
    }
    let lz = work.join("input.lz");

    let mut emit = Command::new(lang_zone_bin());
    emit.arg("--emit=rs-lz").arg(&lz);
    let out = output_with_timeout(emit, 600);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let rust = clean(&String::from_utf8_lossy(&out.stdout));
    if !out.status.success() {
        return RsLzRun {
            rust,
            emitted: false,
            compile_ok: false,
            run_ok: false,
            stdout: String::new(),
            diag: format!("--emit=rs-lz 生成期失败:\n{stderr}"),
        };
    }

    let rs = work.join("gen.rs");
    std::fs::write(&rs, &rust).expect("write generated rust");
    let exe = work.join("gen.exe");
    let mut rustc_cmd = Command::new("rustc");
    rustc_cmd
        .args(["--edition", "2021", "--crate-name", "gen_program"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            builtins_rlib().parent().unwrap().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe);
    let rc = output_with_timeout(rustc_cmd, 300);
    if !rc.status.success() {
        return RsLzRun {
            rust: rust.clone(),
            emitted: true,
            compile_ok: false,
            run_ok: false,
            stdout: String::new(),
            diag: format!(
                "rs-lz 生成的 Rust 编译失败:\n{}\n---- 产物 ----\n{}",
                String::from_utf8_lossy(&rc.stderr),
                rust
            ),
        };
    }

    // T0r20：运行也带上限（30s）——缺陷形态是「编得过但不终止」，不设上限会挂死测试进程
    let prog = output_with_timeout(Command::new(&exe), 30);
    RsLzRun {
        rust,
        emitted: true,
        compile_ok: true,
        run_ok: prog.status.success(),
        stdout: clean(&String::from_utf8_lossy(&prog.stdout)),
        diag: String::from_utf8_lossy(&prog.stderr).to_string(),
    }
}

fn assert_ok(r: &RsLzRun, what: &str) {
    assert!(r.emitted, "[{what}] rs-lz 生成失败:\n{}", r.diag);
    assert!(
        r.compile_ok,
        "[{what}] 生成产物编译失败（E-code 回归）:\n{}\n---- rust ----\n{}",
        r.diag, r.rust
    );
    assert!(r.run_ok, "[{what}] 生成产物运行失败:\n{}", r.diag);
}

// ════════════════════════════════════════════════════════════════════════════
// N2（17 条，最大族）：切片 range 两端类型不一致
//
// 旧产物 `s[idx..(idx + 1i64) as usize]`：`as` 按优先级只作用于上界，起点仍是
// i64 ⇒ E0308（expected i64, found usize）+ E0277（str 不能用 Range<i64> 索引）。
// 修法：`gen_range_index_key` 对**每个已给端点**各发 `((…) as usize)`（缺端点保持
// 开放），切片结果按基底 owned 化（str → `.to_string()`，List/Vec → `.to_vec()`）。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn n2_slice_range_casts_both_endpoints() {
    let src = r##"
def char_at(s: str, i: int) -> str =
    s[i..(i + 2)]

def head2(s: str) -> str =
    s[0..2]

def list_slice(xs: List<int>, a: int, b: int) -> List<int> =
    xs[a..b]

def main() =
    print(char_at("abcdef", 2))
    print(head2("abcdef"))
    print(list_slice([10, 20, 30, 40], 1, 3))
"##;
    let r = run_rs_lz("n2_slice", src);
    assert_ok(&r, "N2 切片两端 cast");
    // 两个端点都必须是 usize（起点不再裸着）
    assert!(
        r.rust.contains("((i) as usize)"),
        "切片起点未 cast（旧缺陷形态）:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("[i.."),
        "切片起点仍是裸 i64（`as` 只落到上界）:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains(".to_string()") && r.rust.contains(".to_vec()"),
        "切片结果未按基底 owned 化（str→to_string / List→to_vec）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("cd"), "输出不对: {}", r.stdout);
    assert!(r.stdout.contains("ab"), "输出不对: {}", r.stdout);
    assert!(
        r.stdout.contains("20") && r.stdout.contains("30") && !r.stdout.contains("40"),
        "List 切片结果不对: {}",
        r.stdout
    );
}

/// N2 姊妹面：标量下标的复合键也要整体 cast（`xs[i + 1]` 不能发成
/// `xs[i + 1i64 as usize]`），且非 Copy 元素须 `.clone()`（否则 E0507）。
#[test]
fn n2_scalar_index_key_casts_whole_expr() {
    let src = r##"
def second(xs: List<int>, i: int) -> int =
    xs[i + 1]

def first_word(ws: List<str>) -> str =
    ws[0]

def main() =
    print(second([10, 20, 30], 0))
    print(first_word(["alpha", "beta"]))
"##;
    let r = run_rs_lz("n2_scalar", src);
    assert_ok(&r, "N2 标量下标整体 cast");
    assert!(
        r.rust.contains("((i + 1i64) as usize)"),
        "复合下标键未整体 cast:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("ws[0i64 as usize].clone()"),
        "非 Copy 元素索引未补 .clone()（E0507）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("20"), "输出不对: {}", r.stdout);
    assert!(r.stdout.contains("alpha"), "输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// N3（6 条）：`int(str)` 发成 `as` cast → E0605 non-primitive cast
//
// BUG-477 只修了「被发成函数调用」的形态，覆盖不到 `as`-cast。修法：`Expr.Cast`
// 按目标/来源类型分派（对齐原生 expr_gen.rs:7329-7405），字符串源走 `.parse()`。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn n3_str_to_number_uses_parse() {
    let src = r##"
def as_int(s: str) -> int =
    s as int

def as_f64(s: str) -> f64 =
    s as f64

def via_field(r: (str, int)) -> int =
    r.0 as int

def main() =
    print(as_int("42"))
    print(as_f64("2.5"))
    print(via_field(("7", 0)))
"##;
    let r = run_rs_lz("n3_parse", src);
    assert_ok(&r, "N3 str→数值 parse");
    assert!(
        r.rust.contains(".parse::<i64>().unwrap()"),
        "str→int 未走 parse:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains(".parse::<f64>().unwrap()"),
        "str→f64 未走 parse:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains(" as i64)"),
        "仍有 String as i64 形态（E0605）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("42"), "输出不对: {}", r.stdout);
    assert!(r.stdout.contains("2.5"), "输出不对: {}", r.stdout);
    assert!(r.stdout.contains("7"), "输出不对: {}", r.stdout);
}

/// N3 反向：数值 → str 必须是 `format!` / `to_string()`，不能是 `as String`。
#[test]
fn n3_number_to_str_uses_format() {
    let src = r##"
def to_text(n: int) -> str =
    n as str

def main() =
    print(to_text(7))
"##;
    let r = run_rs_lz("n3_tostr", src);
    assert_ok(&r, "N3 数值→str");
    assert!(
        r.rust.contains("format!(\"{}\", n)"),
        "数值→str 未走 format!:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("as String")
            && !r.rust.contains("as str"),
        "仍发 `as String`（E0605）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("7"), "输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// N4（5 条）：二元 `+` 字符串拼接只单侧补转换
//
// Rust 的 `String + String` 无 Add impl（只能 `String + &str`），旧实现既要求
// 两侧都是 str 类型、又只补一个 `&`，于是 `"a".to_string() + s`、`s + ", ".to_string()`
// 一律 E0308（expected `&str`, found `String`）。BUG-450 只修了 native 侧。
// 修法：对齐原生 expr_gen.rs:6046-6100 —— 任一侧是 str 即进拼接分支，
// 右操作数统一 `&rhs[..]`，lhs 非 str / rhs 非 str 走 `format!`。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn n4_str_concat_borrows_right_side() {
    let src = r##"
def greet(name: str, n: int) -> str =
    "hi " + name + "#" + n.to_string()

def with_tail(head: str) -> str =
    head + ", " + "tail"

def main() =
    print(greet("bob", 3))
    print(with_tail("head"))
"##;
    let r = run_rs_lz("n4_concat", src);
    assert_ok(&r, "N4 字符串拼接");
    assert!(
        r.rust.contains(" + &"),
        "右操作数未取 &str:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("[..]"),
        "右操作数未按 String→&str 切片（`&rhs[..]`）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("hi bob#3"), "输出不对: {}", r.stdout);
    assert!(r.stdout.contains("head, tail"), "输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// N5（2 条）：三元/`if` 表达式丢 then 块 → E0423
//
// 宏 `guard` 展开出的 if 语句 then 体为空（守卫语义），`gen_block` 对空块返回
// **空串**，于是整块被吞掉：`if x > 3i64  else {` ⇒ rustc「this `if` expression is
// missing a block after the condition」。修法：空块发 `{\n}`（与原生同形）。
// 同一探针还要求 `panic(..)` 发成宏（`panic!("{:?}", ..)`）。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn n5_empty_then_block_and_panic_macro() {
    let src = r##"#!bin macro

macro guard_check(cond: Tokens) -> Tokens =
    quote("guard " + cond + " else:\n    panic(\"check failed\")")

def main() =
    let x = 5
    @guard_check!(x > 3)
    print(x)
"##;
    let r = run_rs_lz("n5_empty_then", src);
    assert_ok(&r, "N5 空 then 块 + panic 宏");
    assert!(
        r.rust.contains("if x > 3i64 {"),
        "then 块仍缺失（旧缺陷形态 `if c  else {{`）:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("if x > 3i64  else"),
        "then 块为空串被吞掉:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("panic!(\"{:?}\", "),
        "panic 未发成宏（E0423）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("5"), "输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// 核实项（红名单 §4 末尾，未计入 40）：`def main() -> int` 的尾字面量被发成语句
//
// 旧产物 `{ ...; 0i64; }` ⇒ E0308 expected i64, found ()。原生 decl_gen.rs:1060/1516
// 把 typed main 改名 `__lz_main` 并按普通函数发尾 `return`，再补
// `pub fn main() { std::process::exit(__lz_main() as i32); }`。此处同款。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn typed_main_keeps_tail_value() {
    let src = r##"
def add(a: int, b: int) -> int =
    a + b

def main() -> int =
    print(add(1, 2))
    0
"##;
    let r = run_rs_lz("typed_main", src);
    assert_ok(&r, "typed main 尾值");
    assert!(
        r.rust.contains("pub fn __lz_main() -> i64"),
        "typed main 未改名为 __lz_main:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("std::process::exit(__lz_main() as i32)"),
        "缺 main 包装:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("return 0i64;"),
        "尾字面量被当语句（E0308）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("3"), "输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// N6（T0r16）：E0382 —— match 臂内复用「上一句已被按值传走」的非 Copy 参数
//
// 旧产物（p43/p44/base_parser 一族，红名单剩 7 条的唯一 E-code）：
//     let t = tok_at(toks, pos);            // toks 在这里被 move
//     match t { Token.If => {
//         let nt = tok_at(toks, pos + 1);   // ← E0382 use of moved value
//         let blk = parse_block(toks, …);   // ← E0382
// 根因：发射器 `call_arg_src` 只按**实参自身的推断类型**判「非 Copy 才补 clone」
// （`is_heap_irtype(expr_ty(e))`）；而 match 臂 / 嵌套块里的 IR 类型会退化成
// `IrType.Any`（`--emit=ir` 显示 `[?]`），`is_heap_irtype(Any) = false` ⇒ 判据
// 落空，裸实参把变量 move 走。原生 expr_gen.rs 的 Call 实参 clone 口径是
// `!param_is_copy && arg_is_var && !arg_is_copy`——`arg_is_copy` 只认
// Int/F64/Bool，Any 同样按非 Copy 处理。
// 修法：`is_clone_place_ty` 把 `IrType.Any` 并入需 clone 的一档（与原生同口径）。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn e0382_match_arm_reuses_moved_param() {
    let src = r##"
enum Token:
    IntLit(v: int)
    If
    Eof

def tok_at(toks: List<Token>, idx: int) -> Token =
    toks[idx].clone()

def tok_int(t: Token) -> int =
    let res: int = match t:
        case Token.IntLit(v: n) => n
        case _ => -1
    res

def parse_block(toks: List<Token>, pos: int) -> (List<str>, int) =
    let t = tok_at(toks, pos)
    let n = tok_int(t)
    (["n" + n.to_string()], pos + 1)

def parse_stmt(toks: List<Token>, pos: int) -> (str, int) =
    let t = tok_at(toks, pos)
    let res: (str, int) = match t:
        case Token.If =>
            let nt = tok_at(toks, pos + 1)
            let cond = tok_int(nt)
            let blk = parse_block(toks, pos + 2)
            let body = blk.0
            let body_s = body[0]
            let rr: (str, int) = ("if " + cond.to_string() + " {" + body_s + "}", blk.1)
            rr
        case _ =>
            let rr: (str, int) = ("?", pos + 1)
            rr
    res

def main() =
    let toks = [Token.If, Token.IntLit(v: 1), Token.IntLit(v: 7), Token.Eof]
    let r = parse_stmt(toks, 0)
    print(r.0)
"##;
    let r = run_rs_lz("e0382_match_arm", src);
    assert_ok(&r, "N6 match 臂内复用已 move 的参数");
    // 臂内两处 `toks` 实参都必须带 clone（第二处是 E0382 的报错点）
    assert!(
        r.rust.contains("tok_at(toks.clone(), pos + 1i64)"),
        "臂内第一处实参未补 .clone()（E0382）:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("parse_block(toks.clone(), pos + 2i64)"),
        "臂内第二处实参未补 .clone()（E0382）:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("tok_at(toks, "),
        "仍有裸 `toks` 实参（会 move，E0382）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("if 1 {n7}"), "输出不对: {}", r.stdout);
}

/// N6 姊妹面：同一形态作用在**元组字段实参**上（base_parser 的
/// `keyword_token(r.0)` / `Token.Ident(name: r.0)`）。`p.0` 在臂内同样退化为
/// `?`，旧产物 `probe(p.0)` 把字段 move 走，下一句 `let tail = p.0` 即 E0382。
#[test]
fn e0382_match_arm_reuses_moved_field() {
    let src = r##"
enum T:
    A
    B

def probe(s: str) -> bool =
    s == "a"

def scan(p: (str, int)) -> str =
    let t = T.A
    let res: str = match t:
        case T.A =>
            let ok = probe(p.0)
            let tail = p.0
            if ok:
                tail
            else:
                "no"
        case _ =>
            "?"
    res

def main() =
    print(scan(("a", 1)))
"##;
    let r = run_rs_lz("e0382_field", src);
    assert_ok(&r, "N6 match 臂内复用已 move 的元组字段");
    assert!(
        r.rust.contains("probe(p.0.clone())"),
        "元组字段实参未补 .clone()（E0382）:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("probe(p.0)"),
        "仍有裸 `p.0` 实参（会 move，E0382）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains('a'), "输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// N7（T0r20）：循环体内的「重赋」被发成 shadow-let ⇒ 循环变量永不推进（运行不终止）
//
// 实测（p43_nested_block.lz / p44_nested_fixed.lz）：native 正常退出并打印，
// rs-lz 产物能编过但 `timeout 30` 内不退出（exit 124）。
// 根因不在循环发射，而在 `Stmt.Let` 的两种来源在 IR 里**同形**：
//   `let mut i = 0`（真声明）与 `i = r.1`（LZ 无 `let` 的重赋）都是 Stmt.Let，
//   后者只有 is_mut=true 可用，且 value 不含自身名字（BUG-454 的自引用启发不覆盖），
//   于是照直发 `let mut i = r.1;` —— Rust 的 shadow-let，外层 i 永不推进。
// 修法：发射器自持作用域表（decl_has / decl_add / decl_after_stmt，见
// lz_codegen_lib.lz T0r20 段），名字已在词法作用域内 + is_mut=true ⇒ 发 `i = <expr>;`。
// 本用例同时守住「真·遮蔽不误伤」：显式 `let y = 2` 必须仍是 let（否则 E0384）。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn loop_reassign_emits_assignment_not_shadow() {
    let src = r##"
def step(p: int) -> (int, int) =
    (p, p + 1)

def sum_loop(n: int) -> int =
    let mut i = 0
    let mut total = 0
    while i < n:
        total = total + i
        let r = step(i)
        i = r.1
    total

def main() =
    print(sum_loop(4))
    let x = 1
    let x = x + 10
    print(x)
    let y = 1
    let y = 2
    print(y)
"##;
    let r = run_rs_lz("n7_loop_reassign", src);
    assert_ok(&r, "N7 循环内重赋不遮蔽");
    // ① 重赋必须发成赋值（旧缺陷形态是 `let mut i = r.1;`）
    assert!(
        r.rust.contains("i = r.1;"),
        "循环变量重赋未发成赋值（仍是 shadow-let ⇒ 死循环）:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("let mut i = r.1"),
        "仍是 `let mut i = r.1`（外层 i 永不推进 ⇒ 死循环）:\n{}",
        r.rust
    );
    // ② 真·遮蔽（显式 let 重复声明）必须仍发 let，不得被改成对不可变变量的赋值
    //    （形态为 `let y: i64 = 2i64;`，故只断言「y 的 let 出现两次」而非字面量文本）
    assert!(
        r.rust.matches("let y").count() >= 2,
        "真·遮蔽被误改成赋值（显式 let 应保持 let）:\n{}",
        r.rust
    );
    // ③ 运行必须在 output_with_timeout 的 30s 上限内正常退出，且输出与 native 同口径
    assert!(r.run_ok, "生成产物未正常退出（超时/死循环）:\n{}", r.diag);
    assert!(r.stdout.contains('6'), "sum_loop(4) 输出不对: {}", r.stdout);
    assert!(r.stdout.contains("11"), "`let x = x + 10` 输出不对: {}", r.stdout);
    assert!(r.stdout.contains('2'), "真·遮蔽 `let y = 2` 输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// G5（红名单 r3 剩余 2 条）：字符串**单字符**下标 `s[i]`
//
// `str`/`String` 在 Rust 只实现 Range 索引，`s[i]` 直接 E0277（the type `str`
// cannot be indexed by `usize`）。原生 expr_gen.rs:5665-5683 发「先 chars().collect()
// 再按字符下标取」的块：字符串语境取 `.to_string()`，数值语境取码点 `as i64`。
// 修法：`gen_expr` 的 IndexGet 在「str 基底 + 非 Range 键」时发同一块；
// `Expr.Cast` 的数值分支对 str 下标改发码点形态（旧实现会先 to_string 再 parse）。
// 覆盖样本：`div-tools/probes/p22_str_index.lz`（p12b_single 见 G6）。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn g5_str_single_char_index_uses_char_table() {
    let src = r##"
def char_at(s: str, i: int) -> str =
    s[i]

def code_at(s: str, i: int) -> int =
    int(s[i])

def main() =
    print(char_at("abc", 1))
    print(code_at("abc", 1))
"##;
    let r = run_rs_lz("g5_char_index", src);
    assert_ok(&r, "G5 字符串单字符下标");
    assert!(
        r.rust.contains("let __cs: Vec<char> = "),
        "字符串下标未走 chars 收集块（仍会 E0277 str cannot be indexed by usize）:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("__cs[__i].to_string()"),
        "字符串语境未取 to_string 形态:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("__cs[__i] as i64"),
        "数值语境未取码点形态（旧实现是 to_string 后 parse）:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("s[i64 as usize]"),
        "仍是裸 `s[i as usize]`（E0277）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains('b'), "char_at 输出不对: {}", r.stdout);
    assert!(r.stdout.contains("98"), "code_at 输出不对: {}", r.stdout);
}

// ════════════════════════════════════════════════════════════════════════════
// G6（2 条）：tuple / typepack 的索引与字段访问
//
// 三个叠加缺陷：
//   ① `t[0]` 发成 `t[0i64 as usize]` ⇒ E0608 cannot index into `(A, B)`
//      （原生 expr_gen.rs:5554-5596 无条件发 `.0` 字段访问）；
//   ② 单元素元组类型发成 `(String)` —— Rust 里那只是加括号的 String，
//      故 `.0` 报 E0609；必须 `(String,)`；
//   ③ 变参形参 `..: Tuple<...>` 的签名与调用点未打包 ⇒ E0061
//      （定长前缀形态 → `(T1, T2, Vec<Box<dyn Any>>)`；typepack 形态按调用点
//       具体化成 `(T1, T2, T3)`）。
// 覆盖样本：`p12_typepack.lz`、`p12c_pack.lz`、`p12b_single.lz`。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn g6_tuple_index_and_typepack_packing() {
    let src = r##"
def log(..: Tuple<str, int, ..>) =
    print(args[0])
    print(args[1])

def show<Ts...>(..: Tuple<Ts...>) =
    print(args.0, args.1)

def main() =
    log("hello", 42, 3.14, 1)
    show(1, "a", True)
"##;
    let r = run_rs_lz("g6_typepack", src);
    assert_ok(&r, "G6 元组下标 / typepack 打包");
    // ① 元组下标 → `.0` / `.1`
    assert!(
        r.rust.contains("args.0") && r.rust.contains("args.1"),
        "元组下标未发成字段访问（E0608）:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("args[0i64 as usize]"),
        "元组下标仍是 `args[0]`（E0608）:\n{}",
        r.rust
    );
    // ② 定长前缀形态的变参签名 + 调用点打包
    assert!(
        r.rust.contains("Vec<Box<dyn Any>>"),
        "定长前缀变参签名未补尾部收集器:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("vec![Box::new("),
        "调用点未把多余实参 Box 打包:\n{}",
        r.rust
    );
    // ③ typepack 具体化：泛型 Ts 被剔除，签名是具体异质元组
    assert!(
        r.rust.contains("pub fn show(mut args: (i64, String, bool))"),
        "typepack 未按调用点具体化:\n{}",
        r.rust
    );
    assert!(r.stdout.contains("hello"), "log 输出不对: {}", r.stdout);
    assert!(r.stdout.contains("42"), "log 第二字段输出不对: {}", r.stdout);
    assert!(
        r.stdout.contains('1') && r.stdout.contains('a'),
        "show 输出不对: {}",
        r.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// G7（2 条）：模块 import 未展开 / 外部符号未声明
//
//   ① `import moda as m`：CLI 已把模块项平铺进同一 IrModule，原生 imported_modules
//      （codegen/mod.rs:1335-1342）把 `m.add(..)` 直接渲染成 `add(..)`，且不给用户
//      模块发 `use`（decl_gen.rs:2809-2960 只对已知 std 路径发）。rs-lz 旧实现
//      无条件发 `use moda;` ⇒ E0432 unresolved import `moda`。
//   ② 外部函数（.lzi 签名未加载）：原生 emit.rs:396-416 发 `-> i64 { i64::MAX }` 桩
//      兜底；rs-lz 旧实现裸调用 ⇒ E0425 cannot find function。
// 覆盖样本：`p5_alias.lz`、`p16_lzi.lz`。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn g7_module_alias_call_and_extern_stub() {
    // `import moda as m` 需要兄弟模块文件才能解析（CLI 按输入文件所在目录找）
    let src = r##"
import moda as m
def main() =
    let r = m.add(2, 3)
    print(r)
    let x = external_add(3, 4)
    print(x)
"##;
    let moda = r##"
def add(a: int, b: int) -> int =
    a + b
"##;
    let r = run_rs_lz_files("g7_alias_extern", &[("input.lz", src), ("moda.lz", moda)]);
    assert_ok(&r, "G7 模块别名调用 / 外部符号桩");
    // ① 别名命名空间调用降级为普通调用
    assert!(
        r.rust.contains("add(2i64, 3i64)"),
        "`m.add(2, 3)` 未降级成 `add(2, 3)`:\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("m.add("),
        "别名前缀仍透传（E0425 cannot find value `m`）:\n{}",
        r.rust
    );
    // ② 未定义外部函数发 i64 桩
    assert!(
        r.rust.contains("fn external_add(__a0: i64, __a1: i64) -> i64 { i64::MAX }"),
        "未定义外部函数未发兜底桩（E0425）:\n{}",
        r.rust
    );
    assert!(r.stdout.contains('5'), "add 输出不对: {}", r.stdout);
    assert!(
        r.stdout.contains("9223372036854775807"),
        "外部桩返回值不对: {}",
        r.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// G8（4 条）：杂项单例 —— 插值未绑定名 / `raise` / `.slice()` / `yield` 生成器
//
//   ① `f"hello {name}"` 引外部符号：原生 scan.rs:1245 预扫「未绑定插值名」并发空串，
//      旧实现直发 `name` ⇒ E0425 cannot find value `name`。
//   ② `raise x`（无异常签名）：Rust 的 panic! 首参必须是格式串字面量，旧实现发
//      `panic!(x)` ⇒ `expected `,`, found `.``。原生形态 `panic!("{:?}", x)`。
//   ③ `src.slice(a, b)`：String/Vec 都没有 slice 方法，原生映射 lz_slice（E0599）。
//   ④ `iterator` + `yield`：原生把函数改造成 Vec 收集器（`__gen_vec` 声明 +
//      `return __gen_vec;`，返回类型 Vec<T>）；旧实现只发 push，`__gen_vec` 未声明
//      ⇒ E0425 cannot find value `__gen_vec`。
// 覆盖样本：`lit_probe.lz`、`neg_error_uncaught_panic_ok.lz`、
//           `p24_slice_method.lz`、`p8_yield.lz`。
// ════════════════════════════════════════════════════════════════════════════
#[test]
fn g8_fstring_unbound_raise_slice_yield() {
    let src = r##"
iterator counter(n: int) =
    let mut i = 0
    while i < n:
        yield i
        i = i + 1

def boom() =
    raise "boom"

def main() =
    for x in counter(3):
        print(x)
    let src = "def add"
    let w = src.slice(0, 3)
    print(w)
    let fs = f"hello {name}"
    print(fs)
"##;
    let r = run_rs_lz("g8_misc", src);
    assert_ok(&r, "G8 插值/raise/slice/yield");
    // ① 未绑定插值名 → 空串
    assert!(
        r.rust.contains("format!(\"hello {:?}\", \"\")"),
        "未绑定 f-string 插值名未替换为空串（E0425）:\n{}",
        r.rust
    );
    // ② raise → panic!("{:?}", ..)
    assert!(
        r.rust.contains("panic!(\"{:?}\", "),
        "raise 未发成 `panic!(\"{{:?}}\", v)`（语法错）:\n{}",
        r.rust
    );
    // ③ slice → lz_slice
    assert!(
        r.rust.contains(".lz_slice(0i64, 3i64)"),
        "`.slice(a, b)` 未映射 lz_slice（E0599）:\n{}",
        r.rust
    );
    // ④ yield 生成器：Vec 返回 + __gen_vec 声明/回收
    assert!(
        r.rust.contains("let mut __gen_vec = Vec::new();"),
        "生成器缺 __gen_vec 声明（E0425）:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("return __gen_vec;"),
        "生成器缺 `return __gen_vec;`:\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("pub fn counter(n: i64) -> Vec<i64>"),
        "生成器返回类型未改成 Vec<T>:\n{}",
        r.rust
    );
    assert!(r.stdout.contains('0'), "yield 输出不对: {}", r.stdout);
    assert!(r.stdout.contains('2'), "yield 输出不对: {}", r.stdout);
    assert!(r.stdout.contains("def"), "slice 输出不对: {}", r.stdout);
    assert!(r.stdout.contains("hello"), "f-string 输出不对: {}", r.stdout);
}
