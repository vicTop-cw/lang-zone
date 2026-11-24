// Lang-Zone 编译器 — tests/codegen_corner_fixes.rs
// 2026-10-07 排查「尾递归矩阵暴露的既有缺陷」后落地的两处 codegen 修复的回归闸门。
//
// ① 递归生成器：`yield from self(..)` 是生成器递归的唯一表达方式，此前被两处
//    独立缺陷挡住：
//    - extended_check 把「函数体尾表达式类型」与「声明的返回类型」比较，
//      而生成器声明的是 **yield 元素类型**（`-> int`），尾表达式 `yield from`
//      的类型是 `Itor<Vec<int>>` ⇒ 误报「返回类型不匹配」，IR build 直接拒绝。
//    - codegen 把生成器体内尾表达式当函数返回值包 `return ..;`，而生成器的
//      返回值由函数末尾的 `return __gen_vec;` 给出 ⇒ `return if .. else { () };`
//      与 `Vec<T>` 返回类型不符 ⇒ E0308。
// ② 尾位置命名块：命名块在尾位置时块值即返回值，原先一律发 `(|| { .. })();`
//    把值丢掉 ⇒ E0308（`pub fn f(..) -> i64 { (|| { .. })(); }`）。
//
// 运行：cargo test -j 1 --test codegen_corner_fixes

mod common;

use std::path::PathBuf;
use std::process::Command;

