# 🔴 P1.5: IR codegen `await` 关键字错误

**Bug 标题**: IR 路线生成代码在变量名/上下文中使用 `await`，与 Rust 关键字冲突

**严重等级**: 🟡 P1.5
**发现日期**: 2026-07-31
**环境**: commit `6a85c17`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// keywords.lz 中包含 await 作为关键字演示
// async_spawn.lz 使用 await 语法
def main() =
    let result = await task
```

编译: `lang-zone keywords.lz` → 生成 .rs → `rustc keywords.rs`

## 实际结果
rustc 错误: `error: incorrect use of 'await'`

生成代码中 `await` 出现在了错误的语法位置（非 `.await` 后缀）。

## 预期结果
若 `await` 是 LZ 关键字，应正确转换为 Rust 的 `fut.await` 或 `await!()` 语法。

## 影响范围
- `DEMO/01_basics/keywords.lz`
- `DEMO/11_concurrency/async_spawn.lz`
- `DEMO/11_concurrency/async_more.lz`
