# Lang-Zong 宏 API 工具集 v1

> 编译期内置函数集 — 在 `macro ... -> Tokens =` 体内调用，全部操作 `Tokens` 类型。
> `!` 后缀命名约定：与 `@name!` 宏调用一致，标识为编译期操作。

---

## 一、API 速查表

| API | 签名 | 返回 | 说明 |
|-----|------|------|------|
| `quote` | `(tokens: Tokens) -> Tokens` | `Tokens` | 原样捕获（即 ` ``` ` 的函数等价形式） |
| `token_stream` | `(tokens: Tokens) -> Tokens` | `Tokens` | 构造带分组层级的 TokenStream |
| `merge_tokens` | `(...streams: Tokens) -> Tokens` | `Tokens` | 可变参数拼接 |
| `remove_tokens` | `(src: Tokens, pattern: Tokens) -> Tokens` | `Tokens` | 移除匹配的 token 模式 |
| `replace_tokens` | `(src: Tokens, rules: Tokens) -> Tokens` | `Tokens` | 查找替换（多组规则） |
| `filter_tokens` | `(src: Tokens, kind: str) -> Tokens` | `Tokens` | 按类型过滤 |
| `token_count` | `(stream: Tokens) -> int` | `int` | 顶层 token 数（= `len` 别名） |
| `is_empty_tokens` | `(stream: Tokens) -> bool` | `bool` | 判空（= `is_empty` 别名） |

---

## 二、逐一设计

### 2.1 `quote(tokens)` — Token 原样捕获

**本质**：` ``` ` 反引号块的函数等价形式。当需要通过变量间接传递 tokens 时使用。

**签名**：
```
quote(input: Tokens) -> Tokens
```

**示例**：
```lz
macro identity(input: Tokens) -> Tokens =
    quote(input)        # 完全等同于直接返回 input

macro wrap_expr(input: Tokens) -> Tokens =
    let inner = quote(input)
    ```
        ($(inner))
    ```
```

**与 ` ``` ` 的关系**：

| 场景 | 推荐 |
|------|------|
| 静态 token 字面量 | ` ```code``` ` |
| 变量引用 / 传参 | `quote(var)` |

---

### 2.2 `token_stream(tokens)` — 层级化 TokenStream 构造

**签名**：
```
token_stream(input: Tokens) -> Tokens
```

**核心概念**：普通的 `Tokens` 是扁平的 token 列表。`token_stream` 将嵌套的分隔符组 `()`, `[]`, `{}` 组织为 **树形结构**，每个组成为一个 `TokenGroup` 子节点。

**TokenGroup 数据结构**：
```
TokenGroup:
  - Atom(Token)                    单个 token
  - Group(Delimiter, Vec<TokenGroup>)  分隔符组

Delimiter:
  - Paren   ( )
  - Bracket [ ]
  - Brace   { }
```

**示例**：
```lz
// 宏体中使用
let flat: Tokens = ```
    def foo(x: int) -> int =
        x + 1
```
// flat 是扁平 token 序列

let tree: Tokens = token_stream(flat)
// tree 是树形结构：
//   Group(Paren, [
//     Atom(Ident("x")), Atom(Colon), Atom(Ident("int"))
//   ])
//   Atom(Arrow)
//   Group(Brace, [
//     Atom(Ident("x")), Atom(Plus), Atom(IntLit(1))
//   ])

// 可以访问特定分组
let params = tree.find_group(Paren)  # 提取参数列表 (x: int)
let body = tree.find_group(Brace)    # 提取函数体 { x + 1 }
```

**用途**：配合 `replace_tokens` 做结构化替换——只替换特定分组内的内容。

---

### 2.3 `merge_tokens(streams...)` — 可变参数合并

**签名**：
```
merge_tokens(a: Tokens, b: Tokens, ...rest: Tokens) -> Tokens
```

**行为**：按参数顺序拼接，保持原始 token。等价于 `a + b + c + ...` 但支持可变参数。

**示例**：
```lz
macro build_getter(input: Tokens) -> Tokens =
    let prefix = ```
        def get_
    ```
    let suffix = ```
        (self: Self) -> int =
            self.field
    ```
    merge_tokens(prefix, input, suffix)

