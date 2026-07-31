# Bug 批量修复最终状态报告

> 日期: 2026-07-25 | 全阶段 Bug 修复

---

## ✅ 本轮总修复：21 个 Bug

### 自举 Bug（Bug-25~29）
| Bug | 描述 | 修复方式 | 状态 |
|-----|------|----------|------|
| Bug-25 | 桥接 Result 自动 `.unwrap()` | `CallResolveResult.ret_result` + `result = true` | ✅ |
| Bug-26 | 字符串链式拼接 `format!` | `expr_is_string_like` + Binary Add | ✅ |
| Bug-27 | `StrLit` 自动 `.to_string()` | stmt.rs + func.rs 全覆盖 | ✅ |
| Bug-28 | RustBridge 跳过冗余 `use` | `gen_import` 检测 bridge::rust 前缀 | ✅ |
| Bug-29 | typer `String+&str` 误报 | zonk 检测字符串类型跳过 Int | ✅ |

### Phase 1~6（Bug-30~58）
| Bug | 描述 | 修复方式 | 状态 |
|-----|------|----------|------|
| Bug-30 | 桥接构造器 `PathBuf::from()` | bare import 检测 | ✅ |
| Bug-31 | HashMap 缺类型注解 | 已知泛型类型加 `::<_>` | ✅ |
| Bug-32 | 函数实参 StrLit→String | `gen_call_arg` `.to_string()` | ✅ |
| Bug-35 | `from import` → `__call_magic` | is_imported_fn 排除 | ✅ |
| Bug-36 | 多文件依赖编译 | `compile_dep` 函数 | ✅ |
| Bug-37 | 模块项缺 `pub` | 已在 func.rs/decl.rs | ✅（已有） |
| Bug-42 | `self.field` 从 `&self` 移动 | FieldAccess self→`.clone()` | ✅ |
| Bug-47/58 | `__call_magic` 误触发 | 直接 `name(args)` | ✅ |
| — | magic.rs borrow-after-move | `self_param.clone()` | ✅（附带） |

### Phase 7~16（Bug-59~66 + 新 Bug-35~48）
| Bug | 描述 | 修复方式 | 状态 |
|-----|------|----------|------|
| Bug-59 | 空列表 `[]` → `Vec<>` | 空泛型参数→`_` | ✅ |
| Bug-36/43 | `None` → `Option<>` | 同上 | ✅ |
| Bug-39 | `Ok(42)` → `Result<i64, >` | 同上 | ✅ |
| Bug-42/phase12 | `Err("...")` → `Result<, String>` | 同上 | ✅ |
| Bug-40/phase12 | 双重 `.to_string()` | Err 用 raw gen_expr | ✅ |
| Bug-63 | `s.len()` → `usize` vs `i64` | `local_vars` 类型表 + `as i64` | ✅ |
| Bug-64 | `s[i]` 字符串索引 | `chars().nth().unwrap()` | ✅ |

## 🔄 自动覆盖的 Bug
- Bug-33/34（Result 链式）→ 被 Bug-25 覆盖 ✅
- Bug-41/52/53（`&str+String` 顺序）→ 被 Bug-26 `format!` 覆盖 ✅
- Bug-62（字符串拼接）→ 被 Bug-26 `format!` 覆盖 ✅
- Bug-66（枚举变体 StrLit）→ 被 Bug-32 `gen_call_arg` 覆盖 ✅
- Bug-44/phase12（闭包 `__call_magic`）→ 被直接调用修复覆盖 ✅

## ❌ 遗留待修复
| Bug | 描述 | 复杂度 | 类型 |
|-----|------|--------|------|
| Bug-38 | 模块路径 `M.Point()` 构造 | 高 | 跨模块 struct |
| Bug-60 | 嵌套列表索引 move | 低 | `.clone()` |
| Bug-61 | `pop()` 返回 `Option` | 低 | 解包 |
| Bug-65 | Enum match 前缀 | 中 | 解析器 |
| Bug-45 | `print(T)` 缺 Debug 约束 | 中 | 泛型约束 |
| Bug-48/49 | 泛型方法 impl 块 | 高 | 架构级 |
| Bug-51 | 单行 if 解析 | 中 | 解析器 |
| Bug-55 | try/catch match 语法 | 中 | 控制流 |
| Bug-56 | guard let else + return | 中 | 控制流 |
| Bug-57 | 闭包 fn 指针捕获 | 中 | 类型推断 |
| Bug-47/phase12 | 安全导航 `?.` | 中 | 代码生成 |
| Bug-43/phase12 | 单行 `def x() = expr` | 中 | 解析器 |

## 测试状态
- `cargo test`: **399/399 通过** | `cargo build`: 零警告
- `bootstrap/main.lz` → **rustc 零错误编译**
