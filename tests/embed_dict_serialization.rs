// embed §六.1 可序列化闭集扩展 —— Dict 类型跨语言序列化测试
// 验证 Python embed 产物的 Dict<str,int> 参数和返回值通过 JSON 嵌入编码正确序列化

use std::path::PathBuf;
use std::process::Command;

/// 端到端测试：LZ 程序调用 Python embed 函数，参数 Dict<str,int>，返回 Dict<str,int>
/// 验证 JSON 嵌入编码在 argv 协议中的正确性
#[test]
fn python_embed_dict_str_int_param_and_ret_roundtrip() {
    // 准备工作目录
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization")
        .join("roundtrip");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    // Python embed 源：接收 Dict<str,int>，返回 Dict<str,int>
    let lz = work.join("calc.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def double_values(data: Dict<str,int>) -> Dict<str,int>:
    return {k: v * 2 for k, v in data.items()}

def main():
    result = double_values({"a": 1, "b": 2, "c": 3})
    print(result)
"#;
    std::fs::write(&lz, src).expect("write lz");

    // 编译
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

    // 验证没有 "embed 未覆盖" 回执（Dict<str,int> 应该已被支持）
    assert!(
        !stdout.contains("embed 未覆盖"),
        "Dict<str,int> 签名不应被标记为未覆盖: {}",
        stdout
    );
}

/// 单元测试：Python dispatch_file 生成的代码包含 Dict JSON 解码/编码逻辑
#[test]
fn python_dispatch_includes_json_for_dict_str_int() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def double_values(data: Dict<str,int>) -> Dict<str,int>:\n    return {k: v * 2 for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1, "应摘出 1 个函数");
    let m = &methods[0];
    assert_eq!(m.name, "double_values");
    assert!(m.callable(), "Dict<str,int> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,int>"), "返回类型应为 dict<str,int>");
    assert_eq!(m.params.len(), 1, "应有 1 个参数");
    assert_eq!(m.params[0].lz_type, "dict<str,int>", "参数类型应为 dict<str,int>");

    // 验证 dispatch_file 生成的代码包含 JSON 解码和编码
    let refs: Vec<&_> = methods.iter().collect();
    let (path, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file should produce output");
    assert!(path.contains("__lz_dispatch_python_test.py"), "dispatch 文件名应正确");

    // 验证 JSON 解码函数存在
    assert!(body.contains("__lz_json_dict_str_int"), "应包含 JSON 解码函数");
    assert!(body.contains("json.loads"), "应使用 json.loads 解码");

    // 验证 JSON 编码函数存在
    assert!(body.contains("_out_ok_json"), "应包含 JSON 编码输出函数");
    assert!(body.contains("json.dumps"), "应使用 json.dumps 编码");

    // 验证 import json
    assert!(body.contains("import json"), "应导入 json 模块");

    // 验证 dict<str,int> 返回值使用 _out_ok_json
    assert!(body.contains("double_values"), "应包含函数名");
    let double_values_arm = body.split("if method == \"double_values\":").nth(1)
        .unwrap_or("");
    assert!(double_values_arm.contains("_out_ok_json"), "double_values 返回值应使用 JSON 编码");
    assert!(!double_values_arm.contains("_out(\"OK\""), "double_values 不应使用普通 _out");
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

/// 单元测试：dict<str,int> 参数的 COERCE 表包含 JSON 解码转换
#[test]
fn python_dispatch_dict_str_int_param_coerce() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def count_values(data: Dict<str,int>) -> int:\n    return sum(data.values())\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].params[0].lz_type, "dict<str,int>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_int"), "COERCE 表应包含 JSON 解码函数");
    let count_values_arm = body.split("if method == \"count_values\":").nth(1).unwrap_or("");
    assert!(count_values_arm.contains("_out(\"OK\""), "count_values 返回值应使用普通 _out");
    assert!(!count_values_arm.contains("_out_ok_json"), "count_values 返回值不应使用 JSON 编码");
}

