// Lang-Zone 编译器 — tests/native_variant_qual.rs
// T0r11 / N1 回归闸门：自定义枚举变体构造器在「实参是函数调用」时丢失枚举限定。
//
// 缺陷形态（修复前实测，DEMO/lz_std/result.lz or_else）：
//   `enum Result<T, E>: Ok(T) Err(E)` 与
//   `a.or_else(|e: str| -> Result<int, str> = Ok(len(e)))`
//   → 发射裸 `Ok((e.len() as i64))`，Rust 解析到 std::result::Result::Ok → E0308。
//   判别实验：`Ok(5)` / `Ok(x)` / `Ok(x+1)` 正确发 `Result::Ok(..)`，
//   仅 `Ok(len(e))` / `Ok(inc(e))`（实参为 Call）丢限定。
//
// 根因：闭包体重生成路径（src/ir/codegen/expr_gen.rs，Lambda 分支的
//   `body_refs_noncopy_external` → child CodeGen 重生成 body）没有继承父
//   CodeGen 的 enum_variants 表。实参是字面量/变量/二元式时该判定为 false，
//   body 沿用已限定（闭包返回注解后处理）的 body_s；实参是 Call 时判定递归到
//   callee Var（`len`/`inc` 非 Copy 且非形参）→ true → 走 child 重生成 → 裸 `Ok(`。
//
// 运行：CARGO_TARGET_DIR=target/lane-native cargo test --test native_variant_qual
//
// T0r14（同源的另一半）：用户自定义枚举变体名以 `Var` 出现在**闭包体内** Call 的
//   callee 位时，clone 路径只认 Some/None/Ok/Err/集合别名（`is_type_ctor_name`），
//   把 `Bar(inc(n))` 整形为 `Foo::Bar::clone()(inc(n))` → rustc
//   `error[E0433]: Bar is a variant, not a module`。修法：把变体名表传进 helpers 的
//   四个自由函数（`is_ctor_name`），变体构造器不再进 clone 路径。回归闸门见 ④。

use std::path::PathBuf;
use std::process::Command;

/// lane 感知：以本次测试实际构建出的 lzc 可执行文件所在目录为 profile 目录
/// （CARGO_TARGET_DIR=target/lane-xxx 时即 target/lane-xxx/debug）。
fn profile_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
        .parent()
        .expect("lzc exe has parent dir")
        .to_path_buf()
}

fn builtins_rlib() -> PathBuf {
    let dir = profile_dir();
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
    panic!(
        "lz_builtins rlib not found under {}（先 `cargo build -p lz_builtins`）",
        dir.display()
    );
}

