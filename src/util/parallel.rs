// Lang-Zong 编译器 — util/parallel.rs
// 并行基础原语：线程池 + 隔离临时目录
//
// 对标 Rust rayon / thread::scope 的核心模式
// 零上层依赖，仅 std::thread + std::sync

use std::sync::{Arc, Mutex, mpsc};
use std::thread;

// ──────────────── 线程池 ────────────────

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<thread::JoinHandle<()>>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        let actual = size.max(1);
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(actual);
        for id in 0..actual {
            let rx = Arc::clone(&receiver);
            let handle = thread::Builder::new()
                .name(format!("lz-pool-{}", id))
                .spawn(move || loop {
                    let job = rx.lock().unwrap().recv();
                    match job { Ok(job) => job(), Err(_) => break }
                })
                .expect("ThreadPool: spawn failed");
            workers.push(handle);
        }
        Self { workers, sender: Some(sender) }
    }

    pub fn execute<F>(&self, f: F) where F: FnOnce() + Send + 'static {
        if let Some(ref s) = self.sender { let _ = s.send(Box::new(f)); }
    }

    pub fn spawn<T, F>(&self, f: F) -> JoinHandle<T>
    where T: Send + 'static, F: FnOnce() -> T + Send + 'static {
        let (tx, rx) = mpsc::channel();
        self.execute(move || { let _ = tx.send(f()); });
        JoinHandle { receiver: Some(rx) }
    }

    pub fn worker_count(&self) -> usize { self.workers.len() }
}

impl Drop for ThreadPool {
    fn drop(&mut self) { drop(self.sender.take()); }
}

pub struct JoinHandle<T> {
    receiver: Option<mpsc::Receiver<T>>,
}

impl<T> JoinHandle<T> {
    pub fn join(mut self) -> T { self.receiver.take().unwrap().recv().unwrap() }
}

// ──────────────── 隔离临时目录 ────────────────

use std::path::{Path, PathBuf};
use std::{fs, io};

/// RAII 隔离临时目录：Drop 自动清理，解决并行测试 temp 竞态
pub struct TempDir {
    path: PathBuf,
    cleanup: bool,
}

impl TempDir {
    pub fn new(prefix: &str) -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!("{}_{:08x}", prefix, rand_u64()));
        fs::create_dir_all(&path)?;
        Ok(Self { path, cleanup: true })
    }

    pub fn in_dir(parent: &Path, prefix: &str) -> io::Result<Self> {
        let path = parent.join(format!("{}_{:08x}", prefix, rand_u64()));
        fs::create_dir_all(&path)?;
        Ok(Self { path, cleanup: true })
    }

    pub fn path(&self) -> &Path { &self.path }
    pub fn join(&self, name: &str) -> PathBuf { self.path.join(name) }

    pub fn create_file(&self, name: &str, content: &str) -> io::Result<PathBuf> {
        let fp = self.path.join(name);
        fs::write(&fp, content)?;
        Ok(fp)
    }

    pub fn remove(mut self) -> io::Result<()> {
        self.cleanup = false;
        fs::remove_dir_all(&self.path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.cleanup && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

impl std::fmt::Debug for TempDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TempDir").field("path", &self.path).finish()
    }
}

fn rand_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_pool() {
        let pool = ThreadPool::new(4);
        let handles: Vec<_> = (0..10).map(|i| pool.spawn(move || i * i)).collect();
        let squares: Vec<i32> = handles.into_iter().map(|h| h.join()).collect();
        assert_eq!(squares, vec![0,1,4,9,16,25,36,49,64,81]);
    }

    #[test]
    fn test_temp_dir_lifecycle() {
        let p;
        { let d = TempDir::new("lz-t").unwrap(); p = d.path().to_path_buf();
          assert!(p.exists()); d.create_file("x.txt", "hi").unwrap(); }
        assert!(!p.exists());
    }

    #[test]
    fn test_temp_dir_parallel_isolation() {
        let pool = ThreadPool::new(4);
        let handles: Vec<_> = (0..8).map(|i| pool.spawn(move || {
            let d = TempDir::new("lz-iso").unwrap();
            let c = format!("w{}", i);
            d.create_file("data.txt", &c).unwrap();
            assert_eq!(fs::read_to_string(d.join("data.txt")).unwrap(), c);
            d.remove().unwrap();
            i
        })).collect();
        let r: Vec<i32> = handles.into_iter().map(|h| h.join()).collect();
        assert_eq!(r.len(), 8);
    }
}
