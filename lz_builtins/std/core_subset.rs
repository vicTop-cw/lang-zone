
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

pub fn lz_all(xs: Vec<bool>) -> bool {
    let mut ok: bool = true;
    for x in (xs).into_iter() {
        if !(x) {
            ok = false;
        }
    }
    return ok;
}

pub fn lz_any(xs: Vec<bool>) -> bool {
    let mut found: bool = false;
    for x in (xs).into_iter() {
        if x {
            found = true;
        }
    }
    return found;
}

pub fn lz_count_if(xs: Vec<i64>, mut pred: impl Fn(i64) -> bool + 'static) -> i64 {
    let mut c: i64 = 0i64;
    for x in (xs).into_iter() {
        if pred(x) {
            c += 1i64;
        }
    }
    return c;
}

pub fn lz_sum_ints(xs: Vec<i64>) -> i64 {
    let mut s: i64 = 0i64;
    for x in (xs).into_iter() {
        s += x;
    }
    return s;
}

pub fn lz_join_words(xs: Vec<String>, sep: String) -> String {
    let mut out: String = "".to_string();
    let mut first: bool = true;
    for w in (xs).into_iter() {
        if !(first) {
            out = out.to_string() + &sep[..];
        }
        out = out.to_string() + &w[..];
        first = false;
    }
    return out;
}

pub fn lz_ends_with(s: String, suffix: String) -> bool {
    let mut n: i64 = (s.len() as i64);
    let mut m: i64 = (suffix.len() as i64);
    let mut ok: bool = n >= m;
    let mut i: i64 = 0i64;
    while ok && i < m {
        if ({let __cs: Vec<char> = (s).chars().collect(); let __i = ((n - m + i) as usize); __cs[__i].to_string()}).to_string() != ({let __cs: Vec<char> = (suffix).chars().collect(); let __i = ((i) as usize); __cs[__i].to_string()}).to_string() {
            ok = false;
        }
        i += 1i64;
    }
    return ok;
}

pub fn lz_abs(x: i64) -> i64 {
    return if x < 0i64 { -x } else { x };
}

pub fn lz_clamp(x: i64, lo: i64, hi: i64) -> i64 {
    if x < lo {
        return lo;
    } else { ()}
    if x > hi {
        return hi;
    } else { ()}
    return x;
}

pub fn is_pos(x: i64) -> bool {
    return x > 0i64;
}

pub fn main() {
    println!("{:?}", lz_all(vec![true, true, true]));
    println!("{:?}", lz_all(vec![true, false]));
    println!("{:?}", lz_any(vec![false, false]));
    println!("{:?}", lz_any(vec![false, true]));
    println!("{:?}", lz_count_if(vec![-1i64, 2i64, 3i64, -4i64], (is_pos as fn(i64) -> bool)));
    println!("{:?}", lz_sum_ints(vec![1i64, 2i64, 3i64, 4i64]));
    println!("{:?}", lz_sum_ints(Vec::<i64>::new()));
    println!("{}", lz_join_words(vec!["a".to_string(), "b".to_string(), "c".to_string()], "-".to_string()));
    println!("{}", lz_join_words(Vec::<String>::new(), "-".to_string()));
    println!("{:?}", lz_ends_with("hello".to_string(), "lo".to_string()));
    println!("{:?}", lz_ends_with("hello".to_string(), "x".to_string()));
    println!("{:?}", lz_abs(-7i64));
    println!("{:?}", lz_abs(7i64));
    println!("{:?}", lz_clamp(100i64, 0i64, 10i64));
    println!("{:?}", lz_clamp(-5i64, 0i64, 10i64));
    println!("{:?}", lz_clamp(5i64, 0i64, 10i64));
    println!("{}", "core_subset.lz: self-test passed!");
}

const __name__: &str = "main";

const __file__: &str = "core_subset.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\lz_builtins\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

