// Lang-Zong SIMD — simd/bench.rs
// SIMD vs 普通代码基准测试
//
// 使用 std::time::Instant 精确计时，对比：
//   SimdStack::add/mul/reduce/map/filter/sort
//   与等效 `for` 循环 / Vec 操作
//
// 运行: cargo test simd::bench -- --nocapture

use std::time::Instant;

/// 基准测试运行器
struct Bench {
    name: &'static str,
    simd_ns: u128,
    plain_ns: u128,
}

impl Bench {
    fn speedup(&self) -> f64 {
        self.plain_ns as f64 / self.simd_ns as f64
    }
}

fn run_bench(label: &'static str, iterations: u64, simd_fn: impl Fn(), plain_fn: impl Fn()) -> Bench {
    // Warmup
    for _ in 0..100 { simd_fn(); plain_fn(); }

    // SIMD timing
    let t0 = Instant::now();
    for _ in 0..iterations { simd_fn(); }
    let simd_ns = t0.elapsed().as_nanos() / iterations as u128;

    // Plain timing
    let t0 = Instant::now();
    for _ in 0..iterations { plain_fn(); }
    let plain_ns = t0.elapsed().as_nanos() / iterations as u128;

    Bench { name: label, simd_ns, plain_ns }
}

// ──────────────── 测试用数据 ────────────────

const N: usize = 256;  // AVX2: 8×f32 per register → 32 regs needed
const ITERS: u64 = 10_000;

use crate::simd::{DType, Simd, SimdStack, SimdOps};

fn make_simd() -> SimdStack<N> {
    let mut elements = [0.0f64; N];
    for i in 0..N { elements[i] = i as f64; }
    SimdStack::<N>::from_elements(DType::F32, &elements)
}

fn make_vec() -> Vec<f64> {
    (0..N).map(|i| i as f64).collect()
}

// ──────────────── 基准测试 ────────────────

#[test]
fn bench_element_wise_add() {
    let a = make_simd();
    let b = make_simd();
    let va = make_vec();
    let vb = make_vec();

    let r = run_bench("add", ITERS,
        || { let _ = a.add(&b); },
        || {
            let mut result = vec![0.0; N];
            for i in 0..N { result[i] = va[i] + vb[i]; }
            let _ = result;
        },
    );
    println!("[add]        SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_reduce_sum() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("reduce_sum", ITERS,
        || { let _ = v.reduce_add(); },
        || { let _: f64 = vv.iter().sum(); },
    );
    println!("[reduce_sum] SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_map() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("map", ITERS,
        || { let _ = v.map(&|x| x * x + 1.0); },
        || { let _: Vec<f64> = vv.iter().map(|&x| x * x + 1.0).collect(); },
    );
    println!("[map]        SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_filter() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("filter", ITERS,
        || {
            let mask: Vec<bool> = (0..N).map(|i| i % 2 == 0).collect();
            let _ = v.filter(&mask);
        },
        || {
            let mut result = Vec::new();
            for i in 0..N { if i % 2 == 0 { result.push(vv[i]); } }
            let _ = result;
        },
    );
    println!("[filter]     SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_sort() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("sort", ITERS / 10,
        || { let _ = v.sorted_asc(); },
        || { let mut c = vv.clone(); c.sort_by(|a,b| a.partial_cmp(b).unwrap()); let _ = c; },
    );
    println!("[sort]       SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_cumsum() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("cumsum", ITERS,
        || { let _ = v.cumsum(); },
        || {
            let mut acc = 0.0;
            let mut result = Vec::with_capacity(N);
            for &x in &vv { acc += x; result.push(acc); }
            let _ = result;
        },
    );
    println!("[cumsum]     SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_fold() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("fold", ITERS,
        || { let _ = v.fold(0.0, &|a,b| a + b * b); },
        || { let _: f64 = vv.iter().fold(0.0, |a,&b| a + b * b); },
    );
    println!("[fold]       SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_mean_std() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("mean+std", ITERS,
        || { let _ = (v.mean(), v.std_dev()); },
        || {
            let m: f64 = vv.iter().sum::<f64>() / N as f64;
            let ss: f64 = vv.iter().map(|&x| { let d = x - m; d * d }).sum();
            let _ = (m, (ss / N as f64).sqrt());
        },
    );
    println!("[mean+std]   SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_clip() {
    let v = make_simd();
    let vv = make_vec();

    let r = run_bench("clip", ITERS,
        || { let _ = v.clip(50.0, 200.0); },
        || { let _: Vec<f64> = vv.iter().map(|&x| x.clamp(50.0, 200.0)).collect(); },
    );
    println!("[clip]       SIMD: {:>8}ns | plain: {:>8}ns | {:.2}x", r.simd_ns, r.plain_ns, r.speedup());
}

#[test]
fn bench_summary() {
    println!("\n═══ SIMD vs Plain Rust — Benchmark Summary ({} elements, {} iters) ═══", N, ITERS);
    bench_element_wise_add();
    bench_reduce_sum();
    bench_map();
    bench_filter();
    bench_sort();
    bench_cumsum();
    bench_fold();
    bench_mean_std();
    bench_clip();
    println!("═══ End Summary ═══\n");
}