// @build_getter!(name) 
// → def get_name(self: Self) -> int = self.field
```

**变长参数处理**：
```lz
macro concat_all(input: Tokens) -> Tokens =
    // input 包含多个子 Tokens（如逗号分隔的参数列表）
    // 将它们全部合并
    if is_empty(input):
        ```
        
        ```
    else
        let head = first(input)
        let tail = rest(input)
        merge_tokens(head, concat_all(tail))
```

---

### 2.4 `remove_tokens(src, pattern)` — 模式匹配移除

**签名**：
```
remove_tokens(source: Tokens, pattern: Tokens) -> Tokens
```

**Token 模式 DSL**：

| 模式元素 | 语法 | 匹配规则 |
|---------|------|---------|
| 通配符 | `_` | 匹配任意单个 token |
| 精确匹配 | `Ident("name")` / `+` / `42` | 匹配同名 token |
| 类型匹配 | `:ident` | 匹配任意 Ident（捕获名称） |
| 类型匹配 | `:int` | 匹配任意 IntLit |
| 类型匹配 | `:str` | 匹配任意 StrLit |
| 捕获绑定 | `$name` | 捕获匹配的 token，绑定到变量 `name` |
| 重复 | `...` | 匹配前一个模式的 0 次或多次重复 |

> **语法规则**：模式本身也用 Token 表示——`_` 就是 `Token::Underscore`，`$name` 是 `Token::Dollar + Token::Ident("name")`，`:ident` 是 `Token::Colon + Token::Ident("ident")`。
> 模式引擎解析这些特殊 token 为 `TokenPattern` 结构进行匹配。

**示例**：
```lz
macro strip_debug(input: Tokens) -> Tokens =
    // 移除所有 print(...) 调用
    let pattern = ```
        print(_)
    ```
    remove_tokens(input, pattern)

// @strip_debug!(x = 1; print(x); return x)  
// → x = 1; return x

macro remove_commas(input: Tokens) -> Tokens =
    // 移除所有逗号
    let pattern = ```
        ,
    ```
    remove_tokens(input, pattern)
```

**通配符示例**：
```lz
macro remove_trailing_comma(input: Tokens) -> Tokens =
    // 移除末尾的 ", )" 中的逗号
    let pattern = ```
        , )
    ```
    remove_tokens(input, pattern)

// 但要注意: 括号内如果有逗号分隔的内容，这个模式会移除所有 ", )" 出现
// 更精确的方式是配合 token_stream 做结构化匹配
```

---

### 2.5 `replace_tokens(src, rules)` — 查找替换

**签名**：
```
replace_tokens(source: Tokens, rules: Tokens) -> Tokens
```

**规则语法**：`rules` 是一个包含 `from => to` 对的 token 序列：
```
from_pattern_1 => to_tokens_1, from_pattern_2 => to_tokens_2, ...
```

**示例**：
```lz
macro rename_ident(input: Tokens, old_name: Tokens, new_name: Tokens) -> Tokens =
    let rules = f```
        $(old_name) => $(new_name)
    ```
    replace_tokens(input, rules)

// @rename_ident!(def foo() = x + old_var, old_var, new_var)
// → def foo() = x + new_var
```

**多规则示例**：
```lz
macro auto_ref(input: Tokens) -> Tokens =
    let rules = ```
        self.x => (*self).x,
        self.y => (*self).y,
        self => (*self)
    ```
    replace_tokens(input, rules)
```

**带捕获的重写**：
```lz
macro swap_args(input: Tokens) -> Tokens =
    // 交换函数参数: foo(a, b) → foo(b, a)
    let rules = ```
        $fn($a, $b) => $fn($b, $a)
    ```
    replace_tokens(input, rules)
