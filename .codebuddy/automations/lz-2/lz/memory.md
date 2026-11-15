# LZ 开发自动化记忆

## 最近运行: 2026-08-03 (Round 20)

### 状态
- rustc 实际编译通过率: 111 → 119/129 core（145 total 含 boundary-coverage）
- 核心失败从 18 降至 10

### 本轮完成 (Round 20)

1. **修复非尾 match 语句 E0308** (ir/codegen.rs)
   - match 非尾语句产出非 () 值时用 `let _ = match {...};` 丢弃值
   - 修复 keywords.lz（原 match 值被误判为函数返回值）

2. **全局变量分析过度提升 E0530** (ir/codegen.rs)
   - `Stmt::Let` 现在加入遮蔽集（原 `let x = ...` 不遮蔽 x）
   - 构建块（闭包）内裸赋值视为局部声明
   - 修复 var_call_block 中 a/b/c 被误提升为 static mut

3. **C 风格三元运算符 `cond ? a : b`** (parser/expr.rs)
   - 新增 is_ternary_after 区分三元中缀 `?` 与错误传播后缀 `?`
   - 修复 combo_match_ternary

4. **恢复 Arc/Display prelude 导入** (ir/codegen.rs)
   - 修复用户"remove unused imports"导致的 generics/box_rc_arc/rc_arc_more 回归

5. **__Params 使用 std::boxed::Box** (ir/codegen.rs)
   - 避免与用户自定义 `struct Box<T>` 冲突（combo_generic_struct_method）

6. **幂运算符 `**` 修复** (ir/codegen.rs)
   - 移除重复的 i64 后缀（2i64_i64 → 2i64）
   - 指数参数 cast 为 u32（.pow() 需要 u32）
   - 修复 ir-edge-boundary-values

7. **String 值语义 clone** (ir/codegen.rs)
   - 用户函数参数为 String 且实参为 String 变量时自动 .clone()
   - 避免 E0382 use of moved value（如 parse_int(s)? 两次）
   - 修复 try_more

### 剩余核心失败 (10)
- 不完整 demo（引用未定义函数）: guard_for_3, panic_raise_try
- 深特性簇: checker (__Params 下转型), operators/precedence (LazyLock/globals),
  var_call_block (~: dict/var 拆包), duck_test (duck typing), iterator_demo (Iter),
  combo_ternary_walrus (walrus+三元语义)
- 用户修改: guard.lz

### 下一步
- 优先: var_call_block ~: 拆包、combo_ternary_walrus 语义
- 标注不完整 demo（guard_for_3/panic_raise_try）为未实现特性
