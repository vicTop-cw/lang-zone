// Lang-Zone 编译器 — tests/bridge_boundary.rs
// 边界用例：bridge 路由、embed 收集、三层路由的边界与异常场景

use lang_zone::ir;
use lang_zone::ir::codegen::CodeGen;
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;

fn gen_rust(source: &str) -> String {
    let tokens = Lexer::new(source).tokenize();
    let mut parser = Parser::new(tokens);
    let module_ast = parser.parse_module().expect("parse ok");
    let mut ir_module = ir::builder::build_ir(&module_ast).expect("ir ok");
    ir_module.file_path = Some("input.lz".to_string());
    let mut cg = CodeGen::new();
    cg.generate_with_bridge(&ir_module).0
}

// ─── bridge 路由边界 ───

#[test]
fn bridge_single_segment_tnr() {
    let rs = gen_rust("import tnr\n\ndef main():\n    print(1)\n");
    assert!(
        !rs.contains("tnr_transpiled"),
        "单段 tnr 无函数名，不应路由到 LibApi，生成的 .rs:\n{rs}"
    );
}

#[test]
fn bridge_deep_path() {
    let rs = gen_rust("import tnr::a::b::c\n\ndef main():\n    print(1)\n");
    assert!(
        rs.contains("tnr_transpiled::a"),
        "深层路径 tnr::a::b::c 应路由到 LibApi，生成的 .rs:\n{rs}"
    );
}

#[test]
fn bridge_multiple_imports() {
    let src = r#"import tnr::matmul
import tnr::sigmoid
import tnr_lib::tensor::Tensor

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("tnr_transpiled::matmul") && rs.contains("tnr_transpiled::sigmoid"),
        "多个 bridge import 应各自路由，生成的 .rs:\n{rs}"
    );
    assert!(
        rs.contains("tnr::tensor::Tensor"),
        "tnr_lib import 应路由到 tnr::tensor::Tensor，生成的 .rs:\n{rs}"
    );
}

#[test]
fn bridge_compute_deep_path() {
    let rs = gen_rust("import tnr::compute::kernel\n\ndef main():\n    print(1)\n");
    assert!(
        rs.contains("tnr_compute::kernel"),
        "compute 路径应路由到 tnr_compute，生成的 .rs:\n{rs}"
    );
}

#[test]
fn bridge_unknown_module_not_routed() {
    let rs = gen_rust("import unknown_mod::func\n\ndef main():\n    print(1)\n");
    assert!(
        !rs.contains("tnr_transpiled") && !rs.contains("tnr_compute"),
        "未知模块 unknown_mod 不应路由到 tnr bridge，生成的 .rs:\n{rs}"
    );
}

// ─── embed 收集边界 ───

#[test]
fn embed_multiple_blocks_same_lang() {
    let src = r#"#[embed(tnr)]
```
fn helper1(a, b) { a + b }
```

#[embed(tnr)]
```
fn helper2(a, b) { a * b }
```

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("helper1") && rs.contains("helper2"),
        "两个同语言 embed 块都应出现在生成产物中，生成的 .rs:\n{rs}"
    );
}

#[test]
fn embed_multiple_blocks_different_lang() {
    let src = r#"#[embed(tnr)]
```
fn tnr_func(a, b) { a + b }
```

#[embed(rust)]
```
fn rust_func(a: i64) -> i64 { a * 2 }
```

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("tnr_func"),
        "tnr embed 块应出现在生成产物中，生成的 .rs:\n{rs}"
    );
    assert!(
        rs.contains("rust_func"),
        "rust embed 块应出现在生成产物中，生成的 .rs:\n{rs}"
    );
}

// ─── 三层路由边界 ───

#[test]
fn routing_embed_and_bridge_coexist() {
    let src = r#"#[embed(tnr)]
```
fn embedded(a, b) { a + b }
```

import input_tnr
import tnr::matmul

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("use input_tnr"),
        "embed 模块 input_tnr 应走 embed 路由，生成的 .rs:\n{rs}"
    );
    assert!(
        rs.contains("tnr_transpiled::matmul"),
        "bridge import tnr::matmul 应走 bridge 路由，生成的 .rs:\n{rs}"
    );
}

#[test]
fn routing_multiple_unknown_all_bridge() {
    let src = r#"import tnr::matmul
import tnr::sigmoid
import tnr::compute::kernel

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("tnr_transpiled::matmul")
            && rs.contains("tnr_transpiled::sigmoid")
            && rs.contains("tnr_compute::kernel"),
        "三个未知模块都应走 bridge 路由，生成的 .rs:\n{rs}"
    );
}

#[test]
fn routing_std_not_intercepted_by_bridge() {
    let src = r#"import std::collections::HashMap
import tnr::matmul

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("std::collections"),
        "std 模块应走 std 路由，生成的 .rs:\n{rs}"
    );
    assert!(
        rs.contains("tnr_transpiled::matmul"),
        "tnr 模块应走 bridge 路由，生成的 .rs:\n{rs}"
    );
}

// ─── 大量导入压力测试 ───

#[test]
fn bridge_many_imports() {
    let mut src = String::new();
    for i in 0..20 {
        src.push_str(&format!("import tnr::func{}\n", i));
    }
    src.push_str("\ndef main():\n    print(1)\n");
    let rs = gen_rust(&src);
    for i in 0..20 {
        let expected = format!("tnr_transpiled::func{}", i);
        assert!(
            rs.contains(&expected),
            "bridge import func{} 未路由，生成的 .rs:\n{rs}", i
        );
    }
}
