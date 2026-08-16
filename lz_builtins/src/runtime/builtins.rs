// lz_builtins::builtins — Python 对齐的内置函数 & LZ 运行时基础
// 这些函数/类型在 LZ 中无需导入即可直接使用
//
// 分类标记:
//   [RT]  = 运行时可用（也可在编译期调用）
//   [CT]  = 仅编译期可用（comptime only，运行时不可调用）
//   [LZ]  = LZ 特有扩展

use std::fmt::Debug;
use std::fmt::Display;

// ══════════════════════════════════════════════════════════════
// [RT] print — 对齐 Python print(*objects, sep=' ', end='\n')
// ══════════════════════════════════════════════════════════════

/// 多参数 print — 接受 &[String] 切片
pub fn print(args: &[String], sep: &str, end: &str) {
    let mut first = true;
    for a in args {
        if !first {
            print!("{}", sep);
        }
        print!("{}", a);
        first = false;
    }
    print!("{}", end);
}

/// 简化版 print_val — 单个 Debug 值（编译期生成默认使用）
pub fn print_val<T: Debug>(value: T) {
    println!("{:?}", value);
}

/// print_str — 单个 Display 值
pub fn print_str<T: Display>(value: T) {
    println!("{}", value);
}

// ══════════════════════════════════════════════════════════════
// [RT] len — 返回容器长度
// ══════════════════════════════════════════════════════════════

pub trait Len {
    fn __len__(&self) -> i64;
}

impl<T> Len for Vec<T> {
    fn __len__(&self) -> i64 {
        self.len() as i64
    }
}
impl<K, V> Len for std::collections::HashMap<K, V> {
    fn __len__(&self) -> i64 {
        self.len() as i64
    }
}
impl<T> Len for std::collections::HashSet<T> {
    fn __len__(&self) -> i64 {
        self.len() as i64
    }
}
impl Len for String {
    fn __len__(&self) -> i64 {
        self.len() as i64
    }
}
impl Len for &str {
    fn __len__(&self) -> i64 {
        self.len() as i64
    }
}

pub fn len<T: Len>(obj: &T) -> i64 {
    obj.__len__()
}

// ══════════════════════════════════════════════════════════════
// [RT] range — Python range(stop) / range(start, stop, step=1)
// ══════════════════════════════════════════════════════════════

pub struct Range {
    start: i64,
    end: i64,
    step: i64,
}

impl Range {
    pub fn new(stop: i64) -> Self {
        Range {
            start: 0,
            end: stop,
            step: 1,
        }
    }
    pub fn with_start(start: i64, stop: i64) -> Self {
        Range {
            start,
            end: stop,
            step: 1,
        }
    }
    pub fn with_step(start: i64, stop: i64, step: i64) -> Self {
        Range {
            start,
            end: stop,
            step,
        }
    }
    pub fn len(&self) -> i64 {
        if self.step == 0 {
            return 0;
        }
        let n = (self.end - self.start + self.step - self.step.signum()) / self.step;
        if n < 0 {
            0
        } else {
            n
        }
    }
    pub fn contains(&self, val: i64) -> bool {
        if self.step > 0 {
            val >= self.start && val < self.end && (val - self.start) % self.step == 0
        } else if self.step < 0 {
            val <= self.start && val > self.end && (self.start - val) % (-self.step) == 0
        } else {
            false
        }
    }
}

impl IntoIterator for Range {
    type Item = i64;
    type IntoIter = RangeIter;
    fn into_iter(self) -> RangeIter {
        RangeIter {
            current: self.start,
            end: self.end,
            step: self.step,
        }
    }
}

pub struct RangeIter {
    current: i64,
    end: i64,
    step: i64,
}

impl Iterator for RangeIter {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.step > 0 && self.current >= self.end {
            return None;
        }
        if self.step < 0 && self.current <= self.end {
            return None;
        }
        let val = self.current;
        self.current += self.step;
        Some(val)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = if self.step == 0 {
            0
        } else {
            let remaining = (self.end - self.current + self.step - self.step.signum()) / self.step;
            if remaining < 0 {
                0
            } else {
                remaining as usize
            }
        };
        (n, Some(n))
    }
}

impl DoubleEndedIterator for RangeIter {
    fn next_back(&mut self) -> Option<i64> {
        if self.step > 0 && self.current >= self.end {
            return None;
        }
        if self.step < 0 && self.current <= self.end {
            return None;
        }
        self.end -= self.step;
        Some(self.end)
    }
}

pub fn range(stop: i64) -> Range {
    Range::new(stop)
}
pub fn range2(start: i64, stop: i64) -> Range {
    Range::with_start(start, stop)
}
pub fn range3(start: i64, stop: i64, step: i64) -> Range {
    Range::with_step(start, stop, step)
}

// ══════════════════════════════════════════════════════════════
// [RT] enumerate / zip / map / filter / sorted / reversed
// ══════════════════════════════════════════════════════════════

