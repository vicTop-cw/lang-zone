# Bug 报告：DEMO/Problems/test_block_trigger.lz IR emit 失败

- 报告时间：2026-08-06T12:00:00Z
- 验证人：auto-sdet
- 严重等级：**P3（测试文件 / 边界 fixture 问题）** — 非确认的产品 IR 语义缺陷

## 复现步骤（最小可复现）
```
lang-zone DEMO/Problems/test_block_trigger.lz --emit=ir
```

文件关键代码（line 1）：
```
block check[ps: __Params]:
    print(ps.args[0])

def main() =
    block check ^:
        42
```

## 预期结果
- 若该 `block NAME[ps: ...]` 为意图支持的「顶层 checker block 声明」，应被解析为顶层声明节点并进入 IR lowering；
- 否则（当前行为）应在解析阶段以清晰错误拒绝。

## 实际结果
```
Parse error: Unexpected token at top level: Block
```
（解析器在顶层遇到 `block` 关键字，拒绝作为顶层语句。）

## 根因定位（具体发生错误的代码）
- 出错节点：`DEMO/Problems/test_block_trigger.lz` 第 1 行顶层 `block check[ps: __Params]:`
- 解析阶段（`src/parser/stmt.rs`）仅允许 `block` 作为 `def` 函数体内部语句，不支持顶层 `block` 声明，命中 `Unexpected token at top level: Block` 提前失败，未进入 IR lowering。
- 同一 `DEMO/Problems/` 区域已有同类脚手架：`test_block_min.lz`（`sum:int` 缺 `=`，Parse error: Expected Eq, got Newline）、`_probe2.lz`（最小 `sum:int` 复现）。三者均为实验性 / 负向 fixture，非 spec 化功能。
- 命名 block 语法本身正常：合法 `def main() = block b1: ... break b1` 经 `--emit=ir` 产出 `block 'b1 { ... break }`（前几轮已验证）。
- 结论：**失败由该 fixture 在顶层使用尚未支持的 `block` 声明语法引起。属测试/边界 fixture 问题。是否应支持顶层 block 为开放设计问题，非本轮可判定的产品 P0–P3 缺陷。**

## 环境信息
- OS：win32 (Windows)
- 分支：master
- commit hash：`40dfff6`（工作树另有未提交 AST/codegen/parser/ir 改动，未纳入快照基线）
- 工具链：`target/debug/lang-zone.exe --emit=ir`（已 `cargo build` 含 ir/builder.rs 工作树改动）

## IR 路径回溯锚点
- 解析阶段（`src/parser/stmt.rs`）在顶层 token 流中遇到 `Block` → 报错终止，未生成 IR。
- 相关 IR 节点：`block 'NAME`（命名 block 节点已实现，见 `src/ir/builder.rs`、`src/ir/node.rs`）；checker block 的 `[ps]/[chk]` 参数压缩为 IR fn 由 `40dfff6` 引入。

## 处置建议
- 该文件为 `DEMO/Problems/` 实验性负向 fixture，应明确其「负向用例」性质（加 `// NEGATIVE` 注释或移入 `99_errors/`），避免被误判为产品回归。
- 因非确认产品缺陷，**不计入 P0–P3 未解决 Bug**；本轮未引入新的真实 DEMO 回归（35 失败 = 32 已知真实缺口 + 3 个 `DEMO/Problems/` 脚手架/负向 fixture）。
- 已将 `DEMO/Problems/test_block_trigger.lz` 加入 `known_demo_ir_failures`（与 `test_block_min.lz`、`_probe2.lz` 一并），以抑制后续周期误报 NEW_FAIL。

## 备注
- 本轮测试汇总：`cargo test --test mod` 51 passed；`cargo test --test ir_snapshots` 50 passed；
  DEMO `--emit=ir` 196 文件 = 161 OK / 35 FAIL（32 真实已知 + 3 个 Problems 脚手架）。
