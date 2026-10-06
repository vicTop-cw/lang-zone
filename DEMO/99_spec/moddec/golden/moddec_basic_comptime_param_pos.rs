
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

pub fn f(n: i64) -> i64 {
    // STMT:Expr
    return n + 1i64;
}

pub fn main() {
    // STMT:Expr
    println!("{:?}", f__lzspec_b29bbf82ee19e898());
}

const __name__: &str = "main";

const __file__: &str = "input.lz";

const __package__: &str = "";

const __path__: &str = "";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn f__lzspec_b29bbf82ee19e898() -> i64 {
    // STMT:Expr
    return 41i64 + 1i64;
}

