# ir-codegen-try-catch-return-type

> 状态: Open | 严重等级: P1 | 发现日期: 2026-08-01 | 分类: IR Codegen

## Bug 标题 (N16)
try-catch 代码生成返回类型错误 + 函数查找失败

## 复现步骤
1. 编译 `DEMO/10_error_handling/panic_raise_try.lz` (IR路线)
2. 用 rustc 编译生成的 `panic_raise_try.rs`

## 预期结果
try-catch 块生成的 Rust 代码类型正确，函数引用可解析。

## 实际结果

14 个 E0308 错误 + 2 个 E0425 错误 + 6 warnings：

1. **try 块返回 `()` 而非预期类型**:
```rust
let result_from_try: i64 = {
    let __result = (|| { checked_divide(10, 2); Ok(()) })();
    if let Err(e) = __result { -1 }
    __result.ok();  // 返回 Option<i64>，非 i64
};
// E0308: expected i64, found ()
```

2. **catch 臂缺少 return**:
```rust
if let Err(e) = __result {
    -1       // E0308: expected (), found integer (if 表达式缺 else)
}
```

3. **函数未找到**:
```rust
let val = parse_int(s).try_into();   // E0425: parse_int not found
let content = read_file(path).try_into(); // E0425: read_file not found
```

## 影响
- `panic_raise_try.lz` 14 errors
- `try_more.lz` 若干 errors
- try-catch 核心控制流完全无法使用

## 环境信息
- 编译器版本: commit cc5ebad
- Rust 版本: rustc 1.96.0 stable-x86_64-pc-windows-msvc
- 复现率: 100%
