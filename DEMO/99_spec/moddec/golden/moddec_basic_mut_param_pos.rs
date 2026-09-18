
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

pub fn f(mut n: i64) -> i64 {
    // STMT:Expr
    return n + 1i64;
}

pub fn main() {
    // STMT:Expr
    println!("{:?}", f(41i64));
}

const __name__: &str = "main";

const __file__: &str = "input.lz";

const __package__: &str = "";

const __path__: &str = "";

const __doc__: &str = "";

const __is_macro__: bool = false;

