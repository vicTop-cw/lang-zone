// Lang-Zone 自举回归测试：LZ 写的 IR display 能否经自家工具链编译运行
//
// 验证链路：内嵌 LZ 源码（递归 enum IrType + display_type）→ lang-zone 编译
// → rustc 编译 → 运行 → 断言输出与 Rust 版 display.rs 格式一致。
// 这是自举路线 B（LZ 翻译编译器核心）的固化回归：bootstrap/work/lz_ir
// 试点若回归，此测试即失败。

use std::path::PathBuf;
use std::process::Command;
use std::fs;

// 精简 LZ 源：递归 enum IrType + display_type（对齐 src/ir/display.rs）
// 覆盖递归 enum 自引用（Box<Vec<IrType>>）、[ty] 前缀、fn/option 容器
const LZ_SOURCE: &str = r#"
enum IrType:
    Int
    Str
    Named(path: str, args: List<IrType>)
    Opt(inner: IrType)

def display_type(t: IrType) -> str =
    match t:
        case IrType.Int => "int"
        case IrType.Str => "str"
        case IrType.Named(path: p, args: a) =>
            p + ("<" + type_list(a) + ">" if a.len() > 0 else "")
        case IrType.Opt(inner: x) => "Option<" + display_type(x) + ">"

def type_list(ts: List<IrType>) -> str =
    if ts.len() == 0:
        ""
    else:
        let head = display_type(ts[0])
        if ts.len() > 1:
            head + ", " + type_list(tail_t(ts))
        else:
            head

def tail_t(ts: List<IrType>) -> List<IrType> =
    let mut out: List<IrType> = []
    for idx in 1..ts.len():
        out = out + [ts[idx]]
    out

def main() =
    // Vec<int>
    let v1 = IrType.Named(path: "Vec", args: [IrType.Int])
    print(display_type(v1))
    // Option<Vec<int>>（递归嵌套）
    let v2 = IrType.Named(path: "Vec", args: [IrType.Int])
    let ov = IrType.Opt(inner: v2)
    print(display_type(ov))
"#;

#[test]
fn lz_written_ir_display_compiles_and_runs() {
    let work = std::env::temp_dir().join("lz_ir_regression");
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join("ir_types.lz");
    let rs_path = work.join("ir_types.rs");
    let exe_path = work.join("ir_types.exe");
    fs::write(&lz_path, LZ_SOURCE).expect("write lz source");

    // 1. lang-zone 编译 .lz → .rs
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz_path)
        .output()
        .expect("run lang-zone");
    assert!(
        out.status.success(),
        "lang-zone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(rs_path.exists(), "generated .rs missing");

    // 2. rustc 编译 .rs（链接 lz_builtins）
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    let rustc_out = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs_path)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-L")
        .arg(format!("dependency={}", builtins.parent().unwrap().join("deps").display()))
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("run rustc");
    assert!(
        rustc_out.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&rustc_out.stderr)
    );

    // 3. 运行并断言输出（与 Rust 版 display.rs 格式一致）
    let run_out = Command::new(&exe_path).output().expect("run exe");
    assert!(run_out.status.success(), "generated exe failed");
    let stdout = String::from_utf8_lossy(&run_out.stdout);
    let first: Vec<&str> = stdout.lines().collect();
    assert!(
        first.len() >= 2,
        "expected >=2 lines, got: {}",
        stdout
    );
    // Vec<int>（Named 无泛型括号展开）
    assert!(
        stdout.contains("Vec<int>"),
        "expected Vec<int>, got: {}",
        stdout
    );
    // Option<Vec<int>>（递归嵌套容器）
    assert!(
        stdout.contains("Option<Vec<int>>"),
        "expected Option<Vec<int>>, got: {}",
        stdout
    );

    // 清理
    let _ = fs::remove_dir_all(&work);
}

