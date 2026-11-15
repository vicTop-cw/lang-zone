// Lang-Zone 编译器 — util/registry.rs
// 阶段 3（缺口 G3）：三方库（registry 包）解析与依赖校验。
//
// 背景：`lang-zone push` 已能把包发布到 `local-registry/<name>/<version>/`
// （含 lz.toml / 源文件 / checksum.txt / files.txt，实测 2026-10-16），
// 但**没有任何东西会去读它**：
//   - `Manifest`（cli.rs）此前只有 name/version/entry → 依赖无从声明（G2）
//   - import 解析（main.rs::merge_imports_into / project.rs / semantic_check.rs）
//     只在「当前模块目录」找文件，registry 不在搜索路径里（G3）
// ⇒ 「发布给他人使用」这条链路整体缺失。本模块补上读取侧。
//
// 搜索顺序（对既有行为零影响，纯增量）：
//   1. 当前模块目录下的普通文件（既有行为，优先级不变）
//   2. registry：<root>/local-registry/<pkg>/<version>/**（新增）
// 其中 <root> 由「入口文件所在目录」与「逐级向上」共同确定，
// 故 `myapp/` 内 push 出的包可被 `myapp/src/main.lz` 直接 import。
//
// 版本策略：精确匹配优先；未指定版本时取该包**最高**的已发布版本。
// 这让「写了依赖但版本对不上」成为可诊断的显式失败，而非静默取错版本。

use std::fs;
use std::path::{Path, PathBuf};

/// registry 根目录名（与 cli.rs::cmd_push 的落盘口径一致）
pub const REGISTRY_DIR_NAME: &str = "local-registry";

/// 从 `start` 逐级向上寻找含 `local-registry/` 的目录。
///
/// 逐级向上而非只看当前目录：`push` 把包落在**项目根**（`<root>/local-registry`），
/// 而 import 发生在 `<root>/src/main.lz`（或其子目录），故需从入口目录向上找。
pub fn find_registry_root(start: &Path) -> Option<PathBuf> {
    // 必须先规范化：调用方（如 ProjectPaths::root）常传**相对路径** `.`，
    // 而 `Path::new(".").parent()` 返回 None → 向上查找第一步就终止，
    // 导致 registry 与包同级时永远找不到（实测 consumer/ 在用例根、
    // registry 也在用例根的场景下 build 报「未找到 registry」）。
    let abs = std::fs::canonicalize(start).unwrap_or_else(|_| {
        // canonicalize 失败（路径不存在）时退化为手工绝对化
        if start.is_absolute() {
            start.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|c| c.join(start))
                .unwrap_or_else(|_| start.to_path_buf())
        }
    });
    let mut cur = Some(abs.as_path());
    // 最多向上 8 层，防止异常深的路径
    for _ in 0..8 {
        let dir = cur?;
        let candidate = dir.join(REGISTRY_DIR_NAME);
        if candidate.is_dir() {
            return Some(candidate);
        }
        cur = dir.parent();
    }
    None
}

