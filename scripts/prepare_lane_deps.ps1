# 车道 deps 目录准备（Loomy 定制 cargo shim 环境适配，迭代-14:00）
#
# 背景：本机 cargo 是 Loomy 环境的 shim（`C:\Users\Administrator\.cargo\bin\cargo.exe`
# 为 0 字节占位，转发到真实 rustup 代理）。它的 --out-dir 为
#   `<target>/<profile>/build/<crate>/<hash>/out/`，并以多个
#   `-L dependency=<target>/<profile>/build/<dep>/<hash>/out` 传依赖，
#   **从不生成 `deps/` 子目录**。
# 而项目内大量测试（tests/*.rs）与 src/bridge/project.rs 按标准 cargo 布局，
# 以 `-L dependency=<rlib 父目录>/deps` 链接，于是恒报
#   E0463 can't find crate for `lz_builtins`/`num_bigint`
# （详见 loomy-learned-stale-lane-test-triage 族经验）。
#
# 本脚本把某个（或全部）profile 目录下的所有 `*.rlib` 复制到一个合成的 `deps/`
# 目录，使 `-L dependency=<profile>/deps` 可解析主 rlib 及其传递依赖。
#
# 用法：
#   powershell -NoProfile -File scripts/prepare_lane_deps.ps1                 # 处理 target/debug + target/lane-bootstrap/debug
#   powershell -NoProfile -File scripts/prepare_lane_deps.ps1 -Profile debug   # 只处理指定 profile 目录（相对仓库根）
#   powershell -NoProfile -File scripts/prepare_lane_deps.ps1 -Build           # 先 cargo build -p lz_builtins 再收集
param(
    [string[]]$Profile = @("target\debug", "target\lane-bootstrap\debug"),
    [switch]$Build
)
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

if ($Build) {
    Write-Host "[prepare_lane_deps] 预构建 lz_builtins（含传递依赖）..." -ForegroundColor Yellow
    Push-Location $Root
    try {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        foreach ($p in $Profile) {
            $profileName = Split-Path $p -Leaf
            $targetRoot = Split-Path $p -Parent
            if ($targetRoot -eq "target") {
                Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
            } else {
                $env:CARGO_TARGET_DIR = Join-Path $Root $targetRoot
            }
            Write-Host "  cargo build -p lz_builtins @ $targetRoot ($profileName)"
            cargo build -p lz_builtins 2>&1 | Select-String -Pattern 'Finished|error' | ForEach-Object { Write-Host "    $_" }
        }
    } finally { Pop-Location }
}

$total = 0
foreach ($p in $Profile) {
    $profileDir = Join-Path $Root $p
    if (-not (Test-Path $profileDir)) {
        Write-Host "[prepare_lane_deps] 跳过（不存在）: $profileDir" -ForegroundColor DarkGray
        continue
    }
    $deps = Join-Path $profileDir "deps"
    New-Item -ItemType Directory -Path $deps -Force | Out-Null
    # 递归收集所有 rlib（含 build/<dep>/<hash>/out 下的传递依赖），排除 deps 自身
    $src = Get-ChildItem $profileDir -Recurse -Filter "*.rlib" -ErrorAction SilentlyContinue |
        Where-Object { $_.DirectoryName -ne $deps }
    $cnt = 0
    foreach ($f in $src) {
        Copy-Item $f.FullName -Destination $deps -Force -ErrorAction SilentlyContinue
        $cnt++
    }
    $have = (Get-ChildItem $deps -Filter "*.rlib" -ErrorAction SilentlyContinue | Measure-Object).Count
    Write-Host "[prepare_lane_deps] $p -> deps（复制 $cnt，现有 $have 个 rlib）" -ForegroundColor Green
    $total += $have
}
Write-Host "[prepare_lane_deps] 完成（合计 $total 个 rlib 就位）" -ForegroundColor Green
exit 0
