//! fist-mbt **T0r21** / namespace `lang-zone` / assignee `atomcode-beh`
//!
//! # 行为差分门禁：native 产物 vs `--emit=rs-lz` 产物的**运行结果**比对
//!
//! ## 为什么需要这个 target（动机）
//!
//! `Reports/redlist-2026-10-28-r3.md` 把 rs-lz 的**编译**红名单从 40 条压到 0 条，
//! 但同轮的行为差分实验暴露出一个编译级门禁**永远看不见**的缺陷面：
//! 30 件「新转绿」里 **10 件运行死循环**（native RC=0 / rs-lz RC=124）。
//! 根因是发射器把「块内对已在作用域内的变量重赋」发成 `let mut x = …`
//! （Rust shadow-let），外层变量被冻住 → `while i < n` 永不推进。
//! 该缺陷在 r1 里被编译错误**遮住**（那 10 件当时根本编不过），所以
//! 「编译绿」与「跑得对」必须是两个独立门禁。本文件固化后者。
//!
//! ## 方法（每件样本两条链路，各编各跑，比退出码 + stdout）
//!
//! ```text
//! native： lang-zone <abs>.lz                    -> <abs>.rs     -> rustc -> exe -> run(30s)
//! rs-lz ： lang-zone <abs>.lz --emit=rs-lz       -> stdout(源码) -> rustc -> exe -> run(30s)
//! ```
//!
//! 三个**必须踩准**的实现细节（踩错就不会报错，只会静默测错东西）：
//!
//! 1. **`--emit=rs-lz` 的 stdout 就是目标程序的 Rust 源码**，诊断信息走 stderr
//!    （`Generated <in> -> <out>.lzrs`）。CLI 自己会在入参同级写
//!    `<stem>.lzrs` / `<stem>.rs` / `<stem>.exe`（自举路线 B 的自检链会真跑一遍），
//!    所以**绝不能**把 stdout 重定向到那个同名 `.rs`——那会把 CLI 自己刚写的
//!    自检产物踩掉。本文件写到独立的 `gen_rslz_source.rs`。
//! 2. **两侧必须用各自的目录 + 各自的文件副本**：native 链会写 `<stem>.rs`，
//!    rs-lz 链的自检也写同名 `<stem>.rs`，共用目录会互相覆盖（能跑出「假 SAME」）。
//! 3. **运行必须带 30s 上限**：被测缺陷形态正是「编得过但永不退出」，没有上限
//!    就会把 `cargo test` 挂死（挂死 ≠ 失败，也不可诊断）。
//!
//! ## 判据
//!
//! - 两侧产物都 `rustc` 接受（编译失败 → 单项失败，`rs-lz` 侧记 E-code 回归）；
//! - 两侧都不得 30s 超时；
//! - **退出码一致**；
//! - **stdout 逐字节一致**（仅做 CRLF→LF 归一，**不做任何排序/去空白**）。
//!
//! 归一化里**故意不排序**：排序等于承认「输出顺序不确定」，而本集合按
//! `EXCLUDED_CASES` 的判据把这类样本全部排除了（见下）。若哪天有人为了让它绿
//! 而加排序，等于把 HashMap 序假红洗成真绿 —— 这正是本门禁要防的作弊面。
//!
//! ## 样本集合（15 件）
//!
//! ### A. 必含：r3 里实测死循环的 10 件（`REQUIRED_LOOP_CASES`）
//!
//! | 样本 | r3 生成物里的遮蔽点数 |
//! | --- | --- |
//! | `div-tools/probes/p40_indent_tokenize.lz` | 1 |
//! | `div-tools/probes/p41_full_tokenize.lz` | 2 |
//! | `div-tools/probes/p42_min_block.lz` | 4 |
//! | `div-tools/probes/p43_nested_block.lz` | 4 |
//! | `div-tools/probes/p44_nested_fixed.lz` | 1 |
//! | `div-tools/probes/.diffwork/base_parser.lz` | 8 |
//! | `div-tools/probes/.diffwork/p_t1a.lz` | 9 |
//! | `div-tools/probes/.diffwork/parser_step1.lz` | 7 |
//! | `div-tools/probes/.diffwork/parser_step2.lz` | 8 |
//! | `div-tools/probes/.diffwork/parser_v175.lz` | 9 |
//!
//! 这 10 件在 r3 的读数是 `native RC=0 / rs-lz RC=124`。**它们就是本门禁的
//! 回归基线**：修复（HEAD `181fdd1` 的作用域表 / `decl_has` 判据）一旦被改回去，
//! 这里会立刻红。
//!
//! ### B. 代表性新绿 4 件（`NEW_GREEN_CASES`，取自 r3 §3.3 的 30 件新绿）
//!
//! | 样本 | 覆盖形态 | 为何选它 |
//! | --- | --- | --- |
//! | `div-tools/probes/p18_recursive.lz` | 递归下降 + 元组返回 | 调用链/尾值，与死循环族正交 |
//! | `div-tools/probes/p23_lexer_probe.lz` | 字符串切片 `s[i..i+1]` + while 累积 | G1 切片 cast 族的行为面 |
//! | `div-tools/probes/p36_list_concat_cast.lz` | `List + [x]` 拼接 + cast | G3 拼接 not-`String` 面 |
//! | `div-tools/probes/p38_while_let_cast.lz` | `while let` + `int(str)` | G2 parse 族 + while-let 结构 |
//!
//! ### C. 从「已知债」转正 1 件（`PROMOTED_DEBT_CASES`）
//!
//! `div-tools/probes/p29_recursive_parser.lz` 在 r3 §3.4 被登记为**老绿里唯一的
//! 真行为债**（`native RC=0 / rs-lz RC=101` panic）。本文件落盘时在 HEAD 上复测
//! **已不再复现**——根因与上面的死循环族**同源**：while 体内 `p = r.1` / `done = true`
//! 被发成 shadow-let ⇒ 外层 `p` 冻在 1、循环反复 `value *= r.0` 直到 i64 溢出，
//! 同一根因在 p43/p44 上表现为死循环（RC=124），在本件上表现为 debug 溢出 panic（RC=101）。
//! 修它的是同一个「作用域表 / `decl_has`」改动（`181fdd1`），不是独立缺陷。
//!
//! 所以这里**不是**把它当已知不通过跳过，而是**转正成断言用例**：它既是 r3 §9-3
//! 点名的债，又是本族在「panic 症状面」上的代表，锁住它比锁一条新绿更值。
//!
//! ## 已知不通过台账 / 显式排除（`EXCLUDED_CASES`）
//!
//! | 样本 | 处置 | 理由 |
//! | --- | --- | --- |
//! | `div-tools/probes/p3_dict_multi.lz` | **排除**（不进断言集合） | 输出是 `print({k: v for …})` 的 dict 直接打印，Rust `HashMap` 迭代顺序由每进程随机种子决定。实测 native `{2: 11, 1: 11}` / rs-lz `{1: 11, 2: 11}`：**集合相同、顺序不同**，属假红（r3 §3.4 已定性）。按「不放进集合」处置，而不是加排序归一化——理由见上文判据。 |
//!
//! 除 `p3_dict_multi` 外**没有任何样本被跳过**。`p29_recursive_parser` 曾是真债，
//! 但复测已 SAME，故被转正（见 C），不占「已知不通过」名额。
//! 当前**已知不通过清单为空**。若将来出现真债，处置方式写进本节并**在
//! `run_behdiff_case` 之外单独登记**，不允许「失败就忽略」。
//!
//! ## 预算与并行
//!
//! - 每件样本两侧各一条 `rustc` + 一次运行；`--emit=rs-lz` 内部还要跑一遍
//!   自举自检（把 300KB 的 `lz_codegen_lib` 编成 340KB Rust 再编译），
//!   **单件独占约 50s**，是实测的主要成本项。
//! - 因此一件样本 = 一个 `#[test]`（`declare_behdiff_cases!` 生成），由 libtest
//!   默认按 `--test-threads`（= CPU 数）并行。
//! - **每个用例的产物目录在进程内唯一**：`<bin_dir>/behdiff-work/pid-<pid>/<case>/`
//!   （`native/` 与 `rslz/` 两个子目录再分家）。这是硬要求：`tests/native_variant_qual.rs`
//!   曾因多线程共用同一临时路径在 Windows 上撞文件锁；本文件用 pid + 样本名
//!   双维度唯一化，且**不跨用例复用任何路径**。
//! - 成功用例结束即删自己的产物目录（失败保留，路径写进 panic 消息，便于复现）。
//!
//! ### 实测读数（本机 8 核，`CARGO_TARGET_DIR=target/lane-beh`，默认 `--test-threads`）
//!
//! | 轮次 | 用例数 | passed | failed | libtest 耗时 | 墙钟（含 cargo 增量编译） |
//! | --- | --- | --- | --- | --- | --- |
//! | #1 | 16 | **16** | **0** | 207.29s | 3m28s |
//! | #2 | 16 | **16** | **0** | 197.24s | 3m19s |
//! | #3 | 16 | **16** | **0** | 256.14s | 4m17s |
//!
//! 用例数 16 = 15 件样本 + 1 个门禁自检（`declared_samples_are_present_and_not_skipped`）。
//! 三轮读数一致，**无 flaky**；最慢一轮 4m17s，仍在 6 分钟预算内。
//! 波动主要来自 rs-lz 侧的自举自检（每件都要把 300KB `lz_codegen_lib` 编成
//! 340KB Rust 再编译一次），与样本内容无关。
//!
//! 落盘前的**基线对照**（同一 lane 的 shell 版同构脚本，15 件逐件跑）：
//! r3 那 10 件全部从 `native RC=0 / rs-lz RC=124` 变为 `SAME(rc=0)`；
//! 4 件新绿与 p29 也都是 `SAME(rc=0)`。⇒ 本门禁锁的正是「已修好、别再改回去」的形态。
//!
//! 运行：`CARGO_TARGET_DIR=target/lane-beh cargo test --test rs_lz_behdiff`
//! 只跑单件：`… --test rs_lz_behdiff -- p42_min_block --nocapture`

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Once;
use std::time::{Duration, Instant};

