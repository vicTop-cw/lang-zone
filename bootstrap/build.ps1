# ============================================================
# Lang-Zong 自举（Bootstrap）构建脚本 —— PowerShell 入口
# 与 bootstrap/build.sh 等价：All / Closed / Clean / 单文件
# 用法:
#   .\bootstrap\build.ps1 -Mode Closed                      # 自举闭环（两代 + manifest 一致性校验）
#   .\bootstrap\build.ps1 -Mode Closed -Generations 3        # 三代自举收敛（M5 Lane D 验收口径）
#   .\bootstrap\build.ps1 -Mode Closed -Compiler lzcyc       # 闭环入口换成 lzcyc（--backend=rust）
#   .\bootstrap\build.ps1 -Mode All                          # 单轮全量（bootstrap/work/*.lz）
#   .\bootstrap\build.ps1 -Mode Clean                        # 清理产物
#   .\bootstrap\build.ps1 -Mode One -File path\to\file.lz    # 单文件
# 入口参数化（M5 Lane D）:
#   -Compiler    lang-zone（默认）| lzcyc | 任意编译器可执行文件路径
#   -Lzc         显式编译器路径（优先于 -Compiler 的解析结果）
#   -BuiltinsRlib/-StdDir  内置库 rlib / 标准库目录（默认：车道 rlib 优先，回落 target\debug）
#   -Rustc       用于把 .rs 编译成二进制的 rustc（默认 rustc；须与 BuiltinsRlib 同版本，
#                否则 E0514——建议 `rustup run <rust-toolchain.toml 的 channel> rustc`）
#   -Generations Closed 模式的构建代数（默认 2；M5 验收用 3 = 三代收敛）
# 退出码：0 全通过；1 lzc 失败；2 rustc 失败；3 运行失败；4 manifest 不一致；5 环境缺失
# ============================================================
param(
    [ValidateSet("Closed", "All", "Clean", "One")]
    [string]$Mode = "Closed",
    [string]$File = "",
    [string]$Compiler = "lang-zone",
    [string]$Lzc = "",
    [string]$BuiltinsRlib = "",
    [string]$StdDir = "",
    [string]$Rustc = "rustc",
    [int]$Generations = 2
)