/// 自举回归门禁（词法试点）：LZ 写的 Lexer（Token 枚举 + tokenize 简化版）
/// 经 lang-zone → rustc → 运行 端到端输出正确 token 序列。
/// 试点源：bootstrap/work/lz_lexer/lexer.lz（bootstrap/ 被 gitignore 忽略，
/// 故测试内嵌等价精简源——覆盖字符串字面量/多字符运算符/magic method）。
const LZ_LEXER_SOURCE: &str = r#"
enum Token:
    Def
    If
    Return
    StrLit(s: str)
    Ident(name: str)
    MagicMethod(name: str)
    Plus
    EqEq
    AmpAmp
    Dot
    LParen
    RParen
    Newline
    Eof

def is_keyword(s: str) -> bool =
    s == "def" || s == "if" || s == "return"

def keyword_token(s: str) -> Token =
    if s == "def":
        Token.Def
    elif s == "if":
        Token.If
    else:
        Token.Return

def is_digit(c: str) -> bool =
    c >= "0" and c <= "9"

def is_alpha(c: str) -> bool =
    (c >= "a" and c <= "z") or (c >= "A" and c <= "Z") or c == "_"

def is_ident_char(c: str) -> bool =
    is_alpha(c) or is_digit(c)

def char_at(s: str, idx: int) -> str =
    s[idx..(idx + 1)]

def str_len(s: str) -> int =
    s.len()

def scan_ident(src: str, start: int) -> (str, int) =
    let mut i = start
    while i < str_len(src) && is_ident_char(char_at(src, i)):
        i = i + 1
    (src[start..i], i)

def punct_token(c: str) -> (bool, Token) =
    let table: List<(str, Token)> = [
        ("+", Token.Plus),
        (".", Token.Dot),
        ("(", Token.LParen),
        (")", Token.RParen),
    ]
    let mut result: (bool, Token) = (false, Token.Plus)
    for idx in 0..table.len():
        let pair = table[idx].clone()
        if pair.0 == c:
            result = (true, pair.1)
    result

def two_char_token(c1: str, c2: str) -> (bool, Token) =
    let key: str = c1 + c2
    let table: List<(str, Token)> = [
        ("==", Token.EqEq),
        ("&&", Token.AmpAmp),
    ]
    let mut result: (bool, Token) = (false, Token.Plus)
    for idx in 0..table.len():
        let pair = table[idx].clone()
        if pair.0 == key:
            result = (true, pair.1)
    result

def scan_punct(src: str, i: int) -> ((bool, Token), int) =
    let c = char_at(src, i)
    let r = punct_token(c)
    if r.0:
        (r, i + 1)
    elif i + 1 < str_len(src):
        let two = two_char_token(c, char_at(src, i + 1))
        if two.0:
            ((true, two.1), i + 2)
        else:
            (r, i + 1)
    else:
        (r, i + 1)

def scan_string(src: str, start: int) -> (str, int) =
    let mut i = start + 1
    while i < str_len(src) && char_at(src, i) != "\"":
        i = i + 1
    (src[(start + 1)..i], i + 1)

def tokenize(src: str) -> List<Token> =
    let mut tokens: List<Token> = []
    let mut i = 0
    while i < str_len(src):
        let c = char_at(src, i)
        if c == " " || c == "\t":
            i = i + 1
        elif c == "\n":
            tokens = tokens + [Token.Newline]
            i = i + 1
        elif c == "\"":
            let r = scan_string(src, i)
            i = r.1
            tokens = tokens + [Token.StrLit(s: r.0)]
        elif is_alpha(c):
            let r = scan_ident(src, i)
            i = r.1
            let word: str = r.0
            if word.len() > 4 && word[0..2] == "__" && word[(word.len() - 2)..word.len()] == "__":
                tokens = tokens + [Token.MagicMethod(name: word)]
            elif is_keyword(word):
                tokens = tokens + [keyword_token(word)]
            else:
                tokens = tokens + [Token.Ident(name: word)]
        else:
            let r = scan_punct(src, i)
            i = r.1
            let pr = r.0
            if pr.0:
                tokens = tokens + [pr.1]
    tokens = tokens + [Token.Eof]
    tokens

