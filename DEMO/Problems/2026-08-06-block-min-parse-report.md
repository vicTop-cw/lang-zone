# Bug 报告：DEMO/Problems/test_block_min.lz IR emit 失败

- 报告时间：2026-08-06T18:00:00Z
- 验证人：auto-sdet
- 严重等级：**P3（测试文件问题 / 文档级）** — 非产品 IR 语义缺陷

## 复现步骤（最小可复现）

```
lang-zone DEMO/Problems/test_block_min.lz --emit=ir
```

文件关键代码（line 9）：
```
    sum:int        # 仅有类型注解，缺少 `= <value>` 初始化器
```

## 预期结果
- 若意图为「声明未初始化变量」，应被类型检查器拒绝或要求显式初始化；
  但本文件经 `--emit=ir` 阶段即失败，未达到 IR 产出。

## 实际结果
```
Parse error: Expected Eq, got Newline at pos 24
```
（pos 24 落在 `sum:int` 声明行，解析器在 `:` 后期待 `=` 但遇到换行。）

## 根因定位（具体发生错误的代码）
- 出错节点：`DEMO/Problems/test_block_min.lz` 第 9 行 `sum:int`
- 隔离验证：构造最小用例 `def main() = sum:int; print(1)` 复现同错误 `Parse error: Expected Eq, got Newline at pos 11`
- 对照验证：命名 block 语法本身正常 —— `block NAME:` + `break NAME` 的最小用例 `--emit=ir` 成功产出
  `block 'b1 { ... break }`（见 `DEMO/Problems/_probe_block.lz` 验证通过）
- 结论：**失败由测试文件使用不完整的 `name:Type` 声明（缺 `=`）引起，解析器行为正确，属测试 fixture 缺陷，非 IR 语义 Bug。**

## 环境信息
- OS：win32 (Windows)
- 分支：master
- commit hash：`5f1e28a`（含 `ea53b73 feat: block named block syntax` + `5f1e28a fix: block IR compress to closures`）
- 工具链：`target/debug/lang-zone.exe --emit=ir`

## IR 路径回溯锚点
- 解析阶段（parser）在声明语句 `:` 后期待 `Eq` token，命中 `Newline` → 提前失败，未进入 IR lowering。
- 相关 IR 节点：`block 'sum_block` / `block 'sum_block2`（命名 block 节点已正确实现，见 `src/ir/builder.rs`、`src/ir/node.rs` 新增命名 block 支持）。

## 处置建议
- 该文件为 `DEMO/Problems/` 下的负向/草稿 fixture，应在文件内补全 `sum:int = <value>` 或改为注释，使其可被 `--emit=ir` 正常降级（不影响产品语义）。
- 因非产品缺陷，**不计入 P0–P3 未解决 Bug**；本轮 32 个已知失败集之外新增的 1 个失败已定位为 fixture 问题，不触发重复 issue。
- 已生成对照探针 `DEMO/Problems/_probe_block.lz`（合法命名 block，通过）供回归参考。

## 备注
- 本轮测试汇总：`cargo test --test mod` 51 passed；`cargo test --test ir_snapshots` 50 passed；
  DEMO `--emit=ir` 193 文件 = 160 OK / 33 FAIL（32 已知 + 1 本 fixture 问题）。