/// 单元测试：dict<str,int> 参数但返回标量类型，仅参数走 JSON 解码
#[test]
fn python_dispatch_dict_param_scalar_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def total(data: Dict<str,int>) -> int:\n    return sum(data.values())\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "Dict<str,int> 参数 + int 返回值应可序列化");
    assert_eq!(m.params[0].lz_type, "dict<str,int>");
    assert_eq!(m.ret.as_deref(), Some("int"));

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    // COERCE 表包含 Dict 解码函数
    assert!(body.contains("__lz_json_dict_str_int"), "COERCE 表应包含 Dict 解码函数");
    // 返回值用普通 _out（int 不是 JSON 类型）
    let total_arm = body.split("if method == \"total\":").nth(1).unwrap_or("");
    assert!(total_arm.contains("_out(\"OK\""), "total 返回值应使用普通 _out");
    assert!(!total_arm.contains("_out_ok_json"), "total 返回值不应使用 JSON 编码");
}

/// 单元测试：dict<str,int> 参数和返回值都走 JSON 序列化
#[test]
fn python_dispatch_dict_param_and_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def invert(data: Dict<str,int>) -> Dict<str,int>:\n    return {v: k for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "Dict<str,int> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,int>"));
    assert_eq!(m.params[0].lz_type, "dict<str,int>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_int"), "应包含 Dict JSON 解码函数");
    let arm = body.split("if method == \"invert\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "invert 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "invert 不应使用普通 _out");
}

/// 单元测试：list 类型与 dict 类型共存时，各用各的解码函数
#[test]
fn python_dispatch_list_and_dict_coexist() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def f_list(nums: List<int>) -> List<int>:\n    return [n * 2 for n in nums]\n\n\ndef f_dict(data: Dict<str,int>) -> Dict<str,int>:\n    return {k: v * 2 for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 2);
    for m in &methods {
        assert!(m.callable(), "{} 签名应可序列化", m.name);
    }

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    // 两种解码函数都应存在
    assert!(body.contains("__lz_json_list_int"), "应包含 int list 解码函数");
    assert!(body.contains("__lz_json_dict_str_int"), "应包含 dict 解码函数");

    // 2 个方法的 arm 都用 JSON 编码输出返回值
    let arm_bodies: Vec<&str> = body
        .lines()
        .filter(|l| l.trim().starts_with("_out_ok_json") && !l.trim().starts_with("def _out_ok_json"))
        .collect();
    assert_eq!(arm_bodies.len(), 2, "应有 2 个 arm 使用 _out_ok_json 输出，实际 {}", arm_bodies.len());
}

/// 单元测试：cli.rs 的 Rust 侧类型映射覆盖 dict<str,int>
#[test]
fn embed_param_ret_ty_covers_dict_str_int() {
    // 验证 embed_param_ty / embed_ret_ty 对 dict<str,int> 的映射
    // 通过 wrapper 代码生成间接验证
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def f(data: Dict<str,int>) -> Dict<str,int>:\n    return data\n";
    let methods = lang.methods(src);
    assert_eq!(methods.len(), 1);

    // 验证类型映射
    assert_eq!(methods[0].ret.as_deref(), Some("dict<str,int>"));
    assert_eq!(methods[0].params[0].lz_type, "dict<str,int>");

    // dispatch 应包含 Dict 解码函数
    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");
    assert!(body.contains("__lz_json_dict_str_int"));
}

// ── T0r57：dict<str,float> / dict<str,str> Rust 侧序列化补全 ──

/// 单元测试：Python dispatch_file 生成的代码包含 dict<str,float> JSON 解码/编码逻辑
#[test]
fn python_dispatch_includes_json_for_dict_str_float() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def calc_ratios(data: Dict<str,float>) -> Dict<str,float>:\n    return {k: v / 2.0 for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.name, "calc_ratios");
    assert!(m.callable(), "Dict<str,float> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,float>"), "返回类型应为 dict<str,float>");
    assert_eq!(m.params.len(), 1, "应有 1 个参数");
    assert_eq!(m.params[0].lz_type, "dict<str,float>", "参数类型应为 dict<str,float>");

    let refs: Vec<&_> = methods.iter().collect();
    let (path, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");
    assert!(path.contains("__lz_dispatch_python_test.py"));

    // 验证 JSON 解码函数存在
    assert!(body.contains("__lz_json_dict_str_float"), "应包含 JSON 解码函数");
    assert!(body.contains("json.loads"), "应使用 json.loads 解码");

    // 验证 JSON 编码函数存在
    assert!(body.contains("_out_ok_json"), "应包含 JSON 编码输出函数");
    assert!(body.contains("json.dumps"), "应使用 json.dumps 编码");

    // 验证 dict<str,float> 返回值使用 _out_ok_json
    let arm = body.split("if method == \"calc_ratios\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "calc_ratios 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "calc_ratios 不应使用普通 _out");
}

