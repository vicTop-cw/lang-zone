# RELEASE_NOTES — lz 稳定自举 + CLI 子命令集成

## 版本：v0.1.161（stable-161）

### 概要
lz 编译器完成稳定自举（三代收敛）与 CLI 子命令集成（create/build/peek/check/push）。

### 自举收敛
- 宿主编译器（cargo build → lang-zone.exe）处理自举源集（bootstrap/work/ 13 个 .lz）→ 第 1 代产物
- 第 1 代 → 第 2 代 → 第 3 代：连续两代 .rs manifest 与运行输出逐字节一致
- 前端自举链：LZ 写的 frontend_self.lz 编译自身源码，三代收敛（f2.rs == f3.rs）
- 从干净克隆可复现（脚本仅依赖 cargo/rustc/PowerShell + 仓库内路径）

### CLI 子命令
- `lz create <path>`：脚手架（lz.toml + src/main.lz + README），生成即可构建
- `lz build [dir]`：项目构建（ProjectCompiler + CodeGen + rustc），可复现，`--incremental` 增量
- `lz peek <target>`：查看 tokens/AST/IR（复用 --emit=*）
- `lz check [dir]`：类型/语义检查不产出代码，更新检查缓存
- `lz push [--dry-run] [--registry <path>]`：本地 registry 发布，原子写入，版本冲突保护

### 语法特性矩阵
- 36 份 SYNTAX/ 文档 → 40+ 特性清单
- 正例：DEMO/ 文件自动映射 + 编译验证
- 反例：25+ 语法探针，断言非零退出 + 可读诊断

### 验证
- cargo test 全量：320/0（基线不回归）
- DEMO 全量：261/261
- bootstrap closed 闭环：13/13 RC=0
- 三代收敛：g2 == g3（manifest + runout）
- CLI E2E：全部通过
- 异常注入：6 项全部非零退出 + 无脏状态

### 回滚
- `git reset --hard stable-161~1` 或 `powershell -File bootstrap\rollback.ps1`
- push 回滚：删除 registry 条目目录（原子写入保证旧版本不受影响）

### 已知缺口
- D2 codegen 缺口（impl 块/列表推导/生成器/match 模式）——不阻塞稳定自举（前端收敛已验证）
- 行级覆盖率报告（cargo llvm-cov）——需网络安装工具，本轮不做
- 跨平台安装包——超出本轮范围
