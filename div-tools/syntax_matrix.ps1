#!/usr/bin/env powershell
#Requires -Version 5.1
# ============================================================
# lz 语法特性正反例测试矩阵（判据 v2）
#
# 本机只有 Windows PowerShell 5.1（powershell.exe），没有 pwsh 6/7，
# 所以入口按 powershell 对齐；脚本内也只使用 5.1 兼容语法。
#
# 运行：
#   powershell -NoProfile -ExecutionPolicy Bypass -File div-tools\syntax_matrix.ps1
#   powershell -NoProfile -File div-tools\syntax_matrix.ps1 -SelfTest
#   powershell -NoProfile -File div-tools\syntax_matrix.ps1 -LzPath target\lane-matrix\debug\lang-zone.exe
#   也支持环境变量 LZ_BIN 指定二进制；默认 target\debug\lang-zone.exe。
#
# 判据（v2）：
#   正例：rc == 0。
#   正例内容断言（P3）：对 5 个代表性特性，rc == 0 且本次生成的 .rs 含预期构造
#         （正则见 $script:ContentChecks）；不命中记「正例内容不符」。
#   反例：rc != 0 且 输出里有「行首诊断」（正则见 $script:DiagHeadPattern），
#         并排除 NativeCommandError / LZ LINK RECIPE / Generated 路径回显 /
#         Bridge registry / 提示：/ warning / note 这些噪声。
#   采集：子进程 stdout、stderr 各自重定向到独立临时文件（不经 PowerShell 管道），
#         再用 Get-Content -Raw -Encoding UTF8 读回。这样 PS 5.1 不会把 stderr
#         逐行包成 ErrorRecord（NativeCommandError），也不会用 GBK 解码吞字节。
#
# v1 旧判据（已废弃，勿回退）：
#   $errOut = & $LZ $probe.FullName --std-dir $STD 2>&1 | Out-String
#   $readable = $errOut -match 'error'
#   —— 恒真：任何产生 stderr 的样本都被包出 NativeCommandError 字样，判据全命中。
#
# 退出码：0 全部通过；1 有意外结果；2 缺二进制
# ============================================================
param(
    [string]$LzPath = $env:LZ_BIN,
    [switch]$SelfTest
)

$ErrorActionPreference = 'Continue'

$ROOT = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($LzPath)) {
    $LzPath = Join-Path $ROOT 'target\debug\lang-zone.exe'
}
elseif (-not [System.IO.Path]::IsPathRooted($LzPath)) {
    $LzPath = Join-Path $ROOT $LzPath
}
$LZ = $LzPath
$STD = Join-Path $ROOT 'std'
$PROBES = Join-Path $PSScriptRoot 'syntax_probes'
$REPORT = Join-Path $ROOT 'tests\syntax-matrix-report.md'

# ---------- 判据正则（单一真源）----------
# 行首诊断：只有这些才算「诊断可读」。
$script:DiagHeadPattern = '^(?:Parse error|IR build error|Macro definition error|Semantic error|Lexer error|Type error|Codegen error|Link error|Compile error|Error|error\[[0-9A-Za-z]+\])'
# 噪声：PS 5.1 的 stderr 包装、链接配方、路径回显、普通提示。
$script:DiagDenyPattern = '(?i:NativeCommandError|FullyQualifiedErrorId|LZ LINK RECIPE)|^(?i:Generated)\s|^(?i:Bridge registry)|^\s*(?:提示|注意)[：:]|^\s*(?i:warning|note|info|help)\b'

# ============================================================
# 工具函数
# ============================================================

function Format-Short {
    param([string]$Text, [int]$Max = 120)
    if ([string]::IsNullOrEmpty($Text)) { return '' }
    $t = $Text.Trim()
    $t = $t -replace '[\r\n\t]+', ' '
    if ($t.Length -gt $Max) { return $t.Substring(0, $Max) + '…' }
    return $t
}

function Escape-Cell {
    param([string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return '—' }
    return ($Text -replace '\|', '\|')
}

function Get-FirstNonEmptyLine {
    param([string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return '' }
    foreach ($l in ($Text -split "`r?`n")) {
        if ($l.Trim()) { return $l.Trim() }
    }
    return ''
}

function Read-TextUtf8 {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return '' }
    if ((Get-Item -LiteralPath $Path).Length -le 0) { return '' }
    # -Raw + -Encoding UTF8：整段读，按 UTF-8 解码，不经管道、不按行分类。
    $text = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
    if ($null -eq $text) { return '' }
    return [string]$text
}

function Invoke-LzCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Exe,
        [string[]]$ArgList = @(),
        [string]$WorkDir = ''
    )
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    try {
        $quoted = @()
        foreach ($a in $ArgList) {
            if ($a -match '[\s"]') { $quoted += ('"' + ($a -replace '"', '\"') + '"') }
            else { $quoted += $a }
        }
        $argLine = ($quoted -join ' ')
        $startArgs = @{
            FilePath               = $Exe
            ArgumentList           = $argLine
            RedirectStandardOutput = $outFile
            RedirectStandardError  = $errFile
            NoNewWindow            = $true
            Wait                   = $true
            PassThru               = $true
        }
        if ($WorkDir) { $startArgs['WorkingDirectory'] = $WorkDir }
        $proc = Start-Process @startArgs -ErrorAction Stop
        $outBytes = (Get-Item -LiteralPath $outFile).Length
        $errBytes = (Get-Item -LiteralPath $errFile).Length
        return [pscustomobject]@{
            Rc         = $proc.ExitCode
            StdOut     = (Read-TextUtf8 -Path $outFile)
            StdErr     = (Read-TextUtf8 -Path $errFile)
            StdOutBytes = $outBytes
            StdErrBytes = $errBytes
        }
    }
    finally {
        Remove-Item -LiteralPath $outFile, $errFile -Force -ErrorAction SilentlyContinue
    }
}

