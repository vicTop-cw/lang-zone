# 语法规范文档 + 3 特性规范测试

## 任务背景
用户指出 DEMO 偏少，要求：先写几个语法规范文档（部分需修改原语法文档），再为每个新特性加 2–4 个 lz 规范测试。范围经确认限定为 **3 个新特性**，规范测试放入 `DEMO/99_spec/`（不参与解析测试，符合未实现特性惯例）。

## 三特性、落点与实现状态

| 特性 | 修改的原文档 | 新增章节 | 当前实现状态 |
|------|------------|---------|------------|
| 下划线 `_` 偏应用 / 闭包简化 + `_ = expr` 丢弃语句 | `SYNTAX/00-词法基础.md` §4.3 | §4.3.1 占位偏应用、§4.3.2 丢弃语句（并扩展用法表） | 偏应用**已有源码影子**（`src/codegen/expr.rs` `replace_placeholders`）；`_ = expr` 语义待明确 |
| `~` 后缀命名参数糖 | `SYNTAX/12-操作符.md` | §1.19 `~` 后缀命名参数糖 | **未实现**（parser 无后缀 `~` 处理）；与前缀 `~x`（位非）按位置区分 |
| `for … if` / `while … if` 循环守卫 | `SYNTAX/05-控制流.md` | §3.5 for / while 守卫 | **未实现**（for/while 分支不识别 `if` 守卫子句） |

## 新增规范测试（DEMO/99_spec/，共 9 个）
- 下划线：`underscore_partial_1.lz`（单洞）、`underscore_partial_2.lz`（多洞）、`underscore_discard_3.lz`（丢弃语句）
- `~` 糖：`tilde_named_arg_1.lz`（用户示例 `name(b, b~)`）、`tilde_named_arg_2.lz`（重排形参）、`tilde_named_arg_3.lz`（混合传参）
- 守卫：`guard_for_1.lz`（for-if）、`guard_for_2.lz`（for-if + else）、`guard_for_3.lz`（while-if）
- 已同步更新 `DEMO/99_spec/README.md` 文件清单表。

## 验证
- `cargo test --test compile_demos` → **1 passed**，无回归（`99_spec/` 被测试显式跳过，主 demo 计数不变）。
- 本次仅改动 `.md` 文档与新增 `.lz` 规范目标文件，未触碰 Rust 源码，lib 测试不受影响。

## 后续建议
- 实现 `~` 后缀与 `for/while … if` 守卫：需在 `src/parser/stmt.rs`（for/while 分支）与调用实参解析处增加处理。
- 偏应用：在现有 codegen `replace_placeholders` 基础上补齐边界（如 `_ = expr` 丢弃语句的语句级语义）。
