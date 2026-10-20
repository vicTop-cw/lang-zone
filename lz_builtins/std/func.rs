static mut ns: i64 = 0;
static mut ys: i64 = 0;


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

pub fn fold<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, init: b, mut f: impl Fn(b, a) -> b + 'static) -> b {
    let mut acc = init;
    for x in (xs).into_iter() {
        acc = f(acc.clone(), x.clone());
    }
    return acc;
}

pub fn fold1<a: Debug + Clone>(xs: Vec<a>, mut f: impl Fn(a, a) -> a + 'static) -> Option<a> {
    if (xs.len() as i64) == 0i64 {
        return None;
    } else { ()}
    let mut acc = xs[0i64 as usize].clone();
    for i in (1i64..(xs.len() as i64)) {
        acc = f(acc.clone(), xs[i as usize].clone());
    }
    return Some(acc.clone());
}

pub fn reduce<a: Debug + Clone>(xs: Vec<a>, f: impl Fn(a, a) -> a + 'static) -> Option<a> {
    return fold1(xs.clone(), f);
}

pub fn compose<a: Debug + Clone, b: Debug + Clone, c: Debug + Clone>(mut f: impl Fn(b) -> c + 'static + Send + Sync, mut g: impl Fn(a) -> b + 'static + Send + Sync) -> Arc<dyn Fn(a) -> c + Send + Sync> {
    return Arc::new(move |x| { f(g(x.clone())) });
}

pub fn pipe<a: Debug + Clone, b: Debug + Clone>(x: a, mut f: impl Fn(a) -> b + 'static) -> b {
    return f(x.clone());
}

pub fn pipe2<a: Debug + Clone, b: Debug + Clone, c: Debug + Clone>(x: a, mut f: impl Fn(a) -> b + 'static, mut g: impl Fn(b) -> c + 'static) -> c {
    return g(f(x.clone()));
}

pub fn pipe3<a: Debug + Clone, b: Debug + Clone, c: Debug + Clone, d: Debug + Clone>(x: a, mut f: impl Fn(a) -> b + 'static, mut g: impl Fn(b) -> c + 'static, mut h: impl Fn(c) -> d + 'static) -> d {
    return h(g(f(x.clone())));
}

pub fn pipe4<a: Debug + Clone, b: Debug + Clone, c: Debug + Clone, d: Debug + Clone, e: Debug + Clone>(x: a, mut f: impl Fn(a) -> b + 'static, mut g: impl Fn(b) -> c + 'static, mut h: impl Fn(c) -> d + 'static, mut i: impl Fn(d) -> e + 'static) -> e {
    return i(h(g(f(x.clone()))));
}

pub fn pipe_many<a: Debug + Clone>(x: a, funcs: Vec<fn(a) -> a>) -> a {
    let mut acc = x;
    for f in (funcs).into_iter() {
        acc = f(acc.clone());
    }
    return acc;
}

pub fn and_then<a: Debug + Clone, b: Debug + Clone>(x: Option<a>, mut f: impl Fn(a) -> Option<b> + 'static) -> Option<b> {
    return if x.is_none() { None } else { f(x.clone()) };
}

pub fn unique<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>) -> Vec<a> {
    let mut seen: Vec<a> = Vec::new();
    let mut result: Vec<a> = Vec::new();
    for x in (xs).into_iter() {
        let mut found: bool = false;
        for s in (seen).into_iter() {
            if s == x {
                found = true;
            }
        }
        if !(found) {
            (seen).push(x.clone());
            (result).push(x.clone())
        } else { ()}
    }
    return result;
}

pub fn dedup<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>) -> Vec<a> {
    if (xs.len() as i64) == 0i64 {
        return Vec::<a>::new();
    } else { ()}
    let mut result = vec![xs[0i64 as usize].clone()];
    let mut last = xs[0i64 as usize].clone();
    for i in (1i64..(xs.len() as i64)) {
        let mut curr = xs[i as usize].clone();
        if curr != last {
            (result).push(curr.clone());
            last = curr;
        }
    }
    return result;
}

