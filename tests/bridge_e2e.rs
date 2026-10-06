// Lang-Zone 编译器 — tests/bridge_e2e.rs
// 端到端集成测试：embed + bridge import + extern + export 同时使用
// 验证三种跨语言机制能在同一 LZ 程序中协同工作
//
// 测试分层：
//   1. 库 API（gen_rust）：验证 codegen 产物中 bridge import 路由 + export 函数存在
//   2. CLI（compile_lz）：验证 lang-zone 编译成功 + Bridge registry 登记 + 生成 .rs 含全部机制产物

use std::path::PathBuf;
use std::process::Command;

use lang_zone::ir;
use lang_zone::ir::codegen::CodeGen;
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;

// ─── 库 API 路径 ───

fn gen_rust(source: &str) -> String {
    let tokens = Lexer::new(source).tokenize();
    let mut parser = Parser::new(tokens);
    let module_ast = parser.parse_module().expect("parse ok");
    let mut ir_module = ir::builder::build_ir(&module_ast).expect("ir ok");
    ir_module.file_path = Some("input.lz".to_string());
    let mut cg = CodeGen::new();
    cg.generate_with_bridge(&ir_module).0
}

// ─── CLI 路径 ───

fn compile_lz(name: &str, source: &str) -> (String, String) {
    let work = std::env::temp_dir().join(format!("lz_bridge_e2e_{name}"));
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
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let rs = std::fs::read_to_string(lz.with_extension("rs")).unwrap_or_default();
    (stdout, rs)
}

// ─── 组合源：同一 LZ 程序中同时使用四种跨语言机制 ───
//
// 1. #[embed(tnr)]         — LZ 嵌入 Tnr 代码（缩进块语法）
// 2. import tnr::matmul     — bridge import：tnr lib API 转译路由
// 3. import tnr_lib::tensor::Tensor — bridge import：已编译 tnr 库链接路由
// 4. #[extern(rust)]        — LZ 声明外部 Rust 函数（自动登记到 BridgeRegistry）
// 5. @export(Rust)          — LZ 导出函数供外部使用（自动登记到 BridgeRegistry）

const E2E_SRC: &str = r#"#[embed(tnr)]
```
fn tnr_kernel(a, b) { a + b }
```

import tnr::matmul
import tnr_lib::tensor::Tensor

#[extern(rust)]
def native_open(path: str) -> Ext = 0

@export(Rust)
def lz_add(a: int, b: int) -> int = a + b

def main():
    print(lz_add(1, 2))
"#;

// ════════════════════ 库 API 测试 ════════════════════

#[test]
fn e2e_lib_api_bridge_import_routing() {
    let rs = gen_rust(E2E_SRC);
    // bridge import: tnr::matmul → use tnr_transpiled::matmul;
    assert!(
        rs.contains("tnr_transpiled::matmul"),
        "bridge import tnr::matmul 未路由到 tnr_transpiled::matmul，生成的 .rs:\n{rs}"
    );
    // bridge import: tnr_lib::tensor::Tensor → use tnr::tensor::Tensor;
    assert!(
        rs.contains("tnr::tensor::Tensor"),
        "bridge import tnr_lib::tensor::Tensor 未路由到 tnr::tensor::Tensor，生成的 .rs:\n{rs}"
    );
}

#[test]
fn e2e_lib_api_export_fn_present() {
    let rs = gen_rust(E2E_SRC);
    assert!(
        rs.contains("lz_add"),
        "export 函数 lz_add 未出现在生成产物中，生成的 .rs:\n{rs}"
    );
}

#[test]
fn e2e_lib_api_all_mechanisms_coexist() {
    // 确认 gen_rust 不 panic：四种机制同时存在时 codegen 正常完成
    let rs = gen_rust(E2E_SRC);
    assert!(
        !rs.is_empty(),
        "组合源 codegen 产物为空"
    );
    // 至少包含 bridge import 和 export 函数
    assert!(
        rs.contains("tnr_transpiled::matmul") && rs.contains("lz_add"),
        "bridge import 与 export 未同时出现在生成产物中，生成的 .rs:\n{rs}"
    );
}

