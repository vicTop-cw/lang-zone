# Bug: `||` 无参数闭包与逻辑或运算符冲突

**状态**: ✅ 已解决（见决策文件 — 2026-07-31）  
**解决方式**: `| |`（中间空格）作为无参闭包定界符，`||`（紧贴）保持为逻辑或。详见 [decision-closure-fat-arrow.md](decision-closure-fat-arrow.md)  
**发现日期**: 2026-07-31 14:29  
**严重等级**: 🟡 P2 — 功能可用但有语法歧义  
**发现方式**: 闭包边界测试  
**测试工程师**: 自动化边界测试

---

## 描述

LZ 编译器将 `||` 固定解析为 `PipePipe`（逻辑或 `||`），导致无法书写无参数闭包 `|| expr`。

**复现**:
```lz
def main() =
    let f = || 42
    print(f())
```

**实际结果**: `Parse error: Unexpected token in expression: PipePipe`

**变通方案**:
```lz
let f = |_| 42    # 带一个 dummy 参数
```

---

## 技术根因

`src/lexer/lexer.rs:L656` 在 `|` 后紧跟 `|` 时统一产生 `PipePipe` token：
```rust
'|' => {
    if self.peek() == Some('|') {  // || → 逻辑或
        self.advance();
        Token::PipePipe
    } else if self.peek() == Some('>') {  // |> → 管道
        ...
    } else {
        Token::Pipe_
    }
}
```

Lexer 无法区分 `||`(逻辑或) 与 `||`(无参数闭包) 的上下文。parser 层看到 `PipePipe` 只能按二元运算符处理。

---

## 影响范围

- 无参数闭包语法不可用
- `|_| expr` 变通可工作（已验证 ✅）
- 不影响单参数/多参数闭包（`|x|`, `|a,b|` 正常）

---

## 修复建议

### 方案 A: 引入替代语法

为无参数闭包引入独立语法标记：
```lz
let f = \-> 42           # 类似 Rust 的 || 但无歧义
let f = fn() -> 42       # 简短的匿名函数语法
```

### 方案 B: Parser 层上下文分析

在 parser 的表达式解析中，根据上下文判断 `||` 的语义：
- 后面紧跟表达式且前面是赋值 → 无参数闭包
- 前后都有表达式 → 逻辑或

⚠️ 方案 B 增加 parser 复杂度，且可能有边缘歧义（如 `a = b || c || d`）

### 方案 C: 文档化限制

当前变通方案 `|_| expr` 已足够，在文档中明确说明 `||` 表示逻辑或，无参数闭包请用 `|_| expr`。
