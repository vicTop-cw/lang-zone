$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$p = Join-Path $root "probes"
$enc = New-Object System.Text.UTF8Encoding($false)
$bom = New-Object System.Text.UTF8Encoding($true)

$cases = @{}
$cases["n01_bom"]            = "def main() =`n    println(1)"
$cases["n02_crlf"]           = "def main() =`r`n    println(2)`r`n"
$cases["n03_tab_indent"]     = "def main() =`n`tprintln(3)"
$cases["n04_mixed_indent"]   = "def main() =`n`t    println(4)"
$cases["n05_escapes"]        = "def main() =`n    let s = ""a`nb`"c\\d`te""`n    println(s)"
$cases["n06_num_literals"]   = "def main() =`n    let a = 0x1F`n    let b = 0b1010`n    let c = 1_000`n    let d = 1e10`n    let e = 1.5e-3`n    println(a)`n    println(b)`n    println(c)`n    println(d)`n    println(e)"
$cases["n07_dot_float"]      = "def main() =`n    let a = .5`n    let b = 5.`n    println(a)`n    println(b)"
$cases["n08_long_ident"]     = "def main() =`n    let " + ("z" * 900) + " = 8`n    println(" + ("z" * 900) + ")"
$cases["n09_keyword_ident"]  = "def main() =`n    let let = 9`n    println(1)"
$cases["n10_return_top"]     = "return 1"
$cases["n11_deep_if"]        = "def main() =`n    let x = 1`n" + (("    if x > 0:`n        x = x`n") * 1) + "    println(x)"
$cases["n12_float_div_zero"] = "def main() =`n    let a = 1.0`n    let b = 0.0`n    println(a / b)"
$cases["n13_neg_zero"]       = "def main() =`n    let a = 0.0 - 0.0`n    println(a)`n    println(0 - 0)"
$cases["n14_comment_edge"]   = "// only a comment`n"
$cases["n15_semicolon"]      = "def main() =`n    println(1); println(2)"

foreach ($k in $cases.Keys) {
  $txt = $cases[$k] -replace '`n', "`n" -replace '`t', "`t" -replace '`r', "`r"
  $useEnc = if ($k -eq "n01_bom") { $bom } else { $enc }
  [System.IO.File]::WriteAllText((Join-Path $p "$k.lz"), $txt, $useEnc)
}
"created " + $cases.Count
