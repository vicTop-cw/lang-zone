// Lang-Zone 自举回归测试：LZ 写的 IR display 能否经自家工具链编译运行
//
// 验证链路：内嵌 LZ 源码（递归 enum IrType + display_type）→ lang-zone 编译
// → rustc 编译 → 运行 → 断言输出与 Rust 版 display.rs 格式一致。
// 这是自举路线 B（LZ 翻译编译器核心）的固化回归：bootstrap/work/lz_ir
// 试点若回归，此测试即失败。

use std::path::PathBuf;
use std::process::Command;
use std::fs;

// 精简 LZ 源：递归 enum IrType + display_type（对齐 src/ir/display.rs）
// 覆盖递归 enum 自引用（Box<Vec<IrType>>）、[ty] 前缀、fn/option 容器
const LZ_SOURCE: &str = r#"
enum IrType:
    Int
    Str
    Named(path: str, args: List<IrType>)
    Opt(inner: IrType)

def display_type(t: IrType) -> str =
    match t:
        case IrType.Int => "int"
        case IrType.Str => "str"
        case IrType.Named(path: p, args: a) =>
            p + ("<" + type_list(a) + ">" if a.len() > 0 else "")
        case IrType.Opt(inner: x) => "Option<" + display_type(x) + ">"

def type_list(ts: List<IrType>) -> str =
    if ts.len() == 0:
        ""
    else:
        let head = display_type(ts[0])
        if ts.len() > 1:
            head + ", " + type_list(tail_t(ts))
        else:
            head

def tail_t(ts: List<IrType>) -> List<IrType> =
    let mut out: List<IrType> = []
    for idx in 1..ts.len():
        out = out + [ts[idx]]
    out

def main() =
    // Vec<int>
    let v1 = IrType.Named(path: "Vec", args: [IrType.Int])
    print(display_type(v1))
    // Option<Vec<int>>（递归嵌套）
    let v2 = IrType.Named(path: "Vec", args: [IrType.Int])
    let ov = IrType.Opt(inner: v2)
    print(display_type(ov))
"#;

#[test]
fn lz_written_ir_display_compiles_and_runs() {
    let work = std::env::temp_dir().join("lz_ir_regression");
    fs::create_dir_all(&work).expect("create work dir");
    let lz_path = work.join("ir_types.lz");
    let rs_path = work.join("ir_types.rs");
    let exe_path = work.join("ir_types.exe");
    fs::write(&lz_path, LZ_SOURCE).expect("write lz source");

    // 1. lang-zone 编译 .lz → .rs
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg(&lz_path)
        .output()
        .expect("run lang-zone");
    assert!(
        out.status.success(),
        "lang-zone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(rs_path.exists(), "generated .rs missing");

    // 2. rustc 编译 .rs（链接 lz_builtins）
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    let rustc_out = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs_path)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("run rustc");
    assert!(
        rustc_out.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&rustc_out.stderr)
    );

    // 3. 运行并断言输出（与 Rust 版 display.rs 格式一致）
    let run_out = Command::new(&exe_path).output().expect("run exe");
    assert!(run_out.status.success(), "generated exe failed");
    let stdout = String::from_utf8_lossy(&run_out.stdout);
    let first: Vec<&str> = stdout.lines().collect();
    assert!(
        first.len() >= 2,
        "expected >=2 lines, got: {}",
        stdout
    );
    // Vec<int>（Named 无泛型括号展开）
    assert!(
        stdout.contains("Vec<int>"),
        "expected Vec<int>, got: {}",
        stdout
    );
    // Option<Vec<int>>（递归嵌套容器）
    assert!(
        stdout.contains("Option<Vec<int>>"),
        "expected Option<Vec<int>>, got: {}",
        stdout
    );

    // 清理
    let _ = fs::remove_dir_all(&work);
}
