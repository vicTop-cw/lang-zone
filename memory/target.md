# 当前阶段目标（Target）

- 本轮聚焦：
- 验收条件：

---

## 2026-10-17 R26（marvis-probe）：推导式 filter 闭包 `&` 参数模式修复

- 焦点：`strip_lambda_type_with_ref`（helpers.rs）passthrough 导致推导式 filter 闭包缺 `&`，`nesting-data-types.lz`/`nesting-closure-lambda.lz` rustc 编译失败（E0308）。
- 根因：该 helper 只剥类型注解，既不判 body 是否已含 `*param` 解引用，也不补 `&` 模式；旧硬加 `&` 又与 body 已生成的 `*x` 冲突（E0614）。
- 修复：按 body 决策——body 含 `*param` → 保留裸参数；否则加 `&` 前缀。重建时 after 跳过闭管 `|`（避免 `|&item||` E0214）。
- 判据：demo_codegen_compile 全量闸门失败数 20（passthrough 对照）→ 13（本修复），净收益 7 项、零新增回归；magic_demos 全绿；lib 441 / cy_codegen_gate 2 / perf_opt_gates 44 / str_boundary 3 / dual_route 4 / incremental_golden 1 / ir_pass_fixes 4 / lzc_* / moddec_* 全绿。剩余 13 项 DEMO 失败在 passthrough 对照下同样存在，属既有 E0308/E0282 基线。
- 交付：helpers.rs 函数体 + memory/bugs.md BUG-261 台账。
