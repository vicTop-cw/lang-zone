static mut r: i64 = 0;
static mut q: i64 = 0;


// ── LZ rustc link recipe (BUG-10: lz_builtins re-exports num-bigint/num-complex) ──
// To compile & run this generated file in isolation:
//   rustc --edition 2021 --extern lz_builtins=<rlib> -L dependency=<deps_dir> -O <this_file>.rs
//   <rlib>     = liblz_builtins.rlib (build it: `cargo build -p lz_builtins`)
//   <deps_dir> = the directory containing that rlib (e.g. <workspace>/target/debug/deps)
// Omitting -L gives error[E0463] even if this program never uses BigInt/Complex.
// ─────────────────────────────────────────────────────────────────────────────────────

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

pub fn abs(x: i64) -> i64 {
    return if x < 0i64 { -x } else { x };
}

pub fn sign(x: i64) -> i64 {
    return if x < 0i64 { -1i64 } else { (if x > 0i64 { 1i64 } else { 0i64 }) };
}

pub fn max_int(a: i64, b: i64) -> i64 {
    return if a > b { a } else { b };
}

pub fn min_int(a: i64, b: i64) -> i64 {
    return if a < b { a } else { b };
}

pub fn clamp_int(x: i64, lo: i64, hi: i64) -> i64 {
    return if x < lo { lo } else { (if x > hi { hi } else { x }) };
}

pub fn pow(base: i64, exp: i64) -> i64 {
    if exp < 0i64 {
        return 0i64;
    } else { ()}
    if exp == 0i64 {
        return 1i64;
    } else { ()}
    let mut result: i64 = 1i64;
    let mut i: i64 = 0i64;
    while i < exp {
        result *= base;
        i += 1i64;
    }
    return result;
}

pub fn divmod(n: i64, d: i64) -> (i64, i64) {
    if d == 0i64 {
        return (0i64, 0i64);
    } else { ()}
    unsafe { q = n / d; }
    unsafe { r = n % d; }
    return (unsafe { q }, unsafe { r });
}

pub fn is_even(x: i64) -> bool {
    return x % 2i64 == 0i64;
}

pub fn is_odd(x: i64) -> bool {
    return x % 2i64 != 0i64;
}

pub fn gcd(a: i64, b: i64) -> i64 {
    let mut a2: i64 = lz_abs(a);
    let mut b2: i64 = lz_abs(b);
    while b2 != 0i64 {
        (unsafe { q }, unsafe { r }) = divmod(a2, b2);
        a2 = b2;
        b2 = unsafe { r };
    }
    return a2;
}

pub fn lcm(a: i64, b: i64) -> i64 {
    if a == 0i64 || b == 0i64 {
        return 0i64;
    } else { ()}
    return lz_abs(a * b) / gcd(a, b);
}

pub fn factorial(n: i64) -> i64 {
    if n < 0i64 {
        return 0i64;
    } else { ()}
    if n <= 1i64 {
        return 1i64;
    } else { ()}
    let mut result: i64 = 1i64;
    let mut i: i64 = 2i64;
    while i <= n {
        result *= i;
        i += 1i64;
    }
    return result;
}

pub fn fibonacci(n: i64) -> i64 {
    if n <= 0i64 {
        return 0i64;
    } else { ()}
    if n == 1i64 {
        return 1i64;
    } else { ()}
    let mut a: i64 = 0i64;
    let mut b: i64 = 1i64;
    let mut i: i64 = 2i64;
    while i <= n {
        let mut c: i64 = a + b;
        a = b;
        b = c;
        i += 1i64;
    }
    return b;
}

pub fn is_prime(n: i64) -> bool {
    if n < 2i64 {
        return false;
    } else { ()}
    if n == 2i64 {
        return true;
    } else { ()}
    if is_even(n) {
        return false;
    } else { ()}
    let mut i: i64 = 3i64;
    while i * i <= n {
        if n % i == 0i64 {
            return false;
        } else { ()}
        i += 2i64;
    }
    return true;
}

pub fn primes_upto(n: i64) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut i: i64 = 2i64;
    while i <= n {
        if is_prime(i) { let _ = (result).push(i.clone()); } else { () }
        i += 1i64;
    }
    return result;
}