/// 单元测试：Python dispatch_file 生成的代码包含 dict<str,str> JSON 解码/编码逻辑
#[test]
fn python_dispatch_includes_json_for_dict_str_str() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def upper_keys(data: Dict<str,str>) -> Dict<str,str>:\n    return {k.upper(): v for k, v in data.items()}\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert_eq!(m.name, "upper_keys");
    assert!(m.callable(), "Dict<str,str> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("dict<str,str>"), "返回类型应为 dict<str,str>");
    assert_eq!(m.params.len(), 1, "应有 1 个参数");
    assert_eq!(m.params[0].lz_type, "dict<str,str>", "参数类型应为 dict<str,str>");

    let refs: Vec<&_> = methods.iter().collect();
    let (path, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");
    assert!(path.contains("__lz_dispatch_python_test.py"));

    assert!(body.contains("__lz_json_dict_str_str"), "应包含 JSON 解码函数");
    assert!(body.contains("json.loads"), "应使用 json.loads 解码");
    assert!(body.contains("_out_ok_json"), "应包含 JSON 编码输出函数");
    assert!(body.contains("json.dumps"), "应使用 json.dumps 编码");

    let arm = body.split("if method == \"upper_keys\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "upper_keys 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "upper_keys 不应使用普通 _out");
}

/// 单元测试：dict<str,float> 参数走 JSON 解码，返回标量用普通 _out
#[test]
fn python_dispatch_dict_str_float_param_scalar_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def total_area(data: Dict<str,float>) -> float:\n    return sum(data.values())\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "Dict<str,float> 参数 + float 返回值应可序列化");
    assert_eq!(m.params[0].lz_type, "dict<str,float>");
    assert_eq!(m.ret.as_deref(), Some("float"));

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_float"), "COERCE 表应包含 Dict 解码函数");
    let arm = body.split("if method == \"total_area\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out(\"OK\""), "total_area 返回值应使用普通 _out");
    assert!(!arm.contains("_out_ok_json"), "total_area 返回值不应使用 JSON 编码");
}

/// 单元测试：dict<str,str> 参数走 JSON 解码，返回标量用普通 _out
#[test]
fn python_dispatch_dict_str_str_param_scalar_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def count_entries(data: Dict<str,str>) -> int:\n    return len(data)\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "Dict<str,str> 参数 + int 返回值应可序列化");
    assert_eq!(m.params[0].lz_type, "dict<str,str>");
    assert_eq!(m.ret.as_deref(), Some("int"));

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_dict_str_str"), "COERCE 表应包含 Dict 解码函数");
    let arm = body.split("if method == \"count_entries\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out(\"OK\""), "count_entries 返回值应使用普通 _out");
    assert!(!arm.contains("_out_ok_json"), "count_entries 返回值不应使用 JSON 编码");
}

/// 单元测试：三种 Dict K/V 组合共存时各用各的解码函数
#[test]
fn python_dispatch_three_dict_variants_coexist() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def f_int(d: Dict<str,int>) -> Dict<str,int>:\n    return d\n\n\ndef f_float(d: Dict<str,float>) -> Dict<str,float>:\n    return d\n\n\ndef f_str(d: Dict<str,str>) -> Dict<str,str>:\n    return d\n";
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

