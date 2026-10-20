
// ── LZ rustc link recipe (BUG-10: lz_builtins re-exports num-bigint/num-complex) ──
// To compile & run this generated file in isolation:
//   rustc --edition 2021 --extern lz_builtins=<rlib> -L dependency=<deps_dir> -O <this_file>.rs
//   <rlib>     = liblz_builtins.rlib (build it: `cargo build -p lz_builtins`)
//   <deps_dir> = the directory containing that rlib (e.g. <workspace>/target/debug/deps)
// Omitting -L gives error[E0463] even if this program never uses BigInt/Complex.
// ─────────────────────────────────────────────────────────────────────────────────────

#[allow(unused_imports)]
#[allow(unused_variables)]
#[allow(dead_code)]
#[allow(non_snake_case)]

use std::collections::{HashMap, HashSet};
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;
use std::fmt::Debug;
use std::fmt::Display;

use lz_builtins::*;

#[allow(non_snake_case)] #[allow(dead_code)]
fn panic(__a0: i64) -> i64 { i64::MAX }

pub fn some<a: Debug + Clone>(x: a) -> Option<a> {
    return Some(x);
}

pub fn none<a: Debug + Clone>() -> Option<a> {
    return None;
}

pub fn option(b: bool) -> Option<bool> {
    return if b { Some(b.clone()) } else { None };
}

pub fn option_from_ptr(p: i64) -> Option<i64> {
    return if p != 0i64 { Some(p.clone()) } else { None };
}

pub fn is_some<a: Debug + Clone>(x: Option<a>) -> bool {
    return x.is_some();
}

pub fn is_none<a: Debug + Clone>(x: Option<a>) -> bool {
    return x.is_none();
}

pub fn option_contains<a: Debug + Clone + PartialEq + Eq>(mut x: Option<a>, val: a) -> bool {
    return if x.is_none() { false } else { x.unwrap() == val };
}

pub fn option_map<a: Debug + Clone, b: Debug + Clone>(mut x: Option<a>, mut f: impl Fn(a) -> b + 'static) -> Option<b> {
    return if x.is_none() { None } else { Some(f((x)?)) };
}

pub fn map_or<a: Debug + Clone, b: Debug + Clone>(mut x: Option<a>, default: b, mut f: impl Fn(a) -> b + 'static) -> b {
    return if x.is_none() { default } else { f(x.unwrap()) };
}

pub fn map_or_else<a: Debug + Clone, b: Debug + Clone>(mut x: Option<a>, mut default_fn: impl Fn() -> b + 'static, mut f: impl Fn(a) -> b + 'static) -> b {
    return if x.is_none() { default_fn() } else { f(x.unwrap()) };
}

pub fn and_then<a: Debug + Clone, b: Debug + Clone>(mut x: Option<a>, mut f: impl Fn(a) -> Option<b> + 'static) -> Option<b> {
    return if x.is_none() { None } else { f((x)?) };
}

pub fn or_else<a: Debug + Clone>(x: Option<a>, mut f: impl Fn() -> Option<a> + 'static) -> Option<a> {
    return if x.is_none() { f() } else { x };
}

pub fn filter_option<a: Debug + Clone>(mut x: Option<a>, mut pred: impl Fn(a) -> bool + 'static) -> Option<a> {
    return if x.is_none() { None } else { if pred((x)?) { x } else { None } };
}

pub fn unwrap<a: Debug + Clone>(mut x: Option<a>) -> a {
    if x.is_none() { panic!("{:?}", "called unwrap on None".to_string()) }
    return x.unwrap();
}

pub fn unwrap_or<a: Debug + Clone>(mut x: Option<a>, default: a) -> a {
    return if x.is_none() { default } else { x.unwrap() };
}

pub fn unwrap_or_else<a: Debug + Clone>(mut x: Option<a>, mut f: impl Fn() -> a + 'static) -> a {
    return if x.is_none() { f() } else { x.unwrap() };
}

pub fn expect<a: Debug + Clone>(mut x: Option<a>, msg: String) -> a {
    if x.is_none() { panic!("{:?}", "expected: ".to_string().to_string() + &msg[..]) }
    return x.unwrap();
}

pub fn ok_or<a: Debug + Clone, e: Debug + Clone>(mut x: Option<a>, err: e) -> Result<a, e> {
    return if x.is_none() { Err(err) } else { Ok((x).ok_or_else(|| LzError::new("None".to_string()))?) };
}

pub fn is_ok<a: Debug + Clone, e: Debug + Clone>(r: Result<a, e>) -> bool {
    return true;
}

