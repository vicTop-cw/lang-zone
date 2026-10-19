# Round1 bug-hunt: reproduce BUG-1..5 (hunt-20260926)
$ErrorActionPreference = "Continue"
Set-Location E:\IDEProjects\AI\lang-zone
$exe = "target\debug\lang-zone.exe"
$lz  = (Get-ChildItem "target\debug\deps\liblz_builtins-*.rlib" | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
$nb  = (Get-ChildItem "target\debug\deps\libnum_bigint-*.rlib"  | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
"rlib lz_builtins: $lz"
"rlib num_bigint : $nb"
$tmp = ".fist-wf-20260926\tmp"
New-Item -ItemType Directory -Force $tmp | Out-Null

function Repro($dir, $file, $extra) {
  $name = [IO.Path]::GetFileNameWithoutExtension($file)
  "== $dir/$file =="
  & $exe "FIND_BUG\hunt-20260926\$dir\$file" 2>&1 | Select-Object -Last 2
  $rs = "FIND_BUG\hunt-20260926\$dir\$name.rs"
  if (-not (Test-Path $rs)) { "TRANSLATE FAILED (no .rs)"; return "TFAIL" }
  Copy-Item $rs "$tmp\$name.rs" -Force
  $out = "$tmp\$name.rlib"
  $args = @("--edition","2021","--crate-type","lib","-L","dependency=target/debug/deps","--extern","lz_builtins=$lz") + $extra + @($rs, "-o", $out)
  $o = & rustc @args 2>&1 | Out-String
  if ($LASTEXITCODE -eq 0) { "COMPILE OK" ; return "OK" }
  else { ($o -split "`n" | Where-Object { $_ -match "^error" } | Select-Object -First 3); return "CFAIL" }
}

$r1 = Repro "bug-A-numcrate-import-e0432" "bigint_min.lz" @()
$r1c = Repro "bug-A-numcrate-import-e0432" "complex_min.lz" @()
$r2 = Repro "bug-B-bigint-const-no-from-wrap-e0308" "bigint_min.lz" @("--extern","num_bigint=$nb")
$r3 = Repro "bug-C-bigint-const-nonconst-op-e0015" "bigint_min.lz" @("--extern","num_bigint=$nb")
$r4 = Repro "bug-D-bigint-fnbody-type-collapse" "bigint_fnbody.lz" @("--extern","num_bigint=$nb")
$r5 = Repro "bug-E-list3-empty-nested-pushdown-e0308" "rv_list3.lz" @()
"SUMMARY A=$r1 A-cplx=$r1c B=$r2 C=$r3 D=$r4 E=$r5"