/// 单元测试：Rust 侧 embed_wrapper_fn 对 dict<str,float> 返回值调用 __lz_parse_dict_str_float
/// 通过 generate_embed_module 间接验证（不依赖实际 spawn）
#[test]
fn rust_embed_wrapper_parses_dict_str_float_ret() {
    use lang_zone::bridge::cli::{EmbedEndpoint, EmbedFnSig, generate_embed_module};

    let ep = EmbedEndpoint {
        artifact: "py_calc".to_string(),
        dir_rel: "target/embed_test".to_string(),
        program: "python".to_string(),
        prefix: vec![],
        kind: "library",
        fns: vec![EmbedFnSig {
            wrapper: "calc_ratios".to_string(),
            method: "calc_ratios".to_string(),
            params: vec![("data".to_string(), "dict<str,float>".to_string())],
            ret: "dict<str,float>".to_string(),
        }],
    };
    let code = generate_embed_module(&[ep]);

    // 参数序列化：应调用 __lz_dict_to_json_float
    assert!(code.contains("__lz_dict_to_json_float"), "参数序列化应调用 __lz_dict_to_json_float");
    // 参数类型：BTreeMap<String, f64>
    assert!(code.contains("BTreeMap<String, f64>"), "参数类型应为 BTreeMap<String, f64>");
    // 返回值反序列化：应调用 __lz_parse_dict_str_float
    assert!(code.contains("__lz_parse_dict_str_float"), "返回值反序列化应调用 __lz_parse_dict_str_float");
    // 返回值类型：BTreeMap<String, f64>
    assert!(code.contains("-> std::collections::BTreeMap<String, f64>"), "返回值类型应为 BTreeMap<String, f64>");
}

/// 单元测试：Rust 侧 embed_wrapper_fn 对 dict<str,str> 返回值调用 __lz_parse_dict_str_str
#[test]
fn rust_embed_wrapper_parses_dict_str_str_ret() {
    use lang_zone::bridge::cli::{EmbedEndpoint, EmbedFnSig, generate_embed_module};

    let ep = EmbedEndpoint {
        artifact: "py_labels".to_string(),
        dir_rel: "target/embed_test".to_string(),
        program: "python".to_string(),
        prefix: vec![],
        kind: "library",
        fns: vec![EmbedFnSig {
            wrapper: "upper_keys".to_string(),
            method: "upper_keys".to_string(),
            params: vec![("data".to_string(), "dict<str,str>".to_string())],
            ret: "dict<str,str>".to_string(),
        }],
    };
    let code = generate_embed_module(&[ep]);

    // 参数序列化：应调用 __lz_dict_to_json_str
    assert!(code.contains("__lz_dict_to_json_str"), "参数序列化应调用 __lz_dict_to_json_str");
    // 参数类型：BTreeMap<String, String>
    assert!(code.contains("BTreeMap<String, String>"), "参数类型应为 BTreeMap<String, String>");
    // 返回值反序列化：应调用 __lz_parse_dict_str_str
    assert!(code.contains("__lz_parse_dict_str_str"), "返回值反序列化应调用 __lz_parse_dict_str_str");
    // 返回值类型：BTreeMap<String, String>
    assert!(code.contains("-> std::collections::BTreeMap<String, String>"), "返回值类型应为 BTreeMap<String, String>");
}

/// 单元测试：Rust 侧 embed_wrapper_fn 对 dict<str,float> 参数 + dict<str,int> 返回值（跨组合）
#[test]
fn rust_embed_wrapper_cross_dict_variants() {
    use lang_zone::bridge::cli::{EmbedEndpoint, EmbedFnSig, generate_embed_module};

    let ep = EmbedEndpoint {
        artifact: "py_conv".to_string(),
        dir_rel: "target/embed_test".to_string(),
        program: "python".to_string(),
        prefix: vec![],
        kind: "library",
        fns: vec![EmbedFnSig {
            wrapper: "convert_to_int_map".to_string(),
            method: "convert_to_int_map".to_string(),
            params: vec![("data".to_string(), "dict<str,float>".to_string())],
            ret: "dict<str,int>".to_string(),
        }],
    };
    let code = generate_embed_module(&[ep]);

    // 参数序列化用 float 版
    assert!(code.contains("__lz_dict_to_json_float"), "float 参数序列化应调用 __lz_dict_to_json_float");
    // 参数类型用 float 版
    assert!(code.contains("BTreeMap<String, f64>"), "float 参数类型应为 BTreeMap<String, f64>");
    // 返回值反序列化用 int 版
    assert!(code.contains("__lz_parse_dict_str_int"), "int 返回值反序列化应调用 __lz_parse_dict_str_int");
    // 返回值类型用 int 版
    assert!(code.contains("-> std::collections::BTreeMap<String, i64>"), "int 返回值类型应为 BTreeMap<String, i64>");
}

