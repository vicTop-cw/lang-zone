// Lang-Zong SIMD — simd/ops.rs
// 扩展算子：对标 Polars DataFrame + Spark RDD 的核心 API
//
// 六类 30 个算子，通过 SimdOps trait 扩展到所有 SIMD 类型
// 设计原则：不可变、纯函数、返回 Vec 避免 borrow 纠缠

use super::Simd;
use std::collections::HashSet;

// ──────────────── SimdOps trait ────────────────

/// SIMD 扩展算子 trait
/// 对标 Polars `Series` + Spark RDD 的变换/聚合/过滤 API
pub trait SimdOps: Simd {
    // ── A. 过滤与选择 ──

    /// Polars `filter(mask)` + Spark RDD `filter(f)`
    fn filter(&self, mask: &[bool]) -> Vec<f64> {
        (0..self.len()).filter(|&i| mask[i]).map(|i| self.lane(i)).collect()
    }

    /// 过滤+映射（单次遍历）
    fn filter_map(&self, pred: &dyn Fn(f64) -> bool, f: &dyn Fn(f64) -> f64) -> Vec<f64> {
        (0..self.len())
            .map(|i| self.lane(i))
            .filter(|&x| pred(x))
            .map(|x| f(x))
            .collect()
    }

    /// Polars `head(n)` + Spark `take(n)`
    fn head(&self, n: usize) -> Vec<f64> {
        let end = n.min(self.len());
        (0..end).map(|i| self.lane(i)).collect()
    }

    /// Polars `tail(n)`
    fn tail(&self, n: usize) -> Vec<f64> {
        let start = if n >= self.len() { 0 } else { self.len() - n };
        (start..self.len()).map(|i| self.lane(i)).collect()
    }

    /// Python `slice[start:start+len]` / Rust `&[start..start+len]`
    fn slice(&self, start: usize, len: usize) -> Vec<f64> {
        let end = (start + len).min(self.len());
        (start..end).map(|i| self.lane(i)).collect()
    }

    /// Spark RDD `take(indices)` — 按索引集合提取
    fn take_indices(&self, indices: &[usize]) -> Vec<f64> {
        indices.iter().filter_map(|&i| {
            if i < self.len() { Some(self.lane(i)) } else { None }
        }).collect()
    }

    /// 等间隔采样 take_every(2) → 取 0, 2, 4, ...
    fn take_every(&self, n: usize) -> Vec<f64> {
        (0..self.len()).step_by(n).map(|i| self.lane(i)).collect()
    }

    // ── B. 聚合与统计 ──

    /// Spark RDD `fold(init, f)` — 自定义折叠
    fn fold(&self, init: f64, f: &dyn Fn(f64, f64) -> f64) -> f64 {
        let mut acc = init;
        for i in 0..self.len() { acc = f(acc, self.lane(i)); }
        acc
    }

    /// Polars `mean()`
    fn mean(&self) -> f64 {
        if self.is_empty() { return 0.0; }
        self.reduce_sum() / self.len() as f64
    }

    /// Polars `std()` — 总体标准差
    fn std_dev(&self) -> f64 { self.variance().sqrt() }

    /// Polars `var()` — 总体方差
    fn variance(&self) -> f64 {
        if self.len() < 2 { return 0.0; }
        let m = self.mean();
        let ss: f64 = (0..self.len()).map(|i| {
            let d = self.lane(i) - m;
            d * d
        }).sum();
        ss / self.len() as f64
    }

