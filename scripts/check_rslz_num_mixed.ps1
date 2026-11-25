# 判据：rs-lz 混合数值提升（缺口②，iter-22:00 T0r20）
# 跑回归锁 m_num_mixed_promotes_to_f64（真 .lz→.rs→rustc→运行）
$ErrorActionPreference = "Continue"
$env:CARGO_TARGET_DIR = "target/iter22-lz"
cargo test --test rs_lz_families m_num_mixed_promotes_to_f64 -- --exact 2>&1 | Select-String -Pattern "test result"
exit $LASTEXITCODE
