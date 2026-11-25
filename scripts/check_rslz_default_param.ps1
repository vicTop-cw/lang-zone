# 判据：rs-lz 默认参数展开（缺口③，iter-22:00 T0r20）
$ErrorActionPreference = "Continue"
$env:CARGO_TARGET_DIR = "target/iter22-lz"
cargo test --test rs_lz_families m_default_param_option_shim_and_call_padding -- --exact 2>&1 | Select-String -Pattern "test result"
exit $LASTEXITCODE
