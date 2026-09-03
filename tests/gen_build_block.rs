// Lang-Zone 编译器 — tests/gen_build_block.rs
// 阶段3（FIST 任务 T4.5）缺口补齐回归：`func *:` 生成器构建块
//
// 背景：`*:` 构建块此前被 IR 层错误归类为普通生成器（__gen_vec 路径），
// 导致生成代码缺少 callee 调用且块返回类型错误（rustc E0308）。
// 修复：新增 ExprKind::GenBuild（node.rs），builder 生成 GenBuild 节点
// 并从函数符号表取返回类型，codegen 走 __bb 收集器 + map 调用 callee。
//
// 测试覆盖（仅走 IR 路线）：
// 1. callee 双参数包 → map 逐包调用
// 2. callee 单参数包
// 3. 条件 yield（*: 块内 if 分支）
// 4. 无 callee 仅收集包（迭代器语义）

use std::path::PathBuf;
use std::process::Command;

fn builtins_rlib() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return e.path();
            }
        }
    }
    panic!("lz_builtins rlib not found under target/debug");
}

fn run_lz(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lz_gen_build_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
    assert!(
        out.status.success(),
        "[{name}] lang-zone 编译失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rs = lz.with_extension("rs");
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    assert!(
        rc.status.success(),
        "[{name}] rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );

    let run = Command::new(&exe).output().expect("run compiled exe");
    assert!(
        run.status.success(),
        "[{name}] 程序运行失败: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

fn check_case(name: &str, source: &str, expected: &str) {
    let got = run_lz(name, source);
    assert_eq!(got, expected, "[{name}] 运行输出与 golden 不一致");
}

// 1. callee 双参数包：逐包调用 add，结果累加
#[test]
fn gen_build_callee_multi_pack() {
    check_case(
        "multi_pack",
        r#"
def add(a: int, b: int) -> int = a + b

def main() =
    xs = add *:
        yield (1, 2)
        yield (3, 4)
    total = 0
    for x in xs:
        total = total + x
    print(total)
"#,
        "10\n",
    );
}

// 2. callee 单参数包 + 直接打印结果列表
#[test]
fn gen_build_callee_single_pack() {
    check_case(
        "single_pack",
        r#"
def mul(a: int, b: int) -> int = a * b

def main() =
    xs = mul *:
        yield (2, 3)
    print(xs)
"#,
        "[6]\n",
    );
}

// 3. 条件 yield：*: 块内 if 分支按条件 yield
#[test]
fn gen_build_conditional_yield() {
    check_case(
        "conditional",
        r#"
def add(a: int, b: int) -> int = a + b

def main() =
    xs = add *:
        yield (1, 2)
        if 1 > 0:
            yield (10, 20)
    print(xs)
"#,
        "[3, 30]\n",
    );
}

// 4. 类型注解正确性：*: 结果应推断为 List<callee_ret>（int 列表），
//    可直接传给需要 List<int> 的函数（验证先前 List<(a,b)> 错误类型修复）
#[test]
fn gen_build_callee_type_annotation() {
    check_case(
        "type_annotation",
        r#"
def add(a: int, b: int) -> int = a + b

def sum_list(xs: List<int>) -> int:
    total = 0
    for x in xs:
        total = total + x
    return total

def main() =
    xs = add *:
        yield (1, 2)
        yield (3, 4)
    print(sum_list(xs))
"#,
        "10\n",
    );
}

// 5. 生成器构建块与 for 循环嵌套（块内迭代 yield）
#[test]
fn gen_build_combo_listcomp() {
    check_case(
        "combo_listcomp",
        r#"
def add(a: int, b: int) -> int = a + b

def main() =
    xs = add *:
        for i in [1, 2]:
            yield (i, i * 2)
    print(xs)
"#,
        "[3, 6]\n",
    );
}
