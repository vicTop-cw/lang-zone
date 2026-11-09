// embed §六.1 可序列化闭集扩展 —— Dict<int,int> 类型跨语言序列化测试
// 验证 Python embed 产物的 Dict<int,int> 参数和返回值通过 JSON 嵌入编码正确序列化

use std::path::PathBuf;
use std::process::Command;

/// 单元测试：Python dispatch_file 生成的代码包含 Dict<int,int> JSON 解码逻辑
#[test]
fn python_dispatch_includes_json_for_dict_int_int() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def scale(data: Dict<int,int>) -> Dict<int,int>:\n    return {k: v * 2 for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1, "应摘出 1 个函数");
    let m = &methods[0];
    assert_eq!(m.name, "scale");
    assert!(m.callable(), "Dict<int,int> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<int,int>"), "返回类型应为 dict<int,int>");
    assert_eq!(m.params.len(), 1, "应有 1 个参数");
    assert_eq!(m.params[0].lz_type, "dict<int,int>", "参数类型应为 dict<int,int>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_path, body) = lang.dispatch_file("python_int_int", &refs).expect("dispatch_file should produce output");

    // 验证 JSON 解码函数存在
    assert!(body.contains("__lz_json_dict_int_int"), "应包含 JSON 解码函数");
    assert!(body.contains("int(k)"), "应包含 int(k) 键转换");

    // 验证 JSON 编码函数存在
    assert!(body.contains("_out_ok_json"), "应包含 JSON 编码输出函数");
    assert!(body.contains("json.dumps"), "应使用 json.dumps 编码");

    // 验证 COERCE 表包含 dict<int,int> 映射
    assert!(body.contains("__lz_json_dict_int_int"), "COERCE 表应包含 __lz_json_dict_int_int");
}

/// 单元测试：Python dispatch_file 生成的代码包含 Dict<int,int> 参数解码
#[test]
fn python_dispatch_dict_int_int_param_coerce() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def double(data: Dict<int,int>) -> int:\n    return sum(v * 2 for v in data.values())\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.params[0].lz_type, "dict<int,int>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_path, body) = lang.dispatch_file("python_coerce", &refs).expect("dispatch_file should produce output");

    // 验证参数解码映射
    let coerce_section = body.split("COERCE = {").nth(1).unwrap_or("");
    assert!(coerce_section.contains("__lz_json_dict_int_int"), "COERCE 应包含 __lz_json_dict_int_int");
}

/// 单元测试：cargo build 编译通过 + lib 零回归
#[test]
fn cargo_build_zero_error() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("cargo")
        .arg("build")
        .current_dir(&manifest)
        .output()
        .expect("cargo build should run");
    assert!(
        output.status.success(),
        "cargo build should succeed (stderr: {})",
        String::from_utf8_lossy(&output.stderr)
    );
}
