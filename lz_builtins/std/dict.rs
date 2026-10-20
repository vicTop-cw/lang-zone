
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
use std::collections::BTreeMap;
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;
use std::fmt::Debug;
use std::fmt::Display;

use lz_builtins::*;

pub fn get<k: Debug + Clone + PartialEq + Eq + std::cmp::Ord, v: Debug + Clone + PartialEq + Eq + std::cmp::Ord>(d: BTreeMap<k, v>, key: k) -> Option<v> {
    return if (d).contains_key(&key) { Some((d).get(&key).cloned().unwrap()) } else { None };
}

pub fn get_or<k: Debug + Clone + PartialEq + Eq + std::cmp::Ord, v: Debug + Clone + PartialEq + Eq + std::cmp::Ord>(d: BTreeMap<k, v>, key: k, default: v) -> v {
    return if (d).contains_key(&key) { (d).get(&key).cloned().unwrap() } else { default };
}

pub fn has_key<k: Debug + Clone + PartialEq + Eq + std::cmp::Ord, v: Debug + Clone + PartialEq + Eq + std::cmp::Ord>(d: BTreeMap<k, v>, key: k) -> bool {
    return (d).contains_key(&key);
}

pub fn keys<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>) -> Vec<k> {
    let mut result: Vec<k> = Vec::new();
    for (k, _) in (d).into_iter() {
        (result).push(k);
    }
    return result;
}

pub fn values<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>) -> Vec<v> {
    let mut result: Vec<v> = Vec::new();
    for (_, v) in (d).into_iter() {
        (result).push(v);
    }
    return result;
}

pub fn items<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>) -> Vec<(k, v)> {
    let mut result: Vec<(k, v)> = Vec::new();
    for (k, v) in (d).into_iter() {
        (result).push((k, v));
    }
    return result;
}

pub fn dict_len<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>) -> i64 {
    return (keys(d.clone()).len() as i64);
}

pub fn is_empty<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>) -> bool {
    return dict_len(d.clone()) == 0i64;
}

pub fn set<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(mut d: BTreeMap<k, v>, key: k, val: v) -> BTreeMap<k, v> {
    d.insert(key, val);
    return d;
}

pub fn delete<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(mut d: BTreeMap<k, v>, key: k) -> BTreeMap<k, v> {
    (d).remove(key);
    return d;
}

pub fn clear<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(mut d: BTreeMap<k, v>) -> BTreeMap<k, v> {
    d = std::collections::BTreeMap::new();
    return d;
}

pub fn update<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(mut d: BTreeMap<k, v>, other: BTreeMap<k, v>) -> BTreeMap<k, v> {
    for (k, v) in (other).into_iter() {
        d.insert(k, v);
    }
    return d;
}

pub fn set_default<k: Debug + Clone + PartialEq + Eq + std::cmp::Ord, v: Debug + Clone + PartialEq + Eq + std::cmp::Ord>(mut d: BTreeMap<k, v>, key: k, default: v) -> v {
    if !((d).contains_key(&key)) {
        d.insert(key, default);
    }
    return (d).get(&key).cloned().unwrap();
}

pub fn map_values<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord, w: Debug + Clone>(d: BTreeMap<k, v>, mut f: impl Fn(v) -> w + 'static) -> BTreeMap<k, w> {
    let mut result: BTreeMap<k, w> = std::collections::BTreeMap::new();
    for (k, v) in (d).into_iter() {
        result.insert(k, f(v.clone()));
    }
    return result;
}

pub fn map_keys<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord, k2: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>, mut f: impl Fn(k) -> k2 + 'static) -> BTreeMap<k2, v> {
    let mut result: BTreeMap<k2, v> = std::collections::BTreeMap::new();
    for (k, v) in (d).into_iter() {
        result.insert(f(k.clone()), v);
    }
    return result;
}

