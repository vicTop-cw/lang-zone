# 🔴 P0: IR codegen match 枚举变体生成表达式而非模式

**Bug 标题**: IR 路线 match 臂中枚举变体路径（如 `Color.Red`）被生成为表达式，Rust 编译器要求模式

**严重等级**: 🔴 P0 — 导致 ~10+ 个 DEMO 文件 rustc 编译失败
**发现日期**: 2026-07-31 16:10
**环境**: commit `3101d07`, Windows, rustc 1.92.0, IR codegen（默认路线）

## 复现步骤

```lz
enum Color: Red | Green | Blue

def main() =
    let c = Color.Red
    let desc = match c:
        Color.Red: "warm"
        Color.Green: "nature"
        Color.Blue: "calm"
    print(desc)
```

编译: `lang-zone test.lz` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
match c {
    Color.Red => {          // ❌ E0164: expected a pattern, found an expression
        return "warm".to_string();
    }
}
```

rustc 报错: `error: expected a pattern, found an expression`
- `Color.Red` 不是合法的 Rust 模式（它是路径表达式）

## 预期结果

match 枚举臂应使用模式匹配语法：
```rust
match c {
    Color::Red => { "warm".to_string() }
}
```

或者转换为 if-else 链：
```rust
if c == Color::Red { "warm".to_string() }
else if c == Color::Green { "nature".to_string() }
else { "calm".to_string() }
```

## 技术根因

IR builder (`src/ir/builder.rs`) 在转换 `AstExpr::Match` 时将模式中的枚举变体路径直接作为表达式嵌入条件判断，没有转换为 Rust 模式语法或 `==` 比较。

## 影响范围

- `DEMO/06_control_flow/match.lz`
- `DEMO/06_control_flow/match_more.lz`
- `DEMO/07_data_structures/enum.lz`
- `DEMO/07_data_structures/enum_more.lz`
- `DEMO/03_variables/mutable_let.lz`
- `DEMO/05_expressions/if_match_expr.lz`
- `DEMO/99_spec/extractor_unapply.lz`
- 以及其他使用 match + enum 的文件

## 关联

- `issue/ir-codegen-match-var-scope.md` — 同一 match 代码生成链路上的另一个缺陷
