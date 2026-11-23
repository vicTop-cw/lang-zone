# 账本一致性门禁（BUG 台账 memory/bugs.md）
#
# 规则：每条 `## BUG-N [ts] [sev] <STATUS>` 的标题状态位，必须与正文 `状态：<STATUS>` 一致。
#       （正文可带尾注，如 `FIXED（盘点完成）`，按其前缀判定。）
# 背景：历史遗留条目存在「标题 OPEN / 正文 FIXED」错位（BUG-497~509 一批，2026-11-23 收口），
#       本门禁防止再次漂移。
#
# 用法：powershell -NoProfile -File scripts/bug_ledger_consistency.ps1
# 退出码：0 = 全部一致；1 = 发现不一致（打印条目）。
$ErrorActionPreference = "Stop"
$p = Join-Path (Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)) "memory\bugs.md"
if (-not (Test-Path $p)) { Write-Output "FAIL: bugs.md not found at $p"; exit 1 }

$lines = Get-Content $p -Encoding UTF8
$statuses = @("OPEN", "FIXED", "REVERTED", "DUPLICATE", "FALSE_POSITIVE")

$entries = @()
for ($i = 0; $i -lt $lines.Count; $i++) {
    $m = [regex]::Match($lines[$i], '^## (BUG-\d+) \[([^\]]+)\] \[([^\]]+)\]\s+(\S+)\s*$')
    if ($m.Success) {
        $entries += [pscustomobject]@{
            Id     = $m.Groups[1].Value
            Status = $m.Groups[4].Value
            Line   = $i
        }
    }
}

$mismatch = @()
foreach ($e in $entries) {
    if ($statuses -notcontains $e.Status) { continue }
    # 从标题行向下扫，直到下一条 `## BUG-` 标题，找第一条 `状态：`
    $bodyStatus = $null
    for ($j = $e.Line + 1; $j -lt $lines.Count; $j++) {
        if ($lines[$j] -match '^## BUG-\d+') { break }
        $sm = [regex]::Match($lines[$j], '状态[：:]\s*(\S+)')
        if ($sm.Success) { $bodyStatus = $sm.Groups[1].Value; break }
    }
    if ($null -eq $bodyStatus) { continue }
    $bodyBase = ($bodyStatus -split '[（(]')[0].Trim()
    if ($bodyBase -ne $e.Status) {
        $mismatch += [pscustomobject]@{ Id = $e.Id; Title = $e.Status; Body = $bodyBase }
    }
}

Write-Output "账本条目（带标题状态位）: $($entries.Count)"
Write-Output "标题/正文不一致: $($mismatch.Count)"
foreach ($x in $mismatch) {
    Write-Output ("  不一致: {0} 标题={1} 正文={2}" -f $x.Id, $x.Title, $x.Body)
}
if ($mismatch.Count -eq 0) { Write-Output "PASS"; exit 0 } else { Write-Output "FAIL"; exit 1 }
