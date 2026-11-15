// Lang-Zone 编译器 — tests/lzc_loop_unit_variant.rs
//
// 回归闸门：`Enum.Variant()` 这类**调用式构造无数据变体**在 Rust 后端的产物形态。
//
// 用例来源（2026-10-02 实测，非推测）：
//   CY/TESTS/07_data_structures/enum_data.lz 走默认（rust）后端时 rustc 报
//   `error[E0618]: expected function, found enum variant \`Shape::Point\``，
//   而同一份 .lz 在 cy 后端能编译能跑 ⇒ 跨后端行为分叉。生成面在
//   src/ir/codegen/mod.rs 的 FieldAccess 构造分支：`Type.Variant(args…)` 恒发
//   `Type::Variant(args…)`，零参数时单元变体被当成函数调用。
//
// 四条用例是一组**双向锁**（`#[test]` 4 个，端到端跑 6 份 .lz）：
//   1. dotted 零参变体（缺陷正身）——修复前必红；
//   2. 裸简写零参变体（另一条已通的路线）——证明修复没把它带坏；
//   3. 同一形状出现在返回位 / 实参位 / 列表元素位——三个 lowering 现场都要一致；
//   4. 零参关联函数 `Counter.new()`（护栏对照）——若把「去括号」做得过宽，
//      Rust 侧 `Counter::new` 会 E0015/E0423 编译失败 ⇒ 本条会红。
// 只增不删；每条都是 .lz → lang-zone → rustc → 运行 → 断言 stdout 的端到端面。

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

/// 编译并运行单个 .lz 源，返回运行 stdout；任一环节失败则 panic 并附诊断
fn run_lz(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lzc_loop_uv_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
    assert!(
        out.status.success(),
        "[{name}] lang-zone 编译失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rs = lz.with_extension("rs");
    assert!(rs.exists(), "[{name}] 未生成 .rs 产物");
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("-L")
        .arg(format!(
            "dependency={}/target/debug/deps",
            env!("CARGO_MANIFEST_DIR")
        ))
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    assert!(
        rc.status.success(),
        "[{name}] rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );

    let run = Command::new(&exe).output().expect("run compiled exe");
    assert!(
        run.status.success(),
        "[{name}] 程序运行失败: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// 只跑 codegen，返回生成的 .rs 文本（产物形态判据用，不经 rustc）
fn codegen_rs_text(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lzc_loop_uv_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
    assert!(
        out.status.success(),
        "[{name}] lang-zone 编译失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(lz.with_extension("rs")).expect("read rs")
}

/// 1. 缺陷正身：`Shape.Point()` → 必须是 `Shape::Point`（不带调用括号）
#[test]
fn unit_variant_dotted_ctor_has_no_call_parens() {
    let src = "enum Shape:\n    Circle(f64)\n    Point\n\ndef main() =\n    let p = Shape.Point()\n    print(p)\n";
    // 产物面单独钉一次：只看能否编译的话，「给变体补一个 0 元 fn」这类路子也能蒙绿。
    let code = codegen_rs_text("dotted", src);
    let point_lines = code
        .lines()
        .filter(|l| l.contains("Shape::Point"))
        .collect::<Vec<_>>()
        .join(" / ");
    assert!(
        !code.contains("Shape::Point("),
        "产物里仍在**调用**单元变体 ⇒ {point_lines}"
    );
    assert!(
        code.contains("Shape::Point"),
        "产物里找不到 `Shape::Point` ⇒ {point_lines}"
    );
    let out = run_lz("dotted", src);
    assert_eq!(out.trim(), "Point", "零参变体构造结果不符：{out}");
}

/// 2. 已通路线不受影响：裸简写 `Red()` 也走零参分支
#[test]
fn unit_variant_bare_ctor_still_works() {
    let out = run_lz(
        "bare",
        "enum Color:\n    Red\n    Blue\n\ndef main() =\n    let c = Red()\n    print(c)\n",
    );
    assert_eq!(out.trim(), "Red", "裸简写零参变体结果不符：{out}");
}

/// 3. 嵌套位置的同一形状（返回位 / 实参位 / 列表元素位）：三条都是 `Shape::Point`，
///    期望值取自 Rust 后端实跑（2026-10-02：'Point' / '7' / '[Point, Point]'）
#[test]
fn unit_variant_ctor_in_nested_positions() {
    let enum_pre = "enum Shape:\n    Circle(f64)\n    Point\n\n";
    let out = run_lz(
        "nested_return",
        &format!("{enum_pre}def mk() -> Shape =\n    Shape.Point()\ndef main() =\n    print(mk())\n"),
    );
    assert_eq!(out.trim(), "Point", "返回位零参变体：{out}");
    let out = run_lz(
        "nested_arg",
        &format!("{enum_pre}def take(s: Shape) -> int =\n    7\ndef main() =\n    print(take(Shape.Point()))\n"),
    );
    assert_eq!(out.trim(), "7", "实参位零参变体：{out}");
    let out = run_lz(
        "nested_list",
        &format!("{enum_pre}def main() =\n    let xs = [Shape.Point(), Shape.Point()]\n    print(xs)\n"),
    );
    assert_eq!(out.trim(), "[Point, Point]", "列表元素位零参变体：{out}");
}
/// 4. 护栏对照：零参**关联函数**必须保留括号（去括号做宽了就 E0015/E0423）
#[test]
fn zero_arg_associated_fn_keeps_parens() {
    let src = "struct Counter =\n    total: int\n\n    def new() -> Counter =\n        Counter(total: 0)\n\ndef main() =\n    let c = Counter.new()\n    print(c.total)\n";
    let code = codegen_rs_text("assoc", src);
    assert!(
        code.contains("Counter::new()"),
        "零参关联函数被摘掉括号 ⇒ 产物里的 Counter 相关行：{}",
        code.lines()
            .filter(|l| l.contains("Counter::new"))
            .collect::<Vec<_>>()
            .join(" / ")
    );
    let out = run_lz("assoc", src);
    assert_eq!(out.trim(), "0", "零参关联函数语义改变：{out}");
}
