# Token → Scope → IR 设计

## 一、Token（词法）

### Token 类型

```rust
enum Token {
    // ── 关键字 ──
    Def, Mut, Ref, Const, Return, Yield,
    If, Else, Elif, Match, Case, Guard,
    For, In, While, Loop, Break, Continue,
    Struct, Enum, Trait, Impl, Where, Type,
    Try, Catch, Finally, With, As,
    True, False, None, Some, Ok, Err,

    // ── 类型关键字（软关键字，可作为标识符）──
    Int, F64, Str, Bool, List, Dict, Set, Option, Result,

    // ── 字面量 ──
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    
    // ── 标识符 ──
    Ident(String),          // 普通标识符
    MagicMethod(String),    // __xxx__ 魔法方法（特殊处理）
    
    // ── 符号 ──
    Eq,         // =
    Colon,      // :
    Comma,      // ,
    Dot,        // .
    DotDot,     // ..
    DotDotEq,   // ..=
    Arrow,      // ->
    Pipe,       // |>
    LT,         // <
    GT,         // >
    LAngle,     // <:
    Plus, Minus, Star, Slash,
    LParen, RParen, LBrack, RBrack, LBrace, RBrace,
    Amp,        // &
    At,         // @
    Question,   // ?
    QuestionQuestion, // ??
    Exclamation, // !
    Underscore, // _
    Newline,    // \\n
    Indent,     // 缩进（虚拟 token）
    Dedent,     // 退缩进（虚拟 token）
    Eof,
}
```

### 关键点

1. **缩进处理** — 词法分析阶段生成 `Indent`/`Dedent` 虚拟 token（仿 Python）
2. **魔法方法识别** — `__xxx__` 模式单独标记为 `MagicMethod`
3. **`<:` 约束符号** — 区别于 `<=`，需要一个字符前瞻
4. **`..` vs `..=` ** — 需要两个字符前瞻

---

## 二、Scope（作用域）

### 作用域层级

```
Module 作用域
  ├── 全局变量 (const, 静态变量)
  ├── struct/enum/trait 定义
  └── 函数作用域
        ├── 参数
        ├── 局部变量
        ├── 引用/借用标记
        └── 嵌套块作用域 (if/for/while/loop/match)
```

### 关键数据结构

```rust
struct Scope {
    kind: ScopeKind,
    parent: Option<usize>,    // 父作用域索引
    symbols: HashMap<String, Symbol>,
    depth: usize,              // 缩进深度
}

enum ScopeKind {
    Module,      // 文件级
    Function,    // def 函数
    Block,       // if/for/while/loop/match 块
    Struct,      // struct 体
    Trait,       // trait 体
}

struct Symbol {
    name: String,
    kind: SymbolKind,
    mutable: bool,
    ty: Option<Type>,
    scope_id: usize,
    def_line: usize,
}

enum SymbolKind {
    Variable,
    Parameter,
    Function,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Module,
}
```

### 作用域规则

1. **内层遮蔽外层** — 允许同名变量
2. **Move 后失效** — `b = a` 后 `a` 在作用域内标记为 moved
3. **借用检查** — 不可变引用存在时，不能有可变引用
4. **生命周期推断** — 根据作用域嵌套自动推导

---

## 三、IR（中间表示）

### 方案 A：直接 AST → Rust（简单）

```
.lz → Tokenize → Parse → AST → CodeGen → .rs
                                    ↑
                            这里直接生成 Rust 源码
```

**优点：** 实现简单，2 周可跑通  
**缺点：** 没有优化空间，全靠 rustc

---

### 方案 B：AST → HIR → Rust（推荐）

```
.lz → Tokenize → Parse → AST → 语义分析 → HIR → CodeGen → .rs
```

HIR（高层 IR）做：
1. **类型推导** — 推断 `x = 1` 的 `x: int`
2. **魔法方法展开** — `__add__` → `impl Add`
3. **guard 去糖** — `guard cond else: x` → `if !cond { return x }`（else 后不写 return，编译器自动插入）
4. **可变参数展开** — `rest: List<T>` → 收集逻辑
5. **闭包捕获分析** — `|x| x + y` 中 `y` 是借用还是移动

HIR 仍是树结构，不做 SSA。

---

### 方案 C：AST → HIR → MIR → Rust（完整）

```
AST → 语义分析 → HIR → 降级 → MIR → 优化 → CodeGen → .rs
```

MIR（中层 IR）做：
1. **借用检查** — 仿 Rust borrowck
2. **所有权分析** — move/copy 语义
3. **死代码消除**
4. **常量折叠**

**不推荐。** 第一阶段不需要——Rust 编译器自带这些。

---

## 四、建议路线

```
阶段 0 (2 周): 方案 A — 直接 AST → Rust
  目标: lzc hello.lz → hello.rs → rustc → Hello World
  语法: 变量/函数/if/for/struct/match
  不做的: 泛型/魔法方法/所有权分析

阶段 1 (2 周): + HIR
  加入: 语义分析、类型推导、魔法方法展开、guard 去糖

阶段 2 (4 周): 完整语法
  加入: 泛型、trait、所有权、闭包
```

**现在开始写阶段 0 吗？**