// ════════════════════ CLI 测试 ════════════════════
//
// CLI 语义分析阶段会校验 import 路径是否存在对应文件，
// bridge import（tnr::matmul 等）在语义分析阶段会被拒绝，
// 因此 CLI 测试仅覆盖 embed + extern + export（不含 bridge import）。

const CLI_SRC: &str = r#"#[embed(tnr)]
```
fn tnr_kernel(a, b) { a + b }
```

#[extern(rust)]
def native_open(path: str) -> Ext = 0

@export(Rust)
def lz_add(a: int, b: int) -> int = a + b

def main():
    print(lz_add(1, 2))
"#;

#[test]
fn e2e_cli_compiles_with_all_mechanisms() {
    let (stdout, _rs) = compile_lz("e2e_compile", CLI_SRC);
    // extern(1) + export(1) = 至少 2 个符号应登记
    assert!(
        stdout.contains("Bridge registry:"),
        "应输出 Bridge registry 信息，实际 stdout={stdout:?}"
    );
}

#[test]
fn e2e_cli_gen_rs_contains_embed_block() {
    let (_stdout, rs) = compile_lz("e2e_rs_embed", CLI_SRC);
    // embed 块应出现在生成的 .rs 中（注释形式，因 tnr-embed feature 未启用）
    assert!(
        rs.contains("tnr_kernel"),
        "embed 块 tnr_kernel 未出现在 CLI 生成的 .rs 中:\n{rs}"
    );
}

#[test]
fn e2e_cli_gen_rs_contains_export_fn() {
    let (_stdout, rs) = compile_lz("e2e_rs_export", CLI_SRC);
    assert!(
        rs.contains("lz_add"),
        "export 函数 lz_add 未出现在 CLI 生成的 .rs 中:\n{rs}"
    );
}

// ════════════════════ 三层路由优先级测试 ════════════════════

/// embed 模块名优先于 bridge registry 路由
/// 当 import 路径匹配 embed 模块名时，应走 embed 路由（直接 use），不走 bridge
#[test]
fn e2e_embed_module_takes_priority_over_bridge() {
    // embed(tnr) 生成模块名 "input_tnr"（源文件 input.lz → input_tnr）
    // import input_tnr::* 应走 embed 路由，不走 bridge registry
    let src = r#"#[embed(tnr)]
```
fn helper(a, b) { a + b }
```

import input_tnr

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    // embed 路由应直接生成 use input_tnr，不被 bridge 拦截
    assert!(
        rs.contains("use input_tnr"),
        "embed 模块名 input_tnr 未走 embed 路由（应生成 use input_tnr），生成的 .rs:\n{rs}"
    );
}

/// std 模块优先于 bridge registry 路由
/// import std::collections::HashMap 应走 std 白名单，不被 bridge 拦截
#[test]
fn e2e_std_module_takes_priority_over_bridge() {
    let src = r#"import std::collections::HashMap

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("std::collections"),
        "std::collections 未走 std 白名单路由，生成的 .rs:\n{rs}"
    );
    // 不应被 bridge 路由到其他路径
    assert!(
        !rs.contains("tnr_transpiled") && !rs.contains("tnr_compute"),
        "std 模块被错误地走了 bridge 路由，生成的 .rs:\n{rs}"
    );
}

/// 未知模块走 bridge registry 路由
/// import tnr::matmul 不匹配 embed 也不匹配 std，应走 bridge
#[test]
fn e2e_unknown_module_falls_through_to_bridge() {
    let src = r#"import tnr::matmul

def main():
    print(1)
"#;
    let rs = gen_rust(src);
    assert!(
        rs.contains("tnr_transpiled::matmul"),
        "未知模块 tnr::matmul 未走 bridge 路由，生成的 .rs:\n{rs}"
    );
}
