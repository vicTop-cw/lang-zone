# -*- coding: utf-8 -*-
"""FIND_BUG.md 追加轮次 16 记录（只增不删不改）"""
import io

report = """
## 轮次 16（2026-09-26 · wf-20260926 第 1 轮）：BUG-1~5 全部修复转正 + 新确诊 BUG-6

### FIST 任务链
根任务 T0r8（namespace lz-wf-20260926，Omega 强验证 + laya_decide + issue_scan + call_log 全程开启）。
寻虫 T0r8.1 → 修复 T0r8.2 → 验证 T0r8.3 → 打磨 T0r8.4，叶子全部 Omega 闭环
（spec_create → spec_review → execute → run_check → submit → result_verify → verify）。

### 根因与修复（单点收敛）
| Bug | 根因 | 修复 |
|-----|------|------|
| BUG-1 (high) | codegen 裸发 `use num_bigint::BigInt`，孤立 crate E0432 | lz_builtins re-export `BigInt`/`Complex64`；codegen 全部 23 处 num_* 路径改 `lz_builtins::*` 全限定路由，删除 use 注入 |
| BUG-2 (high) | `types.rs::from_ast_type` 缺 BigInt/Complex 臂 → 注解沦为 `Named("bigint")`，gen_const_def 包装臂永不命中 | from_ast_type 补 `bigint/BigInt → IrType::BigInt`、`complex/Complex/Complex64 → IrType::Complex` |
| BUG-3 (medium) | needs_lazy 集合不含 BigInt → BigInt 运算落 const 上下文 E0015 | gen_const_def needs_lazy + lazy_static_names 预扫描补 `IrType::BigInt | IrType::Complex` |
| BUG-4 (critical) | module_uses_bigint 不扫函数体 + 函数体 bigint 注解坍塌 i128 | 新增泛型遍历器 block_ty_hits/stmt_ty_hits/expr_ty_hits，FnDef 检测加函数体扫描 |
| BUG-5 (medium) | gen_const_def 不注入 current_expected_ty（Let 路径有）+ comptime_value_to_lit 空容器有损内联为 () | gen_const_def 注入/还原 expected；comptime_value_to_lit 空容器/含不可内联元素返回 None 回退静态引用 |
| BUG-6 (medium·新) | bigint 注解 let 位接受、struct 字段/返回类型位报「未知类型」；ex_check_stmts let 绑定忽略注解 | ast::builtin_type_names 补 bigint/complex 族；ex_check_stmts Let/Const 注解类型优先；kwarg 构造 + StructCtor BigInt/Complex 字面量包装（超 i64 补 i128 后缀） |

### 附带修复（修复过程暴露的基建缺口）
lz_builtins re-export 后产生 num-* 传递依赖，rustc `--extern` 单独无法解析 →
全部 12 个测试脚手架 + src 内部 4 处 rustc 调用补 `-L dependency=target/debug/deps`。

### 转正与回归
- `tests/find_bug_bugs.rs`：hunt_bug1~bug6 共 7 个 `full()` 用例转正（find_bug_bugs 47 passed）。
- 全量闸门：**680 passed / 0 failed**（基线 673 + 新增 7，零回归）；demo_codegen_compile 269 项 DEMO 全绿。
- 复现样例：`FIND_BUG/hunt-20260926/bug-A~F/`；BUG-6 账本 memory/bugs.md。
"""

with io.open(r'FIND_BUG\FIND_BUG.md', 'a', encoding='utf-8', newline='') as f:
    f.write(report)
print('appended')