def main() =
    let src = "def f:\n    x.__len__()\n    \"hi\" + y\n    a == b && c"
    let toks = tokenize(src)
    for idx in 0..toks.len():
        let t = toks[idx].clone()
        match t:
            case Token.Def => print("Def")
            case Token.If => print("If")
            case Token.Return => print("Return")
            case Token.StrLit(s: s) => print("StrLit(" + s + ")")
            case Token.Ident(name: n) => print("Ident(" + n + ")")
            case Token.MagicMethod(name: n) => print("MagicMethod(" + n + ")")
            case Token.Plus => print("Plus")
            case Token.EqEq => print("EqEq")
            case Token.AmpAmp => print("AmpAmp")
            case Token.Dot => print("Dot")
            case Token.LParen => print("LParen")
            case Token.RParen => print("RParen")
            case Token.Newline => print("Newline")
            case Token.Eof => print("Eof")
"#;

#[test]
fn lz_written_lexer_compiles_and_runs() {
    let work = std::env::temp_dir().join("lz_lexer_gate");
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join("lexer.lz");
    let rs_path = work.join("lexer.rs");
    let exe_path = work.join("lexer.exe");
    fs::write(&lz_path, LZ_LEXER_SOURCE).expect("write lz source");

    // 1. lang-zone 编译 .lz → .rs
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz_path).output().expect("run lang-zone");
    assert!(
        out.status.success(),
        "lang-zone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(rs_path.exists(), "generated .rs missing");

    // 2. rustc 编译
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    let rustc_out = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs_path)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-L")
        .arg(format!("dependency={}", builtins.parent().unwrap().join("deps").display()))
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("run rustc");
    assert!(
        rustc_out.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&rustc_out.stderr)
    );

    // 3. 运行并断言 token 序列（字符串字面量/magic method/双字符运算符）
    let run_out = Command::new(&exe_path).output().expect("run exe");
    assert!(run_out.status.success(), "generated exe failed");
    let stdout = String::from_utf8_lossy(&run_out.stdout);
    assert!(stdout.contains("Def"), "缺 Def: {}", stdout);
    assert!(stdout.contains("MagicMethod(__len__)"), "缺 magic method: {}", stdout);
    assert!(stdout.contains("StrLit(hi)"), "缺字符串字面量: {}", stdout);
    assert!(stdout.contains("EqEq") && stdout.contains("AmpAmp"), "缺双字符运算符: {}", stdout);

    let _ = fs::remove_dir_all(&work);
}

/// 自举回归门禁（增强）：lzc --emit=ir-lz 端到端。
#[test]
fn lz_emit_ir_lz_roundtrip() {
    let work = std::env::temp_dir().join("lz_ir_lz_gate");
    fs::create_dir_all(&work).expect("create work dir");
    // 复制一个基础 DEMO 到临时目录（避免 --emit=ir-lz 在 DEMO 目录落盘）
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("DEMO/04_functions/basic.lz");
    let lz_path = work.join("basic.lz");
    fs::copy(&src, &lz_path).expect("copy demo");
    let out_path = work.join("basic.lzlz");
    let rs_path = work.join("basic.rs");
    let exe_path = work.join("basic.exe");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz_path)
        .arg("--emit=ir-lz")
        .output()
        .expect("run lang-zone --emit=ir-lz");
    assert!(
        out.status.success(),
        "--emit=ir-lz 失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out_path.exists(), "生成 .lzlz 缺失");
    assert!(rs_path.exists(), "中间 .rs 缺失");
    assert!(exe_path.exists(), "最终 exe 缺失");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // IR 文本输出（Debug 字符串：带引号与 \n 转义；断言关键子串）
    assert!(
        stdout.contains("LZIR v1"),
        "输出应含 LZIR v1 头，got: {}",
        stdout
    );
    assert!(
        stdout.contains("fn add") && stdout.contains("binop"),
        "输出应含 fn add / binop，got: {}",
        stdout
    );
    assert!(
        stdout.contains("items"),
        "输出应含 items 计数，got: {}",
        stdout
    );

    let _ = fs::remove_dir_all(&work);
}

