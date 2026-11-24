// embed Scala 适配器回归锁（T0r17）：**真编译 + 真运行**。
//
// 背景：ScalaLang::dispatch_file 以前恒返回 None（「JVM 侧入口需要 object 包装，
// 本版本不生成」），于是 scala 库产物的所有方法都被 surface_gated 挡下，
// LZ 侧压根调不到。并且本机没有可分发的 scala-library jar
// （Coursier 缓存里全是 0 字节占位）——「能编译、跑不起来」。
//
// 本测试钉三件事：
//   1. dispatch_file 现在真的产出入口（不再 None）；
//   2. 产物里**不出现** `scala.Array` 静态字段与 `sys.exit`/`return` 早退
//      （这两处会拉 scala/Array$、scala/runtime/Nothing$ ⇒ 无 runtime 必崩）；
//   3. 用路径上的 scalac 真编译、路径上的 java 真运行，逐条比对 `OK:<值>`。
//
// 工具链不在场时优雅 SKIP（不谎报通过）。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("-version")
        .output()
        .map(|o| o.status.success() || !o.stderr.is_empty() || !o.stdout.is_empty())
        .unwrap_or(false)
}

fn fixture(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "lz-scala-embed-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).expect("create fixture dir");
    d
}

/// 被测 Scala 源（顶格 `def`，与 Mojo 同形；返回类型是 Scala 的 `): T =` 形态）。
const LIB_SRC: &str = "def add(a: Long, b: Long): Long = a + b\n\
def greet(name: String): String = \"hi \" + name\n\
def ratio(x: Double): Double = x\n\
def is_ok(flag: Boolean): Boolean = flag\n";

/// 走 public 路径取 (产物文件名, 产物正文)。
fn scala_dispatch(artifact: &str, src: &str) -> Option<(String, String)> {
    use lang_zone::embed::EmbedLang as _;
    let a = lang_zone::embed::adapter("scala");
    let ms = a.methods(src);
    let refs: Vec<&lang_zone::embed::ForeignFn> = ms.iter().collect();
    a.dispatch_file(artifact, &refs)
}

#[test]
fn scala_dispatch_file_is_generated_not_none() {
    let d = scala_dispatch("scala_calc", LIB_SRC);
    let (path, body) = d.expect(
        "Scala 适配器应生成分发入口（T0r17 前恒为 None ⇒ 所有方法被挡成不可调用）",
    );
    assert!(path.contains("dispatch"), "产物路径应含 dispatch: {path}");
    assert!(path.ends_with(".scala"), "产物应为 .scala: {path}");
    // 四个方法都要有臂
    for m in ["add", "greet", "ratio", "is_ok"] {
        assert!(body.contains(&format!("\"{}\"", m)), "缺 {m} 的分发臂:\n{body}");
    }
    assert!(body.contains("OK:"), "应走单行 OK: 协议");
    assert!(body.contains("ERR:missing method"), "缺方法参数时应显式报错");
    assert!(body.contains("help"), "应有 help 臂列出已知方法");
}

#[test]
fn scala_dispatch_avoids_scala_runtime_symbols() {
    let (_, body) = scala_dispatch("scala_rt", LIB_SRC).expect("应有分发入口");
    // 只看**代码行**，跳过注释（模板头部的注释本身就在解释「为什么不用 scala.Array」）
    let code: String = body
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//") && !t.starts_with("*") && !t.starts_with("/*")
        })
        .collect::<Vec<_>>()
        .join("\n");
    // 致命坑 1：静态字段用 Array.empty[String] ⇒ <clinit> 拉 scala/Array$
    assert!(
        !code.contains("Array.empty"),
        "分发入口不得在静态初始化里构造 scala 数组:\n{code}"
    );
    assert!(
        !code.contains("scala.Array") && !code.contains("scala.runtime"),
        "分发入口不得引用 scala 运行时类型:\n{code}"
    );
    // 致命坑 2：sys.exit / 早退 return 会拉 scala/runtime/Nothing$
    assert!(!code.contains("sys.exit"), "不得用 scala 的 sys.exit:\n{code}");
    assert!(!code.contains("scala.sys"), "不得引用 scala.sys:\n{code}");
    // 退出码必须走 java.lang.System.exit
    assert!(
        code.contains("java.lang.System.exit"),
        "退出应走 java.lang.System.exit:\n{code}"
    );
    // 字符串拼数值必须显式 toString（裸 `"OK:" + Long` 会拉 BoxesRunTime）
    assert!(
        !code.contains("\"OK:\" + Lib."),
        "不得把返回值直接拼到字符串上（会拉 scala.runtime.BoxesRunTime）:\n{code}"
    );
    // stdlib 名称冲突规避：入口自己声明 argvGet/__lz_bool/__lz_escape
    for helper in ["argvGet", "__lz_bool", "__lz_escape"] {
        assert!(code.contains(helper), "缺辅助函数 {helper}:\n{code}");
    }
}