pub fn count_distinct<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>) -> i64 {
    return (unique(xs.clone()).len() as i64);
}

pub fn chunk<a: Debug + Clone>(xs: Vec<a>, n: i64) -> Vec<Vec<a>> {
    if n <= 0i64 {
        return Vec::<a>::new();
    } else { ()}
    let mut result: Vec<Vec<a>> = Vec::new();
    let mut i: i64 = 0i64;
    while i < (xs.len() as i64) {
        let mut group: Vec<a> = Vec::new();
        let mut j: i64 = 0i64;
        while j < n && i + j < (xs.len() as i64) {
            (group).push(xs[((i + j) as usize)].clone());
            j += 1i64;
        }
        (result).push(group.clone());
        i += n;
    }
    return result;
}

pub fn partition<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> (Vec<a>, Vec<a>) {
    unsafe { ys = Vec::<i64>::new(); }
    unsafe { ns = Vec::<i64>::new(); }
    for x in (xs).into_iter() {
        if pred(x.clone()) { (unsafe { ys }).push(x.clone()) } else { (unsafe { ns }).push(x.clone()) }
    }
    return (unsafe { ys }, unsafe { ns });
}

pub fn flatten<a: Debug + Clone>(xss: Vec<Vec<a>>) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    for xs in (xss).into_iter() {
        for x in (xs).into_iter() {
            (result).push(x);
        }
    }
    return result;
}

pub fn flatten_map<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, f: impl Fn(a) -> Vec<b> + 'static) -> Vec<b> {
    return flatten((xs).into_iter().map(f).collect::<Vec<_>>());
}

pub fn intersperse<a: Debug + Clone>(xs: Vec<a>, sep: a) -> Vec<a> {
    if (xs.len() as i64) == 0i64 {
        return Vec::<a>::new();
    } else { ()}
    let mut result = vec![xs[0i64 as usize].clone()];
    for i in (1i64..(xs.len() as i64)) {
        (result).push(sep);
        (result).push(xs[i as usize].clone());
    }
    return result;
}

pub fn zip<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, ys: Vec<b>) -> Vec<(a, b)> {
    let mut min_len: i64 = if (xs.len() as i64) < (unsafe { ys }.len() as i64) { (xs.len() as i64) } else { (unsafe { ys }.len() as i64) };
    let mut result: Vec<(a, b)> = Vec::new();
    for i in (0i64..min_len) {
        (result).push((xs[i as usize].clone(), unsafe { ys }[i as usize].clone()));
    }
    return result;
}

pub fn unzip<a: Debug + Clone, b: Debug + Clone>(pairs: Vec<(a, b)>) -> (Vec<a>, Vec<b>) {
    let mut xs: Vec<a> = Vec::new();
    unsafe { ys = Vec::<i64>::new(); }
    for p in (pairs).into_iter() {
        (xs).push(p.0);
        (unsafe { ys }).push(p.1);
    }
    return (xs, unsafe { ys });
}

pub fn reverse<a: Debug + Clone>(xs: Vec<a>) -> Vec<a> {
    let mut n: i64 = (xs.len() as i64);
    let mut result: Vec<a> = Vec::new();
    let mut i: i64 = n - 1i64;
    while i >= 0i64 {
        (result).push(xs[i as usize].clone());
        i -= 1i64;
    }
    return result;
}

pub fn test_fold() {
    let mut r: i64 = fold(vec![1i64, 2i64, 3i64, 4i64, 5i64], 0i64, move |acc, x| { LzAdd::__add__(acc, x) });
    println!("{} {:?}", "[fold] sum [1..5]:", r);
    return assert_eq!(r.clone(), 15i64);
}

pub fn test_fold1() {
    let mut r: Option<i64> = fold1(vec![1i64, 2i64, 3i64, 4i64], move |a, b| { LzAdd::__add__(a, b) });
    println!("{} {:?}", "[fold1] sum:", r);
    return assert_eq!(r.clone(), Some(10i64));
}