/// 自举 C3 门禁（50% 里程碑）：--emit=ir vs --emit=ir-lz 双路输出逐字符一致。
///
/// Rust 版 display.rs（--emit=ir）与 LZ 版 lz_ir_lib.lz（--emit=ir-lz，经
/// 递归管线 lzc→rustc→run）对同一输入产出完全相同 IR 文本。若 lz_ir_lib.lz
/// 或 lz_codegen.rs 回归（ty 前缀 / 缩进式 body / 泛型签名等丢失），本测试失败。
#[test]
fn lz_emit_ir_lz_matches_ir_byte_exact() {
    // 覆盖：函数定义（含泛型）、const、let/赋值、binop/unop、call/index/field、
    // if/else、for/while、match、return、struct 定义、enum 定义、列表/字典字面量
    let source = r#"
def add(x: int, y: int) -> int =
    x + y

const LIMIT: int = 100

struct Pair<T> =
    first: T
    second: T

enum Shape:
    Circle(r: f64)
    Rect(w: f64, h: f64)

def classify(x: int) -> str =
    match x:
        case 0 => "zero"
        case _ => "many"

def sum_to(n: int) -> int =
    let mut total = 0
    for i in 0..n:
        total = total + i
    while total > 100:
        total = total - 1
    total

def area(s: Shape) -> f64 =
    match s:
        case Shape.Circle(r: r) => 3.14 * r * r
        case Shape.Rect(w: w, h: h) => w * h

def main() =
    let xs: List<int> = [1, 2]
    let d = {"a": 1}
    print(xs[0])
    print(add(LIMIT, 1))
    print(classify(0))
    print(sum_to(5))
    let c = Shape.Circle(r: 3.0)
    print(area(c))
"#;

    let work = std::env::temp_dir().join("lz_ir_diff_gate");
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join("diff_input.lz");
    fs::write(&lz_path, source).expect("write lz source");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));

    // 路 1：Rust 版 display（--emit=ir）
    let out_ir = Command::new(&bin)
        .arg(&lz_path)
        .arg("--emit=ir")
        .output()
        .expect("run lang-zone --emit=ir");
    assert!(
        out_ir.status.success(),
        "--emit=ir 失败: {}",
        String::from_utf8_lossy(&out_ir.stderr)
    );

    // 路 2：LZ 版 display（--emit=ir-lz，递归管线）
    let out_lz = Command::new(&bin)
        .arg(&lz_path)
        .arg("--emit=ir-lz")
        .output()
        .expect("run lang-zone --emit=ir-lz");
    assert!(
        out_lz.status.success(),
        "--emit=ir-lz 失败: {}",
        String::from_utf8_lossy(&out_lz.stderr)
    );

    // 逐字符一致断言（C3 判据）
    assert_eq!(
        out_ir.stdout, out_lz.stdout,
        "--emit=ir 与 --emit=ir-lz 输出不一致（LZ 版 display 回归？）\nir: {}\nlz: {}",
        String::from_utf8_lossy(&out_ir.stdout),
        String::from_utf8_lossy(&out_lz.stdout)
    );
    assert!(!out_ir.stdout.is_empty(), "IR 输出不应为空");

    let _ = fs::remove_dir_all(&work);
}

