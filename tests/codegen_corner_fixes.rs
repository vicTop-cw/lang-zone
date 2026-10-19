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
    let work = std::env::temp_dir().join(format!("lz_cf_{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("input.lz");
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
    let rc = Command::new("rustc")
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
}