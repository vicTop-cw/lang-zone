# T0r52：验证 + 修复记录（2026-11-04）

> 任务：lang-zone 语言迭代开发 T0r52——验证 BUG-499/T0r51 修复 + 推进 embed §6 已知限制第 1/3/4 条 + BUG-485/486 修复验证。
> 约束：遵循 lang-zone 开发标准（AST→LZIR→Rust 路线），编译产物并运行对比验证正确性，不碰 CY 后端。

## 一、验证结果（全部通过）

| 验证项 | 判据 | 结果 | commit |
|--------|------|------|--------|
| BUG-499（embed 短名调用被 i64::MAX 兜底桩遮蔽） | `cargo test --test embed_stub_shadowing` | **4/4** 全绿 | f3d2080（已在 HEAD 祖先） |
| T0r51（值表达式位重赋扫描） | `cargo test --test reassign_scan` | **5/5** 全绿 | fea1bf6（已在 HEAD 祖先） |
| BUG-485（let x: Complex 类型注解发射裸 Complex） | `cargo test --test codegen_corner_fixes complex_bigint_annotation` | **1/1** 全绿 | 44216e4（已在 HEAD 祖先） |
| BUG-486（LINK RECIPE -L dependency 路径） | `cargo test --test find_bug_bugs cg_generated_rs_header_link_recipe` + 实际配方验证 | **1/1** + 端到端 | 4520840（已在 HEAD 祖先） |
| §6.1（静默丢弃返回值，T0r50 已修） | `cargo test --test embed_gapfixes` | **17/17** 全绿 | 9f802e8 |

### BUG-486 实际配方验证

生成 `def main() = let a=1; print(a)` 的 stderr 链接配方：
```
rustc --edition 2021 --extern lz_builtins=...\target\debug\deps\liblz_builtins-*.rlib -L dependency=F:\AI\lang-zone\target\debug\deps -O ...
```
- `-L dependency` 指向 `target/debug/deps` ✅（修复前指向 `target/debug`，照抄即 E0463）
- 按配方 rustc 编译通过（仅 warnings），运行输出 `1` ✅

## 二、embed §6 已知限制推进

### 第 1 条：可序列化闭集外签名不出垫片
- 已在 T0r50 修复（静默丢弃返回值问题）
- 本次验证 `tests/embed_gapfixes.rs` **17/17** 全绿

### 第 3 条：默认路径不起子进程
- 确认是**刻意设计**（文档原文：「避免编译器一跑就 spawn 外部程序成为既成事实」）
- 不修改，行为不变

### 第 4 条：print(<垫片调用>) 返回 str 走 Debug 带引号 —— 已修复

**根因**：`print_arg_is_display`（`src/ir/codegen/expr_gen.rs:200`，static 函数）只看表达式自身 `e.ty`，垫片调用的 `e.ty` 在 IR 层不可见（非 LZ 定义的函数），故返回 false 走 `{:?}`（Debug，带引号）。

**修复**：`src/ir/codegen/expr_gen.rs:1962`，print 格式化选择处把
```rust
Self::print_arg_is_display(a)
```
改为
```rust
Self::print_arg_is_display(a) || self.is_str_producing(a)
```
`is_str_producing`（`expr_gen.rs:8346`）查 `fn_returns`（垫片函数名与返回类型已由 `embed_export_names` 在 `mod.rs:905-910` 登记）判定 Call 表达式是否返回 str。

**零回归论证**：LZ 自定义函数返回 str 的场景两者都覆盖（`e.ty == Str` 时 `print_arg_is_display` 已 true），`||` 不改已有行为，只修复垫片调用这一条之前漏掉的路径。

**判据**（`tests/embed_unify_r15.rs` 新增 2 条）：
- `print_embed_shim_call_returning_str_uses_display_not_debug`：直接 print 垫片 str 调用输出 `AB` 无引号 + 生成代码含 `println!("{}", …)`
- `print_embed_shim_call_returning_int_unchanged`：负向对照返回 int 的垫片调用输出不变

**commit**：6ecb6be（修复+测试）、e38836b（BUG-511 入账 + 文档 §六.4 标记已修复）

## 三、全量回归（零回归）

| 测试套件 | 结果 |
|----------|------|
| `cargo test --lib` | **469/469** |
| `cargo test --test embed_unify_r15` | **23/23**（原 21 + 新增 2） |
| `cargo test --test embed_stub_shadowing` | **4/4** |
| `cargo test --test embed_gapfixes` | **17/17** |
| `cargo test --test codegen_corner_fixes` | **44/44** |
| `cargo test --test lz_ir_bootstrap` | **25/25** |
| `cargo test --test demo_codegen_compile -- --ignored --nocapture` | 全量 DEMO **327/327** |
| `cargo test --test lz_frontend_bootstrap` | **7/7** |
| `cargo build` | 0 error，lib 告警 9（与基线逐条同 kind 同位置） |

## 四、提交

| commit | 内容 |
|--------|------|
| 6ecb6be | fix(embed): T0r52 — print 垫片调用返回 str 走 Display 而非 Debug（§6.4 修复）+ 2 回归测试 |
| e38836b | docs: T0r52 — BUG-511 入账（§6.4 修复已落地）+ 文档 §六.4 标记已修复 |

两个 commit 已 push 到 GitCode master。
