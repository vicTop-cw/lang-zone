# Bug: Variadic `..` 生成无效 Rust 代码

**状态**: Open
**严重等级**: 🔴 P0 — 代码生成严重缺陷
**发现日期**: 2026-07-31 15:06
**发现方式**: 自动化回归测试
**环境**: Windows 10, Rust 1.89.0, lang-zone HEAD (cce21c2)

---

## 一、Bug 描述

Variadic `..` 参数标记在 parser 层被正确消费，但 codegen 层未做相应处理，导致生成的 Rust 代码存在两类致命错误：
1. Variadic 参数生成时被当作普通单值参数
2. 调用方传递多个参数但不打包
3. `for in` 循环对非可迭代类型迭代

## 二、复现步骤

### 最小复现

**输入** (`test_variadic.lz`):
```lz
def sum_all(..nums: int) -> int =
    let total = 0
    for n in nums:
        total = total + n
    total

def main() = print(sum_all(1, 2, 3, 4))
```

**执行**: `cargo run -- test_variadic.lz`

**生成的 Rust 代码**:
```rust
fn sum_all(nums: i64) -> i64 {     // ❌ 错误1: 应为 &[i64] 或 Vec<i64>
    let total = 0;
    for mut n in nums {             // ❌ 错误2: 不能对 i64 类型迭代
        total = total + n;
    }
    total
}
// ...
fn main() {
    println!("{}", sum_all(1, 2, 3, 4))  // ❌ 错误3: 传递 4 个参数给 1 参数函数
}
```

## 三、预期结果 vs 实际结果

| 方面 | 预期 | 实际 |
|------|------|------|
| LZ 编译 | 成功 | 成功（LZ 编译器不报错） |
| 生成 Rust 代码 | 可被 rustc 编译 | 不可编译（3 处错误） |
| Variadic 参数类型 | `&[i64]` / `Vec<i64>` | `i64`（丢失 variadic 语义） |
| 调用方参数打包 | `&[1, 2, 3, 4]` | `(1, 2, 3, 4)`（多个独立参数） |

## 四、技术根因

1. **Parser** (`src/parser/parser.rs`): 消费 `..` token 并标记参数为 variadic，但 variadic 标记在参数对象中未被保存/传递给 codegen
2. **Codegen** (`src/codegen/`): 将 `..args: T` 的参数直接映射为 `args: T`（Rust 类型），丢失 variadic 语义
3. **调用方 codegen**: 不对 variadic 参数做参数打包处理
4. **类型检查**: `for in` 不对迭代对象做 `IntoIterator` trait 检查

## 五、影响范围

- **所有**使用 `..` variadic 参数语法的 LZ 代码生成的 Rust 产物均无法被 rustc 编译
- 由于 LZ 编译器本身不报错（静默生成无效代码），属于"编译器撒谎"类问题
- 当前已知 DEMO/DEMO_old 中有 variadic 相关测试文件，需要检查是否受影响

## 六、修复建议

### 方案 A（简单修复）: 拒绝 variadic 并报错
在 codegen 中检测 variadic 参数并返回明确错误："Variadic parameters not yet supported in codegen"

### 方案 B（完整实现）:
1. 在 AST 参数节点中保留 variadic 标记
2. Codegen 将 `..args: T` 生成为 `args: &[T]`
3. 调用方将多个参数打包: `foo(a, b, c)` → `foo(&[a, b, c])`
4. `for in` 检查迭代对象类型

## 七、相关文件

- `src/parser/parser.rs` — variadic 解析
- `src/codegen/stmt.rs` — 函数参数 codegen
- `src/codegen/expr.rs` — 调用方 codegen
- `src/ast/` — 参数节点 variadic 标记
