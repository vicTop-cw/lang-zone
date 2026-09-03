// lz_builtins::functional — 函数式编程工具库
// 零外部依赖，纯 Rust std。

use std::collections::HashMap;
use std::hash::Hash;

// ══════════════════════════════════════════════════════════════
// Fold — 左折叠
// ══════════════════════════════════════════════════════════════

pub fn fold<B, T, F>(iter: impl IntoIterator<Item = T>, init: B, f: F) -> B
where
    F: FnMut(B, T) -> B,
{
    iter.into_iter().fold(init, f)
}

pub fn fold1<T, F>(iter: impl IntoIterator<Item = T>, mut f: F) -> Option<T>
where
    F: FnMut(T, T) -> T,
{
    let mut iter = iter.into_iter();
    let first = iter.next()?;
    Some(iter.fold(first, f))
}

pub fn reduce<T, F>(iter: impl IntoIterator<Item = T>, f: F) -> Option<T>
where
    F: FnMut(T, T) -> T,
{
    fold1(iter, f)
}

// ══════════════════════════════════════════════════════════════
// Partition — 分区
// ══════════════════════════════════════════════════════════════

pub fn partition<I: IntoIterator, P>(iter: I, pred: P) -> (Vec<I::Item>, Vec<I::Item>)
where
    P: FnMut(&I::Item) -> bool,
{
    iter.into_iter().partition(pred)
}

pub fn group_by<I, K>(iter: I, key_fn: impl Fn(&I::Item) -> K) -> HashMap<K, Vec<I::Item>>
where
    I: IntoIterator,
    K: Eq + Hash,
    I::Item: Clone,
{
    let mut map = HashMap::new();
    for item in iter {
        let key = key_fn(&item);
        map.entry(key).or_insert_with(Vec::new).push(item);
    }
    map
}

// ══════════════════════════════════════════════════════════════
// Composition — 函数组合
// ══════════════════════════════════════════════════════════════

pub fn compose<A, B, C, F, G>(f: F, g: G) -> impl Fn(A) -> C
where
    F: Fn(B) -> C,
    G: Fn(A) -> B,
{
    move |x| f(g(x))
}

pub fn pipe<A, B>(value: A, f: impl Fn(A) -> B) -> B { f(value) }
pub fn pipe2<A, B, C>(value: A, f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> C { g(f(value)) }
pub fn pipe3<A, B, C, D>(value: A, f: impl Fn(A) -> B, g: impl Fn(B) -> C, h: impl Fn(C) -> D) -> D {
    h(g(f(value)))
}

// ══════════════════════════════════════════════════════════════
// Unique / Distinct — 去重
// ══════════════════════════════════════════════════════════════

pub fn unique<I: IntoIterator>(iter: I) -> Vec<I::Item>
where
    I::Item: Eq + Hash + Clone,
{
    let mut seen = std::collections::HashSet::new();
    iter.into_iter().filter(|item| seen.insert(item.clone())).collect()
}

pub fn unique_by<I, K>(iter: I, key_fn: impl Fn(&I::Item) -> K) -> Vec<I::Item>
where
    I: IntoIterator,
    K: Eq + Hash,
    I::Item: Clone,
{
    let mut seen = std::collections::HashSet::new();
    iter.into_iter().filter(|item| seen.insert(key_fn(item))).collect()
}

// ══════════════════════════════════════════════════════════════
// Chunk — 分块
// ══════════════════════════════════════════════════════════════

pub fn chunk<I: IntoIterator>(iter: I, n: usize) -> Vec<Vec<I::Item>> {
    let mut chunks = Vec::new();
    let mut current = Vec::with_capacity(n);
    for item in iter {
        current.push(item);
        if current.len() == n {
            chunks.push(std::mem::take(&mut current));
            current = Vec::with_capacity(n);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

// ══════════════════════════════════════════════════════════════
// Sum / Product
// ══════════════════════════════════════════════════════════════

pub fn sum_i64(iter: impl IntoIterator<Item = i64>) -> i64 { iter.into_iter().sum() }
pub fn product_i64(iter: impl IntoIterator<Item = i64>) -> i64 { iter.into_iter().product() }
pub fn sum_f64(iter: impl IntoIterator<Item = f64>) -> f64 { iter.into_iter().sum() }
pub fn product_f64(iter: impl IntoIterator<Item = f64>) -> f64 { iter.into_iter().product() }

// ══════════════════════════════════════════════════════════════
// find / position / count
// ══════════════════════════════════════════════════════════════

pub fn find<I: IntoIterator>(iter: I, mut pred: impl FnMut(&I::Item) -> bool) -> Option<I::Item>
where I::Item: Sized { iter.into_iter().find(|item| pred(item)) }

pub fn find_map<I: IntoIterator, B>(iter: I, f: impl FnMut(I::Item) -> Option<B>) -> Option<B>
where I: IntoIterator { iter.into_iter().find_map(f) }

pub fn position<I: IntoIterator>(iter: I, mut pred: impl FnMut(&I::Item) -> bool) -> Option<usize>
where I::Item: Sized { iter.into_iter().position(|item| pred(&item)) }

pub fn count<I: IntoIterator>(iter: I) -> usize { iter.into_iter().count() }
pub fn nth<I: IntoIterator>(iter: I, n: usize) -> Option<I::Item> { iter.into_iter().nth(n) }
pub fn last<I: IntoIterator>(iter: I) -> Option<I::Item> { iter.into_iter().last() }

// ══════════════════════════════════════════════════════════════
// Collect
// ══════════════════════════════════════════════════════════════

pub fn to_vec<I: IntoIterator>(iter: I) -> Vec<I::Item> { iter.into_iter().collect() }

pub fn to_hashmap<K, V, I: IntoIterator<Item = (K, V)>>(iter: I) -> HashMap<K, V>
where K: Eq + Hash { iter.into_iter().collect() }

// ══════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold() { assert_eq!(fold(vec![1,2,3,4,5], 0, |a,x| a+x), 15); }

    #[test]
    fn test_fold1() { assert_eq!(fold1(vec![1,2,3,4,5], |a,x| a+x), Some(15)); }

    #[test]
    fn test_fold1_empty() { assert_eq!(fold1(Vec::<i32>::new(), |a,x| a+x), None); }

    #[test]
    fn test_reduce() { assert_eq!(reduce(vec![1,2,3,4], |a,b| a+b), Some(10)); }

    #[test]
    fn test_partition() {
        let (even, odd) = partition(0..10, |&x| x % 2 == 0);
        assert_eq!(even, vec![0,2,4,6,8]);
        assert_eq!(odd, vec![1,3,5,7,9]);
    }

    #[test]
    fn test_compose() {
        let h = compose(|x: i32| x*2, |x: i32| x+3);
        assert_eq!(h(5), 16);
    }

    #[test]
    fn test_pipe() { assert_eq!(pipe(5, |x| x*2), 10); }

    #[test]
    fn test_unique() { assert_eq!(unique(vec![1,2,2,3,1,4]), vec![1,2,3,4]); }

    #[test]
    fn test_chunk() {
        assert_eq!(chunk(vec![1,2,3,4,5,6,7], 3), vec![vec![1,2,3], vec![4,5,6], vec![7]]);
    }

    #[test]
    fn test_sum() { assert_eq!(sum_i64(vec![1,2,3,4,5]), 15); }
}
