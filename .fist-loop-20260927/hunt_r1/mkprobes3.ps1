$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$p = Join-Path $root "probes"
$enc = New-Object System.Text.UTF8Encoding($false)
$cases = @{}
$cases["m19_undefined_fn"]  = "def main() =`n    println(foo())"
$cases["m20_undefined_var"] = "def main() =`n    println(zzz)"
$cases["m21_bad_method"]    = "def main() =`n    let s = ""abc""`n    println(s.nosuch())"
$cases["m22_wrong_arity"]   = "def f(a: int) -> int =`n    a + 1`ndef main() =`n    println(f(1, 2))"
$cases["m23_str_idx_neg"]   = "def main() =`n    let s = ""abc""`n    println(s[0 - 1])"
$cases["m24_bigint_from_huge"] = "let x: bigint = " + ("7" * 60) + "`nprintln(x)"
foreach ($k in $cases.Keys) {
  $txt = $cases[$k] -replace '`n', "`n"
  [System.IO.File]::WriteAllText((Join-Path $p "$k.lz"), $txt, $enc)
}
"created " + $cases.Count