    /// Polars `median()`
    fn median(&self) -> f64 {
        if self.is_empty() { return 0.0; }
        let mut sorted: Vec<f64> = (0..self.len()).map(|i| self.lane(i)).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        }
    }

    /// Polars `quantile(q)` — q ∈ [0.0, 1.0]
    fn quantile(&self, q: f64) -> f64 {
        if self.is_empty() { return 0.0; }
        let q = q.clamp(0.0, 1.0);
        let mut sorted: Vec<f64> = (0..self.len()).map(|i| self.lane(i)).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((sorted.len() - 1) as f64 * q) as usize;
        sorted[idx]
    }

    // ── C. 变换 ──

    /// Polars `sort(ascending=true)` — 返回排序后副本
    fn sorted_asc(&self) -> Vec<f64> {
        let mut v: Vec<f64> = (0..self.len()).map(|i| self.lane(i)).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    }

    fn sorted_desc(&self) -> Vec<f64> {
        let mut v = self.sorted_asc();
        v.reverse();
        v
    }

    /// Polars `argsort()` — 返回排序后的索引
    fn argsort_asc(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.len()).collect();
        indices.sort_by(|&a, &b| self.lane(a).partial_cmp(&self.lane(b)).unwrap());
        indices
    }

    /// Polars `unique()` + Spark `distinct()`
    fn unique(&self) -> Vec<f64> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for i in 0..self.len() {
            // 用 bits 表示 f64 以实现 HashSet（避免 NaN 问题）
            let bits = self.lane(i).to_bits();
            if seen.insert(bits) {
                result.push(self.lane(i));
            }
        }
        result
    }

    /// 唯一值数量
    fn unique_count(&self) -> usize { self.unique().len() }

    /// Polars `clip(min, max)` — 限制范围
    fn clip(&self, min: f64, max: f64) -> Vec<f64> {
        (0..self.len()).map(|i| self.lane(i).clamp(min, max)).collect()
    }

    /// Polars `shift(n)` — lag (n>0) / lead (n<0)
    fn shift(&self, n: isize, fill: f64) -> Vec<f64> {
        let len = self.len();
        let mut result = vec![fill; len];
        for i in 0..len {
            let src = i as isize - n;
            if src >= 0 && (src as usize) < len {
                result[i] = self.lane(src as usize);
            }
        }
        result
    }

    /// Spark RDD `flatMap(f)` — 一对一多展开
    fn flat_map(&self, f: &dyn Fn(f64) -> Vec<f64>) -> Vec<f64> {
        let mut result = Vec::new();
        for i in 0..self.len() {
            result.extend(f(self.lane(i)));
        }
        result
    }

    /// Spark RDD `sortBy(func, ascending)` — 按派生键排序
    fn sort_by_key(&self, key: &dyn Fn(f64) -> f64, ascending: bool) -> Vec<f64> {
        let mut pairs: Vec<(f64, f64)> = (0..self.len())
            .map(|i| { let v = self.lane(i); (key(v), v) })
            .collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if !ascending { pairs.reverse(); }
        pairs.into_iter().map(|(_, v)| v).collect()
    }

    /// Spark RDD `foreach(f)` — 副作用遍历（返回遍历的元素数）
    fn foreach(&self, f: &mut dyn FnMut(f64)) -> usize {
        for i in 0..self.len() { f(self.lane(i)); }
        self.len()
    }

    /// Spark RDD `forall` → true if all elements match predicate
    fn all(&self, pred: &dyn Fn(f64) -> bool) -> bool {
        for i in 0..self.len() { if !pred(self.lane(i)) { return false; } }
        true
    }

    /// Spark RDD `exists` → true if any element matches predicate  
    fn any(&self, pred: &dyn Fn(f64) -> bool) -> bool {
        for i in 0..self.len() { if pred(self.lane(i)) { return true; } }
        false
    }

    // ── D. 累积 ──

    /// Polars `cumsum()`
    fn cumsum(&self) -> Vec<f64> {
        let mut acc = 0.0;
        (0..self.len()).map(|i| { acc += self.lane(i); acc }).collect()
    }

    /// Polars `cumprod()`
    fn cumprod(&self) -> Vec<f64> {
        let mut acc = 1.0;
        (0..self.len()).map(|i| { acc *= self.lane(i); acc }).collect()
    }

    /// 累积最大值
    fn cummax(&self) -> Vec<f64> {
        let mut acc = f64::NEG_INFINITY;
        (0..self.len()).map(|i| { acc = acc.max(self.lane(i)); acc }).collect()
    }

    /// 累积最小值
    fn cummin(&self) -> Vec<f64> {
        let mut acc = f64::INFINITY;
        (0..self.len()).map(|i| { acc = acc.min(self.lane(i)); acc }).collect()
    }

    /// Spark RDD `product()` — 所有元素的乘积
    fn product(&self) -> f64 {
        let mut p = 1.0;
        for i in 0..self.len() { p *= self.lane(i); }
        p
    }

    /// Spark RDD `top(n)` — 最大的 n 个元素（降序）
    fn top(&self, n: usize) -> Vec<f64> {
        let mut v = self.sorted_desc();
        v.truncate(n);
        v
    }

    /// Spark RDD `takeOrdered(n)` → 最小的 n 个元素（升序）
    fn bottom(&self, n: usize) -> Vec<f64> {
        let mut v = self.sorted_asc();
        v.truncate(n);
        v
    }

    /// Spark RDD `countByValue()` — 值频率统计 HashMap
    fn count_by_value(&self) -> std::collections::HashMap<u64, (f64, usize)> {
        let mut map: std::collections::HashMap<u64, (f64, usize)> = std::collections::HashMap::new();
        for i in 0..self.len() {
            let v = self.lane(i);
            let k = v.to_bits();
            map.entry(k).or_insert((v, 0)).1 += 1;
        }
        map
    }

    // ── E. 集合与组合 ──

    /// Spark RDD `zip(other).map(f)` — 逐对二元操作
    fn zip_with(&self, other: &[f64], f: &dyn Fn(f64, f64) -> f64) -> Vec<f64> {
        let n = self.len().min(other.len());
        (0..n).map(|i| f(self.lane(i), other[i])).collect()
    }

    /// Spark RDD `partition(f)` — 按谓词一分为二
    fn partition(&self, pred: &dyn Fn(f64) -> bool) -> (Vec<f64>, Vec<f64>) {
        let mut yes = Vec::new();
        let mut no = Vec::new();
        for i in 0..self.len() {
            if pred(self.lane(i)) { yes.push(self.lane(i)); } else { no.push(self.lane(i)); }
        }
        (yes, no)
    }

    /// Spark RDD `sample(false, fraction)` — 不放回随机采样 n 个
    fn sample(&self, n: usize, seed: u64) -> Vec<f64> {
        if n >= self.len() {
            return (0..self.len()).map(|i| self.lane(i)).collect();
        }
        // Fisher-Yates 索引采样
        let mut indices: Vec<usize> = (0..self.len()).collect();
        let mut rng = SimpleRng::new(seed);
        for i in 0..n {
            let j = i + (rng.next() as usize % (indices.len() - i));
            indices.swap(i, j);
        }
        indices[..n].iter().map(|&i| self.lane(i)).collect()
    }

    // ── G. Spark 集合运算 ──

    /// Spark RDD `intersection(other)` — 交集（保留 self 中的值）
    fn intersection(&self, other: &dyn Simd) -> Vec<f64> {
        let other_set: std::collections::HashSet<u64> =
            (0..other.len()).map(|i| other.lane(i).to_bits()).collect();
        self.unique().into_iter()
            .filter(|&v| other_set.contains(&v.to_bits()))
            .collect()
    }

    /// Spark RDD `union(other)` — 并集（去重）
    fn union(&self, other: &dyn Simd) -> Vec<f64> {
        let mut result = self.unique();
        let self_set: std::collections::HashSet<u64> =
            result.iter().map(|&v| v.to_bits()).collect();
        for i in 0..other.len() {
            let v = other.lane(i);
            if !self_set.contains(&v.to_bits()) {
                result.push(v);
            }
        }
        result
    }

    /// Spark RDD `subtract(other)` — 差集（self 中有，other 中没有）
    fn subtract(&self, other: &dyn Simd) -> Vec<f64> {
        let other_set: std::collections::HashSet<u64> =
            (0..other.len()).map(|i| other.lane(i).to_bits()).collect();
        self.unique().into_iter()
            .filter(|&v| !other_set.contains(&v.to_bits()))
            .collect()
    }

    /// Spark RDD `cartesian(other)` — 笛卡尔积：每对组合应用 f(a,b)
    fn cartesian(&self, other: &dyn Simd, f: &dyn Fn(f64, f64) -> f64) -> Vec<f64> {
        let mut result = Vec::with_capacity(self.len() * other.len());
        for i in 0..self.len() {
            let a = self.lane(i);
            for j in 0..other.len() {
                result.push(f(a, other.lane(j)));
            }
        }
        result
    }

    /// 直方图分桶：等距分 n 个桶，返回 (bucket_start, count)
    fn histogram(&self, n_buckets: usize) -> Vec<(f64, usize)> {
        if self.is_empty() || n_buckets == 0 { return Vec::new(); }
        let min = self.reduce_min();
        let max = self.reduce_max();
        let range = max - min;
        let bucket_width = if range == 0.0 { 1.0 } else { range / n_buckets as f64 };
        let mut buckets = vec![0usize; n_buckets];
        for i in 0..self.len() {
            let idx = ((self.lane(i) - min) / bucket_width) as usize;
            let idx = idx.min(n_buckets - 1);
            buckets[idx] += 1;
        }
        (0..n_buckets).map(|i| (min + i as f64 * bucket_width, buckets[i])).collect()
    }

    // ── F. 类型与空值 ──

    /// Polars `cast(Int64)` — 类型转换
    fn cast_to_i64(&self) -> Vec<i64> {
        (0..self.len()).map(|i| self.lane(i) as i64).collect()
    }

    /// Polars `fill_null(value)` — 替换哨兵空值（NaN 安全：使用 to_bits 比较）
    fn fill_null(&self, value: f64, null_sentinel: f64) -> Vec<f64> {
        let sentinel_bits = null_sentinel.to_bits();
        (0..self.len()).map(|i| {
            let v = self.lane(i);
            if v.to_bits() == sentinel_bits { value } else { v }
        }).collect()
    }
}

