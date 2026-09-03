// Lang-Zone 编译器 — tests/reject_more.rs
// 测试强化（阶段2b / FIST 任务 T4.4）：内联反例测试对（错误路径 + 边界）
//
// 与 tests/reject_errors.rs 的分工：
// - reject_errors：遍历 DEMO/99_errors/ 目录文件（文件级反例语料）
// - reject_more：内联反例用例（含边界/异常路径），逐条断言编译器拒绝且
//   错误信息非空可读（input → expected: reject + diagnostic）
//
// 拒绝判定：lang-zone 二进制完整编译管线（宏展开→parse→IR→codegen）退出码非 0。

use std::path::PathBuf;
use std::process::Command;

struct RejectCase {
    name: &'static str,
    source: &'static str,
    /// 期望被拒绝的大致阶段（lexer/parser/ir/codegen），仅用于可读性
    phase: &'static str,
}

const REJECT_CASES: &[RejectCase] = &[
    RejectCase {
        name: "unclosed_paren",
        source: "def main() =\n    let x = (1 + 2\n    print(x)\n",
        phase: "parser",
    },
    RejectCase {
        name: "missing_colon_if",
        source: "def main() =\n    if True\n        print(1)\n",
        phase: "parser",
    },
    RejectCase {
        name: "bare_else",
        source: "def main() =\n    else:\n        print(1)\n",
        phase: "parser",
    },
    RejectCase {
        name: "empty_fn_no_body",
        source: "def f() -> int =\n",
        phase: "parser",
    },
    RejectCase {
        name: "invalid_hex",
        source: "let x = 0xGG\n",
        phase: "lexer",
    },
    RejectCase {
        name: "int_overflow",
        source: "let x = 99999999999999999999999999999\n",
        phase: "lexer",
    },
    RejectCase {
        name: "unterminated_string",
        source: "def main() =\n    let s = \"abc\n    print(s)\n",
        phase: "lexer",
    },
    RejectCase {
        name: "import_inside_fn",
        source: "def main() =\n    import foo.bar\n",
        phase: "parser",
    },
    RejectCase {
        name: "bad_type_annotation",
        source: "def f(x: ???) -> int = 1\n",
        phase: "parser",
    },
    RejectCase {
        name: "broken_while_condition",
        source: "def main() =\n    while :\n        print(1)\n",
        phase: "parser",
    },
    RejectCase {
        name: "trailing_comma_let",
        source: "def main() =\n    let x, = 1\n",
        phase: "parser",
    },
    RejectCase {
        name: "deep_nested_unclosed",
        source: "def main() =\n    let xs = [[[[1, 2]]\n    print(xs)\n",
        phase: "parser",
    },
    RejectCase {
        // 2026-08-28 语义修订：字符串字面量 raise（raise "msg"）为消息式错误，
        // 免 raises 声明（已移入 ACCEPTED_CASES 锁定）；类型化 raise 仍必须声明。
        name: "typed_raise_without_raises",
        source: "def f(x: int) -> int =\n    if x < 0:\n        raise NegError(x)\n    x\n",
        phase: "semantic（G2：类型化 raise 未声明 raises 须拒绝）",
    },
];

/// 已知宽松语义（当前编译器接受，非拒绝）：
/// 作为语义边界回归锁定——若未来收紧为拒绝，本表须同步迁移至 REJECT_CASES。
const ACCEPTED_CASES: &[RejectCase] = &[
    RejectCase {
        name: "bad_indent_body",
        source: "def main() =\nprint(1)\n",
        phase: "parser（宽松：无缩进函数体被接受）",
    },
    RejectCase {
        name: "duplicate_def",
        source: "def f() = 1\ndef f() = 2\n",
        phase: "ir/codegen（宽松：重复定义未拒绝）",
    },
    RejectCase {
        name: "unknown_keyword",
        source: "def main() =\n    frobnicate(1)\n",
        phase: "lexer/parser（宽松：未知标识符按函数调用通过，交 rustc 阶段）",
    },
    RejectCase {
        name: "struct_no_fields",
        source: "struct Empty =\n",
        phase: "parser（宽松：空 struct 被接受）",
    },
    RejectCase {
        name: "yield_outside_iterator",
        source: "def f() =\n    yield 1\n",
        phase: "ir（宽松：yield 未强制 iterator 上下文）",
    },
    RejectCase {
        // 2026-08-28 起：字符串字面量 raise 为消息式错误，可免 raises 声明
        //（codegen has_raise 豁免）；类型化 raise 见 REJECT_CASES。
        name: "string_raise_without_raises",
        source: "def f(x: int) -> int =\n    if x < 0:\n        raise \"neg\"\n    x\n",
        phase: "semantic（宽松：字符串 raise 免 raises 声明）",
    },
];

#[test]
fn inline_error_boundaries_are_rejected() {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let work = std::env::temp_dir().join("lz_reject_more");
    let _ = std::fs::create_dir_all(&work);

    let mut rejected = 0;
    let mut failures = Vec::new();

    for case in REJECT_CASES {
        let lz_path = work.join(format!("{}.lz", case.name));
        std::fs::write(&lz_path, case.source).expect("write lz source");

        let out = Command::new(&bin)
            .arg(&lz_path)
            .output()
            .expect("run lang-zone");

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // 错误信息必须非空且包含可读诊断（防静默失败）
            assert!(
                !stderr.trim().is_empty(),
                "[{}] 编译器拒绝但错误信息为空",
                case.name
            );
            rejected += 1;
            eprintln!("  ✅ {} ({}): {}", case.name, case.phase, stderr.lines().next().unwrap_or(""));
        } else {
            failures.push(case.name);
        }
    }

    println!("\n===== 内联反例拒绝测试报告 =====");
    println!("  总计: {} 用例", REJECT_CASES.len());
    println!("  正确拒绝: {}", rejected);
    println!("  意外通过: {}", failures.len());
    if !failures.is_empty() {
        for name in &failures {
            println!("    ⚠️  {} — 编译器未拒绝", name);
        }
    }
    println!("================================\n");

    assert!(
        failures.is_empty(),
        "{} 个内联反例意外通过编译: {:?}",
        failures.len(),
        failures
    );

    // 已知宽松语义：断言编译器当前接受（回归锁定，防止意外收紧破坏既有程序）
    let mut accepted = 0usize;
    for case in ACCEPTED_CASES {
        let lz_path = work.join(format!("acc_{}.lz", case.name));
        std::fs::write(&lz_path, case.source).expect("write lz source");
        let out = Command::new(&bin).arg(&lz_path).output().expect("run lang-zone");
        assert!(
            out.status.success(),
            "[{}] 已知宽松语义被拒绝——行为收紧？({})",
            case.name,
            String::from_utf8_lossy(&out.stderr)
        );
        accepted += 1;
        eprintln!("  🔓 {} ({})", case.name, case.phase);
    }
    println!("  已知宽松语义（接受）: {}", accepted);
}
