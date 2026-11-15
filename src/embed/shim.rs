// Lang-Zone 编译器 — embed/shim.rs
// 把 EmbedTarget 翻成 bridge::cli 的端点描述，再让 cli.rs 生成子进程代码。
//
// 为什么请求走 argv 而不是 stdin：
//   `CliEndpoint` 那套是 Level 3 的 stdin 管道版（`METHOD k=v` 一行），它要求对端实现
//   行读取 + 超时轮询；而"摘出来的外语言程序"绝大多数只有 `sys.argv`，且 stdin 版把
//   含空格的字符串参数直接打散。argv 逐段传递天然免疫空格，且不需要对端有交互能力。
//   ⇒ 请求 = `<method> key=value ...`（argv），响应 = stdout 最后一行 `OK:<值>` / `ERR:<原因>`。
//   这是「值只走最容易的路」在本方案里的具体落点，不是对 LineProtocol 的偏离后补的借口。
//
// 失败约定：垫片出错一律 panic 并带上 `产物名 + 原因`。
//   理由：返回默认值会把"外语言没跑起来"伪装成一个合法的 0/空串 —— 正是本项目历来在清的那类静默错。

use crate::bridge::cli::{EmbedEndpoint, EmbedFnSig};

use super::{ArtifactKind, EmbedTarget};

/// lz 基础类型 → Rust 形参类型（垫片签名里用）
fn rust_param_ty(lz: &str) -> &'static str {
    match lz {
        "int" => "i64",
        "float" => "f64",
        "bool" => "bool",
        _ => "&str",
    }
}

/// lz 基础类型 → Rust 返回类型
fn rust_ret_ty(lz: Option<&str>) -> &'static str {
    match lz {
        Some("int") => "i64",
        Some("float") => "f64",
        Some("bool") => "bool",
        Some("str") => "String",
        _ => "String",
    }
}

/// 把一个 Cli 档产物描述成一个 CLI 端点（可调用面 = callable 的那些函数）
pub fn endpoint(t: &EmbedTarget) -> EmbedEndpoint {
    let fns: Vec<EmbedFnSig> = t
        .callable_methods()
        .iter()
        .map(|m| EmbedFnSig {
            wrapper: format!("{}_{}", t.artifact, m.name),
            method: m.name.clone(),
            params: m
                .params
                .iter()
                .map(|p| (p.name.clone(), p.lz_type.clone()))
                .collect(),
            ret: m.ret.clone().unwrap_or_else(|| "unit".to_string()),
        })
        .collect();
    EmbedEndpoint {
        artifact: t.artifact.clone(),
        dir_rel: t.dir_rel.clone(),
        program: t.run.program.clone(),
        prefix: t.run.args.clone(),
        kind: match t.kind {
            ArtifactKind::Executable => "executable",
            ArtifactKind::Library => "library",
        },
        fns,
    }
}

pub fn module_text(targets: &[EmbedTarget]) -> String {
    let eps: Vec<EmbedEndpoint> = targets.iter().map(endpoint).collect();
    crate::bridge::cli::generate_embed_module(&eps)
}

/// lz 侧 `import <artifact>` 应展开成的 use 行
pub fn use_line() -> String {
    "use __lz_embed::*;".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(lang: &str, stem: &str, src: &str) -> EmbedTarget {
        let mut x = crate::embed::resolve(lang, stem, src).unwrap();
        // 与 codegen 同一把尺：目录用的是去后缀的模块主干，不是原始 file_path
        x.dir_rel = super::super::artifact::dir_rel(
            crate::embed::base_stem(stem).as_str(),
            x.artifact.as_str(),
        );
        x
    }

    #[test]
    fn library_endpoint_exposes_typed_wrappers() {
        let e = endpoint(&t(
            "python",
            "calc.lz",
            "def add(a: int, b: int) -> int:\n    return a + b\n",
        ));
        assert_eq!(e.artifact, "python_calc");
        assert_eq!(e.kind, "library");
        assert_eq!(e.fns.len(), 1);
        assert_eq!(e.fns[0].wrapper, "python_calc_add");
        assert_eq!(e.fns[0].method, "add");
        assert_eq!(e.fns[0].ret, "int");
    }

    #[test]
    fn uncallable_signatures_produce_no_wrapper() {
        let e = endpoint(&t(
            "python",
            "calc.lz",
            "def f(x: SomeObject) -> int:\n    return 1\n",
        ));
        assert!(e.fns.is_empty(), "非闭集签名不许出垫片：{:?}", e.fns);
    }

    #[test]
    fn executable_produces_no_library_wrappers() {
        let e = endpoint(&t(
            "python",
            "runme.lz",
            "if __name__ == \"__main__\":\n    print(1)\n",
        ));
        assert_eq!(e.kind, "executable");
        assert!(e.fns.is_empty());
    }

    #[test]
    fn generated_module_has_no_absolute_paths_and_is_deterministic() {
        let targets = vec![t(
            "python",
            "calc.lz",
            "def add(a: int, b: int) -> int:\n    return a + b\n",
        )];
        let a = module_text(&targets);
        let b = module_text(&targets);
        assert_eq!(a, b, "垫片文本必须逐字可复现");
        assert!(a.contains("pub fn python_calc_add"), "{}", a);
        assert!(a.contains("build/embed/calc/python_calc"), "{}", a);
        assert!(!a.contains("E:\\"), "垫片不许焊死本机绝对路径");
        assert!(!a.contains("mod __lz_embed"), "模块外壳由调用方包，这里只出内容");
    }
}
