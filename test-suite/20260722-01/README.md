# 测试运行 20260722-01 — Lang-Zong 编译器测试集成

> 运行日期：2026-07-22 ｜ 归档路径：`test-suite/20260722-01/`

本次为 `test-suite/` 下的**首次（当前）完整测试运行**，覆盖功能 / 边界 / 构建块 / 异常四大类，共 **39** 个用例。

## 目录内容

| 文件 / 目录 | 说明 |
|-------------|------|
| `TEST_PLAN.md` | 测试计划（范围 / 目标 / 类别 / 优先级 / 用例目录） |
| `run_tests.py` | 黑盒测试驱动（39 用例，三种模式：tokens / ast / rust / error） |
| `SUGGESTIONS.md` | 改进建议（针对发现问题的具体优化方向） |
| `reports/TEST_REPORT_001.md` | 详细测试报告（范围 / 结果 / 缺陷与风险 / 通过率） |
| `_work/` | 运行期产物（输入 `.lz`、生成 `.rs`、`results.json`） |
| `cases/` | 用例源占位（权威定义见 `run_tests.py` 的 `CATALOG`） |

## 如何运行

```bash
cd test-suite/20260722-01
python3 run_tests.py          # 或托管解释器：<python> run_tests.py
```

- **SUT**：`../../target/debug/lang-zong.exe`（CLI：`lzc <file.lz> [--tokens] [--ast]`）
- **输出**：
  - 控制台逐用例 ✅/❌ 与汇总
  - `20260722-01/_work/results.json`：完整机器可读结果

## 结果摘要

- **39 / 39 通过，0 失败，0 崩溃（退出码 101）**，总通过率 **100%**
- 优先级：P0 = 10/10，P1 = 29/29
- 分类：功能 15/15、边界 11/11、构建块 7/7、异常 6/6

## 关键发现

- 🟡 **D1 字面量溢出被静默处理**（整数→0、浮点→+inf），应编译期拦截 —— 详见 `SUGGESTIONS.md` 与报告 §3。
- 💭 D2 未知类型名透传、D3 if/match 表达式层、D4 缩进错配静默 —— 观察/增强项。
- ✅ D5 异常处理健壮性优秀（零 panic）、D6 `owned` 契约设计巧妙。

> 完整内容见 `reports/TEST_REPORT_001.md`；优化方向见 `SUGGESTIONS.md`。
