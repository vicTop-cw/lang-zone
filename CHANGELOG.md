---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 9f2a11add43fbf12a546606fb2b962ab_9f4bc76d9acc11f1a98a525400f8a581
    ReservedCode1: 5HXw5SYAkMeIB+u+ldEgMTCUo6Ob+mOUe8IurQRGCWpAuwZrunemlEPdctoHuy+ONvfBZGwSw+aiBmQ9yz/2OyHG7Lk5JLn27fv7goM2C9I7gc3/PxkJpYqpdaEvivdRIVS6CQKISQgk97M4yBu3nmZ65VAGjz0Xo4DNpH7ckNrf3yrrUAzwOUKOtSg=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 9f2a11add43fbf12a546606fb2b962ab_9f4bc76d9acc11f1a98a525400f8a581
    ReservedCode2: 5HXw5SYAkMeIB+u+ldEgMTCUo6Ob+mOUe8IurQRGCWpAuwZrunemlEPdctoHuy+ONvfBZGwSw+aiBmQ9yz/2OyHG7Lk5JLn27fv7goM2C9I7gc3/PxkJpYqpdaEvivdRIVS6CQKISQgk97M4yBu3nmZ65VAGjz0Xo4DNpH7ckNrf3yrrUAzwOUKOtSg=
---

# CHANGELOG — Lang-Zone (LZ)

本文件记录 LZ 编译器（`lzc` / `lzcyc`）的版本变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。

## [v0.1.161] - 2026-08-17

### 稳定自举 + CLI 子命令集成

- 完成稳定自举（三代收敛）：宿主编译器处理自举源集 13 个 .lz，连续两代 .rs manifest 与运行输出逐字节一致
- 前端自举链：LZ 写的 `frontend_self.lz` 编译自身源码，三代收敛（f2.rs == f3.rs）
- 新增 CLI 子命令：`lz create` / `lz build`（含 `--incremental`）/ `lz peek` / `lz check` / `lz push`
- 语法特性矩阵：36 份 SYNTAX/ 文档 → 40+ 特性清单；DEMO 261/261；bootstrap closed 13/13 RC=0
- cargo test 全量 320/0（基线不回归）

### 已知缺口

- D2 codegen 缺口（impl 块/列表推导/生成器/match 模式）
- 行级覆盖率报告（cargo llvm-cov）待网络安装工具
- 跨平台安装包超出当前范围

## [v0.1.160] - 2026-08-17

### 自举 100% 里程碑

- 达成自举 100%（v160）：cargo test 320/0、DEMO 261/261、bootstrap closed 13/13 RC=0
- `--emit=rs-lz` 与 Rust codegen 逐字符一致

## [自举 50% 里程碑] - 2026-08-17

- 自举进度过半，前端自举链收敛验证通过

## [v0.1.x] - 2026-07-31 起

### IR 路线决策

- 全力走 IR 中间表示路线：代码生成统一以 LZIR 为中间层（AST → LZIR → 目标语言）
- 旧 `src/codegen/`（AST → Rust 直接 codegen）视为遗留，逐步退役
- 双后端：Rust（`lzc`）与 Cython/Python（`lzcyc`）
*（内容由AI生成，仅供参考）*
