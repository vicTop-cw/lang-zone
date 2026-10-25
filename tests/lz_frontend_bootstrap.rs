// Lang-Zone 自举回归测试（路线 B 前端接入）：--emit=lex-lz / --emit=parse-lz
//
// 验证链路：lzc --emit=lex-lz/--emit=parse-lz <输入.lz>
//   → main.rs 内嵌 src/frontend/lz_lexer.lz / lz_parser.lz（include_str!）
//   → 生成 wrapper .lz → 递归 lzc → .rs → rustc → exe → 运行输出
// 断言输出与入库 golden 一致（golden = 2026-08-16 验证通过的输出，见 bootstrap/05 §4）。
//
// 若 src/frontend/*.lz 回归，本测试即失败；失败信息直接指向对应组件。

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run_flag(tmp: &str, name: &str, source: &str, flag: &str) -> String {
    let work = std::env::temp_dir().join(tmp);
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join(format!("{}.lz", name));
    fs::write(&lz_path, source).expect("write input lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz_path)
        .arg(flag)
        .output()
        .expect("run lang-zone with flag");
    assert!(
        out.status.success(),
        "{} failed: {}",
        flag,
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&work);
    String::from_utf8_lossy(&out.stdout).to_string()
}

const SAMPLE_BASIC: &str = "def f(x):\n    return x + 1\n";

/// A1/A2：LZ 版 lexer 行为对齐 + 主流程入口
/// golden：固定输入集的 token 流（2026-08-16 验证输出；IntLit 由 v170 遗留
/// 缺陷修复后的查表折叠生成）。
#[test]
fn lz_lexer_matches_native_golden() {
    let stdout = run_flag("lz_lex_golden", "sample", SAMPLE_BASIC, "--emit=lex-lz");
    let got: Vec<&str> = stdout
        .lines()
        .map(|l| l.trim().trim_matches('"'))
        .filter(|l| !l.is_empty() && !l.starts_with("Generated"))
        .collect();
    let expected = vec![
        "Def", "Ident(f)", "LParen", "Ident(x)", "RParen", "Colon", "Newline",
        "Return", "Ident(x)", "Plus", "IntLit(1)", "Newline", "Eof",
    ];
    assert_eq!(
        got, expected,
        "LZ lexer token 流与 golden 不一致（词法前端回归？）"
    );
}

/// A2：--emit=lex-lz 独立入口（字符串/magic method/比较运算符覆盖）
#[test]
fn emit_lex_lz_flag_runs() {
    let src = "def g(a, b):\n    if a >= b:\n        return \"ok\"\n    else:\n        return a + 1\n";
    let stdout = run_flag("lz_lex_flag", "rich", src, "--emit=lex-lz");
    assert!(stdout.contains("StrLit(ok)"), "缺 StrLit: {}", stdout);
    assert!(stdout.contains("IntLit(1)"), "缺 IntLit: {}", stdout);
    assert!(stdout.contains("Gt") && stdout.contains("If") && stdout.contains("Else"), "缺关键字/比较符: {}", stdout);
}

/// A3：BUG-462 修复回归 —— 此前 LZ lexer 的 `punct_token` 表缺 `=`、`is_keyword`
/// 缺 `let`，导致 `let x = 1` 被切成 `Ident(let)` 且等号被静默丢弃（tokenize
/// 未命中即跳过）。本测试锁定修复：`let x = 1` 须产生完整
/// `Let Ident(x) Eq IntLit(1)` 序列。
#[test]
fn emit_lex_lz_let_and_eq() {
    let src = "def main() =\n    let x = 1\n    print(x)\n";
    let stdout = run_flag("lz_lex_let_eq", "let_eq", src, "--emit=lex-lz");
    let got: Vec<&str> = stdout
        .lines()
        .map(|l| l.trim().trim_matches('"'))
        .filter(|l| !l.is_empty() && !l.starts_with("Generated"))
        .collect();
    // 期望前缀子序列（def main() = 后的 let x = 1）
    let needle = vec![
        "Def", "Ident(main)", "LParen", "RParen", "Eq", "Newline", "Let", "Ident(x)", "Eq",
        "IntLit(1)",
    ];
    assert!(
        got.windows(needle.len()).any(|w| w == needle.as_slice()),
        "lex-lz 未生成 let/= 关键 token（BUG-462 回归?）: got={:?}",
        got
    );
    assert!(got.contains(&"Eq"), "lex-lz 丢了 Eq token: {:?}", got);
    assert!(got.contains(&"Let"), "lex-lz 丢了 Let 关键字: {:?}", got);
}

/// A4：BUG-463 修复回归 —— parse-lz 的 parse_stmt Def 分支此前只跳 `Colon`，
/// `def main() =` 的 `=` 卡住 parse_block（block_tag(`=`)==0 → 空块），
/// 函数体整块被吞成 `def main() {}`。本测试锁定 `=` 式定义能解析出函数体。
#[test]
fn emit_parse_lz_eq_form_def() {
    let src = "def main() =\n    let x = 1\n    print(x)\n";
    let stdout = run_flag("lz_parse_eqdef", "eqdef", src, "--emit=parse-lz");
    // 函数体不再被吞：须出现 let 绑定与 print 调用
    assert!(
        stdout.contains("def main()") && stdout.contains("let x = 1"),
        "parse-lz 未解析 `=` 式 def 的函数体（BUG-463 回归?）: {}",
        stdout
    );
}

const SAMPLE_BASIC_GOLDEN: &str = "def f(x) {return x + 1}";

/// B1/B2：LZ 版 parser 行为对齐 + 主流程入口
/// golden：固定输入集解析描述（2026-08-16 验证输出；parse_params 支持
/// 无类型标注参数 `def f(x)` 后的结果）。
#[test]
fn lz_parser_matches_golden() {
    let stdout = run_flag("lz_parse_golden", "sample", SAMPLE_BASIC, "--emit=parse-lz");
    assert!(
        stdout.contains(SAMPLE_BASIC_GOLDEN),
        "parser 描述与 golden 不一致: {}",
        stdout
    );
}

/// B2：--emit=parse-lz 独立入口（多参数/if-else/字符串字面量覆盖）
#[test]
fn emit_parse_lz_flag_runs() {
    let src = "def g(a, b):\n    if a >= b:\n        return \"ok\"\n    else:\n        return a + 1\n";
    let stdout = run_flag("lz_parse_flag", "rich", src, "--emit=parse-lz");
    assert!(
        stdout.contains("def g(a, b)") && stdout.contains("if a >= b") && stdout.contains("else"),
        "parser 描述缺关键子串: {}",
        stdout
    );
}
