// Lang-Zone 编译器 — embed/mod.rs
// 嵌入式语法统一抽象（规划：plans/azure-storm-starling.md）
//
// 只有两档 tier，判据是「值怎么跨过边界」，不是「内嵌的是哪门语言」：
//   Source —— 纯转译为 Rust 源码（tnr / 原样 rust），编译期消解，零运行时开销。
//   Cli    —— 其余一切：摘源码 → 用该语言自己的工具链建库编译 → 值只走
//             CLI 命令行 + 结果序列化（LineProtocol，stdin 一行请求 / stdout 一行响应）。
//
// 明确不做：LinkTime(静态链接) / Runtime(DLL·so) / SharedMemory(共享内存) 三档。
// 「最容易的路」是本抽象的第一约束，不是兜底方案。
//
// 产物身份由**摘出来的外语言源码里有没有 main** 决定，不由作者额外声明：
//   有 main → Executable：建出来直接执行，结果取其 stdout。
//   无 main → Library：产物名即模块名，既能被同语言程序 import，
//             也能被内嵌它的 LZ 经生成的 CLI 垫片调用。
//
// 已知限制（写在这里而不是藏在实现里）：LineProtocol 的 `key=value` 以空格分隔，
// 故字符串参数含空格/换行/`=` 不被本版本支持 —— 这类签名在 ForeignFn.unsupported
// 里逐条记账，不出垫片（宁可不生成，也不生成一个会静默错位的调用面）。

pub mod adapters;
pub mod artifact;
pub mod dedup;
pub mod emit;
pub mod shim;

use std::path::Path;

pub use adapters::{adapter, is_source_lang, source_langs};

/// 跨界方式只有两档
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedTier {
    /// Level 0：编译期消解，外语言源码 → Rust 源码
    Source,
    /// Level 3：跨进程，CLI 管道 + 序列化
    Cli,
}

/// 产物身份：由 has_main 判
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Executable,
    Library,
}

