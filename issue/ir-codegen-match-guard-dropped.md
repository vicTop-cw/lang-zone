# 🔴 P1: IR codegen match guard 条件被静默丢弃

**Bug 标题**: `case n if condition` 生成 `n =>` 通配模式，guard 条件完全丢失

**严重等级**: 🔴 P1 — match guard 功能完全不可用，且无任何报错
**发现日期**: 2026-07-31
**环境**: commit `ff8c61a` (含未提交 codegen 改动), Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
def classify(x: int) -> str =
    match x:
        case n if n < 0 => "negative"
        case 0 => "zero"
        case n if n < 10 => "single_digit"
        case n if n < 100 => "double_digit"
        case n => "large"
```

编译: `lang-zone classify.lz` → 生成 .rs → `rustc classify.rs`

## 实际结果

```rust
pub fn classify(x: i64) -> String {
    match x {
        n => {                          // ❌ guard `if n < 0` 丢失
            "negative".to_string()
        }
        0 => {                          // ⚠️ 不可达(unreachable)
            "zero".to_string()
        }
        n => {                          // ❌ guard 丢失
            "single_digit".to_string()
        }
        n => {                          // ❌ guard 丢失
            "double_digit".to_string()
        }
        n => {                          // ❌ guard 丢失
            "large".to_string()
        }
    }
}
```

rustc 输出: 4 个 `unreachable_pattern` 警告。所有 `n =>` 臂互相覆盖，仅有第一臂可达。

## 预期结果

```rust
pub fn classify(x: i64) -> String {
    match x {
        n if n < 0 => {                 // ✅
            "negative".to_string()
        }
        0 => {
            "zero".to_string()
        }
        n if n < 10 => {                // ✅
            "single_digit".to_string()
        }
        n if n < 100 => {               // ✅
            "double_digit".to_string()
        }
        _ => {
            "large".to_string()
        }
    }
}
```

## 根因

IR codegen 在处理 `MatchArm` 时，模式变量绑定了但 guard 条件（`IrExpr`）未生成。IR 中 guard 被表示为 `Pattern::Guarded { pattern, condition }`，codegen 可能未消费 `condition` 字段。

对比：**for 循环 guard 正常**（`for item in items if item > 0` 正确生成 `.filter(|&item| item > 0)`），说明 guard 处理逻辑不是全面缺失，仅是 match arm 分支遗漏。

## 影响范围

- 所有带 `case pattern if condition =>` 的 match 语句
- 测试文件: `DEMO/06_control_flow/match_more.lz` (可能受影响)
- 测试文件: `DEMO/99_spec/ir-edge-guard-combo.lz`

## 修复建议

在 `gen_pattern` 或 match arm 生成逻辑中，检查 pattern 是否有 guard 条件：
```rust
if let IrPattern::Guarded { pattern, condition } = &arm.pattern {
    let pat_s = self.gen_pattern(pattern);
    let cond_s = self.gen_expr(condition);
    format!("{} if {}", pat_s, cond_s)
}
```
