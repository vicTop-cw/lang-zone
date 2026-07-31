# 🔴 P1: IR codegen 默认参数值未生成

**Bug 标题**: IR 路线函数默认参数值丢失，调用缺少参数时 rustc 编译失败

**严重等级**: 🔴 P1
**发现日期**: 2026-07-31 15:43
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

```lz
def greet_default(name: str = "World") -> str =
    f"Hello, {name}"

def main() =
    print(greet_default())       // 无参调用，应使用默认值
    print(greet_default("LZ"))   // 有参调用
```

编译: `lang-zone test.lz --emit=ir` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
pub fn greet_default(name: String) -> String {
    return format!("Hello, {name}");
}
pub fn main() {
    println!("{:?}", greet_default());
    //               ^^^^^^^^^^^^^ E0061: this function takes 1 argument but 0 were supplied
}
```

## 预期结果

```rust
pub fn greet_default(name: Option<String>) -> String {
    let name = name.unwrap_or_else(|| "World".to_string());
    ...
}
// 或生成两个重载 / 使用 builder 模式
```

## 根因

IR builder 在转换函数参数时未保留默认值信息，生成的目标代码直接映射为必需参数。

## 影响范围

- `DEMO/04_functions/basic.lz` — `greet_default()` 无参调用
- 所有使用默认参数的 DEMO 文件
