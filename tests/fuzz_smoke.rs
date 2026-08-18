// Lang-Zone 编译器 — tests/fuzz_smoke.rs
// 测试强化（阶段2b / FIST 任务 T4.4）：伪随机模糊 smoke 测试
//
// 目标（方向G · 模糊测试验收"无崩溃"的轻量落地）：
// - 确定性伪随机（LCG，固定种子）生成 .lz 片段组合
// - 完整编译管线（lexer → 宏展开 → parser → IR）必须**不 panic**
// - 任何 Err 返回都必须带非空可读诊断（防静默/空错误）
// 注意：随机程序绝大多数非法，测试只约束"不崩溃 + 诊断可读"，不约束结果。

use std::panic::{catch_unwind, AssertUnwindSafe};

use lang_zone::ir::build_ir;
use lang_zone::lexer::Lexer;
use lang_zone::macros::expand::{extract_macro_defs, MacroExpander};
use lang_zone::parser::Parser;

/// 运行完整前端管线；Ok=成功（含 IR JSON 长度），Err=诊断文本
fn run_pipeline(source: &str) -> Result<usize, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let (registry, _) = extract_macro_defs(&tokens).map_err(|e| format!("{e}"))?;
    let expander = MacroExpander::new(registry);
    let expanded = expander.expand(&tokens).map_err(|e| format!("{e}"))?;
    let mut parser = Parser::new(expanded);
    let module = parser.parse_module().map_err(|e| format!("{e}"))?;
    let ir = build_ir(&module).map_err(|e| format!("{e}"))?;
    Ok(ir.to_json().map(|s| s.len()).unwrap_or(0))
}

/// LCG 伪随机（确定性）
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u64
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const KEYWORDS: &[&str] = &[
    "def", "let", "if", "else", "for", "while", "return", "import",
    "struct", "print", "true", "false", "None", "match", "case", "yield",
];
const IDENTS: &[&str] = &["x", "y", "acc", "foo", "main", "value", "items", "self"];
const OPS: &[&str] = &["+", "-", "*", "//", "%", "==", ">", "<", "and", "or"];
const LITERALS: &[&str] = &["0", "1", "42", "3.14", "\"hi\"", "\"\"", "[1, 2]", "[]"];
const BAD_TOKENS: &[&str] = &["@", "#", "0xG", "\"unterminated", "??", "::", "..,", "&&&"];

/// 生成一个随机程序行
fn gen_line(rng: &mut Lcg) -> String {
    match rng.below(10) {
        0 => format!("let {} = {}", IDENTS[rng.below(IDENTS.len())], LITERALS[rng.below(LITERALS.len())]),
        1 => format!("def {}({}: int) -> int = {}", IDENTS[rng.below(IDENTS.len())], IDENTS[rng.below(IDENTS.len())], LITERALS[rng.below(LITERALS.len())]),
        2 => format!("{} = {} {} {}", IDENTS[rng.below(IDENTS.len())], LITERALS[rng.below(LITERALS.len())], OPS[rng.below(OPS.len())], LITERALS[rng.below(LITERALS.len())]),
        3 => format!("if {}:", "true"),
        4 => "else:".to_string(),
        5 => format!("print({})", LITERALS[rng.below(LITERALS.len())]),
        6 => format!("for {} in 0..{}:", IDENTS[rng.below(IDENTS.len())], rng.below(20)),
        7 => format!("while {} < {}:", IDENTS[rng.below(IDENTS.len())], rng.below(10)),
        8 => format!("import {}.{}", IDENTS[rng.below(IDENTS.len())], IDENTS[rng.below(IDENTS.len())]),
        _ => format!("{}", BAD_TOKENS[rng.below(BAD_TOKENS.len())]),
    }
}

/// 生成随机程序：若干顶层/缩进行组合
fn gen_program(rng: &mut Lcg) -> String {
    let mut out = String::new();
    let lines = 1 + rng.below(12);
    let mut indent = 0usize;
    for _ in 0..lines {
        let line = gen_line(rng);
        // 随机加深缩进（受控），模拟嵌套块
        if rng.below(4) == 0 && indent < 4 {
            indent += 1;
        } else if rng.below(6) == 0 && indent > 0 {
            indent -= 1;
        }
        for _ in 0..indent {
            out.push_str("    ");
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[test]
fn fuzz_pipeline_never_panics() {
    let mut rng = Lcg(0xC0FFEE_2026_0817);
    let total = 400usize;
    let mut ok_count = 0usize;
    let mut err_count = 0usize;
    let mut empty_diag = 0usize;

    for i in 0..total {
        let src = gen_program(&mut rng);

        let outcome = catch_unwind(AssertUnwindSafe(|| run_pipeline(&src)));
        match outcome {
            Ok(Ok(_)) => ok_count += 1,
            Ok(Err(diag)) => {
                err_count += 1;
                if diag.trim().is_empty() {
                    empty_diag += 1;
                    eprintln!("  ⚠️  seed#{i}: 空错误诊断，源码:\n{}", src);
                }
            }
            Err(_) => {
                // panic 恢复，随后断言失败（本轮收集所有崩溃点）
                eprintln!("  💥 seed#{i}: 管线 PANIC，源码:\n{}", src);
                panic!("模糊测试发现管线 panic（seed#{i}）");
            }
        }
    }

    println!("\n===== 模糊 smoke 测试报告 =====");
    println!("  随机程序总数: {}", total);
    println!("  管线成功: {}", ok_count);
    println!("  管线报错(预期): {}", err_count);
    println!("  空诊断: {}", empty_diag);
    println!("===============================\n");

    assert_eq!(empty_diag, 0, "存在空错误诊断，须保证错误信息可读");
}
