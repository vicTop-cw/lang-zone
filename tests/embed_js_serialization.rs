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

// ──────────────── T0r64：JS 适配器产物「真跑」回归（node --check + 运行） ────────────────
//
// 背景：T0r63 的 JS 测试只断言 `body.contains(...)` 子串，产物实际是 **非法 JS**
// （dispatch 发成 Rust 形态 `if method === "x" {`、PARAMS 发成 Rust 元组、
// artifact 保留 `# 注释`、函数未 module.exports、标量类型匹配错词汇 ⇒ `add(3,4)` 得 `"34"`）。
// 这一组用 `node --check` 做语法判据、用 `node <dispatch>` 做运行判据，堵住「字符串命中≠可运行」。

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn node_check(path: &std::path::Path) -> Result<(), String> {
    let out = Command::new("node")
        .arg("--check")
        .arg(path)
        .output()
        .map_err(|e| format!("spawn node: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "node --check {} 失败：{}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

/// 端到端：编译 LZ（含标量 + 容器函数），对产物做 node --check + 运行比对。
/// 判据覆盖 T0r64 修的五个面：语法有效、`#` 注释转换、module.exports、类型转换、容器 JSON。
#[test]
fn js_embedded_artifacts_are_valid_and_run_correctly() {
    if !node_available() {
        eprintln!("跳过：本机无 node");
        return;
    }
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_js_serialization")
        .join("runtime");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc.lz");
    let src = r#"
#[embed(js)]
# js_lib.js
function add(a: int, b: int) { return a + b; }
function sum_all(nums: List<int>) { return nums.reduce((a, b) => a + b, 0); }
function make_list() { return [1, 2, 3]; }

def main():
    print("runtime demo")
"#;
    std::fs::write(&lz, src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(&work)
        .arg(&lz)
        .arg("--embed-build")
        .output()
        .expect("run lang-zone");
    assert!(
        out.status.success(),
        "lang-zone 编译失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let art_dir = work.join("build/embed/calc/js_calc");
    let artifact = art_dir.join("js_calc.js");
    let dispatch = art_dir.join("__lz_dispatch_js_calc.js");
    assert!(artifact.is_file(), "缺 artifact: {}", artifact.display());
    assert!(dispatch.is_file(), "缺 dispatch: {}", dispatch.display());

    let art_src = std::fs::read_to_string(&artifact).unwrap();

    // ① 产物不得残留 `#` 行注释（JS 无此语法）
    for line in art_src.lines() {
        assert!(
            !line.trim_start().starts_with('#'),
            "artifact 残留 `#` 行注释（非法 JS）：{line}"
        );
    }
    // ② 顶层函数必须显式 module.exports（否则 dispatch require 取到 undefined）
    assert!(
        art_src.contains("module.exports"),
        "artifact 缺 module.exports：\n{art_src}"
    );
    // ③ 形参类型注解必须剥除（JS 无 `a: int`）
    assert!(
        art_src.contains("function add(a, b)"),
        "形参类型注解未剥除：\n{art_src}"
    );
    assert!(
        !art_src.contains("a: int") && !art_src.contains("nums: List<int>"),
        "artifact 仍含 LZ 类型注解（非法 JS）：\n{art_src}"
    );

    // ④ 两个文件都必须过 node --check
    node_check(&artifact).unwrap_or_else(|e| panic!("{e}\n---\n{art_src}"));
    let dispatch_src = std::fs::read_to_string(&dispatch).unwrap();
    node_check(&dispatch).unwrap_or_else(|e| panic!("{e}\n---\n{dispatch_src}"));

    // ⑤ dispatch 不得含 Rust 形态残留
    assert!(
        !dispatch_src.contains("if method ==="),
        "dispatch 残留 Rust 形态 `if method ===`（缺括号）：\n{dispatch_src}"
    );
    assert!(
        dispatch_src.contains("if (method === \"add\")"),
        "dispatch 缺合法 JS 条件 arms：\n{dispatch_src}"
    );

    // ⑥ 运行判据：标量 3+4=7、容器求和 10、容器返回 JSON、help 列表
    let run = |args: &[&str]| -> String {
        let o = Command::new("node")
            .current_dir(&art_dir)
            .arg("__lz_dispatch_js_calc.js")
            .args(args)
            .output()
            .expect("run dispatch");
        assert!(o.status.success(), "dispatch 非零退出：{:?}", o.status);
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    assert_eq!(run(&["add", "a=3", "b=4"]), "7", "标量参数未做数值转换");
    assert_eq!(
        run(&["sum_all", "nums=[1,2,3,4]"]),
        "10",
        "容器参数未走 JSON.parse"
    );
    assert_eq!(run(&["make_list"]), "[1,2,3]", "容器返回值未走 JSON.stringify");
    assert_eq!(run(&["help"]), "OK:add,sum_all,make_list", "help 列表错误");
}

// ──────────────── T0r65：JS 适配器**嵌套容器**运行时回归 ────────────────
//
// 背景：T0r64 只锁定了标量 + 扁平 `list<int>` 的运行判据；JS dispatch 里
// `_ if is_nested_container(&p.lz_type) => "__lz_json_nested"` 这条分支（adapters.rs）
// 一直没有端到端护栏。本组用**真实编译 + node 运行**锁住嵌套容器的 roundtrip：
//   dict<str,list<int>> / list<list<int>> / dict<str,dict<str,int>>。
// 判据口径：`node __lz_dispatch_*.js <method> key=<json>` 的 stdout 必须等于期望值，
// 而不是断言生成的源码字符串里含某个子串（那正是 T0r63→T0r64 暴露的「字符串命中≠可运行」教训）。

/// 端到端：LZ 程序含 `#[embed(js)]`，承载三类嵌套容器函数；编译后对每个方法
/// 用 node 跑分发入口，比对运行输出。参数以 **Command::arg 逐字传入**（不经 shell，
/// 避免 PowerShell 吃掉 JSON 里的双引号 —— 本机实测 shell 会 strip `"`）。
#[test]
fn js_nested_containers_run_correctly_end_to_end() {
    if !node_available() {
        eprintln!("跳过：本机无 node");
        return;
    }
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_js_serialization")
        .join("nested");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc.lz");
    // 三类嵌套形态：
    //   group_counts(Dict<str,List<int>>) —— dict<str,list<int>>
    //   sum_nested(List<List<int>>)       —— list<list<int>>
    //   flat_keys(Dict<str,Dict<str,int>>) —— dict<str,dict<str,int>>（深层嵌套）
    // 逐个用 JS 侧 reduce / 遍历求和，返回值均为标量 int（便于断言）。
    let src = r#"
#[embed(js)]
# js_lib.js
function group_counts(groups: Dict<str,List<int>>) { let t = 0; for (const k in groups) { t += groups[k].reduce((a,b)=>a+b,0); } return t; }
function sum_nested(matrix: List<List<int>>) { let t = 0; for (const row of matrix) { for (const v of row) t += v; } return t; }
function flat_keys(m: Dict<str,Dict<str,int>>) { let t = 0; for (const k in m) { for (const k2 in m[k]) t += m[k][k2]; } return t; }

def main():
    print("nested runtime demo")
"#;
    std::fs::write(&lz, src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(&work)
        .arg(&lz)
        .arg("--embed-build")
        .output()
        .expect("run lang-zone");
    assert!(
        out.status.success(),
        "lang-zone 编译失败: stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // 三类嵌套容器签名都不得被回执点名「不在闭集内」
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("embed 未覆盖"),
        "嵌套容器签名不应被标记为未覆盖: {stdout}"
    );

    let art_dir = work.join("build/embed/calc/js_calc");
    let artifact = art_dir.join("js_calc.js");
    let dispatch = art_dir.join("__lz_dispatch_js_calc.js");
    assert!(dispatch.is_file(), "缺 dispatch: {}", dispatch.display());

    // 生成物必须过 node --check（语法有效）
    node_check(&artifact).unwrap_or_else(|e| panic!("{e}"));
    node_check(&dispatch).unwrap_or_else(|e| panic!("{e}"));

    // dispatch 的 PARAMS 表必须把三类嵌套类型都挂到 __lz_json_nested
    let dispatch_src = std::fs::read_to_string(&dispatch).unwrap();
    assert!(
        dispatch_src.contains("\"group_counts\": [[\"groups\", __lz_json_nested]]"),
        "group_counts 未走 __lz_json_nested:\n{dispatch_src}"
    );
    assert!(
        dispatch_src.contains("\"sum_nested\": [[\"matrix\", __lz_json_nested]]"),
        "sum_nested 未走 __lz_json_nested:\n{dispatch_src}"
    );
    assert!(
        dispatch_src.contains("\"flat_keys\": [[\"m\", __lz_json_nested]]"),
        "flat_keys 未走 __lz_json_nested:\n{dispatch_src}"
    );

    // 运行判据：Command::arg 逐字传参（不经 shell），JSON 里的 `"` 得以保留。
    let run = |args: &[&str]| -> String {
        let o = Command::new("node")
            .current_dir(&art_dir)
            .arg("__lz_dispatch_js_calc.js")
            .args(args)
            .output()
            .expect("run dispatch");
        assert!(
            o.status.success(),
            "dispatch 非零退出：{:?} stderr={}",
            o.status,
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };

    // dict<str,list<int>>：{"a":[1,2,3],"b":[4,5]} → 6+9 = 15
    assert_eq!(
        run(&["group_counts", r#"groups={"a":[1,2,3],"b":[4,5]}"#]),
        "15",
        "dict<str,list<int>> 嵌套容器参数 roundtrip 失败"
    );
    // list<list<int>>：[[1,2],[3,4]] → 10
    assert_eq!(
        run(&["sum_nested", "matrix=[[1,2],[3,4]]"]),
        "10",
        "list<list<int>> 嵌套容器参数 roundtrip 失败"
    );
    // dict<str,dict<str,int>>：{"a":{"x":1,"y":2},"b":{"z":3}} → 6
    assert_eq!(
        run(&["flat_keys", r#"m={"a":{"x":1,"y":2},"b":{"z":3}}"#]),
        "6",
        "dict<str,dict<str,int>> 深层嵌套参数 roundtrip 失败"
    );
}

/// 端到端：JS 嵌套容器的**边界与方向**覆盖 —— 空容器、内层空容器、
/// 嵌套容器**返回值**（JSON.stringify 侧）、list<dict<str,int>> 参数形态。
/// 与上一组同一编译口径（真编译 + node 运行），逐条比对 stdout。
#[test]
fn js_nested_containers_edge_and_return_forms() {
    if !node_available() {
        eprintln!("跳过：本机无 node");
        return;
    }
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_js_serialization")
        .join("nested_edge");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let lz = work.join("calc.lz");
    // 三类边界/方向：
    //   group_counts(Dict<str,List<int>>) —— 空 dict / 内层空 list
    //   make_nested()                     —— 嵌套容器**返回值**
    //   list_of_dicts(List<Dict<str,int>>)—— list<dict<..>> 参数形态
    let src = r#"
#[embed(js)]
# js_lib.js
function group_counts(groups: Dict<str,List<int>>) { let t = 0; for (const k in groups) { t += groups[k].reduce((a,b)=>a+b,0); } return t; }
function make_nested() { return {"a":[1,2],"b":[3]}; }
function list_of_dicts(xs: List<Dict<str,int>>) { let t = 0; for (const d of xs) { for (const k in d) t += d[k]; } return t; }

def main():
    print("nested edge demo")
"#;
    std::fs::write(&lz, src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(&work)
        .arg(&lz)
        .arg("--embed-build")
        .output()
        .expect("run lang-zone");
    assert!(
        out.status.success(),
        "lang-zone 编译失败: stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let art_dir = work.join("build/embed/calc/js_calc");
    let dispatch = art_dir.join("__lz_dispatch_js_calc.js");
    assert!(dispatch.is_file(), "缺 dispatch: {}", dispatch.display());
    node_check(&dispatch).unwrap_or_else(|e| panic!("{e}"));

    let run = |args: &[&str]| -> String {
        let o = Command::new("node")
            .current_dir(&art_dir)
            .arg("__lz_dispatch_js_calc.js")
            .args(args)
            .output()
            .expect("run dispatch");
        assert!(
            o.status.success(),
            "dispatch 非零退出：{:?} stderr={}",
            o.status,
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };

    // 边界①：空 dict（内层无元素）→ 0
    assert_eq!(run(&["group_counts", "groups={}"]), "0", "空 dict 嵌套参数失败");
    // 边界②：内层空 list 与空 list 混合 → 0+1+5 = 6
    assert_eq!(
        run(&["group_counts", r#"groups={"a":[],"b":[1],"c":[2,3]}"#]),
        "6",
        "内层空 list 混合参数失败"
    );
    // 方向③：嵌套容器**返回值**走 JSON.stringify（对象序为插入序）
    assert_eq!(
        run(&["make_nested"]),
        r#"{"a":[1,2],"b":[3]}"#,
        "嵌套容器返回值未走 JSON.stringify"
    );
    // 形态④：list<dict<str,int>> 参数 → 1+2+3 = 6
    assert_eq!(
        run(&["list_of_dicts", r#"xs=[{"a":1},{"b":2},{"c":3}]"#]),
        "6",
        "list<dict<str,int>> 参数 roundtrip 失败"
    );
}

