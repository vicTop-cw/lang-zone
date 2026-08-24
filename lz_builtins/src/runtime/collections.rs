// lz_builtins::collections — List / Dict / Set 类型别名 & 扩展方法

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

// ══════════════════════════════════════════════════════════════
// 类型别名 — 对齐 Python 命名
// ══════════════════════════════════════════════════════════════

pub type List<T> = Vec<T>;
pub type Dict<K, V> = HashMap<K, V>;
pub type Set<T> = HashSet<T>;

// ══════════════════════════════════════════════════════════════
// Vec / List 扩展方法
// ══════════════════════════════════════════════════════════════

pub trait ListExt<T> {
    fn lz_push(&mut self, item: T);
    fn lz_pop(&mut self) -> Option<T>;
    fn lz_len(&self) -> i64;
    fn lz_get(&self, index: i64) -> Option<&T>;
    fn lz_set(&mut self, index: i64, value: T);
    fn lz_contains(&self, item: &T) -> bool
    where
        T: PartialEq;
    fn lz_index(&self, item: &T) -> Option<i64>
    where
        T: PartialEq;
    fn lz_remove(&mut self, index: i64) -> T;
    fn lz_insert(&mut self, index: i64, item: T);
    fn lz_sort(&mut self)
    where
        T: Ord;
    fn lz_reverse(&mut self);
    fn lz_extend(&mut self, other: Vec<T>);
    fn lz_clear(&mut self);
    fn lz_is_empty(&self) -> bool;
    fn lz_first(&self) -> Option<&T>;
    fn lz_last(&self) -> Option<&T>;
    fn lz_slice(&self, start: i64, end: i64) -> Vec<T>
    where
        T: Clone;
}

impl<T> ListExt<T> for Vec<T> {
    fn lz_push(&mut self, item: T) {
        self.push(item);
    }
    fn lz_pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn lz_len(&self) -> i64 {
        self.len() as i64
    }
    fn lz_get(&self, index: i64) -> Option<&T> {
        if index >= 0 {
            self.get(index as usize)
        } else {
            None
        }
    }
    fn lz_set(&mut self, index: i64, value: T) {
        if index >= 0 && (index as usize) < self.len() {
            self[index as usize] = value;
        }
    }
    fn lz_contains(&self, item: &T) -> bool
    where
        T: PartialEq,
    {
        self.contains(item)
    }
    fn lz_index(&self, item: &T) -> Option<i64>
    where
        T: PartialEq,
    {
        self.iter().position(|x| x == item).map(|i| i as i64)
    }
    fn lz_remove(&mut self, index: i64) -> T {
        self.remove(index as usize)
    }
    fn lz_insert(&mut self, index: i64, item: T) {
        self.insert(index as usize, item);
    }
    fn lz_sort(&mut self)
    where
        T: Ord,
    {
        self.sort();
    }
    fn lz_reverse(&mut self) {
        self.reverse();
    }
    fn lz_extend(&mut self, other: Vec<T>) {
        self.extend(other);
    }
    fn lz_clear(&mut self) {
        self.clear();
    }
    fn lz_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn lz_first(&self) -> Option<&T> {
        self.first()
    }
    fn lz_last(&self) -> Option<&T> {
        self.last()
    }
    fn lz_slice(&self, start: i64, end: i64) -> Vec<T>
    where
        T: Clone,
    {
        if start < 0 || end < 0 {
            return vec![];
        }
        let s = start as usize;
        let e = (end as usize).min(self.len());
        if s >= e {
            return vec![];
        }
        self[s..e].to_vec()
    }
}

// ══════════════════════════════════════════════════════════════
// HashMap / Dict 扩展方法
// ══════════════════════════════════════════════════════════════

pub trait DictExt<K, V> {
    fn lz_get(&self, key: &K) -> Option<&V>
    where
        K: Eq + Hash;
    fn lz_set(&mut self, key: K, value: V)
    where
        K: Eq + Hash;
    fn lz_contains(&self, key: &K) -> bool
    where
        K: Eq + Hash;
    fn lz_remove(&mut self, key: &K) -> Option<V>
    where
        K: Eq + Hash;
    fn lz_keys(&self) -> Vec<&K>;
    fn lz_values(&self) -> Vec<&V>;
    fn lz_items(&self) -> Vec<(&K, &V)>;
    fn lz_len(&self) -> i64;
    fn lz_is_empty(&self) -> bool;
    fn lz_clear(&mut self);
    fn lz_update(&mut self, other: HashMap<K, V>)
    where
        K: Eq + Hash;
    fn lz_set_default(&mut self, key: K, default: V) -> &V
    where
        K: Clone + Eq + Hash,
        V: Clone;
}

impl<K: Eq + Hash, V> DictExt<K, V> for HashMap<K, V> {
    fn lz_get(&self, key: &K) -> Option<&V> {
        self.get(key)
    }
    fn lz_set(&mut self, key: K, value: V) {
        self.insert(key, value);
    }
    fn lz_contains(&self, key: &K) -> bool {
        self.contains_key(key)
    }
    fn lz_remove(&mut self, key: &K) -> Option<V> {
        self.remove(key)
    }
    fn lz_keys(&self) -> Vec<&K> {
        self.keys().collect()
    }
    fn lz_values(&self) -> Vec<&V> {
        self.values().collect()
    }
    fn lz_items(&self) -> Vec<(&K, &V)> {
        self.iter().collect()
    }
    fn lz_len(&self) -> i64 {
        self.len() as i64
    }
    fn lz_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn lz_clear(&mut self) {
        self.clear();
    }
    fn lz_update(&mut self, other: HashMap<K, V>) {
        self.extend(other);
    }
    fn lz_set_default(&mut self, key: K, default: V) -> &V
    where
        K: Clone + Eq + Hash,
        V: Clone,
    {
        self.entry(key.clone()).or_insert(default);
        self.get(&key).unwrap()
    }
}

