$ErrorActionPreference = 'Continue'
$exe = "e:\IDEProjects\AI\lang-zone\target\debug\lang-zone.exe"
$files = @()
$files += Get-ChildItem "e:\IDEProjects\AI\lang-zone\DEMO" -Recurse -Filter *.lz -ErrorAction SilentlyContinue
$files += Get-ChildItem "e:\IDEProjects\AI\lang-zone\corpus" -Recurse -Filter *.lz -ErrorAction SilentlyContinue
$files = $files | Where-Object { $_.FullName -notmatch '\\99_errors\\' -and $_.FullName -notmatch '\\lz_std\\' }
$uniq = @{}
foreach ($f in $files) {
  $o = (& $exe $f.FullName 2>&1 | Out-String)
  foreach ($m in [regex]::Matches($o, "Semantic error: ([^\r\n]*)")) {
    $line = $m.Groups[1].Value -replace "\u001b\[[0-9;]*m", ""
    $uniq[$line] = 1
  }
}
"files: " + $files.Count + "  uniq-semantic-errors: " + $uniq.Count
$uniq.Keys | Sort-Object
