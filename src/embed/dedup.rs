// Lang-Zone 编译器 — embed/dedup.rs
// 跨模块产物名去重（T0r19 / P1）：`build/embed/<主干>/<产物名>` 的「模块身份」消歧。
//
// 修前规则（`mod.rs::resolve` 造 artifact + `artifact.rs::dir_rel` 拼目录）：
//   artifact = `<语言>_<源文件主干>`        （`calc.lz` → `python_calc`）
//   dir_rel  = `build/embed/<主干>/<artifact>`
// 判据里**没有「模块」这一维** ⇒ `a/input.lz` 与 `b/input.lz` 各嵌一段 python 时，两次
// 编译都写 `build/embed/input/python_input`：后一次把前一次整目录覆盖（源码、dispatch、
// MANIFEST 一起没了），取回的函数面也串成同一批 `python_input_*`。
//
// 修后规则（确定性 / 无随机数 / 无时间戳 / 不读磁盘状态）：
//   模块身份 = 源文件相对**产物根**（进程 cwd —— `build/embed` 的父目录，见
//              `main.rs::emit_embed_artifacts` 用 `current_dir` 当根）的规范化路径。
//     产物根下的模块（`calc.lz`）      身份为空 ⇒ 产物名一字不动：`python_calc`
//     子目录里的模块（`pkg/input.lz`） ⇒ `python_input__<目录slug>_<路径短哈希8>`
//
// 为什么身份取「相对产物根」而不是直接哈希 argv 原样字符串：产物目录本来就相对 cwd，
// 用同一把尺量模块身份 ⇒ `lz sub/a.lz` 与 `lz /abs/repo/sub/a.lz`（cwd=repo）拿到同一个
// 名字，仓库搬机器/换盘符也不变；仓外文件退回绝对路径身份（同一 argv+cwd ⇒ 同一名字）。
//
// 为什么不给所有产物都加后缀：`python_calc` 是既有兼容面（tests/embed_unify_r15.rs 20 条
// 门禁、bridge 垫片签名、同语言 `import <artifact>` 都钉着它），产物根下的模块必须不变。
//
// 消歧后缀的形状也是硬约束：产物名会进 Python `importlib.import_module(ARTIFACT)`、
// Rust 标识符 `<artifact>_<method>`、dispatch 文件名 ⇒ 只允许 `[a-z0-9_]`（见
// `artifact_is_identifier_safe`）。目录里的中文/空格/`-` 一律折叠或丢弃，靠哈希兜底唯一性。

use std::path::Path;

/// 分隔符统一成 `/`（Windows 的 argv 可能两种都给）
fn slashed(p: &str) -> String {
    p.replace('\\', "/")
}

/// 词法归一：消 `.` / `..` / 重复分隔符；保留盘符（`F:`）、前导根（`/`、`//`）。
/// 纯字符串运算（不 canonicalize）⇒ 不依赖文件是否存在，测试可造虚拟路径。
fn lexical_clean(p: &str) -> String {
    let s = slashed(p);
    // Windows 盘符 `F:` 单独拎出来，避免被当成普通段
    let (prefix, rest) = if s.len() >= 2
        && s.as_bytes()[0].is_ascii_alphabetic()
        && s.as_bytes()[1] == b':'
    {
        (&s[..2], &s[2..])
    } else {
        ("", s.as_str())
    };
    let root = if rest.starts_with("//") {
        "//"
    } else if rest.starts_with('/') {
        "/"
    } else {
        ""
    };
    let mut out: Vec<&str> = Vec::new();
    for seg in rest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if out.last().copied().is_some_and(|last| last != "..") {
                    out.pop();
                } else if root.is_empty() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    let mut s = String::from(prefix);
    s.push_str(root);
    s.push_str(&out.join("/"));
    s
}

