// embed §六.1 可序列化闭集扩展 —— Dict<str,float>/Dict<str,str> 类型跨语言序列化测试
// 验证 Python embed 产物的 Dict<str,float> 和 Dict<str,str> 参数和返回值通过 JSON 嵌入编码正确序列化

use std::path::PathBuf;
use std::process::Command;

/// 单元测试：Python dispatch_file 生成的代码包含 Dict<str,float> JSON 解码/编码逻辑
#[test]
fn python_dispatch_includes_json_for_dict_str_float() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def scale_values(data: Dict<str,float>) -> Dict<str,float>:\n    return {k: v * 2.0 for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1, "应摘出 1 个函数");
    let m = &methods[0];
    assert_eq!(m.name, "scale_values");
    assert!(m.callable(), "Dict<str,float> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,float>"), "返回类型应为 dict<str,float>");
    assert_eq!(m.params.len(), 1, "应有 1 个参数");
    assert_eq!(m.params[0].lz_type, "dict<str,float>", "参数类型应为 dict<str,float>");

    let refs: Vec<&_> = methods.iter().collect();
    let (path, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file should produce output");
    assert!(path.contains("__lz_dispatch_python_test.py"), "dispatch 文件名应正确");

    // 验证 JSON 解码函数存在
    assert!(body.contains("__lz_json_dict_str_float"), "应包含 JSON 解码函数");
    assert!(body.contains("json.loads"), "应使用 json.loads 解码");

    // 验证 JSON 编码函数存在
    assert!(body.contains("_out_ok_json"), "应包含 JSON 编码输出函数");
    assert!(body.contains("json.dumps"), "应使用 json.dumps 编码");

    // 验证 import json
    assert!(body.contains("import json"), "应导入 json 模块");

    // 验证 dict<str,float> 返回值使用 _out_ok_json
    assert!(body.contains("scale_values"), "应包含函数名");
    let scale_values_arm = body.split("if method == \"scale_values\":").nth(1)
        .unwrap_or("");
    assert!(scale_values_arm.contains("_out_ok_json"), "scale_values 返回值应使用 JSON 编码");
    assert!(!scale_values_arm.contains("_out(\"OK\""), "scale_values 不应使用普通 _out");
}

/// 单元测试：Python dispatch_file 生成的代码包含 Dict<str,str> JSON 解码/编码逻辑
#[test]
fn python_dispatch_includes_json_for_dict_str_str() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def upper_values(data: Dict<str,str>) -> Dict<str,str>:\n    return {k: v.upper() for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1, "应摘出 1 个函数");
    let m = &methods[0];
    assert_eq!(m.name, "upper_values");
    assert!(m.callable(), "Dict<str,str> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,str>"), "返回类型应为 dict<str,str>");
    assert_eq!(m.params.len(), 1, "应有 1 个参数");
    assert_eq!(m.params[0].lz_type, "dict<str,str>", "参数类型应为 dict<str,str>");

    let refs: Vec<&_> = methods.iter().collect();
    let (path, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file should produce output");
    assert!(path.contains("__lz_dispatch_python_test.py"), "dispatch 文件名应正确");

    // 验证 JSON 解码函数存在
    assert!(body.contains("__lz_json_dict_str_str"), "应包含 JSON 解码函数");
    assert!(body.contains("json.loads"), "应使用 json.loads 解码");

    // 验证 JSON 编码函数存在
    assert!(body.contains("_out_ok_json"), "应包含 JSON 编码输出函数");
    assert!(body.contains("json.dumps"), "应使用 json.dumps 编码");

    // 验证 import json
    assert!(body.contains("import json"), "应导入 json 模块");

    // 验证 dict<str,str> 返回值使用 _out_ok_json
    assert!(body.contains("upper_values"), "应包含函数名");
    let upper_values_arm = body.split("if method == \"upper_values\":").nth(1)
        .unwrap_or("");
    assert!(upper_values_arm.contains("_out_ok_json"), "upper_values 返回值应使用 JSON 编码");
    assert!(!upper_values_arm.contains("_out(\"OK\""), "upper_values 不应使用普通 _out");
}

/// 单元测试：标量类型仍然使用原有协议（零回归）
#[test]
fn python_dispatch_scalar_types_unchanged() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def add(a: int, b: int) -> int:\n    return a + b\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    assert!(methods[0].callable());
    assert_eq!(methods[0].ret.as_deref(), Some("int"));

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    // 标量类型不应在 arm 中使用 JSON 编码
    let add_arm = body.split("if method == \"add\":").nth(1).unwrap_or("");
    assert!(add_arm.contains("_out(\"OK\""), "add 返回值应使用普通 _out");
    assert!(!add_arm.contains("_out_ok_json"), "add 返回值不应使用 JSON 编码");
    assert!(body.contains("import json"), "应导入 json 模块（辅助函数需要）");
}