/// 自举 D1 门禁（100% 里程碑）：--emit=rs-lz 双路 .rs 产物逐字符一致。
///
/// Rust 版 codegen（常规路径生成 .rs）与 LZ 版 codegen（--emit=rs-lz，经递归
/// 管线 lzc→rustc→run 输出 Rust 源码）对同一输入产出完全相同的 .rs。若
/// lz_codegen_lib.lz 或 lz_codegen.rs 的 rs-lz 生成回归，本测试失败。
#[test]
fn lz_emit_rs_lz_matches_native_codegen() {
    let source = r#"
def add(x: int, y: int) -> int =
    x + y

const LIMIT: int = 100

struct Point =
    x: f64
    y: f64

enum Color:
    Red
    Blue

def main() =
    let a = 1 + 2
    let b = 3.5 * 2.0
    let s = "hi"
    let neg = -a
    let c = add(a, 4)
    print(c)
"#;

    let work = std::env::temp_dir().join("lz_rs_diff_gate");
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join("d1_input.lz");
    fs::write(&lz_path, source).expect("write lz source");
    let rs_path = work.join("d1_input.rs");
    let _ = fs::remove_file(&rs_path);

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));

    // 路 1：常规路径生成 .rs（Rust 版 codegen）
    let out_native = Command::new(&bin)
        .arg(&lz_path)
        .output()
        .expect("run lang-zone (native codegen)");
    assert!(
        out_native.status.success(),
        "常规 codegen 失败: {}",
        String::from_utf8_lossy(&out_native.stderr)
    );
    let native_rs = fs::read(&rs_path).expect("read native .rs");

    // 路 2：--emit=rs-lz（LZ 版 codegen，递归管线）
    let out_lz = Command::new(&bin)
        .arg(&lz_path)
        .arg("--emit=rs-lz")
        .output()
        .expect("run lang-zone --emit=rs-lz");
    assert!(
        out_lz.status.success(),
        "--emit=rs-lz 失败: {}",
        String::from_utf8_lossy(&out_lz.stderr)
    );

    // 逐字符一致断言（D1 判据）
    assert_eq!(
        native_rs, out_lz.stdout,
        "常规 codegen 与 --emit=rs-lz 产物不一致（LZ 版 codegen 回归？）\nnative: {}\nlz: {}",
        String::from_utf8_lossy(&native_rs),
        String::from_utf8_lossy(&out_lz.stdout)
    );
    assert!(!out_lz.stdout.is_empty(), ".rs 输出不应为空");

    // 生成的 Rust 必须能过 rustc（运行行为一致由 D2 全量比对覆盖）
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    let exe_path = work.join("d1_input.exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs_path)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-L")
        .arg(format!("dependency={}", builtins.parent().unwrap().join("deps").display()))
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("run rustc on generated .rs");
    assert!(
        rc.status.success(),
        "LZ 版 codegen 产物 rustc 编译失败: {}",
        String::from_utf8_lossy(&rc.stderr)
    );
    let run = Command::new(&exe_path).output().expect("run generated exe");
    assert!(run.status.success(), "生成产物运行失败");

    let _ = fs::remove_dir_all(&work);
}

// BUG-469 回归：rs-lz 的 Stmt::Match scrutinee 未对齐原生 stmt_gen.rs:2304 的
// 「纯 Var 一律 `.clone()`」规则，导致 `match s` 而非 `match s.clone()`——非 Copy
// 枚举实参匹配后可能 E0382 二次移动。断言 rs-lz 产物含 `.clone()` scrutinee 且能
// 过 rustc。（注：rs-lz 的 match 臂缩进仍与原生有差异，故不做逐字符比对，仅断言
// scrutinee 变换这一功能修复点。）
#[test]
fn rs_lz_match_scrutinee_clones_var() {
    let source = r#"
enum Shape =
    Circle(radius: int)
    Rect(w: int, h: int)

def area(s: Shape) -> int =
    match s:
        case Shape.Circle(radius): return radius * radius * 3
        case Shape.Rect(w, h): return w * h

def main() =
    print(area(Shape.Circle(radius: 3)))
    print(area(Shape.Rect(w: 3, h: 4)))
"#;

    let work = std::env::temp_dir().join("lz_rs_scrut_gate");
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join("g1_input.lz");
    fs::write(&lz_path, source).expect("write lz source");
    let rs_path = work.join("g1_input.rs");
    let _ = fs::remove_file(&rs_path);

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));

    let out_native = Command::new(&bin)
        .arg(&lz_path)
        .output()
        .expect("run lang-zone (native codegen)");
    assert!(
        out_native.status.success(),
        "常规 codegen 失败: {}",
        String::from_utf8_lossy(&out_native.stderr)
    );
    let native_rs = fs::read(&rs_path).expect("read native .rs");
    let _ = fs::remove_file(&rs_path);

    let out_lz = Command::new(&bin)
        .arg(&lz_path)
        .arg("--emit=rs-lz")
        .output()
        .expect("run lang-zone --emit=rs-lz");
    assert!(
        out_lz.status.success(),
        "--emit=rs-lz 失败: {}",
        String::from_utf8_lossy(&out_lz.stderr)
    );

    // BUG-469 核心：原生与 rs-lz 的 scrutinee 都必须带 .clone()（match 纯 Var 实参）
    let n_str = String::from_utf8_lossy(&native_rs);
    let l_str = String::from_utf8_lossy(&out_lz.stdout);
    assert!(
        n_str.contains("match s.clone()"),
        "参考实现未发 `match s.clone()`（探针失效？）: {}",
        n_str.lines().find(|l| l.contains("match")).unwrap_or("(无 match)")
    );
    assert!(
        l_str.contains("match s.clone()"),
        "rs-lz 未对齐原生 scrutinee clone（BUG-469 回归）：\n{}",
        l_str.lines().find(|l| l.contains("match")).unwrap_or("(无 match)")
    );

    // 生成产物必须过 rustc（功能正确性兜底）
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    let out_rs = work.join("g1_input_lz.rs");
    fs::write(&out_rs, &out_lz.stdout).expect("write rs-lz .rs");
    let exe_path = work.join("g1_input.exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&out_rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-L")
        .arg(format!("dependency={}", builtins.parent().unwrap().join("deps").display()))
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("run rustc on rs-lz .rs");
    assert!(
        rc.status.success(),
        "rs-lz 产物 rustc 编译失败: {}",
        String::from_utf8_lossy(&rc.stderr)
    );

    let _ = fs::remove_dir_all(&work);
}