// 为所有实现 Simd 的类型自动获得 SimdOps
impl<T: Simd + ?Sized> SimdOps for T {}

// ──────────────── 简易随机数（无外部依赖）───────────────

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(0x9E3779B97F4A7C15) }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(0x5851F42D4C957F2D).wrapping_add(1);
        // splitmix64 finalizer
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simd::{DType, SimdStack};

    fn test_vec() -> SimdStack<8> {
        SimdStack::<8>::from_elements(DType::F32, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])
    }

    // ── A. Filter & Select ──
    #[test] fn test_filter() { let v = test_vec(); assert_eq!(v.filter(&[true,false,true,false,true,false,true,false]), vec![1.0,3.0,5.0,7.0]); }
    #[test] fn test_filter_map() { let v = test_vec(); let r = v.filter_map(&|x| x > 4.0, &|x| x * x); assert_eq!(r, vec![25.0,36.0,49.0,64.0]); }
    #[test] fn test_head() { assert_eq!(test_vec().head(3), vec![1.0,2.0,3.0]); }
    #[test] fn test_tail() { assert_eq!(test_vec().tail(3), vec![6.0,7.0,8.0]); }
    #[test] fn test_slice() { assert_eq!(test_vec().slice(2, 4), vec![3.0,4.0,5.0,6.0]); }
    #[test] fn test_take_indices() { assert_eq!(test_vec().take_indices(&[0,2,4,7]), vec![1.0,3.0,5.0,8.0]); }
    #[test] fn test_take_every() { assert_eq!(test_vec().take_every(3), vec![1.0,4.0,7.0]); }

    // ── B. Agg & Stats ──
    #[test] fn test_fold() { let v = test_vec(); assert_eq!(v.fold(0.0, &|a,b| a+b), 36.0); }
    #[test] fn test_mean() { assert_eq!(test_vec().mean(), 4.5); }
    #[test] fn test_variance() { let v = test_vec(); assert!((v.variance() - 5.25).abs() < 1e-10); }
    #[test] fn test_std_dev() { let v = test_vec(); assert!((v.std_dev() - 2.2912878).abs() < 1e-6); }
    #[test] fn test_median() { assert_eq!(test_vec().median(), 4.5); }
    #[test] fn test_quantile() { assert_eq!(test_vec().quantile(0.0), 1.0); assert_eq!(test_vec().quantile(0.5), 4.0); assert_eq!(test_vec().quantile(1.0), 8.0); }

    // ── C. Transform ──
    #[test] fn test_sorted() { assert_eq!(SimdStack::<4>::from_elements(DType::F32, &[3.0,1.0,4.0,2.0]).sorted_asc(), vec![1.0,2.0,3.0,4.0]); }
    #[test] fn test_argsort() { assert_eq!(SimdStack::<4>::from_elements(DType::F32, &[3.0,1.0,4.0,2.0]).argsort_asc(), vec![1,3,0,2]); }
    #[test] fn test_unique() { let v = SimdStack::<6>::from_elements(DType::F32, &[1.0,2.0,2.0,3.0,1.0,4.0]); let mut u = v.unique(); u.sort_by(|a,b| a.partial_cmp(b).unwrap()); assert_eq!(u, vec![1.0,2.0,3.0,4.0]); }
    #[test] fn test_clip() { assert_eq!(test_vec().clip(3.0, 6.0), vec![3.0,3.0,3.0,4.0,5.0,6.0,6.0,6.0]); }
    #[test] fn test_shift() { let v = SimdStack::<4>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0]); assert_eq!(v.shift(2, 0.0), vec![0.0,0.0,1.0,2.0]); assert_eq!(v.shift(-1, -1.0), vec![2.0,3.0,4.0,-1.0]); }
    #[test] fn test_flat_map() { let v = SimdStack::<3>::from_elements(DType::F32, &[1.0,2.0,3.0]); let r = v.flat_map(&|x| vec![x, x*10.0]); assert_eq!(r, vec![1.0,10.0,2.0,20.0,3.0,30.0]); }

    // ── D. Cumulative ──
    #[test] fn test_cumsum() { assert_eq!(SimdStack::<4>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0]).cumsum(), vec![1.0,3.0,6.0,10.0]); }
    #[test] fn test_cumprod() { assert_eq!(SimdStack::<4>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0]).cumprod(), vec![1.0,2.0,6.0,24.0]); }
    #[test] fn test_cummax() { assert_eq!(SimdStack::<4>::from_elements(DType::F32, &[3.0,1.0,4.0,2.0]).cummax(), vec![3.0,3.0,4.0,4.0]); }
    #[test] fn test_cummin() { assert_eq!(SimdStack::<4>::from_elements(DType::F32, &[3.0,1.0,4.0,2.0]).cummin(), vec![3.0,1.0,1.0,1.0]); }

    // ── E. Set & Combine ──
    #[test] fn test_zip_with() { let v = SimdStack::<4>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0]); let r = v.zip_with(&[10.0,20.0,30.0,40.0], &|a,b| a+b); assert_eq!(r, vec![11.0,22.0,33.0,44.0]); }
    #[test] fn test_partition() { let (evens, odds) = test_vec().partition(&|x| x as i64 % 2 == 0); assert_eq!(evens, vec![2.0,4.0,6.0,8.0]); assert_eq!(odds, vec![1.0,3.0,5.0,7.0]); }
    #[test] fn test_sample() { let r = test_vec().sample(3, 42); assert_eq!(r.len(), 3); assert!(r.iter().all(|x| (1.0..=8.0).contains(x))); }

    // ── F. Type & Null ──
    #[test] fn test_cast_to_i64() { assert_eq!(test_vec().cast_to_i64(), vec![1,2,3,4,5,6,7,8]); }
    #[test] fn test_fill_null() { let v = SimdStack::<4>::from_elements(DType::F32, &[1.0, -999.0, 3.0, -999.0]); let r = v.fill_null(0.0, -999.0); assert_eq!(r, vec![1.0,0.0,3.0,0.0]); }

    // ── G. Spark extras ──
    #[test] fn test_sort_by_key() { let v = SimdStack::<4>::from_elements(DType::F32, &[3.0,1.0,4.0,2.0]); let r = v.sort_by_key(&|x| -x, true); assert_eq!(r, vec![4.0,3.0,2.0,1.0]); }
    #[test] fn test_foreach() { let mut sum = 0.0; test_vec().foreach(&mut |x| sum += x); assert_eq!(sum, 36.0); }
    #[test] fn test_all_any() { let v = test_vec(); assert!(v.all(&|x| x > 0.0)); assert!(!v.all(&|x| x > 4.0)); assert!(v.any(&|x| x > 7.0)); assert!(!v.any(&|x| x < 0.0)); }
    #[test] fn test_product() { let v = SimdStack::<4>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0]); assert_eq!(v.product(), 24.0); }
    #[test] fn test_top_bottom() { let v = test_vec(); assert_eq!(v.top(3), vec![8.0,7.0,6.0]); assert_eq!(v.bottom(3), vec![1.0,2.0,3.0]); }
    #[test] fn test_count_by_value() { let v = SimdStack::<6>::from_elements(DType::F32, &[1.0,2.0,2.0,3.0,2.0,1.0]); let m = v.count_by_value(); assert_eq!(m.len(), 3); }
    #[test] fn test_intersection() { let a = SimdStack::<4>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0]); let b = SimdStack::<4>::from_elements(DType::F32, &[3.0,4.0,5.0,6.0]); let mut r = a.intersection(&b); r.sort_by(|a,b| a.partial_cmp(b).unwrap()); assert_eq!(r, vec![3.0,4.0]); }
    #[test] fn test_union() { let a = SimdStack::<3>::from_elements(DType::F32, &[1.0,2.0,3.0]); let b = SimdStack::<3>::from_elements(DType::F32, &[3.0,4.0,5.0]); let mut r = a.union(&b); r.sort_by(|a,b| a.partial_cmp(b).unwrap()); assert_eq!(r, vec![1.0,2.0,3.0,4.0,5.0]); }
    #[test] fn test_subtract() { let a = SimdStack::<4>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0]); let b = SimdStack::<4>::from_elements(DType::F32, &[3.0,4.0,5.0,6.0]); let mut r = a.subtract(&b); r.sort_by(|a,b| a.partial_cmp(b).unwrap()); assert_eq!(r, vec![1.0,2.0]); }
    #[test] fn test_cartesian() { let a = SimdStack::<3>::from_elements(DType::F32, &[1.0,2.0,3.0]); let b = SimdStack::<3>::from_elements(DType::F32, &[10.0,20.0,30.0]); let r = a.cartesian(&b, &|x,y| x + y); assert_eq!(r.len(), 9); assert_eq!(r[0], 11.0); assert_eq!(r[8], 33.0); }
    #[test] fn test_histogram() { let v = SimdStack::<8>::from_elements(DType::F32, &[1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0]); let h = v.histogram(4); assert_eq!(h.len(), 4); let total: usize = h.iter().map(|(_,c)| c).sum(); assert_eq!(total, 8); }
}