/// 单元测试：dict<str,float> 参数的 COERCE 表包含 JSON 解码转换
#[test]
fn python_dispatch_dict_str_float_param_coerce() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def total_values(data: Dict<str,float>) -> float:\n    return sum(data.values())\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].params[0].lz_type, "dict<str,float>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_float"), "COERCE 表应包含 JSON 解码函数");
    let total_values_arm = body.split("if method == \"total_values\":").nth(1).unwrap_or("");
    assert!(total_values_arm.contains("_out(\"OK\""), "total_values 返回值应使用普通 _out");
    assert!(!total_values_arm.contains("_out_ok_json"), "total_values 返回值不应使用 JSON 编码");
}

/// 单元测试：dict<str,str> 参数的 COERCE 表包含 JSON 解码转换
#[test]
fn python_dispatch_dict_str_str_param_coerce() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def concat_values(data: Dict<str,str>) -> str:\n    return \".\".join(data.values())\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].params[0].lz_type, "dict<str,str>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_str"), "COERCE 表应包含 JSON 解码函数");
    let concat_values_arm = body.split("if method == \"concat_values\":").nth(1).unwrap_or("");
    assert!(concat_values_arm.contains("_out(\"OK\""), "concat_values 返回值应使用普通 _out");
    assert!(!concat_values_arm.contains("_out_ok_json"), "concat_values 返回值不应使用 JSON 编码");
}

/// 单元测试：dict<str,float> 参数和返回值都走 JSON 序列化
#[test]
fn python_dispatch_dict_str_float_param_and_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def invert_values(data: Dict<str,float>) -> Dict<str,float>:\n    return {v: k for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "Dict<str,float> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,float>"));
    assert_eq!(m.params[0].lz_type, "dict<str,float>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_float"), "应包含 Dict JSON 解码函数");
    let arm = body.split("if method == \"invert_values\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "invert_values 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "invert_values 不应使用普通 _out");
}

/// 单元测试：dict<str,str> 参数和返回值都走 JSON 序列化
#[test]
fn python_dispatch_dict_str_str_param_and_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def swap_values(data: Dict<str,str>) -> Dict<str,str>:\n    return {v: k for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "Dict<str,str> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,str>"));
    assert_eq!(m.params[0].lz_type, "dict<str,str>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_str"), "应包含 Dict JSON 解码函数");
    let arm = body.split("if method == \"swap_values\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "swap_values 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "swap_values 不应使用普通 _out");
}

/// 单元测试：三种 Dict 类型共存时，各用各的解码函数
#[test]
fn python_dispatch_three_dict_types_coexist() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def f_int(data: Dict<str,int>) -> Dict<str,int>:\n    return {k: v * 2 for k, v in data.items()}\n\n\ndef f_float(data: Dict<str,float>) -> Dict<str,float>:\n    return {k: v * 2.0 for k, v in data.items()}\n\n\ndef f_str(data: Dict<str,str>) -> Dict<str,str>:\n    return {k: v.upper() for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 3);
    for m in &methods {
        assert!(m.callable(), "{} 签名应可序列化", m.name);
    }

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    // 三种解码函数都应存在
    assert!(body.contains("__lz_json_dict_str_int"), "应包含 int dict 解码函数");
    assert!(body.contains("__lz_json_dict_str_float"), "应包含 float dict 解码函数");
    assert!(body.contains("__lz_json_dict_str_str"), "应包含 str dict 解码函数");

    // 3 个方法的 arm 都用 JSON 编码输出返回值
    let arm_bodies: Vec<&str> = body
        .lines()
        .filter(|l| l.trim().starts_with("_out_ok_json") && !l.trim().starts_with("def _out_ok_json"))
        .collect();
    assert_eq!(arm_bodies.len(), 3, "应有 3 个 arm 使用 _out_ok_json 输出，实际 {}", arm_bodies.len());
}

/// 端到端测试：LZ 程序调用 Python embed 函数，参数 Dict<str,float>，返回 Dict<str,float>
#[test]
fn python_embed_dict_str_float_param_and_ret_roundtrip() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization_ext")
        .join("roundtrip_float");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def scale_values(data: Dict<str,float>) -> Dict<str,float>:
    return {k: v * 2.0 for k, v in data.items()}

def main():
    result = scale_values({"a": 1.5, "b": 2.0, "c": 3.5})
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

    assert!(
        !stdout.contains("embed 未覆盖"),
        "Dict<str,float> 签名不应被标记为未覆盖: {}",
        stdout
    );
}

/// 端到端测试：LZ 程序调用 Python embed 函数，参数 Dict<str,str>，返回 Dict<str,str>
#[test]
fn python_embed_dict_str_str_param_and_ret_roundtrip() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization_ext")
        .join("roundtrip_str");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def upper_values(data: Dict<str,str>) -> Dict<str,str>:
    return {k: v.upper() for k, v in data.items()}

def main():
    result = upper_values({"a": "hello", "b": "world"})
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

    assert!(
        !stdout.contains("embed 未覆盖"),
        "Dict<str,str> 签名不应被标记为未覆盖: {}",
        stdout
    );
}