// BUG-470 回归：rs-lz 的 Stmt::Match scrutinee 未对齐原生 stmt_gen.rs:2274 的
// Str → `.as_str()` 变换。String scrutinee 直接 match &str 字面量模式报 E0308。
// 断言 rs-lz 产物含 `match name.as_str() {` 且 rustc 可编译运行出正确结果。
#[test]
fn rs_lz_match_scrutinee_as_str() {
    let source = r#"
def kind(name: str) -> str =
    match name:
        case "hello": return "greet"
        case _: return "other"

def main() =
    print(kind("hello"))
    print(kind("zzz"))
"#;

    let work = std::env::temp_dir().join("lz_rs_scrut_str");
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join("s1_input.lz");
    fs::write(&lz_path, source).expect("write lz source");
    let rs_path = work.join("s1_input.rs");
    let _ = fs::remove_file(&rs_path);

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));

    let out_native = Command::new(&bin)
        .arg(&lz_path)
        .output()
        .expect("run lang-zone (native codegen)");
    assert!(
        out_native.status.success(),
        "常规 codegen 失败: {}",
        String::from_utf8_lossy(&out_native.stderr)
    );
    let native_rs = fs::read(&rs_path).expect("read native .rs");
    let _ = fs::remove_file(&rs_path);

    let out_lz = Command::new(&bin)
        .arg(&lz_path)
        .arg("--emit=rs-lz")
        .output()
        .expect("run lang-zone --emit=rs-lz");
    assert!(
        out_lz.status.success(),
        "--emit=rs-lz 失败: {}",
        String::from_utf8_lossy(&out_lz.stderr)
    );

    let n_str = String::from_utf8_lossy(&native_rs);
    let l_str = String::from_utf8_lossy(&out_lz.stdout);
    assert!(
        n_str.contains("match name.as_str()"),
        "参考实现未发 `match name.as_str()`（探针失效？）"
    );
    assert!(
        l_str.contains("match name.as_str()"),
        "rs-lz 未对齐原生 scrutinee as_str（BUG-470 回归）：\n{}",
        l_str.lines().find(|l| l.contains("match")).unwrap_or("(无 match)")
    );

    // rs-lz 产物须过 rustc 且运行输出正确
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    let out_rs = work.join("s1_input_lz.rs");
    fs::write(&out_rs, &out_lz.stdout).expect("write rs-lz .rs");
    let exe_path = work.join("s1_input.exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&out_rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-L")
        .arg(format!("dependency={}", builtins.parent().unwrap().join("deps").display()))
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("run rustc on rs-lz .rs");
    assert!(
        rc.status.success(),
        "rs-lz 产物 rustc 编译失败: {}",
        String::from_utf8_lossy(&rc.stderr)
    );
    let run = Command::new(&exe_path).output().expect("run generated exe");
    assert!(run.status.success(), "rs-lz 产物运行失败");
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "greet\nother",
        "rs-lz 产物运行输出与预期不符"
    );

    let _ = fs::remove_dir_all(&work);
}