```

**规则优先级**：规则按定义顺序匹配，先匹配先生效（类似 Rust `macro_rules!`）。

---

### 2.6 `filter_tokens(src, kind)` — 按类型过滤

**签名**：
```
filter_tokens(source: Tokens, kind: str) -> Tokens
```

**支持的 kind 值**：

| kind | 保留的 token |
|------|-------------|
| `"ident"` | 所有 `Ident(...)` |
| `"literal"` | `IntLit` / `FloatLit` / `StrLit` / `True` / `False` |
| `"keyword"` | `Def` / `If` / `Return` 等关键字 |
| `"operator"` | `+` / `-` / `*` / `=` 等运算符 |
| `"delimiter"` | `(` / `)` / `[` / `]` / `{` / `}` |

**示例**：
```lz
macro extract_idents(input: Tokens) -> Tokens =
    filter_tokens(input, "ident")

// @extract_idents!(def foo(x: int) -> int = x + 1)
// → foo x int int x
```

```lz
macro count_ops(input: Tokens) -> Tokens =
    let ops = filter_tokens(input, "operator")
    ```
        $(token_count(ops))
    ```
// 返回运算符数量的 token 表示
```

---

### 2.7 `token_count(stream)` — 编译期计数

**签名**：
```
token_count(stream: Tokens) -> int
```

**行为**：返回 `stream` 中的顶层 token 数量（与 `len(stream)` 等价）。

**示例**：
```lz
macro arity_check(input: Tokens) -> Tokens =
    let params = extract_params(input)
    if token_count(params) > 5:
        ```
            // error: too many parameters
        ```
    else
        input
```

---

### 2.8 `is_empty_tokens(stream)` — 编译期判空

**签名**：
```
is_empty_tokens(stream: Tokens) -> bool
```

**行为**：返回 stream 是否为空（与 `is_empty(stream)` 等价）。

---

## 三、组合使用

### 3.1 链式数据流

```lz
macro pipeline(input: Tokens) -> Tokens =
    let stripped = remove_tokens(input, debug_pattern)
    let renamed = replace_tokens(stripped, rename_rules)
    let merged = merge_tokens(prefix, renamed, suffix)
    quote(merged)
```

### 3.2 模式：收集所有标识符并去重

```lz
macro unique_idents(input: Tokens) -> Tokens =
    let idents = filter_tokens(input, "ident")
    // 如果 token_count 较大，跳过处理
    if token_count(idents) < 2:
        idents
    else
        deduplicate(idents)
```

### 3.3 模式：条件编译

```lz
macro debug_only(input: Tokens) -> Tokens =
    if get_context("debug") == "true":
        input
    else
        replace_tokens(input, ```
            debug_log(_) =>
        ```)
    // debug_log(...) 被替换为空 Tokens（移除）
```

### 3.4 模式：结构化替换

```lz
macro add_field(struct_decl: Tokens, field_def: Tokens) -> Tokens =
    let tree = token_stream(struct_decl)
    let last_brace = tree.find_last_group(Brace)
    let new_body = merge_tokens(last_brace, field_def)
    replace_tokens(struct_decl, ```
        $(last_brace) => $(new_body)
    ```)
```

---

## 四、Token 模式匹配引擎

### 4.1 模式语法（TokenPattern）

用特殊 Token 表达模式：

| 语法 | Token 表示 | 匹配 |
|------|-----------|------|
| `_` | `Token::Underscore` | 任意 1 个 token |
| `:ident` | `Token::Colon, Token::Ident("ident")` | 任意 Ident token |
| `:int` | `Token::Colon, Token::Ident("int")` | 任意 IntLit token |
| `:str` | `Token::Colon, Token::Ident("str")` | 任意 StrLit token |
| `:bool` | `Token::Colon, Token::Ident("bool")` | True 或 False |
| `$name` | `Token::Dollar, Token::Ident("name")` | 捕获到变量 name |
| `...` | `Token::DotDotDot` | 前一个模式的 0+ 次重复 |
| `X`（字面） | 具体 Token | 精确匹配 |

### 4.2 模式匹配算法

```
match_pattern(tokens: &[Token], pattern: &TokenPattern) -> Option<(usize, Captures)>