pub fn filter_dict<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>, mut pred: impl Fn(k, v) -> bool + 'static) -> BTreeMap<k, v> {
    let mut result: BTreeMap<k, v> = std::collections::BTreeMap::new();
    for (k, v) in (d).into_iter() {
        if pred(k.clone(), v.clone()) {
            result.insert(k, v);
        }
    }
    return result;
}

pub fn invert<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>) -> BTreeMap<v, k> {
    let mut result: BTreeMap<v, k> = std::collections::BTreeMap::new();
    for (k, v) in (d).into_iter() {
        result.insert(v, k);
    }
    return result;
}

pub fn merge<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d1: BTreeMap<k, v>, d2: BTreeMap<k, v>) -> BTreeMap<k, v> {
    let mut result: BTreeMap<k, v> = std::collections::BTreeMap::new();
    for (k, v) in (d1).into_iter() {
        result.insert(k, v);
    }
    for (k, v) in (d2).into_iter() {
        result.insert(k, v);
    }
    return result;
}

pub fn dict_sum<k: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, i64>) -> i64 {
    let mut s: i64 = 0i64;
    for (_, v) in (d).into_iter() {
        s += v;
    }
    return s;
}

pub fn dict_all<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>, mut pred: impl Fn(k, v) -> bool + 'static) -> bool {
    for (k, v) in (d).into_iter() {
        if !(pred(k.clone(), v.clone())) {
            return false;
        } else { ()}
    }
    return true;
}

pub fn dict_any<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(d: BTreeMap<k, v>, mut pred: impl Fn(k, v) -> bool + 'static) -> bool {
    for (k, v) in (d).into_iter() {
        if pred(k.clone(), v.clone()) {
            return true;
        } else { ()}
    }
    return false;
}

pub fn from_pairs<k: Debug + Clone + std::cmp::Ord, v: Debug + Clone + std::cmp::Ord>(pairs: Vec<(k, v)>) -> BTreeMap<k, v> {
    let mut result: BTreeMap<k, v> = std::collections::BTreeMap::new();
    for (k, v) in (pairs).into_iter() {
        result.insert(k, v);
    }
    return result;
}

pub fn group_by<a: Debug + Clone, k: Debug + Clone + PartialEq + Eq + std::cmp::Ord>(xs: Vec<a>, mut key_fn: impl Fn(a) -> k + 'static) -> BTreeMap<k, Vec<a>> {
    let mut result: BTreeMap<k, Vec<a>> = std::collections::BTreeMap::new();
    for x in (xs).into_iter() {
        let mut k = key_fn(x.clone());
        if !((result).contains_key(&k)) {
            result.insert(k, vec![x.clone()]);
        } else {
            let mut group = (result).get(&k).cloned().unwrap();
            (group).push(x.clone());
            result.insert(k, group);
        }
    }
    return result;
}

pub fn count_by<a: Debug + Clone, k: Debug + Clone + PartialEq + Eq + std::cmp::Ord>(xs: Vec<a>, mut key_fn: impl Fn(a) -> k + 'static) -> BTreeMap<k, i64> {
    let mut result: BTreeMap<k, i64> = std::collections::BTreeMap::new();
    for x in (xs).into_iter() {
        let mut k = key_fn(x.clone());
        if !((result).contains_key(&k)) {
            result.insert(k, 1i64);
        } else {
            result.insert(k, (result).get(&k).cloned().unwrap() + 1i64);
        }
    }
    return result;
}

pub fn test_basic() {
    let mut d: BTreeMap<String, i64> = std::collections::BTreeMap::new();
    d.insert("a".to_string(), 1i64);
    d.insert("b".to_string(), 2i64);
    d.insert("c".to_string(), 3i64);
    println!("{} {:?}", "[dict] a:", get(d.clone(), "a".to_string()));
    assert_eq!(get(d.clone(), "a".to_string()), Some(1i64));
    assert_eq!(get(d.clone(), "x".to_string()), None);
    return assert_eq!(get_or(d.clone(), "x".to_string(), 99i64), 99i64);
}

