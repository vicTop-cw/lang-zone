# Bug: 十六进制溢出被误报为"无效"而非"溢出"

**状态**: Open  
**发现日期**: 2026-07-31 14:29  
**严重等级**: 🟡 P2 — 错误消息质量  
**发现方式**: 边界值测试  
**测试工程师**: 自动化边界测试

---

## 描述

lexer 将超过 i64 范围的十六进制字面量误报为"无效的十六进制数字"，但实际是"值溢出"，十六进制格式本身是合法的。

**复现**:
```lz
def main() = print(0xFFFFFFFFFFFFFFFF)
```

**实际结果**: `Parse error: 无效的十六进制数字: 0xFFFFFFFFFFFFFFFF`

**对比验证**:
| 字面量 | 结果 |
|--------|:--:|
| `0x7FFFFFFFFFFFFFFF` (15位hex, i64::MAX) | ✅ 通过 |
| `0x8000000000000000` (15位hex, i64::MIN正值) | ❌ "无效" |
| `0xFFFFFFFFFFFFFFFF` (16位hex, u64::MAX) | ❌ "无效" |
| `0xG` (非法hex字符) | ✅ "无效的十六进制数字" |

---

## 技术根因

`src/lexer/lexer.rs` 的 hex 解析：
```rust
let val = i64::from_str_radix(hex_str, 16);
// val = Err → Token::LexError("无效的十六进制数字: 0x{hex_str}")
```

`i64::from_str_radix` 对"格式正确但值溢出"和"格式错误"返回相同的 `ParseIntError`。当前代码未区分这两种情况。

---

## 影响范围

- 用户看到"无效"错误后可能反复检查 hex 字符格式
- 实际上格式正确，只是值超出 i64 范围
- 浪费调试时间

---

## 修复建议

区分 `ParseIntError` 的两种原因：

```rust
let hex_s = &self.source[start..self.pos];
match i64::from_str_radix(hex_s, 16) {
    Ok(val) => Token::IntLit(val),
    Err(e) => {
        // 检查是否只是值溢出（而非格式错误）
        // 方法：用 u64 解析，如果 u64 也失败则确实格式错误
        if u64::from_str_radix(hex_s, 16).is_ok() {
            Token::LexError(format!(
                "十六进制值溢出 i64 范围: 0x{}（最大值: 0x7FFFFFFFFFFFFFFF）", hex_s))
        } else {
            Token::LexError(format!("无效的十六进制数字: 0x{}", hex_s))
        }
    }
}
```

同样的逻辑也应应用于十进制和八进制溢出检测。
