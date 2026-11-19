# T0r66 判据脚本：Python bridge 委托 + C 适配器 str/容器护栏
# 由 run_check 以相对路径 `scripts/check_t0r66.ps1` 调用（白名单路径规则）。
$ErrorActionPreference = "Stop"
$env:CARGO_TARGET_DIR = "target\lane-loomy-1600"

Write-Output "== lib bridge::python + embed::adapters =="
cargo test --lib bridge::python 2>&1 | Select-String -Pattern "test result:"
cargo test --lib embed::adapters 2>&1 | Select-String -Pattern "test result:"

Write-Output "== 集成测试 =="
cargo test --test bridge_python_delegation 2>&1 | Select-String -Pattern "test result:"
cargo test --test embed_c_serialization 2>&1 | Select-String -Pattern "test result:"

Write-Output "== 全绿判据 =="
$r = cargo test --test embed_c_serialization 2>&1 | Out-String
if ($r -match "test result: ok") { Write-Output "T0r66_OK"; exit 0 } else { Write-Output "T0r66_FAIL"; exit 1 }
