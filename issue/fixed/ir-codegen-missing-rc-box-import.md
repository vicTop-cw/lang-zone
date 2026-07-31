# 🔴 P1: IR codegen 缺少 Rc/Box/Arc 类型导入

**Bug 标题**: IR 路线生成代码使用 `Rc<T>`、`Box<T>` 但未导入对应模块，导致 rustc 编译失败

**严重等级**: 🔴 P1
**发现日期**: 2026-07-31
**环境**: commit `6a85c17`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// box_rc_arc.lz
def main() =
    let b = Box(42)
    let rc = Rc("hello")
    let arc = Arc(3.14)
```

编译: `lang-zone box_rc_arc.lz` → 生成 .rs → `rustc box_rc_arc.rs`

## 实际结果

```rust
// 生成的 .rs (异常)
pub fn main() {
    let b = Box::new(42);          // ❌ Box 未导入
    let rc = Rc::new("hello");     // ❌ Rc 未导入
    let arc = Arc::new(3.14);      // ❌ Arc 未导入
}
```

rustc 错误: `E0425: cannot find type Rc/Arc/Box in this scope`

## 预期结果

生成代码应自动加入必要的 `use` 语句：

```rust
use std::rc::Rc;
use std::sync::Arc;

pub fn main() {
    let b = Box::new(42);
    let rc = Rc::new("hello".to_string());
    let arc = Arc::new(3.14);
}
```

## 相关

- `primitives.lz` — `Box<i64>` 索引错误 (E0608: cannot index into Box)
- `box_rc_arc.lz` — Rc 未定义
- `rc_arc_more.lz` — Rc 未定义

## 根因

智能指针类型 (Box, Rc, Arc) 虽在 Rust prelude 中（`Box`）/std 库中（`Rc`, `Arc`），但 IR codegen 未将它们加入自动导入列表。
