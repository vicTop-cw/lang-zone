// embed §六.1 可序列化闭集扩展 —— JS 适配器测试
// 验证 JS embed 产物的函数摘取、分发入口生成、JSON 序列化逻辑正确

use std::path::PathBuf;
use std::process::Command;

/// 单元测试：JS adapter 注册与基本属性
#[test]
fn js_adapter_registered_and_basic_attrs() {
    use lang_zone::embed::adapters::adapter;

    let a = adapter("js");
    assert_eq!(a.lang(), "js");
    assert_eq!(a.src_ext(), "js");

    let a2 = adapter("javascript");
    assert_eq!(a2.lang(), "js");
}

/// 端到端测试：LZ 程序调用 JS embed 函数，验证编译通过
/// （JS 为解释型语言，build_cmd 返回 None，仅验证 lang-zone 编译 + 产物落盘）
#[test]
fn js_embed_scalar_param_compiles() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_js_serialization")
        .join("scalar");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    // JS embed 源：接收 int，返回 int
    let lz = work.join("calc.lz");
    let src = r#"
#[embed(js)]
# js_lib.js
function add(a: int, b: int) { return a + b; }

def main():
    result = add(3, 4)
    print(result)
"#;
    std::fs::write(&lz, src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(&work)
        .arg(&lz)
        .arg("--embed-build")
        .output()
        .expect("run lang-zone");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        out.status.success(),
        "lang-zone 编译失败: stdout={}\nstderr={}",
        stdout, stderr
    );

    // 验证没有 "embed 未覆盖" 回执（JS int 签名应已被支持）
    assert!(
        !stdout.contains("embed 未覆盖"),
        "JS int 签名不应被标记为未覆盖: {}",
        stdout
    );
}

/// 单元测试：JS dispatch_file 生成的代码包含 JSON 编解码逻辑
#[test]
fn js_dispatch_includes_json_for_containers() {
    use lang_zone::embed::adapters::adapter;
    use lang_zone::embed::ForeignFn;

    let lang = adapter("js");
    let src = "function double_all(nums: List<int>) { return nums.map(n => n * 2); }\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1, "应摘出 1 个函数");
    let m = &methods[0];
    assert_eq!(m.name, "double_all");
    assert!(m.callable(), "List<int> 签名应可序列化");
    assert_eq!(m.params.len(), 1);

    let refs: Vec<&ForeignFn> = methods.iter().collect();
    let (path, body) = lang.dispatch_file("js_calc", &refs).expect("dispatch_file should produce output");

    assert_eq!(path, "__lz_dispatch_js_calc.js");
    // Node.js 分发入口特征
    assert!(body.contains("'use strict'"));
    assert!(body.contains("ARTIFACT = \"js_calc\""));
    assert!(body.contains("JSON.parse"));
    assert!(body.contains("JSON.stringify"));
    assert!(body.contains("\"double_all\""));
    assert!(body.contains("__lz_json_list"));
}

/// 单元测试：JS 容器返回值走 JSON.stringify，标量走 String()
#[test]
fn js_dispatch_container_ret_uses_json_stringify() {
    use lang_zone::embed::adapters::adapter;
    use lang_zone::embed::ForeignFn;

    let lang = adapter("js");
    // JS 无 -> 返回注解语法，ret 一律 None；验证 dispatch 仍能生成
    let src = "function get_list() { return [1, 2, 3]; }\n";
    let methods = lang.methods(src);
    assert_eq!(methods.len(), 1);
    assert!(methods[0].callable());

    let refs: Vec<&ForeignFn> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("js_ret", &refs).unwrap();
    // __lz_ok 对容器用 JSON.stringify，标量用 String()
    assert!(body.contains("Array.isArray(v)"));
    assert!(body.contains("JSON.stringify(v)"));
    assert!(body.contains("String(v)"));
}

/// 单元测试：JS has_main 检测多种入口形态
#[test]
fn js_has_main_detects_entry_forms() {
    use lang_zone::embed::adapters::adapter;

    let a = adapter("js");
    assert!(a.has_main("function main() { console.log(1); }\n"));
    assert!(a.has_main("if (require.main === module) { run(); }\n"));
    assert!(a.has_main("if (require.main == module) { run(); }\n"));
    assert!(!a.has_main("function add(a: int, b: int) { return a + b; }\n"));
}

/// 单元测试：JS run_cmd 使用 node
#[test]
fn js_run_cmd_uses_node_binary() {
    use lang_zone::embed::adapters::adapter;
    use lang_zone::embed::ArtifactKind;

    let a = adapter("js");
    let ex = a.run_cmd(ArtifactKind::Executable, "calc.js");
    assert_eq!(ex.program, "node");
    assert_eq!(ex.args, vec!["calc.js"]);

    let lib = a.run_cmd(ArtifactKind::Library, "calc.js");
    assert_eq!(lib.program, "node");
    assert_eq!(lib.args, vec!["__lz_dispatch_calc.js"]);
}

/// 单元测试：JS build_cmd 为 None（解释型语言无建库步骤）
#[test]
fn js_build_cmd_is_none_for_interpreted() {
    use lang_zone::embed::adapters::adapter;
    use lang_zone::embed::ArtifactKind;

    let a = adapter("js");
    assert!(a.build_cmd(ArtifactKind::Library, "calc.js").is_none());
    assert!(a.build_cmd(ArtifactKind::Executable, "calc.js").is_none());
}