// ════════════════════════════════════════════════════════════════════════════
// 样本集合（唯一真源：宏同时生成 const 与 #[test]，避免两份清单漂移）
// ════════════════════════════════════════════════════════════════════════════

/// 同时生成「样本清单 const」与「一件样本一个 #[test]」。
///
/// 清单与用例同源是刻意的：任何「清单里有、但没生成用例」或「用例跳过样本」
/// 的缝隙都会被 `declared_samples_are_present_and_not_skipped` 抓住。
macro_rules! declare_behdiff_cases {
    (
        $const_name:ident;
        $(
            $fn_name:ident => $rel:literal : $note:literal
        ),+ $(,)?
    ) => {
        #[doc = "本组样本（相对仓库根）：(相对路径, 覆盖说明)"]
        const $const_name: &[(&str, &str)] = &[$(($rel, $note)),+];

        $(
            #[doc = $note]
            #[test]
            fn $fn_name() {
                run_behdiff_case($rel);
            }
        )+
    };
}

declare_behdiff_cases! {
    REQUIRED_LOOP_CASES;
    p40_indent_tokenize => "div-tools/probes/p40_indent_tokenize.lz"
        : "r3 死循环基线 1/10：缩进 tokenize 最小面（生成物 1 处遮蔽）",
    p41_full_tokenize => "div-tools/probes/p41_full_tokenize.lz"
        : "r3 死循环基线 2/10：完整 tokenize（2 处遮蔽）",
    p42_min_block => "div-tools/probes/p42_min_block.lz"
        : "r3 死循环基线 3/10：最小复现件（4 处遮蔽，i = r.1 / line_start = false）",
    p43_nested_block => "div-tools/probes/p43_nested_block.lz"
        : "r3 死循环基线 4/10：嵌套块（4 处遮蔽）",
    p44_nested_fixed => "div-tools/probes/p44_nested_fixed.lz"
        : "r3 死循环基线 5/10：嵌套修复对照（1 处遮蔽）",
    diffwork_base_parser => "div-tools/probes/.diffwork/base_parser.lz"
        : "r3 死循环基线 6/10：parser 在途件（8 处遮蔽）",
    diffwork_p_t1a => "div-tools/probes/.diffwork/p_t1a.lz"
        : "r3 死循环基线 7/10：parser 在途件（9 处遮蔽）",
    diffwork_parser_step1 => "div-tools/probes/.diffwork/parser_step1.lz"
        : "r3 死循环基线 8/10：parser 在途件（7 处遮蔽）",
    diffwork_parser_step2 => "div-tools/probes/.diffwork/parser_step2.lz"
        : "r3 死循环基线 9/10：parser 在途件（8 处遮蔽）",
    diffwork_parser_v175 => "div-tools/probes/.diffwork/parser_v175.lz"
        : "r3 死循环基线 10/10：parser 在途件（9 处遮蔽）",
}

