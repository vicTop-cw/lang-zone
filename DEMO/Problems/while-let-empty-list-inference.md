# while_let.lz — IR 构建期空列表元素类型推断失败

- **发现时间 (UTC)**: 2026-08-06T00:00:00Z
- **发现人**: auto-sdet (lz-2 IR 路线测试)
- **严重等级**: P2（边界行为 / 非核心路径 IR 异常；非回归、非数据丢失）
- **路线**: IR-only（LZ → IR → Rust），复现命令 `--emit=ir`，未触碰 AST→RUST 路径

---

## 复现步骤（最小可复现）
源文件 `DEMO/06_control_flow/while_let.lz`（关键片段）：
```lz
def take_until_zero(it: Iterator<int>) -> List<int> =
    let result = []                 // 行 23：空列表，元素类型未标注
    while let Some(item) = it.next():
        if item == 0:
            break
        result.append(item)         // 后续 append 未固化元素类型
    result
```
执行：
```
target/debug/lang-zone.exe DEMO/06_control_flow/while_let.lz --emit=ir
```

## 预期结果
`while let` 已被 IR 支持（新增 `Stmt::WhileLet`），空列表应推断为 `List<int>`（由返回值注解/`append` 用法推导），正常产出 `LZIR v1`。

## 实际结果
```
IR emission error: IR build error: error[E0282]: type annotations needed
  = cannot infer element type for empty list bound to `result`
  = help: give it an explicit type, e.g. `let result: List<T> = []`
```
失败阶段从**解析期**（旧版 `while let` 未实现，报 `Parse error`）后移至 **IR 构建期**。

## 根因分析（IR 路径回溯锚点）
- 前端：`src/parser/stmt.rs` 现已支持 `while let` 语法 → `src/ast/stmt.rs::AstStmt::WhileLet`。
- IR lowering：`src/ir/builder.rs::convert_stmt` 将 `WhileLet` 映射为 `Stmt::WhileLet`（新增节点）。
- IR 构建：`let result = []` 在 IR 中推断为 `List<?>`，但 `result.append(item)` 未反向固化元素类型；
  IR 构建器拒绝 `List<?>` 元素的空列表绑定 → 报 `E0282`。
- **对照**：顶层裸 `let x = []` 可正常 emit（`const x: List<?> = [List<?>] []`），说明该推断缺陷
  仅在「空列表 + 循环体内 append 且元素类型无显式注解」场景下触发，属边界行为。

## 影响范围
- 仅 `DEMO/06_control_flow/while_let.lz` 一个 DEMO 触发（已在 `known_parser_gaps_parse_error` 清单内，
  现应重分类为 IR 构建期推断缺口）。
- 全量 `--emit=ir` 扫描：156 ok / 21 fail，计数与上周期一致（该文件仍 fail，仅失败阶段迁移）。
- IR 路径单元测试 9/9 通过，IR 核心路径未受影响。

## 建议修复（非本周期范围）
在 IR 构建器对「空列表 + 后续 append」场景做元素类型延迟解析（按首次 append 实参类型或函数返回
注解回填），或在该绑定点要求显式类型注解时给出更精准的诊断位置。

## 验证
- 回归校验：`cargo test --test mod` → 9 passed / 0 failed。
- 对照实验：`.test_meta/min.lz`（`let x = []`）成功 emit，证明非全局空列表缺陷。
- 验证人：auto-sdet；验证时间：2026-08-06T00:00:00Z
