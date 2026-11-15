# Phase 2 模块系统 Bug 修复报告 (Bug-35~39)

## 修复清单

### Bug-35: `from X import Y` 函数调用误生成 `__call_magic`
- **文件**: `src/codegen/expr.rs`
- **改动**: `__call_magic` 守卫条件中增加了 `is_imported_fn` 检查
- **效果**: 通过 `from X import Y` 导入的 `add(3,4)` 正常生成 `add(3,4)` 而非 `__call_magic(add, (3,4))`
- **验证**: ✅ `add(3, 4)` → `add(3, 4)` 普通调用

### Bug-36: `import` 模块缺少 `mod` 声明
- **文件**: `src/codegen/mod.rs`
- **改动**: `use` 语句发出后再遍历一次 imports，非 std 桥接的源模块额外输出 `mod <name>;`
- **效果**: `import test_module_a` 生成 `use test_module_a;` + `mod test_module_a;`
- **验证**: ✅ `mod test_module_a;` 出现在输出顶部

### Bug-37: 模块内函数/结构体缺少 `pub`
- **文件**: `src/codegen/func.rs` + `src/codegen/decl.rs`
- **改动**: 非 `_` 前缀或 dunder 名的函数/结构体自动添加 `pub` 关键字
- **效果**: `def add(...)` → `pub fn add(...)`; `struct Point` → `pub struct Point`
- **验证**: ✅ `pub fn add`, `pub fn greet`, `pub struct Point`

### Bug-38: 模块路径结构体构造函数调用语法
- **文件**: `src/codegen/expr.rs`
- **改动**: 在 `Expr::Call` 中增加 `Expr::PathAccess` 检测分支，当 segment 匹配已知结构体名时生成 `module::Struct { field: val }` 语法
- **现状**: 对同文件/已注册结构体生效；跨文件结构体构造需要多文件编译架构支持（后续阶段）

### Bug-39: `_` 前缀私有性无保护
- **文件**: `src/codegen/func.rs` + `src/codegen/decl.rs`
- **原理**: Bug-37 修复后 `_` 前缀项不生成 `pub`，Rust 模块系统自动拦截跨模块访问
- **验证**: ✅ `fn _internal()` 无 `pub` 前缀

## 回归验证
- `cargo build`: ✅ 0 warning
- `cargo test --lib`: ✅ **399 passed, 0 failed**
- 函数定义语法测试套件: ✅ **12/12 passed**
