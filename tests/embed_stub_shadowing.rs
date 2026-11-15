// Lang-Zone 编译器 — tests/embed_stub_shadowing.rs
// fist-mbt T0r32（命名空间 lang-zone）验收面：**dedup 产物名不许把短名调用遮成
// 一个静默返回 i64::MAX 的兜底桩**。
//
// 修前现场（`temp/projbuild`，entry=src/main.lz，模块在 src/ 子目录）：
//   `dedup::artifact_name` 把 `src/a.lz` 的 python 产物命名成 `python_a__src_4d744cd7`
//   ⇒ `__lz_embed` 里真正的包装函数叫 `python_a__src_4d744cd7_add`；
//   而 LZ 作者按「模块 + 方法」写的是短名 `python_a_add(1, 2)`。
//   短名在 LZ 侧看不见任何定义 ⇒ `collect_unknown_extern_fns` 把它当「未知外部函数」，
//   `emit.rs` 在 crate 根补一个 `fn python_a_add(__a0: i64, __a1: i64) -> i64 { i64::MAX }`。
//   这个桩是**本地定义**，而 `use __lz_embed::*;` 是 glob 导入 —— Rust 里本地定义优先于
//   glob ⇒ 调用被**遮住**，程序打印 9223372036854775807（静默错值，不是报错）。
//
// 修后（判据只读「生成的 .rs + 真跑出来的 stdout」，不靠内部字段自证）：
//   ① 短名 `python_a_add` 指到真包装（1+2 ⇒ 3），且 .rs 里没有同名兜底桩；
//   ② 产物全名 `python_a__src_…_add` 同样指到真包装（10+20 ⇒ 30）；
//   ③ **安全网不许整体拆掉**：真正没有定义的外部函数仍发 `-> i64 { i64::MAX }` 桩；
//   ④ 短名与全名同处一个程序时指同一个函数（别名不是第二份产物）。
//
// 兼容面：产物名/目录/MANIFEST 一字不动（仍带 `__src_<hash>` 去重后缀）—— 本文件
// 显式断言这一点，防止有人用「把产物改名成短名」的方式糊过去。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// ───────────────────────────── 通用夹具 ─────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 每个用例一棵干净的工作目录；cwd 就是它 ⇒ 产物根 = 该目录，与 `lz build` 同口径。
fn case_dir(name: &str) -> PathBuf {
    let d = repo_root().join("target/embed_stub_shadowing").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("src")).expect("create case dir");
    d
}

fn write_src(work: &Path, rel: &str, text: &str) {
    let p = work.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("create src dir");
    }
    std::fs::write(&p, text).expect("write lz");
}

/// 子目录模块（`src/a.lz`）—— 正是 dedup 命名会加 `__src_<hash>` 后缀的那种布局。
const A_LZ: &str = r#"#[embed(python)]
```
def add(a: int, b: int) -> int:
    return a + b
```
"#;

/// 生成的 .rs 要 `--extern lz_builtins=…` 才链得上（`cli.rs::rustc_compile` 默认取
/// `<仓根>/target/debug/liblz_builtins.rlib`）。缺件时当场补建一次，让本套判据对
/// 任何树都自足（与 tests/embed_unify_r15.rs 同一口径）。
fn ensure_builtins_rlib() -> PathBuf {
    let want = repo_root().join("target/debug/liblz_builtins.rlib");
    if want.exists() {
        return want;
    }
    let built = Command::new("cargo")
        .args(["build", "-p", "lz_builtins"])
        .current_dir(repo_root())
        // 夹具本身可能跑在别的 CARGO_TARGET_DIR 下；`rustc_compile` 只认 target/debug
        .env_remove("CARGO_TARGET_DIR")
        .output();
    match built {
        Ok(b) if b.status.success() => {}
        Ok(b) => panic!(
            "lz_builtins 补建失败：{}\n若本机无 cargo，请手工产出 target/debug/liblz_builtins.rlib 后复跑",
            String::from_utf8_lossy(&b.stderr)
        ),
        Err(e) => panic!("无法执行 cargo build -p lz_builtins：{e}"),
    }
    assert!(
        want.exists(),
        "已跑 cargo build -p lz_builtins 仍缺 {} ⇒ 取数路径变了，改本行而非放宽判据",
        want.display()
    );
    want
}

/// rustc 重活串行化：本文件每个用例都要 spawn 一次 rustc，机器内存紧时并行跑不安全。
/// 用一把进程内锁把它们排成队（等价于 `--test-threads=1`，但不要求调用方记得加参数）。
static RUSTC_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn rustc_guard() -> std::sync::MutexGuard<'static, ()> {
    RUSTC_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

