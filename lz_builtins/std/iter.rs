
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

pub fn repeat<a: Debug + Clone>(x: a) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    return result;
}

pub fn cycle<a: Debug + Clone>(xs: Vec<a>) -> Vec<a> {
    if (xs.len() as i64) == 0i64 {
        return Vec::<a>::new();
    } else { ()}
    let mut result: Vec<a> = Vec::new();
    return result;
}

pub fn range(start: i64, end: i64) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut i: i64 = start;
    while i < end {
        (result).push(i.clone());
        i += 1i64;
    }
    return result;
}

pub fn range_step(start: i64, end: i64, step: i64) -> Vec<i64> {
    if step == 0i64 {
        return Vec::<i64>::new();
    } else { ()}
    let mut result: Vec<i64> = Vec::new();
    let mut i: i64 = start;
    while (step > 0i64 && i < end) || (step < 0i64 && i > end) {
        (result).push(i.clone());
        i += step;
    }
    return result;
}

pub fn enumerate<a: Debug + Clone>(xs: Vec<a>) -> Vec<(i64, a)> {
    let mut result: Vec<(i64, a)> = Vec::new();
    let mut i: i64 = 0i64;
    for x in (xs).into_iter() {
        (result).push((i, x));
        i += 1i64;
    }
    return result;
}

pub fn zip_all<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, ys: Vec<b>) -> Vec<(a, b)> {
    let mut min_len: i64 = if (xs.len() as i64) < (ys.len() as i64) { (xs.len() as i64) } else { (ys.len() as i64) };
    let mut result: Vec<(a, b)> = Vec::new();
    for i in (0i64..min_len) {
        (result).push((xs[i as usize].clone(), ys[i as usize].clone()));
    }
    return result;
}

pub fn chain<a: Debug + Clone>(xss: Vec<Vec<a>>) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    for xs in (xss).into_iter() {
        for x in (xs).into_iter() {
            (result).push(x);
        }
    }
    return result;
}

pub fn take<a: Debug + Clone>(xs: Vec<a>, n: i64) -> Vec<a> {
    let mut m: i64 = if n < (xs.len() as i64) { n } else { (xs.len() as i64) };
    let mut result: Vec<a> = Vec::new();
    for i in (0i64..m) {
        (result).push(xs[i as usize].clone());
    }
    return result;
}

pub fn drop<a: Debug + Clone>(xs: Vec<a>, n: i64) -> Vec<a> {
    let mut start: i64 = if n < (xs.len() as i64) { n } else { (xs.len() as i64) };
    let mut result: Vec<a> = Vec::new();
    for i in (start..(xs.len() as i64)) {
        (result).push(xs[i as usize].clone());
    }
    return result;
}

pub fn take_while<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    for x in (xs).into_iter() {
        if pred(x.clone()) { (result).push(x.clone())} else {
            return result;
        }
    }
    return result;
}

pub fn drop_while<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    let mut started: bool = false;
    for x in (xs).into_iter() {
        if started { (result).push(x.clone())} else { if !(pred(x.clone())) {
            started = true;
            (result).push(x.clone())
        } else { ()}}
    }
    return result;
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

pub fn window<a: Debug + Clone>(xs: Vec<a>, size: i64) -> Vec<Vec<a>> {
    if size <= 0i64 || (xs.len() as i64) < size {
        return Vec::<a>::new();
    } else { ()}
    let mut result: Vec<Vec<a>> = Vec::new();
    let mut i: i64 = 0i64;
    while i <= (xs.len() as i64) - size {
        let mut w: Vec<a> = Vec::new();
        for j in (i..i + size) {
            (w).push(xs[j as usize].clone());
        }
        (result).push(w.clone());
        i += 1i64;
    }
    return result;
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

pub fn iter_filter<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    for x in (xs).into_iter() {
        if pred(x.clone()) { let _ = (result).push(x.clone()); } else { () }
    }
    return result;
}

pub fn iter_unique<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>) -> Vec<a> {
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

pub fn iter_dedup<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>) -> Vec<a> {
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

pub fn iter_partition<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> (Vec<a>, Vec<a>) {
    let mut yes: Vec<a> = Vec::new();
    let mut no: Vec<a> = Vec::new();
    for x in (xs).into_iter() {
        if pred(x.clone()) { (yes).push(x.clone()) } else { (no).push(x.clone()) }
    }
    return (yes, no);
}

pub fn iter_map<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, mut f: impl Fn(a) -> b + 'static) -> Vec<b> {
    let mut result: Vec<b> = Vec::new();
    for x in (xs).into_iter() {
        (result).push(f(x.clone()));
    }
    return result;
}

pub fn iter_flat_map<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, mut f: impl Fn(a) -> Vec<b> + 'static) -> Vec<b> {
    let mut result: Vec<b> = Vec::new();
    for x in (xs).into_iter() {
        for y in (f(x.clone())).into_iter() {
            (result).push(y);
        }
    }
    return result;
}

pub fn scan<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, state: b, mut f: impl Fn(b, a) -> b + 'static) -> Vec<b> {
    let mut result: Vec<b> = Vec::new();
    let mut acc = state;
    for x in (xs).into_iter() {
        acc = f(acc.clone(), x.clone());
        (result).push(acc.clone());
    }
    return result;
}

