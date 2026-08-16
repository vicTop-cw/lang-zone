# ============================================================
# Lang-Zong 自举（Bootstrap）构建脚本 —— PowerShell 入口
# 与 bootstrap/build.sh 等价：All / Closed / Clean / 单文件
# 用法:
#   .\bootstrap\build.ps1 -Mode Closed   # 自举闭环：两轮构建 + manifest 一致性校验
#   .\bootstrap\build.ps1 -Mode All      # 单轮全量（bootstrap/work/*.lz）
#   .\bootstrap\build.ps1 -Mode Clean    # 清理产物
#   .\bootstrap\build.ps1 -Mode One -File path\to\file.lz
# 退出码：0 全通过；1 lzc 失败；2 rustc 失败；3 运行失败；4 manifest 不一致；5 环境缺失
# ============================================================
param(
    [ValidateSet("Closed", "All", "Clean", "One")]
    [string]$Mode = "Closed",
    [string]$File = ""
)

$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$WorkDir = Join-Path $ScriptDir "work"
$ManifestDir = Join-Path $WorkDir "manifest"
$LogDir = Join-Path $WorkDir "log"
$BackupGood = Join-Path $WorkDir "backup\good"
$Lzc = Join-Path $ProjectDir "target\debug\lang-zone.exe"
$StdDir = Join-Path $ProjectDir "std"
$BuiltinsRlib = Join-Path $ProjectDir "target\debug\liblz_builtins.rlib"

function Log-Step([string]$msg) { Write-Host "`n>>> $msg" -ForegroundColor Yellow }
function Log-Pass([string]$msg) { Write-Host "[PASS] $msg" -ForegroundColor Green }
function Log-Fail([string]$msg) { Write-Host "[FAIL] $msg" -ForegroundColor Red }
function Log-Info([string]$msg) { Write-Host "[INFO] $msg" -ForegroundColor Yellow }

function Compile-And-Run([string]$lzFile) {
    $base = [System.IO.Path]::GetFileNameWithoutExtension($lzFile)
    $dir = Split-Path -Parent $lzFile
    $rsFile = Join-Path $dir "$base.rs"
    $exeFile = Join-Path $dir "$base.exe"
    Log-Step "编译: $base"

    # Step 1: LZ → Rust
    Log-Info "  [LZC] LZ -> Rust..."
    & $Lzc $lzFile --std-dir $StdDir *> $null
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "  [LZC] LZ 编译失败: $lzFile (exit=$LASTEXITCODE)"
        return 1
    }
    Log-Pass "  [LZC] LZ -> $rsFile"

    # Step 2: Rust → Binary
    Log-Info "  [RUSTC] Rust -> Binary..."
    $rustcErr = & rustc --edition 2021 $rsFile --extern "lz_builtins=$BuiltinsRlib" -o $exeFile 2>&1
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "  [RUSTC] rustc 编译失败: $lzFile"
        $rustcErr | Select-String -Pattern "error\[" | Select-Object -First 5 | ForEach-Object { Write-Host $_ }
        return 2
    }
    Log-Pass "  [RUSTC] rustc -> $exeFile"

    # Step 3: 运行
    Log-Info "  [RUN] 运行..."
    $runOut = & $exeFile 2>&1
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "  [RUN] 运行失败 (exit=$LASTEXITCODE): $runOut"
        return 3
    }
    Log-Pass "  [RUN] 运行成功: [$runOut]"
    return 0
}

