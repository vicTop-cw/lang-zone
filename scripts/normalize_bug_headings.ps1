# 账本标题规范化（BUG-516 + 迭代-14:00 增强）：把历史遗留的非规范标题统一为
# `## BUG-N [ts] [sev] STATUS`，原尾注/描述转为正文 `- note:` 行（信息不丢）。
#
# 覆盖两类形态：
#   A) `## BUG-N [ts] [sev] <任意尾注>` → 状态位取自尾注首个 STATUS 词；缺失记 OPEN。
#   B) `## BUG-N [sev] <描述>`（缺 timestamp 的单括号形态，迭代前的历史遗留）
#      → timestamp 取正文 `发现:`/`[YYYY-...]` 或占位 1970-01-01T00:00:00Z；
#        状态位取正文 `status:`/`状态：`/`状态:` 行首个 STATUS 词；缺失记 OPEN。
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
$samples = New-Object System.Collections.Generic.List[string]

# 预扫描：为每个 `## BUG-N` 标题找紧随其后的正文状态位
function Get-BodyStatus([string[]]$all, [int]$startIdx) {
    # 逐行扫正文，取首个含 STATUS 词的 `status:`/`状态：` 行；
    # 支持箭头形态 `OPEN -> FIXED`：取箭头右侧的终态（最后一个 STATUS 词）。
    for ($j = $startIdx + 1; $j -lt $all.Count; $j++) {
        if ($all[$j] -match '^## BUG-\d+') { break }
        $sm = [regex]::Match($all[$j], '(?i)(?:status|状态)\s*[:：]\s*(.+)$')
        if ($sm.Success) {
            $words = [regex]::Matches($sm.Groups[1].Value, '\b(OPEN|FIXED|REVERTED|DUPLICATE|FALSE_POSITIVE)\b')
            if ($words.Count -gt 0) {
                # 箭头形态取最后一个；单值形态即该值
                return $words[$words.Count - 1].Value
            }
        }
    }
    return $null
}

for ($i = 0; $i -lt $lines.Count; $i++) {
    $l = $lines[$i]
    # 形态 A：`## BUG-N [ts] [sev] <tail>`
    $m = [regex]::Match($l, '^## (BUG-\d+[a-z]?) \[([^\]]+)\] \[([^\]]+)\](.*)$')
    if ($m.Success) {
        $id = $m.Groups[1].Value; $ts = $m.Groups[2].Value; $sev = $m.Groups[3].Value; $tail = $m.Groups[4].Value.Trim()
        if ($tail -match '^(OPEN|FIXED|REVERTED|DUPLICATE|FALSE_POSITIVE)\s*$') { $out.Add($l); continue }
        $st = $null
        if ($tail -match '^(OPEN|FIXED|REVERTED|DUPLICATE|FALSE_POSITIVE)') { $st = $Matches[1] }
        if (-not $st) { $st = Get-BodyStatus $lines $i }
        if (-not $st) { $st = "OPEN" }
        $out.Add("## $id [$ts] [$sev] $st")
        if ($tail) { $out.Add("- note: $tail") }
        if ($samples.Count -lt 12) { $samples.Add("  $l  =>  ## $id [$ts] [$sev] $st") }
        $changed++
        continue
    }
    # 形态 B：`## BUG-N [sev] <desc>`（缺 timestamp）
    $mB = [regex]::Match($l, '^## (BUG-\d+[a-z]?) \[(critical|high|medium|low)\] (.+)$')
    if ($mB.Success) {
        $id = $mB.Groups[1].Value; $sev = $mB.Groups[2].Value; $desc = $mB.Groups[3].Value.Trim()
        $st = Get-BodyStatus $lines $i
        if (-not $st) {
            # 无显式状态行：正文含「修复」或「回归测试」且含 commit ⇒ 视为已修（FIXED）。
            $body = New-Object System.Collections.Generic.List[string]
            for ($j = $i + 1; $j -lt $lines.Count; $j++) {
                if ($lines[$j] -match '^## BUG-\d+') { break }
                $body.Add($lines[$j])
            }
            $bodyText = ($body -join "`n")
            $hasFix = ($bodyText -match '修复')
            $hasTest = ($bodyText -match '回归测试')
            $hasCommit = ($bodyText -match '(?i)commit')
            if (($hasFix -or $hasTest) -and $hasCommit) { $st = "FIXED" }
        }
        if (-not $st) { $st = "OPEN" }
        # timestamp：从正文 `发现:` 或 `[YYYY-MM-DD...]` 提取，否则占位
        $ts = "1970-01-01T00:00:00Z"
        for ($j = $i + 1; $j -lt $lines.Count; $j++) {
            if ($lines[$j] -match '^## BUG-\d+') { break }
            $tm = [regex]::Match($lines[$j], '(\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}[^\]\s]*)?)')
            if ($tm.Success) {
                $raw = $tm.Groups[1].Value
                if ($raw -notmatch 'T') { $raw = $raw + "T00:00:00Z" }
                elseif ($raw -notmatch 'Z$' -and $raw -notmatch '[+-]\d{2}:\d{2}$') { $raw = $raw + "Z" }
                $ts = $raw
                break
            }
        }
        $out.Add("## $id [$ts] [$sev] $st")
        $out.Add("- note: $desc")
        if ($samples.Count -lt 12) { $samples.Add("  $l  =>  ## $id [$ts] [$sev] $st") }
        $changed++
        continue
    }
    # 子标题降级
    $m2 = [regex]::Match($l, '^## (BUG-\d+[a-z]?)\s+(.+)$')
    if ($m2.Success -and $l -notmatch '^## BUG-\d+[a-z]? \[') {
        $out.Add("- " + $l.Substring(3))
        $subHeadings++
        continue
    }
    $out.Add($l)
}
Write-Host "将改标题: $changed 条；降级子标题: $subHeadings 条"
if ($samples.Count -gt 0) { Write-Host "样例（前 $($samples.Count) 条）："; $samples | ForEach-Object { Write-Host $_ } }
if (-not $DryRun) {
    $prev = @{}
    foreach ($x in $out) { if ($x -match '^## (BUG-\d+[a-z]?) \[') { $prev[$Matches[1]] = ($prev[$Matches[1]] + 0) + 1 } }
    $dup = ($prev.GetEnumerator() | Where-Object { $_.Value -gt 1 } | ForEach-Object { $_.Key })
    if ($dup) { Write-Host "重复编号: $($dup -join ', ')" -ForegroundColor Yellow }
    [IO.File]::WriteAllLines($p, $out, (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "已写回 $p（$($out.Count) 行）"
}
