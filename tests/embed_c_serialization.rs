// 方向②回归锁：C 适配器 str 标量对齐 + 容器/Dict-KV 显式护栏。
// 本测试用 **真实 gcc 编译 + 运行** 生成的分发入口，证明：
//   1. `const char *` str 形参/返回的 C 函数经分发入口可被 key=value 调用，
//      输出单行 `OK:<值>`（与 Python/JS 标量闭集同口径）；
//   2. C 无法原生表达的容器形参不静默丢弃，运行分发入口得到显式
//      `ERR:unsupported param type` 回执。
// 无 gcc 时优雅 Skip（不谎报通过）。

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn have_gcc() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn fixture_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lz-c-embed-{}-{}", tag, std::process::id()));
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 从 C 适配器取分发入口源码（走 public 路径）。
fn dispatch_body(artifact: &str, c_src: &str) -> String {
    use lang_zone::embed::EmbedLang as _;
    let a = lang_zone::embed::adapter("c");
    let ms = a.methods(c_src);
    let refs: Vec<&lang_zone::embed::ForeignFn> = ms.iter().collect();
    let (_path, body) = a
        .dispatch_file(artifact, &refs)
        .expect("C dispatch_file 应有产物");
    body
}

#[test]
fn c_str_param_and_return_dispatch_runs_via_gcc() {
    if !have_gcc() {
        eprintln!("SKIP: gcc 不可用，跳过 C 真编译验证");
        return;
    }
    let dir = fixture_dir("str");
    // 被测 C 源：str 形参 + str 返回
    let c_src = "const char *greet(const char *name) { return name; }\n";
    let dispatch = dispatch_body("c_str", c_src);

    let src_c = dir.join("c_str.c");
    fs::write(&src_c, c_src).unwrap();
    let src_disp = dir.join("__lz_dispatch_c_str.c");
    fs::write(&src_disp, &dispatch).unwrap();

    let exe = dir.join("c_str.exe");
    let cc = Command::new("gcc")
        .arg("-O2")
        .arg("-o")
        .arg(&exe)
        .arg(&src_disp)
        .arg(&src_c)
        .output()
        .expect("gcc 应可执行");
    assert!(
        cc.status.success(),
        "C 分发入口应能 gcc 编译（0 error）：\nstderr={}",
        String::from_utf8_lossy(&cc.stderr)
    );

    // 运行：greet name=hi
    let run = Command::new(&exe)
        .arg("greet")
        .arg("name=hi")
        .output()
        .expect("运行分发入口");
    assert!(run.status.success(), "分发入口应成功退出");
    let out = String::from_utf8_lossy(&run.stdout);
    assert_eq!(out.trim(), "OK:hi", "str 返回应为 OK:hi，实得：{:?}", out.trim());

    // 换行转义：name 含转义仅作为单行协议验证（argv 不传换行），此处只证 OK 前缀
    assert!(out.starts_with("OK:"), "应单行 OK: 协议");
}

#[test]
fn c_unsupported_container_param_guards_instead_of_silent_drop() {
    if !have_gcc() {
        eprintln!("SKIP: gcc 不可用，跳过 C 真编译验证");
        return;
    }
    use lang_zone::embed::EmbedLang as _;
    use lang_zone::embed::{ForeignFn, ForeignParam};
    let dir = fixture_dir("guard");

    // 人为构造带容器形参的签名（C 源无法直接声明，闭集判定走同一 dispatch 路径）
    let ms = vec![ForeignFn {
        name: "sum_all".to_string(),
        params: vec![ForeignParam {
            name: "nums".to_string(),
            lz_type: "list<int>".to_string(),
        }],
        ret: Some("int".to_string()),
        unsupported: None,
    }];
    let a = lang_zone::embed::adapter("c");
    let refs: Vec<&ForeignFn> = ms.iter().collect();
    let (_, dispatch) = a.dispatch_file("c_guard", &refs).unwrap();

    // 提供一个同名桩实现，让分发入口能编译链接
    let stub = "int sum_all(int x) { return x; }\n";
    let src_c = dir.join("c_guard.c");
    fs::write(&src_c, stub).unwrap();
    let src_disp = dir.join("__lz_dispatch_c_guard.c");
    fs::write(&src_disp, &dispatch).unwrap();

    let exe = dir.join("c_guard.exe");
    let cc = Command::new("gcc")
        .arg("-O2")
        .arg("-o")
        .arg(&exe)
        .arg(&src_disp)
        .arg(&src_c)
        .output()
        .expect("gcc 应可执行");
    assert!(
        cc.status.success(),
        "护栏臂应能编译：\nstderr={}",
        String::from_utf8_lossy(&cc.stderr)
    );

    let run = Command::new(&exe)
        .arg("sum_all")
        .arg("nums=[1,2,3]")
        .output()
        .expect("运行分发入口");
    let out = String::from_utf8_lossy(&run.stdout);
    assert!(
        out.contains("ERR:unsupported param type"),
        "容器形参应得显式护栏回执，实得：{:?}",
        out.trim()
    );
}