fn dep_dir(rlib: &PathBuf) -> PathBuf {
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

struct Artifact {
    ok: bool,
    rs: Option<String>,
    diag: String,
}

fn run_lz(tag: &str, src: &str) -> Artifact {
    let work = std::env::temp_dir().join(format!("lz_nvq_{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("input.lz");
    std::fs::write(&lz, src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lzc");
    let rs = lz.with_extension("rs");
    if !out.status.success() || !rs.exists() {
        return Artifact {
            ok: false,
            rs: None,
            diag: format!(
                "转译失败:\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        };
    }
    let rs_text = std::fs::read_to_string(&rs).expect("read generated rs");

    let rlib = builtins_rlib();
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", dep_dir(&rlib).display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    if !rc.status.success() {
        return Artifact {
            ok: false,
            rs: Some(rs_text),
            diag: format!(
                "产物 rustc 编译失败:\n{}\n--- 产物 ---\n{}",
                String::from_utf8_lossy(&rc.stderr),
                std::fs::read_to_string(&rs).unwrap_or_default()
            ),
        };
    }
    let prog = Command::new(&exe).output().expect("run exe");
    Artifact {
        ok: prog.status.success(),
        rs: Some(rs_text),
        diag: format!(
            "产物运行失败:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&prog.stdout),
            String::from_utf8_lossy(&prog.stderr)
        ),
    }
}

/// 返回产物中包含 `needle` 的行（用于逐行核对限定形态）。
fn lines_with<'a>(rs: &'a str, needle: &str) -> Vec<&'a str> {
    rs.lines().filter(|l| l.contains(needle)).collect()
}

/// 断言：该行里的每个 `Ok(`/`Err(` 都带 `Result::` 前缀
/// （先摘掉已限定的 `Result::Ok(`，再要求不残留裸 `Ok(`）。
fn assert_variant_qualified(line: &str) {
    let stripped = line.replace("Result::Ok(", "<Q_OK>(");
    assert!(
        !stripped.contains("Ok("),
        "变体构造丢了枚举限定（裸 Ok 会解析到 std::result::Result::Ok）：\n{}",
        line
    );
}

// ────────────────────────────────────────────────────────────────
// ① 语句位置四种形态：Ok(5) / Ok(x) / Ok(x+1) / Ok(len(e))
//    前三者是「不应改变」的基线，第四者是 N1 的判别形态。
// ────────────────────────────────────────────────────────────────
#[test]
fn variant_qualification_statement_position() {
    let src = r#"
enum Result<T, E>:
    Ok(T)
    Err(E)

def len(s: str) -> int =
    return s.length()

def inc(x: int) -> int =
    return x + 1

def main() =
    let x: int = 7
    let e: str = "abcd"
    let ok_lit: Result<int, str> = Ok(5)
    let ok_var: Result<int, str> = Ok(x)
    let ok_bin: Result<int, str> = Ok(x + 1)
    let ok_call: Result<int, str> = Ok(len(e))
    let ok_call2: Result<int, str> = Ok(inc(x))
    print(x + 1)
"#;
    let a = run_lz("stmt", src);
    assert!(a.ok, "{}", a.diag);
    let rs = a.rs.expect("rs text");
    for tag in ["ok_lit:", "ok_var:", "ok_bin:", "ok_call:", "ok_call2:"] {
        let ls = lines_with(&rs, tag);
        assert_eq!(ls.len(), 1, "产物中应恰有一行含 `{}`:\n{}", tag, rs);
        assert!(
            ls[0].contains("Result::Ok("),
            "`{}` 应发射 `Result::Ok(..)`：\n{}",
            tag,
            ls[0]
        );
        assert_variant_qualified(ls[0]);
    }
}

// ────────────────────────────────────────────────────────────────
// ② N1 原始复现形态：带返回类型注解的闭包体内四种形态
//    （child CodeGen 重生成 body 的路径只有这里才会命中）
// ────────────────────────────────────────────────────────────────
#[test]
fn variant_qualification_in_typed_lambda_body() {
    let src = r#"
enum Result<T, E>:
    Ok(T)
    Err(E)

def len(s: str) -> int =
    return s.length()

def inc(x: int) -> int =
    return x + 1

def main() =
    let l_lit: fn(int) -> Result<int, str> = |k: int| -> Result<int, str> = Ok(5)
    let l_var: fn(int) -> Result<int, str> = |k: int| -> Result<int, str> = Ok(k)
    let l_bin: fn(int) -> Result<int, str> = |k: int| -> Result<int, str> = Ok(k + 1)
    let l_call: fn(str) -> Result<int, str> = |e: str| -> Result<int, str> = Ok(len(e))
    let l_call2: fn(int) -> Result<int, str> = |n: int| -> Result<int, str> = Ok(inc(n))
    print(1)
"#;
    let a = run_lz("lambda", src);
    assert!(
        a.ok,
        "N1 复现形态（实参为函数调用的闭包体变体构造）必须可编译:\n{}",
        a.diag
    );
    let rs = a.rs.expect("rs text");
    for tag in ["l_lit:", "l_var:", "l_bin:", "l_call:", "l_call2:"] {
        let ls = lines_with(&rs, tag);
        assert_eq!(ls.len(), 1, "产物中应恰有一行含 `{}`:\n{}", tag, rs);
        assert!(
            ls[0].contains("Result::Ok("),
            "`{}` 闭包体内应发射 `Result::Ok(..)`：\n{}",
            tag,
            ls[0]
        );
        assert_variant_qualified(ls[0]);
    }
    // 判别点：实参为函数调用的两条必须带限定（修复前是裸 `Ok(`）
    for tag in ["l_call:", "l_call2:"] {
        let ls = lines_with(&rs, tag);
        assert!(
            ls[0].contains("Result::Ok("),
            "N1：实参为 Call 时丢限定，`{}` 发成裸 `Ok(..)`:\n{}",
            tag,
            ls[0]
        );
    }
}

// ────────────────────────────────────────────────────────────────
// ②b N1 逐字复现形态：闭包作为**方法实参**（不是 let 绑定的值位置）。
//     这一形态走的是另一条 child 重生成分支（!uses_move，box_lambda=false），
//     与 ② 的 `let l: fn(...) = |..|`（box_lambda 强制 move）不同分支，
//     因此两点都必须有闸门 —— DEMO/lz_std/result.lz:315 的
//     `a.or_else(|e: str| -> Result<int, str> = Ok(len(e)))` 正是本形态。
// ────────────────────────────────────────────────────────────────
#[test]
fn variant_qualification_in_lambda_argument() {
    let src = r#"
enum Result<T, E>:
    Ok(T)
    Err(E)

def len(s: str) -> int =
    return s.length()

def inc(x: int) -> int =
    return x + 1

impl<T, E> Result<T, E> =
    def or_else<F>(self, f: fn(E) -> Result<T, F>) -> Result<T, F> =
        match self:
            case Ok(value):
                return Ok(value)
            case Err(error):
                return f(error)

def main() =
    let a: Result<int, str> = Ok(3)
    let c: Result<int, str> = a.or_else(|e: str| -> Result<int, str> = Ok(len(e)))
    let d: Result<int, str> = a.or_else(|e: str| -> Result<int, str> = Ok(inc(3)))
    let f: Result<int, str> = a.or_else(|e: str| -> Result<int, str> = Ok(5))
    print(1)
"#;
    let a = run_lz("lambda_arg", src);
    assert!(
        a.ok,
        "方法实参位置的闭包体变体构造必须可编译（DEMO result.lz or_else 形态）:\n{}",
        a.diag
    );
    let rs = a.rs.expect("rs text");
    for tag in ["let c:", "let d:", "let f:"] {
        let ls = lines_with(&rs, tag);
        assert_eq!(ls.len(), 1, "产物中应恰有一行含 `{}`:\n{}", tag, rs);
        assert!(
            ls[0].contains("Result::Ok("),
            "N1：`{}` 的闭包体应发射 `Result::Ok(..)`（实参为函数调用时曾丢限定）：\n{}",
            tag,
            ls[0]
        );
        assert_variant_qualified(ls[0]);
    }
}

// ────────────────────────────────────────────────────────────────
// ③ 自定义枚举正例：`enum Foo: Bar(int)` → `Foo::Bar(..)`
//    （不依赖 Result 名字，验证限定来自 enum_variants 表本身）
//
// 说明：本用例只覆盖**语句位置**形态。`Bar(..)` 出现在闭包体内时曾另有一个
// 与 N1 同源的既有缺陷（clone_noncopy_external 只把 Some/None/Ok/Err/集合别名
// 认作类型构造器，用户自定义变体名 `Bar` 被当成外部变量 → `Foo::Bar::clone()(..)`
// → E0433），已在 T0r14 修复（枚举变体表传入 helpers），
// 该形态的回归闸门见下方 ④ `custom_enum_variant_in_closure_body`。
// ────────────────────────────────────────────────────────────────
#[test]
fn custom_enum_variant_qualified() {
    let src = r#"
enum Foo:
    Bar(int)
    Baz

def inc(x: int) -> int =
    return x + 1

def main() =
    let a: Foo = Bar(5)
    let b: Foo = Bar(inc(3))
    let c: Foo = Baz
    print(1)
"#;
    let a = run_lz("foo", src);
    assert!(a.ok, "自定义枚举变体构造产物必须可编译:\n{}", a.diag);
    let rs = a.rs.expect("rs text");
    // 未使用变量的类型标注会被产物省略（`let a = Foo::Bar(5i64);`），故按变体形态筛选
    let bars = lines_with(&rs, "Foo::Bar(");
    assert_eq!(
        bars.len(),
        2,
        "`Bar(5)` 与 `Bar(inc(3))` 都应发射 `Foo::Bar(..)`:\n{}",
        rs
    );
    for line in bars {
        assert!(
            !line.replace("Foo::Bar(", "<Q>(").contains("Bar("),
            "不得发射裸 `Bar(..)`（会解析到同名自由函数/报 E0425）：\n{}",
            line
        );
        assert!(
            !line.contains("Foo::Bar::clone()"),
            "变体构造器名不得被当作变量 clone（clone_noncopy_external 边界）：\n{}",
            line
        );
    }
    // 无数据变体（`Baz`）不应发射调用括号（Rust 单元变体不是函数，E0618）
    let uc = lines_with(&rs, "Foo::Baz");
    assert_eq!(uc.len(), 1, "产物中应恰有一行含 `Foo::Baz`:\n{}", rs);
    assert!(
        !uc[0].contains("Foo::Baz()"),
        "单元变体应发射 `Foo::Baz`（不带括号）：\n{}",
        uc[0]
    );
}

// ────────────────────────────────────────────────────────────────
// ④ N1 修复前的失败形态（T0r14 归零验证）：闭包体内自定义变体构造器
//
// 修复前实测（T0r14 复现样本）：
//   `let mk = |n: int| -> Foo = Bar(inc(n))`
//   → `Arc::new(move |n: i64| -> Foo { Foo::Bar::clone()(inc.clone()(n)) })`
//   → rustc `error[E0433]: cannot find module `Bar` in `Foo`` / `Bar` is a variant,
//     not a module。
//
// 根因（与 N1 同源的另一半）：闭包体内的 clone 路径
//   （helpers.rs `clone_noncopy_external` / `body_refs_noncopy_external`）只把
//   Some/None/Ok/Err/集合别名认作类型构造器（`is_type_ctor_name`），用户枚举变体名
//   `Bar` 被当成「外层非 Copy 变量」→ 包一层 `.clone()`。这两个 helper 是自由函数，
//   拿不到 CodeGen 的 enum_variants 表。
//
// 修法：给四个 helper（body_refs_noncopy_external / clone_noncopy_external /
//   clone_noncopy_stmt / *_contains_nested_move_lambda）加 `variants: &HashMap<..>`
//   参数，判定改走 `is_ctor_name`（内建列表 ∪ 用户变体表）。变体名从此不进 clone 路径。
//
// 本用例覆盖三种闭包体形态：
//   a. 单表达式体 + 变体实参是函数调用（原始复现形态，走 clone_noncopy_external 的 Call 递归）
//   b. 块体 + 局部变量（走 clone_noncopy_stmt）
//   c. 单元变体裸值（`|n| -> Foo = Baz`）
// 运行时用 `assert unwrap(..) == ..` 校验取值，程序 panic → 非 0 退出 → `a.ok` 为 false。
// ────────────────────────────────────────────────────────────────
#[test]
fn custom_enum_variant_in_closure_body() {
    let src = r#"
enum Foo:
    Bar(int)
    Baz

def inc(x: int) -> int =
    return x + 1

def unwrap(f: Foo) -> int =
    match f:
        case Foo.Bar(v): return v
        case Foo.Baz: return 0

def main() =
    let base: int = 4
    let mk = |n: int| -> Foo = Bar(inc(n))
    let mk2 = |n: int| -> Foo =
        let v: int = inc(n)
        Bar(v)
    let mkz = |n: int| -> Foo = Baz
    let a: Foo = mk(base)
    let b: Foo = mk2(base)
    let c: Foo = mkz(1)
    assert unwrap(a) == 5
    assert unwrap(b) == 5
    assert unwrap(c) == 0
    print(1)
"#;
    let a = run_lz("foo_closure", src);
    assert!(
        a.ok,
        "闭包体内的自定义变体构造必须可编译且取值正确（T0r14 原始复现形态）:\n{}",
        a.diag
    );
    let rs = a.rs.expect("rs text");
    // 核心回归：闭包体内变体构造器不得被 clone 路径整形（`Foo::Bar::clone()` / `<..>::clone()`）
    assert!(
        !rs.contains("Bar::clone()"),
        "变体构造器名不得被 clone（E0433: Bar is a variant, not a module）:\n{}",
        rs
    );
    assert!(
        !rs.contains("Foo::Baz::clone()"),
        "单元变体不得发射 `Foo::Baz::clone()`（`::` 路径非法）:\n{}",
        rs
    );
    // 单表达式体（原始复现形态）：该行必须直接以 `Foo::Bar(..)` 构造
    let mk_line = rs
        .lines()
        .find(|l| l.contains("let mut mk:"))
        .expect("产物中应有 `let mut mk:` 行");
    assert!(
        mk_line.contains("Foo::Bar("),
        "闭包体内 `Bar(inc(n))` 应发射 `Foo::Bar(..)`:\n{}",
        mk_line
    );
    assert!(
        !mk_line.contains("Foo::Bar::clone"),
        "闭包体内变体构造器不得进 clone 路径:\n{}",
        mk_line
    );
    // 块体形态：`Bar(v)` 同样限定为 `Foo::Bar(v)`
    assert!(
        rs.lines().any(|l| l.trim_start().starts_with("Foo::Bar(v)")),
        "块体内 `Bar(v)` 应发射 `Foo::Bar(v)`:\n{}",
        rs
    );
}

