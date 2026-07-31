# 🔴 P1: IR codegen `float(str)` 生成无效 Rust 代码 `String as f64`

**Bug 标题**: `float(s)` 当 `s: String` 时生成 `(s as f64)` 而非 `s.parse::<f64>().unwrap()`

**严重等级**: 🔴 P1 — 类型转换功能受损
**发现日期**: 2026-07-31
**环境**: commit `ff8c61a` (含未提交 codegen 改动), Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
def test_str_to_float(s: str) -> f64 =
    float(s)

def main() =
    let x = test_str_to_float("2.718")
    print(x)
```

编译: `lang-zone test.lz` → 生成 .rs → `rustc test.rs`

## 实际结果

```rust
pub fn test_str_to_float(s: String) -> f64 {
    return (s as f64);  // ❌ E0605: non-primitive cast: `String` as `f64`
}
```

rustc 错误: `E0605: non-primitive cast: String as f64`

对比，`int(s)` 当 `s: String` 时能正确生成 `(s).parse::<i64>().unwrap()`，说明 codegen 已为 `int` 区分了 String → parse vs Number → as。`float` 缺少同样的分支。

## 预期结果

```rust
pub fn test_str_to_float(s: String) -> f64 {
    return s.parse::<f64>().unwrap();  // ✅
}
```

## 根因

`src/ir/codegen.rs` 中 `int/str/f64/float` 类型转换处理（未提交改动）：
- `int` 分支: 正确区分了 `IrType::Str` → `.parse()` vs 其他 → `as i64`
- `str` 分支: 始终用 `format!("{}", x)` ✅
- `f64`/`float` 分支: 固定使用 `as f64`，**未检测参数是否为 String**

## 影响范围

- 所有 `float(str_value)` 调用 → 生成的 Rust 代码不编译
- 测试文件: `DEMO/99_spec/ir-edge-type-boundary.lz`

## 修复建议

```rust
"f64" | "float" => {
    if args.len() == 1 {
        let arg_ty = &args[0].ty;
        if matches!(arg_ty, IrType::Str) {
            format!("({}).parse::<f64>().unwrap()", args_s[0])
        } else {
            format!("({} as f64)", args_s[0])
        }
    } else {
        format!("({} as f64)", args_s[0])
    }
}
```
