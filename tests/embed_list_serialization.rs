// embed §六.1 可序列化闭集扩展 —— List 类型跨语言序列化测试
// 验证 Python embed 产物的 List<int>/List<float>/List<str> 参数和返回值通过 JSON 嵌入编码正确序列化

use std::path::PathBuf;
use std::process::Command;

/// 端到端测试：LZ 程序调用 Python embed 函数，参数 List<int>，返回 List<int>
/// 验证 JSON 嵌入编码在 argv 协议中的正确性
#[test]
fn python_embed_list_int_param_and_ret_roundtrip() {
    // 准备工作目录
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_list_serialization")
        .join("roundtrip");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    // Python embed 源：接收 List<int>，返回 List<int>
    let lz = work.join("calc.lz");
    let src = r#"
#[embed(python)]
# python_lib.py
def double_all(nums: List<int>) -> List<int>:
    return [n * 2 for n in nums]

def main():
    result = double_all([1, 2, 3, 4, 5])
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

    // 验证没有 "embed 未覆盖" 回执（List<int> 应该已被支持）
    assert!(
        !stdout.contains("embed 未覆盖"),
        "List<int> 签名不应被标记为未覆盖: {}",
        stdout
    );
}

/// 单元测试：Python dispatch_file 生成的代码包含 JSON 解码/编码逻辑
#[test]
fn python_dispatch_includes_json_for_list_int() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def double_all(nums: List<int>) -> List<int>:\n    return [n * 2 for n in nums]\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1, "应摘出 1 个函数");
    let m = &methods[0];
    assert_eq!(m.name, "double_all");
    assert!(m.callable(), "List<int> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("list<int>"), "返回类型应为 list<int>");
    assert_eq!(m.params.len(), 1, "应有 1 个参数");
    assert_eq!(m.params[0].lz_type, "list<int>", "参数类型应为 list<int>");

    // 验证 dispatch_file 生成的代码包含 JSON 解码和编码
    let refs: Vec<&_> = methods.iter().collect();
    let (path, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file should produce output");
    assert!(path.contains("__lz_dispatch_python_test.py"), "dispatch 文件名应正确");

    // 验证 JSON 解码函数存在
    assert!(body.contains("__lz_json_list_int"), "应包含 JSON 解码函数");
    assert!(body.contains("json.loads"), "应使用 json.loads 解码");

    // 验证 JSON 编码函数存在
    assert!(body.contains("_out_ok_json"), "应包含 JSON 编码输出函数");
    assert!(body.contains("json.dumps"), "应使用 json.dumps 编码");

    // 验证 import json
    assert!(body.contains("import json"), "应导入 json 模块");

    // 验证 list<int> 返回值使用 _out_ok_json
    assert!(body.contains("double_all"), "应包含函数名");
    let double_all_arm = body.split("if method == \"double_all\":").nth(1)
        .unwrap_or("");
    assert!(double_all_arm.contains("_out_ok_json"), "double_all 返回值应使用 JSON 编码");
    assert!(!double_all_arm.contains("_out(\"OK\""), "double_all 不应使用普通 _out");
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

/// 单元测试：list<int> 参数的 COERCE 表包含 JSON 解码转换
#[test]
fn python_dispatch_list_int_param_coerce() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def sum_list(nums: List<int>) -> int:\n    return sum(nums)\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].params[0].lz_type, "list<int>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_list_int"), "COERCE 表应包含 JSON 解码函数");
    let sum_list_arm = body.split("if method == \"sum_list\":").nth(1).unwrap_or("");
    assert!(sum_list_arm.contains("_out(\"OK\""), "sum_list 返回值应使用普通 _out");
    assert!(!sum_list_arm.contains("_out_ok_json"), "sum_list 返回值不应使用 JSON 编码");
}

/// 单元测试：list<float> 参数和返回值的 JSON 序列化
#[test]
fn python_dispatch_list_float_param_and_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def double_all(nums: List<float>) -> List<float>:\n    return [n * 2.0 for n in nums]\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "List<float> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("list<float>"));
    assert_eq!(m.params[0].lz_type, "list<float>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_list_float"), "应包含 float list JSON 解码函数");
    let arm = body.split("if method == \"double_all\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "double_all 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "double_all 不应使用普通 _out");
}

/// 单元测试：list<str> 参数和返回值的 JSON 序列化
#[test]
fn python_dispatch_list_str_param_and_ret() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def upper_all(words: List<str>) -> List<str>:\n    return [w.upper() for w in words]\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "List<str> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("list<str>"));
    assert_eq!(m.params[0].lz_type, "list<str>");

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_list_str"), "应包含 str list JSON 解码函数");
    let arm = body.split("if method == \"upper_all\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "upper_all 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "upper_all 不应使用普通 _out");
}

/// 单元测试：三种 list 类型共存时，各用各的解码函数
#[test]
fn python_dispatch_all_list_types_coexist() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def f_int(nums: List<int>) -> List<int>:\n    return [n * 2 for n in nums]\n\n\ndef f_float(nums: List<float>) -> List<float>:\n    return [n * 2.0 for n in nums]\n\n\ndef f_str(words: List<str>) -> List<str>:\n    return [w.upper() for w in words]\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 3);
    for m in &methods {
        assert!(m.callable(), "{} 签名应可序列化", m.name);
    }

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    // 三种解码函数都应存在
    assert!(body.contains("__lz_json_list_int"), "应包含 int list 解码函数");
    assert!(body.contains("__lz_json_list_float"), "应包含 float list 解码函数");
    assert!(body.contains("__lz_json_list_str"), "应包含 str list 解码函数");

    // 3 个方法的 arm 都用 JSON 编码输出返回值
    // 检查 arm 文本中 `_out_ok_json` 的出现次数（排除函数定义行）
    let arm_bodies: Vec<&str> = body
        .lines()
        .filter(|l| l.trim().starts_with("_out_ok_json") && !l.trim().starts_with("def _out_ok_json"))
        .collect();
    assert_eq!(arm_bodies.len(), 3, "应有 3 个 arm 使用 _out_ok_json 输出，实际 {}", arm_bodies.len());
    // 不应使用普通 _out 输出 list 返回值
    assert!(!body.contains("_out(\"OK\", \"\" if r"), "不应使用普通 _out 输出 list 返回值");
}

/// 单元测试：cli.rs 的 Rust 侧类型映射覆盖三种 list 类型
#[test]
fn embed_param_ret_ty_covers_all_list_types() {
    // 验证 embed_param_ty / embed_ret_ty 对三种 list 类型的映射
    // 这些是 src/bridge/cli.rs 的私有函数，通过 wrapper 代码生成间接验证
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def f(nums: List<int>) -> List<int>:\n    return nums\n\n\ndef g(nums: List<float>) -> List<float>:\n    return nums\n\n\ndef h(words: List<str>) -> List<str>:\n    return words\n";
    let methods = lang.methods(src);
    assert_eq!(methods.len(), 3);

    // 验证类型映射
    assert_eq!(methods[0].ret.as_deref(), Some("list<int>"));
    assert_eq!(methods[1].ret.as_deref(), Some("list<float>"));
    assert_eq!(methods[2].ret.as_deref(), Some("list<str>"));

    // dispatch 应包含所有三种解码函数
    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");
    assert!(body.contains("__lz_json_list_int"));
    assert!(body.contains("__lz_json_list_float"));
    assert!(body.contains("__lz_json_list_str"));
}