/// 路径相等：Windows 下盘符/目录大小写不敏感（`F:\AI` vs `f:/ai`），其余平台逐字节
fn path_eq(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// `p` 在 `base` 之下 ⇒ 返回相对段（不含前导 `/`）；同路径返回空串；否则 None。
/// 只在**整段边界**上认前缀，避免 `/repo2` 被 `/repo` 吞掉。
fn strip_under(p: &str, base: &str) -> Option<String> {
    let b = base.trim_end_matches('/');
    if b.is_empty() {
        return None;
    }
    let head = p.get(..b.len())?;
    if !path_eq(head, b) {
        return None;
    }
    match p.as_bytes().get(b.len()) {
        None => Some(String::new()),
        Some(b'/') => Some(p.get(b.len() + 1..).unwrap_or("").to_string()),
        _ => None,
    }
}

/// 模块身份键：源文件相对产物根的规范化路径。
/// 返回 None ⇔ 模块就在产物根下（`calc.lz`）⇒ 保持历史命名。
pub fn module_key(source_name: &str) -> Option<String> {
    let given = slashed(source_name);
    let rooted = Path::new(&given).is_absolute() || given.starts_with('/');
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| lexical_clean(&p.to_string_lossy()));

    // 先归一成「绝对身份」：相对路径按 cwd 展开，`..` 在这里被消掉 ⇒ 换 cwd 表述同一文件
    // 时（`lz ../other/a.lz` 与 `cd .. && lz other/a.lz`）得到同一个身份。
    let identity = if rooted {
        lexical_clean(&given)
    } else {
        match &cwd {
            Some(c) => lexical_clean(&format!("{}/{}", c.trim_end_matches('/'), given)),
            None => lexical_clean(&given),
        }
    };

    // 在产物根之下 ⇒ 用「仓库内相对路径」当身份（换机器/换盘符不变）；
    // 仓外 ⇒ 用绝对身份（同一 argv+cwd 内仍确定）。
    let rel = match &cwd {
        Some(c) => strip_under(&identity, c).unwrap_or_else(|| identity.clone()),
        None => identity.clone(),
    };

    if rel.is_empty() || !rel.contains('/') {
        return None; // 产物根下 / 纯文件名 ⇒ 不消歧
    }
    Some(rel)
}

/// 目录段 → 可读 slug：取末两级目录，ASCII 小写化，非 `[a-z0-9_]` 折叠成 `_`，≤24 字符。
/// 只用于可读性；唯一性由 `short_hash` 保证。
fn dir_slug(rel: &str) -> String {
    let segs: Vec<&str> = rel.split('/').collect();
    let dirs = &segs[..segs.len().saturating_sub(1)];
    let mut out = String::new();
    for seg in dirs.iter().rev().take(2).rev() {
        let mut piece = String::new();
        for ch in seg.chars() {
            let c = ch.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                piece.push(if c == '-' { '_' } else { c });
            }
        }
        while piece.contains("__") {
            piece = piece.replace("__", "_");
        }
        let piece = piece.trim_matches('_');
        if piece.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('_');
        }
        out.push_str(piece);
        if out.len() >= 24 {
            break;
        }
    }
    let mut s = out;
    s.truncate(24);
    s.trim_matches('_').to_string()
}

/// FNV-1a 64：恒定函数、跨进程/跨平台一致（不用 std 的 RandomState —— 那是随机种子）。
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 路径短哈希（8 位十六进制）：高低 32 位折叠后取，避免低位混合不足。
fn short_hash(s: &str) -> String {
    let h = fnv1a64(s.as_bytes());
    format!("{:08x}", (((h >> 32) ^ h) & 0xffff_ffff) as u32)
}

/// 产物名（唯一命名入口）：
///   `base = <语言>_<主干>`；模块在产物根下 ⇒ 原样返回（兼容面）；
///   否则 `base__<目录slug>_<哈希8>`（目录全不可读时退化为 `base__<哈希8>`）。
pub fn artifact_name(lang: &str, source_name: &str) -> String {
    let base = format!("{}_{}", lang, super::base_stem(source_name));
    match module_key(source_name) {
        None => base,
        Some(rel) => {
            let slug = dir_slug(&rel);
            let h = short_hash(&rel);
            if slug.is_empty() {
                format!("{}__{}", base, h)
            } else {
                format!("{}__{}_{}", base, slug, h)
            }
        }
    }
}

