# Round 2 对抗样例：BigInt/Complex 更多位形 + 嵌套容器
$dir = ".fist-wf-20260926/r2"
New-Item -ItemType Directory -Force $dir | Out-Null

# G1: bigint 作函数参数 + 返回类型（非函数体 let）
@'
def double(x: bigint): bigint =
    x * BigInt::from(2)

def main() =
    let r = double(BigInt::from(123456789012345678901234567890))
    println(r)
'@ | Set-Content -Encoding UTF8 "$dir/g1_bigint_param.lz"

# G2: complex 运算（complex * complex, complex + real）
@'
def main() =
    let a: complex = Complex64::new(1.0, 2.0)
    let b: complex = Complex64::new(3.0, 4.0)
    let c: complex = a * b
    let d: complex = a + b
    println(c)
    println(d)
'@ | Set-Content -Encoding UTF8 "$dir/g2_complex_arith.lz"

# G3: List<bigint> / Option<bigint>
@'
def main() =
    let xs: List<bigint> = [BigInt::from(1), BigInt::from(2), BigInt::from(3)]
    let o: Option<bigint> = Some(BigInt::from(42))
    println(xs)
    println(o)
'@ | Set-Content -Encoding UTF8 "$dir/g3_list_opt_bigint.lz"

# G4: const bigint 算术（惰性）
@'
const K: bigint = BigInt::from(10) * BigInt::from(20) + BigInt::from(5)

def main() =
    println(K)
'@ | Set-Content -Encoding UTF8 "$dir/g4_const_arith.lz"

# G5: 非空嵌套列表 List<List<int>>
@'
def main() =
    let m: List<List<int>> = [[1, 2], [3, 4], []]
    println(m)
'@ | Set-Content -Encoding UTF8 "$dir/g5_nested_list.lz"

# G6: bigint 除法 / 取模
@'
def main() =
    let x: bigint = BigInt::from(100)
    let y: bigint = x / BigInt::from(7)
    let z: bigint = x % BigInt::from(7)
    println(y)
    println(z)
'@ | Set-Content -Encoding UTF8 "$dir/g6_bigint_div.lz"

# G7: complex 作为 struct 字段 + 方法调用
@'
struct P = re: complex, im: complex

def main() =
    let p = P { re: Complex64::new(1.0, 0.0), im: Complex64::new(0.0, 1.0) }
    let mag: complex = p.re * p.re + p.im * p.im
    println(mag)
'@ | Set-Content -Encoding UTF8 "$dir/g7_complex_struct.lz"

# G8: bigint 作 map/dict key
@'
def main() =
    let m: Dict<bigint, int> = { BigInt::from(1): 10, BigInt::from(2): 20 }
    println(m)
'@ | Set-Content -Encoding UTF8 "$dir/g8_dict_bigint.lz"

Write-Host "probes written"