pub fn iter_fold<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, init: b, mut f: impl Fn(b, a) -> b + 'static) -> b {
    let mut acc = init;
    for x in (xs).into_iter() {
        acc = f(acc.clone(), x.clone());
    }
    return acc;
}

pub fn iter_fold1<a: Debug + Clone>(xs: Vec<a>, mut f: impl Fn(a, a) -> a + 'static) -> Option<a> {
    if (xs.len() as i64) == 0i64 {
        return None;
    } else { ()}
    let mut acc = xs[0i64 as usize].clone();
    for i in (1i64..(xs.len() as i64)) {
        acc = f(acc.clone(), xs[i as usize].clone());
    }
    return Some(acc.clone());
}

pub fn iter_sum(xs: Vec<i64>) -> i64 {
    return iter_fold(xs.clone(), 0i64, move |acc, x| { LzAdd::__add__(acc, x) });
}

pub fn iter_product(xs: Vec<i64>) -> i64 {
    if (xs.len() as i64) == 0i64 {
        return 1i64;
    } else { ()}
    return iter_fold(xs.clone(), 1i64, move |acc, x| { acc * x });
}

pub fn iter_any<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> bool {
    for x in (xs).into_iter() {
        if pred(x.clone()) {
            return true;
        } else { ()}
    }
    return false;
}

pub fn iter_all<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> bool {
    for x in (xs).into_iter() {
        if !(pred(x.clone())) {
            return false;
        } else { ()}
    }
    return true;
}

pub fn iter_count<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> i64 {
    let mut c: i64 = 0i64;
    for x in (xs).into_iter() {
        if pred(x.clone()) {
            c += 1i64;
        }
    }
    return c;
}

pub fn test_enumerate() {
    let mut r: Vec<(i64, String)> = (vec!["a".to_string(), "b".to_string(), "c".to_string()]).into_iter().enumerate().map(|(i, x)| (i as i64, x)).collect::<Vec<_>>();
    println!("{} {:?}", "[enumerate]:", r);
    return assert_eq!(r.clone(), vec![(0i64, "a".to_string()), (1i64, "b".to_string()), (2i64, "c".to_string())]);
}

pub fn test_take_drop() {
    let mut r: Vec<i64> = take(vec![1i64, 2i64, 3i64, 4i64, 5i64], 3i64);
    println!("{} {:?}", "[take 3]:", r);
    assert_eq!(r.clone(), vec![1i64, 2i64, 3i64]);
    let mut r2: Vec<i64> = drop(vec![1i64, 2i64, 3i64, 4i64, 5i64], 2i64);
    println!("{} {:?}", "[drop 2]:", r2);
    return assert_eq!(r2.clone(), vec![3i64, 4i64, 5i64]);
}

pub fn test_take_while() {
    let mut r: Vec<i64> = (vec![1i64, 2i64, 3i64, 4i64, 1i64, 2i64]).into_iter().take_while(move |&x| { x < 3i64 }).collect::<Vec<_>>();
    println!("{} {:?}", "[take_while <3]:", r);
    return assert_eq!(r.clone(), vec![1i64, 2i64]);
}

pub fn test_chunk() {
    let mut r: Vec<Vec<i64>> = chunk(vec![1i64, 2i64, 3i64, 4i64, 5i64, 6i64, 7i64], 3i64);
    println!("{} {:?}", "[chunk 3]:", r);
    return assert_eq!(r.clone(), vec![vec![1i64, 2i64, 3i64], vec![4i64, 5i64, 6i64], vec![7i64]]);
}

pub fn test_window() {
    let mut r: Vec<Vec<i64>> = window(vec![1i64, 2i64, 3i64, 4i64], 2i64);
    println!("{} {:?}", "[window 2]:", r);
    return assert_eq!(r.clone(), vec![vec![1i64, 2i64], vec![2i64, 3i64], vec![3i64, 4i64]]);
}

pub fn test_intersperse() {
    let mut r: Vec<i64> = intersperse(vec![1i64, 2i64, 3i64, 4i64, 5i64], 0i64);
    println!("{} {:?}", "[intersperse 0]:", r);
    return assert_eq!(r.clone(), vec![1i64, 0i64, 2i64, 0i64, 3i64, 0i64, 4i64, 0i64, 5i64]);
}

pub fn test_scan() {
    let mut r: Vec<i64> = scan(vec![1i64, 2i64, 3i64, 4i64], 0i64, move |acc, x| { LzAdd::__add__(acc, x) });
    println!("{} {:?}", "[scan sum]:", r);
    return assert_eq!(r.clone(), vec![1i64, 3i64, 6i64, 10i64]);
}

pub fn test_chain() {
    let mut r: Vec<i64> = chain(vec![vec![1i64, 2i64], vec![3i64, 4i64], vec![5i64]]);
    println!("{} {:?}", "[chain]:", r);
    return assert_eq!(r.clone(), vec![1i64, 2i64, 3i64, 4i64, 5i64]);
}

pub fn main() {
    test_enumerate();
    test_take_drop();
    test_take_while();
    test_chunk();
    test_window();
    test_intersperse();
    test_scan();
    test_chain();
    println!("{}", "iter.lz: all tests passed!");
}

const __name__: &str = "main";

const __file__: &str = "iter.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\lz_builtins\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