$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$WorkDir = Join-Path $ScriptDir "work"
$ManifestDir = Join-Path $WorkDir "manifest"
$LogDir = Join-Path $WorkDir "log"
$BackupGood = Join-Path $WorkDir "backup\good"
# 入口解析：显式 -Lzc 优先；否则按 -Compiler 名字映射（lzcyc → CY\target\debug）
if (-not $Lzc) {
    if ($Compiler -match "lzcyc") {
        $Lzc = Join-Path $ProjectDir "CY\target\debug\lzcyc.exe"
    } else {
        $Lzc = Join-Path $ProjectDir "target\debug\lang-zone.exe"
    }
}
if (-not $BuiltinsRlib) {
    # 车道 rlib 优先（target\lane-bootstrap 由钉版工具链构建，规避 E0514 跨版本错配）
    $laneRlib = Join-Path $ProjectDir "target\lane-bootstrap\debug\liblz_builtins.rlib"
    if (Test-Path $laneRlib) { $BuiltinsRlib = $laneRlib }
    else { $BuiltinsRlib = Join-Path $ProjectDir "target\debug\liblz_builtins.rlib" }
}
if (-not $StdDir) { $StdDir = Join-Path $ProjectDir "std" }
# 车道 rlib 与当前工具链同源（防 cron 异版本重建 → E0514；~3s）
$laneDir = Join-Path $ProjectDir "target\lane-bootstrap"
if (Test-Path $laneDir) {
    $saved = $env:CARGO_TARGET_DIR
    $env:CARGO_TARGET_DIR = "target/lane-bootstrap"
    Push-Location $ProjectDir
    try { cargo build -p lz_builtins 2>&1 | Out-Null } finally { Pop-Location; $env:CARGO_TARGET_DIR = $saved }
}
# lzcyc 走 compile 子命令 + Rust 后端（cython 后端本机不可用，见 M5 风险缓解预案）
$IsLzcyc = ([System.IO.Path]::GetFileNameWithoutExtension($Lzc) -ieq "lzcyc")
# 钉版工具链自给（读 rust-toolchain.toml；防 cron/异版本 rustc 造成 E0514）
$tcFile = Join-Path $ProjectDir "rust-toolchain.toml"
if (Test-Path $tcFile) {
    $m = Select-String -Path $tcFile -Pattern 'channel\s*=\s*"([^"]+)"'
    if ($m) {
        $tcBin = Join-Path (Join-Path $env:USERPROFILE ".rustup\toolchains") ($m.Matches[0].Groups[1].Value + "\bin")
        if (Test-Path $tcBin) { $env:PATH = "$tcBin;" + $env:PATH }
    }
}

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
    if ($IsLzcyc) {
        # lzcyc 入口：compile 子命令 + Rust 后端（产物同名 .rs 落源文件旁）。
        # 透传 rustc/rlib 选择（Lane D 参数化），使 lzcyc 内部 rustc 调用同版本（规避 E0514）
        $oldRustc = $env:LZ_RUSTC; $oldRlib = $env:LZ_BUILTINS_RLIB
        $env:LZ_RUSTC = $Rustc; $env:LZ_BUILTINS_RLIB = $BuiltinsRlib
        & $Lzc compile $lzFile --backend=rust --std-dir $StdDir *> $null
        $env:LZ_RUSTC = $oldRustc; $env:LZ_BUILTINS_RLIB = $oldRlib
    } else {
        & $Lzc $lzFile --std-dir $StdDir *> $null
    }
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "  [LZC] LZ 编译失败: $lzFile (exit=$LASTEXITCODE)"
        return 1
    }
    Log-Pass "  [LZC] LZ -> $rsFile"

    # Step 2: Rust → Binary
    Log-Info "  [RUSTC] Rust -> Binary..."
    $rustcErr = & $Rustc --edition 2021 $rsFile --extern "lz_builtins=$BuiltinsRlib" -L "dependency=$(Split-Path $BuiltinsRlib -Parent)\deps" -o $exeFile 2>&1
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
    if (-not (Test-Path $Lzc)) { Log-Fail "环境缺失: $Lzc（先构建编译器：cargo build 或 CY 侧 cargo build）"; return 5 }
    if (-not (Test-Path $BuiltinsRlib)) { Log-Fail "环境缺失: $BuiltinsRlib（先 cargo build -p lz_builtins）"; return 5 }
    if ($Generations -lt 1) { $Generations = 1 }
    New-Item -ItemType Directory -Force -Path $ManifestDir, $LogDir, $BackupGood | Out-Null
    $ts = Get-Date -Format "yyyyMMdd-HHmmss"
    $logFile = Join-Path $LogDir "$ts.log"
    $latestLog = Join-Path $LogDir "latest.log"

    Start-Transcript -Path $logFile -Force | Out-Null
    try {
        Write-Host "=== Lang-Zong 自举闭环（$Generations 代收敛）==="
        Write-Host "时间: $ts"
        Write-Host "入口: $Lzc"
        if ($IsLzcyc) { Write-Host "（lzcyc 入口：compile --backend=rust）" }

        # ── N 代构建：每代清产物 → 全量编译运行 → 记 manifest 与运行输出 ──
        for ($g = 1; $g -le $Generations; $g++) {
            if ($g -gt 1) {
                Log-Info "[GEN $g] 清理上一代产物 (.rs/.exe) ..."
                Get-ChildItem -Path $WorkDir -Recurse -File |
                    Where-Object { ($_.Extension -eq ".rs" -or $_.Extension -eq ".exe") -and
                                   $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\runout\\" } |
                    Remove-Item -Force
            }
            Log-Step "第 $g 代构建"
            $rc = Build-All
            if ($rc -ne 0) { Log-Fail "第 $g 代构建失败（见上方阶段日志定位）"; return 1 }
            Make-Manifest (Join-Path $ManifestDir "gen$g.sha256")
            Log-Info "第 $g 代 manifest: gen$g.sha256"
            Record-RunOut (Join-Path $WorkDir "runout\gen$g")
        }

        # ── 收敛校验：全部 manifest 逐字节一致（跨代确定性）──
        Log-Info "[VERIFY] $Generations 代 .rs manifest 一致性校验..."
        $base = Get-Content (Join-Path $ManifestDir "gen1.sha256")
        for ($g = 2; $g -le $Generations; $g++) {
            $cur = Get-Content (Join-Path $ManifestDir "gen$g.sha256")
            $d = Compare-Object $base $cur
            if ($d) {
                Log-Fail "第 1 代与第 $g 代 .rs manifest 不一致（非确定性构建 / 未收敛）"
                $d | Select-Object -First 20 | ForEach-Object { Write-Host $_ }
                return 4
            }
            Log-Pass "[VERIFY] gen1 与 gen$g .rs manifest 完全一致"
        }
        # ── 收敛校验：全部运行输出一致（行为等价）──
        for ($g = 2; $g -le $Generations; $g++) {
            Log-Info "[VERIFY] gen1 与 gen$g .exe 运行输出一致性校验..."
            if (-not (Test-RunOutSame (Join-Path $WorkDir "runout\gen1") (Join-Path $WorkDir "runout\gen$g"))) {
                Log-Fail "gen1 与 gen$g .exe 运行输出不一致"
                return 4
            }
            Log-Pass "[VERIFY] gen1 与 gen$g 运行输出完全一致"
        }

        Log-Info "[PROMOTE] 更新回滚基线 backup/good ..."
        Get-ChildItem -Path $BackupGood -Recurse -File | Remove-Item -Force -ErrorAction SilentlyContinue
        Copy-Item (Join-Path $ManifestDir "gen$Generations.sha256") (Join-Path $BackupGood "manifest.sha256") -Force
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
        Write-Host "[OK] 自举闭环全部通过（$Generations 代 .rs 一致 + 运行全绿）" -ForegroundColor Green
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