declare_behdiff_cases! {
    NEW_GREEN_CASES;
    p18_recursive => "div-tools/probes/p18_recursive.lz"
        : "新绿代表：递归下降 + 元组返回（调用链/尾值，与死循环族正交）",
    p23_lexer_probe => "div-tools/probes/p23_lexer_probe.lz"
        : "新绿代表：字符串切片 s[i..i+1] + while 累积（G1 切片 cast 族的行为面）",
    p36_list_concat_cast => "div-tools/probes/p36_list_concat_cast.lz"
        : "新绿代表：List + [x] 拼接与 cast（G3 拼接族的行为面）",
    p38_while_let_cast => "div-tools/probes/p38_while_let_cast.lz"
        : "新绿代表：while let + int(str)（G2 parse 族 + while-let 结构）",
}

declare_behdiff_cases! {
    PROMOTED_DEBT_CASES;
    p29_recursive_parser => "div-tools/probes/p29_recursive_parser.lz"
        : "已知债转正：r3 §3.4 记作 native RC=0 / rs-lz RC=101 panic，HEAD 复测已 SAME",
}

/// 显式排除的样本：(相对路径, 排除理由)。不进任何断言集合。
///
/// 排除只允许「输出本身不确定」这一种理由，且必须写明为什么无法归一化。
const EXCLUDED_CASES: &[(&str, &str)] = &[(
    "div-tools/probes/p3_dict_multi.lz",
    "print(dict) 的 HashMap 迭代顺序随每进程随机种子变化：\
     native {2: 11, 1: 11} vs rs-lz {1: 11, 2: 11} —— 集合相同、顺序不同，属假红。\
     按任务口径「不放进集合」处置，不加排序归一化（排序会把这类假红洗成真绿）。",
)];

