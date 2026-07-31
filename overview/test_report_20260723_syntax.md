# Lang-Zone 变更语法测试报告

> 日期: 2026-07-23 | 编译器: `target/debug/lang-zone.exe` (commit 工作区未提交)
> 范围: 验证"变更的语法"——变量绑定与所有权语义，覆盖全量回归 + 新增绑定套件

---

## 一、总览

| 测试套件 | 用例数 | 结果 | 说明 |
|---------|-------|------|------|
| 黑盒 #1 (20260722-01) | 39 | ✅ 39/39 | 断言已按新架构更新 |
| 黑盒 #2 (20260722-02) | 51 | ✅ 51/51 | 断言已按新架构更新 |
| 绑定套件 (20260723-binding) | 12 | ✅ 12/12 | **新增**，含 rustc 端到端校验 |
| Rust 单元测试 (cargo test) | 276 | ✅ 276/276 | FFI 并行竞态已修复 |
| **合计** | **378** | **✅ 378/378** | 全过 |

---

## 二、变更的语法（本次重点验证）

Lang-Zone 的绑定语义与 Rust **相反**：默认可变借用。验证矩阵：

| 写法 | 生成的 Rust | rustc | 结论 |
|------|-----------|-------|------|
| `y = 1` | `let mut y = 1;` | ✅ | ✅ 默认可变 |
| `let x = 1` | `let x = 1;` | ✅ | ✅ 不可变 |
| `ref r = a` | `let mut r = &mut a;` | ✅ | ✅ 可变引用（修复后） |
| `let ref s = a` | `let s = &a;` | ✅ | ✅ 不可变引用（修复后） |
| `mut z = 10` | `let mut z = 10;` | ✅ | ✅ 冗余修饰，仍为可变 |
| `v^` (move) | `f(v)` | ✅ | ✅ `^` 后缀 move，Rust 默认即移动 |
| `const N: int = 10` | `let mut N = 10;` | ✅ | 🟡 退化为 let mut（设计决策） |
| `const G: str = "Hi"` | `let mut G = "Hi";` | ✅ | 🟡 修复后编译通过（见缺陷 B） |

---

## 三、测试过程中发现并修复的缺陷

### 🔴 缺陷 A：`ref` / `let ref` 生成非法 Rust（已修复）
**位置**: `src/codegen/stmt.rs` `Stmt::Let`
**根因**: `is_ref` 把 `&` 加在**绑定名前**（`let mut &r = x`），这是引用**模式解构**语法，对值类型非法。
**修复**: `&` 改为加在**值侧**——`ref r = a` → `let mut r = &mut a`；`let ref s = a` → `let s = &a`。
**影响**: 此前所有使用 `ref` 的代码（含 `DEMO/02_basics/variables.lz`）均无法通过 rustc。`^` 正常，不受影响。

### 🔴 缺陷 B：`const` 退化类型不匹配（已修复）
**位置**: `src/codegen/stmt.rs` `Stmt::Const`
**根因**: 函数体内 `const` 退化为 `let mut`，但保留了类型注解（`let mut G: String = "Hi"`），而 `"Hi"` 是 `&str` → 类型不匹配（E0308）。
**修复**: 退化时省略类型注解，交由 Rust 推断（`let mut G = "Hi";`），int/f64/str 字面量均安全。

### 🟡 缺陷 C：FFI 单元测试并行竞态（已修复）
**位置**: `src/bridge/ffi.rs` 测试模块
**根因**: 8 个测试共享同一临时文件 `test_ffi.toml`，Rust 默认并行执行时发生读写竞态（并行 119/120，串行 120/120）。
**修复**: `create_test_manifest(name)` 按测试名生成独立临时文件（`test_ffi_{name}.toml`）。cargo test 现稳定 276/276。

---

## 四、已知缺口（建议后续修复，本次未改编译器）

### 🔴 缺口 1：`owned` 契约检查丢失
`owned` 标志仅对 `self` 生效（`codegen/func.rs:239`）；普通参数的 `owned` 被静默丢弃，且调用方**无需 `^`** 即可传值。旧架构应对"未以 `^` 调用 owned 形参"注入编译错误（原 F15 期望 `compile_error!`）。
**建议**: 在 codegen 校验阶段对 `is_owned` 参数强制调用侧使用 `^`，否则 `compile_error!`。

### 🔴 缺口 2：lz 不拦截不可变重赋值 / 非法 `&mut`
- `let x = 1; x = 2` —— lz 放行（rc=0），要等 rustc 才报 E0384。
- `let x = 1; ref r = x` —— lz 放行，rustc 报 E0596（`x` 不可变却取 `&mut`）。
**建议**: lz 层维护可变性符号表，在绑定/重赋值时校验，给出友好中文报错而非丢给 rustc。

### 🟡 缺口 3：结构体 `String` 字段未做字面量转换
`Person(name: "Bob")` 生成 `Person { name: "Bob" }`，`"Bob"` 是 `&str` 而字段为 `String` → E0308。
**建议**: struct 字段类型为 `String` 且值为 `str` 字面量时，自动包裹 `.to_string()`。

### 🟡 缺口 4：`const` 退化为 `let mut` 语义偏离
函数体内的 `const` 退化为可变 `let mut`，失去了"编译期常量"语义。若需真常量，应考虑顶层 `const` 或 `once_cell`/`lazy_static` 方案。

---

## 五、测试基础设施增强

新增 `test-suite/20260723-binding/run_tests.py`，在通用 harness 基础上扩展 **`rustc` 端到端模式**：
- `rustc`：lz 必须成功生成 `.rs`，且 **rustc 必须编译通过**（rc=0）——真实验证生成代码可运行。
- `rustc_err`：lz 成功生成 `.rs`，但 **rustc 必须编译失败**（rc≠0）——用于固化"当前已知缺陷"，缺陷修复后该用例会自动变红提醒。
- 自动探测 `rustc` 路径（环境需 `rustc` 1.70+），缺失时优雅跳过端到端校验。

**价值**: 旧 harness 仅检查 lz 退出码与字符串断言，无法发现"lz 接受但生成非法 Rust"类缺陷（如本次的 `ref` bug）。新模式下此类问题会被 rustc 拦截。

---

## 六、回归基线对比

| 维度 | 上次报告 (架构变更后) | 本次 |
|------|---------------------|------|
| 黑盒 #1 | 39/39 | 39/39 |
| 黑盒 #2 | 51/51 | 51/51 |
| cargo test | 274 passed / 2 failed | **276 passed / 0 failed** ✅ |
| 绑定套件 | 不存在 | **12/12 新增** |
| 端到端 rustc 校验 | 无 | **已启用** |

---

## 七、后续建议

1. **优先修复缺口 1、2**（所有权契约与可变性校验），这是 Lang-Zone 区别于 Rust 的核心卖点，不应静默放行。
2. 将 `rustc` 端到端模式推广到现有两套件的关键用例，防止"假绿"（lz 过但 rustc 挂）。
3. 补 `tests_buildblock/` 与 `tests_boundary/` 中受架构变更影响的断言（这部分用例此前与变更语法无关，本次未动）。
4. 提交本次改动（ref/const/FFI 修复 + 绑定套件 + 断言更新）至仓库。