// BUG-466 回归：LZ 侧 IrType/Item 枚举是 Rust 侧（types.rs / node.rs）的子集，
// 新增变体未镜像会令 --emit=ir-lz 生成的 Rust 编译失败（rustc E0599，如
// "no variant named Ext/EmbedBlock found for enum"）。此处静态断言确保
// Ext / EmbedBlock 变体及其 display 分支始终存在，且格式串与原生 display.rs 一致。
#[test]
fn lz_ir_lib_mirrors_ext_and_embedblock() {
    let lib = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ir/lz_ir_lib.lz"),
    )
    .expect("read lz_ir_lib.lz");

    // IrType::Ext：types.rs 有 Ext，lz_codegen.rs:961 会发射 `IrType.Ext`
    assert!(
        lib.lines().any(|l| l.trim_end() == "    Ext"),
        "lz_ir_lib.lz 缺 IrType.Ext 变体 (types.rs 有 Ext, lz_codegen.rs 会发射 IrType.Ext)"
    );
    assert!(
        lib.contains("case IrType.Ext => \"Ext\".to_string()"),
        "IrType.Ext 显示分支缺失或格式漂移 (须对齐 display.rs:23 IrType::Ext 输出 Ext)"
    );

    // Item::EmbedBlock：node.rs 有 EmbedBlock，lz_codegen.rs:403 会发射 Item.EmbedBlock(lang, src_len)
    assert!(
        lib.contains("EmbedBlock(lang: str, src_len: int)"),
        "lz_ir_lib.lz 缺 Item.EmbedBlock 变体 (node.rs 有 EmbedBlock, lz_codegen.rs:403 会发射)"
    );
    assert!(
        lib.contains(
            "\"embed(\".to_string() + lg + \") { \" + n.to_string() + \" chars }\""
        ),
        "Item.EmbedBlock 显示分支缺失或格式漂移 (须对齐 display.rs:735 embed(lang) 后接 空格+花括号+数字+空格+chars+花括号)"
    );
}

#[test]
fn lz_ir_lib_mirrors_genbuild_and_block_ty() {
    let lib = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ir/lz_ir_lib.lz"),
    )
    .expect("read lz_ir_lib.lz");

    // Expr::GenBuild：node.rs 有 GenBuild{callee: Option<Box<Expr>>}，lz_codegen.rs:899 会发射
    // Expr.GenBuild(callee: Some(..)/None, stmts, ty)。callee 用 `Expr?` 触发 lang-zone 的
    // `Self?` 自动 Box（免写 Box<>；`MaybeExpr` 会让 Expr↔MaybeExpr 互递归触发 E0072）。
    assert!(
        lib.lines().any(|l| l.trim() == "GenBuild(callee: Expr?, stmts: List<Stmt>, ty: IrType)"),
        "lz_ir_lib.lz 缺 Expr.GenBuild 变体，或 callee 未用 `Expr?` 自动 Box（否则生成 .rs 报 E0072 互递归）"
    );
    assert!(
        lib.contains("case Expr.GenBuild(callee: c, stmts: s, ty: t) =>"),
        "Expr.GenBuild 显示分支缺失或格式漂移 (须对齐 display.rs:232 GenBuild 落 `_ => \"<expr>\"`)"
    );

    // Expr::BlockExpr 需拆两字段：e.ty（前缀 [e.ty]，display.rs:88）与 block.ty（块内 { [block.ty]，
    // display.rs:427）。原生 BlockExpr{block} 里这两个类型可不同（如 e.ty=?、block.ty=int），
    // 单一 ty 复用会静默错值。
    assert!(
        lib.lines().any(|l| {
            l.trim() == "BlockExpr(stmts: List<Stmt>, block_ty: IrType, ty: IrType)"
        }),
        "lz_ir_lib.lz 缺 Expr.BlockExpr 的 block_ty 字段（需与 e.ty 拆分，否则 block-with-ty 静默错值）"
    );
    assert!(
        lib.contains("case Expr.BlockExpr(stmts: ss, block_ty: bt, ty: t) =>"),
        "Expr.BlockExpr 显示分支未解构 block_ty（须与 e.ty 分槽渲染）"
    );

    // Item::FnDef：display.rs:595-628 不渲染 raises/exc 子句；LZ 侧多出的 `raises_sig(rs)`
    // 会让输出多出 ` raises T`（例 keywords_iterator.lz）。
    assert!(
        lib.contains("+ generic_sig(gs) + param_sig(ps) + \" -> \".to_string() + display_type(r) + \":\\n\""),
        "Item.FnDef 显示分支仍拼接 raises_sig（display.rs 不渲染 raises，须删除）"
    );
}

