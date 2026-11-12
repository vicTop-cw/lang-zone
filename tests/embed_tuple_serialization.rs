// Lang-Zone 编译器 — tests/embed_tuple_serialization.rs
// embed §六.1 可序列化闭集扩展 —— Tuple 组合两端闭环测试（T0r59）。
//
// 覆盖面：
//   ① 17 种 tuple 组合（16 种二元组 + tuple<int,int,int>）的签名摘取全部 callable；
//   ② Python dispatch 侧 JSON 解码函数 + _out_ok_json 返回臂；
//   ③ Rust 垫片双向：参数方向 to_json（具体元组类型）+ 返回方向 parse；
//   ④ LZ → Python 端到端 roundtrip：lang-zone --embed-build → rustc → 运行 stdout 比对。
//
// 背景修复（T0r59 前的真实缺口）：T0r57 只做了 Python 侧表项，Rust 侧
// embed_param_ty/embed_ret_ty/embed_arg_expr/embed_wrapper_fn 对 tuple 零支持 ——
// tuple 参数被当 &str 传、tuple 返回值被静默丢弃，且全仓 embed 测试零 tuple 覆盖。

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ── 产物目录解析（车道隔离，照 tests/embed_unify_r15.rs 的先例）───────────────

fn resolve_from_manifest(p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        repo_root().join(p)
    }
}

fn profile_dir_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let profile_dir = if dir.file_name().map_or(false, |n| n == "deps") {
        dir.parent()?
    } else {
        dir
    };
    if profile_dir.is_dir() {
        Some(profile_dir.to_path_buf())
    } else {
        None
    }
}

fn profile_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "debug".to_string())
}

fn build_profile_dir() -> PathBuf {
    if let Some(raw) = std::env::var_os("CARGO_TARGET_DIR") {
        let raw = PathBuf::from(raw);
        if !raw.as_os_str().is_empty() {
            let root = resolve_from_manifest(raw);
            if root.join("deps").is_dir() {
                return root;
            }
            let profile = profile_dir_from_exe()
                .map(|d| profile_name(&d))
                .unwrap_or_else(|| "debug".to_string());
            return root.join(profile);
        }
    }
    if let Some(dir) = profile_dir_from_exe() {
        return dir;
    }
    repo_root().join("target").join("debug")
}

fn build_target_root() -> PathBuf {
    let dir = build_profile_dir();
    dir.parent().map(|p| p.to_path_buf()).unwrap_or(dir)
}