#[test]
fn scala_packaging_is_idempotent_when_source_already_has_package() {
    use lang_zone::embed::EmbedLang as _;
    let a = lang_zone::embed::adapter("scala");
    let own = "package my.own\nobject Lib {\n  def f(x: Long): Long = x\n}\n";
    let files = a.extract("own_pkg", own);
    assert_eq!(files.len(), 1);
    let text = &files[0].1;
    // 作者已自带 package ⇒ 不再强行包入（避免双 package 非法）
    assert_eq!(
        text.matches("package ").count(),
        1,
        "已有 package 的源不应被二次包入:\n{text}"
    );
    assert!(text.contains("package my.own"), "{text}");
}

#[test]
fn scala_unpackaged_source_is_wrapped_into_object_lib() {
    // Scala 没有顶层函数 ⇒ 解析函数必须收进 object Lib，分发入口的 `Lib.<name>(...)` 才解析得到
    use lang_zone::embed::EmbedLang as _;
    let a = lang_zone::embed::adapter("scala");
    let files = a.extract("scala_wrap", LIB_SRC);
    let text = &files[0].1;
    assert!(text.contains("package lz_embed_scala_wrap"), "{text}");
    assert!(text.contains("object Lib {"), "缺 object Lib 包装:\n{text}");
    assert!(
        text.contains("  def add(a: Long, b: Long): Long = a + b"),
        "函数体应缩进 2 空格留在 object Lib 内:\n{text}"
    );
    // 缩进后的定义不再是「顶格 def」⇒ methods() 摘取仍以原文为准，不受包装影响
    assert_eq!(a.methods(LIB_SRC).len(), 4, "包装不改摘取口径");
}

#[test]
fn scala_ret_type_is_parsed_from_colon_form() {
    // Scala 的返回类型是 `): T =` 形态（冒号），不是 `->`。
    // 共用摘取器 `fn_from_toplevel` 只认 `->`，漏认的后果是整个返回值被当成 None
    // ⇒ 分发入口打印裸 `OK:`（T0r17 实测）。这条锁钉住 ret_sep=":" 的接线。
    use lang_zone::embed::EmbedLang as _;
    let a = lang_zone::embed::adapter("scala");
    let ms = a.methods(LIB_SRC);
    let by: std::collections::HashMap<&str, Option<&str>> = ms
        .iter()
        .map(|m| (m.name.as_str(), m.ret.as_deref()))
        .collect();
    assert_eq!(by.get("add"), Some(&Some("int")), "add 返回 Long → int");
    assert_eq!(by.get("greet"), Some(&Some("str")), "greet 返回 String → str");
    assert_eq!(by.get("ratio"), Some(&Some("float")), "ratio 返回 Double → float");
    assert_eq!(by.get("is_ok"), Some(&Some("bool")), "is_ok 返回 Boolean → bool");
}

#[test]
fn scala_dispatch_output_is_consumed_with_hygienic_names() {
    // 方法名/形参名不应被机械改名（LZ 侧按源码名调用）
    let src = "def add(a: Long, b: Long): Long = a + b\n";
    let (_, body) = scala_dispatch("scala_names", src).expect("应有分发入口");
    assert!(body.contains("Lib.add(a, b)"), "调用应保留源码形参名:\n{body}");
    assert!(body.contains("argvGet(args, \"a\")"), "应从 argv 取 a:\n{body}");
    assert!(body.contains("argvGet(args, \"b\")"), "应从 argv 取 b:\n{body}");
    // 返回值必须显式走 java.lang.*.toString —— 直接字符串拼 Long 会落到
    // scala.runtime.BoxesRunTime（无 runtime 时 NoClassDefFoundError）
    assert!(
        body.contains("java.lang.Long.toString(Lib.add(a, b))"),
        "int 返回值应显式 java.lang.Long.toString:\n{body}"
    );
}

