# -*- coding: utf-8 -*-
import os

d = ".fist-wf-20260926/r2"
os.makedirs(d, exist_ok=True)

probes = {
"g1_bigint_param.lz": '''def double(x: bigint): bigint =
    x * BigInt::from(2)

def main() =
    let r = double(BigInt::from(123456789012345678901234567890))
    println(r)
''',
"g2_complex_arith.lz": '''def main() =
    let a: complex = Complex64::new(1.0, 2.0)
    let b: complex = Complex64::new(3.0, 4.0)
    let c: complex = a * b
    let d: complex = a + b
    println(c)
    println(d)
''',
"g3_list_opt_bigint.lz": '''def main() =
    let xs: List<bigint> = [BigInt::from(1), BigInt::from(2), BigInt::from(3)]
    let o: Option<bigint> = Some(BigInt::from(42))
    println(xs)
    println(o)
''',
"g4_const_arith.lz": '''const K: bigint = BigInt::from(10) * BigInt::from(20) + BigInt::from(5)

def main() =
    println(K)
''',
"g5_nested_list.lz": '''def main() =
    let m: List<List<int>> = [[1, 2], [3, 4], []]
    println(m)
''',
"g6_bigint_div.lz": '''def main() =
    let x: bigint = BigInt::from(100)
    let y: bigint = x / BigInt::from(7)
    let z: bigint = x % BigInt::from(7)
    println(y)
    println(z)
''',
"g7_complex_struct.lz": '''struct P = re: complex, im: complex

def main() =
    let p = P { re: Complex64::new(1.0, 0.0), im: Complex64::new(0.0, 1.0) }
    let mag: complex = p.re * p.re + p.im * p.im
    println(mag)
''',
"g8_dict_bigint.lz": '''def main() =
    let m: Dict<bigint, int> = { BigInt::from(1): 10, BigInt::from(2): 20 }
    println(m)
''',
}

for fn, src in probes.items():
    with open(os.path.join(d, fn), "w", encoding="utf-8", newline="") as f:
        f.write(src)
print("written", len(probes))