/// enumerate(iter[, start=0]) → (index, value) 迭代器
pub fn enumerate<I: IntoIterator>(iter: I) -> Enumerate<I::IntoIter> {
    Enumerate {
        iter: iter.into_iter(),
        index: 0,
    }
}
pub fn enumerate_from<I: IntoIterator>(iter: I, start: i64) -> Enumerate<I::IntoIter> {
    Enumerate {
        iter: iter.into_iter(),
        index: start,
    }
}

pub struct Enumerate<I> {
    iter: I,
    index: i64,
}

impl<I: Iterator> Iterator for Enumerate<I> {
    type Item = (i64, I::Item);
    fn next(&mut self) -> Option<(i64, I::Item)> {
        self.iter.next().map(|item| {
            let idx = self.index;
            self.index += 1;
            (idx, item)
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// zip(*iterables) → 并行迭代
pub fn zip<A: IntoIterator, B: IntoIterator>(a: A, b: B) -> Zip<A::IntoIter, B::IntoIter> {
    Zip {
        a: a.into_iter(),
        b: b.into_iter(),
    }
}

pub struct Zip<A, B> {
    a: A,
    b: B,
}

impl<A: Iterator, B: Iterator> Iterator for Zip<A, B> {
    type Item = (A::Item, B::Item);
    fn next(&mut self) -> Option<(A::Item, B::Item)> {
        match (self.a.next(), self.b.next()) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }
}

// ══════════════════════════════════════════════════════════════
// [RT] map / filter — 函数式操作
// ══════════════════════════════════════════════════════════════

pub fn map<I: IntoIterator, F: FnMut(I::Item) -> O, O>(
    iter: I,
    f: F,
) -> std::iter::Map<I::IntoIter, F> {
    iter.into_iter().map(f)
}

pub fn filter<I: IntoIterator, P: FnMut(&I::Item) -> bool>(
    iter: I,
    pred: P,
) -> std::iter::Filter<I::IntoIter, P> {
    iter.into_iter().filter(pred)
}

/// sorted(iter, *, reverse=false) → Vec<T> 排序后收集
pub fn sorted<T: Ord>(iter: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut v: Vec<T> = iter.into_iter().collect();
    v.sort();
    v
}

pub fn sorted_reverse<T: Ord>(iter: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut v: Vec<T> = iter.into_iter().collect();
    v.sort_by(|a, b| b.cmp(a));
    v
}

/// reversed(iter) → 反转
pub fn reversed<T: Clone>(iter: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut v: Vec<T> = iter.into_iter().collect();
    v.reverse();
    v
}

// ══════════════════════════════════════════════════════════════
// [RT] min / max / abs / sum / product / all / any
// ══════════════════════════════════════════════════════════════

pub fn abs_i64(x: i64) -> i64 {
    x.abs()
}
pub fn abs_f64(x: f64) -> f64 {
    x.abs()
}

pub fn min<T: Ord>(a: T, b: T) -> T {
    a.min(b)
}
pub fn max<T: Ord>(a: T, b: T) -> T {
    a.max(b)
}
pub fn min_iter<T: Ord>(iter: impl IntoIterator<Item = T>) -> Option<T> {
    iter.into_iter().min()
}
pub fn max_iter<T: Ord>(iter: impl IntoIterator<Item = T>) -> Option<T> {
    iter.into_iter().max()
}

pub fn clamp<T: Ord>(value: T, lo: T, hi: T) -> T {
    value.clamp(lo, hi)
}

pub fn pow_i64(base: i64, exp: u32) -> i64 {
    base.pow(exp)
}
pub fn pow_f64(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

pub fn round(x: f64) -> f64 {
    x.round()
}
pub fn floor(x: f64) -> f64 {
    x.floor()
}
pub fn ceil(x: f64) -> f64 {
    x.ceil()
}

pub fn divmod(a: i64, b: i64) -> (i64, i64) {
    (a / b, a % b)
}

pub fn all<I: IntoIterator<Item = bool>>(iter: I) -> bool {
    iter.into_iter().all(|x| x)
}
pub fn any<I: IntoIterator<Item = bool>>(iter: I) -> bool {
    iter.into_iter().any(|x| x)
}

pub fn sum_i64<I: IntoIterator<Item = i64>>(iter: I) -> i64 {
    iter.into_iter().sum()
}
pub fn sum_f64<I: IntoIterator<Item = f64>>(iter: I) -> f64 {
    iter.into_iter().sum()
}

pub fn product_i64<I: IntoIterator<Item = i64>>(iter: I) -> i64 {
    iter.into_iter().product()
}
pub fn product_f64<I: IntoIterator<Item = f64>>(iter: I) -> f64 {
    iter.into_iter().product()
}

// ══════════════════════════════════════════════════════════════
// [RT] 类型查询 & 转换
// ══════════════════════════════════════════════════════════════

/// type_name<T>() — 获取类型名称
pub fn type_name<T>() -> String {
    std::any::type_name::<T>().to_string()
}

/// type_of(val) — 获取值的类型名称
pub fn type_of<T>(_val: &T) -> String {
    std::any::type_name::<T>().to_string()
}

/// isinstance(value, type_name) — 运行时类型检查
pub fn isinstance<T: 'static>(_val: &T) -> String {
    std::any::type_name::<T>().to_string()
}

/// int(s) → Result<i64>
pub fn lz_int(s: &str) -> Result<i64, String> {
    s.parse::<i64>()
        .map_err(|e| format!("invalid int literal: {}", e))
}

/// float(s) → Result<f64>
pub fn lz_float(s: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .map_err(|e| format!("invalid float literal: {}", e))
}

/// bool(s) → Result<bool>
pub fn lz_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(format!("invalid bool: {}", s)),
    }
}

/// str(val) → String
pub fn to_str<T: Display>(val: T) -> String {
    val.to_string()
}

/// repr(val) → String (Debug 格式)
pub fn repr<T: Debug>(val: T) -> String {
    format!("{:?}", val)
}

// ══════════════════════════════════════════════════════════════
// [RT] ord / chr — 字符编码
// ══════════════════════════════════════════════════════════════

pub fn ord(c: char) -> i64 {
    c as i64
}
pub fn chr(code: i64) -> Option<char> {
    std::char::from_u32(code as u32)
}
pub fn chr_unchecked(code: i64) -> char {
    std::char::from_u32(code as u32).unwrap_or('\u{FFFD}')
}

// ══════════════════════════════════════════════════════════════
// [RT] id / hash
// ══════════════════════════════════════════════════════════════

pub fn id<T>(val: &T) -> usize {
    val as *const T as usize
}

pub fn hash<T: std::hash::Hash>(val: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    val.hash(&mut h);
    h.finish()
}

// ══════════════════════════════════════════════════════════════
// [RT] io / open / input
// ══════════════════════════════════════════════════════════════

/// input(prompt: &str) -> String  — 读取一行用户输入
pub fn input(prompt: &str) -> String {
    use std::io::{self, Write};
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap_or_default();
    s.trim_end_matches('\n').trim_end_matches('\r').to_string()
}

/// 文件读取
pub fn read_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("read_file: {}", e))
}

