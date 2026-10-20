
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

pub fn find<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> Option<a> {
    for x in (xs).into_iter() {
        if pred(x.clone()) {
            return Some(x.clone());
        } else { ()}
    }
    return None;
}

pub fn find_index<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> Option<i64> {
    let mut i: i64 = 0i64;
    for x in (xs).into_iter() {
        if pred(x.clone()) {
            return Some(i.clone());
        } else { ()}
        i += 1i64;
    }
    return None;
}

pub fn find_last<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> Option<a> {
    let mut result = None;
    for x in (xs).into_iter() {
        if pred(x.clone()) {
            result = Some(x.clone());
        }
    }
    return result;
}

pub fn contains<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>, x: a) -> bool {
    for y in (xs).into_iter() {
        if y == x {
            return true;
        } else { ()}
    }
    return false;
}

pub fn count<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>, x: a) -> i64 {
    let mut c: i64 = 0i64;
    for y in (xs).into_iter() {
        if y == x {
            c += 1i64;
        }
    }
    return c;
}

pub fn count_pred<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> i64 {
    let mut c: i64 = 0i64;
    for x in (xs).into_iter() {
        if pred(x.clone()) {
            c += 1i64;
        }
    }
    return c;
}

pub fn all<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> bool {
    for x in (xs).into_iter() {
        if !(pred(x.clone())) {
            return false;
        } else { ()}
    }
    return true;
}

pub fn any<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> bool {
    for x in (xs).into_iter() {
        if pred(x.clone()) {
            return true;
        } else { ()}
    }
    return false;
}

pub fn lz_map<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, mut f: impl Fn(a) -> b + 'static) -> Vec<b> {
    let mut result: Vec<b> = Vec::new();
    for x in (xs).into_iter() {
        (result).push(f(x.clone()));
    }
    return result;
}

pub fn lz_filter<a: Debug + Clone>(xs: Vec<a>, mut pred: impl Fn(a) -> bool + 'static) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    for x in (xs).into_iter() {
        if pred(x.clone()) { let _ = (result).push(x.clone()); } else { () }
    }
    return result;
}

pub fn flat_map<a: Debug + Clone, b: Debug + Clone>(xs: Vec<a>, mut f: impl Fn(a) -> Vec<b> + 'static) -> Vec<b> {
    let mut result: Vec<b> = Vec::new();
    for x in (xs).into_iter() {
        let mut ys = f(x.clone());
        for y in (ys).into_iter() {
            (result).push(y);
        }
    }
    return result;
}

pub fn append<a: Debug + Clone>(mut xs: Vec<a>, x: a) -> Vec<a> {
    (xs).push(x);
    return xs;
}

pub fn prepend<a: Debug + Clone>(xs: Vec<a>, x: a) -> Vec<a> {
    let mut result = vec![x.clone()];
    for y in (xs).into_iter() {
        (result).push(y);
    }
    return result;
}

pub fn insert_at<a: Debug + Clone>(mut xs: Vec<a>, i: i64, x: a) -> Vec<a> {
    if i <= 0i64 {
        return prepend(xs.clone(), x.clone());
    } else { ()}
    if i >= (xs.len() as i64) {
        (xs).push(x.clone());
        return xs;
    } else { ()}
    let mut result: Vec<a> = Vec::new();
    for j in (0i64..i) {
        (result).push(xs[j as usize].clone());
    }
    (result).push(x.clone());
    for j in (i..(xs.len() as i64)) {
        (result).push(xs[j as usize].clone());
    }
    return result;
}

pub fn remove_at<a: Debug + Clone>(xs: Vec<a>, i: i64) -> Vec<a> {
    if i < 0i64 || i >= (xs.len() as i64) {
        return xs;
    } else { ()}
    let mut result: Vec<a> = Vec::new();
    for j in (0i64..(xs.len() as i64)) {
        if j != i { let _ = (result).push(xs[j as usize].clone()); } else { () }
    }
    return result;
}

pub fn remove_first<a: Debug + Clone + PartialEq + Eq>(xs: Vec<a>, x: a) -> Vec<a> {
    let mut idx: Option<i64> = None;
    let mut i: i64 = 0i64;
    for y in (xs).into_iter() {
        if y == x && idx.is_none() {
            idx = Some(i.clone());
        }
        i += 1i64;
    }
    if idx.is_none() {
        return xs;
    } else { ()}
    return remove_at(xs.clone(), idx.unwrap());
}