TokenPattern::Wildcard    → 消耗 1 个 token，不捕获
TokenPattern::TypeIdent   → 消耗 1 个 Ident token，不捕获
TokenPattern::Exact(t)    → 消耗 1 个 token（必须 == t）
TokenPattern::Capture(n)  → 消耗 1 个 token，捕获到 n
TokenPattern::Repeat(p)   → 消耗 0+ 个 token（贪婪匹配 p）
TokenPattern::Seq(ps)     → 按序匹配 ps 中的所有子模式
```

### 4.3 替换模板语法

在 `=>` 右侧，`$name` 引用左侧捕获的值：

```lz
// 规则: $a + $b => $b + $a
// 输入: x + y
// 捕获: a=[x], b=[y]
// 输出: y + x
```

---

## 五、实现扩展

### 5.1 新增至 `MacroExpr`

```rust
// 新增表达式变体
pub enum MacroExpr {
    // ... 现有 ...
    VarArgs(Vec<MacroExpr>),     // merge_tokens 的可变参数
    DotDotDot,                    // ... 重复模式
}
```

### 5.2 新增 Token（如需）

```rust
// lexer/token.rs
DotDotDot,    // ... 模式重复标记
```

### 5.3 扩展 `eval_builtin`

在 `is_builtin` 和白名单中添加：
```rust
"quote" | "token_stream" | "merge_tokens" | "remove_tokens"
| "replace_tokens" | "filter_tokens" | "token_count" | "is_empty_tokens"
```

### 5.4 新增 `TokenPattern` 引擎

```rust
// src/macros/pattern.rs（新文件）
pub enum TokenPattern {
    Wildcard,                    // _
    Exact(Token),                // 精确 token
    TypeIdent,                   // :ident
    TypeInt,                     // :int
    TypeStr,                     // :str
    TypeBool,                    // :bool
    Capture(String),             // $name
    Repeat(Box<TokenPattern>),   // ... 重复
    Seq(Vec<TokenPattern>),      // 序列
}

pub struct Captures {
    groups: HashMap<String, Vec<Token>>,
}

impl TokenPattern {
    /// 解析模式 tokens 为 TokenPattern
    pub fn parse(tokens: &[Token]) -> Result<TokenPattern, String>;
    
    /// 从 tokens[start..] 开始匹配，返回消耗的 token 数和捕获组
    pub fn match_from(&self, tokens: &[Token], start: usize) -> Option<(usize, Captures)>;
}
```

---

## 六、与现有 API 对照

| 新 API | 现有等价 | 差异 |
|--------|---------|------|
| `quote(x)` | `x` 自身或 ` ``` ` 块 | 明确语义、可传变量 |
| `token_stream(x)` | 无 | 全新：层级化 Token 树 |
| `merge_tokens(a,b,c)` | `a + b + c` | 可变参数，更简洁 |
| `remove_tokens(s, p)` | 无 | 全新：模式匹配移除 |
| `replace_tokens(s, r)` | 无 | 全新：模式匹配替换 |
| `filter_tokens(s, k)` | 无 | 全新：按类型过滤 |
| `token_count(s)` | `len(s)` | 别名，语义更明确 |
| `is_empty_tokens(s)` | `is_empty(s)` | 别名，语义更明确 |

---

## 七、错误处理约定

所有 API 在编译期执行，错误直接作为编译错误报告：

```
error: macro 'my_macro' expansion failed
  → remove_tokens: unmatched pattern at position 5
  → expected '_,' but found 'Identifier("x")'
```

---

## 八、实现优先级

| 优先级 | API | 理由 |
|--------|-----|------|
| P0 | `quote`, `merge_tokens`, `token_count`, `is_empty_tokens` | 基础工具，无模式匹配复杂度 |
| P1 | `filter_tokens` | 简单分类，实现成本低 |
| P2 | `remove_tokens` | 需要 TokenPattern 引擎 |
| P2 | `replace_tokens` | 依赖 remove 的模式引擎 + 替换模板 |
| P3 | `token_stream` | 需要 Token 树形结构，改动较大 |
