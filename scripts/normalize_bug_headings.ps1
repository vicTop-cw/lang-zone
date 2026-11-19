# 账本标题规范化（BUG-516）：把 `## BUG-N [ts] [sev] <任意尾注>` 统一为
# `## BUG-N [ts] [sev] STATUS`，原尾注转为正文 `- note:` 行（信息不丢）。
# 子标题 `## BUG-N 修复追记 ...` 降级为正文 `- 修复追记 ...`。
# -DryRun 只打印将改动的行数与样例，不写文件。
param([switch]$DryRun)
$ErrorActionPreference = "Stop"
$p = Join-Path (Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)) "memory\bugs.md"
$lines = Get-Content $p -Encoding UTF8
$statuses = @("OPEN", "FIXED", "REVERTED", "DUPLICATE", "FALSE_POSITIVE")
$out = New-Object System.Collections.Generic.List[string]
$changed = 0
$subHeadings = 0
foreach ($l in $lines) {
    $m = [regex]::Match($l, '^## (BUG-\d+) \[([^\]]+)\] \[([^\]]+)\](.*)$')
    if ($m.Success) {
        $id = $m.Groups[1].Value; $ts = $m.Groups[2].Value; $sev = $m.Groups[3].Value; $tail = $m.Groups[4].Value.Trim()
        if ($tail -match '^(OPEN|FIXED|REVERTED|DUPLICATE|FALSE_POSITIVE)\s*$') { $out.Add($l); continue }
        $st = $null
        if ($tail -match '^(OPEN|FIXED|REVERTED|DUPLICATE|FALSE_POSITIVE)') { $st = $Matches[1] }
        if (-not $st) { $st = "OPEN" }
        $out.Add("## $id [$ts] [$sev] $st")
        if ($tail) { $out.Add("- note: $tail") }
        $changed++
        continue
    }
    $m2 = [regex]::Match($l, '^## (BUG-\d+)\s+(.+)$')
    if ($m2.Success -and $l -notmatch '^## BUG-\d+ \[') {
        $out.Add("- " + $l.Substring(3))
        $subHeadings++
        continue
    }
    $out.Add($l)
}
Write-Host "将改标题: $changed 条；降级子标题: $subHeadings 条"
if (-not $DryRun) {
    $prev = @{}
    $ln = 0
    foreach ($x in $out) { $ln++; if ($x -match '^## (BUG-\d+) \[') { $prev[$Matches[1]] = ($prev[$Matches[1]] + 0) + 1 } }
    $dup = ($prev.GetEnumerator() | Where-Object { $_.Value -gt 1 } | ForEach-Object { $_.Key })
    if ($dup) { Write-Host "重复编号: $($dup -join ', ')" -ForegroundColor Yellow }
    [IO.File]::WriteAllLines($p, $out, (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "已写回 $p（$($out.Count) 行）"
}