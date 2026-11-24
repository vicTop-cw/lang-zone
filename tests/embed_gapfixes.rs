// Lang-Zone 编译器 — tests/embed_gapfixes.rs
// T0r50 的判据：T0r49 体检报告（Reports/embed-gaps-2026-10-30.md）里四处「度量失真 / 静默丢弃」，
// 每处一条正向判据 + 一条负向对照（防止判据自己坏掉 —— 与 tests/embed_unify_r15.rs 同一套写法）。
//
//   ① §2.1 toolchain_available 量的是 run 命令而不是建库工具链 ⇒ 拆成两行，各自可归因
//   ② §2.3 scala dispatch=none 却照发可调用垫片          ⇒ callable_methods 加 has_dispatch_file 门
//   ③ §2.4 `.bat` 形态工具链读数说在场、spawn 说找不到    ⇒ 裸名 .bat/.cmd 经 cmd /C（建库侧与垫片侧同一把尺）
//   ④ §一   无返回注解但体内有带值 return ⇒ 值被静默丢弃    ⇒ 判 unsupported 并逐条点名
//
// 本文件只读 src/embed/** 与 src/bridge/cli.rs 的公开面；不碰 src/ir/**。

use std::path::{Path, PathBuf};
use std::process::Command;

use lang_zone::embed::artifact::{build_artifact, manifest_text, manifest_text_at, BuildStatus};
use lang_zone::embed::{
    cmd_route_shape, needs_cmd_route, resolve, EmbedTarget, NO_DISPATCH_SURFACE,
};