/// 真编译 + 真运行：scalac 编出 classes/，再用 java 起分发 object。
#[test]
fn scala_dispatch_compiles_and_runs_under_bare_java() {
    if !have("scalac") || !have("java") {
        eprintln!("SKIP: scalac / java 不可用，跳过 Scala 真编译验证");
        return;
    }
    use lang_zone::embed::EmbedLang as _;
    let a = lang_zone::embed::adapter("scala");
    let files = a.extract("scala_calc", LIB_SRC);
    let (lib_name, lib_text) = files.into_iter().next().expect("应有 lib 文件");
    let (disp_name, disp_text) = scala_dispatch("scala_calc", LIB_SRC).expect("应有分发入口");

    let dir = fixture("run");
    let lib_path: PathBuf = dir.join(&lib_name);
    let disp_path: PathBuf = dir.join(&disp_name);
    fs::write(&lib_path, &lib_text).unwrap();
    fs::write(&disp_path, &disp_text).unwrap();
    let classes = dir.join("classes");
    fs::create_dir_all(&classes).unwrap();

    // 包名：`scala_calc` → `lz_embed_scala_calc`（与 scala_pkg_of 同一把尺）
    let pkg = "lz_embed_scala_calc";

    let cc = Command::new("scalac")
        .current_dir(&dir)
        .arg("-d")
        .arg("classes")
        .arg(&disp_name)
        .arg(&lib_name)
        .output()
        .expect("scalac 应可执行");
    assert!(
        cc.status.success(),
        "Scala 分发入口应能 scalac 编译（0 error）：\nstderr={}\n--- lib ---\n{}\n--- dispatch ---\n{}",
        String::from_utf8_lossy(&cc.stderr),
        lib_text,
        disp_text
    );

    let entry = format!("{pkg}.__lz_scala_calc");
    let run = |args: &[&str]| -> String {
        let o = Command::new("java")
            .current_dir(&dir)
            .arg("-cp")
            .arg("classes")
            .arg(&entry)
            .args(args)
            .output()
            .expect("java 应可执行");
        assert!(
            o.status.success(),
            "分发入口运行失败（args={:?}）:\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };

    assert_eq!(run(&["add", "a=3", "b=4"]), "OK:7", "int 形参/返回");
    assert_eq!(run(&["greet", "name=bob"]), "OK:hi bob", "str 形参/返回");
    assert_eq!(run(&["ratio", "x=2.5"]), "OK:2.5", "float 形参/返回");
    assert_eq!(run(&["is_ok", "flag=true"]), "OK:true", "bool 形参/返回");
    assert_eq!(run(&["is_ok", "flag=false"]), "OK:false", "bool 负向");
    assert_eq!(
        run(&["help"]),
        "OK:add,greet,ratio,is_ok",
        "help 应列出全部可调用方法"
    );

    // 未知方法：非 0 退出且 ERR: 回执（不静默成功）
    let bad = Command::new("java")
        .current_dir(&dir)
        .arg("-cp")
        .arg("classes")
        .arg(&entry)
        .arg("nope")
        .output()
        .expect("java 应可执行");
    assert!(!bad.status.success(), "未知方法应非 0 退出");
    assert!(
        String::from_utf8_lossy(&bad.stdout).contains("ERR:no such method"),
        "未知方法应有显式 ERR 回执"
    );
}

/// resolve() 层的整车判据：scala 库产物现在应当 **有** 分发入口，
/// 可调用方法不再被 NO_DISPATCH_SURFACE 挡下。
#[test]
fn scala_library_target_has_dispatch_surface() {
    if !have("java") {
        eprintln!("SKIP: java 不可用（scala 零 runtime 分发需 java）");
        return;
    }
    let t = lang_zone::embed::resolve("scala", "calc.lz", LIB_SRC).expect("应解析出 target");
    assert_eq!(t.kind, lang_zone::embed::ArtifactKind::Library);
    assert!(
        t.has_dispatch_file(),
        "scala 库产物应有分发入口（此前恒无 ⇒ 调用面为空）"
    );
    let callable = t.callable_methods();
    assert_eq!(
        callable.len(),
        4,
        "四个闭集内方法都应有调用面，实得: {:?}",
        callable.iter().map(|m| m.name.clone()).collect::<Vec<_>>()
    );
}

#[allow(dead_code)]
fn _unused(_p: &Path) {}