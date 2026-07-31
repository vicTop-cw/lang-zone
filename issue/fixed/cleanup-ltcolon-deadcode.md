# Open: 清理 `LtColon` 死代码（移除 `<:` 子类型运算符后）

**状态**: Open
**优先级**: P3（非紧急，死代码清理）
**指派**: 工程 Agent（文档侧不改动 `src/`）
**关联决策**: [decision-drop-subtype-operators.md](decision-drop-subtype-operators.md)

## 背景

语言已正式**移除子类型运算符 `<:` 与 `>:`**（2026-07-31 拍板，「全面改掉」）。
语法仅保留：

- `:` —— 约束 / bound
- `==` —— 类型等同

实测结论（见决策文档）：

- `>:` 从未有 token，解析 `where T >: X` 直接 `Parse error: Expected Colon, got Gt`。
- `<:` 能被词法分析器识别为 `LtColon` token，但约束求解器只实现 `Eq`，**子类型约束留待 P1** —— 即能过解析、不强制，等于无效死语法。

因此 `LtColon` 现已是**死代码**，且其存在会误导实现者以为 `<:` 仍可用。本 issue 把它从 `src/` 彻底清除，并取消 `constraint.rs` 里「子类型约束留待 P1」的计划。

## 待清理清单（务必逐项）

### 1. `src/lexer/token.rs`（两处：枚举定义 + 词法分支）
- **L60**：删除枚举项 `LtColon,       // <: 约束符号`。
  ```rust
  // 删除这一行：
  // LtColon,       // <: 约束符号
  ```
- **L609**：删除 `<` 词法分支里特判 `:` 的小块（与 `lexer.rs:481` 同源）：
  ```rust
  if self.peek() == Some(':') {
      self.advance();
      tokens.push(Token::LtColon);   // ← 删除整个 if 块（约 L607–609）
  } else if self.peek() == Some('<') {
  ```
  删除后该 `<` 分支直接走 `<=` / `<<` 逻辑，不再特判 `:`。

### 2. `src/lexer/lexer.rs`（一处：词法分支）
- **L481**：与 `token.rs:609` 结构相同的 `<` 分支，删除特判 `:` 的小块：
  ```rust
  if self.peek() == Some(':') {
      self.advance();
      tokens.push(Token::LtColon);   // ← 删除整个 if 块（约 L479–481）
  } else if self.peek() == Some('<') {
  ```

> 删除后，`<:` 会被词法化为 `Lt` `Colon` 两个独立 token，下游解析时自然报错——符合「符号已移除」的预期行为。

### 3. `src/parser/parser.rs`
- **L712–725** `parse_where_clause`：当前同时接受 `<:` 与 `:` 作为约束分隔符：
  ```rust
  // 支持 <: 或 : 作为约束分隔符
  if self.check(&Token::LtColon) {
      self.advance();
  } else {
      self.expect(Token::Colon)?;
  }
  ```
  改为**只允许 `:`**：
  ```rust
  self.expect(Token::Colon)?;
  ```
  `where T: X` 行为与清理前完全一致，不受影响。

### 4. `src/macros/interp.rs`
- **L527**：从 `"operator"` 匹配臂中删除 `Token::LtColon |`（否则 token 枚举删掉后此处编译报错）：
  ```rust
  | Token::Le | Token::Ge | Token::LtColon | Token::PlusEq | Token::MinusEq
  //                    ↑ 删除 Token::LtColon |
  ```

### 5. `src/hints/constraint.rs`（取消 P1 计划）
- **L3–4** 模块注释「子类型约束（Subtype）留待 P1 … `where T <: Any` 等场景」：
  改为说明「LZ 无子类型运算符，约束仅 `Eq`；`<:` 已移除，P1 子类型计划取消」。
- `enum Constraint` 当前只有 `Eq` 变体，**无需改动**。

## 验收标准

1. `grep -rn "LtColon" src/` 返回 **0** 处。
2. `cargo build` 通过，**0 errors / 0 warnings**（注意 `LtColon` 删除后 `interp.rs` 必须同步改，否则编译失败）。
3. `cargo test --lib` 全绿（预期 430/430，与清理前一致）。
4. `cargo test --test compile_demos` 通过（绿 demo 套件 77/77，99_spec / 99_errors 被跳过）。
5. `where T: Clone` 仍正常编译；`where T <: Clone` 现在**应该**报解析错误（符号已移除）。
6. `<<`（Shl）、`<=`（Le）运算符不受影响（词法分支独立）。

## 风险 / 注意

- **不要**删除 `Lt` / `Gt` / `Le` / `Ge` / `Shl` 等真实运算符 token——只删 `LtColon`。
- 改动是纯删除 + 解析器收窄，**不涉及 IR / codegen / 语义**，回归范围小。
- 完成后把本 issue 移入 `issue/README.md` 的 Fixed 段，并记一行修复摘要。