/// 语义化版本三元组比较（缺失段按 0 处理，支持 "0.1" / "0.1.0" / "0.1.0-beta"）
fn parse_version(v: &str) -> (u64, u64, u64) {
    // 去掉预发布/构建元数据后缀（-xxx / +xxx）
    let core = v
        .split(['-', '+'])
        .next()
        .unwrap_or("0")
        .trim();
    let mut it = core.split('.');
    let major = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let minor = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let patch = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// 列出某包已发布的所有版本（目录名即版本号），按语义化版本**降序**。
pub fn list_versions(registry_root: &Path, pkg: &str) -> Vec<String> {
    let pkg_dir = registry_root.join(pkg);
    let Ok(rd) = fs::read_dir(&pkg_dir) else {
        return Vec::new();
    };
    let mut versions: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    versions.sort_by(|a, b| parse_version(b).cmp(&parse_version(a)));
    versions
}

/// 解析一个依赖：给定包名 + 版本约束（空串或 "*" = 取最新），返回其包根目录。
///
/// 找不到时返回带**可操作诊断**的错误（列出该包已发布的版本），
/// 而不是静默回退 —— 静默取错版本是包管理最難排查的问题。
pub fn resolve_package(
    registry_root: &Path,
    pkg: &str,
    version_spec: &str,
) -> Result<PathBuf, String> {
    let available = list_versions(registry_root, pkg);
    if available.is_empty() {
        return Err(format!(
            "依赖未找到：包 `{}` 不在 registry（{}/{}）。\
             请先在包目录执行 `lang-zone push` 发布，或检查 lz.toml 的 [dependencies]。",
            pkg,
            registry_root.display(),
            pkg
        ));
    }

    let chosen = if version_spec.is_empty() || version_spec == "*" {
        available[0].clone() // list_versions 已按版本降序
    } else if available.iter().any(|v| v == version_spec) {
        version_spec.to_string()
    } else {
        return Err(format!(
            "版本冲突：`{}` 依赖 `{}` v{}，但registry 中只有 {}{}。\
             可用版本：{}",
            "<当前包>",
            pkg,
            version_spec,
            pkg,
            if available.len() > 1 { "" } else { "" },
            available.join(", ")
        ));
    };

    let dir = registry_root.join(&pkg).join(&chosen);
    if !dir.is_dir() {
        return Err(format!(
            "registry 条目损坏：{} 存在但不是目录",
            dir.display()
        ));
    }
    Ok(dir)
}

/// 校验依赖声明：逐个确认 registry 中存在对应版本。
///
/// 返回 `(已解析的包名→目录)`，失败则返回诊断串（列出全部问题，一次报全，
/// 避免用户「改一个报一个」来回试）。
pub fn resolve_dependencies(
    registry_root: &Path,
    deps: &[(String, String)],
) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for (name, spec) in deps {
        match resolve_package(registry_root, name, spec) {
            Ok(dir) => out.push((name.clone(), dir)),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(format!(
            "依赖解析失败（{} 项）：\n  - {}",
            errors.len(),
            errors.join("\n  - ")
        ))
    }
}

/// 校验已发布包的 checksum.txt 与当前内容是否一致（防篡改 / 防陈旧）。
///
/// 仅在文件存在时校验——旧版本 push 未写 checksum 时跳过而非失败（向后兼容）。
pub fn verify_checksum(pkg_dir: &Path) -> Result<Option<bool>, String> {
    let cs_file = pkg_dir.join("checksum.txt");
    if !cs_file.is_file() {
        return Ok(None);
    }
    let recorded = fs::read_to_string(&cs_file)
        .map_err(|e| format!("读取 {}: {}", cs_file.display(), e))?
        .trim()
        .to_string();
    if recorded.is_empty() {
        return Ok(None);
    }
    let actual = compute_checksum(pkg_dir);
    Ok(Some(actual == recorded))
}

/// 与 cmd_push 使用的校验和算法保持一致：遍历源文件、拼接内容后哈希。
fn compute_checksum(pkg_dir: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    let mut files: Vec<PathBuf> = Vec::new();
    collect_source_files(pkg_dir, pkg_dir, &mut files);
    files.sort();
    for f in files {
        let rel = f.strip_prefix(pkg_dir).unwrap_or(&f);
        rel.to_string_lossy().hash(&mut hasher);
        if let Ok(content) = fs::read(&f) {
            content.hash(&mut hasher);
        }
    }
    format!("{:016x}", hasher.finish())
}

/// 收集包目录下参与校验的源文件（.lz 与 lz.toml），跳过构建产物。
fn collect_source_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        // 构建产物与 registry 自身不参与校验和
        if name == "build" || name == REGISTRY_DIR_NAME || name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            collect_source_files(root, &p, out);
        } else if p.extension().map(|x| x == "lz").unwrap_or(false)
            || name == "lz.toml"
        {
            out.push(p);
        }
    }
}