pub fn test_keys_values() {
    let mut d = std::collections::BTreeMap::from([("x".to_string(), 1i64), ("y".to_string(), 2i64)]);
    println!("{} {:?}", "[keys]:", keys(d.clone()));
    println!("{} {:?}", "[values]:", values(d.clone()));
    assert_eq!((keys(d.clone()).len() as i64), 2i64);
    return assert_eq!((values(d.clone()).len() as i64), 2i64);
}

pub fn test_update() {
    let mut d1 = std::collections::BTreeMap::from([("a".to_string(), 1i64), ("b".to_string(), 2i64)]);
    let mut d2 = std::collections::BTreeMap::from([("b".to_string(), 99i64), ("c".to_string(), 3i64)]);
    let mut d3 = update(d1.clone(), d2.clone());
    println!("{} {:?}", "[update]:", d3);
    assert_eq!((d3).get(&"a".to_string()).cloned().unwrap(), 1i64);
    assert_eq!((d3).get(&"b".to_string()).cloned().unwrap(), 99i64);
    return assert_eq!((d3).get(&"c".to_string()).cloned().unwrap(), 3i64);
}

pub fn test_filter() {
    let mut d = std::collections::BTreeMap::from([("a".to_string(), 1i64), ("b".to_string(), 2i64), ("c".to_string(), 3i64), ("d".to_string(), 4i64)]);
    let mut d2 = filter_dict(d.clone(), move |k: i64, v| { v > 2i64 });
    println!("{} {:?}", "[filter >2]:", d2);
    assert_eq!((d2).get(&"c".to_string()).cloned().unwrap(), 3i64);
    assert_eq!((d2).get(&"d".to_string()).cloned().unwrap(), 4i64);
    return assert!(!has_key(d2.clone(), "a".to_string()));
}

pub fn test_map_values() {
    let mut d = std::collections::BTreeMap::from([("a".to_string(), 1i64), ("b".to_string(), 2i64)]);
    let mut d2 = map_values(d.clone(), move |v| { v * 10i64 });
    println!("{} {:?}", "[map_values *10]:", d2);
    assert_eq!((d2).get(&"a".to_string()).cloned().unwrap(), 10i64);
    return assert_eq!((d2).get(&"b".to_string()).cloned().unwrap(), 20i64);
}

pub fn test_group_by() {
    let mut words: Vec<String> = vec!["apple".to_string(), "banana".to_string(), "apricot".to_string(), "blueberry".to_string(), "cherry".to_string()];
    let mut grouped = group_by(words.clone(), move |s: String| { (s.len() as i64) });
    println!("{} {:?}", "[group_by len]:", grouped);
    return assert_eq!(((grouped).get(&6i64).cloned().unwrap().len() as i64), 2i64);
}

pub fn test_from_pairs() {
    let mut pairs: Vec<(String, i64)> = vec![("x".to_string(), 1i64), ("y".to_string(), 2i64), ("z".to_string(), 3i64)];
    let mut d = from_pairs(pairs.clone());
    println!("{} {:?}", "[from_pairs]:", d);
    assert_eq!((d).get(&"x".to_string()).cloned().unwrap(), 1i64);
    return assert_eq!((d).get(&"z".to_string()).cloned().unwrap(), 3i64);
}

pub fn test_merge() {
    let mut d1 = std::collections::BTreeMap::from([("a".to_string(), 1i64)]);
    let mut d2 = std::collections::BTreeMap::from([("b".to_string(), 2i64)]);
    let mut d3 = merge(d1.clone(), d2.clone());
    println!("{} {:?}", "[merge]:", d3);
    return assert_eq!((keys(d3.clone()).len() as i64), 2i64);
}

pub fn main() {
    test_basic();
    test_keys_values();
    test_update();
    test_filter();
    test_map_values();
    test_group_by();
    test_from_pairs();
    test_merge();
    println!("{}", "dict.lz: all tests passed!");
}

const __name__: &str = "main";

const __file__: &str = "dict.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\lz_builtins\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

