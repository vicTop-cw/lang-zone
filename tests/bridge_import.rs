// Lang-Zone 编译器 — tests/bridge_import.rs
// P1-P4：bridge 接入 tnr 验证

use lang_zone::ir;
use lang_zone::ir::codegen::CodeGen;
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;

fn gen_rust(source: &str) -> String {
    let tokens = Lexer::new(source).tokenize();
    let mut parser = Parser::new(tokens);
    let module_ast = parser.parse_module().expect("parse ok");
    let ir_module = ir::builder::build_ir(&module_ast).expect("ir ok");
    let mut cg = CodeGen::new();
    cg.generate_with_bridge(&ir_module).0
}

#[test]
fn tnr_bridge_lib_api_import() {
    let src = "import tnr::matmul\n";
    let rs = gen_rust(src);
    assert!(
        rs.contains("use tnr_transpiled::matmul;"),
        "expect use tnr_transpiled::matmul; in:\n{rs}"
    );
}

#[test]
fn tnr_bridge_compiled_lib_import() {
    let src = "import tnr_lib::tensor::Tensor\n";
    let rs = gen_rust(src);
    assert!(
        rs.contains("use tnr::tensor::Tensor;"),
        "expect use tnr::tensor::Tensor; in:\n{rs}"
    );
}

#[test]
fn tnr_bridge_compute_import() {
    let src = "import tnr::compute::kernel\n";
    let rs = gen_rust(src);
    assert!(
        rs.contains("use tnr_compute::kernel;"),
        "expect use tnr_compute::kernel; in:\n{rs}"
    );
}

#[test]
fn std_import_still_works() {
    let src = "import std::collections::HashMap\n";
    let rs = gen_rust(src);
    assert!(
        rs.contains("std::collections"),
        "expect std::collections in:\n{rs}"
    );
}
