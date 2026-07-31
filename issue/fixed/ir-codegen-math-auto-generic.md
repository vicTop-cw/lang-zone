# 🔴 P1: IR codegen `@math` 自动泛型未实现

**Bug 标题**: IR 路线 `@math` 装饰器不生效，泛型函数硬编码为 `i64`，生成类型不匹配 Rust 代码

**严重等级**: 🔴 P1 — 所有带 `@math` 的文件 rustc 编译失败
**发现日期**: 2026-07-31 15:43
**环境**: commit `488718d`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
@math
def sq(x, y) = x * x + y * y

def main() =
    print(sq(3, 4))        // int 版本
    print(sq(3.0, 4.0))    // float 版本
```

编译: `lang-zone test.lz --emit=ir` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
// IR codegen 产出 — sq 被硬编码为 i64
pub fn sq(x: i64, y: i64) -> i64 {
    return x * x + y * y;
}

pub fn main() {
    println!("{:?}", sq(3, 4));
    println!("{:?}", sq(3.0, 4.0));
    //                 ^^ E0308: expected i64, found floating-point number
}
```

- 只有一份 `sq(i64, i64) -> i64`
- `sq(3.0, 4.0)` 类型不匹配

## 预期结果

`@math` 应该为每个调用点生成对应的单态化版本（或泛型版本 `sq<T: Number>(x:T, y:T) -> T`）。

## 根因

IR builder (`src/ir/builder.rs`) 不识别 `@math` 装饰器，没有对 `@math` 标记的函数进行类型参数泛化。AST→类型推断时已将 `x`, `y` 推断为 `?`（未知），但 IR codegen 在遇到 `sq(3, 4)` 时错误地具体化为 `i64`。

## 影响范围

- `DEMO/04_functions/basic.lz` — `sq(3.0, 4.0)`
- `DEMO/04_functions/composite.lz` — `sq(3.0, 4.0)`
- 所有使用 `@math` 装饰器的文件

## 关联

AST 路径 (src/codegen/) 已正确实现 `@math` 泛型化。IR 路径需补齐。