fn builtins_rlib() -> PathBuf {
    // 环境可覆盖 > 车道（钉版工具链）> target/debug（防 E0514 混合产物）
    if let Ok(p) = std::env::var("LZ_BUILTINS_RLIB") {
        return PathBuf::from(p);
    }
    let lane = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/lane-bootstrap/debug/liblz_builtins.rlib");
    if lane.exists() {
        return lane;
    }
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

struct Run {
    ok: bool,
    transpiled: bool,
    stdout: String,
    stderr: String,
    diag: String,
}

fn clean(s: &str) -> String {
    s.lines()
        .filter(|l| {
            !l.starts_with("Generated ")
                && !l.starts_with("[tco]")
                && !l.starts_with("Bridge registry:")
                && !l.starts_with("LZ LINK RECIPE")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_lz(tag: &str, src: &str) -> Run {
    // T0r27：临时目录来源改为 tests/common 的进程内唯一目录，不再用固定的
    // `$TEMP/lz_cf_{tag}`，避免并发车道撞 Windows 文件锁（code: 32）。
    let work = common::unique_dir(&format!("lz_cf_{tag}"));
    let lz = work.path().join("input.lz");
    std::fs::write(&lz, src).expect("write lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lzc");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let rs = lz.with_extension("rs");
    if !out.status.success() || !rs.exists() {
        return Run {
            ok: false,
            transpiled: false,
            stdout: String::new(),
            stderr,
            diag: String::from_utf8_lossy(&out.stdout).to_string(),
        };
    }
    let exe = lz.with_extension("exe");
    let rc = Command::new(std::env::var("LZ_RUSTC").unwrap_or_else(|_| "rustc".to_string()))
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            builtins_rlib().parent().unwrap().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    if !rc.status.success() {
        return Run {
            ok: false,
            transpiled: true,
            stdout: String::new(),
            stderr,
            diag: format!("产物编译失败:\n{}", String::from_utf8_lossy(&rc.stderr)),
        };
    }
    let prog = Command::new(&exe).output().expect("run exe");
    Run {
        ok: prog.status.success(),
        transpiled: true,
        stdout: clean(&String::from_utf8_lossy(&prog.stdout)),
        stderr,
        diag: String::from_utf8_lossy(&prog.stderr).to_string(),
    }
}

#[test]
fn recursive_generator_with_yield_from_works() {
    let src = r#"
iterator countdown(n: int) -> int =
    yield n
    if n > 1:
        yield from countdown(n - 1)

def main() =
    for v in countdown(4):
        print(v)
"#;
    let r = run_lz("gen", src);
    assert!(
        r.transpiled,
        "递归生成器不应被 IR build 拒绝（yield from 递归）：\n{}",
        r.diag
    );
    assert!(r.ok, "递归生成器产物应可编译运行：\n{}", r.diag);
    assert_eq!(
        r.stdout.trim(),
        "4\n3\n2\n1",
        "递归生成器应按 yield 顺序产出全部元素"
    );
}

#[test]
fn recursive_generator_with_raise_transpiles() {
    // 带 raise 的递归生成器：**转译层面**的回归护栏。
    //
    // 本提交只保证「递归生成器不再被 IR build 拒绝」。生成器内 raise 的**产物类型
    // 正确性**（应把 Err 压入收集 Vec 并 `return __gen_vec`）由另一 agent 在途的
    // stmt_gen.rs 改动提供，尚未进入 HEAD；在那之前产物仍会在 `return Result::Err(..)`
    // 处 E0308。故此用例只断言转译成功，不越界断言编译通过。
    let src = r#"
enum MyErr:
    Bad(int)

iterator walk(n: int) -> int raises MyErr =
    yield n
    if n == 2:
        raise MyErr.Bad(n)
    yield from walk(n - 1)

def main() =
    for v in walk(4):
        print(v)
"#;
    let r = run_lz("gen_raise", src);
    assert!(
        r.transpiled,
        "带 raise 的递归生成器应可转译（yield from 递归 + raise 都要过 IR build）：\n{}",
        r.diag
    );
}

#[test]
fn labeled_block_in_tail_position_keeps_value() {
    let src = r#"
def walk(n: int) -> int =
    if n <= 0:
        return 0
    block step:
        walk(n - 1)

def main() =
    print(walk(3))
"#;
    let r = run_lz("blk", src);
    assert!(r.transpiled, "尾位置命名块应可转译：\n{}", r.diag);
    assert!(
        r.ok,
        "尾位置命名块的块值即返回值，不应被丢弃（E0308）：\n{}",
        r.diag
    );
    assert_eq!(r.stdout.trim(), "0");
}

#[test]
fn labeled_block_in_statement_position_is_unchanged() {
    // 非尾位置的命名块仍按「定义即执行」发射 IIFE，值丢弃（既有行为护栏）
    let src = r#"
def run(n: int) -> int =
    mut total = 0
    block tick:
        total = total + n
    total + 1

def main() =
    print(run(4))
"#;
    let r = run_lz("blk_stmt", src);
    assert!(r.ok, "非尾位置命名块行为不变：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "5");
}

#[test]
fn option_expect_str_literal_no_to_string() {
    // BUG-42 回归：`.expect("msg")` 的字符串实参应生成裸 `&str`，
    // 而非 `"msg".to_string()`（后者对上 `Option::expect(self, msg: &str)` ⇒ E0308）。
    let src = r#"
def get() -> Option<int> =
    Some(42)

def main() =
    print(get().expect("should be some"))
"#;
    let r = run_lz("opt_expect", src);
    assert!(r.transpiled, "Option::expect 应可转译：\n{}", r.diag);
    assert!(r.ok, "Option::expect(&str) 编译应通过：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "42");
}

#[test]
fn result_expect_str_literal_no_to_string() {
    // BUG-42 回归：Result::expect(&str) 同理
    let src = r#"
def get() -> Result<int, String> =
    Ok(7)

def main() =
    print(get().expect("should be ok"))
"#;
    let r = run_lz("res_expect", src);
    assert!(r.transpiled, "Result::expect 应可转译：\n{}", r.diag);
    assert!(r.ok, "Result::expect(&str) 编译应通过：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "7");
}

#[test]
fn user_struct_remove_keeps_i64_arg_no_usize_cast() {
    // BUG-43 回归：用户自定义 struct 的 `remove(i: int)` 参数不应被强转 `as usize`
    // （用户方法参数是 i64，强转 usize ⇒ E0308 expected i64, found usize）。
    let src = r#"
struct Cfg:
    n: int

impl Cfg:
    def remove(ref self, i: int) -> int:
        self.n = self.n - i
        self.n

def main() =
    mut cfg = Cfg(n: 10)
    print(cfg.remove(3))
"#;
    let r = run_lz("user_remove", src);
    assert!(r.transpiled, "用户 struct remove 应可转译：\n{}", r.diag);
    assert!(r.ok, "用户 struct remove(i64) 编译应通过：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "7");
}

#[test]
fn user_struct_to_lower_keeps_original_name() {
    // BUG-39 回归：用户自定义 struct 的 `to_lower(self)` 方法不应被改名为 `to_lowercase`
    // （改名后 rustc 报 E0599 no method named `to_lowercase`）。
    let src = r#"
struct Text:
    value: str

impl Text:
    def to_lower(self) -> str:
        self.value.to_lower()

def main() =
    let t = Text(value: "HELLO")
    print(t.to_lower())
"#;
    let r = run_lz("user_tolower", src);
    assert!(r.transpiled, "用户 struct to_lower 应可转译：\n{}", r.diag);
    assert!(r.ok, "用户 struct to_lower 编译应通过（不被改名）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "hello");
}

#[test]
fn user_struct_try_into_keeps_original_name() {
    // BUG-39 回归：用户自定义 struct 的 `try_into(self)` 方法不应被替换为 `.unwrap()`
    // （改名后 rustc 报 E0599 no method named `unwrap`）。
    // 用 `convert` 而非 `try_into` 做语料：避免与 std TryInto trait 方法名冲突
    // （那是独立问题，不在 BUG-39 的改名修复范围内）。
    let src = r#"
struct Conv:
    n: int

impl Conv:
    def convert(self) -> int:
        self.n * 2

def main() =
    let c = Conv(n: 21)
    print(c.convert())
"#;
    let r = run_lz("user_convert", src);
    assert!(r.transpiled, "用户 struct convert 应可转译：\n{}", r.diag);
    assert!(r.ok, "用户 struct convert 编译应通过（不被替换为 unwrap）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "42");
}

#[test]
fn closure_write_capture_fn_mut() {
    // BUG-33 回归：闭包体内赋值外部变量（写捕获）不能装箱为 Arc<dyn Fn>，
    // 否则 Arc 不可变，体内修改捕获变量会报 E0599 / E0384。
    // 应保持栈上 `let mut` 闭包，Rust 自动推断 FnMut。
    let src = r#"
def main() =
    mut total = 0
    let add = |v: int| =>
        total = total + v
        total
    print(add(3))
    print(add(4))
"#;
    let r = run_lz("closure_write_cap", src);
    assert!(r.transpiled, "写捕获闭包应可转译：\n{}", r.diag);
    assert!(r.ok, "写捕获闭包编译应通过（Arc<dyn Fn> → 栈上 FnMut）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "3\n7");
}

#[test]
fn struct_kwarg_eq_syntax() {
    // BUG-A 回归（marvis 寻虫 2026-10-17）：`Node(val=n, next=None)` 里 `=` 形式的
    // 关键字参数此前被当作赋值表达式（Expr::Assign），codegen 生成 `Node { val: val = n }`
    // → rustc E0425 cannot find value `val`。修复：调用参数位识别 `Ident = expr` 为 KwArg
    // （与 `Ident : expr`、`Ident~` 一致）。
    let src = r#"
struct Node:
    val: int
    next: int?

def main() =
    let n = Node(val=10, next=None)
    print(n.val)
"#;
    let r = run_lz("struct_kwarg_eq", src);
    assert!(r.transpiled, "`=` 关键字参数应可转译：\n{}", r.diag);
    assert!(r.ok, "`=` 关键字参数产物编译应通过（不再生成 val: val = n）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "10");
}

#[test]
fn generic_struct_eq_injects_partial_eq() {
    // BUG-B 回归（marvis 寻虫 2026-10-17）：泛型 struct 内联 `__eq__` 比较泛型字段
    // （`def __eq__(self, other: Box<T>) -> bool = self.val == other.val`）此前未把
    // `T: PartialEq` 注入 impl 块（compute_cmp_constraints 不扫描 StructDef 方法，且
    // scan 不识别 `Named{path:"T"}` 形式的泛型）→ rustc E0369 binary operation `==`
    // cannot be applied to type `T`。
    let src = r#"
struct Box<T> =
    val: T
    def __eq__(self, other: Box<T>) -> bool =
        self.val == other.val

def main() =
    let b1 = Box<int>(val=42)
    let b2 = Box<int>(val=42)
    print(b1 == b2)
"#;
    let r = run_lz("generic_struct_eq", src);
    assert!(r.transpiled, "泛型 struct __eq__ 应可转译：\n{}", r.diag);
    assert!(r.ok, "泛型 struct __eq__ 产物编译应通过（T: PartialEq 注入 impl）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "true");
}

#[test]
fn to_string_on_primitives_prints_display_not_debug() {
    // BUG-44(续) 回归（fist-agent 寻虫 2026-10-18）：`.to_string()` 的 IR 返回类型
    // 推断存在 **match 顺序缺陷**——`IrType::Int`/`IrType::F64` 臂先于末尾追加的
    // 「字符串化」臂命中，`to_string => String` 对 int/f64 不可达 ⇒ `n.to_string()`
    // 推断为 Any → `print` 走 `{:?}` Debug 输出 `"42"`（带引号）。Bool/Int128/BigInt
    // 没有更早的臂、能到达字符串化臂，故此前只有 int/f64 出错（BUG-44 曾记录「两侧
    // 同错」并标记为独立洞未修）。
    // 修复：把 `to_string => String` 提前登记到 Int/F64 自身臂。
    let src = r#"
def main() =
    let n = 42
    print(n.to_string())
    let x = 3.14
    print(x.to_string())
    let b = true
    print(b.to_string())
    print("abc".to_string())
"#;
    let r = run_lz("tostr_display", src);
    assert!(r.transpiled, "to_string() 应可转译：\n{}", r.diag);
    assert!(r.ok, "to_string() 产物应可编译运行：\n{}", r.diag);
    assert_eq!(
        r.stdout.trim(),
        "42\n3.14\ntrue\nabc",
        "print(<primitive>.to_string()) 应走 Display（无引号），而非 Debug（带引号）"
    );
}

#[test]
fn list_first_get_are_option_consistent_with_codegen() {
    // BUG-281 vs BUG-316 类型漂移回归（fist-agent 寻虫 2026-10-18）：
    // codegen 已按 BUG-316 保留 Option（`values.first()` → `Option<&T>` → `.cloned()`
    // → `Option<T>`），但 infer.rs 仍把 `.get()/.first()/.last()/.pop()` 标成 `T`，
    // 造成 IR 类型与产物值矛盾——`xs.get(1) + 1` 在 IR 层面通过（都算 int），
    // 产物却是 `Option<i64> + i64` → rustc E0369。
    // 修复：infer 跟进 codegen，把这四个方法标成 `Option<T>`。
    // 语料实证（全语料无一处与 `-1` 比较）：DEMO/edge-walrus-operator.lz:12 写
    // `values.first().unwrap_or(0)` ⇒ Option 是现行语义。
    let src = r#"
def main() =
    let xs = [10, 20, 30]
    print(xs.first().unwrap_or(0))
    print(xs.get(1).unwrap_or(-1))
    let last = xs.last().unwrap_or(0)
    print(last)
    print(xs.get(99).unwrap_or(-1))
"#;
    let r = run_lz("list_opt", src);
    assert!(r.transpiled, "first/get/last 应按 Option 语义转译：\n{}", r.diag);
    assert!(r.ok, "unwrap_or 后产物应编译运行通过：\n{}", r.diag);
    assert_eq!(
        r.stdout.trim(),
        "10\n20\n30\n-1",
        "first/get/last 应为 Option<T>（调用方可 unwrap_or），越界回退到 -1"
    );
}

#[test]
fn list_max_min_return_option_and_empty_does_not_panic() {
    // BUG-266 回归（fist-agent 寻虫 2026-10-18）：`List.max()/.min()` 原先生成
    // `(*recv.iter().max().unwrap())`，空 List 实跑 panic
    // `called Option::unwrap() on a None value`（TEMP/probe/list_minmax_empty.lz）。
    // 改为 `.iter().max().cloned()` 保留 Option<T>，与 first/last/get/pop
    // （BUG-316 / BUG-265）同一语义族，调用方自写 `.unwrap_or(-1)`。
    let src = r#"
def main() =
    let xs = [3, 1, 4, 1, 5]
    print(xs.max().unwrap_or(-1))
    print(xs.min().unwrap_or(-1))
    let empty: List<int> = []
    print(empty.max().unwrap_or(-1))
    print(empty.min().unwrap_or(-1))
"#;
    let r = run_lz("list_minmax", src);
    assert!(r.transpiled, "List.max()/.min() 应可转译：\n{}", r.diag);
    assert!(r.ok, "空 List 的 max/min 不应 panic（Option 语义，unwrap_or 兜底）：\n{}", r.diag);
    assert_eq!(
        r.stdout.trim(),
        "5\n1\n-1\n-1",
        "max/min 应为 Option<T>：非空给 Some，空 List 给 None（unwrap_or 后 -1）"
    );
}

#[test]
fn dict_get_is_option_matching_codegen() {
    // BUG-267 回归（fist-agent 寻虫 2026-10-18）：Dict.get 的 IR 类型原先标 V
    // （infer.rs "get" => args[1]），但 codegen 生成 `(d.get(k)).cloned()` →
    // Option<V>（TEMP/probe/dict_get.lz 实测 `Some(1)` / `None`），类型与值矛盾
    // （同 BUG-265 家族）。infer 跟进标 Option<V> 后两边一致。
    let src = r#"
def main() =
    let d = {"a": 1, "b": 2}
    print(d.get("a"))
    print(d.get("zzz"))
    print(d.get("b").unwrap_or(-1))
"#;
    let r = run_lz("dict_get", src);
    assert!(r.transpiled, "Dict.get 应可转译：\n{}", r.diag);
    assert!(r.ok, "Dict.get 应编译运行通过：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "Some(1)\nNone\n2", "Dict.get 应为 Option<V>：命中给 Some，未命中给 None");
}

#[test]
fn dict_pop_routes_to_remove_and_returns_option() {
    // BUG-268 回归（fist-agent 寻虫 2026-10-18）：LZ Dict.pop(key) 原先生成
    // `d.pop(key)`，但 BTreeMap/HashMap 无 pop 方法 → E0599
    // `no method named 'pop'`（TEMP/probe/dict_pop.lz 实测 RUSTC_FAIL）。
    // 现路由到 `d.remove(key)`（返回 Option<V>），与 Dict.remove（BUG-289）一致。
    let src = r#"
def main() =
    let d = {"a": 1, "b": 2}
    let p = d.pop("a")
    print(p)
    print(d.pop("zzz"))
"#;
    let r = run_lz("dict_pop", src);
    assert!(r.transpiled, "Dict.pop 应可转译：\n{}", r.diag);
    assert!(r.ok, "Dict.pop 不应报 E0599 no method named `pop`：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "Some(1)\nNone", "Dict.pop 应返回 Option<V>：命中给 Some，未命中给 None");
}

#[test]
fn option_in_scalar_op_is_clean_lz_type_error() {
    // Option<int> 直接参与运算（`xs.first() + 1`），原先 codegen 直出
    // `Option<i64> + 1i64` → rustc E0369（TEMP/probe/option_plus_int.lz 实测
    // RUSTC_FAIL exit=1），而非干净 LZ 类型错误——与 Result 家族同源（extended_check
    // Binary 臂 L1316 只拦 Result）。现补 Option 拦截，报引导性错误提示解包方式。
    let src = r#"
def main() =
    let xs = [1, 2, 3]
    let a = xs.first() + 1
    print(a)
"#;
    let r = run_lz("option_plus_int", src);
    assert!(
        !r.transpiled,
        "Option<int> + int 应在 LZ 类型检查阶段被拦截，不应产出坏 Rust 到 rustc"
    );
    let all = format!("{}\n{}", r.diag, r.stderr);
    assert!(
        all.contains("Option 值不能直接参与运算"),
        "应报干净 LZ 类型错误（含解包提示），实际诊断：\n{}",
        all
    );
}

#[test]
fn bool_in_scalar_op_is_clean_lz_type_error() {
    // bool 不参与算术/位/序运算：`true + 1` 原先 codegen 直出 `b + 1i64`（b: bool）→
    // rustc E0369（TEMP/probe/bool_plus_int.lz 实测 RUSTC_FAIL），现补 Bool 拦截。
    // 同时验证合法形态未被误伤：f64+int=3.5、str+str 拼接、list+list 拼接。
    let src = r#"
def main() =
    let b = true
    print(b + 1)
"#;
    let r = run_lz("bool_plus_int", src);
    assert!(!r.transpiled, "Bool + int 应在 LZ 阶段被拦截，不应到达 rustc");
    let all = format!("{}\n{}", r.diag, r.stderr);
    assert!(
        all.contains("Bool 值不能直接参与运算"),
        "应报干净 LZ 类型错误，实际诊断：\n{}",
        all
    );
    // 合法形态回归：数字混算 / str 拼接 / list 拼接
    let ok_src = r#"
def main() =
    let a = 1.5
    let b = 2
    print(a + b)
    let s1 = "hi"
    let s2 = " there"
    print(s1 + s2)
    let xs = [1, 2]
    print(xs + [3])
"#;
    let ok = run_lz("arith_valid_kept", ok_src);
    assert!(ok.ok, "合法算术/拼接应继续通过：\n{}", ok.diag);
    assert_eq!(
        ok.stdout.replace('\n', " ").trim(),
        "3.5 hi there [1, 2, 3]"
    );
    // 链式比较豁免：`1 < 5 < 10` 解析为 `(1<5) < 10`（lhs 为 bool），
    // codegen 会分解为 `1<5 && 5<10`（expr_gen.rs ~L5460）。bool 拦截需放行，
    // 否则破坏 CY/15_feature_matrix/precedence.lz 与 DEMO/13_operators/precedence.lz。
    let chain_src = r#"
def main() =
    let chain = 1 < 5 < 10
    print(chain)
"#;
    let c = run_lz("cmp_chain_valid", chain_src);
    assert!(c.ok, "比较链应编译运行：\n{}", c.diag);
    assert_eq!(c.stdout.trim(), "true");
    // 真 bool 错误仍拦截：`true < 5`（非链，lhs 为字面量）
    let bad_src = r#"
def main() =
    let b = true
    print(b < 5)
"#;
    let b2 = run_lz("bool_lt_int", bad_src);
    assert!(
        !b2.transpiled
            && format!("{}\n{}", b2.diag, b2.stderr).contains("Bool 值不能直接参与运算"),
        "`true < 5` 应报干净 LZ 类型错误：\n{}{}",
        b2.diag,
        b2.stderr
    );
}

/// 标量运算的类型族交叉拦截（TEMP/probe/sweep2 五组实测 RUSTC_FAIL 反哺）：
/// ① `Option * Option`（两侧皆 Option 的算术）② `f64 << int`（浮点位/移位）
/// ③ `str < int`（字符串跨族比较）④ `bool == int`（Bool 跨族 Eq/Ne）。
/// 同时锁定合法形态未被误伤：`Option<Lt>Option` 比较、`list < list`、
/// `bool == bool`、`f64 + int` 数字混算。
#[test]
fn scalar_op_type_family_cross_is_clean_lz_type_error() {
    let assert_clean = |tag: &str, body: &str, needle: &str| {
        let src = format!("def main() =\n    let v = {}\n    print(v)\n", body);
        let r = run_lz(tag, &src);
        let all = format!("{}\n{}", r.diag, r.stderr);
        assert!(
            !r.transpiled && all.contains(needle),
            "`{}` 应报干净 LZ 类型错误（含 `{}`），实际：\n{}{}",
            body,
            needle,
            r.diag,
            r.stderr
        );
    };
    // ① 两侧皆 Option 的算术
    assert_clean(
        "opt_mul_opt",
        "[1,2].first() * [1,2].first()",
        "Option 值不能直接参与算术/位运算",
    );
    // ② 浮点位/移位
    assert_clean("f64_shl_int", "1.5 << 2", "浮点值不能参与位/移位运算");
    assert_clean("f64_bitand", "1.5 & 2", "浮点值不能参与位/移位运算");
    // ③ 字符串跨族比较
    assert_clean("str_lt_int", "\"a\" < 1", "字符串与其他类型不能直接比较");
    // ④ Bool 跨族 Eq/Ne
    assert_clean("bool_eq_int", "true == 1", "Bool 值不能与其他类型比较");
    // ⑤ None 字面量（含 Ident("None") 形态）参与算术/位 → E0369
    assert_clean(
        "none_plus_int",
        "None + 1",
        "None 值不能直接参与算术/位运算",
    );
    assert_clean(
        "none_bitand_int",
        "None & 1",
        "None 值不能直接参与算术/位运算",
    );

    // 合法形态回归
    let ok_src = r#"
def main() =
    let a = [1, 2].first()
    let b = [3, 4].first()
    print(a < b)
    print([1] < [2])
    print(true == true)
    print(1.5 + 2)
    print(a == None)
    print(None == None)
    print(None != None)
    print(1.5 == 1)
    print(1.5 > 2)
    print(3 < 2.5)
    print(1 == 1)
    print("ab" * 2)
    print([1, 2] * 2)
    print("ab" * -1)
    print([1] * 0)
"#;
    let ok = run_lz("scalar_family_valid_kept", ok_src);
    assert!(ok.ok, "合法标量运算应继续通过：\n{}", ok.diag);
    assert_eq!(
        ok.stdout.replace('\n', " ").trim(),
        "true true true 3.5 false true false false false false true abab [1, 2, 1, 2]  []"
    );
}

// BUG-278：type alias 解析。`type Pair = (int, int)` 后 `p: Pair` 的 IR 类型是
// Named("Pair")，IndexGet 若不解析回 Tuple 会生成 `p[0i64 as usize]` → rustc E0608。
// 覆盖顶层 alias 与函数内 alias 两种形态。
#[test]
fn type_alias_tuple_index_is_tuple_field_access() {
    let src = r#"
type Pair = (int, int)
def main() =
    let p: Pair = (1, 2)
    print(p[0])
    print(p[1])
    type Local = (int, int)
    let q: Local = (3, 4)
    print(q[0] + q[1])
"#;
    let ok = run_lz("type_alias_tuple_index", src);
    assert!(ok.ok, "type alias 元组索引应通过：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "1 2 7");
}

// BUG-279：裸 Vec 的 `.filter(|x| => x > 2)` 方法闭包。Rust Iterator::filter 的
// 闭包参数是 &Item，body 若直接用参数名比较则 `x > 2i64` 是 &i64 > i64 → E0308。
// 修法：裸 Vec filter 走 into_iter 链时对闭包套 strip_lambda_type_with_ref（加 & 模式）。
// map 闭包参数是 owned，不能加 &，故只 filter 套。
#[test]
fn vec_filter_method_closure_ref_param() {
    let src = r#"
def main() =
    let a = [1, 2, 3, 4, 5]
    let b = a.filter(|x| => x > 2)
    print(b)
    let c = [1, 2, 3, 4, 5].map(|x| => x * 2)
    print(c)
    let d = [1, 2, 3, 4, 5].filter(|x| => x % 2 == 0)
    print(d)
"#;
    let ok = run_lz("vec_filter_method_closure", src);
    assert!(ok.ok, "裸 Vec filter/map 闭包应通过：\n{}", ok.diag);
    assert_eq!(
        ok.stdout.replace('\n', " ").trim(),
        "[3, 4, 5] [2, 4, 6, 8, 10] [2, 4]"
    );
}

// BUG-280：裸 Vec 无 `.reduce(closure, init)`（E0599）。Rust 归约是 `Iterator::fold`。
#[test]
fn vec_reduce_method_fold() {
    let src = r#"
def main() =
    let a = [1, 2, 3, 4]
    let b = a.reduce(|acc, x| => acc + x, 0)
    print(b)
    let c = [1, 2, 3].reduce(|acc, x| => acc * x, 1)
    print(c)
"#;
    let ok = run_lz("vec_reduce_method", src);
    assert!(ok.ok, "裸 Vec reduce 应通过：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "10 6");
}

// BUG-281：裸 Vec 无 `.enumerate()`（E0599），自由函数 enumerate 缺 `.collect()`（E0608）。
// 且 Rust Enumerate 产出 (usize, T)，需映射回 LZ 的 int（i64），否则类型注解
// `let b: Vec<(i64, String)>` 与 collect 的 `Vec<(usize, String)>` 冲突 → E0308。
#[test]
fn vec_enumerate_method_and_freefn() {
    let src = r#"
def main() =
    let a = ["x", "y"]
    let b = a.enumerate()
    let c = enumerate(["p", "q"])
    print(b[0])
    print(b[1][0])
    print(b[1][1])
    print(c[0])
"#;
    let ok = run_lz("vec_enumerate_method", src);
    assert!(ok.ok, "裸 Vec enumerate 应通过：\n{}", ok.diag);
    assert_eq!(
        ok.stdout.replace('\n', " ").trim(),
        "(0, \"x\") 1 y (0, \"p\")"
    );
}

// BUG-282：`b[0][1]`（b 是 List<(int, str)>）—— Vec 索引返回 &T，字段 String 非 Copy
// 时从借用 move → E0507。需 `.clone()`。
#[test]
fn tuple_index_after_index_clone() {
    let src = r#"
def main() =
    let a = ["x", "y"]
    let b = a.enumerate()
    let i = b[0][0]
    let s = b[0][1]
    print(i)
    print(s)
"#;
    let ok = run_lz("tuple_index_after_index_clone", src);
    assert!(ok.ok, "元组索引后字段访问应通过：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "0 x");
}

// BUG-283：`def gen(): yield 1; yield 2; yield 3`（未标 `iterator` 关键字）的生成器。
// ① 返回类型推断把尾表达式 `yield 3` 推成 Itor<i64>（lz_builtins 无此类型）→ E0425；
// ② 调用点 `let g: Vec<i64> = gen()` 后 `g.next()` —— 裸 Vec 无 next（E0599）。
// 修法：collect_functions/convert_decl 对含 yield 的无注解函数按 yield 元素类型推断，
// 调用点登记为 Vec<T>；codegen 把 Vec 上的 `.next()` 路由到 `into_iter().next()`。
#[test]
fn generator_func_unannotated() {
    let src = r#"
def gen():
    yield 1
    yield 2
    yield 3
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_func_unannotated", src);
    assert!(ok.ok, "未注解生成器应通过：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(1)");
}

// BUG-284：`def evens(n: int) = ... while ...: if ...: yield i` 中 yield 位于
// `if` 表达式分支体内。`ast_body_has_yield` / `scan_iterator_yield_ty` 只遍历
// AstStmt 语句，未下钻 Stmt::Expr(Expr::If) 的分支体 → yield 漏检 → 元素类型
// 推断回退 Unit → `-> Vec<()>`（E0599 Vec<()> 无 next）。
// 修法：新增 ast_expr_has_yield / scan_expr_yield_ty 下钻 If/Match/BlockExpr/Closure。
#[test]
fn generator_if_in_while() {
    let src = r#"
def evens(n: int) =
    let mut i = 0
    while i < n:
        if i % 2 == 0:
            yield i
        i += 1
def main() =
    let g = evens(6)
    print(g.next())
"#;
    let ok = run_lz("generator_if_in_while", src);
    assert!(ok.ok, "if 分支内的 yield 应正确推断元素类型：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(0)");
}

// BUG-285：`match` 分支体是 Block，`Stmt::Match` 各分支内的 yield 未被
// block_has_yield 下钻 → 函数返回类型 i64（而非 Vec<i64>）且 __gen_vec 未声明
// → E0425（TEMP/probe/sweep6/gen_match.lz）。修法：helpers.rs block_has_yield /
// first_yield_type 增加 Stmt::Match 分支。
#[test]
fn generator_match_arms() {
    let src = r#"
def classify(x: int) =
    match x:
        0 => yield 100
        1 => yield 200
        _ => yield 300
def main() =
    let g = classify(0)
    print(g.next())
"#;
    let ok = run_lz("generator_match_arms", src);
    assert!(ok.ok, "match 分支内的 yield 应正确推断：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(100)");
}

// BUG-286：生成器返回值 Vec<T> 的 `.next()` 应返回 Option<elem>（与
// Iterator<T>::next 同构）；此前 lookup_builtin_method_ret 无 Vec 分支 → 回退
// Any → `yield g.next()` 元素类型归一成 i64 → Vec<i64> 却 push(Option<i64>)
// → E0308（TEMP/probe/sweep6/gen_nested.lz）。
#[test]
fn generator_yield_next_of_gen() {
    let src = r#"
def make_gen():
    yield 1
    yield 2
def outer():
    let g = make_gen()
    yield g.next()
def main() =
    let o = outer()
    print(o.next())
"#;
    let ok = run_lz("generator_yield_next_of_gen", src);
    assert!(ok.ok, "yield g.next() 应正确推断 Option<elem>：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(Some(1))");
}

// BUG-287：`rewrite_iterator_returns` 把生成器内所有 `return` 重写为 `raise ()`
// （等价 Python StopIteration）。但 Raise 的 codegen 只要 `in_generator` 就
// `push(Err(..))`，无视函数是否有 `raises` 子句 → 非 raises 生成器 `Vec<T>`
// 里 push `Err(())` → E0308（TEMP/probe/sweep6/gen_return.lz）。
#[test]
fn generator_return_stops() {
    let src = r#"
def gen():
    yield 1
    return
    yield 2
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_return_stops", src);
    assert!(ok.ok, "生成器内 return 应提前退出收集：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(1)");
}

// BUG-288：生成器 + raises 的返回类型与调用点类型错配。
// (1) convert_decl.rs：`def gen() raises str:` 无返回注解时 ok_ty 应取
//     scan_iterator_yield_ty 推断出的 yield 元素类型（旧逻辑 unwrap_or(Unit) →
//     Vec<Result<(),String>> 与 `push(Ok(1i64))` E0308）。
// (2) mod.rs：生成器 + raises 时返回类型应为 `Vec<Result<T,E>>`（每次 yield 一个
//     Result），不是 `Result<Vec<T>,E>`；否则调用点 `let g: Result<Vec<T>,E>`
//     与实际 codegen 的 `Vec<Result<T,E>>` 不符 → g.next() E0599。
// (3) current_is_iterator 只按 func.is_iterator 设置，漏掉 body 含 yield 的函数
//     → return 值触发隐式转换（"boom" → i64），随后 rewrite_iterator_returns
//     转成 Stmt::Raise 时值已被污染 → push(Err(<i64 as ImplicitFrom<String>>::
//     __implicit_from__("boom"))) E0277。
#[test]
fn generator_raises() {
    let src = r#"
def gen() raises str:
    yield 1
    raise "boom"
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_raises", src);
    assert!(ok.ok, "生成器 + raises：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(Ok(1))");
}

#[test]
fn generator_raises_return() {
    let src = r#"
def gen() raises str:
    yield 1
    return "boom"
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_raises_return", src);
    assert!(ok.ok, "生成器 + raises + return 应视为 raise：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(Ok(1))");
}// BUG-289：生成器内 try/catch/finally 各分支体含 yield 时，返回类型与 __gen_vec
// 声明漏检（helpers.rs block_has_yield / first_yield_type 无 Stmt::TryCatch 分支）。
// 旧行为：codegen 生成 `-> Itor<i64>`（类型来自 try 表达式）但 __gen_vec 未声明
// → E0425（TEMP/probe/sweep6/gen_try.lz）。
#[test]
fn generator_try_yield() {
    let src = r#"
def gen():
    try:
        yield 1
    catch:
        yield 2
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_try_yield", src);
    assert!(ok.ok, "生成器内 try/catch yield：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(1)");
}// BUG-290：生成器 `yield None; yield Some(1)` 元素类型推断。
// scan_iterator_yield_ty 对 NoneLit 返回 Any → 归一 i64 → Vec<i64> 却
// push(Option::None) → E0308（TEMP/probe/sweep6/gen_opt.lz）。
#[test]
fn generator_yield_none() {
    let src = r#"
def gen():
    yield None
    yield Some(1)
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_yield_none", src);
    assert!(ok.ok, "生成器 yield None + Some(1)：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(None)");
}// BUG-292：生成器 if 分支体内 `return`（语句位置的 if 是 Stmt::ExprStmt 包裹
// ExprKind::IfExpr，原 ir_block_has_yield / rewrite_iterator_returns 均未下钻
// Stmt::ExprStmt）。旧行为：return 未重写为 raise → codegen 生成
// panic!("generator return")（TEMP/probe/sweep6/gen_nested_if_ret.lz）。
#[test]
fn generator_if_return() {
    let src = r#"
def gen():
    if true:
        yield 1
        return
    yield 2
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_if_return", src);
    assert!(ok.ok, "生成器 if 内 return：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(1)");
}// BUG-293：raises 生成器 if 分支体内裸 `return`。`rewrite_iterator_returns` 把裸
// return 重写为 `raise ()`（value=Unit），raises 生成器是 Vec<Result<String,String>>，
// push(Err(())) → E0308（TEMP/probe/sweep7/gen_if_yield_ret.lz）。修法：Stmt::Raise
// codegen 对 raises 生成器的 Unit 值视为成功终止（return __gen_vec），不 push Err。
#[test]
fn generator_raises_if_return() {
    let src = r#"
def gen() raises str:
    if true:
        yield "ok"
        return
    yield "bad"
def main() =
    let g = gen()
    print(g.next())
"#;
    let ok = run_lz("generator_raises_if_return", src);
    assert!(ok.ok, "raises 生成器 if 内裸 return：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(Ok(\"ok\"))");
}// BUG-294：struct impl 方法生成器调用返回类型在 IR 侧为 Any（`let g: ? =
// c.gen()`），codegen .next() 特判 recv_is_list=false → 未转 into_iter().next()
// → E0599 no method next found for Vec<i64>（TEMP/probe/sweep8/gen_struct.lz）。
// 修法：.next() 特判增加 Any 接收者（codegen 发射 `fn gen(&self) -> Vec<i64>`，
// 运行期确实是 Vec）。
#[test]
fn generator_struct_method() {
    let src = r#"
struct Counter:
    n: int
impl Counter:
    def gen(self):
        yield self.n
        yield self.n + 1
def main() =
    let c = Counter { n: 42 }
    let g = c.gen()
    print(g.next())
"#;
    let ok = run_lz("generator_struct_method", src);
    assert!(ok.ok, "struct impl 生成器 .next()：\n{}", ok.diag);
    assert_eq!(ok.stdout.replace('\n', " ").trim(), "Some(42)");
}

// BUG-485：`let comp: Complex = 3.14i` 的类型注解在 IR 里是 `Named{path:"Complex",
// args:[]}`（builder 保留用户写法大小写），type_map 只挂小写 `"complex"` →
// `.get("Complex")` 未命中 → 注解发射裸 `Complex`（未导入，E0425 cannot find type
// `Complex`），而值侧构造器却发射 `lz_builtins::Complex64::new(..)`，注解/值两侧
// 类型名不一致。修法：rust_type 的 Named 分支对 bigint/complex 内置标量做大小写
// 不敏感映射到 `lz_builtins::BigInt`/`lz_builtins::Complex64`。同理 bigint 注解。
#[test]
fn complex_bigint_annotation_maps_to_builtin_path() {
    let src = r#"
def main() =
    let big: BigInt = 1234567890123456789012345678901234567890
    let comp: Complex = 3.14i
    let c2: complex = 2.5i
    let b2: bigint = 42
    print(big)
    print(comp)
    print(c2)
    print(b2)
"#;
    let r = run_lz("complex_bigint_annot", src);
    assert!(
        r.transpiled && r.ok,
        "Complex/BigInt 注解应映射到 lz_builtins 路径并编译运行：\n{}",
        r.diag
    );
    let out = r.stdout.replace('\n', " ").trim().to_string();
    assert!(
        out.contains("1234567890123456789012345678901234567890"),
        "BigInt 注解值应正确打印：{}",
        out
    );
    assert!(
        out.contains("Complex { re: 0.0, im: 3.14 }"),
        "Complex 注解值应以 Complex64 结构打印：{}",
        out
    );
    assert!(
        out.contains("Complex { re: 0.0, im: 2.5 }"),
        "小写 complex 注解值应同样打印：{}",
        out
    );
    assert!(
        out.ends_with("42"),
        "小写 bigint 注解值应为 42：{}",
        out
    );
}

// ════════════════════════════════════════════════════════════════════════════════
// rs-lz（自举路线 B）回归闸门 —— fist-mbt T0r5：N1 / N2 / R1 / R2
//
// 链路说明（决定了断言的写法）：`--emit=rs-lz <abs>.lz` 做的是
//   input.lz → input.lzrs（LZ 版 codegen 库 + IR 构造 + 打印）
//            → input.rs（**打印器**的 Rust）→ rustc → exe → 运行
// 运行的 stdout 才是**目标程序的 Rust 源码**；CLI 并不编译这份 Rust。
// 所以「生成的 Rust 能否编译运行」必须由本文件自己再 rustc 一次。
//
// 已知台账外缺陷（超出 T0r5 四条范围，不改产品码，仅记录）：
// BUG-475 的 Box 解引用按**字段名**匹配，而定位（非命名）变体字段名是空串，
// 导致 `let val = *val;` 落到 i64 上（rustc E0614）。R1 样本因此用命名字段变体；
// 该缺陷在语句位 match 上同样存在，属 HEAD 既有（T0r5 未改 box 判定逻辑）。
// 注：产物里的 `List<T>`/`Dict<K,V>` 由 `use lz_builtins::*;` 提供
// （lz_builtins/src/flat.rs:522-523 `pub type List<T> = Vec<T>`），故无需任何 shim。
// ════════════════════════════════════════════════════════════════════════════════

/// rs-lz 一次完整跑的观测面。
struct RsLzRun {
    /// `--emit=rs-lz` 自身是否成功（生成期 panic，如 N2 的字节切片 panic，在这里就红）
    emitted: bool,
    /// 生成的目标程序 Rust 源码（stdout）
    rust: String,
    /// 生成的目标程序 Rust 源码是否被 rustc 接受
    compile_ok: bool,
    /// 生成的目标程序是否跑通
    run_ok: bool,
    /// 目标程序 stdout
    stdout: String,
    diag: String,
}

fn rslz_workdir(tag: &str) -> common::TempDir {
    // T0r27：同 run_lz，工作目录改为进程内唯一（pid + 原子计数 + 时间戳），
    // `Drop` 时递归删除；`LZ_KEEP_TEMP=1` 可保留现场。
    common::unique_dir(&format!("lz_rslz_{tag}"))
}

fn lang_zone_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 编译并运行一段 Rust 源码（生成产物原样编译，不做任何改写）。
fn rustc_and_run(tag: &str, rust_src: &str) -> (bool, bool, String, String) {
    let work = rslz_workdir(&format!("{tag}_rust"));
    let rs = work.path().join(format!("{tag}_gen.rs"));
    std::fs::write(&rs, rust_src).expect("write gen rust");
    let exe = work.path().join(format!("{tag}_gen.exe"));
    let rc = Command::new(std::env::var("LZ_RUSTC").unwrap_or_else(|_| "rustc".to_string()))
        .args(["--edition", "2021", "--crate-name"])
        .arg(format!("{tag}_gen"))
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            builtins_rlib().parent().unwrap().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc on rs-lz output");
    if !rc.status.success() {
        return (
            false,
            false,
            String::new(),
            format!(
                "rs-lz 生成的 Rust 编译失败:\n{}",
                String::from_utf8_lossy(&rc.stderr)
            ),
        );
    }
    let prog = Command::new(&exe).output().expect("run rs-lz output exe");
    let stdout = clean(&String::from_utf8_lossy(&prog.stdout));
    (
        true,
        prog.status.success(),
        stdout,
        String::from_utf8_lossy(&prog.stderr).to_string(),
    )
}

/// 跑 `--emit=rs-lz` 并把生成的目标程序 Rust 编译运行起来。
fn run_rs_lz(tag: &str, src: &str) -> RsLzRun {
    let work = rslz_workdir(tag);
    let lz = work.path().join("input.lz");
    std::fs::write(&lz, src).expect("write lz");
    let out = Command::new(lang_zone_bin())
        .arg("--emit=rs-lz")
        .arg(&lz)
        .output()
        .expect("run lzc --emit=rs-lz");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let rust = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        return RsLzRun {
            emitted: false,
            rust,
            compile_ok: false,
            run_ok: false,
            stdout: String::new(),
            diag: format!("--emit=rs-lz 生成期失败（panic/编译错误）:\n{stderr}"),
        };
    }
    let (compile_ok, run_ok, stdout, diag) = rustc_and_run(tag, &rust);
    RsLzRun {
        emitted: true,
        rust,
        compile_ok,
        run_ok,
        stdout,
        diag,
    }
}

fn build_ir_from(src: &str) -> lang_zone::ir::IrModule {
    let tokens = lang_zone::lexer::Lexer::new(src).tokenize();
    let mut parser = lang_zone::parser::Parser::new(tokens);
    let ast = parser.parse_module().expect("parse ok");
    lang_zone::ir::builder::build_ir(&ast).expect("build_ir ok")
}

// ── N1（E0507，高）：非 Copy 元素的列表模式解引用 ─────────────────────────────
//
// 根因：`list_elem_bindings` 只按 `Pattern.Ident` 收名字、**不看元素类型**，于是
// `match xs: case [a, ..]`（xs: List<str>）在臂首发 `let a = *a;` —— a 是 slice
// 元素 `&String`，`*a` 是从共享引用移出 ⇒ rustc E0507 cannot move out of `*a`。
// 修法：按 scrutinee 元素类型判 Copy（`list_elem_ty` + `is_copy_irtype`）；Copy 保持
// `let a = *a;`（List<int> 行为不变），非 Copy 发移动语义 `let a = (*a).clone();`。
#[test]
fn n1_noncopy_list_pattern_element_clones() {
    let src = r#"
def main() =
    let xs = ["x", "y"]
    match xs:
        case [a, ..]:
            print(a)
        case []:
            print("<empty>")
    let ys = [1, 2, 3]
    match ys:
        case [b, ..]:
            print(b)
        case []:
            print(-1)
"#;
    let r = run_rs_lz("n1_list_pattern", src);
    assert!(r.emitted, "rs-lz 生成不应失败:\n{}", r.diag);
    assert!(
        r.rust.contains("let a = (*a).clone();"),
        "非 Copy（str）列表模式绑定必须走移动语义 clone：\n{}",
        r.rust
    );
    assert!(
        !r.rust.contains("let a = *a;"),
        "非 Copy 元素不得再发 `let a = *a;`（E0507）：\n{}",
        r.rust
    );
    assert!(
        r.rust.contains("let b = *b;"),
        "Copy（int）元素必须保持 `let b = *b;`（List<int> 行为不变）：\n{}",
        r.rust
    );
    assert!(
        r.compile_ok && r.run_ok,
        "生成的 Rust 应可编译运行（修前 E0507）：\n{}",
        r.diag
    );
    let out = r.stdout.replace('\n', " ");
    assert!(out.contains("\"x\""), "str 列表首元素应打印 x：{out}");
    assert!(out.contains('1'), "int 列表首元素应打印 1：{out}");
}

// ── N2（同族残留，高）：f-string / 字符串辅助按字节切多字节串 ─────────────────
//
// 根因：`gen_fstring` 用 `s[i..(i+1)]`/`s[(i+1)..j]` 按**字节**取字符与子串，
// 多字节 UTF-8 下下标落在字符中间 ⇒ 生成期 panic「end byte index N is not a char
// boundary」，`f"中文{x}"` 一行即崩。同族：starts_upper 的 `s[0..1]`（非 ASCII 首
// 字符/空串）、str_contains 的滑窗（多字节标识符）、codegen_module 的
// `out[0..len-2]` 裁尾。修法：一律改按字符（str_chars/chars_slice、String::contains、
// 分隔符前置不再切尾）。
#[test]
fn n2_fstring_and_string_helpers_are_char_safe() {
    let src = r#"
struct Box2:
    v: int

def main() =
    let mut 累加 = 0
    累加 = 累加 + 1
    let mut 总计 = 0
    总计 = 总计 + 累加
    let mut 汇总 = 0
    汇总 = 汇总 + 总计
    let 盒 = Box2(v: 5)
    print(盒.v)
    print(f"中文{汇总}值")
"#;
    let r = run_rs_lz("n2_cjk_fstring", src);
    assert!(
        r.emitted,
        "非 ASCII f-string / 中文标识符不应在生成期 panic（修前：not a char boundary）：\n{}",
        r.diag
    );
    assert!(
        !r.diag.contains("char boundary"),
        "不应再出现字节切片 panic：\n{}",
        r.diag
    );
    assert!(
        r.rust.contains("format!(\"中文{:?}值\""),
        "f-string 应按字符解析成 format! 模板：\n{}",
        r.rust
    );
    assert!(
        r.compile_ok && r.run_ok,
        "生成的 Rust 应可编译运行：\n{}",
        r.diag
    );
    let out = r.stdout.replace('\n', " ");
    assert!(out.contains('5'), "中文字段访问应打印 5：{out}");
    assert!(out.contains("中文1值"), "中文 f-string 应原样保留：{out}");
}

// ── R1（BUG-475 收口不全，中）：块表达式内的 match 不带 Box 上下文 ──────────────
//
// 根因：`gen_expr` 的 `Expr.BlockExpr` 分支调 `gen_body(ss, true, "", fn_raises,
// user_fns, [], [])` 传**空** tbm/vem。表达式位的 `match` 被 builder 包成 BlockExpr，
// 于是 `let r = match t:` 里的递归 enum 解构不做 `let l = *l;` ⇒ 绑定是 `Box<T>`
// 却当 `T` 用（rustc E0308 expected T, found Box<T>）。
// 修法：gen_expr（及其全部调用者，40 个函数 / 185 个调用点）携带 tbm/vem，BlockExpr
// 分支如实下传。
#[test]
fn r1_blockexpr_match_carries_box_context() {
    let src = r#"
enum BT:
    Node(v: int, l: BT, r: BT)
    Empty

def total(t: BT) -> int =
    let r = match t:
        case BT.Empty:
            0
        case BT.Node(v: vv, l: ll, r: rr):
            vv + total(ll) + total(rr)
    r

def main() =
    let t = BT.Node(v: 1, l: BT.Node(v: 2, l: BT.Empty, r: BT.Empty), r: BT.Empty)
    print(total(t))
"#;
    let r = run_rs_lz("r1_block_match", src);
    assert!(r.emitted, "rs-lz 生成不应失败:\n{}", r.diag);
    assert!(
        r.rust.contains("let ll = *ll;") && r.rust.contains("let rr = *rr;"),
        "块表达式内 match 的递归 Box 字段必须解引用（修前缺失 ⇒ E0308）：\n{}",
        r.rust
    );
    assert!(
        r.compile_ok && r.run_ok,
        "生成的 Rust 应可编译运行（修前 E0308 expected BT, found Box<BT>）：\n{}",
        r.diag
    );
    assert_eq!(r.stdout.trim(), "3", "递归求和应为 1+2=3");
}

// ── R2（BUG-483 缺口，中）：Pattern::LitF64 发裸 NaN/inf ───────────────────────
//
// 根因：`src/ir/lz_codegen.rs:1068` 对 `Pattern::Lit(LitKind::F64(n))` 直接
// `format!("... v: {})", n)`，NaN/inf 时 .lzrs 里就是 `Pattern.LitF64(v: NaN)`，
// LZ 编译器把 `NaN` 当未绑定变量 ⇒ 生成的 .lzrs 重编译失败；LZ 侧 gen_pattern 则发
// `NaN_f64`（非法 Rust）。修法：Rust 侧统一走 lz_f64_expr（0.0/0.0、1.0/0.0、
// -1.0/0.0，与表达式位同口径），LZ 侧 gen_pattern 走 fmt_f64（f64::NAN/INFINITY，
// 与原生 expr_gen.rs:8390 对齐）。
//
// 说明：`case 2.0:` 这类 f64 模式在 builder 里会退化成 Pattern::LitStr("2")，
// 源码层面**写不出** NaN 模式，故本用例在 IR 层注入（与台账「可复现性：低」一致）。
#[test]
fn r2_pattern_lit_f64_nan_serializes_to_valid_lz() {
    let src = r#"
def f(x: f64) -> int =
    match x:
        case 2.0:
            1
        case _:
            0

def main() =
    print(f(2.0))
"#;

    fn patch(ir: &mut lang_zone::ir::IrModule, v: f64) -> usize {
        use lang_zone::ir::node::{Item, LitKind, Pattern, Stmt};
        let mut n = 0;
        for item in &mut ir.items {
            if let Item::FnDef(fd) = item {
                for st in &mut fd.body.stmts {
                    if let Stmt::Match { arms, .. } = st {
                        if let Some(a0) = arms.first_mut() {
                            a0.pattern = Pattern::Lit(LitKind::F64(v));
                            n += 1;
                        }
                    }
                }
            }
        }
        n
    }

    // ① NaN：.lzrs 里必须是合法 LZ 表达式，不得是裸 NaN
    let mut ir = build_ir_from(src);
    assert!(patch(&mut ir, f64::NAN) > 0, "样本应含 match 臂");
    let lzrs = lang_zone::ir::lz_codegen::ir_module_to_rs_lz_source(&ir);
    let lit_lines = lzrs
        .lines()
        .filter(|l| l.contains("LitF64"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        lzrs.contains("Pattern.LitF64(v: 0.0/0.0)"),
        "NaN 模式应以合法 LZ f64 表达式序列化（R2）:\n{lit_lines}"
    );
    assert!(
        !lzrs.contains("v: NaN"),
        "不得再发裸 `NaN` 字面量（LZ 会报「未绑定变量: NaN」）:\n{lit_lines}"
    );
    let _ = lit_lines;

    // ② ±inf 同口径
    let mut ir2 = build_ir_from(src);
    patch(&mut ir2, f64::INFINITY);
    let lzrs_inf = lang_zone::ir::lz_codegen::ir_module_to_rs_lz_source(&ir2);
    assert!(
        lzrs_inf.contains("Pattern.LitF64(v: 1.0/0.0)"),
        "inf 模式应以 1.0/0.0 序列化"
    );
    assert!(!lzrs_inf.contains("v: inf"), "不得再发裸 `inf` 字面量");

    // ③ 生成的 .lzrs 必须能重新过一遍 LZ 编译器（修前：未绑定变量: NaN）
    let work = rslz_workdir("r2_nan_pattern");
    let lzrs_path = work.path().join("nanpat.lzrs");
    std::fs::write(&lzrs_path, &lzrs).expect("write lzrs");
    let out = Command::new(lang_zone_bin())
        .arg(&lzrs_path)
        .output()
        .expect("compile generated lzrs");
    assert!(
        out.status.success(),
        ".lzrs 应重新编译通过（修前「未绑定变量: NaN」）:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // ④ 打印器跑起来后，生成的 Rust 模式位应是 f64::NAN（与原生同口径），不是 NaN_f64
    let printer_rs = work.path().join("nanpat.rs");
    let printer_src = std::fs::read_to_string(&printer_rs).expect("read printer .rs");
    let (compile_ok, run_ok, stdout, diag) = rustc_and_run("r2_printer", &printer_src);
    assert!(compile_ok && run_ok, "打印器应可编译运行:\n{diag}");
    assert!(
        stdout.contains("f64::NAN"),
        "生成的 Rust 模式位应为 f64::NAN：\n{stdout}"
    );
    assert!(
        !stdout.contains("NaN_f64"),
        "不得再发 `NaN_f64`（非法 Rust）"
    );

    // ⑤ 反证：把 .lzrs 改回修前形态（裸 NaN）后 LZ 编译器确实拒绝 —— 证明 ③ 真的在测 R2
    let broken = lzrs.replace("Pattern.LitF64(v: 0.0/0.0)", "Pattern.LitF64(v: NaN)");
    let broken_path = work.path().join("nanpat_broken.lzrs");
    std::fs::write(&broken_path, broken).expect("write broken lzrs");
    let bad = Command::new(lang_zone_bin())
        .arg(&broken_path)
        .output()
        .expect("compile broken lzrs");
    let bad_err = String::from_utf8_lossy(&bad.stderr).to_string();
    assert!(
        !bad.status.success() && bad_err.contains("未绑定变量"),
        "反证失败：修前形态（v: NaN）应被 LZ 编译器拒绝并报「未绑定变量」:\n{bad_err}"
    );
}

// ════════════════════════════════════════════════════════════════════════════════
// fist-mbt T0r6：D1 / D2（HEAD 2aa5088 既有缺陷，均在 src/ir/lz_codegen_lib.lz）
// ════════════════════════════════════════════════════════════════════════════════

// ── D1（高，E0614）：定位（元组）变体字段被误判为 Box ─────────────────────────
//
// 根因：`type_box_map` 把 enum 各变体字段摊平成 `(field_name, is_boxed)`，而**定位
// 变体**的字段名在 IR 里是空串（builder/convert_decl.rs:1980-1984 对非命名字段写
// `name: String::new()`）。旧 `field_is_boxed` 按字段名线性查找，空串等于「任意
// 未命名字段」，同枚举下只要有一个盒化字段（Node 的 left/right：BinaryTree 递归
// 自引用 ⇒ Box），`field_is_boxed(tbm, "BinaryTree", "")` 就命中 ⇒ Node 的首个
// `int val` 也被当成 Box，臂首发 `let val = *val;` → rustc E0614
// `type i64 cannot be dereferenced`。
// 修法：定位变体字段改用合成键 `#pos#<variant>#<index>`（`#` 非 LZ 标识符字符，
// 不与命名字段撞名），`type_box_map` 写入侧与 `pat_field_bindings` 查询侧同规则；
// 命名变体仍按字段名匹配，行为不变（对齐原生 pattern_gen.rs:281-304 的
// (enum, variant, index) 查表口径）。
#[test]
fn d1_positional_enum_variant_box_binding_by_index() {
    let src = r#"
enum BinaryTree:
    Node(int, BinaryTree, BinaryTree)
    Empty

def inorder(tree: BinaryTree) -> List<int> =
    match tree:
        case BinaryTree.Empty:
            []
        case BinaryTree.Node(val, left, right):
            inorder(left) + [val] + inorder(right)

def make_node(val: int, left: BinaryTree, right: BinaryTree) -> BinaryTree =
    BinaryTree.Node(val, left, right)

def make_empty() -> BinaryTree =
    BinaryTree.Empty

// 语句位 match（臂后还有语句，故 match 不是尾表达式）：台账指出 D1 在语句位同样存在，
// 这里一并覆盖 —— 修前臂首的 `let val = *val;` 直接 E0614。
def describe(tree: BinaryTree) =
    match tree:
        case BinaryTree.Empty:
            print("empty")
        case BinaryTree.Node(val, left, right):
            print(val)
    print("")

def main() =
    let n4 = make_node(4, make_empty(), make_empty())
    let n5 = make_node(5, make_empty(), make_empty())
    let n2 = make_node(2, n4, n5)
    let tree = make_node(1, n2, make_empty())
    describe(tree)
    assert inorder(tree) == [4, 2, 5, 1]
    print("d1 ok")
"#;
    let r = run_rs_lz("d1_pos_variant_box", src);
    assert!(r.emitted, "rs-lz 生成不应失败:\n{}", r.diag);
    // 盒化字段（递归自引用）仍要解引用
    assert!(
        r.rust.contains("let left = *left;") && r.rust.contains("let right = *right;"),
        "Box 字段 left/right 必须仍发 `let n = *n;`：\n{}",
        r.rust
    );
    // 非盒化的 i64 字段绝不能再解引用（修前 E0614 的直接来源）
    assert!(
        !r.rust.contains("let val = *val;"),
        "定位变体的 int 字段不得被误判为 Box（rustc E0614 type i64 cannot be dereferenced）：\n{}",
        r.rust
    );
    assert!(
        r.compile_ok && r.run_ok,
        "生成的 Rust 应可编译运行（修前 E0614 + E0369）：\n{}",
        r.diag
    );
    assert_eq!(
        r.stdout.trim(),
        "1\n\nd1 ok",
        "语句位 match 应打印根值 1，中序遍历断言应全部通过"
    );
}

// ── D2（高，E0369）：List + List 拼接发成 Rust `+` ────────────────────────────
//
// 根因：`gen_expr` 的 BinOp 分支只对 `in`/字符串 `+`/`||`/`&&` 做特判，列表 `+`
// 落进通用分支原样透传 `a + b`。Rust 的 `Vec<T>` 没有 `Add` impl ⇒ rustc
// `error[E0369]: cannot add Vec<i64> to Vec<i64>`（树遍历 `inorder(l) + [v] + inorder(r)`
// 即中招）。原生基准：expr_gen.rs:5955-5972 的 `add_is_list_concat` 主分派，产物形态
// 是 `{ let mut __cat = <lhs>.clone(); __cat.extend(<rhs>); __cat }`（拼接返回新 Vec，
// 两侧操作数不被移动）。rs-lz 对齐同一形态。
//
// 连带项（同一产物不修就仍然编不过，故一并在本用例锁定）：
//   · `let acc = acc + xs` 是 LZ 重绑定，按 BUG-454 发成赋值；若声明没标 mut 即
//     E0384（原生 auto_mut_locals 是函数级扫描，match 臂/值位置块内同样算）。
//     修法：gen_value_body 按自身语句表跑 auto_mut_names，且自引用 Let 的目标名并入
//     mut 集。
#[test]
fn d2_list_concat_emits_extend_form() {
    let src = r#"
def concat2(a: List<int>, b: List<int>) -> List<int> =
    a + b

def main() =
    let xs = concat2([1, 2], [3, 4])
    print(xs.len())
    let acc: List<int> = []
    let acc = acc + xs
    let acc = acc + [5]
    print(acc.len())
"#;
    let r = run_rs_lz("d2_list_concat", src);
    assert!(r.emitted, "rs-lz 生成不应失败:\n{}", r.diag);
    // 与原生同形态：先 clone 左值再 extend 右值
    assert!(
        r.rust.contains("let mut __cat = ") && r.rust.contains("__cat.extend("),
        "列表拼接必须发原生基准形态 `{{ let mut __cat = <lhs>.clone(); __cat.extend(<rhs>); __cat }}`：\n{}",
        r.rust
    );
    // 反证：不得再有 Vec 之间的裸 `+`（修前 E0369 的直接来源）
    assert!(
        !r.rust.contains("vec![1i64, 2i64] + vec![3i64, 4i64]"),
        "列表字面量之间不得直接发 `+`（E0369 cannot add Vec to Vec）：\n{}",
        r.rust
    );
    // 自引用重绑定需要 mut（E0384 连带项）
    assert!(
        r.rust.contains("let mut acc"),
        "自引用重绑定 `let acc = acc + ..` 的前置声明必须标 mut（rustc E0384）：\n{}",
        r.rust
    );
    assert!(
        r.compile_ok && r.run_ok,
        "生成的 Rust 应可编译运行（修前 E0369）：\n{}",
        r.diag
    );
    assert_eq!(
        r.stdout.trim(),
        "4\n5",
        "拼接结果长度应为 4，再拼 [5] 后为 5"
    );
}


// BUG-1 回归锁（iter-04:00）：**未标注类型的高阶函数形参**曾按 `rust_type(Any)` 渲染成
// `i64`，`def apply(f, x)` 生成 `f: i64` —— 调用点传闭包 E0308、体内 `f(f(x))` E0618，
// 未标注高阶函数完全不可用。修复：按函数体内 `f(...)` 的调用形态推断出
// `impl Fn(实参类型..) -> 返回类型`。
#[test]
fn unannotated_hof_param_infers_fn_from_call_shape() {
    let src = r#"
def apply_twice(f, x: int) -> int:
    return f(f(x))

def apply_typed(f: fn(int) -> int, x: int) -> int:
    return f(f(x))

def main():
    let inc = |x: int| x + 1
    print(apply_twice(inc, 10))
    print(apply_typed(inc, 20))
"#;
    let r = run_lz("unannotated_hof", src);
    assert!(r.transpiled, "未标注高阶函数应能转译：\n{}", r.diag);
    assert!(r.ok, "未标注高阶函数形参应推断为 impl Fn 并编译运行：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "12\n22");
}

// BUG-1 回归锁（iter-04:00）：未标注形参的**实参类型**也须参与推断——
// `def reduce(f, xs: List<int>)` 里 `f(acc, x)` 两个实参（acc:int / x:int）
// 决定 arity=2 与参数类型，闭包 `|a: int, b: int| a + b` 应能传入。
// 注意：`mul` 不适用本场景——`reduce` 累加器从 0 起，`0 * x` 恒 0，与推断无关，
// 故第二个断言改用无初值歧义的二参直接应用 `apply_pair` 来锁定 arity=2 推断本身。
#[test]
fn unannotated_hof_two_arg_call_shape() {
    let src = r#"
def reduce(f, xs: List<int>) -> int:
    let mut acc: int = 0
    for x in xs:
        acc = f(acc, x)
    return acc

def apply_pair(f, x: int, y: int) -> int:
    return f(x, y)

def main():
    let add = |a: int, b: int| a + b
    print(reduce(add, [1, 2, 3, 4]))
    let mul = |a: int, b: int| a * b
    print(apply_pair(mul, 3, 4))
"#;
    let r = run_lz("unannotated_hof2", src);
    assert!(r.transpiled, "二参未标注高阶函数应能转译：\n{}", r.diag);
    assert!(r.ok, "二参未标注高阶函数形参应推断为二参 impl Fn 并编译运行：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "10\n12");
}