function Build-All {
    Log-Step "=== Lang-Zong 自举构建 ==="
    Log-Info "编译器: $Lzc"
    Log-Info "标准库: $StdDir"
    $total = 0; $passed = 0; $failed = @()
    $lzFiles = Get-ChildItem -Path $WorkDir -Recurse -Filter "*.lz" |
        Where-Object { $_.FullName -notmatch "\\std\\" -and $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\fail_probe\\" }
    foreach ($f in $lzFiles) {
        $total++
        $rc = Compile-And-Run $f.FullName
        if ($rc -eq 0) { $passed++ } else { $failed += $f.FullName }
    }
    Write-Host ""
    Write-Host "=========================================="
    Write-Host "通过: $passed/$total" -ForegroundColor Green
    if ($failed.Count -gt 0) {
        Write-Host "失败:" -ForegroundColor Red
        $failed | ForEach-Object { Write-Host "  - $_" }
    }
    Write-Host "=========================================="
    if ($failed.Count -eq 0) { return 0 } else { return 1 }
}

function Make-Manifest([string]$out) {
    # 一致性口径：.rs 哈希参与比对；.exe 二进制哈希受 rustc 非确定性影响不比对，
    # 行为等价以运行输出为准（见 05-自举里程碑台账 §5）
    Set-Content -Path $out -Value "" -Encoding utf8
    $files = Get-ChildItem -Path $WorkDir -Recurse -File |
        Where-Object { ($_.Extension -eq ".rs") -and
                       $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\manifest\\" -and $_.FullName -notmatch "\\log\\" -and $_.FullName -notmatch "\\runout\\" }
    $lines = $files | ForEach-Object {
        $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
        $rel = $_.FullName.Substring($WorkDir.Length + 1)
        "$hash  $rel"
    }
    $lines | Sort-Object | Set-Content -Path $out -Encoding utf8
}

function Record-RunOut([string]$out) {
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    Get-ChildItem -Path $WorkDir -Recurse -File |
        Where-Object { ($_.Extension -eq ".exe") -and
                       $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\manifest\\" -and $_.FullName -notmatch "\\log\\" -and $_.FullName -notmatch "\\runout\\" } |
        ForEach-Object {
            $base = [System.IO.Path]::GetFileNameWithoutExtension($_.Name)
            & $_.FullName *> (Join-Path $out "$base.out")
        }
}

function Test-RunOutSame([string]$a, [string]$b) {
    $ok = $true
    Get-ChildItem -Path $a -File | ForEach-Object {
        $rel = $_.Name
        $other = Join-Path $b $rel
        if (-not (Test-Path $other)) { Write-Host "  运行输出缺失: $rel"; $ok = $false }
        elseif ((Get-FileHash $_.FullName).Hash -ne (Get-FileHash $other).Hash) {
            Write-Host "  运行输出不一致: $rel"; $ok = $false
        }
    }
    return $ok
}

function Invoke-Closed {
    if (-not (Test-Path $Lzc)) { Log-Fail "环境缺失: $Lzc（先 cargo build）"; return 5 }
    if (-not (Test-Path $BuiltinsRlib)) { Log-Fail "环境缺失: $BuiltinsRlib（先 cargo build -p lz_builtins）"; return 5 }
    New-Item -ItemType Directory -Force -Path $ManifestDir, $LogDir, $BackupGood | Out-Null
    $ts = Get-Date -Format "yyyyMMdd-HHmmss"
    $logFile = Join-Path $LogDir "$ts.log"
    $latestLog = Join-Path $LogDir "latest.log"
    Copy-Item $logFile $latestLog -Force -ErrorAction SilentlyContinue

    Start-Transcript -Path $logFile -Force | Out-Null
    try {
        Write-Host "=== Lang-Zong 自举闭环 ==="
        Write-Host "时间: $ts"

        Log-Step "第 1 轮构建"
        $rc = Build-All
        if ($rc -ne 0) { Log-Fail "第 1 轮构建失败（见上方阶段日志定位）"; return 1 }
        Make-Manifest (Join-Path $ManifestDir "run1.sha256")
        Log-Info "第 1 轮 manifest: run1.sha256"
        Record-RunOut (Join-Path $WorkDir "runout\run1")

        Log-Info "[VERIFY] 清理产物后执行第 2 轮..."
        Get-ChildItem -Path $WorkDir -Recurse -File |
            Where-Object { ($_.Extension -eq ".rs" -or $_.Extension -eq ".exe") -and $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\runout\\" } |
            Remove-Item -Force

        Log-Step "第 2 轮构建"
        $rc = Build-All
        if ($rc -ne 0) { Log-Fail "第 2 轮构建失败"; return 1 }
        Make-Manifest (Join-Path $ManifestDir "run2.sha256")
        Record-RunOut (Join-Path $WorkDir "runout\run2")

        Log-Info "[VERIFY] 两轮 .rs manifest 一致性校验..."
        $d = Compare-Object (Get-Content (Join-Path $ManifestDir "run1.sha256")) (Get-Content (Join-Path $ManifestDir "run2.sha256"))
        if ($d) {
            Log-Fail "两轮 .rs manifest 不一致（非确定性构建）"
            $d | Select-Object -First 20 | ForEach-Object { Write-Host $_ }
            return 4
        }
        Log-Pass "[VERIFY] 两轮 .rs manifest 完全一致"
        Log-Info "[VERIFY] 两轮 .exe 运行输出一致性校验..."
        if (-not (Test-RunOutSame (Join-Path $WorkDir "runout\run1") (Join-Path $WorkDir "runout\run2"))) {
            Log-Fail "两轮 .exe 运行输出不一致"
            return 4
        }
        Log-Pass "[VERIFY] 两轮 .exe 运行输出完全一致"

        Log-Info "[PROMOTE] 更新回滚基线 backup/good ..."
        Get-ChildItem -Path $BackupGood -Recurse -File | Remove-Item -Force -ErrorAction SilentlyContinue
        Copy-Item (Join-Path $ManifestDir "run2.sha256") (Join-Path $BackupGood "manifest.sha256") -Force
        Get-ChildItem -Path $WorkDir -Recurse -File |
            Where-Object { ($_.Extension -eq ".rs" -or $_.Extension -eq ".exe") -and
                           $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\manifest\\" -and $_.FullName -notmatch "\\log\\" -and $_.FullName -notmatch "\\runout\\" } |
            ForEach-Object {
                $rel = $_.FullName.Substring($WorkDir.Length + 1)
                $dest = Join-Path $BackupGood $rel
                New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dest) | Out-Null
                Copy-Item $_.FullName $dest -Force
            }
        Log-Pass "[PROMOTE] 回滚基线已更新"
        Write-Host ""
        Write-Host "=========================================="
        Write-Host "[OK] 自举闭环全部通过（两轮一致 + 运行全绿）" -ForegroundColor Green
        Write-Host "日志: $logFile"
        Write-Host "=========================================="
        return 0
    } finally {
        Stop-Transcript | Out-Null
        Copy-Item $logFile $latestLog -Force
    }
}

switch ($Mode) {
    "Closed" { exit (Invoke-Closed) }
    "All"    { exit (Build-All) }
    "Clean"  {
        Log-Info "清理构建产物..."
        Get-ChildItem -Path $WorkDir -Recurse -File |
            Where-Object { ($_.Extension -eq ".rs" -or $_.Extension -eq ".exe" -or $_.Extension -eq ".pdb") -and $_.FullName -notmatch "\\backup\\" } |
            Remove-Item -Force
        Log-Pass "清理完成"
        exit 0
    }
    "One"    {
        if (-not $File) { Log-Fail "One 模式需 -File 参数"; exit 1 }
        exit (Compile-And-Run $File)
    }
}
