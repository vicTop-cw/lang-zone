// ERROR_BUG 负向回归测试：语义非法用例必须被编译器拒绝（EXIT != 0）。
// 2026-08-24 建立的 25 用例负向测试集；v179 扩展语义检查器达成 25/25 拦截。
// 本测试守护该防线：任何用例从"拒绝"回退为"放行"都会让回归变红。

use std::path::{Path, PathBuf};
use std::process::Command;

fn collect_lz(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_lz(&p, out);
            } else if p.extension().map(|x| x == "lz").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

#[test]
fn error_bug_all_rejected() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("ERROR_BUG");
    let mut cases: Vec<PathBuf> = Vec::new();
    collect_lz(&root, &mut cases);
    cases.sort();
    assert!(
        cases.len() >= 25,
        "ERROR_BUG 用例数量异常：仅发现 {} 个 .lz（预期 ≥25）",
        cases.len()
    );

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let mut leaked: Vec<String> = Vec::new();
    for lz in &cases {
        let out = Command::new(&bin).arg(lz).output()
            .map_err(|e| format!("spawn err: {}", e))
            .expect("spawn lang-zone");
        if out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            leaked.push(format!(
                "\n[漏报] {}\n  （编译器 EXIT 0，未拒绝非法代码）",
                lz.display()
            ));
            let _ = stderr; // 放行时 stderr 通常为空
        }
    }
    assert!(
        leaked.is_empty(),
        "负向防线回退：以下 {} 个非法用例被编译器放行：{}",
        leaked.len(),
        leaked.join("")
    );
}
