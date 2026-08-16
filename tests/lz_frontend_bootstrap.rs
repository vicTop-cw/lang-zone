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

const SAMPLE_BASIC_GOLDEN: &str = "\"def f(x) {return x + 1}\"";

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