/// 一次外部命令的形状（不执行，只描述 —— 执行面在 artifact::build_all / 生成的垫片里）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandSpec {
    pub fn new(program: &str, args: &[&str]) -> Self {
        CommandSpec {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    pub fn with_args(program: &str, args: Vec<String>) -> Self {
        CommandSpec {
            program: program.to_string(),
            args,
        }
    }

    /// 确定性文本：进 MANIFEST 与报告，不含本机绝对路径
    pub fn display(&self) -> String {
        let mut s = self.program.clone();
        for a in &self.args {
            s.push(' ');
            s.push_str(a);
        }
        s
    }

    pub fn tool_available(&self) -> bool {
        tool_available(&self.program)
    }
}

/// Windows 上「裸名 + PATH 上只有 `.bat`/`.cmd` 包装」的工具链（Coursier 装的 scala/scalac 是典型）。
///
/// 为什么要单独判：`tool_available` 按文件名扫 PATH，认 `scalac.bat`；而
/// `std::process::Command::new("scalac")` 走 CreateProcess —— 裸名只会补 `.exe`，
/// **不认 PATHEXT** ⇒ 恒 `program not found`（实测 T0r49 §2.4：MANIFEST 写 yes、spawn 说找不到，
/// 两把尺不一致）。这类工具链必须经 `cmd /C` 起，`cmd` 自己按 PATHEXT 解析。
///
/// 换尺边界由本机实测划出（temp/gapfix/quot2 + quot4，同一台 Windows，可复跑）：
///  - `Command::new("lztool")`（PATH 上只有 lztool.bat/lztool.cmd）⇒ **ERR program not found** ⇒ 要换尺
///  - `Command::new("lztool.bat")`（裸名带扩展名）⇒ 起得来 ⇒ 不换
///  - `Command::new(<绝对 .bat 路径>)` ⇒ 起得来 ⇒ 不换
///  - 裸名且 PATH 上有 `x.exe`（python/gcc 这类原生映像）⇒ 不换，行为与修前一字不动
///
/// 返回 `true` 表示「本次 spawn 要换成 `cmd` + `/C <原程序> <原参数…>`」。
/// 非 Windows 恒 `false`（那里 .bat 本来就不是可执行映像，不属本问题的范围）。
pub fn needs_cmd_route(program: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let lower = program.to_ascii_lowercase();
    // 带扩展名或含分隔符的形态，CreateProcess 自己解得开（见上实测边界），原样起
    if lower.ends_with(".bat") || lower.ends_with(".cmd") || lower.ends_with(".exe") {
        return false;
    }
    if program.contains('/') || program.contains('\\') {
        return false;
    }
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let dirs: Vec<std::path::PathBuf> = std::env::split_paths(&path).collect();
    // CreateProcess 对裸名的语义：只补 `.exe` ⇒ 有同名 .exe 就不换尺
    if dirs.iter().any(|d| d.join(format!("{}.exe", program)).is_file()) {
        return false;
    }
    // 没有任何可直接执行的映像时，才认 .bat/.cmd 包装并换 `cmd /C`
    dirs.iter().any(|d| {
        d.join(format!("{}.bat", program)).is_file()
            || d.join(format!("{}.cmd", program)).is_file()
    })
}

/// 把一条命令换成实际可 spawn 的形状：需要时前置 `cmd /C` 包装段。
/// 返回 `(实际程序, 要插在原实参之前的段)`。
pub fn cmd_route_shape(program: &str) -> (String, Vec<String>) {
    if needs_cmd_route(program) {
        ("cmd".to_string(), vec!["/C".to_string(), program.to_string()])
    } else {
        (program.to_string(), Vec::new())
    }
}

/// 工具链在场性：扫 PATH，**不 spawn** ⇒ 无副作用、不受被测量程序影响
pub fn tool_available(program: &str) -> bool {
    if program.contains('/') || program.contains('\\') {
        return Path::new(program).exists();
    }
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let mut names = vec![program.to_string()];
    for suffix in [".exe", ".cmd", ".bat"] {
        names.push(format!("{}{}", program, suffix));
    }
    for dir in std::env::split_paths(&path) {
        for n in &names {
            if dir.join(n).is_file() {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignParam {
    pub name: String,
    /// LZ 侧的基础类型名（闭集：int / float / str / bool）；不在闭集里的走 unsupported
    pub lz_type: String,
}

/// 从外语言源码里摘出的一个可调用函数
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignFn {
    pub name: String,
    pub params: Vec<ForeignParam>,
    /// None = 无返回值（生成的垫片按 Unit 处理）
    pub ret: Option<String>,
    /// Some(原因) ⇒ 该签名不在闭集内，**不出 CLI 垫片**，逐条记账
    pub unsupported: Option<String>,
}

impl ForeignFn {
    pub fn callable(&self) -> bool {
        self.unsupported.is_none()
    }
}

/// 库产物没有分发入口时的点名文案（进 `unsupported`、MANIFEST 与「embed 未覆盖」回执）。
/// 判据文本本身是可归因的：测试与报告都按这个串认，改它必须同批改判据。
pub const NO_DISPATCH_SURFACE: &str = "该语言本版本不生成分发入口 ⇒ 无 CLI 调用面，只可被同语言导入";

/// 一个 `#[embed(lang)]` 块的完整解析结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedTarget {
    pub lang: String,
    /// 对外可见的产物名 = `<语言>_<源文件主干>`，如 `mojo_input` / `python_input`；
    /// 模块在产物根的子目录里时再补 `__<目录slug>_<路径短哈希>`（`src/embed/dedup.rs`），
    /// 保证同名跨模块产物不互相覆盖。
    pub artifact: String,
    pub tier: EmbedTier,
    pub kind: ArtifactKind,
    /// 产物目录（仓内相对路径，确定性文本）
    pub dir_rel: String,
    /// 摘出来的文件：(产物内相对路径, 内容)
    pub files: Vec<(String, String)>,
    pub methods: Vec<ForeignFn>,
    /// None = 该语言无需建库步骤（解释型）
    pub build: Option<CommandSpec>,
    /// 运行命令（目录由垫片经 `LZ_EMBED_DIR` 解析，故这里只有 program + 相对参数）
    pub run: CommandSpec,
    /// 工具链不在场时的提示语
    pub hint: String,
}

impl EmbedTarget {
    /// 可被 LZ 侧生成调用垫片的库函数（Library 档）
    ///
    /// 两道门，缺一都会产出「调得起来的样子、实际必失败」的假垫片：
    /// ① 签名本身可序列化（`ForeignFn::callable()`）；
    /// ② **Library 档必须有分发入口**（`has_dispatch_file()`）。②是 T0r49 §2.3 补的：
    ///    scala 的 `dispatch_file()` 明确返回 `None`（JVM 入口要 object 包装，本版本不生成），
    ///    但旧实现只看①，于是 `dispatch=none` 的产物照样出 `pub fn scala_calc_add(...)`，
    ///    真调起来打到 `scala scala_calc.scala add a=1 b=2`（没有分发入口）⇒ 客户端拿「无响应」panic。
    ///    被②挡下的方法在 `resolve()` 里已就地记进 `unsupported`（保持「逐条点名」口径），
    ///    这里再挡一次是给手工构造的 EmbedTarget 兜底 —— 两处同一判据，不产生第二套语义。
    pub fn callable_methods(&self) -> Vec<&ForeignFn> {
        let surface_gated = self.kind == ArtifactKind::Library && !self.has_dispatch_file();
        self.methods
            .iter()
            .filter(|m| m.callable() && !surface_gated)
            .collect()
    }

    pub fn unsupported_methods(&self) -> Vec<&ForeignFn> {
        self.methods.iter().filter(|m| !m.callable()).collect()
    }

    /// 产物目录里是否存在 CLI 分发入口（Library 档没有它 = 不可被 LZ 调用，只能被同语言导入）
    pub fn has_dispatch_file(&self) -> bool {
        self.files.iter().any(|(p, _)| p.contains("dispatch"))
    }

    pub fn entry_rel(&self) -> String {
        self.files
            .iter()
            .map(|(p, _)| p.clone())
            .find(|p| p.ends_with(&format!(".{}", self.lang_src_ext())))
            .unwrap_or_else(|| format!("{}.{}", self.artifact, self.lang_src_ext()))
    }

    fn lang_src_ext(&self) -> String {
        crate::embed::adapter(&self.lang).src_ext().to_string()
    }
}

/// 每门语言一个 adapter —— 新增语言只写这个 trait，不写第二套 codegen
pub trait EmbedLang {
    fn lang(&self) -> &str;

    /// 摘出来的源文件扩展名
    fn src_ext(&self) -> &str;

    /// 由源码判「有没有 main」，不由作者声明
    fn has_main(&self, src: &str) -> bool;

    /// 摘出顶层可调用函数（含类型闭集判定与 unsupported 记账）
    fn methods(&self, src: &str) -> Vec<ForeignFn>;

    /// 摘源码 → (产物内相对路径, 内容)
    fn extract(&self, artifact: &str, src: &str) -> Vec<(String, String)>;

    /// 无 main 的库产物需要一个 CLI 分发入口；无法生成则 None（⇒ 只落盘、不出调用面）
    fn dispatch_file(&self, artifact: &str, methods: &[&ForeignFn]) -> Option<(String, String)>;

    /// 建库/建可执行命令；None = 无需该步
    fn build_cmd(&self, kind: ArtifactKind, entry: &str) -> Option<CommandSpec>;

    /// 运行命令（entry 为产物内相对路径）
    fn run_cmd(&self, kind: ArtifactKind, entry: &str) -> CommandSpec;
}

/// `foo.lz` / `dir/foo` → `foo`
pub fn base_stem(source_name: &str) -> String {
    let last = source_name
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(source_name);
    last.trim_end_matches(".lz").to_string()
}

/// 解析一个 embed 块：Source 档返回 None（走既有逐字行为），Cli 档返回完整 target
///
/// 产物名（`artifact`）由 `dedup::artifact_name` 统一给出：产物根下的模块仍是
/// `<语言>_<主干>`（`python_calc`，兼容面一字不动），子目录里的模块补
/// `__<目录slug>_<路径短哈希>` —— 这样 `a/input.lz` 与 `b/input.lz` 的同名产物
/// 落到 `build/embed/input/python_input__a_*` 与 `...__b_*`，不再互相覆盖目录。
pub fn resolve(lang: &str, source_name: &str, src: &str) -> Option<EmbedTarget> {
    if is_source_lang(lang) {
        return None;
    }
    let a = adapter(lang);
    let artifact = dedup::artifact_name(a.lang(), source_name);
    let kind = if a.has_main(src) {
        ArtifactKind::Executable
    } else {
        ArtifactKind::Library
    };
    let mut files = a.extract(&artifact, src);
    let mut methods = a.methods(src);

    if kind == ArtifactKind::Library {
        let refs_owned: Vec<&ForeignFn> = methods.iter().collect();
        if let Some(d) = a.dispatch_file(&artifact, &refs_owned) {
            files.push(d);
        } else {
            // 没有分发入口 ⇒ 整个调用面都不存在：逐条点名记进 unsupported，
            // 而不是留着它们标 callable（T0r49 §2.3 的失真面）。
            // 判据字符串固定，供测试与回执归因；已被签名层点名（①）的不覆盖。
            for m in &mut methods {
                if m.unsupported.is_none() {
                    m.unsupported = Some(NO_DISPATCH_SURFACE.to_string());
                }
            }
        }
    }

    let entry = files
        .iter()
        .find(|(p, _)| p.ends_with(&format!(".{}", a.src_ext())))
        .map(|(p, _)| p.clone())
        .unwrap_or_else(|| format!("{}.{}", artifact, a.src_ext()));

    let build = a.build_cmd(kind, &entry);
    let run = a.run_cmd(kind, &entry);
    let hint = format!(
        "未找到 {} 工具链（命令 `{}`）⇒ 本产物只落盘并跳过建库/执行，不报成功",
        a.lang(),
        run.program
    );

    Some(EmbedTarget {
        lang: a.lang().to_string(),
        artifact,
        tier: EmbedTier::Cli,
        kind,
        dir_rel: String::new(),
        files,
        methods,
        build,
        run,
        hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_tier_langs_are_exactly_the_untouched_ones() {
        // 兼容面：这三档语言必须继续走今天的逐字行为，任何新语言都不得进这个集合
        for &l in source_langs() {
            assert!(is_source_lang(l), "{} 应属 Source 档", l);
        }
        assert!(!is_source_lang("mojo"));
        assert!(!is_source_lang("python"));
        assert!(!is_source_lang("unknownlang"));
    }

    #[test]
    fn base_stem_strips_dir_and_suffix() {
        assert_eq!(base_stem("foo.lz"), "foo");
        assert_eq!(base_stem("a/b/foo.lz"), "foo");
        assert_eq!(base_stem("a\\b\\foo"), "foo");
    }

    #[test]
    fn resolve_names_artifact_with_language_prefix() {
        let t = resolve("mojo", "input.lz", "fn greet(name: String) -> String:\n    return name\n").unwrap();
        assert_eq!(t.artifact, "mojo_input");
        assert_eq!(t.tier, EmbedTier::Cli);
        assert_eq!(t.kind, ArtifactKind::Library);
        // Source 档必须返回 None —— 它不归本抽象管
        assert!(resolve("tnr", "input.lz", "fn f(a, b) { a + b }").is_none());
        assert!(resolve("rust", "input.lz", "fn f() {}").is_none());
    }

    #[test]
    fn has_main_decides_executable_not_a_declaration() {
        let lib = resolve("python", "m.lz", "def add(a: int, b: int) -> int:\n    return a + b\n").unwrap();
        assert_eq!(lib.kind, ArtifactKind::Library);
        let exe = resolve("python", "m.lz", "if __name__ == \"__main__\":\n    print(1)\n").unwrap();
        assert_eq!(exe.kind, ArtifactKind::Executable);
        // Executable 不该带分发入口（它有自家 main）
        assert!(!exe.files.iter().any(|(p, _)| p.contains("dispatch")));
        assert!(lib.files.iter().any(|(p, _)| p.contains("dispatch")));
    }

    #[test]
    fn tool_probe_does_not_spawn() {
        assert!(tool_available("rustc"), "rustc 必在场，否则本判据本身坏了");
        assert!(!tool_available("definitely_not_a_real_tool_lz_r15"));
        // 含分隔符的显式路径按存在性判
        assert!(!tool_available("./no/such/thing.exe"));
    }
}
