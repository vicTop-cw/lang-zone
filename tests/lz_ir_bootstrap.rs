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