fn newest_deps_rlib(dir: &Path) -> Option<PathBuf> {
    let deps = dir.join("deps");
    let entries = std::fs::read_dir(&deps).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if best.as_ref().map_or(true, |(t, _)| mtime > *t) {
                best = Some((mtime, e.path()));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn probe_rlib(dir: &Path) -> Option<PathBuf> {
    if let Some(p) = newest_deps_rlib(dir) {
        return Some(p);
    }
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return Some(direct);
    }
    None
}

fn builtins_rlib() -> PathBuf {
    let profile_dir = build_profile_dir();
    if let Some(p) = probe_rlib(&profile_dir) {
        return p;
    }
    let _ = std::fs::create_dir_all(&profile_dir);
    let profile = profile_name(&profile_dir);
    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(repo_root())
        .args(["build", "-p", "lz_builtins"])
        .arg("--target-dir")
        .arg(build_target_root());
    if profile != "debug" {
        cmd.arg("--profile").arg(&profile);
    }
    let built = cmd.output();
    if let Ok(b) = &built {
        if !b.status.success() {
            panic!(
                "lz_builtins 补建失败：{}\n若本机无 cargo，请手工产出 {}/liblz_builtins.rlib 后复跑",
                String::from_utf8_lossy(&b.stderr),
                profile_dir.display()
            );
        }
    }
    probe_rlib(&profile_dir).unwrap_or_else(|| {
        panic!(
            "已跑 cargo build -p lz_builtins（{:?}）仍在 {} 找不到产物 ⇒ 取数路径变了，改本行而非放宽判据",
            built.as_ref().map(|b| b.status.code()).ok(),
            profile_dir.display()
        )
    })
}

struct Cc {
    work: PathBuf,
    rs_text: String,
}

fn compile_case(name: &str, src: &str, extra: &[&str]) -> Cc {
    let work = repo_root().join("target/embed_tuple_ser").join(name);
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("calc.lz");
    std::fs::write(&lz, src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let mut cmd = Command::new(&bin);
    cmd.current_dir(&work).arg(&lz).args(extra);
    let out = cmd.output().expect("run lang-zone");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "[{name}] lang-zone 编译失败: stdout={stdout} stderr={stderr}"
    );
    let rs_text = std::fs::read_to_string(work.join("calc.rs")).expect("read generated rs");
    Cc { work, rs_text }
}

/// 编译并运行生成的 .rs，返回程序 stdout（验证铁律：编译产物并运行比对）
fn run_case(cc: &Cc, name: &str) -> String {
    let rs = cc.work.join("calc.rs");
    let exe = cc.work.join("calc.exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            build_profile_dir().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .current_dir(&cc.work)
        .output()
        .expect("run rustc");
    assert!(
        rc.status.success(),
        "[{name}] rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );
    let run = Command::new(&exe)
        .current_dir(&cc.work)
        .output()
        .expect("run exe");
    let so = String::from_utf8_lossy(&run.stdout).to_string();
    let se = String::from_utf8_lossy(&run.stderr).to_string();
    assert!(run.status.success(), "[{name}] 运行失败: {se}\nstdout={so}");
    so
}

/// 17 种 tuple 组合闭集（lz_type 串）：16 种二元组 + 1 种三元组
const TUPLE_COMBOS: [&str; 17] = [
    "tuple<int,int>",
    "tuple<int,float>",
    "tuple<int,str>",
    "tuple<int,bool>",
    "tuple<float,int>",
    "tuple<float,float>",
    "tuple<float,str>",
    "tuple<float,bool>",
    "tuple<str,int>",
    "tuple<str,float>",
    "tuple<str,str>",
    "tuple<str,bool>",
    "tuple<bool,int>",
    "tuple<bool,float>",
    "tuple<bool,str>",
    "tuple<bool,bool>",
    "tuple<int,int,int>",
];

/// ① 全部 17 种 tuple 组合的签名都必须被摘出且 callable
#[test]
fn all_17_tuple_combos_are_callable() {
    use lang_zone::embed::adapters::adapter;

    for combo in TUPLE_COMBOS {
        let src = format!("def f(p: Tuple<{}>) -> int:\n    return 0\n", trim_tuple(combo));
        let lang = adapter("python");
        let methods = lang.methods(&src);
        assert_eq!(methods.len(), 1, "{} 应摘出 1 个函数", combo);
        assert!(
            methods[0].callable(),
            "{} 签名应可序列化（T0r59 后 17 种组合全在闭集内），实际 unsupported={:?}",
            combo,
            methods[0].unsupported
        );
        assert_eq!(methods[0].params[0].lz_type, combo, "{} 参数类型回读一致", combo);
    }
}

/// 去掉 tuple<> 外壳，如 "tuple<int,float>" → "int,float"
fn trim_tuple(lz: &str) -> &str {
    lz.strip_prefix("tuple<")
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or(lz)
}

/// ② Python dispatch：tuple 参数走 __lz_json_tuple_* 解码、tuple 返回值走 _out_ok_json
#[test]
fn python_dispatch_tuple_int_float_param_and_ret_uses_json() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def scale(p: Tuple<int,float>) -> Tuple<int,float>:\n    return (p[0], p[1] * 2.0)\n";
    let methods = lang.methods(src);

    assert_eq!(methods.len(), 1);
    let m = &methods[0];
    assert!(m.callable(), "Tuple<int,float> 签名应可序列化");
    assert_eq!(m.ret.as_deref(), Some("tuple<int,float>"));

    let refs: Vec<&_> = methods.iter().collect();
    let (_, body) = lang.dispatch_file("python_test", &refs).expect("dispatch_file");

    assert!(body.contains("__lz_json_tuple_int_float"), "应包含 tuple<int,float> 解码函数");
    assert!(body.contains("json.loads"), "应使用 json.loads 解码");

    // 返回值臂必须走 JSON 编码输出
    let arm = body.split("if method == \"scale\":").nth(1).unwrap_or("");
    assert!(arm.contains("_out_ok_json"), "tuple 返回值应使用 JSON 编码");
    assert!(!arm.contains("_out(\"OK\""), "tuple 返回值不应使用普通 _out");
}

/// ③ Rust 垫片双向：wrapper 签名出具体元组类型 + 参数方向 to_json + 返回方向 parse
#[test]
fn rust_shim_generates_concrete_tuple_types_and_both_directions() {
    use lang_zone::bridge::cli::generate_embed_module;
    use lang_zone::embed::shim;

    let src = "def scale(p: Tuple<int,float>) -> Tuple<int,float>:\n    return (p[0], p[1] * 2.0)\n\n\ndef label(p: Tuple<int,str>) -> str:\n    return p[1]\n";
    let target = lang_zone::embed::resolve("python", "calc.lz", src).expect("resolve");
    let ep = shim::endpoint(&target);
    let module = generate_embed_module(&[ep]);

    // 参数/返回类型：具体 Rust 元组（T0r59 前这里是 &str 形参 + 无返回）
    assert!(
        module.contains("pub fn python_calc_scale(p: (i64, f64)) -> (i64, f64)"),
        "wrapper 签名应携带具体元组类型:\n{}",
        module
    );
    // 参数方向：JSON 数组序列化
    assert!(
        module.contains("__lz_tuple_int_float_to_json(&"),
        "tuple 参数应经 to_json 序列化:\n{}",
        module
    );
    // 返回方向：parse 反序列化（T0r59 前返回值被静默丢弃）
    assert!(
        module.contains("__lz_parse_tuple_int_float(&__r"),
        "tuple 返回值应经 parse 反序列化:\n{}",
        module
    );
    // str 元素走 String + 引号转义
    assert!(
        module.contains("(i64, String)"),
        "tuple<int,str> 的 str 元素应为 String:\n{}",
        module
    );
    // 垫片是常驻静态文本：17 个 parse 助手全量在场
    assert!(module.contains("__lz_parse_tuple_int_int_int("));
    assert!(module.contains("__lz_split_json_array"));
    assert!(module.contains("__lz_json_elem_bool"));
}

/// ④ 闭集错误消息覆盖 tuple 家族（T0r59 前只列到 dict<int,int>）
#[test]
fn closed_set_error_message_includes_tuple_family() {
    use lang_zone::embed::adapters::adapter;

    let lang = adapter("python");
    let src = "def f() -> list<bool>:\n    return []\n";
    let methods = lang.methods(src);
    assert!(!methods[0].callable(), "list<bool> 不在闭集内");
    let msg = methods[0].unsupported.as_deref().unwrap_or("");
    assert!(
        msg.contains("tuple<int,float>"),
        "闭集错误消息应包含 tuple 家族（如 tuple<int,float>），实际：{}",
        msg
    );
}

/// ⑤ 端到端：tuple<int,str> 参数 + 返回值 roundtrip（编译 + rustc + 运行比对）
#[test]
fn e2e_tuple_int_str_roundtrip_runs() {
    let src = "#[embed(python)]\n```\ndef make_pair(a: int, b: str) -> Tuple<int,str>:\n    return (a, b)\n```\n\ndef main():\n    print(python_calc_make_pair(7, \"hi\"))\n";
    let cc = compile_case("e2e_int_str", src, &["--embed-build"]);
    // 短名别名 + 产物全名都指同一包装；生成码必须携带元组返回路径
    assert!(
        cc.rs_text.contains("__lz_parse_tuple_int_str(&__r"),
        "生成码应包含 tuple<int,str> 返回值反序列化:\n{}",
        cc.rs_text
    );
    let out = run_case(&cc, "e2e_int_str");
    assert_eq!(
        out.trim(),
        "(7, \"hi\")",
        "roundtrip 输出应为 LZ tuple 的 Debug 形态"
    );
}

/// ⑥ 端到端：tuple<int,bool> 参数方向 roundtrip（bool 元素走 JSON true/false）
#[test]
fn e2e_tuple_int_bool_param_roundtrip_runs() {
    let src = "#[embed(python)]\n```\ndef pick(p: Tuple<int,bool>) -> int:\n    return p[0] if p[1] else -p[0]\n```\n\ndef main():\n    print(python_calc_pick((3, true)))\n    print(python_calc_pick((3, false)))\n";
    let cc = compile_case("e2e_int_bool", src, &["--embed-build"]);
    assert!(
        cc.rs_text.contains("__lz_tuple_int_bool_to_json(&"),
        "tuple<int,bool> 参数应经 to_json 序列化:\n{}",
        cc.rs_text
    );
    let out = run_case(&cc, "e2e_int_bool");
    let lines: Vec<&str> = out.trim().lines().collect();
    assert_eq!(lines, vec!["3", "-3"], "bool 元素应正确传为 Python True/False");
}
