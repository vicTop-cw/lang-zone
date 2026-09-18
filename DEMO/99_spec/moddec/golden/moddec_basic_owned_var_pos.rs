
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

pub fn __lz_main() -> i64 {
    // STMT:Let
    let fd: i64 = 7i64;
    // STMT:Expr
    return fd;
}
pub fn main() {
    std::process::exit(__lz_main() as i32);
}

const __name__: &str = "main";

const __file__: &str = "input.lz";

const __package__: &str = "";

const __path__: &str = "";

const __doc__: &str = "";

const __is_macro__: bool = false;