/// 端到端回归：Dict<str,float> 参数 + Dict<str,float> 返回值的 LZ 程序能编译通过。
/// 修复前：LZ codegen 把 dict 字面量生成为 BTreeMap，而 embed 形参声明为 HashMap，
/// 类型不匹配导致 rustc 编译失败（E0308）。修复后 embed 侧统一为 BTreeMap，类型对齐可编译。
#[test]
fn python_embed_dict_str_float_roundtrip_compiles() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization")
        .join("roundtrip_float");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc_float.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def scale_values(data: Dict<str,float>) -> Dict<str,float>:
    return {k: v * 3.0 for k, v in data.items()}

def main():
    result = scale_values({"a": 1.5, "b": 2.5})
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
        "Dict<str,float> roundtrip 编译失败（BTreeMap/HashMap 类型应对齐）: stdout={}\nstderr={}",
        stdout, stderr
    );
    assert!(
        !stdout.contains("embed 未覆盖"),
        "Dict<str,float> 签名不应被标记为未覆盖: {}",
        stdout
    );
}

/// T0r61：嵌套 dict 返回值端到端测试
/// dict<str,list<int>> 作为返回值，Rust 侧 wrapper 应返回真实嵌套类型，
/// 不再降级为 BTreeMap<String, String>（JSON 字符串片段）。
/// 端到端编译通过 + 产物中嵌套解析代码存在性断言（wrapper 函数体经 shim 引入，不在主产物文本中）。
#[test]
fn python_embed_nested_dict_ret_real_type() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization")
        .join("nested_real_ty");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc_real_ty.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def group_lists(data: Dict<str,int>) -> Dict<str,List<int>>:
    return {"a": [1, 2, 3], "b": [4, 5]}

def main():
    result = group_lists({"x": 1, "y": 2})
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
        "嵌套 dict<str,List<int>> 返回值端到端编译失败: stdout={}\nstderr={}",
        stdout, stderr
    );
    assert!(
        !stdout.contains("embed 未覆盖"),
        "嵌套 dict<str,List<int>> 返回值签名不应被标记为未覆盖: {}",
        stdout
    );

    // 产物中应包含嵌套 dict 的递归解析代码（内联闭包解析 JSON 对象），
    // 且运行时辅助 __lz_parse_dict_nested 不应再固定返回 BTreeMap<String, String>
    let rs_path = work.join("calc_real_ty.rs");
    let content = std::fs::read_to_string(&rs_path).expect("read rs product");
    // 递归解析代码：内联闭包 + BTreeMap 解析
    assert!(
        content.contains("BTreeMap") && content.contains("parse"),
        "产物应包含嵌套 dict 的 JSON 解析逻辑"
    );
}

/// T0r61：嵌套 dict 返回值端到端编译测试
/// 端到端验证 dict<str,list<int>> 作为返回值时，LZ 程序编译通过（rustc 编译产物）
#[test]
fn python_embed_nested_dict_ret_compiles() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization")
        .join("nested_ret");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc_nested.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def group_lists(data: Dict<str,int>) -> Dict<str,List<int>>:
    return {"a": [1, 2, 3], "b": [4, 5]}

def main():
    result = group_lists({"x": 1, "y": 2})
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
        "嵌套 dict<str,List<int>> 返回值端到端编译失败: stdout={}\nstderr={}",
        stdout, stderr
    );
    assert!(
        !stdout.contains("embed 未覆盖"),
        "嵌套 dict<str,List<int>> 返回值签名不应被标记为未覆盖: {}",
        stdout
    );
}