/// 台账：r3 报告里登记为「已知不通过」、本文件落盘时的复测状态。
///
/// 只作记录，不参与断言——真正的判据是上面的三个样本集合。
const KNOWN_UNPASSING_LEDGER: &[(&str, &str)] = &[(
    "div-tools/probes/p29_recursive_parser.lz",
    "r3 §3.4：native RC=0 / rs-lz RC=101（rs-lz 产物运行期 panic，r1 即存在）。\
     根因与死循环族同源（while 体内 `p = r.1` 发成 shadow-let ⇒ 外层 p 冻在 1 ⇒ \
     `value *= r.0` 反复执行至 i64 溢出），修它的是同一个作用域表改动（181fdd1）。\
     T0r21 落盘时（HEAD e2b352a）连跑 4 次均 SAME（rc=0、stdout 一致），该债已不再复现 \
     ⇒ 已转正为断言用例（见 PROMOTED_DEBT_CASES），不占跳过名额。",
)];

// ════════════════════════════════════════════════════════════════════════════
// 路径与工具链定位（lane 感知：全部以本次构建出的 lang-zone.exe 所在目录为锚）
// ════════════════════════════════════════════════════════════════════════════

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lang_zone_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 生成产物需要 `--extern lz_builtins=<rlib>`（`List<T>`/`Dict<K,V>` 别名在其中）。
/// 优先取**本 lane** 的 rlib（`target/lane-xxx/debug`），再退到 deps/，
/// 最后才退到 `target/debug` —— 避免跨 lane 静默用别家产物。
fn builtins_rlib() -> PathBuf {
    let dir = lang_zone_bin().parent().expect("bin dir").to_path_buf();
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    if let Ok(entries) = fs::read_dir(dir.join("deps")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return e.path();
            }
        }
    }
    let fallback = manifest_dir().join("target/debug/liblz_builtins.rlib");
    if fallback.exists() {
        return fallback;
    }
    panic!(
        "找不到 liblz_builtins.rlib（先跑 `cargo build -p lz_builtins`），查找基准目录: {}",
        dir.display()
    );
}

fn dep_dir(rlib: &Path) -> PathBuf {
    let p = rlib.parent().expect("rlib has parent").to_path_buf();
    if p.file_name().map(|n| n == "deps").unwrap_or(false) {
        p
    } else {
        let d = p.join("deps");
        if d.exists() {
            d
        } else {
            p
        }
    }
}

/// 进程内唯一的产物根目录：`<bin_dir>/behdiff-work/pid-<pid>`。
///
/// pid 已足以区分进程；进程内再按样本名分目录（见 `case_dir`）。
/// 首次调用时清掉同 pid 的历史残留（pid 复用）。
fn work_root() -> PathBuf {
    static INIT: Once = Once::new();
    let base = lang_zone_bin()
        .parent()
        .expect("bin dir")
        .join("behdiff-work")
        .join(format!("pid-{}", std::process::id()));
    INIT.call_once(|| {
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create behdiff work root");
    });
    base
}

fn case_dir(rel: &str) -> PathBuf {
    let case = rel
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    work_root().join(case)
}