/// 文件写入
pub fn write_file(path: &str, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("write_file: {}", e))
}

// ══════════════════════════════════════════════════════════════
// [LZ] checker 参数结构体
// ══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct __Params {
    pub args: Vec<Box<dyn std::any::Any>>,
    pub kwargs: std::collections::HashMap<String, Box<dyn std::any::Any>>,
}

impl __Params {
    pub fn new() -> Self {
        __Params {
            args: Vec::new(),
            kwargs: std::collections::HashMap::new(),
        }
    }

    /// 从一组 Box<dyn Any> 参数构建
    pub fn from_args(args: Vec<Box<dyn std::any::Any>>) -> Self {
        __Params {
            args,
            kwargs: std::collections::HashMap::new(),
        }
    }

    /// 获取第 i 个参数（不拆箱，返回引用）
    pub fn get_raw(&self, i: usize) -> Option<&dyn std::any::Any> {
        self.args.get(i).map(|b| b.as_ref())
    }

    /// 获取第 i 个参数并拆箱为具体类型
    pub fn get<T: 'static>(&self, i: usize) -> Option<&T> {
        self.args.get(i).and_then(|b| b.downcast_ref::<T>())
    }

    /// 获取第 i 个参数的可变引用
    pub fn get_mut<T: 'static>(&mut self, i: usize) -> Option<&mut T> {
        self.args.get_mut(i).and_then(|b| b.downcast_mut::<T>())
    }

    /// 设置第 i 个参数
    pub fn set<T: 'static>(&mut self, i: usize, val: T) {
        if i < self.args.len() {
            self.args[i] = Box::new(val);
        } else {
            self.args.push(Box::new(val));
        }
    }

    /// 参数数量
    pub fn len(&self) -> i64 {
        self.args.len() as i64
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }
}

// ══════════════════════════════════════════════════════════════
// [RT] async / spawn 运行时
// ══════════════════════════════════════════════════════════════

pub async fn __spawn_task<T>(f: impl std::future::Future<Output = T>) -> T {
    f.await
}

pub fn __block_on<F: std::future::Future>(mut f: F) -> F::Output {
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    unsafe fn clone_raw(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    unsafe fn noop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_raw, noop, noop, noop);

    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut f = unsafe { Pin::new_unchecked(&mut f) };
    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

// ══════════════════════════════════════════════════════════════
// [RT] 路径/类型适配 shims
// ══════════════════════════════════════════════════════════════

#[inline(always)]
pub fn __lz_path(s: &str) -> &std::path::Path {
    std::path::Path::new(s)
}
#[inline(always)]
pub fn __lz_pathbuf(s: String) -> std::path::PathBuf {
    std::path::PathBuf::from(s)
}
#[inline(always)]
pub fn __lz_str_ref(s: &String) -> &str {
    s.as_str()
}
#[inline(always)]
pub fn __lz_bytes(s: &str) -> &[u8] {
    s.as_bytes()
}
#[inline(always)]
pub fn __lz_duration_ms(ms: i64) -> std::time::Duration {
    std::time::Duration::from_millis(ms as u64)
}
#[inline(always)]
pub fn __lz_duration_secs(secs: f64) -> std::time::Duration {
    std::time::Duration::from_secs_f64(secs)
}
