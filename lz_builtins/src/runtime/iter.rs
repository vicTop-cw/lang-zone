// lz_builtins::iter
// 对齐 Python itertools + LZ Iterator trait

// ══════════════════════════════════════════════════════════════
// Map — 映射迭代器
// ══════════════════════════════════════════════════════════════

pub struct Map<I, F> {
    pub iter: I,
    pub func: F,
}

impl<I: Iterator, F: FnMut(I::Item) -> B, B> Iterator for Map<I, F> {
    type Item = B;
    fn next(&mut self) -> Option<B> {
        self.iter.next().map(&mut self.func)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

pub fn lz_map<I: IntoIterator, F: FnMut(I::Item) -> B, B>(iter: I, f: F) -> Map<I::IntoIter, F> {
    Map {
        iter: iter.into_iter(),
        func: f,
    }
}

// ══════════════════════════════════════════════════════════════
// Filter — 过滤迭代器
// ══════════════════════════════════════════════════════════════

pub struct Filter<I, P> {
    pub iter: I,
    pub pred: P,
}

impl<I: Iterator, P: FnMut(&I::Item) -> bool> Iterator for Filter<I, P> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        while let Some(item) = self.iter.next() {
            if (self.pred)(&item) {
                return Some(item);
            }
        }
        None
    }
}

pub fn lz_filter<I: IntoIterator, P: FnMut(&I::Item) -> bool>(
    iter: I,
    pred: P,
) -> Filter<I::IntoIter, P> {
    Filter {
        iter: iter.into_iter(),
        pred,
    }
}

// ══════════════════════════════════════════════════════════════
// Enumerate — 枚举迭代器
// ══════════════════════════════════════════════════════════════

pub struct Enumerate<I> {
    pub iter: I,
    pub index: i64,
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
}

pub fn lz_enumerate<I: IntoIterator>(iter: I) -> Enumerate<I::IntoIter> {
    Enumerate {
        iter: iter.into_iter(),
        index: 0,
    }
}

// ══════════════════════════════════════════════════════════════
// Zip — 同时迭代两个迭代器
// ══════════════════════════════════════════════════════════════

pub struct Zip<A, B> {
    pub a: A,
    pub b: B,
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

pub fn lz_zip<A: IntoIterator, B: IntoIterator>(a: A, b: B) -> Zip<A::IntoIter, B::IntoIter> {
    Zip {
        a: a.into_iter(),
        b: b.into_iter(),
    }
}

// ══════════════════════════════════════════════════════════════
// Chain — 链式连接两个迭代器
// ══════════════════════════════════════════════════════════════

pub struct Chain<A, B> {
    pub first: A,
    pub second: B,
    pub first_done: bool,
}

impl<A: Iterator, B: Iterator<Item = A::Item>> Iterator for Chain<A, B> {
    type Item = A::Item;
    fn next(&mut self) -> Option<A::Item> {
        if !self.first_done {
            match self.first.next() {
                Some(item) => return Some(item),
                None => self.first_done = true,
            }
        }
        self.second.next()
    }
}

pub fn lz_chain<A: IntoIterator, B: IntoIterator<Item = A::Item>, I: IntoIterator>(
    first: I,
    second: B,
) -> Chain<I::IntoIter, B::IntoIter>
where
    I::Item: Into<A::Item>,
    I: IntoIterator,
    I::IntoIter: Iterator,
{
    Chain {
        first: first.into_iter(),
        second: second.into_iter(),
        first_done: false,
    }
}

// ══════════════════════════════════════════════════════════════
// Take — 取前 n 个元素
// ══════════════════════════════════════════════════════════════

pub struct Take<I> {
    pub iter: I,
    pub remaining: i64,
}

impl<I: Iterator> Iterator for Take<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        if self.remaining <= 0 {
            return None;
        }
        self.remaining -= 1;
        self.iter.next()
    }
}

pub fn lz_take<I: IntoIterator>(iter: I, n: i64) -> Take<I::IntoIter> {
    Take {
        iter: iter.into_iter(),
        remaining: n,
    }
}

// ══════════════════════════════════════════════════════════════
// Skip — 跳过前 n 个元素
// ══════════════════════════════════════════════════════════════

pub struct Skip<I> {
    pub iter: I,
    pub to_skip: i64,
}

impl<I: Iterator> Iterator for Skip<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        while self.to_skip > 0 {
            self.iter.next();
            self.to_skip -= 1;
        }
        self.iter.next()
    }
}

pub fn lz_skip<I: IntoIterator>(iter: I, n: i64) -> Skip<I::IntoIter> {
    Skip {
        iter: iter.into_iter(),
        to_skip: n,
    }
}

// ══════════════════════════════════════════════════════════════
// Repeat — 无限重复迭代器
// ══════════════════════════════════════════════════════════════

pub struct Repeat<T: Clone> {
    pub value: T,
}

impl<T: Clone> Iterator for Repeat<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        Some(self.value.clone())
    }
}

pub fn lz_repeat<T: Clone>(value: T) -> Repeat<T> {
    Repeat { value }
}

// ══════════════════════════════════════════════════════════════
// Once — 单元素迭代器
// ══════════════════════════════════════════════════════════════

pub struct Once<T> {
    pub value: Option<T>,
}

impl<T> Iterator for Once<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.value.take()
    }
}

pub fn lz_once<T>(value: T) -> Once<T> {
    Once { value: Some(value) }
}

// ══════════════════════════════════════════════════════════════
// Empty — 空迭代器
// ══════════════════════════════════════════════════════════════

pub struct Empty<T>(std::marker::PhantomData<T>);

impl<T> Iterator for Empty<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        None
    }
}

pub fn lz_empty<T>() -> Empty<T> {
    Empty(std::marker::PhantomData)
}

// ══════════════════════════════════════════════════════════════
// Range — LZ 范围迭代器 [start, end)
// ══════════════════════════════════════════════════════════════

pub struct LzRange {
    pub current: i64,
    pub end: i64,
    pub step: i64,
}

impl Iterator for LzRange {
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
}

pub fn lz_range(start: i64, end: i64) -> LzRange {
    LzRange {
        current: start,
        end,
        step: 1,
    }
}

pub fn lz_range_step(start: i64, end: i64, step: i64) -> LzRange {
    LzRange {
        current: start,
        end,
        step,
    }
}

// ══════════════════════════════════════════════════════════════
// Collect — 收集迭代器为 List
// ══════════════════════════════════════════════════════════════

pub fn lz_collect<T, I: IntoIterator<Item = T>>(iter: I) -> Vec<T> {
    iter.into_iter().collect()
}