function Get-LzDiagnostic {
    param([string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return $null }
    foreach ($line in ($Text -split "`r?`n")) {
        $t = $line.Trim()
        if (-not $t) { continue }
        if ($t -match $script:DiagDenyPattern) { continue }
        if ($t -match $script:DiagHeadPattern) { return (Format-Short -Text $t -Max 160) }
    }
    return $null
}

# 反例判定单一入口：报告与 -SelfTest 都调它，不许各写一份。
function Test-NegativeVerdict {
    param([Parameter(Mandatory = $true)]$Capture)
    $combined = ([string]$Capture.StdOut + "`n" + [string]$Capture.StdErr)
    $diag = Get-LzDiagnostic -Text $combined
    $readable = ($null -ne $diag)
    $rcOk = ($Capture.Rc -ne 0)
    $pass = ($rcOk -and $readable)
    $reason = ''
    if (-not $rcOk) { $reason = 'rc=0（编译未失败）' }
    elseif (-not $readable) { $reason = ('rc={0} 但输出里没有行首诊断（诊断不可读）' -f $Capture.Rc) }
    return [pscustomobject]@{
        Pass     = $pass
        Rc       = $Capture.Rc
        Readable = $readable
        Diag     = $diag
        Reason   = $reason
    }
}

# 正例内容断言的单一入口：判定一份生成物里是否有全部预期构造。
# 空文件/文件不存在一律判不通过（防「读不到就当通过」）。
function Test-ContentVerdict {
    param(
        [Parameter(Mandatory = $true)][string]$RsPath,
        [Parameter(Mandatory = $true)][string[]]$Patterns
    )
    $text = Read-TextUtf8 -Path $RsPath
    $hits = @()
    $misses = @()
    foreach ($pat in $Patterns) {
        if ($text -match $pat) { $hits += $pat } else { $misses += $pat }
    }
    return [pscustomobject]@{
        Pass   = (($misses.Count -eq 0) -and ($text.Length -gt 0))
        Hits   = $hits
        Misses = $misses
        Length = $text.Length
    }
}

# ============================================================
# 特性清单（SYNTAX 36 文档 → 特性）
# ============================================================
$features = @(
    @{ n = '词法-缩进块'; d = '00'; pos = '01_basics\control_flow.lz' },
    @{ n = '词法-字符串/转义'; d = '00'; pos = '01_basics\strings.lz' },
    @{ n = '词法-数字字面量'; d = '00'; pos = '01_basics\literals.lz' },
    @{ n = '类型-基础类型'; d = '01'; pos = '02_types\primitives.lz' },
    @{ n = '类型-容器'; d = '01'; pos = '02_types\containers.lz' },
    @{ n = '类型-类型标注'; d = '01'; pos = '03_variables\const.lz' },
    @{ n = 'duck 关系约束'; d = '01b'; pos = '07_data_structures\duck_typing.lz' },
    @{ n = '变量-不可变绑定'; d = '02'; pos = '03_variables\const.lz' },
    @{ n = '变量-可变绑定'; d = '02'; pos = '03_variables\mutable_let.lz' },
    @{ n = '变量-引用绑定'; d = '02'; pos = '03_variables\ref_binding.lz' },
    @{ n = '变量-海象'; d = '02'; pos = '03_variables\walrus.lz' },
    @{ n = '函数-基础'; d = '03'; pos = '04_functions\basic.lz' },
    @{ n = '函数-泛型'; d = '03b'; pos = '04_functions\generics.lz' },
    @{ n = '函数-复合'; d = '03e'; pos = '04_functions\composite.lz' },
    @{ n = '检查站 checker'; d = '03c'; pos = '06_control_flow\def_checker.lz' },
    @{ n = '可变参数'; d = '03d'; pos = '04_functions\varargs.lz' },
    @{ n = '闭包'; d = '03e'; pos = '01_basics\functions_advanced.lz' },
    @{ n = '表达式-管道'; d = '04'; pos = '05_expressions\pipe.lz' },
    @{ n = '表达式-三元'; d = '04'; pos = '05_expressions\ternary.lz' },
    @{ n = '表达式-推导式'; d = '04'; pos = '05_expressions\comprehension.lz' },
    @{ n = '控制流-if/elif'; d = '05'; pos = '06_control_flow\if_elif_else.lz' },
    @{ n = '控制流-循环'; d = '05'; pos = '06_control_flow\for_while_loop.lz' },
    @{ n = '控制流-break/continue'; d = '05'; pos = '06_control_flow\break_continue.lz' },
    @{ n = '控制流-guard'; d = '05'; pos = '06_control_flow\guard.lz' },
    @{ n = 'block 命名块'; d = '05b'; pos = '06_control_flow\block_demo.lz' },
    @{ n = '数据结构-struct'; d = '06a'; pos = '07_data_structures\struct.lz' },
    @{ n = '数据结构-enum'; d = '06b'; pos = '01_basics\enums.lz' },
    @{ n = 'trait/impl'; d = '06c'; pos = '07_data_structures\trait_impl.lz' },
    @{ n = '魔法方法'; d = '06f'; pos = '07_data_structures\magic_methods.lz' },
    @{ n = '自引用 Self'; d = '06h'; pos = '07_data_structures\self_recursive.lz' },
    @{ n = '模块与导入'; d = '07'; pos = '08_modules\import_demo.lz' },
    @{ n = '宏'; d = '08'; pos = '09_macros\macro_demo.lz' },
    @{ n = 'comptime'; d = '08b'; pos = '09_macros\comptime_demo.lz' },
    @{ n = '错误处理'; d = '09'; pos = '10_error_handling\panic_raise_try.lz' },
    @{ n = '并发异步'; d = '10'; pos = '11_concurrency\async_spawn.lz' },
    @{ n = '构建块'; d = '11'; pos = '12_build_blocks\var_call_block.lz' },
    @{ n = '操作符'; d = '12'; pos = '13_operators\compound_assign_more.lz' },
    @{ n = '指针与引用'; d = '13'; pos = '14_pointers\box_rc_arc.lz' },
    @{ n = '生成器'; d = '14'; pos = '15_generators\yield_demo.lz' },
    @{ n = '测试框架'; d = '15'; pos = '16_testing\test_suite.lz' }
)

# ---------- 正例内容断言表（P3，单一真源）----------
# 每行 = { tag 特性名; pos DEMO 相对路径; pat 生成 .rs 里必须全部命中的正则 }。
# 断言口径：rc == 0 且 .rs 是本次编译产物 且 全部 pat 命中；任一不满足 = 「正例内容不符」。
# 只收「生成物里有稳定构造」的特性；不合适的在 $script:RcOnlyNotes 里显式标注「仅 rc 判定」。
# 新增/调整断言只改这张表（以及必要的 RcOnlyNotes 注释），不要改判定函数。
$script:ContentChecks = @(
    @{ tag = 'struct';     pos = '07_data_structures\struct.lz';      pat = @('pub struct\s+Point\b', 'pub struct\s+Pair<T>') },
    @{ tag = 'enum';       pos = '01_basics\enums.lz';                pat = @('pub enum\s+Color\b', 'pub enum\s+Shape\b') },
    @{ tag = 'match';      pos = '06_control_flow\match.lz';          pat = @('\bmatch\s+\w+') },
    @{ tag = 'closure';    pos = '01_basics\functions_advanced.lz';   pat = @('Arc<dyn Fn\(i64\) -> i64', 'Arc::new\(move ') },
    @{ tag = 'trait/impl'; pos = '07_data_structures\trait_impl.lz';  pat = @('pub trait\s+Drawable\b', 'impl\s+Drawable\s+for\s+Rectangle\b') }
)

# 显式标注「仅 rc 判定」的特性：这些正例不适合做内容断言，报告中单列原因。
# 其余没进 $ContentChecks 的特性同样「仅 rc 判定」，报告里按「未配置内容断言」汇总。
$script:RcOnlyNotes = @{
    '词法-缩进块'     = '缩进是解析层语法，生成物里没有可断言的独立构造'
    '词法-字符串/转义' = '字符串字面量被直接内联，生成物形态随内容变化，无稳定构造'
    '词法-数字字面量'  = '数字字面量被直接内联，生成物形态随数值变化，无稳定构造'
}

$psVersion = $PSVersionTable.PSVersion.ToString()

if (-not (Test-Path -LiteralPath $LZ)) {
    Write-Host ("[ERROR] 缺少二进制：{0}" -f $LZ) -ForegroundColor Red
    Write-Host '        用 -LzPath 或环境变量 LZ_BIN 指定（默认 target\debug\lang-zone.exe）'
    exit 2
}

# ============================================================
# -SelfTest：判据自检（不写报告）
# ============================================================
if ($SelfTest) {
    Write-Host '===== -SelfTest 判据自检（不写报告）====='
    Write-Host ("二进制：{0}" -f $LZ)
    Write-Host ("shell ：{0}（powershell.exe）" -f $psVersion)
    $fails = @()

    # --- 证据 1：编译成功但 stderr 有输出的样本，新判据必须判 False（v1 判 True）---
    $sample = $null
    $sampleCap = $null
    foreach ($f in $features) {
        $p = Join-Path $ROOT ('DEMO\' + $f.pos)
        if (-not (Test-Path -LiteralPath $p)) { continue }
        $c = Invoke-LzCapture -Exe $LZ -ArgList @($p, '--std-dir', $STD) -WorkDir $ROOT
        if ($c.Rc -eq 0 -and $c.StdErrBytes -gt 0) { $sample = $p; $sampleCap = $c; break }
    }
    if (-not $sample) {
        Write-Host '[SELFTEST FAIL] 找不到「编译成功但 stderr 非空」的样本' -ForegroundColor Red
        exit 1
    }
    $oldMerged = (& $LZ $sample --std-dir $STD 2>&1 | Out-String)
    $oldMatch = [bool]($oldMerged -match 'error')
    $oldNative = [bool]($oldMerged -match 'NativeCommandError')
    $newVerdict = Test-NegativeVerdict -Capture $sampleCap

    Write-Host ''
    Write-Host '[1] 编译成功但 stderr 有输出的样本（缺陷 1 的回归证据）'
    Write-Host ("    样本 = {0}" -f (Split-Path -Leaf $sample))
    Write-Host ("    rc = {0}；stderr 字节 = {1}" -f $sampleCap.Rc, $sampleCap.StdErrBytes)
    Write-Host ("    stderr 首行 = {0}" -f (Format-Short -Text (Get-FirstNonEmptyLine -Text $sampleCap.StdErr) -Max 80))
    Write-Host ("    v1 旧判据 (2>&1 | Out-String) -match 'error' = {0}   ← 恒真" -f $oldMatch)
    Write-Host ("    v1 合并输出里含 NativeCommandError            = {0}" -f $oldNative)
    Write-Host ("    v2 新判据 Test-NegativeVerdict.Pass           = {0}   ← 必须 False" -f $newVerdict.Pass)
    Write-Host ("    v2 诊断可读 Readable                          = {0}" -f $newVerdict.Readable)
    if ($sampleCap.Rc -ne 0) { $fails += '样本 rc 应为 0' }
    if (-not $oldMatch) { $fails += 'v1 旧判据在该样本上未命中（与已知缺陷不符）' }
    if ($newVerdict.Pass) { $fails += 'v2 新判据在该样本上误判为通过' }
    if ($newVerdict.Readable) { $fails += 'v2 把「成功样本的 stderr」误判成诊断' }

    # --- 证据 2：中文诊断不乱码（按字节对照 UTF-8 / GBK 两种解码）---
    $encProbe = Join-Path $PROBES 'neg_dict_missing_colon.lz'
    $encOut = [System.IO.Path]::GetTempFileName()
    $encErr = [System.IO.Path]::GetTempFileName()
    $rcEnc = -1
    $utf8Text = ''
    $gbkText = ''
    $readBack = ''
    try {
        $proc = Start-Process -FilePath $LZ `
            -ArgumentList (('"{0}" --std-dir "{1}"') -f $encProbe, $STD) `
            -WorkingDirectory $ROOT -RedirectStandardOutput $encOut `
            -RedirectStandardError $encErr -NoNewWindow -Wait -PassThru
        $rcEnc = $proc.ExitCode
        $bytes = [System.IO.File]::ReadAllBytes($encErr)
        $utf8Text = [System.Text.Encoding]::UTF8.GetString($bytes)
        $gbkText = [System.Text.Encoding]::GetEncoding(936).GetString($bytes)
        $readBack = Read-TextUtf8 -Path $encErr
    }
    finally {
        Remove-Item -LiteralPath $encOut, $encErr -Force -ErrorAction SilentlyContinue
    }
    $encDiag = Get-LzDiagnostic -Text $readBack
    $noReplacement = -not ($readBack -match [string][char]0xFFFD)
    $utf8HasCjk = [bool]($utf8Text -match '字典')
    $gbkHasCjk = [bool]($gbkText -match '字典')
    $sameAsUtf8 = ($readBack -eq $utf8Text)

    Write-Host ''
    Write-Host '[2] 中文诊断不乱码（缺陷 2 的回归证据）'
    Write-Host ("    样本 = neg_dict_missing_colon.lz；rc = {0}" -f $rcEnc)
    Write-Host ("    Get-Content -Raw -Encoding UTF8 读回 = {0}" -f (Format-Short -Text $readBack -Max 100))
    Write-Host ("    判据取到的诊断首行               = {0}" -f (Format-Short -Text $encDiag -Max 100))
    Write-Host ("    与 .NET UTF-8 严格解码逐字相等    = {0}" -f $sameAsUtf8)
    Write-Host ("    含中文「字典」                    = {0}" -f $utf8HasCjk)
    Write-Host ("    GBK(936) 解码结果含「字典」       = {0}   ← 必须 False（说明字节确实是 UTF-8）" -f $gbkHasCjk)
    Write-Host ("    含替换符 U+FFFD                   = {0}   ← 必须 False" -f (-not $noReplacement))
    if ($rcEnc -eq 0) { $fails += '中文诊断样本 rc 应为非 0' }
    if (-not $sameAsUtf8) { $fails += 'Get-Content -Encoding UTF8 与严格 UTF-8 解码不一致' }
    if (-not $utf8HasCjk) { $fails += 'UTF-8 解码结果不含中文（乱码或编码不对）' }
    if ($gbkHasCjk) { $fails += 'GBK 解码也能读出中文（样本不是 UTF-8，结论不可靠）' }
    if (-not $noReplacement) { $fails += '读回文本含 U+FFFD 替换符（解码丢失）' }
    if (-not $encDiag) { $fails += '中文诊断未被判据识别' }

    # --- 证据 3：判据边界（纯文本，不编译）---
    $cases = @(
        @{ name = '行首 Parse error';          text = 'Parse error: Unexpected token';                          want = $true  },
        @{ name = '行首 IR build error';       text = 'IR build error: IR build error: Semantic error: x';       want = $true  },
        @{ name = '行首 error[E0384]';         text = 'error[E0384]: cannot assign twice';                       want = $true  },
        @{ name = 'LZ LINK RECIPE 噪声';       text = 'LZ LINK RECIPE (BUG-10): rustc --edition 2021';           want = $false },
        @{ name = 'NativeCommandError 包装行'; text = 'lang-zone.exe : Parse error: Unexpected token';           want = $false },
        @{ name = '路径回显（含 error 字样）'; text = 'Generated F:	mp\my error.lz -> my error.rs (IR codegen)'; want = $false },
        @{ name = '普通提示';                  text = '提示：#[tail_call] 是弃用别名';                            want = $false },
        @{ name = 'warning 行';                text = 'warning: unused variable x';                              want = $false }
    )
    Write-Host ''
    Write-Host '[3] 判据边界（纯文本，不编译）'
    foreach ($c in $cases) {
        $got = ($null -ne (Get-LzDiagnostic -Text $c.text))
        $mark = 'ok'
        if ($got -ne $c.want) {
            $mark = 'FAIL'
            $fails += ('判据边界不符：{0}（期望可读={1} 实得={2}）' -f $c.name, $c.want, $got)
        }
        Write-Host ('    [{0}] {1}：期望可读={2} 实得={3}' -f $mark, $c.name, $c.want, $got)
    }

    # --- 证据 4：正例内容断言既能判真也能判假（防「恒真」）---
    $cHitFile = [System.IO.Path]::GetTempFileName()
    $cMissFile = [System.IO.Path]::GetTempFileName()
    $cEmptyFile = [System.IO.Path]::GetTempFileName()
    $cHitOk = $false; $cMissOk = $false; $cEmptyOk = $false
    try {
        Set-Content -LiteralPath $cHitFile -Value "pub struct Point {`n    x: i64,`n}" -Encoding UTF8
        # 应有构造 → 必须通过
        $cHitOk = (Test-ContentVerdict -RsPath $cHitFile -Patterns @('pub struct\s+Point\b')).Pass
        # 缺构造 → 必须不通过（样本里只有 Point，没有 Color 枚举）
        $cMissOk = -not (Test-ContentVerdict -RsPath $cMissFile -Patterns @('^pub enum\s+Color\b')).Pass
        # 空文件 → 必须不通过
        $cEmptyOk = -not (Test-ContentVerdict -RsPath $cEmptyFile -Patterns @('pub')).Pass
    }
    finally {
        Remove-Item -LiteralPath $cHitFile, $cMissFile, $cEmptyFile -Force -ErrorAction SilentlyContinue
    }
    Write-Host ''
    Write-Host '[4] 正例内容断言（rc 通过 ≠ 生成物含该特性）'
    Write-Host ("    样本含 pub struct Point，断言通过        = {0}   ← 必须 True" -f $cHitOk)
    Write-Host ("    空文件/缺构造样本，断言不通过            = {0}   ← 必须 True（{1}）" -f ($cMissOk -and $cEmptyOk), '防恒真')
    if (-not $cHitOk) { $fails += '内容断言在应有构造的样本上未命中（判据失效）' }
    if (-not $cMissOk) { $fails += '内容断言在缺构造的样本上仍判通过（恒真）' }
    if (-not $cEmptyOk) { $fails += '内容断言在空文件上判通过（恒真）' }

    Write-Host ''
    if ($fails.Count -eq 0) {
        Write-Host '===== -SelfTest: 全部通过 =====' -ForegroundColor Green
        exit 0
    }
    foreach ($x in $fails) { Write-Host ("[SELFTEST FAIL] {0}" -f $x) -ForegroundColor Red }
    exit 1
}

# ============================================================
# 正例（DEMO 编译）
# ============================================================
Write-Host '===== 正例（DEMO 编译）====='
$PASS_POS = 0; $FAIL_POS = 0; $SKIP_POS = 0; $POS_STDERR = 0
$posFailList = @(); $skipList = @()
$posRows = New-Object System.Collections.Generic.List[string]
$posRows.Add('| 特性 | 规范文档 | 正例 | rc | 结论 | stderr 首行 |')
$posRows.Add('|---|---|---|---|---|---|')

foreach ($f in $features) {
    $p = Join-Path $ROOT ('DEMO\' + $f.pos)
    if (-not (Test-Path -LiteralPath $p)) {
        $SKIP_POS++
        $skipList += ('{0} ({1})' -f $f.n, $f.pos)
        Write-Host ('[SKIP] {0} — 无正例文件 DEMO\{1}' -f $f.n, $f.pos)
        $posRows.Add(('| {0} | {1} | {2} | - | SKIP | 无正例文件 |' -f $f.n, $f.d, $f.pos))
        continue
    }
    $cap = Invoke-LzCapture -Exe $LZ -ArgList @($p, '--std-dir', $STD) -WorkDir $ROOT
    if ($cap.StdErrBytes -gt 0) { $POS_STDERR++ }
    $errFirst = Format-Short -Text (Get-FirstNonEmptyLine -Text $cap.StdErr) -Max 60
    if ($cap.Rc -eq 0) {
        $PASS_POS++
        Write-Host ('[PASS] {0}' -f $f.n)
        $posRows.Add(('| {0} | {1} | {2} | 0 | PASS | {3} |' -f $f.n, $f.d, $f.pos, (Escape-Cell $errFirst)))
    }
    else {
        $FAIL_POS++
        $posFailList += ('{0} ({1}) rc={2}' -f $f.n, $f.pos, $cap.Rc)
        Write-Host ('[FAIL] {0} rc={1}' -f $f.n, $cap.Rc)
        $posRows.Add(('| {0} | {1} | {2} | {3} | FAIL | {4} |' -f $f.n, $f.d, $f.pos, $cap.Rc, (Escape-Cell $errFirst)))
    }
}
$totalPos = $PASS_POS + $FAIL_POS

# ============================================================
# 正例内容断言（P3）：rc=0 只证明「编译没失败」，不证明「生成物里还有这个特性」
# ============================================================
Write-Host ''
Write-Host '===== 正例内容断言（生成物构造）====='
$PASS_CONTENT = 0; $FAIL_CONTENT = 0
$contentFailList = @()
$contentRows = New-Object System.Collections.Generic.List[string]
$contentRows.Add('| 特性 | 正例文件 | rc | 预期构造（正则，全部命中） | 命中 | 结论 |')
$contentRows.Add('|---|---|---|---|---|---|')

foreach ($chk in $script:ContentChecks) {
    $p = Join-Path $ROOT ('DEMO\' + $chk.pos)
    $rs = [System.IO.Path]::ChangeExtension($p, '.rs')
    $patText = ($chk.pat -join ' ∧ ')
    if (-not (Test-Path -LiteralPath $p)) {
        $FAIL_CONTENT++
        $contentFailList += ('{0} — 无正例文件 DEMO\{1}' -f $chk.tag, $chk.pos)
        Write-Host ('[FAIL] {0} — 无正例文件 DEMO\{1}' -f $chk.tag, $chk.pos)
        $contentRows.Add(('| {0} | {1} | - | {2} | 0/{3} | 正例文件不存在 |' -f $chk.tag, $chk.pos, (Escape-Cell $patText), $chk.pat.Count))
        continue
    }
    # 记录编译前 .rs 的 mtime：编译后必须变新，否则说明看的不是本次产物。
    $stale = $null
    if (Test-Path -LiteralPath $rs) { $stale = (Get-Item -LiteralPath $rs).LastWriteTimeUtc }
    $cap = Invoke-LzCapture -Exe $LZ -ArgList @($p, '--std-dir', $STD) -WorkDir $ROOT
    $fresh = $false
    if (Test-Path -LiteralPath $rs) {
        $fresh = $true
        if ($null -ne $stale) {
            $fresh = ((Get-Item -LiteralPath $rs).LastWriteTimeUtc -gt $stale)
        }
    }
    $hitCount = 0
    $reason = ''
    $verdict = $false
    if ($cap.Rc -ne 0) {
        $reason = ('rc={0}（正例编译失败）' -f $cap.Rc)
    }
    elseif (-not $fresh) {
        $reason = '生成 .rs 缺失或不是本次编译产物'
    }
    else {
        $cv = Test-ContentVerdict -RsPath $rs -Patterns $chk.pat
        $hitCount = $cv.Hits.Count
        if ($cv.Pass) { $verdict = $true }
        else { $reason = ('正例内容不符，缺构造：{0}' -f (Format-Short -Text ($cv.Misses -join ' ∧ ') -Max 100)) }
    }
    if ($verdict) {
        $PASS_CONTENT++
        Write-Host ('[PASS] {0} — 命中 {1}/{2} 个预期构造' -f $chk.tag, $hitCount, $chk.pat.Count)
        $contentRows.Add(('| {0} | {1} | 0 | {2} | {3}/{4} | PASS |' -f $chk.tag, $chk.pos, (Escape-Cell $patText), $hitCount, $chk.pat.Count))
    }
    else {
        $FAIL_CONTENT++
        $contentFailList += ('{0}（{1}）— {2}' -f $chk.tag, $chk.pos, $reason)
        Write-Host ('[FAIL] {0} — {1}' -f $chk.tag, $reason)
        $contentRows.Add(('| {0} | {1} | {2} | {3} | {4}/{5} | 正例内容不符：{6} |' -f $chk.tag, $chk.pos, $cap.Rc, (Escape-Cell $patText), $hitCount, $chk.pat.Count, (Escape-Cell $reason)))
    }
}
$totalContent = $PASS_CONTENT + $FAIL_CONTENT

# ============================================================
# 反例（探针应失败）
# ============================================================
Write-Host ''
Write-Host '===== 反例（探针应失败）====='
$PASS_NEG = 0; $FAIL_NEG = 0; $UNREADABLE_NEG = 0
$negFailList = @()
$negRows = New-Object System.Collections.Generic.List[string]
$negRows.Add('| 探针 | rc | 诊断可读 | 结论 | 诊断首行 |')
$negRows.Add('|---|---|---|---|---|')

$negProbes = @(Get-ChildItem -LiteralPath $PROBES -Filter 'neg_*.lz' -File | Sort-Object Name)
foreach ($probe in $negProbes) {
    $cap = Invoke-LzCapture -Exe $LZ -ArgList @($probe.FullName, '--std-dir', $STD) -WorkDir $ROOT
    $v = Test-NegativeVerdict -Capture $cap
    if ($v.Pass) {
        $PASS_NEG++
        Write-Host ('[PASS] {0} (rc={1})' -f $probe.Name, $v.Rc)
        $negRows.Add(('| {0} | {1} | 是 | 按预期失败 | {2} |' -f $probe.Name, $v.Rc, (Escape-Cell $v.Diag)))
    }
    else {
        $FAIL_NEG++
        if ($v.Rc -ne 0 -and -not $v.Readable) { $UNREADABLE_NEG++ }
        $negFailList += ('{0} — {1}' -f $probe.Name, $v.Reason)
        $rawFirst = Format-Short -Text (Get-FirstNonEmptyLine -Text ([string]$cap.StdOut + "`n" + [string]$cap.StdErr)) -Max 100
        $shown = $v.Diag
        if (-not $shown) { $shown = ('（无可读诊断）原始首行：' + $rawFirst) }
        Write-Host ('[FAIL] {0} rc={1} readable={2} — {3}' -f $probe.Name, $v.Rc, $v.Readable, $v.Reason)
        $negRows.Add(('| {0} | {1} | {2} | 未按预期失败 | {3} |' -f $probe.Name, $v.Rc, $(if ($v.Readable) { '是' } else { '否' }), (Escape-Cell $shown)))
    }
}
$totalNeg = $PASS_NEG + $FAIL_NEG

# ============================================================
# 报告
# ============================================================
$commitSha = (& git -C $ROOT rev-parse --short HEAD 2>$null)
if (-not $commitSha) { $commitSha = '（git 不可用）' }
$binItem = Get-Item -LiteralPath $LZ
$binRel = $LZ
if ($binRel.StartsWith($ROOT, [System.StringComparison]::OrdinalIgnoreCase)) {
    $binRel = $binRel.Substring($ROOT.Length).TrimStart('\')
}

$L = New-Object System.Collections.Generic.List[string]
$L.Add('# lz 语法特性测试矩阵报告')
$L.Add('')
$L.Add(('- 日期：{0}' -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss')))
$L.Add(('- commit：{0}' -f $commitSha))
$L.Add(('- 二进制：{0}（{1} 字节，{2}）' -f $binRel, $binItem.Length, $binItem.LastWriteTime.ToString('yyyy-MM-dd HH:mm')))
$L.Add('- 测试脚本：div-tools\syntax_matrix.ps1（判据 v2，本报告由脚本生成，勿手改）')
$L.Add(('- shell：{0}（powershell.exe；本机无 pwsh）' -f $psVersion))
$L.Add('')
$L.Add('## 判据（可复核）')
$L.Add('')
$L.Add('- 采集：子进程 stdout、stderr **分别重定向到独立临时文件**（不经 PowerShell 管道），再用')
$L.Add('  `Get-Content -Raw -Encoding UTF8` 读回，临时文件用完即删。这样子进程 stderr 不会被')
$L.Add('  PS 5.1 逐行包成 ErrorRecord（`FullyQualifiedErrorId : NativeCommandError`），也不会被 GBK 解码吞字节。')
$L.Add('- 正例判据：`rc == 0`。')
$L.Add('- 正例内容断言（P3）：对 5 个代表性特性，除 `rc == 0` 外还要求本次编译生成的 `.rs`')
$L.Add('  含预期构造（正则表见脚本 `$script:ContentChecks`）；任一不命中记「正例内容不符」，')
$L.Add('  并计入退出码。未覆盖的特性仍「仅 rc 判定」，清单见「仅 rc 判定清单」。')
$L.Add('- 反例判据：`rc != 0` **且** 输出含「行首诊断」，并排除噪声：')
$L.Add('  - 行首诊断（正则）：`^(Parse error|IR build error|Macro definition error|Semantic error|Lexer error|Type error|Codegen error|Link error|Compile error|Error|error[E\d]+)`')
$L.Add('  - 排除：`NativeCommandError`、`FullyQualifiedErrorId`、`LZ LINK RECIPE`、`Generated` 路径回显、`Bridge registry`、`提示：`、`warning/note/info`。')
$L.Add('- 旧判据（v1，已废弃）：`(2>&1 | Out-String) -match ''error''`。在 PS 5.1 下 stderr 被包成')
$L.Add('  ErrorRecord 且带 `NativeCommandError` 字样，因此**任何产生 stderr 的样本都命中**，判据实际退化成只看 rc。')
$L.Add('- 判据自检（可复跑）：`powershell -NoProfile -File div-tools\syntax_matrix.ps1 -SelfTest`')
$L.Add('')
$L.Add('## 正例（DEMO 编译）')
$L.Add('')
$L.Add(('- 特性总数：{0}' -f $features.Count))
$L.Add(('- 正例通过：{0} / {1}（分母 = 非 SKIP 特性数）' -f $PASS_POS, $totalPos))
$L.Add(('- 失败：{0}' -f $FAIL_POS))
$L.Add(('- SKIP：{0} 条（无正例文件，显式列在第 1 张表末尾与下方清单）' -f $SKIP_POS))
$L.Add(('- 正例中 rc=0 但 stderr 非空：{0} 条 —— v1 旧判据会把它们全判成「诊断可读」' -f $POS_STDERR))
$L.Add(('- 内容断言（P3）：{0} / {1} 通过 —— rc=0 且生成 .rs 含预期构造' -f $PASS_CONTENT, $totalContent))
$L.Add(('- 内容断言失败：{0}{1}' -f $FAIL_CONTENT, $(if ($contentFailList.Count -eq 0) { '' } else { '（' + ($contentFailList -join '; ') + '）' })))
$L.Add(('- 失败清单：{0}' -f $(if ($posFailList.Count -eq 0) { '无' } else { $posFailList -join '; ' })))
$L.Add('')
$L.Add('### 正例明细')
$L.Add('')
foreach ($r in $posRows) { $L.Add($r) }
$L.Add('')
$L.Add('### SKIP 清单（无正例文件，不计入分母）')
$L.Add('')
if ($skipList.Count -eq 0) {
    $L.Add('- 无')
}
else {
    $L.Add('| 特性 | 期望正例（不存在） | 原因 |')
    $L.Add('|---|---|---|')
    foreach ($f in $features) {
        $p = Join-Path $ROOT ('DEMO\' + $f.pos)
        if (-not (Test-Path -LiteralPath $p)) {
            $L.Add(('| {0} | {1} | DEMO\{1} 不存在，无正例可编译 |' -f $f.n, $f.pos))
        }
    }
}
$L.Add('')
$L.Add('### 正例内容断言（P3：rc 通过 ≠ 生成物含该特性）')
$L.Add('')
$L.Add('- 断言口径：`rc == 0` **且** `.rs` 是本次编译产物（mtime 变新）**且** 该特性全部预期正则命中；')
$L.Add('  任一不满足记「正例内容不符」并计入退出码。')
$L.Add(('- 覆盖：{0} 条（{1}）' -f $totalContent, (($script:ContentChecks | ForEach-Object { $_.tag }) -join ' / ')))
$L.Add(('- 通过：{0} / {1}' -f $PASS_CONTENT, $totalContent))
$L.Add(('- 失败：{0}{1}' -f $FAIL_CONTENT, $(if ($contentFailList.Count -eq 0) { '（无）' } else { '（' + ($contentFailList -join '; ') + '）' })))
$L.Add('')
foreach ($r in $contentRows) { $L.Add($r) }
$L.Add('')
$contentPos = @{}
foreach ($chk in $script:ContentChecks) { $contentPos[$chk.pos] = $true }
$rcOnly = @($features | Where-Object { -not $contentPos.ContainsKey($_.pos) })
$rcOnlyExplicit = @($rcOnly | Where-Object { $script:RcOnlyNotes.ContainsKey($_.n) })
$rcOnlyPlain = @($rcOnly | Where-Object { -not $script:RcOnlyNotes.ContainsKey($_.n) })
$L.Add('### 仅 rc 判定清单（正例只判 rc，不保证语义正确）')
$L.Add('')
$L.Add('- 显式标注（该特性的正例不适合内容断言）：')
if ($rcOnlyExplicit.Count -eq 0) {
    $L.Add('  - 无')
}
else {
    foreach ($f in $rcOnlyExplicit) {
        $L.Add(('  - {0} — {1}' -f $f.n, $script:RcOnlyNotes[$f.n]))
    }
}
$L.Add(('- 其余未配置内容断言（可继续扩 `$script:ContentChecks`）：{0} 条{1}' -f $rcOnlyPlain.Count, $(if ($rcOnlyPlain.Count -eq 0) { '' } else { ' — ' + (($rcOnlyPlain | ForEach-Object { $_.n }) -join '、') })))
$L.Add('- 内容断言是生成 `.rs` 的**正则文本匹配**，只证明「构造还在」，不是语义等价性证明。')
$L.Add('')
$L.Add('## 反例（探针应失败）')
$L.Add('')
$L.Add(('- 反例探针数：{0}' -f $totalNeg))
$L.Add(('- 按预期失败：{0} / {1}' -f $PASS_NEG, $totalNeg))
$L.Add(('- 意外结果：{0}' -f $FAIL_NEG))
$L.Add(('- 其中 `rc != 0` 但诊断不可读：{0}（本口径下为 0 表示「有失败就有可读诊断」）' -f $UNREADABLE_NEG))
$L.Add(('- 意外结果清单：{0}' -f $(if ($negFailList.Count -eq 0) { '无' } else { $negFailList -join '; ' })))
$L.Add('')
$L.Add('### 反例明细')
$L.Add('')
foreach ($r in $negRows) { $L.Add($r) }
$L.Add('')
$L.Add('## 缺口清单')
$L.Add('')
$L.Add(('- 无正例的特性：{0} 条（{1}）' -f $SKIP_POS, $(if ($skipList.Count -eq 0) { '无' } else { $skipList -join '; ' })))
$uncovered = $features.Count - $totalNeg
$L.Add(('- 无反例探针的特性：{0} - {1} = {2} 个（粗估：探针未与特性一一映射，只作覆盖差提示）' -f $features.Count, $totalNeg, $uncovered))
$knownRaise = @($negFailList | Where-Object { $_ -like 'neg_error_uncaught_panic_ok.lz*' })
if ($knownRaise.Count -gt 0) {
    $L.Add('- 已知不一致（**非本脚本判据缺陷**）：`neg_error_uncaught_panic_ok.lz` 未按预期失败，是 raise 语义')
    $L.Add('  的文档/实现分歧，归另一条车道；本脚本只如实标红，未修改探针。')
}
$L.Add('- 未修的次要缺陷：')
$L.Add('  - 反例探针与特性清单未建立一一映射，因此“特性级反例覆盖”仍是粗估，不是精确矩阵。')
$L.Add('  - 正例内容断言只覆盖 5 个代表性特性（struct/enum/match/closure/trait/impl），其余特性仍「仅 rc 判定」；')
$L.Add('    且断言是生成 `.rs` 的正则文本匹配，不是语义等价性证明。')
$L.Add('  - 脚本仍按目录约定命名探针（neg_*.lz），命名不规范的新探针不会被收集。')

$reportText = ($L -join "`n")
Set-Content -LiteralPath $REPORT -Value $reportText -Encoding UTF8

Write-Host ''
Write-Host '=========================================='
Write-Host ('正例: {0}/{1}（SKIP {2}）  内容断言: {3}/{4}  反例: {5}/{6}' -f $PASS_POS, $totalPos, $SKIP_POS, $PASS_CONTENT, $totalContent, $PASS_NEG, $totalNeg)
Write-Host ('报告: {0}' -f ($REPORT.Substring($ROOT.Length).TrimStart('\')))
Write-Host '=========================================='

if ($FAIL_POS -eq 0 -and $FAIL_NEG -eq 0 -and $FAIL_CONTENT -eq 0) { exit 0 } else { exit 1 }
