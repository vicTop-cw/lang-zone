# 99_errors 目录 10/15 文件为纯注释占位

- **状态**：✅ 已解决（占位文件已于 2026-07-31 ~13:51 回填，harness 现覆盖全部 15 个负例）
- **优先级**：P2（测试覆盖率缺口）
- **发现日期**：2026-07-31 13:00
- **发现方式**：自动化测试审计（test-report-2026-07-31-1300.md #N2）
- **位置**：`DEMO/99_errors/`

---

## 一、问题描述

`DEMO/99_errors/` 目录有 15 个 `.lz` 文件，但其中 10 个文件仅含注释，无有效代码。`reject_errors` 测试自动跳过纯注释文件，因此这些文件**不参与任何测试**。

## 二、跳过的文件清单

| 文件名 | 应该测试的错误类型 |
|--------|-------------------|
| `00_lexical_errors.lz` | 词法错误（但 00a-00d 已覆盖） |
| `01_type_errors.lz` | 类型错误 |
| `02_variable_errors.lz` | 变量错误 |
| `03_function_errors.lz` | 函数错误 |
| `05_control_flow_errors.lz` | 控制流错误 |
| `07_module_errors.lz` | 模块错误 |
| `09_error_handling_errors.lz` | 错误处理 |
| `10_concurrency_errors.lz` | 并发错误 |
| `99_syntax_errors.lz` | 语法错误 |
| `99_type_errors_new.lz` | 类型错误（新版） |

## 三、有效文件（5 个）

- `00a_lexer_invalid_hex.lz` ✅ 正确拒绝：无效的十六进制数字
- `00b_lexer_bad_exponent.lz` ✅ 正确拒绝：科学计数法缺少指数
- `00c_lexer_overflow.lz` ✅ 正确拒绝：整数溢出
- `00d_lexer_unterminated_string.lz` ✅ 正确拒绝：未终止字符串
- `duck_demo.lz` ✅ 正确拒绝：duck 类型相关错误

## 四、建议

1. 补充实际错误代码到纯注释文件中
2. 或如果不打算填充，将文件移至 `99_errors/_todo/` 子目录避免误导
3. 新增 `go` 语句的错误边界测试（如 `go` 在非函数上下文）

## 五、整改状态（2026-07-31 续）

本 issue 提出时（13:00）`99_errors/` 确有 10 个纯注释占位文件；但在 ~13:51 这些文件已被回填为真实负例代码片段（见各文件 mtime）。实测 `tests/reject_errors.rs` 的纯注释跳过逻辑（`source.lines().filter(|l| !l.trim().starts_with("//")).count() == 0`）现已对全部 15 个文件返回 >0，即 **harness 当前覆盖全部 15 个负例**，不再有静默跳过的占位文件。

结论：**本 issue 已自然解决，无需额外动作。** 原「建议 1（补充代码）」已落实；「建议 2（移至 _todo/）」不再需要；「建议 3（新增 go 错误边界）」属功能测试扩充，可单独排期，非本 issue 范畴。

## 六、关联

- 关联报告: `test-report-2026-07-31-1300.md` #N2
