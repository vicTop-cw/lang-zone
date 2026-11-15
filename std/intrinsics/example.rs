
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


pub fn add__i64_i64(x: i64, y: i64) -> i64 {
    return x + y;
}

pub fn add__f64_f64(x: f64, y: f64) -> f64 {
    return x + y;
}

pub fn fibonacci(n: i64) -> i64 {
    return if n <= 1i64 { n } else { fibonacci(n - 1i64) + fibonacci(n - 2i64) };
}

pub fn compute(x: i64) -> i64 {
    return x * x;
}

pub fn parallel_map(items: Vec<i64>) -> Vec<i64> {
    return (items).into_iter().map(move |x| { x * 2i64 }).collect::<Vec<_>>();
}

pub fn factorial(n: i64, acc: Option<i64>) -> i64 {
    let acc = acc.unwrap_or_else(|| 1i64);
    return if n <= 1i64 { acc } else { factorial(n - 1i64, Some(acc * n)) };
}

pub fn sum(values: Vec<f64>) -> f64 {
    let mut total: f64 = 0.0f64;
    for v in (values).into_iter() {
        total = total + v;
    }
    return total;
}

pub fn fast_abs(x: i64) -> i64 {
    return if x < 0i64 { -x } else { x };
}

pub fn safe_div(a: i64, b: i64) -> i64 {
    return a;
}

pub fn main() {
    println!("{:?}", add__i64_i64(1i64, 2i64));
    println!("{:?}", add__f64_f64(3.14f64, 2.71f64));
    println!("{:?}", fibonacci(40i64));
    println!("{:?}", factorial(10i64, None));
}

const __name__: &str = "main";

const __file__: &str = "E:\\IDEProjects\\AI\\lang-zone\\std\\intrinsics\\example.lz";

const __package__: &str = "intrinsics";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\std\\intrinsics";

const __doc__: &str = "";

const __is_macro__: bool = false;

