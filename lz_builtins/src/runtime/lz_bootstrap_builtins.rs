// lz_builtins::runtime::lz_bootstrap_builtins — 自举路线 B · E 环节
// 由 bootstrap/work/lz_builtins/core_subset.lz 经 lzc 生成，2026-08-17；改 LZ 源后重生成。
// 生成方式:
//   lang-zone.exe bootstrap\work\lz_builtins\core_subset.lz --std-dir std
//   生成 core_subset.rs 后提取 pub fn 定义段（剔除文件头 use 块/allow 属性、main 与 __magic__ consts、
//   自检辅助函数 is_pos），手工并入本文件；本文件函数名保持 pub。
// 注意: 本文件位于 crate 内部，不可含 `use lz_builtins::*;` 与 `fn main`。
// 全部函数只用 LZ 原语实现（运算符/索引/len/循环/push/三元），不调用 Rust builtins 等价物。

pub fn lz_all(xs: Vec<bool>) -> bool {
    let mut ok = true;
    for x in (xs).into_iter() {
        if !(x) {
            ok = false;
        } else { ()};
    }
    return ok;
}

pub fn lz_any(xs: Vec<bool>) -> bool {
    let mut found = false;
    for x in (xs).into_iter() {
        if x {
            found = true;
        } else { ()};
    }
    return found;
}

pub fn lz_count_if(xs: Vec<i64>, mut pred: impl FnMut(i64) -> bool) -> i64 {
    let mut c: i64 = 0i64;
    for x in (xs).into_iter() {
        if pred(x) {
            c = c + 1i64;
        } else { ()};
    }
    return c;
}

pub fn lz_sum_ints(xs: Vec<i64>) -> i64 {
    let mut s: i64 = 0i64;
    for x in (xs).into_iter() {
        s = s + x;
    }
    return s;
}

pub fn lz_join_words(xs: Vec<String>, sep: String) -> String {
    let mut out: String = "".to_string();
    let mut first = true;
    for w in (xs).into_iter() {
        if !(first) {
            out = out + &sep[..];
        } else { ()};
        out = out + &w[..];
        first = false;
    }
    return out;
}

pub fn lz_ends_with(s: String, suffix: String) -> bool {
    let mut n: i64 = (s.len() as i64);
    let mut m: i64 = (suffix.len() as i64);
    let mut ok: bool = n >= m;
    let mut i: i64 = 0i64;
    while ok && i < m {
        if ((s).as_bytes()[((n - m + i) as usize)] as i64) != ((suffix).as_bytes()[((i) as usize)] as i64) {
            ok = false;
        } else { ()};
        i = i + 1i64;
    }
    return ok;
}

pub fn lz_abs(x: i64) -> i64 {
    return if x < 0i64 { -x } else { x };
}

pub fn lz_clamp(x: i64, lo: i64, hi: i64) -> i64 {
    if x < lo {
        return lo;
    } else { ()};
    if x > hi {
        return hi;
    } else { ()};
    return x;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_pos_for_test(x: i64) -> bool {
        x > 0
    }

    #[test]
    fn test_lz_all() {
        assert_eq!(lz_all(vec![true, true]), true);
        assert_eq!(lz_all(vec![true, false]), false);
        assert_eq!(lz_all(Vec::<bool>::new()), true);
    }

    #[test]
    fn test_lz_any() {
        assert_eq!(lz_any(vec![false, false]), false);
        assert_eq!(lz_any(vec![false, true]), true);
        assert_eq!(lz_any(Vec::<bool>::new()), false);
    }

    #[test]
    fn test_lz_count_if() {
        assert_eq!(lz_count_if(vec![-1, 2, 3, -4], is_pos_for_test), 2);
        assert_eq!(lz_count_if(Vec::<i64>::new(), is_pos_for_test), 0);
        assert_eq!(lz_count_if(vec![0, 0, -9], is_pos_for_test), 0);
    }

    #[test]
    fn test_lz_sum_ints() {
        assert_eq!(lz_sum_ints(vec![1, 2, 3]), 6);
        assert_eq!(lz_sum_ints(Vec::<i64>::new()), 0);
        assert_eq!(lz_sum_ints(vec![-5, 5]), 0);
    }

    #[test]
    fn test_lz_join_words() {
        assert_eq!(lz_join_words(vec!["a".to_string(), "b".to_string(), "c".to_string()], "-".to_string()), "a-b-c");
        assert_eq!(lz_join_words(Vec::<String>::new(), "-".to_string()), "");
        assert_eq!(lz_join_words(vec!["x".to_string()], ",".to_string()), "x");
    }

    #[test]
    fn test_lz_ends_with() {
        assert_eq!(lz_ends_with("hello".to_string(), "lo".to_string()), true);
        assert_eq!(lz_ends_with("hello".to_string(), "x".to_string()), false);
        assert_eq!(lz_ends_with("abc".to_string(), String::new()), true);
        assert_eq!(lz_ends_with("ab".to_string(), "abc".to_string()), false);
    }

    #[test]
    fn test_lz_abs() {
        assert_eq!(lz_abs(-7), 7);
        assert_eq!(lz_abs(7), 7);
        assert_eq!(lz_abs(0), 0);
    }

    #[test]
    fn test_lz_clamp() {
        assert_eq!(lz_clamp(100, 0, 10), 10);
        assert_eq!(lz_clamp(-5, 0, 10), 0);
        assert_eq!(lz_clamp(5, 0, 10), 5);
    }
}
