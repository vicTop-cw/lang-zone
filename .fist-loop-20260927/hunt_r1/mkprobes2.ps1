$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$p = Join-Path $root "probes"
New-Item -ItemType Directory -Force -Path $p | Out-Null
$enc = New-Object System.Text.UTF8Encoding($false)

$cases = @{}
$cases["m01_lone_brace"]    = "{"
$cases["m02_unterm_str"]    = "let s = ""abc"
$cases["m03_let_alone"]     = "let"
$cases["m04_def_alone"]     = "def"
$cases["m05_lone_op"]       = "+"
$cases["m06_empty_rhs"]     = "let x ="
$cases["m07_bad_indent"]    = "def main() =`nlet x = 1`n    println(x)"
$cases["m08_unclosed_list"] = "let a = [1, 2,"
$cases["m09_unclosed_def"]  = "def f("
$cases["m10_int_i128max_p1"] = "let x = 170141183460469231731687303715884105728`nprintln(x)"
$cases["m11_huge_200dig"]   = "let x = " + ("9" * 200) + "`nprintln(x)"
$cases["m12_div_zero"]      = "def main() =`n    let a = 10`n    let b = 0`n    println(a / b)"
$cases["m13_mod_zero"]      = "def main() =`n    let a = 10`n    let b = 0`n    println(a % b)"
$cases["m14_list_oob"]      = "def main() =`n    let xs = [1, 2, 3]`n    println(xs[10])"
$cases["m15_str_oob"]       = "def main() =`n    let s = ""abc""`n    println(s[10])"
$cases["m16_neg_zero_idx"]  = "def main() =`n    let xs = [1, 2, 3]`n    println(xs[0 - 1])"

# 深嵌套列表（200 层）
$sb = New-Object System.Text.StringBuilder
for ($i = 0; $i -lt 200; $i++) { [void]$sb.Append("[") }
[void]$sb.Append("1")
for ($i = 0; $i -lt 200; $i++) { [void]$sb.Append("]") }
$cases["m17_deep_list200"] = "let a = " + $sb.ToString() + "`nprintln(a)"

# 深嵌套括号表达式（500 层）
$sb2 = New-Object System.Text.StringBuilder
for ($i = 0; $i -lt 500; $i++) { [void]$sb2.Append("(") }
[void]$sb2.Append("1 + 2")
for ($i = 0; $i -lt 500; $i++) { [void]$sb2.Append(")") }
$cases["m18_deep_paren500"] = "def main() =`n    let x = " + $sb2.ToString() + "`n    println(x)"

foreach ($k in $cases.Keys) {
  $txt = $cases[$k] -replace '`n', "`n"
  [System.IO.File]::WriteAllText((Join-Path $p "$k.lz"), $txt, $enc)
}
"created " + $cases.Count + " malformed probes"