// ══════════════════════════════════════════════════════════════
// HashSet / Set 扩展方法
// ══════════════════════════════════════════════════════════════

pub trait SetExt<T> {
    fn lz_add(&mut self, item: T) -> bool
    where
        T: Eq + Hash;
    fn lz_remove(&mut self, item: &T) -> bool
    where
        T: Eq + Hash;
    fn lz_contains(&self, item: &T) -> bool
    where
        T: Eq + Hash;
    fn lz_len(&self) -> i64;
    fn lz_is_empty(&self) -> bool;
    fn lz_clear(&mut self);
    fn lz_union(&self, other: &HashSet<T>) -> HashSet<T>
    where
        T: Clone + Eq + Hash;
    fn lz_intersection(&self, other: &HashSet<T>) -> HashSet<T>
    where
        T: Clone + Eq + Hash;
}

impl<T: Eq + Hash> SetExt<T> for HashSet<T> {
    fn lz_add(&mut self, item: T) -> bool {
        self.insert(item)
    }
    fn lz_remove(&mut self, item: &T) -> bool {
        self.remove(item)
    }
    fn lz_contains(&self, item: &T) -> bool {
        self.contains(item)
    }
    fn lz_len(&self) -> i64 {
        self.len() as i64
    }
    fn lz_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn lz_clear(&mut self) {
        self.clear();
    }
    fn lz_union(&self, other: &HashSet<T>) -> HashSet<T>
    where
        T: Clone + Eq + Hash,
    {
        self.union(other).cloned().collect()
    }
    fn lz_intersection(&self, other: &HashSet<T>) -> HashSet<T>
    where
        T: Clone + Eq + Hash,
    {
        self.intersection(other).cloned().collect()
    }
}

// ══════════════════════════════════════════════════════════════
// String 扩展方法
// ══════════════════════════════════════════════════════════════

pub trait StringExt {
    fn lz_len(&self) -> i64;
    fn lz_is_empty(&self) -> bool;
    fn lz_contains(&self, substr: &str) -> bool;
    fn lz_starts_with(&self, prefix: &str) -> bool;
    fn lz_ends_with(&self, suffix: &str) -> bool;
    fn lz_split(&self, delimiter: &str) -> Vec<String>;
    fn lz_join(&self, iter: Vec<String>) -> String;
    fn lz_replace(&self, from: &str, to: &str) -> String;
    fn lz_trim(&self) -> String;
    fn lz_to_upper(&self) -> String;
    fn lz_to_lower(&self) -> String;
    fn lz_slice(&self, start: i64, end: i64) -> String;
    fn lz_find(&self, substr: &str) -> Option<i64>;
}

impl StringExt for String {
    fn lz_len(&self) -> i64 {
        self.len() as i64
    }
    fn lz_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn lz_contains(&self, substr: &str) -> bool {
        self.contains(substr)
    }
    fn lz_starts_with(&self, prefix: &str) -> bool {
        self.starts_with(prefix)
    }
    fn lz_ends_with(&self, suffix: &str) -> bool {
        self.ends_with(suffix)
    }
    fn lz_split(&self, delimiter: &str) -> Vec<String> {
        self.split(delimiter).map(|s| s.to_string()).collect()
    }
    fn lz_join(&self, iter: Vec<String>) -> String {
        iter.join(self)
    }
    fn lz_replace(&self, from: &str, to: &str) -> String {
        self.replace(from, to)
    }
    fn lz_trim(&self) -> String {
        self.trim().to_string()
    }
    fn lz_to_upper(&self) -> String {
        self.to_uppercase()
    }
    fn lz_to_lower(&self) -> String {
        self.to_lowercase()
    }
    fn lz_slice(&self, start: i64, end: i64) -> String {
        let s = start.max(0) as usize;
        let e = (end as usize).min(self.len());
        if s >= e {
            return String::new();
        }
        self[s..e].to_string()
    }
    fn lz_find(&self, substr: &str) -> Option<i64> {
        self.find(substr).map(|i| i as i64)
    }
}

impl StringExt for str {
    fn lz_len(&self) -> i64 {
        self.len() as i64
    }
    fn lz_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn lz_contains(&self, substr: &str) -> bool {
        self.contains(substr)
    }
    fn lz_starts_with(&self, prefix: &str) -> bool {
        self.starts_with(prefix)
    }
    fn lz_ends_with(&self, suffix: &str) -> bool {
        self.ends_with(suffix)
    }
    fn lz_split(&self, delimiter: &str) -> Vec<String> {
        self.split(delimiter).map(|s| s.to_string()).collect()
    }
    fn lz_join(&self, iter: Vec<String>) -> String {
        iter.join(self)
    }
    fn lz_replace(&self, from: &str, to: &str) -> String {
        self.replace(from, to)
    }
    fn lz_trim(&self) -> String {
        self.trim().to_string()
    }
    fn lz_to_upper(&self) -> String {
        self.to_uppercase()
    }
    fn lz_to_lower(&self) -> String {
        self.to_lowercase()
    }
    fn lz_slice(&self, start: i64, end: i64) -> String {
        let s = start.max(0) as usize;
        let e = (end as usize).min(self.len());
        if s >= e {
            return String::new();
        }
        self[s..e].to_string()
    }
    fn lz_find(&self, substr: &str) -> Option<i64> {
        self.find(substr).map(|i| i as i64)
    }
}
