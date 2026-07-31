# 🔴 P0: IR codegen match 臂无 `case` 关键字时 `.` 不转换为 `::`

**Bug 标题**: IR 路线 match 臂 `Color.Red =>` 生成 `Color.Red =>` 而非 `Color::Red =>`，导致 rustc 编译失败

**严重等级**: 🔴 P0 — 核心 match 功能受阻
**发现日期**: 2026-07-31
**环境**: commit `a542178`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// enum.lz (match 臂不用 case 关键字)
def describe_color(c: Color) -> str =
    let desc = match c:
        Color.Red => "red"
        Color.Green => "green"
        Color.Blue => "blue"
```

编译: `lang-zone enum.lz` → 生成 .rs → `rustc enum.rs`

## 实际结果

```rust
// 生成的 enum.rs (异常)
match c {
    Color.Red => {      // ❌ . 未转为 ::
        "red".to_string()
    }
    Color.Green => {    // ❌ . 未转为 ::
        "green".to_string()
    }
}
```

rustc 错误: `not a pattern` — Rust 将 `Color.Red` 视为字段访问表达式，不是模式

## 预期结果

```rust
match c {
    Color::Red => {     // ✅ :: 正确
        "red".to_string()
    }
    Color::Green => {
        "green".to_string()
    }
}
```

## 根因分析

IR codegen 对 `case Color.Red =>` （带 `case` 关键字）能正确翻译 `.` → `::`，但对无 `case` 的 `Color.Red =>` 直接保留 `.`。

| LZ 写法 | 生成 Rust | 结果 |
|---------|----------|------|
| `case Color.Red =>` | `Color::Red =>` | ✅ 正确 |
| `Color.Red =>` | `Color.Red =>` | ❌ 错误 |

## 影响范��

- `DEMO/07_data_structures/enum.lz` — match 臂全用无 `case` 语法
- 所有使用 `match x: Pattern => body`（无 `case`）语法的文件

## 修复建议

IR codegen 在处理 match 臂时，统一将 LZ 模式中的 `.` 路径分隔符转为 Rust 的 `::`，无论是否有 `case` 关键字。
