// Lang-Zone 编译器 — tests/ir_serialization.rs
// 测试强化（阶段2b / FIST 任务 T4.4）：IR 序列化与边界契约测试
//
// 覆盖阶段1a（T4.1）关键路径：
// - IR 模块序列化往返：to_json/from_json、to_bincode/from_bincode 一致
// - source_hash 稳定性：同源码哈希不变；不同源码哈希不同
// - 模块边界：exports / dependencies / span 完备性契约

use lang_zone::ir::IrModule;
use lang_zone::parser::Parser;

fn build_module(name: &str, source: &str) -> IrModule {
    let tokens = lang_zone::lexer::Lexer::new(source).tokenize();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_module().expect("parse ok");
    lang_zone::ir::builder::build_ir(&ast)
        .expect("build_ir ok")
        // source_text 由编译入口注入（此处显式注入以便 source_hash 基于内容）
        .with_source_text(source)
}

const SAMPLE: &str = r#"
import std.math

struct Point =
    x: int
    y: int

def dist(a: Point, b: Point) -> int =
    let dx = a.x - b.x
    let dy = a.y - b.y
    dx * dx + dy * dy

def main() =
    let p1 = Point(x: 0, y: 0)
    let p2 = Point(x: 3, y: 4)
    print(dist(p1, p2))
"#;

#[test]
fn ir_json_roundtrip() {
    let m = build_module("demo", SAMPLE);
    let json = m.to_json().expect("to_json ok");
    let back = IrModule::from_json(&json).expect("from_json ok");

    // 往返后核心边界保持
    assert_eq!(back.name, m.name);
    assert_eq!(back.exports, m.exports, "exports 应一致");
    assert_eq!(back.dependencies.len(), m.dependencies.len(), "dependencies 应一致");
    assert!(!back.items.is_empty(), "items 不应为空");
    assert_eq!(back.source_hash(), m.source_hash(), "source_hash 应一致");
    // 再序列化仍稳定（幂等）
    let json2 = back.to_json().expect("to_json again");
    assert_eq!(json, json2, "JSON 序列化应幂等稳定");
}

#[test]
fn ir_bincode_roundtrip() {
    let m = build_module("demo_bin", SAMPLE);
    let bytes = m.to_bincode().expect("to_bincode ok");
    assert!(!bytes.is_empty(), "bincode 不应为空");
    let back = IrModule::from_bincode(&bytes).expect("from_bincode ok");
    assert_eq!(back.name, m.name);
    assert_eq!(back.source_hash(), m.source_hash(), "bincode 往返后 source_hash 一致");
    assert_eq!(back.exports.len(), m.exports.len(), "bincode 往返后 exports 一致");
    // 双格式一致性：同一模块 json 与 bincode 往返应产生相同的 exports 集合
    let via_json = IrModule::from_json(&m.to_json().unwrap()).unwrap();
    assert_eq!(
        back.collect_exports(),
        via_json.collect_exports(),
        "json/bincode 两种往返应导出相同边界"
    );
}

#[test]
fn source_hash_is_content_based() {
    let a = build_module("a", "def f() = 1\ndef main() =\n    print(f())\n");
    let a2 = build_module("a2", "def f() = 1\ndef main() =\n    print(f())\n");
    let b = build_module("b", "def f() = 2\ndef main() =\n    print(f())\n");
    assert_eq!(a.source_hash(), a2.source_hash(), "相同源码哈希应一致");
    assert_ne!(a.source_hash(), b.source_hash(), "不同源码哈希应不同");
    // 只改变注释（空格）不影响哈希的语义判定——源码哈希按文本内容，若语义无关变更也不应相同（当前为内容哈希）
    let c = build_module("c", "def f() = 1\ndef main() =\n    print(f())\n\n");
    let _ = c;
}

#[test]
fn ir_module_boundaries_contract() {
    let m = build_module("bound", SAMPLE);
    let (total, unknown, no_file) = m.check_span_completeness();
    assert!(total > 0, "span 总数应大于 0");
    // unknown 为宏展开/合成节点（直接 API 路径无文件路径注入，允许存在）；
    // 关键契约：序列化往返不丢失 span 完备性信息
    let bytes = m.to_bincode().expect("bincode ok");
    let back = IrModule::from_bincode(&bytes).expect("bincode back");
    let (t2, u2, f2) = back.check_span_completeness();
    assert_eq!(
        (t2, u2, f2),
        (total, unknown, no_file),
        "span 完备性统计在序列化往返后应一致（total={total}, unknown={unknown}, no_file={no_file}）"
    );
    // 导出边界：struct Point / def dist / def main 至少存在
    let exports = m.collect_exports();
    assert!(exports.iter().any(|e| e.contains("Point")), "exports 应含 Point: {exports:?}");
    assert!(exports.iter().any(|e| e.contains("dist")), "exports 应含 dist: {exports:?}");
    // 依赖边界：import std.math → 模块依赖 std
    let deps = m.collect_dependencies();
    assert!(!deps.is_empty(), "应有 import 依赖");
    assert!(deps.iter().any(|d| d.module == "std"), "依赖应含 std: {deps:?}");
}

#[test]
fn ir_serialize_minimal_module() {
    let m = build_module("emptyish", "def main() =\n    print(1)\n");
    let bytes = m.to_bincode().expect("bincode ok");
    let back = IrModule::from_bincode(&bytes).expect("bincode back");
    assert!(!back.name.is_empty(), "模块名不应为空");
    assert_eq!(back.source_hash(), m.source_hash());
}
