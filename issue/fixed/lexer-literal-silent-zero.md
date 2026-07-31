# 词法器：非法/溢出数字字面量静默变成 0

- **状态**：✅ 已修复（工作区，未提交 `git commit`）
- **优先级**：P0（编译器撒谎型 bug —— 最危险的一类）
- **根因文件**：`src/lexer/lexer.rs` → `read_number()`
- **发现方式**：`findings-2026-07-31.md` #2 报告 → 独立复现 + 源码定位
- **关联报告**：`findings-2026-07-31.md`（P0）、`verdict-2026-07-31.md`（#2 判为真 bug）

## 一、Bug 现象（用户视角的"编译器撒谎"）

非法或溢出的数字字面量被**静默**转成 `0` / `0.0`，全程零报错，开发者完全无感知：

| 输入 | 期望行为 | 实际行为（修复前 / committed 版本） |
|------|----------|--------------------------------------|
| `0xG` | 词法错误 | 解析为 `0`，`G` 被当成独立标识符/`0; G;` |
| `0xZZ` | 词法错误 | → `0` |
| `0b2` | 词法错误 | → `0` |
| `0o8` | 词法错误 | → `0` |
| `123e` | 词法错误（科学计数法缺指数） | → `0.0` |
| `123E` | 词法错误 | → `0.0` |
| `1.2.3` | 词法错误 | → `0.0` |
| `99999999999999999999999999999999` | 溢出错误 | → `0` |

**危害**：这类 bug 最阴险 —— 程序"正常运行"但数值全错，没有报错、没有警告，只有在生产环境算出错误结果时才会暴露。任何依赖该类字面量的逻辑都会静默损坏。

## 二、根因（源码定位）

`read_number()` 对进制前缀与浮点/整数解析统一使用 `unwrap_or(0)` / `unwrap_or(0.0)` 兜底，
把"解析失败"当成了"合法 0"：

```rust
// —— 修复前（committed 版本，buggy）——
let val = i64::from_str_radix(&num[2..].replace('_', ""), 16).unwrap_or(0);
return Token::IntLit(val);

// ... 八进制 / 二进制同上 .unwrap_or(0)

Token::FloatLit(num.parse().unwrap_or(0.0))   // 浮点兜底 0.0
Token::IntLit(num.parse().unwrap_or(0))        // 整数兜底 0
```

`from_str_radix` / `parse` 在字面量非法或溢出时返回 `Err`，但 `unwrap_or(0)` 把它吞掉并当成 `0`。

## 三、修复（工作区未提交）

改为 `match` + `Token::LexError`，解析失败即抛出明确词法错误：

```rust
// —— 修复后（working tree，uncommitted）——
match i64::from_str_radix(&num[2..].replace('_', ""), 16) {
    Ok(val) => return Token::IntLit(val),
    Err(_) => return Token::LexError(format!("无效的十六进制数字: {}", num)),
}
// 八进制 / 二进制同理 → "无效的八进制数字" / "无效的二进制数字"

if is_float {
    match num.parse::<f64>() {
        Ok(v) => Token::FloatLit(v),
        Err(_) => {
            if num.ends_with('e') || num.ends_with('E')
                || num.ends_with("e+") || num.ends_with("E+")
                || num.ends_with("e-") || num.ends_with("E-") {
                Token::LexError(format!("科学计数法缺少指数: {}", num))
            } else {
                Token::LexError(format!("无效的浮点数: {}", num))
            }
        }
    }
} else {
    match num.parse::<i64>() {
        Ok(v) => Token::IntLit(v),
        Err(_) => Token::LexError(format!("无效的整数（可能溢出）: {}", num)),
    }
}
```

`Token::LexError` 会在 parser 阶段令编译**直接失败并给出可读信息**，不再静默降级。

## 四、验证（实测）

修复后逐一复现上述全部 8 类非法字面量，编译器**均正确报错**（退出码非 0）：

```
'0xG'  -> Parse error: 无效的十六进制数字: 0x
'0xZZ' -> Parse error: 无效的十六进制数字: 0x
'0b2'  -> Parse error: 无效的二进制数字: 0b
'0o8'  -> Parse error: 无效的八进制数字: 0o
'123e' -> Parse error: 科学计数法缺少指数: 123e
'123E' -> Parse error: 科学计数法缺少指数: 123E
'1.2.3'-> Parse error: Expected field/method, got IntLit(3)
'9999…'-> Parse error: 无效的整数（可能溢出）: 9999…
```

测试套件（working tree 含修复）：

- `cargo test --lib` → **292 passed; 0 failed**
- `cargo test --test compile_demos` → **1 passed**（全量 DEMO 编译通过，确认无合法 demo 依赖旧静默行为）

## 五、结论与待办

- **#2 是真 bug**，审计报告判断正确；不是误报。
- 根因已定位到 `src/lexer/lexer.rs::read_number` 的 `unwrap_or(0)` 兜底。
- **修复已存在于工作区但未提交**。committed 版本（即当前 `main`/HEAD 上的 `lang-zone.exe` 若从干净 checkout 构建）仍带此 bug。
- **待办（交工程侧 / 项目 owner）**：将 `src/lexer/lexer.rs` 的修改 `git commit`，使修复进入版本历史。
  建议提交信息：`fix(lexer): 非法/溢出数字字面量不再静默降级为 0，改为抛出 LexError`。
- ⚠️ 若从干净 checkout 重新 `cargo build`，在提交前会再次得到带 bug 的二进制 —— 请尽快提交。