pub fn digit_sum(n: i64) -> i64 {
    if n < 0i64 {
        return digit_sum(-n);
    } else { ()}
    if n == 0i64 {
        return 0i64;
    } else { ()}
    return digit_sum(n / 10i64) + (n % 10i64);
}

pub fn digit_count(n: i64) -> i64 {
    if n == 0i64 {
        return 1i64;
    } else { ()}
    if n < 0i64 {
        return digit_count(-n) + 1i64;
    } else { ()}
    let mut count: i64 = 0i64;
    let mut temp: i64 = n;
    while temp > 0i64 {
        count += 1i64;
        temp /= 10i64;
    }
    return count;
}

pub fn range_sum(n: i64) -> i64 {
    return n * (n + 1i64) / 2i64;
}

pub fn abs_f(x: f64) -> f64 {
    return if x < 0.0f64 { -x } else { x };
}

pub fn sign_f(x: f64) -> f64 {
    return if x < 0.0f64 { -1.0f64 } else { (if x > 0.0f64 { 1.0f64 } else { 0.0f64 }) };
}

pub fn pow_f(base: f64, exp: f64) -> f64 {
    return pow_i64(base as i64, exp as i64 as u32) as f64;
}

pub fn floor(x: f64) -> i64 {
    if x >= 0.0f64 {
        return x as i64;
    } else { ()}
    return (if (x as i64 as f64) == x {
        return x as i64;
    } else { ()}(x - 1.0f64)).parse::<i64>().unwrap();
}

pub fn ceil(x: f64) -> i64 {
    if x <= 0.0f64 {
        return x as i64;
    } else { ()}
    return (if (x as i64 as f64) == x {
        return x as i64;
    } else { ()}(x + 1.0f64)).parse::<i64>().unwrap();
}

pub fn round(x: f64) -> i64 {
    return (if x >= 0.0f64 {
        return (x + 0.5f64) as i64;
    } else { ()}(x - 0.5f64)).parse::<i64>().unwrap();
}

pub fn clamp_float(x: f64, lo: f64, hi: f64) -> f64 {
    return if x < lo { lo } else { (if x > hi { hi } else { x }) };
}

pub fn is_nan(x: f64) -> bool {
    return x != x;
}

pub fn is_inf(x: f64) -> bool {
    return x > 100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.0f64 || x < -100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.0f64;
}

pub fn arithmetic_series(a: i64, d: i64, n: i64) -> i64 {
    if n <= 0i64 {
        return 0i64;
    } else { ()}
    return n * a + d * (n - 1i64) * n / 2i64;
}

pub fn geometric_series(a: i64, r: i64, n: i64) -> i64 {
    if n <= 0i64 {
        return 0i64;
    } else { ()}
    if (unsafe { r }) == 1i64 {
        return a * n;
    } else { ()}
    return a * (pow_i64(unsafe { r }, n as u32) - 1i64) / ((unsafe { r }) - 1i64);
}

pub fn sum_of_squares(n: i64) -> i64 {
    return n * (n + 1i64) * (2i64 * n + 1i64) / 6i64;
}

pub fn sum_of_cubes(n: i64) -> i64 {
    let mut m: i64 = n * (n + 1i64) / 2i64;
    return m * m;
}

pub fn binomial(n: i64, k: i64) -> i64 {
    if k < 0i64 || k > n {
        return 0i64;
    } else { ()}
    if k == 0i64 || k == n {
        return 1i64;
    } else { ()}
    let mut k2: i64 = if k < n - k { k } else { (n - k) };
    let mut result: i64 = 1i64;
    let mut i: i64 = 0i64;
    while i < k2 {
        result = result * (n - i) / (i + 1i64);
        i += 1i64;
    }
    return result;
}

pub fn permutations(n: i64, k: i64) -> i64 {
    if k < 0i64 || k > n {
        return 0i64;
    } else { ()}
    let mut result: i64 = 1i64;
    let mut i: i64 = 0i64;
    while i < k {
        result *= (n - i);
        i += 1i64;
    }
    return result;
}

pub fn test_abs() {
    println!("{} {:?} {:?} {:?}", "[abs]:", lz_abs(-5i64), lz_abs(0i64), lz_abs(5i64));
    assert_eq!(lz_abs(-5i64), 5i64);
    assert_eq!(lz_abs(0i64), 0i64);
    return assert_eq!(lz_abs(5i64), 5i64);
}