#[test]
fn lz_ir_lib_mirrors_latent_literal_variants() {
    let lib = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ir/lz_ir_lib.lz"),
    )
    .expect("read lz_ir_lib.lz");

    // IrType::{Int128,BigInt,Complex}：display.rs:14-16 分别渲染 int128/BigInt/Complex。
    // 缺变体时生成的 .rs 报 E0599（探针 _probes/latent_variants.lz 实测 5 处 E0599）。
    for (variant, want) in [
        ("    Int128", "case IrType.Int128 => \"int128\".to_string()"),
        ("    BigInt", "case IrType.BigInt => \"BigInt\".to_string()"),
        ("    Complex", "case IrType.Complex => \"Complex\".to_string()"),
    ] {
        assert!(
            lib.lines().any(|l| l.trim() == variant.trim()),
            "lz_ir_lib.lz IrType 缺 {variant} 变体（lz_codegen.rs:999-1001 会发射，缺失触发 E0599）"
        );
        assert!(
            lib.contains(want),
            "IrType 显示分支缺失/漂移: {want}"
        );
    }

    // Expr::LitInt128/LitBigInt/LitComplex：display.rs:243-245 渲染 {n}_i128 / BigInt({s}) /
    // Complex({re}, {im})，前缀仍为 [e.ty]（display.rs:88）。数值可能超 i64/i128 域，
    // 故 v 存 str；复数实虚部为 f64 元组。
    for (variant, want) in [
        (
            "LitInt128(v: str, ty: IrType)",
            "case Expr.LitInt128(v: n, ty: t) => \"[\".to_string() + display_type(t) + \"] \".to_string() + n + \"_i128\"",
        ),
        (
            "LitBigInt(v: str, ty: IrType)",
            "case Expr.LitBigInt(v: s, ty: t) => \"[\".to_string() + display_type(t) + \"] BigInt(\".to_string() + s + \")\".to_string()",
        ),
        (
            "LitComplex(v: (f64, f64), ty: IrType)",
            "case Expr.LitComplex(v: reim, ty: t) => \"[\".to_string() + display_type(t) + \"] Complex(\".to_string() + reim.0.to_string() + \", \".to_string() + reim.1.to_string() + \")\".to_string()",
        ),
    ] {
        assert!(
            lib.lines().any(|l| l.trim() == variant.trim()),
            "lz_ir_lib.lz Expr 缺 {variant} 变体（lz_codegen.rs:1132-1134 会发射）"
        );
        assert!(lib.contains(want), "Expr 字面量显示分支缺失/漂移: {variant}");
    }

    // Expr::Spread / Expr::EnumFieldGet：display.rs:232 Expr 兜底 `_ => "<expr>"` 覆盖二者，
    // 故 LZ 显示分支须落 `<expr>`（探针实测 `...lst` → `<expr>`）。
    for variant in [
        "Spread(expr: Expr, ty: IrType)",
        "EnumFieldGet(base: Expr, enum_name: str, variants: List<str>, field: str, ty: IrType)",
    ] {
        assert!(
            lib.lines().any(|l| l.trim() == variant),
            "lz_ir_lib.lz Expr 缺 {variant} 变体（lz_codegen.rs:697/:776 会发射）"
        );
    }
    assert!(
        lib.contains("case Expr.Spread(expr: _, ty: t) =>"),
        "Expr.Spread 显示分支未落 `<expr>` 兜底（display.rs:232）"
    );
}