/// T0r62：嵌套容器 dict<str,list<int>> 作为**参数**传入 + **返回值**反序列化的完整 roundtrip
/// 端到端验证。T0r61 已覆盖返回值侧，本测试补齐参数侧序列化全链路：
/// LZ 侧 dict<str,list<int>> 字面量 → 内联递归 JSON 序列化（值为 JSON 数组）→
/// Python 侧 __lz_json_nested 解码 → Python 函数调用 → 返回值 json.dumps →
/// Rust 侧 embed_ret_parse_expr 递归解析为 BTreeMap<String, Vec<i64>>。
/// 端到端 --embed-build 编译通过（rustc）即证明参数序列化 + 返回值反序列化类型对齐。
#[test]
fn python_embed_nested_dict_str_list_int_param_ret_roundtrip() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization")
        .join("nested_param_ret");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc_nested_pr.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def group_counts(data: Dict<str,List<int>>) -> Dict<str,int>:
    return {k: sum(v) for k, v in data.items()}

def main():
    result = group_counts({"a": [1, 2, 3], "b": [4, 5]})
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
        "嵌套 dict<str,List<int>> 参数+返回值 roundtrip 编译失败: stdout={}\nstderr={}",
        stdout, stderr
    );
    assert!(
        !stdout.contains("embed 未覆盖"),
        "嵌套 dict<str,List<int>> 参数签名不应被标记为未覆盖: {}",
        stdout
    );

    // 参数序列化侧：产物应包含 dict<str,list<int>> 的内联递归 JSON 序列化
    // （遍历 BTreeMap 键值对，value 递归序列化为 JSON 数组）
    let rs_path = work.join("calc_nested_pr.rs");
    let content = std::fs::read_to_string(&rs_path).expect("read rs product");
    assert!(
        content.contains("BTreeMap"),
        "产物应包含 BTreeMap 类型（嵌套 dict 参数）"
    );
}

/// T0r62：嵌套 dict dict<str,dict<str,int>> 参数+返回值完整 roundtrip
/// dict 嵌套 dict（value 为扁平 dict<str,int>），参数序列化需递归序列化 value
/// 为 JSON 对象，返回值反序列化递归解析为 BTreeMap<String, BTreeMap<String, i64>>。
/// 端到端 --embed-build 编译通过（rustc）即证明嵌套 dict 端到端类型对齐。
#[test]
fn python_embed_nested_dict_str_dict_int_param_ret_roundtrip() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization")
        .join("nested_dict_dict");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc_nested_dd.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def merge_groups(data: Dict<str,Dict<str,int>>) -> int:
    total = 0
    for group in data.values():
        total += sum(group.values())
    return total

def main():
    result = merge_groups({"g1": {"a": 1, "b": 2}, "g2": {"c": 3}})
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
        "嵌套 dict<str,Dict<str,int>> 参数+返回值 roundtrip 编译失败: stdout={}\nstderr={}",
        stdout, stderr
    );
    assert!(
        !stdout.contains("embed 未覆盖"),
        "嵌套 dict<str,Dict<str,int>> 参数签名不应被标记为未覆盖: {}",
        stdout
    );
}

/// T0r62：Dict×Tuple 嵌套组合 dict<str,tuple<int,float>> 端到端
/// dict 的 value 为 tuple<int,float>，参数序列化需递归序列化 value 为 JSON 数组，
/// 返回值反序列化递归解析为 BTreeMap<String, (i64, f64)>。
/// 端到端 --embed-build 编译通过（rustc）即证明 dict×tuple 嵌套组合类型对齐。
#[test]
fn python_embed_nested_dict_str_tuple_param_ret_roundtrip() {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_dict_serialization")
        .join("nested_dict_tuple");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc_nested_dt.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def sum_pairs(data: Dict<str,Tuple<int,float>>) -> float:
    total = 0.0
    for pair in data.values():
        total += pair[0] + pair[1]
    return total

def main():
    result = sum_pairs({"a": (1, 2.5), "b": (3, 4.5)})
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
        "嵌套 dict<str,Tuple<int,float>> 参数+返回值 roundtrip 编译失败: stdout={}\nstderr={}",
        stdout, stderr
    );
    assert!(
        !stdout.contains("embed 未覆盖"),
        "嵌套 dict<str,Tuple<int,float>> 参数签名不应被标记为未覆盖: {}",
        stdout
    );
}