/// 本文件里所有「读 PATH」的判据共用的串行锁：`gapfix③` 会临时改进程 PATH（换尺判据要看 PATH），
/// 同二进制内并行跑时不许让别的用例在改窗口的瞬间读到半截 PATH。
/// 用 try_lock().unwrap_or_else(|e| e.into_inner())：即便某个持锁用例 panic，其余用例仍能推进。
static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn path_guard() -> std::sync::MutexGuard<'static, ()> {
    match PATH_LOCK.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn tmp_root(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!(
        "lz_gapfix_{}_{}_{}",
        tag,
        std::process::id(),
        nanos as u64 + SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn target(lang: &str, stem: &str, src: &str) -> EmbedTarget {
    let mut t = resolve(lang, stem, src).unwrap();
    t.dir_rel = lang_zone::embed::artifact::dir_rel(
        lang_zone::embed::base_stem(stem).as_str(),
        t.artifact.as_str(),
    );
    t
}

/// MANIFEST 里某个 key 的取值（同一 key 出现多次直接发红 —— 两行必须各管一件事）
fn manifest_field(mani: &str, key: &str) -> String {
    let hits: Vec<&str> = mani
        .lines()
        .filter(|l| l.starts_with(&format!("{}=", key)))
        .collect();
    assert_eq!(hits.len(), 1, "`{key}=` 必须恰好一行，实际 {:?}\n{}", hits, mani);
    hits[0][key.len() + 1..].to_string()
}

// ════════════════ ① toolchain_available（建库工具链） / run_available（运行命令在位）════════════════

/// 正向：python 是解释型 —— **没有建库这一步**，所以工具链读数是 `n/a` 而不是 `no`；
/// run 是裸名 `python` ⇒ `run_available` 走 PATH 判。
/// 修前的唯一一行 `toolchain_available=` 把「python 在不在 PATH」当成工具链读数，
/// 既答不了建库、也答不了产物在位。
#[test]
fn interpreted_lang_reports_na_for_build_toolchain() {
    let t = target("python", "a.lz", "def f(x: int) -> int:\n    return x\n");
    let m = manifest_text(&t);
    assert_eq!(manifest_field(&m, "toolchain_available"), "n/a", "{m}");
    // 解释型语言的 run 是裸名 python ⇒ 本机在场就是 yes（不在场也不该由这行冒充工具链缺席）
    let want = if lang_zone::embed::tool_available("python") { "yes" } else { "no" };
    assert_eq!(manifest_field(&m, "run_available"), want, "{m}");
}

/// 失真本体（§2.1 的复现）：c 档 gcc 在场时，修前**建库成功之后仍报 toolchain_available=no**
/// —— 因为它量的是 `./c_calc.exe`（相对产物名，按编译器 cwd 解不开）。
/// 修后：toolchain_available 只看 gcc（建库工具链），run_available 才看产物 exe 在不在位。
#[test]
fn c_lines_separate_build_toolchain_from_run_artifact() {
    let gcc_present = lang_zone::embed::tool_available("gcc");
    let t = target("c", "a.lz", "int add(int a, int b) { return a + b; }\n");
    // ① 无产物目录时：run_available 必须 no（exe 还没建出来），这行不代表工具链缺席
    let m0 = manifest_text(&t);
    assert_eq!(
        manifest_field(&m0, "toolchain_available"),
        if gcc_present { "yes" } else { "no" },
        "toolchain_available 必须只回答「建库工具链在不在」：{m0}"
    );
    assert_eq!(manifest_field(&m0, "run_available"), "no", "{m0}");

    // ② 造出产物 exe（用 rustc 写一个占位 exe 即可 —— 本条测读数口径，不测 gcc 能不能编）
    let root = tmp_root("crun");
    let dir = lang_zone::embed::artifact::artifact_dir(&root, &t);
    std::fs::create_dir_all(&dir).unwrap();
    // run 命令是 `./<产物名>.exe`（c adapter 固定形状），占位文件必须同名
    std::fs::write(dir.join(format!("{}.exe", t.artifact)), b"stub").unwrap();
    let m1 = manifest_text_at(&t, Some(&dir));
    assert_eq!(
        manifest_field(&m1, "run_available"), "yes",
        "产物 exe 在位后 run_available 必须转 yes（修前恒 no）：{m1}"
    );
    // 两行各自独立：加了 exe 不该改工具链读数
    assert_eq!(
        manifest_field(&m1, "toolchain_available"),
        manifest_field(&m0, "toolchain_available"),
        "建库工具链读数不该随产物在位而变"
    );
    // 仍是确定性文本：无绝对路径、无时间戳
    assert!(!m1.contains("C:\\") && !m1.contains("F:\\"), "{m1}");
    let _ = std::fs::remove_dir_all(&root);

    if !gcc_present {
        eprintln!("[gapfix①] 本机无 gcc ⇒ toolchain_available=no 走的是缺席分支，yes 面由下一条负向对照兜住");
    }
}

/// 负向对照：工具链**确实缺席**时必须报 no（不许被 `n/a` 或 `yes` 冒充），
/// 且 `run_available` 与它互不串台 —— 这是「两行各自可归因」的另一半。
#[test]
fn absent_build_toolchain_is_no_not_na_and_does_not_leak_into_run_line() {
    let root = tmp_root("absent");
    let mut t = target("python", "a.lz", "def f(x: int) -> int:\n    return x\n");
    t.build = Some(lang_zone::embed::CommandSpec::new(
        "definitely_not_a_real_tool_lz_gapfix",
        &["build", "x"],
    ));
    let m = manifest_text_at(&t, Some(&root));
    assert_eq!(manifest_field(&m, "toolchain_available"), "no", "{m}");
    // run 是裸名 python：与「建库工具链缺席」无关，各自独立成行
    let want = if lang_zone::embed::tool_available("python") { "yes" } else { "no" };
    assert_eq!(manifest_field(&m, "run_available"), want, "{m}");
    // 建库判据本身仍是 Skipped（不是 Failed、更不是 Built）
    assert_eq!(build_artifact(&root, &t, false).status, BuildStatus::Skipped);
    let _ = std::fs::remove_dir_all(&root);
}

// ════════════════ ② scala：**有**分发入口后不再被 surface_gated（T0r17 契约反转）════════════════

/// T0r17 前：scala 的 `dispatch_file()` 返回 None ⇒ `callable_methods()` 被 has_dispatch_file 门挡下，
/// MANIFEST 写 `dispatch=none` + 逐条点名 NO_DISPATCH_SURFACE。
/// T0r17 后：scala 生成「零 runtime」分发入口（java 在场时），调用面**打开** ⇒ 本用例反转为
/// 「有分发入口 ⇒ 有可调用面」，而 NO_DISPATCH_SURFACE 门本身仍由 `langs_with_dispatch_surface_keep_their_callable_face`
/// 与 `scala_no_dispatch_when_java_absent_gets_gated` 两条钉住（门没被拆，只是 scala 现在过门了）。
#[test]
fn scala_with_dispatch_surface_exposes_callable_face() {
    if !lang_zone::embed::tool_available("java") {
        eprintln!("SKIP: java 不可用（scala 零 runtime 分发需 java）⇒ 本条走下面的 gated 分支");
        return;
    }
    let t = target("scala", "a.lz", "def add(a: Int, b: Int): Int = a + b\n");
    assert!(
        t.has_dispatch_file(),
        "T0r17：scala 应生成分发入口（此前恒无）"
    );
    let callable = t.callable_methods();
    assert_eq!(callable.len(), 1, "add 现应可调用：{:?}", t.methods);
    assert_eq!(callable[0].name, "add");
    // MANIFEST 反映新状态
    let m = manifest_text(&t);
    assert!(m.contains("dispatch=generated"), "{m}");
    assert!(
        m.contains("method=add params=a:int,b:int ret=int callable"),
        "MANIFEST 应把 add 标 callable（且 ret 已从 Scala 冒号形态解析出 int）：\n{m}"
    );
    assert!(!m.contains(&NO_DISPATCH_SURFACE.replace(' ', "_")), "{m}");

    // 垫片现在**应当**发这个函数（调用面真的存在）
    // 注意 artifact 名：resolve("scala", "a.lz", …) → `scala_a` ⇒ wrapper `scala_a_add`
    let shim = lang_zone::embed::shim::module_text(std::slice::from_ref(&t));
    assert!(
        shim.contains("scala_a_add"),
        "有分发入口却不肯发垫片：\n{shim}"
    );
    assert!(shim.contains("__lz_spawn_plan"), "垫片头仍在");
}

/// 负向对照：NO_DISPATCH_SURFACE 门**依然在**。用 python 的 Executable 档（有自家 main、
/// 无分发入口）证明「没有分发入口 ⇒ 调用面为空 + 逐条点名」这条规则没被 T0r17 拆掉。
#[test]
fn no_dispatch_surface_still_gates_callable_face() {
    // Executable 档按定义带 main、不出分发入口（见 has_main_decides_executable_not_a_declaration）
    let t = target("python", "a.lz", "if __name__ == \"__main__\":\n    print(1)\n");
    assert!(!t.has_dispatch_file(), "Executable 档不应有分发入口");
    assert!(
        t.callable_methods().is_empty(),
        "无分发入口 ⇒ 不许有任何可调用面：{:?}",
        t.callable_methods()
    );
    let m = manifest_text(&t);
    assert!(m.contains("dispatch=none"), "{m}");
}

/// 负向对照：有分发入口的语言（python）不受这道门影响 ⇒ 不许被误伤成全员不可调用。
#[test]
fn langs_with_dispatch_surface_keep_their_callable_face() {
    let t = target("python", "a.lz", "def add(a: int, b: int) -> int:\n    return a + b\n");
    assert!(t.has_dispatch_file());
    assert_eq!(t.callable_methods().len(), 1, "{:?}", t.methods);
    let m = manifest_text(&t);
    assert!(m.contains("method=add params=a:int,b:int ret=int callable"), "{m}");
    let shim = lang_zone::embed::shim::module_text(std::slice::from_ref(&t));
    assert!(shim.contains("pub fn python_a_add("), "{shim}");
}

// ════════════════ ③ `.bat` 形态工具链：读数与 spawn 必须同一把尺（§2.4）════════════════

/// 造一个只有 `.bat` 包装的假工具链，把它放到 PATH 上，然后问两把尺。
/// 判据只在 Windows 有意义（非 Windows 上 needs_cmd_route 恒 false，本条直接打印后返回）。
#[test]
fn bare_bat_tool_is_seen_by_probe_and_routable_by_spawn() {
    if !cfg!(windows) {
        println!("[gapfix③] 非 Windows ⇒ needs_cmd_route 恒 false，本条不适用");
        return;
    }
    // 本条会临时改进程 PATH（换尺判据必须读 PATH）⇒ 与同二进制内其它读 PATH 的用例互斥
    let _guard = path_guard();
    let dir = tmp_root("battool");
    std::fs::write(
        dir.join("lzbatsim.bat"),
        "@echo off\r\necho SIM-RUN %*\r\nexit /b 0\r\n",
    )
    .unwrap();

    let saved = std::env::var_os("PATH");
    std::env::set_var(
        "PATH",
        format!("{};{}", dir.display(), saved.clone().unwrap_or_default().to_string_lossy()),
    );

    // 读数：扫文件名认 .bat ⇒ tool_available 说在场（这一侧不该改，改了会把「装了却用不了」说成「没装」）
    assert!(
        lang_zone::embed::tool_available("lzbatsim"),
        "tool_available 必须继续认 .bat/.cmd 形态"
    );
    // 换尺判据：裸名 + 无 .exe 映像 + 有 .bat ⇒ 需要 cmd /C
    assert!(needs_cmd_route("lzbatsim"), "裸名 .bat 必须走 cmd /C");
    let (prog, pre) = cmd_route_shape("lzbatsim");
    assert_eq!(prog, "cmd");
    assert_eq!(pre, vec!["/C".to_string(), "lzbatsim".to_string()]);
    // 边界（实测 quot4）：带扩展名的形态 CreateProcess 自己解得开 ⇒ 不换尺
    assert!(!needs_cmd_route("lzbatsim.bat"), "带 .bat 扩展名的裸名不许被多包一层 cmd /C");
    assert!(!needs_cmd_route(&dir.join("lzbatsim.bat").display().to_string()), "绝对 .bat 路径不换尺");
    // 原生映像（rustc.exe 这类）一律不换尺 ⇒ python/gcc 行为与修前一字不动
    assert!(!needs_cmd_route("rustc"), "有 .exe 映像的裸名不许换尺");

    // 端到端：只有 .bat 的工具链，建库步必须真的跑起来（修前恒 Failed: spawn 失败: program not found）
    let root = tmp_root("batbuild");
    let mut t = target("python", "a.lz", "def f(x: int) -> int:\n    return x\n");
    let adir = lang_zone::embed::artifact::artifact_dir(&root, &t);
    std::fs::create_dir_all(&adir).unwrap();
    t.build = Some(lang_zone::embed::CommandSpec::new("lzbatsim", &["scala_calc.scala"]));
    let o = build_artifact(&root, &t, false);
    assert_eq!(o.status, BuildStatus::Built, "{:?}", o);
    assert!(
        !o.detail.contains("program not found"),
        "仍按 CreateProcess 裸名起 ⇒ 本条正是要修的失真：{:?}",
        o
    );

    // 还原 PATH（同二进制内其它用例不受影响）
    match saved {
        Some(v) => std::env::set_var("PATH", v),
        None => std::env::remove_var("PATH"),
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&root);
}

/// 换尺只在 Windows 有意义 ⇒ 非 Windows 上必须恒 false，不许把 `.bat` 判断漏到跨平台路径上。
#[test]
fn cmd_route_is_windows_only_and_never_wraps_plain_names_elsewhere() {
    // 不存在的裸名：任何平台都不该换尺（没有 .bat/.cmd 命中）
    assert!(!needs_cmd_route("definitely_not_a_real_tool_lz_gapfix"));
    // 空串/带分隔符的怪形状也不炸、不换尺
    assert!(!needs_cmd_route(""));
    assert!(!needs_cmd_route("./no/such/thing"));
    let (p, pre) = cmd_route_shape("definitely_not_a_real_tool_lz_gapfix");
    assert_eq!(p, "definitely_not_a_real_tool_lz_gapfix");
    assert!(pre.is_empty());
}

// ════════════════ ④ 无返回注解 + 体内带值 return ⇒ 值被静默丢弃，必须点名（§一）════════════════

/// 正向：`def as_list(n: int):` 无返回注解但 `return [n]` —— 修前标 `callable`、
/// 垫片发成无返回值函数，LZ 侧调用成功、值被丢掉、**零回执**。
#[test]
fn value_return_without_annotation_is_named_not_silently_dropped() {
    let t = target(
        "python",
        "a.lz",
        "def as_list(n: int):\n    return [n]\ndef ok(n: int) -> int:\n    return n\n",
    );
    let as_list = t.methods.iter().find(|m| m.name == "as_list").unwrap();
    assert_eq!(as_list.ret, None, "无返回注解仍是 None（不猜类型）");
    assert!(!as_list.callable(), "无返回注解却有返回值 ⇒ 不许 callable");
    assert!(as_list.unsupported.as_deref().unwrap().contains("静默丢弃"), "{:?}", as_list);
    assert_eq!(t.unsupported_methods().len(), 1, "{:?}", t.methods);

    // MANIFEST 逐条点名（`callable` 那个词不许再出现在这一行）
    let m = manifest_text(&t);
    assert!(
        !m.contains("method=as_list params=n:int ret=none callable"),
        "修前的失真读数又回来了：\n{m}"
    );
    assert!(m.contains("method=as_list params=n:int ret=none") && m.contains("静默丢弃"), "{m}");

    // 垫片不出这个函数；`ok` 不受影响
    let shim = lang_zone::embed::shim::module_text(std::slice::from_ref(&t));
    assert!(!shim.contains("python_a_as_list"), "{shim}");
    assert!(shim.contains("pub fn python_a_ok("), "{shim}");
}

/// 探针 p1 报的单行形态 `def f(n: int): return [n]`（体就在定义行冒号之后）。
#[test]
fn one_line_value_return_body_is_also_named() {
    let t = target("python", "a.lz", "def one(n: int): return [n]\n");
    let m = &t.methods[0];
    assert!(!m.callable(), "单行体的带值 return 同样会丢返回值：{:?}", m);
    assert!(m.unsupported.as_deref().unwrap().contains("静默丢弃"));
}

/// 负向对照（不许误伤）：真 Unit 的写法必须继续 callable ——
/// `pass` / 裸 `return` / 注释里的 return 字样 / `returns` 同前缀标识符都不算「带值 return」。
#[test]
fn genuine_unit_functions_are_not_flagged() {
    let src = "def p(x: int):\n    pass\ndef r(x: int):\n    if x:\n        return\ndef c(x: int):\n    # return [x]\n    print(x)\ndef rn(x: int):\n    returns = x\n    print(returns)\ndef g(x: int) -> int:\n    return x\n";
    let t = target("python", "a.lz", src);
    let names: Vec<&str> = t.callable_methods().iter().map(|m| m.name.as_str()).collect();
    for want in ["p", "r", "c", "rn", "g"] {
        assert!(names.contains(&want), "真 Unit / 有注解被误判成丢弃返回值：{:?}", t.methods);
    }
    assert_eq!(t.unsupported_methods().len(), 0, "{:?}", t.methods);
}

/// 同族也收（`fn_from_toplevel` 这条路：mojo/scala 共用）——
/// 无 `->` 注解、体内 `return <expr>` ⇒ 同样点名。
#[test]
fn toplevel_fn_shape_without_return_annotation_is_also_named() {
    let t = target("mojo", "a.lz", "fn bad(a: Int64)\n    return a\n");
    let bad = &t.methods[0];
    assert_eq!(bad.ret, None);
    assert!(!bad.callable(), "mojo 同族不许留静默面：{:?}", bad);
    assert!(bad.unsupported.as_deref().unwrap().contains("静默丢弃"), "{:?}", bad);

    // 有注解的正常形态不受影响
    let ok = target("mojo", "b.lz", "fn add(a: Int64, b: Int64) -> Int64\n    return a + b\n");
    assert!(ok.methods[0].callable(), "{:?}", ok.methods);
}

/// 归因不覆盖：签名层已有更高优先级的点名原因（闭集外类型）时，保留先报的那条，
/// 不许把「不在闭集」改写成「静默丢弃」——两个原因对应两种修法。
#[test]
fn existing_signature_reason_is_not_overwritten() {
    let t = target("python", "a.lz", "def bad(x: DenseMatrix):\n    return [x]\n");
    let u = t.methods[0].unsupported.as_deref().unwrap();
    assert!(u.contains("不在闭集"), "原因被改写了：{}", u);
    assert!(!u.contains("静默丢弃"), "不该二次归因：{}", u);
}

// ════════════════ ④ 端到端：回执逐条点名 + 母件仍能跑（非零面不许伪装成功）═══════════════

fn compile_case(name: &str, src: &str, extra: &[&str]) -> (PathBuf, String, String) {
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/embed_gapfixes")
        .join(name);
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("calc.lz");
    std::fs::write(&lz, src).expect("write lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(&work)
        .arg(&lz)
        .args(extra)
        .output()
        .expect("run lang-zone");
    (
        work,
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// 静默面的**回执**判据：`embed 未覆盖: <产物>::<方法> —— <原因>` 逐条打出来，
/// 且未标注的那个函数在生成的 .rs 里根本不存在（垫片面收缩，不是只写日志）。
#[test]
fn discarded_return_is_reported_on_the_receipt_and_absent_from_generated_code() {
    let src = "#[embed(python)]\n```\ndef as_list(n: int):\n    return [n]\ndef ok(n: int) -> int:\n    return n\n```\n\ndef main():\n    print(python_calc_ok(7))\n";
    let (work, stdout, stderr) = compile_case("gap_ret", src, &[]);
    let receipts = format!("{stdout}{stderr}");
    assert!(
        receipts.contains("embed 未覆盖: python_calc::as_list"),
        "静默丢弃必须逐条点名，实际回执: {receipts}"
    );
    assert!(receipts.contains("静默丢弃"), "点名要带归因，实际: {receipts}");
    let rs = std::fs::read_to_string(work.join("calc.rs")).expect("read rs");
    assert!(!rs.contains("python_calc_as_list"), "垫片不许发这个函数:\n{rs}");
    assert!(rs.contains("pub fn python_calc_ok("), "同产物里的可调用面不受牵连:\n{rs}");
}

/// scala 的端到端（T0r17 契约反转）：修前 `dispatch=none` ⇒ 垫片发空 + 回执点名；
/// T0r17 后 scala 生成「零 runtime」分发入口 ⇒ 垫片应当真的发出来，MANIFEST 标 dispatch=generated。
#[test]
fn scala_embed_emits_dispatch_and_wrapper_after_t0r17() {
    if !lang_zone::embed::tool_available("java") {
        eprintln!("SKIP: java 不可用 ⇒ scala 仍走 gated 分支（由 no_dispatch_surface_still_gates_callable_face 覆盖）");
        return;
    }
    let src = "#[embed(scala)]\n```\ndef add(a: Int, b: Int): Int = a + b\n```\n\ndef main():\n    print(1)\n";
    let (work, stdout, stderr) = compile_case("gap_scala", src, &[]);
    let receipts = format!("{stdout}{stderr}");
    // T0r17 后 add 已可调用 ⇒ 不应再被点名「未覆盖」
    assert!(
        !receipts.contains("embed 未覆盖: scala_calc::add"),
        "add 现已可调用，不该被点名未覆盖：{receipts}"
    );
    let rs = std::fs::read_to_string(work.join("calc.rs")).expect("read rs");
    assert!(
        rs.contains("scala_calc_add"),
        "有分发入口 ⇒ 垫片应真的发出来：\n{rs}"
    );
    let mani = std::fs::read_to_string(work.join("build/embed/calc/scala_calc/MANIFEST.txt"))
        .expect("manifest");
    assert!(mani.contains("dispatch=generated"), "{mani}");
    assert!(
        mani.contains("method=add params=a:int,b:int ret=int callable"),
        "add 应标 callable 且 ret 已解析:\n{mani}"
    );
    // ①+③ 同一份 MANIFEST 的两行：scalac（.bat）在场时 toolchain_available=yes
    let line = manifest_field(&mani, "toolchain_available");
    assert!(
        line == "yes" || line == "no",
        "scala 有建库步 ⇒ 该行只能是 yes/no（不许 n/a）：{mani}"
    );
}

// ════════════════ 门失效面的残留（点名，不掩盖）════════════════

/// ②④ 把可调用面收缩后，**仍在调用被点名函数**的作者会落到哪：
/// 编译期回执必须逐条点名（本条钉这条）；运行期则落到 codegen 的 unknown_extern 兜底桩（i64::MAX）。
///
/// 实测对照（temp/gapfix/{before,after}/p_ret_call，母件 `print(python_calc_as_list(3))`）：
///  - 修前：垫片发 `pub fn python_calc_as_list(n: i64)`（**无返回值**），运行打印 `()`，
///          编译期零点名 ⇒ 值被静默丢弃，作者看不见任何线索；
///  - 修后：编译期打 `embed 未覆盖: python_calc::as_list —— …静默丢弃`，运行打印 i64::MAX
///          （codegen 兜底桩，见 T0r32 的既有缺口，收口要动 `src/ir/codegen/**`＝本车道禁区）。
/// 两侧都比「静默」更可归因，但**运行期仍不是编译错误** —— 这一条留给下一车道，
/// 本条只钉「编译期必须点名」这个已经兑现的面，不把兜底桩行为写成正面契约
/// （否则别的车道修好兜底桩时会被本文件拖红）。
#[test]
fn author_still_calling_a_gated_method_gets_a_named_receipt() {
    // ④：无返回注解却被调用
    let src1 = "#[embed(python)]\n```\ndef as_list(n: int):\n    return [n]\n```\n\ndef main():\n    print(python_calc_as_list(3))\n";
    let (_w1, so1, se1) = compile_case("gap_ret_call", src1, &[]);
    let r1 = format!("{so1}{se1}");
    assert!(
        r1.contains("embed 未覆盖: python_calc::as_list"),
        "调用被门挡下的函数，编译期必须点名：{r1}"
    );

    // ②：**泛型函数**（带类型参数）被点名 —— 这类签名永远在闭集外（与 java 是否在场无关），
    // 是 NO_DISPATCH_SURFACE 之外「逐条点名」口径的稳定样本（T0r17 后 scala 的普通 def 已可调用，
    // 不能再拿它当 gated 样本）。
    let src2 = "#[embed(scala)]\n```\ndef pick[T](xs: List[T]): T = xs(0)\n```\n\ndef main():\n    print(1)\n";
    let (_w2, so2, se2) = compile_case("gap_generic_call", src2, &[]);
    let r2 = format!("{so2}{se2}");
    assert!(
        r2.contains("embed 未覆盖: scala_calc::pick") || r2.contains("泛形函数"),
        "闭集外的泛型函数必须点名：{r2}"
    );
}

/// 不变量守住：换尺逻辑只返回数据，生成物里的 `Command::new` 仍是恰好两处
/// （embed_unify_r15 也钉这条，这里从**文本面**再钉一次，防止将来有人图省事在垫片里多起一个进程）。
#[test]
fn shim_still_has_exactly_two_spawn_sites_after_the_bat_route() {
    let t = target("python", "a.lz", "def add(a: int, b: int) -> int:\n    return a + b\n");
    let shim = lang_zone::embed::shim::module_text(std::slice::from_ref(&t));
    assert_eq!(
        shim.matches("Command::new(").count(),
        2,
        "垫片的两个 runner 各一处，多一处就是漏到了别处:\n{shim}"
    );
    assert!(shim.contains("__lz_spawn_plan(&dir, program)"), "{shim}");
    assert!(shim.contains("__lz_resolve_program"), "T0r49 的相对程序名解析仍在:\n{shim}");
}

/// ③ 的**垫片侧**运行时判据：把生成物里的 `__lz_spawn_plan` 单独取出来编成小程序，
/// 让**这份生成代码本身**（而不是 Rust 侧的镜像 needs_cmd_route）面对一个只有 `.bat` 包装的工具链。
/// 修前这条不存在：生成物按裸名 CreateProcess ⇒ `program not found`。
/// 本机没有「有分发入口 + run 是 .bat 裸名」的语言（scala 无入口、python 有 .exe），
/// 所以垫片运行期的 bat 面只能用这种隔离方式真跑，**不谎称端到端验证过**。
#[test]
fn generated_shim_plan_really_spawns_a_bat_only_tool() {
    if !cfg!(windows) {
        println!("[gapfix③-shim] 非 Windows ⇒ 本条不适用");
        return;
    }
    let _guard = path_guard();
    let t = target("python", "a.lz", "def add(a: int, b: int) -> int:\n    return a + b\n");
    let shim = lang_zone::embed::shim::module_text(std::slice::from_ref(&t));
    // 只取垫片头里的工具函数（到第一个 pub fn <artifact>_ 之前），保证测的是生成代码原文
    let cut = shim.find("pub fn python_a_").expect("垫片体");
    let harness = format!(
        "{}\nfn main() {{\n    let dir = std::path::PathBuf::from(std::env::var(\"PROBE_DIR\").unwrap());\n    let (prog, pre) = __lz_spawn_plan(&dir, \"lzbatsim3\");\n    println!(\"plan={{:?}} {{:?}}\", prog, pre);\n    let mut c = Command::new(prog);\n    c.args(&pre);\n    c.args([\"add\", \"a=1\"]);\n    match c.output() {{\n        Ok(o) => println!(\"code={{:?}} out={{}}\", o.status.code(), String::from_utf8_lossy(&o.stdout).trim()),\n        Err(e) => println!(\"ERR {{}}\", e),\n    }}\n}}\n",
        &shim[..cut]
    );
    let dir = tmp_root("shimbat");
    std::fs::write(
        dir.join("lzbatsim3.bat"),
        "@echo off\r\necho OK:42 %*\r\n",
    )
    .unwrap();
    let src = dir.join("harness.rs");
    std::fs::write(&src, harness).unwrap();
    let exe = dir.join("harness.exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021", "-o"])
        .arg(&exe)
        .arg(&src)
        .output()
        .expect("rustc harness");
    assert!(rc.status.success(), "生成码编不过: {}", String::from_utf8_lossy(&rc.stderr));
    let run = Command::new(&exe)
        .env("PATH", format!("{};{}", dir.display(), std::env::var("PATH").unwrap_or_default()))
        .env("PROBE_DIR", &dir)
        .output()
        .expect("run harness");
    let out = String::from_utf8_lossy(&run.stdout).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.contains("plan=\"cmd\" [\"/C\", \"lzbatsim3\"]"), "生成码没换成 cmd /C: {out}");
    assert!(out.contains("out=OK:42"), "换尺后仍没真跑起来: {out}");
}
