$ErrorActionPreference = 'Stop'
$files = @(
  'SYNTAX/00-词法基础.md','SYNTAX/01-类型系统.md','SYNTAX/02-变量与绑定.md',
  'SYNTAX/overview/语法冻结基线.md','SYNTAX/overview/想法仓库.md','SYNTAX/overview/缺失语法特性报告.md','SYNTAX/overview/语法文档编写规范.md','SYNTAX/overview/项目进度报告.md',
  'IR/design.md','IR/implementation-plan.md','IR/migration-roadmap.md','IR/README.md','IR/kinds.md','IR/frontend-gap-plan.md','IR/design-magic-init-priority.md',
  'src/ir/node.rs','src/ir/types.rs','src/ir/mod.rs','src/ir/builder.rs','src/ir/codegen.rs','src/ir/display.rs',
  'tests/ir_snapshots.rs','tests/mod.rs','tests/reject_errors.rs','Cargo.toml'
)
foreach ($f in $files) {
  if (Test-Path $f) {
    $h = (Get-FileHash -Algorithm SHA256 $f).Hash.ToLower()
    Write-Output "$h  $f"
  } else {
    Write-Output "MISSING  $f"
  }
}