pub fn test_gcd_lcm() {
    println!("{} {:?}", "[gcd 12,8]:", gcd(12i64, 8i64));
    println!("{} {:?}", "[lcm 12,8]:", lcm(12i64, 8i64));
    assert_eq!(gcd(12i64, 8i64), 4i64);
    return assert_eq!(lcm(12i64, 8i64), 24i64);
}

pub fn test_factorial() {
    println!("{} {:?}", "[factorial 5]:", factorial(5i64));
    assert_eq!(factorial(5i64), 120i64);
    assert_eq!(factorial(0i64), 1i64);
    return assert_eq!(factorial(1i64), 1i64);
}

pub fn test_fibonacci() {
    println!("{} {:?}", "[fibonacci 10]:", fibonacci(10i64));
    assert_eq!(fibonacci(0i64), 0i64);
    assert_eq!(fibonacci(1i64), 1i64);
    assert_eq!(fibonacci(6i64), 8i64);
    return assert_eq!(fibonacci(10i64), 55i64);
}

pub fn test_is_prime() {
    println!("{} {:?} {:?} {:?} {:?}", "[is_prime]:", is_prime(7i64), is_prime(8i64), is_prime(2i64), is_prime(1i64));
    assert_eq!(is_prime(7i64), true);
    assert_eq!(is_prime(8i64), false);
    assert_eq!(is_prime(2i64), true);
    return assert_eq!(is_prime(1i64), false);
}

pub fn test_primes_upto() {
    unsafe { r = primes_upto(20i64); }
    println!("{} {:?}", "[primes_upto 20]:", unsafe { r });
    return assert_eq!(unsafe { r }.clone(), vec![2i64, 3i64, 5i64, 7i64, 11i64, 13i64, 17i64, 19i64]);
}

pub fn test_digit() {
    println!("{} {:?}", "[digit_sum 123]:", digit_sum(123i64));
    println!("{} {:?}", "[digit_count 12345]:", digit_count(12345i64));
    assert_eq!(digit_sum(123i64), 6i64);
    return assert_eq!(digit_count(12345i64), 5i64);
}

pub fn test_binomial() {
    println!("{} {:?}", "[binomial 5,2]:", binomial(5i64, 2i64));
    assert_eq!(binomial(5i64, 2i64), 10i64);
    assert_eq!(binomial(10i64, 0i64), 1i64);
    return assert_eq!(binomial(10i64, 10i64), 1i64);
}

pub fn test_sum_squares_cubes() {
    println!("{} {:?}", "[sum_of_squares 3]:", sum_of_squares(3i64));
    println!("{} {:?}", "[sum_of_cubes 3]:", sum_of_cubes(3i64));
    assert_eq!(sum_of_squares(3i64), 14i64);
    return assert_eq!(sum_of_cubes(3i64), 36i64);
}

pub fn test_range_sum() {
    println!("{} {:?}", "[range_sum 100]:", range_sum(100i64));
    return assert_eq!(range_sum(100i64), 5050i64);
}

pub fn test_clamp() {
    println!("{} {:?}", "[clamp_int 100,0,10]:", clamp_int(100i64, 0i64, 10i64));
    assert_eq!(clamp_int(100i64, 0i64, 10i64), 10i64);
    assert_eq!(clamp_int(-5i64, 0i64, 10i64), 0i64);
    return assert_eq!(clamp_int(5i64, 0i64, 10i64), 5i64);
}

pub fn test_divmod() {
    (unsafe { q }, unsafe { r }) = divmod(17i64, 5i64);
    println!("{} {:?} {:?}", "[divmod 17,5]:", unsafe { q }, unsafe { r });
    assert_eq!(unsafe { q }.clone(), 3i64);
    return assert_eq!(unsafe { r }.clone(), 2i64);
}

pub fn main() {
    test_abs();
    test_gcd_lcm();
    test_factorial();
    test_fibonacci();
    test_is_prime();
    test_primes_upto();
    test_digit();
    test_binomial();
    test_sum_squares_cubes();
    test_range_sum();
    test_clamp();
    test_divmod();
    println!("{}", "math.lz: all tests passed!");
}

const __name__: &str = "main";

const __file__: &str = "math.lz";

const __package__: &str = "std";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\lz_builtins\\std";

const __doc__: &str = "";

const __is_macro__: bool = false;