/// 产物名是否会破坏 Python 模块名 / Rust 标识符 / 文件名三处消费方。
pub fn artifact_is_identifier_safe(artifact: &str) -> bool {
    !artifact.is_empty()
        && artifact
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && artifact
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::artifact::dir_rel;

    #[test]
    fn root_level_module_keeps_legacy_name() {
        // 兼容面：`calc.lz` 就在产物根下，产物名一字不许动（r15 20 条门禁钉着）
        assert_eq!(artifact_name("python", "calc.lz"), "python_calc");
        assert_eq!(artifact_name("mojo", "input.lz"), "mojo_input");
        // 但「主干相同、模块不同」必须消歧（Windows 两种分隔符都认，不能把整条路径当主干）
        let nested = artifact_name("python", "a\\b\\calc.lz");
        assert!(nested.starts_with("python_calc__"), "{nested}");
        assert_ne!(nested, "python_calc");
    }

    #[test]
    fn same_stem_in_different_modules_gets_distinct_names_and_dirs() {
        let a = artifact_name("python", "alpha/input.lz");
        let b = artifact_name("python", "beta/input.lz");
        assert_ne!(a, b, "不同模块的同名产物必须消歧: {a} vs {b}");
        assert!(a.starts_with("python_input__"), "{a}");
        assert!(b.starts_with("python_input__"), "{b}");
        // 目录（`build/embed/<主干>/<产物名>`）随之不同 ⇒ 互不覆盖
        assert_ne!(dir_rel("input", &a), dir_rel("input", &b));
        assert!(dir_rel("input", &a).starts_with("build/embed/input/"));
    }

    #[test]
    fn naming_is_deterministic_and_order_free() {
        // 同一输入两次调用逐字相同；与调用顺序、与别的模块是否出现过都无关
        let inputs = ["pkg/a/input.lz", "pkg/b/input.lz", "input.lz"];
        let first: Vec<String> = inputs
            .iter()
            .map(|s| artifact_name("python", s))
            .collect();
        let second: Vec<String> = inputs
            .iter()
            .rev()
            .map(|s| artifact_name("python", s))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        assert_eq!(first, second, "命名不许依赖调用顺序/全局状态");
    }

    #[test]
    fn relative_and_absolute_spellings_agree() {
        // 身份是「相对产物根」，不是 argv 原样字符串 ⇒ 同一文件两种写法同一名字
        let cwd = std::env::current_dir().unwrap();
        let rel = artifact_name("python", "a/input.lz");
        let abs = artifact_name(
            "python",
            &cwd.join("a").join("input.lz").to_string_lossy(),
        );
        assert_eq!(rel, abs);
    }

    #[test]
    fn names_are_safe_for_python_and_rust() {
        // 主干部分来自文件名，既有规则是**逐字保留**（本模块不擅自改名）；
        // 本判据管的是新增的消歧段：主干本身标识符安全时，整体仍必须可消费。
        for src in ["a/input.lz", "pkg/sub-1/input_v2.lz", "x/y.lz", "input.lz"] {
            let a = artifact_name("python", src);
            assert!(
                artifact_is_identifier_safe(&a),
                "产物名 {a}（来自 {src}）不能做 Python 模块名/Rust 标识符"
            );
        }
        // 主干本身不可用于标识符时（中文名），消歧段也不许再添乱：只有 `[a-z0-9_]`
        let a = artifact_name("python", "模块/输入.lz");
        let (_, tag) = a.split_once("__").expect("子目录模块必须消歧");
        assert!(
            tag.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "消歧段 {tag} 混进了非标识符字符"
        );
    }

    #[test]
    fn deep_paths_are_still_distinct_after_slug_truncation() {
        // slug 只取末两级且截断到 24 字符 ⇒ 唯一性必须落在哈希上
        let a = artifact_name("python", "a/one/very_long_directory_segment_name/input.lz");
        let b = artifact_name("python", "a/two/very_long_directory_segment_name/input.lz");
        assert_ne!(a, b, "{a} vs {b}");
        assert!(a.len() < 80, "产物名不该长到把路径顶爆: {a}");
    }

    #[test]
    fn lexical_clean_resolves_dot_segments() {
        assert_eq!(lexical_clean("./a/./b/../input.lz"), "a/input.lz");
        assert_eq!(lexical_clean("a//b///input.lz"), "a/b/input.lz");
        assert_eq!(lexical_clean("C:\\repo\\pkg\\input.lz"), "C:/repo/pkg/input.lz");
        assert_eq!(lexical_clean("/a/../../b.lz"), "/b.lz"); // 根之上不再冒 `..`
    }

    #[test]
    fn strip_under_respects_segment_boundaries() {
        assert_eq!(strip_under("/repo/a/x.lz", "/repo").as_deref(), Some("a/x.lz"));
        assert_eq!(strip_under("/repo2/a/x.lz", "/repo"), None);
        assert_eq!(strip_under("/repo", "/repo").as_deref(), Some(""));
    }
}
