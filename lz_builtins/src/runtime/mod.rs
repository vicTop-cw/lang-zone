// lz_builtins::runtime — 任何上下文可用
pub mod builtins;
pub mod collections;
pub mod error;
pub mod functional;
pub mod iter;
pub mod lz_bootstrap_builtins;
pub mod ops;
pub mod types;

pub use builtins::*;
pub use collections::*;
pub use error::*;
pub use functional::*;
pub use iter::*;
pub use lz_bootstrap_builtins::*;
pub use ops::*;
pub use types::*;

// ── @parallel 装饰器运行时原语 ──────────────────────────────

/// 并行 map：将 items 分块到多个线程并行执行 f，返回结果（保持顺序）。
/// 元素少于 64 个时退化为串行（线程开销不值得）。纯 std 实现，零外部依赖。
pub fn __lz_par_map<T, U, F>(items: Vec<T>, f: F) -> Vec<U>
where
    T: Send + 'static,
    U: Send + 'static,
    F: Fn(T) -> U + Send + Sync + 'static,
{
    if items.len() < 64 {
        return items.into_iter().map(f).collect();
    }
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let chunk = items.len().div_ceil(n_threads);
    std::thread::scope(|s| {
        let mut handles = Vec::new();
        let mut it = items.into_iter();
        let mut buf: Vec<T> = it.by_ref().take(chunk).collect();
        while !buf.is_empty() {
            let owned = std::mem::take(&mut buf);
            let f_ref = &f;
            handles.push(s.spawn(move || owned.into_iter().map(f_ref).collect::<Vec<U>>()));
            buf = it.by_ref().take(chunk).collect();
        }
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    })
}