// ───────────────────────────── 一次构建 ─────────────────────────────

struct Built {
    work: PathBuf,
    /// 生成的 Rust 全文
    rs: String,
    /// 盘上的产物全名（`python_a__src_……`），从 `build/embed/a/` 直接读盘
    artifact: String,
    build_stdout: String,
}

impl Built {
    /// 跑 `build/<name>.exe`（cwd = 工程根 ⇒ 垫片里的相对产物目录可解析）
    fn run(&self) -> String {
        let exe = self.work.join("build").join(exe_name()).with_extension("exe");
        let exe = if exe.exists() {
            exe
        } else {
            self.work.join("build").join(exe_name())
        };
        assert!(
            exe.exists(),
            "构建产物不存在（lz build 未产出可执行文件）: {}",
            exe.display()
        );
        let out = Command::new(&exe)
            .current_dir(&self.work)
            .output()
            .unwrap_or_else(|e| panic!("启动 {} 失败: {e}", exe.display()));
        assert!(
            out.status.success(),
            "运行失败（code={:?}）:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            out.status.code(),
            stdout(&out),
            stderr(&out)
        );
        stdout(&out)
    }
}

fn exe_name() -> &'static str {
    "shadow"
}

/// 盘上 `build/embed/a/` 下的产物目录名（应为 1 个；多于 1 个说明命名/落盘变了）
fn artifacts_on_disk(work: &Path) -> Vec<String> {
    let root = work.join("build/embed");
    let mut out = Vec::new();
    let stems = match std::fs::read_dir(&root) {
        Ok(rd) => rd,
        Err(_) => return out,
    };
    for stem in stems.flatten().filter(|e| e.path().is_dir()) {
        if let Ok(arts) = std::fs::read_dir(stem.path()) {
            for a in arts.flatten().filter(|e| e.path().is_dir()) {
                out.push(a.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

/// 走真实 `lz build` 路径：`lz.toml` + `src/main.lz` + `src/a.lz` → `build/shadow.rs` + exe。
fn build_project(case: &str, main_src: &str) -> Built {
    let _guard = rustc_guard();
    let _ = ensure_builtins_rlib();
    let work = case_dir(case);
    write_src(
        &work,
        "lz.toml",
        "name = \"shadow\"\nversion = \"0.1.0\"\nentry = \"src/main.lz\"\n",
    );
    write_src(&work, "src/a.lz", A_LZ);
    write_src(&work, "src/main.lz", main_src);

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg("build")
        // 明确全量：增量命中会跳过前端 codegen，判据就测不到本轮改动
        .arg("--no-incremental")
        .current_dir(&work)
        // 不落 rustc incremental 目录，省盘也省得旧对象干扰
        .env("LZ_RUSTC_INCREMENTAL", "0")
        .output()
        .unwrap_or_else(|e| panic!("启动 {} 失败: {e}", bin.display()));
    let build_stdout = stdout(&out);
    let build_stderr = stderr(&out);
    assert!(
        out.status.success(),
        "[{case}] `lang-zone build` 失败（退出码 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.status.code(),
        build_stdout,
        build_stderr
    );

    let rs_path = work.join("build").join(format!("{}.rs", exe_name()));
    let rs = std::fs::read_to_string(&rs_path)
        .unwrap_or_else(|e| panic!("读不到生成的 {}: {e}", rs_path.display()));

    let arts = artifacts_on_disk(&work);
    assert_eq!(
        arts.len(),
        1,
        "[{case}] 子目录模块应恰有一个产物目录，实际 {arts:?}\n--- stdout ---\n{build_stdout}"
    );
    Built {
        work,
        rs,
        artifact: arts[0].clone(),
        build_stdout,
    }
}

/// 标准布局（`src/a.lz` @ cwd=工程根）下的产物全名，供「全名调用」用例使用。
/// 进程内只探一次：同一布局的 module_key 恒为 `src/a.lz` ⇒ 产物名确定（无随机数）。
fn standard_full_artifact() -> String {
    static ONCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        // `import a` 是必须的：没有它 ProjectCompiler 不会把 src/a.lz 编进来，也就
        // 不会产生 embed 产物（探针会读到空目录）。
        let b = build_project("_artifact_probe", "import a\n\ndef main():\n    print(1)\n");
        b.artifact
    })
    .clone()
}

// ───────────────────────── ① 短名 ⇒ 真包装（本次修复的主判据）─────────────────────────

#[test]
fn short_name_calls_the_real_wrapper_instead_of_the_stub() {
    let b = build_project(
        "short_name",
        "import a\n\ndef main():\n    print(python_a_add(1, 2))\n",
    );

    // 命名契约没变：产物仍是 dedup 全名（不是被改名成短名糊过去）
    assert!(
        b.artifact.starts_with("python_a__src_"),
        "产物全名应保留 dedup 后缀 `__src_<hash>`，实际 {}：\n{}",
        b.artifact,
        b.build_stdout
    );

    // 短名必须真的被导出成别名（指向同一个包装函数）
    let want_alias = format!("pub use self::{}_add as python_a_add;", b.artifact);
    assert!(
        b.rs.contains(&want_alias),
        "缺 `{want_alias}` —— 短名没有别名，LZ 侧写短名就无处可指：\n{}",
        b.rs
    );

    // 兜底桩必须让位：桩是 crate 根的本地定义，会遮住 glob 导入的别名
    assert!(
        !b.rs.contains("fn python_a_add(__a0"),
        "兜底桩仍在（会遮住 __lz_embed 的短名别名，静默返回 i64::MAX）：\n{}",
        b.rs
    );

    // 真跑：1 + 2 = 3，不是 i64::MAX
    let out = b.run();
    assert_eq!(
        out.trim(),
        "3",
        "短名调用取回的值不对（i64::MAX ⇒ 又走到兜底桩了）: {out:?}"
    );
}

// ───────────────────────── ② 产物全名 ⇒ 同样到真包装 ─────────────────────────

#[test]
fn full_artifact_name_calls_the_real_wrapper() {
    let full = standard_full_artifact();
    let b = build_project(
        "full_name",
        &format!("import a\n\ndef main():\n    print({full}_add(10, 20))\n"),
    );
    assert_eq!(
        b.artifact, full,
        "同一布局两次编译的产物名必须一致（确定性命名）"
    );
    assert!(
        b.rs.contains(&format!("pub fn {full}_add(")),
        "`__lz_embed` 里应有产物全名包装 `{full}_add`：\n{}",
        b.rs
    );
    let out = b.run();
    assert_eq!(out.trim(), "30", "全名调用取回的值不对: {out:?}");
}

// ───────────────────────── ③ 真未定义函数仍走兜底桩（安全网不许拆）─────────────────────────

#[test]
fn genuinely_undefined_extern_still_gets_the_stub() {
    let b = build_project(
        "unknown_extern",
        "import a\n\ndef main():\n    print(python_a_add(1, 2))\n    print(truly_undefined_extern(7))\n",
    );

    // 安全网在：真正没有定义、也没有 embed 产物的外部函数照旧发 i64 桩
    assert!(
        b.rs.contains("fn truly_undefined_extern(__a0: i64) -> i64 { i64::MAX }"),
        "兜底桩被整体拆掉了 —— 跨模块/.lzi 未加载的探测用例会 E0425：\n{}",
        b.rs
    );
    // 同时短名仍走别名（两者共存，互不遮挡）
    assert!(
        !b.rs.contains("fn python_a_add(__a0"),
        "短名不该再发桩：\n{}",
        b.rs
    );

    let out = b.run();
    let lines: Vec<&str> = out.trim().lines().map(|l| l.trim()).collect();
    assert_eq!(lines.len(), 2, "应有两行输出: {out:?}");
    assert_eq!(lines[0], "3", "短名调用必须是真包装: {out:?}");
    assert_eq!(
        lines[1], "9223372036854775807",
        "真未定义函数的兜底桩语义应保持 i64::MAX: {out:?}"
    );
}

// ───────────────────────── ④ 短名与全名同处一个程序：指同一个函数 ─────────────────────────

#[test]
fn short_and_full_names_are_the_same_function() {
    let full = standard_full_artifact();
    let b = build_project(
        "both_names",
        &format!(
            "import a\n\ndef main():\n    print(python_a_add(1, 2))\n    print({full}_add(3, 4))\n"
        ),
    );
    let out = b.run();
    let lines: Vec<&str> = out.trim().lines().map(|l| l.trim()).collect();
    assert_eq!(lines.len(), 2, "应有两行输出: {out:?}");
    assert_eq!(lines[0], "3", "短名: {out:?}");
    assert_eq!(lines[1], "7", "全名: {out:?}");
}