pub fn update_at<a: Debug + Clone>(xs: Vec<a>, i: i64, x: a) -> Vec<a> {
    if i < 0i64 || i >= (xs.len() as i64) {
        return xs;
    } else { ()}
    let mut result: Vec<a> = Vec::new();
    for j in (0i64..(xs.len() as i64)) {
        if j == i { (result).push(x) } else { (result).push(xs[j as usize].clone()) }
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
    let mut i: i64 = 0i64;
    while i < (xs.len() as i64) && pred(xs[i as usize].clone()) {
        i += 1i64;
    }
    for j in (i..(xs.len() as i64)) {
        (result).push(xs[j as usize].clone());
    }
    return result;
}

pub fn sorted_by<a: Debug + Clone, b: Ord + Debug + Clone + PartialEq + Eq>(xs: Vec<a>, mut key_fn: impl Fn(a) -> b + 'static) -> Vec<a> {
    let mut n: i64 = (xs.len() as i64);
    let mut result = xs;
    let mut i: i64 = 0i64;
    while i < n - 1i64 {
        let mut min_j: i64 = i;
        let mut j: i64 = i + 1i64;
        while j < n {
            if key_fn(result[j as usize].clone()) < key_fn(result[min_j as usize].clone()) {
                min_j = j;
            }
            j += 1i64;
        }
        if min_j != i {
            let mut tmp = result[i as usize].clone();
            result[i as usize] = result[min_j as usize].clone();
            result[min_j as usize] = tmp;
        }
        i += 1i64;
    }
    return result;
}

pub fn concat<a: Debug + Clone>(xss: Vec<Vec<a>>) -> Vec<a> {
    let mut result: Vec<a> = Vec::new();
    for xs in (xss).into_iter() {
        for x in (xs).into_iter() {
            (result).push(x);
        }
    }
    return result;
}

pub fn sum(xs: Vec<i64>) -> i64 {
    let mut s: i64 = 0i64;
    for x in (xs).into_iter() {
        s += x;
    }
    return s;
}

pub fn product(xs: Vec<i64>) -> i64 {
    if (xs.len() as i64) == 0i64 {
        return 1i64;
    } else { ()}
    let mut p: i64 = 1i64;
    for x in (xs).into_iter() {
        p *= x;
    }
    return p;
}

pub fn join_str(parts: Vec<String>, sep: String) -> String {
    if (parts.len() as i64) == 0i64 {
        return "".to_string();
    } else { ()}
    let mut result: String = parts[0i64 as usize];
    for i in (1i64..(parts.len() as i64)) {
        result = result.to_string() + &sep[..] + &parts[i as usize];
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

pub fn unique_by<a: Debug + Clone, b: Debug + Clone + PartialEq + Eq>(xs: Vec<a>, mut key_fn: impl Fn(a) -> b + 'static) -> Vec<a> {
    let mut seen_keys: Vec<b> = Vec::new();
    let mut result: Vec<a> = Vec::new();
    for x in (xs).into_iter() {
        let mut k = key_fn(x.clone());
        let mut found: bool = false;
        for s in (seen_keys).into_iter() {
            if s == k {
                found = true;
            }
        }
        if !(found) {
            (seen_keys).push(k.clone());
            (result).push(x.clone())
        } else { ()}
    }
    return result;
}

pub fn test_find() {
    let mut r: Option<i64> = find(vec![1i64, 3i64, 5i64, 7i64], move |x| { x > 4i64 });
    println!("{} {:?}", "[find] >4:", r);
    assert_eq!(r.clone(), Some(5i64));
    return assert_eq!(find(Vec::<i64>::new(), move |x| { x > 0i64 }), None);
}

pub fn test_count() {
    println!("{} {:?}", "[count] [1,2,2,3,2] of 2:", count(vec![1i64, 2i64, 2i64, 3i64, 2i64], 2i64));
    return assert_eq!(count(vec![1i64, 2i64, 2i64, 3i64, 2i64], 2i64), 3i64);
}

pub fn test_take_drop() {
    println!("{} {:?} {:?}", "[take/drop]:", take(vec![1i64, 2i64, 3i64, 4i64, 5i64], 3i64), drop(vec![1i64, 2i64, 3i64, 4i64, 5i64], 2i64));
    assert_eq!(take(vec![1i64, 2i64, 3i64, 4i64, 5i64], 3i64), vec![1i64, 2i64, 3i64]);
    return assert_eq!(drop(vec![1i64, 2i64, 3i64, 4i64, 5i64], 2i64), vec![3i64, 4i64, 5i64]);
}

pub fn test_insert_remove() {
    let mut xs: Vec<i64> = vec![1i64, 2i64, 3i64];
    let mut xs2: Vec<i64> = insert_at(xs.clone(), 1i64, 99i64);
    println!("{} {:?}", "[insert_at]:", xs2);
    assert_eq!(xs2.clone(), vec![1i64, 99i64, 2i64, 3i64]);
    let mut xs3: Vec<i64> = remove_at(xs2.clone(), 1i64);
    println!("{} {:?}", "[remove_at]:", xs3);
    return assert_eq!(xs3.clone(), vec![1i64, 2i64, 3i64]);
}

pub fn test_sorted_by() {
    let mut r: Vec<String> = sorted_by(vec!["banana".to_string(), "apple".to_string(), "cherry".to_string()], move |s: String| { (s.len() as i64) });
    println!("{} {:?}", "[sorted_by len]:", r);
    return assert_eq!(r.clone(), vec!["apple".to_string(), "banana".to_string(), "cherry".to_string()]);
}

pub fn test_dedup() {
    let mut r: Vec<i64> = dedup(vec![1i64, 1i64, 1i64, 2i64, 2i64, 3i64, 3i64, 3i64]);
    println!("{} {:?}", "[dedup]:", r);
    return assert_eq!(r.clone(), vec![1i64, 2i64, 3i64]);
}

pub fn test_window() {
    let mut r: Vec<Vec<i64>> = window(vec![1i64, 2i64, 3i64, 4i64, 5i64], 3i64);
    println!("{} {:?}", "[window 3]:", r);
    return assert_eq!(r.clone(), vec![vec![1i64, 2i64, 3i64], vec![2i64, 3i64, 4i64], vec![3i64, 4i64, 5i64]]);
}

pub fn test_flat_map() {
    let mut r: Vec<i64> = flat_map(vec![1i64, 2i64, 3i64], move |x: i64| { vec![x, x * 10i64] });
    println!("{} {:?}", "[flat_map]:", r);
    return assert_eq!(r.clone(), vec![1i64, 10i64, 2i64, 20i64, 3i64, 30i64]);
}

pub fn main() {
    test_find();
    test_count();
    test_take_drop();
    test_insert_remove();
    test_sorted_by();
    test_dedup();
    test_window();
    test_flat_map();
    println!("{}", "list.lz: all tests passed!");
}

const __name__: &str = "main";

const __file__: &str = "list.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\lz_builtins\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

