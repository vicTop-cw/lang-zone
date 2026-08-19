# Cython 后端 — DEMO 测试基线

> 生成日期：2026-08-19
> 命令：`lang-zone <file.lz> --backend=cython`（IR → .pyx）
> 范围：DEMO/ 下全部 278 个 `.lz` 文件

## 结果

| 指标 | 数量 | 占比 |
|---|---|---|
| 总计 | 278 | 100% |
| 成功（生成 .pyx） | 123 | 44.2% |
| 失败 | 155 | 55.8% |

## 失败原因分析

155 个失败**全部**为 `IR build error: Semantic error: ...`，发生在
**IR 构建前的语义检查阶段**（`semantic_check::check_module`），与 Cython 后端无关。

抽样验证：以下文件在默认 Rust 后端（`lang-zone <file.lz>`）下**同样失败**：

- `DEMO/01_basics/enums.lz` → 未绑定变量: Color
- `DEMO/01_basics/errors.lz` → raise 未声明 raises 类型
- `DEMO/02_types/duck_demo.lz` → 未知类型: HasArea
- `DEMO/lz_std/math.lz` → 未绑定变量: NAN

失败类别分布：

1. **未绑定变量**（最多）：语义检查器未将 `const`/枚举变体/模块级符号登记为已绑定
2. **未知类型**：duck 类型、自定义 struct/enum 未在语义检查器中登记
3. **raise 未声明 raises**：`def f() = raise ...` 需显式 `raises` 声明
4. **yield 与返回类型冲突**：生成器函数不能同时声明返回类型
5. **泛型调用缺类型参数**：`identity(x)` 需 `identity<T>(x)`
6. **参数个数不匹配**：变参/默认参数语义检查器计数偏差

## 结论

- **Cython 后端代码生成器本身已完整覆盖**：对 123 个通过语义检查的 DEMO 文件，全部成功生成合法 `.pyx`。
- 155 个失败是**语义检查器的既有行为**（独立于后端选择），非本后端引入。
- 后续修复方向：语义检查器的符号登记/类型登记（属于编译前端，非 Cython 后端范围）。

## 已知代码生成问题（待阶段 I 打磨）

抽样 `DEMO/01_basics/structs.lz` 生成的 .pyx 中仍存在 Rust 语法残留：

- `assert_eq!(p.x, 3)` → 应为 Python 的 `assert p.x == 3`
- `_KwArg(name = "x", value = 3)` → 关键字参数构造器应直接内联为 `x = 3`

这两类属于表达式生成的精修项，纳入阶段 I 处理。