pub fn test_fold1_empty() {
    let mut r = fold1(Vec::<i64>::new(), move |a, b| { LzAdd::__add__(a, b) });
    println!("{} {:?}", "[fold1] empty:", r);
    return assert_eq!(r.clone(), None);
}

pub fn test_compose() {
    let mut f = compose(move |x| { x * 2i64 }, move |x| { LzAdd::__add__(x, 3i64) });
    let mut r = f(5i64);
    println!("{} {:?}", "[compose] (5+3)*2:", r);
    return assert_eq!(r.clone(), 16i64);
}

pub fn test_pipe3() {
    let mut r = pipe3(2i64, move |x| { x * 3i64 }, move |x| { LzAdd::__add__(x, 4i64) }, move |x| { x * 5i64 });
    println!("{} {:?}", "[pipe3] ((2*3)+4)*5:", r);
    return assert_eq!(r.clone(), 50i64);
}

pub fn test_unique() {
    let mut r: Vec<i64> = unique(vec![1i64, 2i64, 2i64, 3i64, 1i64, 4i64]);
    println!("{} {:?}", "[unique]:", r);
    return assert_eq!(r.clone(), vec![1i64, 2i64, 3i64, 4i64]);
}

pub fn test_dedup() {
    let mut r: Vec<i64> = dedup(vec![1i64, 1i64, 1i64, 2i64, 2i64, 3i64, 3i64, 3i64]);
    println!("{} {:?}", "[dedup]:", r);
    return assert_eq!(r.clone(), vec![1i64, 2i64, 3i64]);
}

pub fn test_chunk() {
    let mut r: Vec<Vec<i64>> = chunk(vec![1i64, 2i64, 3i64, 4i64, 5i64, 6i64, 7i64], 3i64);
    println!("{} {:?}", "[chunk 7/3]:", r);
    return assert_eq!(r.clone(), vec![vec![1i64, 2i64, 3i64], vec![4i64, 5i64, 6i64], vec![7i64]]);
}

pub fn test_partition() {
    (unsafe { ys }, unsafe { ns }) = partition(vec![1i64, 2i64, 3i64, 4i64, 5i64, 6i64], move |x| { x % 2i64 == 0i64 });
    println!("{} {:?} {} {:?}", "[partition] evens:", unsafe { ys }, "odds:", unsafe { ns });
    assert_eq!(unsafe { ys }.clone(), vec![2i64, 4i64, 6i64]);
    return assert_eq!(unsafe { ns }.clone(), vec![1i64, 3i64, 5i64]);
}

pub fn test_flatten() {
    let mut r: Vec<i64> = flatten(vec![vec![1i64, 2i64], vec![3i64, 4i64], vec![5i64]]);
    println!("{} {:?}", "[flatten]:", r);
    return assert_eq!(r.clone(), vec![1i64, 2i64, 3i64, 4i64, 5i64]);
}

pub fn test_zip() {
    let mut r: Vec<(i64, String)> = (vec![1i64, 2i64, 3i64]).into_iter().zip(vec!["a".to_string(), "b".to_string(), "c".to_string()].into_iter());
    println!("{} {:?}", "[zip]:", r);
    assert_eq!((r.len() as i64), 3i64);
    return assert_eq!(r[0i64 as usize], (1i64, "a".to_string()));
}

pub fn test_reverse() {
    let mut r: Vec<i64> = { let mut _tmp = vec![1i64, 2i64, 3i64, 4i64, 5i64]; _tmp.reverse(); _tmp };
    println!("{} {:?}", "[reverse]:", r);
    return assert_eq!(r.clone(), vec![5i64, 4i64, 3i64, 2i64, 1i64]);
}

pub fn main() {
    test_fold();
    test_fold1();
    test_fold1_empty();
    test_compose();
    test_pipe3();
    test_unique();
    test_dedup();
    test_chunk();
    test_partition();
    test_flatten();
    test_zip();
    test_reverse();
    println!("{}", "func.lz: all tests passed!");
}

const __name__: &str = "main";

const __file__: &str = "func.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\lz_builtins\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