pub fn is_err<a: Debug + Clone, e: Debug + Clone>(r: Result<a, e>) -> bool {
    return false;
}

pub fn unwrap_result<a: Debug + Clone, e: Debug + Clone>(mut r: Result<a, e>) -> a {
    return r.unwrap();
}

pub fn unwrap_err<a: Debug + Clone, e: Debug + Clone>(r: Result<a, e>) -> e {
    return r;
}

pub fn map_err_result<a: Debug + Clone, e: Debug + Clone, e2: Debug + Clone>(r: Result<a, e>, f: impl Fn(e) -> e2 + 'static) -> Result<a, e2> {
    return r;
}

pub fn flatten_result<a: Debug + Clone, e: Debug + Clone>(mut r: Result<Result<a, e>, e>) -> Result<a, e> {
    return (r)?;
}

pub fn both<a: Debug + Clone, b: Debug + Clone>(mut x: Option<a>, mut y: Option<b>) -> Option<(a, b)> {
    return if x.is_none() || y.is_none() { None } else { Some(((x)?, (y)?)) };
}

pub fn either<a: Debug + Clone>(x: Option<a>, y: Option<a>) -> Option<a> {
    return if x.is_some() { x } else { y };
}

pub fn xor_option<a: Debug + Clone>(x: Option<a>, y: Option<a>) -> Option<a> {
    return if x.is_some() && y.is_none() { x } else { if x.is_none() && y.is_some() { y } else { None } };
}

pub fn collect_options<a: Debug + Clone>(xs: Vec<Option<a>>) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    for x in (xs).into_iter() {
        if x.is_some() { let _ = (result).push(x.unwrap()); } else { () }
    }
    return result;
}

pub fn test_is_some() {
    println!("{} {:?}", "[is_some] Some(5):", is_some(Some(5i64)));
    println!("{} {:?}", "[is_none] None:", is_none(None));
    assert_eq!(is_some(Some(5i64)), true);
    return assert_eq!(is_none(None), true);
}

pub fn test_unwrap_or() {
    println!("{} {:?}", "[unwrap_or] Some(3),10:", unwrap_or(Some(3i64), 10i64));
    println!("{} {:?}", "[unwrap_or] None,10:", unwrap_or(None, 10i64));
    assert_eq!(unwrap_or(Some(3i64), 10i64), 3i64);
    return assert_eq!(unwrap_or(None, 10i64), 10i64);
}

pub fn test_map() {
    let mut r1 = option_map(Some(5i64), move |x| { x * 2i64 });
    let mut r2 = option_map(None, move |x| { x * 2i64 });
    println!("{} {:?}", "[map] Some(5)*2:", r1);
    println!("{} {:?}", "[map] None*2:", r2);
    assert_eq!(r1.clone(), Some(10i64));
    return assert_eq!(r2.clone(), None.clone());
}

pub fn test_and_then() {
    let mut r = and_then(Some(4i64), move |x| { if x > 2i64 { Some(LzAdd::__add__(x, 1i64)) } else { None } });
    println!("{} {:?}", "[and_then] Some(4):", r);
    return assert_eq!(r.clone(), Some(5i64));
}

pub fn test_contains() {
    println!("{} {:?}", "[contains] Some(3),3:", option_contains(Some(3i64), 3i64));
    println!("{} {:?}", "[contains] Some(3),4:", option_contains(Some(3i64), 4i64));
    assert_eq!(option_contains(Some(3i64), 3i64), true);
    return assert_eq!(option_contains(Some(3i64), 4i64), false);
}

pub fn test_both() {
    let mut r: Option<(i64, String)> = both(Some(1i64), Some("a".to_string()));
    println!("{} {:?}", "[both] Some(1),Some(\'a\'):", r);
    assert_eq!(r.clone(), Some((1i64, "a".to_string())));
    return assert_eq!(both(None, Some("a".to_string())), None.clone());
}

pub fn test_collect() {
    let mut xs: Vec<Option<i64>> = vec![Some(1i64), None.clone(), Some(3i64), None.clone(), Some(5i64)];
    let mut r: Vec<i64> = collect_options(xs.clone());
    println!("{} {:?}", "[collect_options]:", r);
    return assert_eq!(r.clone(), vec![1i64, 3i64, 5i64]);
}

pub fn test_join_placeholder() {
    return println!("{}", "[join] placeholder (skipped)");
}

pub fn main() {
    test_is_some();
    test_unwrap_or();
    test_map();
    test_and_then();
    test_contains();
    test_both();
    test_collect();
    println!("{}", "option.lz: all tests passed!");
}

const __name__: &str = "main";

const __file__: &str = "option.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\lz_builtins\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