fn brief(bytes: &[u8], max: usize) -> String {
    let s = String::from_utf8_lossy(bytes);
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let head: String = t.chars().take(max).collect();
        format!("{head}…（已截断，原长 {} 字符）", t.chars().count())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 带上限的子进程执行
// ════════════════════════════════════════════════════════════════════════════

/// 一次子进程执行的结果。`timed_out` 与 `code` 分开存，因为「被杀」和
/// 「自己退出成非零」在行为差分里是**完全不同的结论**（前者=死循环/挂死）。
struct Outcome {
    timed_out: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Outcome {
    fn success(&self) -> bool {
        !self.timed_out && self.code == Some(0)
    }
    /// 供 assert 消息用的退出态摘要
    fn status_text(&self) -> String {
        if self.timed_out {
            "TIMEOUT(被杀)".to_string()
        } else {
            match self.code {
                Some(c) => format!("exit={c}"),
                None => "exit=<无 code，异常终止>".to_string(),
            }
        }
    }
}

/// 跑子进程，超过 `secs` 就 kill 并标记 `timed_out`。
///
/// 两个关键点：
/// 1. **stdout/stderr 必须由独立线程抽干**：`try_wait` 轮询期间若子进程写满
///    管道缓冲（64KB）就会阻塞在写端、永远不退出——那就把「读管道」当成了
///    「死循环」，误判成 RC=124。rs-lz 的产物源码正好可能超过 64KB。
/// 2. **stdin 置 null**：被测程序若意外读 stdin，不该把测试进程挂住。
fn run_bounded(cmd: &mut Command, secs: u64) -> Outcome {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("无法启动子进程 {:?}: {e}", cmd));

    let mut out_pipe = child.stdout.take().expect("child stdout");
    let mut err_pipe = child.stderr.take().expect("child stderr");
    let t_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = out_pipe.read_to_end(&mut v);
        v
    });
    let t_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = err_pipe.read_to_end(&mut v);
        v
    });

    let start = Instant::now();
    let status;
    let timed_out;
    loop {
        match child.try_wait().expect("try_wait") {
            Some(st) => {
                status = Some(st);
                timed_out = false;
                break;
            }
            None => {
                if start.elapsed() >= Duration::from_secs(secs) {
                    let _ = child.kill();
                    status = child.wait().ok();
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    Outcome {
        timed_out,
        code: status.and_then(|s| s.code()),
        stdout,
        stderr,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 两条链路
// ════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Native,
    RsLz,
}

impl Side {
    fn label(self) -> &'static str {
        match self {
            Side::Native => "native",
            Side::RsLz => "rs-lz",
        }
    }
    /// 转译/发射阶段的上限。native 实测 <20s；rs-lz 含自举自检，实测单件约 50s，
    /// 8 线程并跑时可达 130s —— 留足余量，但仍是一个**会失败**的上限（不是无限等）。
    fn emit_budget(self) -> u64 {
        match self {
            Side::Native => 180,
            Side::RsLz => 300,
        }
    }
}

/// 产物运行的硬上限。任务口径 30s：r3 里 native 全部 <1s，死循环那 10 件是
/// 真·永不退出，30s 足够把两者分开。
const RUN_BUDGET_SECS: u64 = 30;

/// rustc 编译产物的上限（实测 <10s，给宽松余量）。
const RUSTC_BUDGET_SECS: u64 = 180;

struct Chain {
    /// 转译（native 写成 `<stem>.rs`）或发射（`--emit=rs-lz` 打到 stdout）成功
    translated: bool,
    /// rustc 接受了产物
    compiled: bool,
    /// 运行结果（`compiled == true` 时才为 Some）
    run: Option<Outcome>,
    /// 产物 Rust 源码的字节数（用于诊断「生成物是不是空的/截断了」）
    rust_bytes: usize,
    /// 失败时的人类可读诊断
    diag: String,
}

/// 跑完整条链路：转译/发射 → rustc → 运行。
///
/// `dir` 必须是本用例**独占**的目录；`sample_name` 是落在该目录里的文件名
/// （保留原 basename，避免样本靠文件名区分模块时行为漂移）。
fn run_chain(dir: &Path, sample_name: &str, src: &[u8], side: Side) -> Chain {
    let lz_path = dir.join(sample_name);
    if let Err(e) = fs::write(&lz_path, src) {
        return Chain {
            translated: false,
            compiled: false,
            run: None,
            rust_bytes: 0,
            diag: format!("写样本副本失败 {}: {e}", lz_path.display()),
        };
    }
    // 入参必须是**绝对路径**（`--emit=rs-lz` 的自检链要按入参派生 exe 路径并运行它，
    // 相对入参在 std 的 `Command::new` 下不查 cwd，见 tests/rs_lz_relpath.rs），
    // 但**不能**用 `canonicalize()`：Windows 上它返回 `\\?\F:\…` 逐字路径，而 CLI 派生
    // 产物名时会拼出 `…\native/p42.rs` 这种混用分隔符的形式；带 `\\?\` 前缀时
    // 正斜杠不再被规范化 → `os error 123 文件名、目录名或卷标语法不正确`。
    // work_root() 本身由 CARGO_BIN_EXE 的绝对目录派生，已是绝对且非逐字路径。
    let abs_lz = lz_path.clone();
    assert!(
        abs_lz.is_absolute(),
        "产物目录必须是绝对路径（否则 --emit=rs-lz 自检链按入参派生的 exe 跑不起来）: {}",
        abs_lz.display()
    );

    let mut cmd = Command::new(lang_zone_bin());
    if side == Side::RsLz {
        cmd.arg("--emit=rs-lz");
    }
    cmd.arg(&abs_lz);

    let emit = run_bounded(&mut cmd, side.emit_budget());
    if !emit.success() {
        return Chain {
            translated: false,
            compiled: false,
            run: None,
            rust_bytes: 0,
            diag: format!(
                "{} 转译/发射失败（{}）\n  stderr: {}",
                side.label(),
                emit.status_text(),
                brief(&emit.stderr, 1200)
            ),
        };
    }

    // 产物 Rust 源码落盘：两侧都写进**本用例独占**目录，且 rs-lz 侧刻意用
    // 独立文件名——CLI 自己已在本目录写过 `<stem>.rs`（自举自检链的产物），
    // 重定向到那个名字会把它踩掉（见文件头「三个必须踩准的细节」#1）。
    let (rs_path, rust_bytes) = match side {
        Side::Native => {
            let rs = lz_path.with_extension("rs");
            if !rs.is_file() {
                return Chain {
                    translated: false,
                    compiled: false,
                    run: None,
                    rust_bytes: 0,
                    diag: format!("native 未产出 {}（转译成功但无 .rs）", rs.display()),
                };
            }
            let len = fs::metadata(&rs).map(|m| m.len() as usize).unwrap_or(0);
            (rs, len)
        }
        Side::RsLz => {
            let rs = dir.join("gen_rslz_source.rs");
            if let Err(e) = fs::write(&rs, &emit.stdout) {
                return Chain {
                    translated: false,
                    compiled: false,
                    run: None,
                    rust_bytes: 0,
                    diag: format!("写 rs-lz 产物源码失败 {}: {e}", rs.display()),
                };
            }
            (rs, emit.stdout.len())
        }
    };

    if rust_bytes == 0 {
        return Chain {
            translated: false,
            compiled: false,
            run: None,
            rust_bytes: 0,
            diag: format!(
                "{} 产物 Rust 源码为空（比对 rd 侧 stdout 是否被吞）\n  stdout: {}",
                side.label(),
                brief(&emit.stdout, 400)
            ),
        };
    }

    let exe = dir.join(format!("prog_{}.exe", side.label().replace('-', "_")));
    let rlib = builtins_rlib();
    let mut rustc_cmd = Command::new("rustc");
    rustc_cmd
        .args(["--edition", "2021"])
        .arg("--crate-name")
        .arg(match side {
            Side::Native => "lz_behdiff_native",
            Side::RsLz => "lz_behdiff_rslz",
        })
        .arg("-A")
        .arg("warnings")
        .arg(&rs_path)
        .arg("--extern")
        .arg(format!("lz_builtins={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", dep_dir(&rlib).display()))
        .arg("-o")
        .arg(&exe);
    let cc = run_bounded(&mut rustc_cmd, RUSTC_BUDGET_SECS);
    if !cc.success() {
        return Chain {
            translated: true,
            compiled: false,
            run: None,
            rust_bytes,
            diag: format!(
                "{} 产物 rustc 编译失败（{}）\n  stderr: {}",
                side.label(),
                cc.status_text(),
                brief(&cc.stderr, 1600)
            ),
        };
    }

    let mut run_cmd = Command::new(&exe);
    let run = run_bounded(&mut run_cmd, RUN_BUDGET_SECS);
    Chain {
        translated: true,
        compiled: true,
        run: Some(run),
        rust_bytes,
        diag: String::new(),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 输出归一化与差异定位
// ════════════════════════════════════════════════════════════════════════════

/// **只做换行符归一**（CRLF→LF）。不排序、不去空白、不 trim 行内空格。
///
/// 不排序是判据的一部分：本集合已排除所有输出顺序不确定的样本，
/// 所以任何 stdout 差异都应视为真差异。
fn normalize(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

/// 给出第一条差异行 + 若干上下文，避免把整个 stdout 灌进 panic 消息。
fn first_stdout_diff(native: &str, rslz: &str) -> String {
    let a: Vec<&str> = native.split('\n').collect();
    let b: Vec<&str> = rslz.split('\n').collect();
    let total = a.len().max(b.len());
    for i in 0..total {
        let la = a.get(i).copied().unwrap_or("<缺行>");
        let lb = b.get(i).copied().unwrap_or("<缺行>");
        if la != lb {
            let from = i.saturating_sub(2);
            let mut ctx = String::new();
            for j in from..=i.min(total.saturating_sub(1)) {
                ctx.push_str(&format!(
                    "    L{j}: native={:?}\n",
                    a.get(j).copied().unwrap_or("<缺行>")
                ));
            }
            return format!(
                "首个差异在第 {} 行（共 native {} 行 / rs-lz {} 行）\n    native: {:?}\n    rs-lz : {:?}\n  上下文:",
                i + 1,
                a.len() - 1,
                b.len() - 1,
                la,
                lb
            ) + &format!("\n{ctx}");
        }
    }
    "（逐行相同：差异只可能来自行尾/空输出以外的字节）".to_string()
}

// ════════════════════════════════════════════════════════════════════════════
// 单件用例
// ════════════════════════════════════════════════════════════════════════════

fn run_behdiff_case(rel: &str) {
    let corpus = manifest_dir().join(rel);
    assert!(
        corpus.is_file(),
        "[{rel}] 样本不存在：{}（样本被改名/删除时本门禁必须红，不许静默变绿）",
        corpus.display()
    );
    let src = fs::read(&corpus).expect("read sample");
    let sample_name = corpus
        .file_name()
        .expect("sample basename")
        .to_string_lossy()
        .to_string();

    // 每件样本独占一个目录；native / rs-lz 再各占一个子目录（细节 #2）。
    let root = case_dir(rel);
    let _ = fs::remove_dir_all(&root);
    let nat_dir = root.join("native");
    let rlz_dir = root.join("rslz");
    fs::create_dir_all(&nat_dir).expect("create native dir");
    fs::create_dir_all(&rlz_dir).expect("create rslz dir");

    let nat = run_chain(&nat_dir, &sample_name, &src, Side::Native);
    let rlz = run_chain(&rlz_dir, &sample_name, &src, Side::RsLz);

    assert!(
        nat.translated,
        "[{rel}] native 转译失败（基线链路断了，先修基线再谈差分）\n  {}\n  产物目录: {}",
        nat.diag,
        root.display()
    );
    assert!(
        rlz.translated,
        "[{rel}] rs-lz 发射失败（--emit=rs-lz 侧断了）\n  {}\n  产物目录: {}",
        rlz.diag,
        root.display()
    );
    assert!(
        nat.compiled,
        "[{rel}] native 产物 rustc 编译失败\n  {}\n  产物目录: {}",
        nat.diag,
        root.display()
    );
    assert!(
        rlz.compiled,
        "[{rel}] rs-lz 产物 rustc 编译失败（E-code 回归）\n  {}\n  产物目录: {}",
        rlz.diag,
        root.display()
    );

    let nr = nat.run.as_ref().expect("native run outcome");
    let rr = rlz.run.as_ref().expect("rs-lz run outcome");

    assert!(
        !nr.timed_out,
        "[{rel}] native 产物 {} 内未退出 —— 基线侧不该挂住，先查样本/环境\n  产物目录: {}",
        RUN_BUDGET_SECS,
        root.display()
    );
    assert!(
        !rr.timed_out,
        "[{rel}] rs-lz 产物 {} 内未退出 —— **死循环**（T0r20 遮蔽族形态：块内重赋发成 `let mut`，\
         外层循环变量被冻住）。对照 r3：该件 native {} / rs-lz RC=124。\n  产物目录: {}",
        RUN_BUDGET_SECS,
        nr.status_text(),
        root.display()
    );

    assert_eq!(
        nr.code,
        rr.code,
        "[{rel}] 两侧退出码不一致（native {} vs rs-lz {}）\n  产物目录: {}",
        nr.status_text(),
        rr.status_text(),
        root.display()
    );

    let n_out = normalize(&nr.stdout);
    let r_out = normalize(&rr.stdout);
    assert!(
        !n_out.trim().is_empty(),
        "[{rel}] native 产物没有任何 stdout —— 本集样本都应当有输出，空输出说明样本或管线异常"
    );
    assert_eq!(
        n_out,
        r_out,
        "[{rel}] stdout 不一致（native {} vs rs-lz {}）\n  {}\n  native stdout 长度 {} / rs-lz {}（Rust 源码 {} 字节）\n  产物目录: {}",
        nr.status_text(),
        rr.status_text(),
        first_stdout_diff(&n_out, &r_out),
        n_out.len(),
        r_out.len(),
        rlz.rust_bytes,
        root.display()
    );

    // 全绿才清产物；失败保留现场（路径已写进上面的 panic 消息）。
    let _ = fs::remove_dir_all(&root);
}

// ════════════════════════════════════════════════════════════════════════════
// 门禁自检：样本必须都在、排除项不许混进来、清单不许被悄悄改小
// ════════════════════════════════════════════════════════════════════════════

/// 本用例不跑任何样本，只守住「门禁本身不许作弊」的四条：
///
/// 1. 三个集合里的每个样本文件都真实存在、非空、含 `def main`；
/// 2. 10 件 r3 死循环基线**一件都不能少**（防止有人删样本让门禁变绿）；
/// 3. `EXCLUDED_CASES` 里的样本**不得**出现在任何断言集合（防止假红混入）；
/// 4. 排除项必须写清理由（非空），且必须真能读到（排除也要可复核）。
#[test]
fn declared_samples_are_present_and_not_skipped() {
    let all: Vec<(&str, &str)> = REQUIRED_LOOP_CASES
        .iter()
        .copied()
        .chain(NEW_GREEN_CASES.iter().copied())
        .chain(PROMOTED_DEBT_CASES.iter().copied())
        .collect();

    // ① 样本存在性 + 形态
    for (rel, note) in &all {
        let p = manifest_dir().join(rel);
        assert!(p.is_file(), "样本缺失: {rel}（{note}）");
        let text = fs::read_to_string(&p).unwrap_or_else(|e| panic!("读 {rel} 失败: {e}"));
        assert!(!text.trim().is_empty(), "样本为空: {rel}");
        assert!(
            text.contains("def main"),
            "样本没有 def main（不可运行，不该进行为差分集合）: {rel}"
        );
        assert!(!note.trim().is_empty(), "样本缺少覆盖说明: {rel}");
    }

    // ② 基线件数不许缩水（任务口径：必含 r3 那 10 件）
    assert_eq!(
        REQUIRED_LOOP_CASES.len(),
        10,
        "r3 死循环基线必须是 10 件，实际 {} 件",
        REQUIRED_LOOP_CASES.len()
    );
    for must in [
        "div-tools/probes/p40_indent_tokenize.lz",
        "div-tools/probes/p41_full_tokenize.lz",
        "div-tools/probes/p42_min_block.lz",
        "div-tools/probes/p43_nested_block.lz",
        "div-tools/probes/p44_nested_fixed.lz",
        "div-tools/probes/.diffwork/base_parser.lz",
        "div-tools/probes/.diffwork/p_t1a.lz",
        "div-tools/probes/.diffwork/parser_step1.lz",
        "div-tools/probes/.diffwork/parser_step2.lz",
        "div-tools/probes/.diffwork/parser_v175.lz",
    ] {
        assert!(
            REQUIRED_LOOP_CASES.iter().any(|(r, _)| *r == must),
            "必含样本被删出基线集合: {must}"
        );
    }

    // ③ 代表性新绿 3~5 件（任务口径）
    assert!(
        (3..=5).contains(&NEW_GREEN_CASES.len()),
        "代表性新绿应 3~5 件，实际 {} 件",
        NEW_GREEN_CASES.len()
    );

    // ④ 排除项不得混进断言集合，且理由必须写清
    assert!(
        !EXCLUDED_CASES.is_empty(),
        "EXCLUDED_CASES 不该为空：至少 p3_dict_multi（HashMap 序）需要显式登记"
    );
    for (rel, why) in EXCLUDED_CASES {
        assert!(
            !why.trim().is_empty(),
            "排除项没有写理由（不允许『跳过失败样本』式排除）: {rel}"
        );
        assert!(
            manifest_dir().join(rel).is_file(),
            "被排除的样本本身也应当存在（排除要可复核）: {rel}"
        );
        assert!(
            !all.iter().any(|(r, _)| r == rel),
            "被排除的样本混进了断言集合: {rel}"
        );
    }

    // ⑤ 台账里的每条都必须仍在断言集合里（本文件把已知债转正，不留跳过名额）
    for (rel, status) in KNOWN_UNPASSING_LEDGER {
        assert!(
            !status.trim().is_empty(),
            "台账条目缺状态说明: {rel}"
        );
        assert!(
            all.iter().any(|(r, _)| r == rel),
            "台账样本未进入断言集合（若是真债应写清处置，不许静默丢弃）: {rel}"
        );
    }
}
