$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$p = Join-Path $root "probes"
New-Item -ItemType Directory -Force -Path $p | Out-Null
$enc = New-Object System.Text.UTF8Encoding($false)

$cases = @{
"p01_dictcomp_var" = @'
def main() =
    let xs = [1, 2, 3]
    let d = {"k": x for x in xs}
    println(d)
'@
"p02_dictcomp_lit" = @'
let d = {"a": 1, "b": 2}
println(d)
'@
"p03_bigint_neg" = @'
let x: bigint = -12345678901234567890
println(x)
'@
"p04_bigint_divmod" = @'
let x: bigint = 12345678901234567890
let y: bigint = x / 7
let z: bigint = x % 7
println(y)
println(z)
'@
"p05_bigint_cmp" = @'
let x: bigint = 12345678901234567890
let y: bigint = 12345678901234567891
println(x < y)
println(x == y)
'@
"p06_bigint_fnparam" = @'
def addbig(a: bigint, b: bigint) -> bigint =
    a + b
def main() =
    println(addbig(12345678901234567890, 1))
'@
"p07_complex_arith" = @'
let z: complex = 1.0 + 2.0i
let w: complex = 3.0 - 4.0i
println(z + w)
println(z * w)
'@
"p08_bigint_mix_int" = @'
let x: bigint = 12345678901234567890
let y = x + 1
println(y)
'@
"p09_deep_list4" = @'
let a: List<List<List<List<int>>>> = [[], [[]], [[[]]]]
println(a)
'@
"p10_empty" = @'
'@
"p11_unicode" = @'
def main() =
    let s = "中文字符串 hello"
    println(s.len())
    println(s)
'@
"p12_bigint_struct" = @'
struct Big = val: bigint
def main() =
    let b = Big { val: 123456789012345678901234567890 }
    println(b.val + 1)
'@
"p13_trunc_paren" = @'
def main() =
    let x = (1 + 2
    println(x)
'@
"p14_deep_paren" = @'
def main() =
    let x = ((((((((((1 + 2))))))))))
    println(x)
'@
"p15_bigint_toplevel_print" = @'
let x: bigint = 99999999999999999999999999999999999999
println(x * x)
'@
}

foreach ($k in $cases.Keys) {
  [System.IO.File]::WriteAllText((Join-Path $p "$k.lz"), $cases[$k], $enc)
}
"created " + $cases.Count + " probes"
