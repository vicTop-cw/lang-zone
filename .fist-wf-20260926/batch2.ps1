$ErrorActionPreference = "Continue"
Set-Location E:\IDEProjects\AI\lang-zone
$log = ".fist-wf-20260926"
$cmds = @(
  "cargo test -j 1 --test demo_codegen_compile -- --include-ignored --no-fail-fast > $log\t_gate_demo.log 2>&1",
  "cargo test -j 1 --test find_bug_bugs -- --include-ignored --no-fail-fast > $log\t_gate_fbb.log 2>&1",
  "cargo test -j 1 --test find_bug_libs -- --include-ignored --no-fail-fast > $log\t_gate_fbl.log 2>&1",
  "cargo test -j 1 --test cy_codegen_gate -- --ignored --no-fail-fast > $log\t_gate_cyg.log 2>&1"
)
foreach ($c in $cmds) {
  Write-Output "RUN: $c"
  cmd /c $c
  Write-Output "rc=$LASTEXITCODE"
}
Write-Output "ALL-BATCH2-DONE"
