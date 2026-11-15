# lz-2 自动化记忆

## 最近周期（2026-08-05T01:00Z, 未提交 — 用户拒绝自动 commit）
- 用户任务：修复两处「应报错却被放过」的 LZ 语义缺口（完整 Rust 语义）。
- 修复内容（`src/ir/builder.rs`）：
  - 移除 `collect_reassigned`/`mark_mut` 的「自动提升为 mut」逻辑；改为不可变 `let` 重赋值报 E0384。
  - 空列表字面量元素类型不可推断报 E0282；新增 `resolve_empty_list_elems` 支持 `.append()`/`.push()` 上下文推断（贴合 Rust）。
  - `TypeCtx` 增 `errors: Rc<RefCell<Vec<String>>>` + `report_error`，并在 `convert_fn_def`/顶层 build 共享根 `ctx.errors`；`build_ir` 末尾统一报错。
- **关键设计约束**：LZ 解析器显式忽略 `let mut`（`let` 默认不可变）；真正的可变绑定写法是 `mut x = ...`（无 `let`）。Demo 修正用 `mut x =` 而非 `let mut x`。
- Demo 修正：25 个文件；重赋值 `let X=`→`mut X=`；`primitives.lz`/`ir-edge-empty-collections.lz` 裸 `let x=[]` 加 `List<int>` 注解。
- 门禁：292 lib + 8 ir_snapshots（43/43）全绿。
- 无回归证明：192 demo 全量基线比对，改动前后失败的 23 个完全一致（预存解析器语法限制，与本改动无关，且不在 43 批次内）。
- 报告：`issue/test-report-2026-08-05-0100.md`、`DEMO/Problems/immutable-reassign-empty-list.md`。
- 注意：改动未提交（用户拒绝自动 commit），下次周期 Phase 0 会检测到 builder.rs hash 变化并触发 Phase 1。
