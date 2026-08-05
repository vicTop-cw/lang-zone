# Bug 报告：identifiers.lz magic 方法置于模块级导致 IR 解析失败

- **Bug 标题**：`DEMO/01_basics/identifiers.lz` 的 `magic __str__` / `magic __add__` 定义在 struct 块外（模块级），IR 解析报 `Expected Colon, got LParen`
- **严重等级**：P1（回归失败 — DEMO 全量 IR 快照测试 `ir_demo_snapshots` 中断）
- **类型**：回归失败（由 demo-proofread A14 编辑引入）
- **报告时间（UTC）**：2026-08-05T17:18:00Z
- **报告人**：auto-sdet

## 复现步骤（最小可复现用例）

```lz
struct MyStruct =
    data: int

magic __str__(self: MyStruct) -> str = "MyStruct"   // 错误：模块级
```

编译命令（IR 路径）：

```
lang-zone DEMO/01_basics/identifiers.lz --emit=ir
```

## 预期结果

文件成功解析为 LZIR，struct 携带 `__str__` / `__add__` 魔法方法，IR 包含 `method ... s1.__add__(...)`.

## 实际结果

```
Parse error: Expected Colon, got LParen at pos 13
```

隔离测试确认：`magic NAME(...)` 在模块级（struct 块外）一律报该错；在 struct 块内（缩进）则正常。对比 `DEMO/boundary-coverage/edge-keyword-identifier.lz`（magic 方法定义在 struct 内）可正常 IR 编译。

## 根因

`src/parser/parser.rs` 的 `magic` 声明解析路径（L1156 起）要求 magic 方法处于 struct 体作用域；模块级 `magic` 头部被当作普通 item 解析，遇到 `(` 时期待 `:`（Colon）而非参数列表，从而报错。demo-proofread 报告的 A14 建议「魔法方法统一为行内形式」被误实施为「移到模块级行内」，破坏了作用域约束。

## 环境信息

- OS：Microsoft Windows NT 10.0.26200.0
- 分支：master
- commit（引入）：3f0c860（demo-proofread A14 编辑）
- commit（验证基线）：7571650
- IR 路径锚点：`tests/ir_snapshots.rs::ir_demo_snapshots` → `DEMO/01_basics/identifiers.lz` → parser `magic` 分支（parser.rs L1154-1200）

## 修复

将 `magic __str__` / `magic __add__` 移回 `struct MyStruct` 块内（缩进），并统一为行内 `= expr` 形式：

```lz
struct MyStruct =
    data: int

    magic __str__(self: MyStruct) -> str = "MyStruct"

    magic __add__(self: MyStruct, other: MyStruct) -> MyStruct =
        MyStruct(data: self.data + other.data)
```

- **修复 commit hash**：7571650
- **验证时间（UTC）**：2026-08-05T17:18:00Z
- **验证人**：auto-sdet
- **验证结果**：`cargo test --test ir_snapshots` → 8 passed / 0 failed（`ir_demo_snapshots` 43/43 通过）；`reject_errors` 1/1 通过。
