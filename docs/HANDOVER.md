# HANDOVER — lz 稳定自举 + CLI 子命令（维护者交接文档）

> 目标读者：下一个接手者。按本文档操作无需额外口头说明。

## 1. 当前状态

- 版本 v0.1.161，标签 stable-161（commit 见 RELEASE_NOTES）
- 自举：三代收敛（g2==g3）；前端自举链收敛（f2.rs==f3.rs）
- CLI 子命令：create/build/peek/check/push 全部可用
- 测试基线：cargo test 320/0；DEMO 261/261；closed 闭环 13/13

## 2. 从干净克隆开始

```powershell
git clone <repo-url> lang-zone && cd lang-zone
cargo build --release
# 运行全部测试
cargo test                       # 320/0
# 语法矩阵
powershell -File div-tools\syntax_matrix.ps1
# 自举闭环（两轮一致性）
powershell -File bootstrap\build.ps1 -Mode Closed
# 三代收敛
powershell -File bootstrap\stage\stage.ps1
# CLI E2E
powershell -File div-tools\cli_regression.ps1
```

## 3. 日常工作流

- 单文件编译：`lang-zone.exe file.lz --std-dir std`
- 新项目：`lang-zone.exe create myproj && cd myproj && lang-zone.exe build`
- 检查：`lang-zone.exe check`
- 发布：`lang-zone.exe push --registry <local-dir>`（先 `--dry-run` 预检）

## 4. 回滚

- 代码回滚：`git reset --hard stable-161~1`
- 产物回滚：`powershell -File bootstrap\rollback.ps1`
- 发布回滚：registry 条目目录即版本目录，删除即回滚（原子写入保证无半发布状态）

## 5. 已知异常与恢复

| 异常 | 表现 | 恢复 |
|---|---|---|
| 坏 lz.toml | build/check/push 非零退出 + 诊断 | 修复清单重跑 |
| 坏 .lzcache | 增量命中错误 | 删 build/ 下缓存重跑全量 |
| registry 不可达 | push 非零退出 | 检查路径权限；dry-run 先验证 |
| 版本冲突 | push 非零退出（已存在） | 改 lz.toml version 或先删旧条目 |
| 目标路径非法 | create 非零退出 | 换合法路径 |

## 6. 性能预算

- 三代收敛全流程 ≤ X 分钟（实测值见 stage/ledger.md）
- 编译内存：宿主 512MB 大栈线程，无异常增长
