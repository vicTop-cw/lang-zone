
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

use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct MemBudget {
    pub current: Cell<i64>,
    pub max: i64,
}

impl MemBudget {
    pub fn new(max: i64) -> MemBudget {
        let mut current = Cell::new(0i64);
        return MemBudget { current: (current).clone(), max: max };
    }

    fn remaining(&self) -> i64 {
        return self.max.clone() - self.current.get();
    }

    fn used(&self) -> i64 {
        return self.current.get();
    }

    fn alloc(&mut self, n: i64) -> Result<(), String> {
        let mut old = self.current.get();
        let mut new_current: i64 = old + n;
        return if new_current > self.max.clone() { Err("exceeded memory budget".to_string())} else {
            self.current.set(new_current);
            Ok(())
        };
    }

    fn dealloc(&mut self, n: i64) {
        let mut old = self.current.get();
        let mut new: i64 = old - n;
        return if new < 0i64 { self.current.set(0i64) } else { self.current.set(new) };
    }

    fn tryDealloc(&mut self, n: i64) -> Result<(), String> {
        let mut old = self.current.get();
        return if n > old { Err("cannot dealloc more than allocated".to_string())} else {
            self.current.set(old - n);
            Ok(())
        };
    }

}

const __name__: &str = "main";

const __file__: &str = "E:\\IDEProjects\\AI\\lang-zone\\std\\memguard.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn main() {
    // auto-generated: LZ module has no main entry point
}
