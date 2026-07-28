# 词法与 AST 审计

> 审计日期: 2026-07-26 | 基于源码扫描 + test-suite/DEMO/boundary/e2e 测试覆盖匹配
> 覆盖标记: ✅ = test-suite 有测试, ⚠️ = DEMO 有示例但无自动化测试, ❌ = 无测试

---

## 一、关键字 (按 Keyword 映射 → Token 分类)

### 1.1 声明关键字

| Token | 关键字/字面量 | 分类 | 测试 |
|-------|-------------|------|:---:|
| Token::Def | `def` | 函数声明 | ✅ |
| Token::Struct | `struct` | 结构体声明 | ✅ |
| Token::Enum | `enum` | 枚举声明 | ⚠️ |
| Token::Trait | `trait` | 特征声明 | ⚠️ |
| Token::Impl | `impl` | 实现块 | ⚠️ |
| Token::Type | `type` | 类型别名 | ⚠️ |
| Token::Const | `const` | 常量声明 | ⚠️ |
| Token::Mut | `mut` | 可变修饰 | ⚠️ |
| Token::Ref | `ref` | 引用修饰 | ⚠️ |
| Token::Owned | `owned` / `owend` | 所有权声明 | ⚠️ |
| Token::Let | `let` | 不可变绑定 | ✅ |
| Token::Magic | `magic` | 魔法方法声明 | ⚠️ |

### 1.2 控制流

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::If | `if` | 条件分支 | ✅ |
| Token::Elif | `elif` | 否则如果 | ✅ |
| Token::Else | `else` | 否则分支 | ✅ |
| Token::Match | `match` | 模式匹配 | ✅ |
| Token::Case | `case` | 匹配分支 | ✅ |
| Token::Guard | `guard` | 守卫语句 | ⚠️ |
| Token::For | `for` | for 循环 | ⚠️ |
| Token::In | `in` | 成员判断/迭代 | ⚠️ |
| Token::While | `while` | while 循环 | ✅ |
| Token::Loop | `loop` | 无限循环 | ⚠️ |
| Token::Break | `break` | 跳出循环 | ⚠️ |
| Token::Continue | `continue` | 继续循环 | ⚠️ |
| Token::Return | `return` | 函数返回 | ⚠️ |
| Token::With | `with` | 上下文管理 | ⚠️ |
| Token::Pass | `pass` | 空操作占位 | ⚠️ |
| Token::Defer | `defer` | 延迟执行 | ⚠️ |

### 1.3 异常处理

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::Try | `try` | 尝试块 | ⚠️ |
| Token::Catch | `catch` | 捕获异常 | ⚠️ |
| Token::Finally | `finally` | 最终块 | ⚠️ |
| Token::Raise | `raise` | 抛出异常 | ⚠️ |
| Token::Raises | `raises` | 异常标注 | ⚠️ |
| Token::Panic | `panic` | 立即中止 | ⚠️ |

### 1.4 测试框架

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::Test | `test` | 测试用例 | ✅ |
| Token::Assert | `assert` | 断言 | ✅ |
| Token::Suite | `suite` | 测试套件 | ✅ |
| Token::Check | `check` | 软断言 | ⚠️ |

### 1.5 并发

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::Async | `async` | 异步函数 | ⚠️ |
| Token::Await | `await` | 等待异步 | ⚠️ |
| Token::Spawn | `spawn` | 生成任务 | ⚠️ |

### 1.6 迭代/生成器

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::Yield | `yield` | 生成器产出 | ⚠️ |
| Token::Sum | `sum` | 求和迭代 | ❌ |
| Token::Prod | `prod` | 乘积迭代 | ❌ |

### 1.7 导入

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::Import | `import` | 导入模块 | ⚠️ |
| Token::From | `from` | 从模块导入 | ⚠️ |
| Token::As | `as` | 别名导入 | ⚠️ |

### 1.8 类型/泛型

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::Where | `where` | 泛型约束 | ⚠️ |
| Token::Self_ | `Self` | 自身类型 | ⚠️ |

### 1.9 宏/编译期

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::Macro | `macro` | 宏定义 | ⚠️ |
| Token::Comptime | `comptime` | 编译期求值 | ⚠️ |
| Token::Template | `template` | 模板 | ⚠️ |

### 1.10 逻辑/字面量关键字

| Token | 关键字 | 分类 | 测试 |
|-------|--------|------|:---:|
| Token::And | `and` | 逻辑与 | ⚠️ |
| Token::Or | `or` | 逻辑或 | ✅ |
| Token::Not | `not` | 逻辑非 | ✅ |
| Token::Is | `is` | 类型判断 | ❌ |
| Token::True | `True` | 布尔真 | ✅ |
| Token::False | `False` | 布尔假 | ✅ |
| Token::None_ | `None` | 空值 | ⚠️ |
| Token::Some_ | `Some` | Option 包装 | ⚠️ |
| Token::Ok_ | `Ok` | Result 成功 | ❌ |
| Token::Err_ | `Err` | Result 失败 | ❌ |

---

## 二、字面量与标识符 Token

| Token | 语法 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::IntLit | `42` `0xff` `0o77` `0b1010` | 整数 / 进制字面量 | ✅ |
| Token::FloatLit | `3.14` `1e10` | 浮点数字面量 | ⚠️ |
| Token::StrLit | `"hello"` | 普通字符串 | ✅ |
| Token::FStrLit | `f"x = {x}"` | 字符串插值 (含三引号) | ✅ |
| Token::RawStrLit | `r"regex\d"` | 原始字符串 | ⚠️ |
| Token::TripleStrLit | `"""..."""` | 多行字符串 | ⚠️ |
| Token::Ident | `my_var` | 标识符 | ✅ |
| Token::MagicMethod | `__init__` | 魔法方法名 | ⚠️ |

---

## 三、运算符与标点 Token

### 3.1 赋值/比较

| Token | 符号 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::Eq | `=` | 赋值 | ✅ |
| Token::EqEq | `==` | 相等比较 | ✅ |
| Token::NotEq | `!=` | 不等比较 | ✅ |
| Token::Lt | `<` | 小于 | ✅ |
| Token::Gt | `>` | 大于 | ✅ |
| Token::Le | `<=` | 小于等于 | ⚠️ |
| Token::Ge | `>=` | 大于等于 | ⚠️ |
| Token::LtColon | `<:` | 约束符号 | ❌ |

### 3.2 算术

| Token | 符号 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::Plus | `+` | 加法 | ✅ |
| Token::Minus | `-` | 减法/负号 | ✅ |
| Token::Star | `*` | 乘法 | ✅ |
| Token::Slash | `/` | 除法 | ✅ |
| Token::Percent | `%` | 取模 | ❌ |
| Token::StarStar | `**` | 幂运算 | ❌ |

### 3.3 复合赋值

| Token | 符号 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::PlusEq | `+=` | 加法赋值 | ❌ |
| Token::MinusEq | `-=` | 减法赋值 | ❌ |
| Token::StarEq | `*=` | 乘法赋值 | ❌ |
| Token::SlashEq | `/=` | 除法赋值 | ❌ |
| Token::PercentEq | `%=` | 取模赋值 | ❌ |
| Token::PowEq | `**=` | 幂等赋值 | ❌ |
| Token::AmpEq | `&=` | 位与赋值 | ❌ |
| Token::PipeEq | `|=` | 位或赋值 | ❌ |
| Token::CaretEq | `^=` | 位异或赋值 | ❌ |
| Token::ShlEq | `<<=` | 左移赋值 | ❌ |
| Token::ShrEq | `>>=` | 右移赋值 | ❌ |

### 3.4 位/逻辑运算

| Token | 符号 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::Amp | `&` | 位与/引用 | ❌ |
| Token::Pipe_ | `|` | 位或/模式 | ❌ |
| Token::Caret (CaretOp) | `^` (紧贴) | 所有权 move/XOR | ⚠️ |
| Token::CaretInfix | ` ^` (前置留白) | 强制中缀 XOR | ❌ |
| Token::Shl | `<<` | 左移 | ❌ |
| Token::Shr | `>>` | 右移 | ❌ |
| Token::AmpAmp | `&&` | 短路逻辑与 | ⚠️ |
| Token::PipePipe | `\|\|` | 短路逻辑或 | ❌ |

### 3.5 标点

| Token | 符号 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::Colon | `:` | 冒号/类型标注 | ✅ |
| Token::Comma | `,` | 逗号 | ✅ |
| Token::Dot | `.` | 成员访问 | ✅ |
| Token::DotDot | `..` | 区间 | ⚠️ |
| Token::DotDotEq | `..=` | 闭区间 | ⚠️ |
| Token::DotDotDot | `...` | (已废弃) | ❌ |
| Token::Semicolon | `;` | 分号 | ⚠️ |
| Token::PathSep | `::` | 路径分隔 ← lexer.rs 已标记为废弃 | ❌ |
| Token::Arrow | `->` | 返回类型标注 | ✅ |
| Token::FatArrow | `=>` | match case 箭头 | ✅ |
| Token::Pipe | `\|>` | 管道操作符 | ⚠️ |
| Token::ColonEq | `:=` | 海象运算符 | ⚠️ |
| Token::LParen | `(` | 左圆括号 | ✅ |
| Token::RParen | `)` | 右圆括号 | ✅ |
| Token::LBrack | `[` | 左方括号 | ✅ |
| Token::RBrack | `]` | 右方括号 | ✅ |
| Token::LBrace | `{` | 左花括号 | ✅ |
| Token::RBrace | `}` | 右花括号 | ✅ |

### 3.6 特殊符号

| Token | 符号 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::At | `@` | 装饰器 | ⚠️ |
| Token::Question | `?` | 错误传播 | ⚠️ |
| Token::QuestionQuestion | `??` | 空值合并（旧版） | ❌ |
| Token::SafeNav | `?.` | 安全导航 | ⚠️ |
| Token::Exclamation | `!` | 宏调用/解引用 | ⚠️ |
| Token::Underscore | `_` | 占位符/模式通配 | ✅ |
| Token::Backtick | `` ` `` | 代码字面量 | ❌ |
| Token::Dollar | `$` | 宏插值 | ⚠️ |
| Token::Pound | `#` | 属性宏前缀 | ⚠️ |

### 3.7 构建块专用

| Token | 符号 | 说明 | 测试 |
|-------|------|------|:---:|
| Token::BuildAssign | `=:` | 变量构建块 | ✅ |
| Token::BuildCall | `~:` | 调用构建块 | ✅ |
| Token::BuildGen | `*:` | 生成器调用构建块 | ✅ |

### 3.8 虚拟 Token

| Token | 用途 | 测试 |
|-------|------|:---:|
| Token::Indent | 缩进增加 | ✅ |
| Token::Dedent | 缩进减少 | ✅ |
| Token::Newline | 逻辑行结束 | ✅ |
| Token::Eof | 流结束 | ✅ |
| Token::LexError | 词法错误 | ✅ |

---

## 四、表达式 AST (Expr 变体)

### 4.1 字面量

| Expr 变体 | 语法 | 测试 |
|----------|------|:---:|
| Expr::IntLit(i64) | `42` | ✅ |
| Expr::FloatLit(f64) | `3.14` | ⚠️ |
| Expr::StrLit(String) | `"hello"` | ✅ |
| Expr::FStrLit(String) | `f"...{x}..."` | ✅ |
| Expr::RawStrLit(String) | `r"..."` | ⚠️ |
| Expr::BoolLit(bool) | `True` `False` | ✅ |
| Expr::NoneLit | `None` | ⚠️ |
| Expr::Ident(String) | `my_var` | ✅ |
| Expr::Underscore | `_` | ✅ |

### 4.2 容器

| Expr 变体 | 语法 | 测试 |
|----------|------|:---:|
| Expr::ListLit(Vec<Expr>) | `[1, 2, 3]` | ✅ |
| Expr::DictLit(Vec<(Expr, Expr)>) | `{key: val}` | ⚠️ |
| Expr::SetLit(Vec<Expr>) | `{a, b, c}` | ⚠️ |
| Expr::TupleLit(Vec<Expr>) | `(1, 2)` | ✅ |

### 4.3 运算

| Expr 变体 | 语法 | 测试 |
|----------|------|:---:|
| Expr::Binary { op: Add } | `a + b` | ✅ |
| Expr::Binary { op: Sub } | `a - b` | ✅ |
| Expr::Binary { op: Mul } | `a * b` | ✅ |
| Expr::Binary { op: Div } | `a / b` | ✅ |
| Expr::Binary { op: Mod } | `a % b` | ❌ |
| Expr::Binary { op: Pow } | `a ** b` | ❌ |
| Expr::Binary { op: Eq } | `a == b` | ✅ |
| Expr::Binary { op: Ne } | `a != b` | ✅ |
| Expr::Binary { op: Lt } | `a < b` | ✅ |
| Expr::Binary { op: Gt } | `a > b` | ✅ |
| Expr::Binary { op: Le } | `a <= b` | ⚠️ |
| Expr::Binary { op: Ge } | `a >= b` | ⚠️ |
| Expr::Binary { op: And } | `a and b` | ⚠️ |
| Expr::Binary { op: Or } | `a or b` | ✅ |
| Expr::Binary { op: BitAnd } | `a & b` | ❌ |
| Expr::Binary { op: BitOr } | `a \| b` | ❌ |
| Expr::Binary { op: BitXor } | `a ^ b` | ❌ |
| Expr::Binary { op: Shl } | `a << b` | ❌ |
| Expr::Binary { op: Shr } | `a >> b` | ❌ |
| Expr::Binary { op: In } | `a in b` | ⚠️ |
| Expr::Binary { op: Is } | `a is int` | ❌ |
| Expr::Unary { op: Neg } | `-x` | ✅ |
| Expr::Unary { op: Not } | `not x` | ✅ |
| Expr::Unary { op: BitNot } | `~x` | ❌ |

### 4.4 调用

| Expr 变体 | 语法 | 测试 |
|----------|------|:---:|
| Expr::Call { func, args } | `f(1, 2)` | ✅ |
| Expr::Call { checker } | `f[checker](1, 2)` | ❌ |
| Expr::KwArg { name, value } | `f(key=1)` | ❌ |
| Expr::MethodCall { receiver, method, args } | `obj.method()` | ❌ |
| Expr::FieldAccess { receiver, field } | `obj.field` | ✅ |
| Expr::PathAccess { receiver, segment } | `mod::sub::item` | ❌ |
| Expr::Index { receiver, index } | `lst[0]` | ✅ |

### 4.5 控制流表达式

| Expr 变体 | 语法 | 测试 |
|----------|------|:---:|
| Expr::If { cond, then, elif, else } | `if x > 0: 1 elif x < 0: -1 else: 0` | ✅ |
| Expr::Match { expr, arms } | `match x: case 0: ...` | ✅ |

### 4.6 特殊表达式

| Expr 变体 | 语法 | 测试 |
|----------|------|:---:|
| Expr::Closure { params, body } | `\|x\| x + 1` 或匿名闭包 | ⚠️ |
| Expr::Range { start, end, inclusive } | `0..10` `0..=9` | ⚠️ |
| Expr::Walrus { target, value } | `(x := expr)` | ⚠️ |
| Expr::Pipe { receiver, func, args } | `data \|> f()` | ⚠️ |
| Expr::SafeNav { receiver, field } | `obj?.field` | ⚠️ |
| Expr::Try(Box<Expr>) | `expr?` | ⚠️ |
| Expr::NullCoalesce { left, right } | `a ?? b` | ❌ |
| Expr::ListComprehension { output, var, iter, cond } | `[x*2 for x in lst if x>0]` | ⚠️ |
| Expr::Assign { target, op, value } | `x += 1` | ❌ |
| Expr::Spawn(Box<Expr>) | `spawn f()` | ⚠️ |
| Expr::Move(Box<Expr>) | `x^` | ⚠️ |
| Expr::Panic(Box<Expr>) | `panic("msg")` | ⚠️ |
| Expr::Await(Box<Expr>) | `await f()` | ❌ |
| Expr::BuildBlock { kind, lhs, body } | `=:` / `~:` / `*:` 块 | ✅ |
| Expr::Comptime(Box<Expr>) | `comptime { ... }` | ⚠️ |
| Expr::TryCatch { body, catches, else_body, finally_body } | `try: ... catch e: ...` | ⚠️ |

---

## 五、语句 AST (Stmt 变体)

| Stmt 变体 | 语法 | 测试 |
|----------|------|:---:|
| Stmt::Expr(Expr) | 表达式语句 | ✅ |
| Stmt::Let { name, mutable, is_ref, comptime, ty, value } | `let x = 1` | ✅ |
| Stmt::Const { name, comptime, ty, value } | `const MAX = 100` | ⚠️ |
| Stmt::FnDef(Function) | 内嵌 `def f():` | ⚠️ |
| Stmt::TypeAlias(TypeAlias) | 内嵌 `type T = int` | ⚠️ |
| Stmt::Comptime(Vec<Stmt>) | `comptime: ...` | ⚠️ |
| Stmt::Pass | `pass` | ⚠️ |
| Stmt::Return(Option<Expr>) | `return x` | ⚠️ |
| Stmt::Yield(Option<Expr>) | `yield x` | ⚠️ |
| Stmt::YieldFrom { expr, transform } | `yield from expr` | ❌ |
| Stmt::While { cond, body } | `while i < n: ...` | ✅ |
| Stmt::For { var, iter, body } | `for x in lst: ...` | ⚠️ |
| Stmt::Loop(Vec<Stmt>) | `loop { ... }` | ⚠️ |
| Stmt::Break(Option<Expr>) | `break` `break expr` | ⚠️ |
| Stmt::Continue | `continue` | ⚠️ |
| Stmt::Defer(Vec<Stmt>) | `defer: cleanup()` | ⚠️ |
| Stmt::Raise(Expr) | `raise Error("msg")` | ⚠️ |
| Stmt::Guard { cond, let_binding, else_body } | `guard x > 0 else: ...` | ⚠️ |
| Stmt::With { expr, alias, body } | `with open(f) as fh: ...` | ⚠️ |
| Stmt::Assign { target, op, value } | `x += 1` | ✅ |
| Stmt::Test { name, body } | `test "name": assert ...` | ✅ |
| Stmt::Assert { expr, expected, message } | `assert x == 1` | ✅ |
| Stmt::Suite { name, tests } | `suite "s": test ...` | ✅ |
| Stmt::Check { expr, expected, message } | `check x > 0` | ⚠️ |

---

## 六、类型系统 (Type 枚举)

| Type 变体 | Rust 映射 | 说明 | 测试 |
|----------|----------|------|:---:|
| Type::Var(TypeVar) | `_` (推断) | 类型推断变量 | ⚠️ |
| Type::Int | `i64` | 64位有符号整数 | ✅ |
| Type::F64 | `f64` | 64位浮点 | ⚠️ |
| Type::Float | `f64` | float 别名 (= f64) | ❌ |
| Type::Str | `String` | 字符串 | ✅ |
| Type::Bool | `bool` | 布尔 | ✅ |
| Type::None_ | `()` | 空类型/Unit | ⚠️ |
| Type::Never | `!` | 发散类型 | ❌ |
| Type::Any | `std::any::Any` | 动态类型 | ❌ |
| Type::Unit | `""` (空) | 枚举无字段变体 | ⚠️ |
| Type::Named(String) | 原样传递 | 命名类型 | ✅ |
| Type::Generic { base, args } | `Vec<T>` `HashMap<K,V>` | 泛型实例 | ⚠️ |
| Type::Option(Box<Type>) | `Option<T>` | Option 容器 | ⚠️ |
| Type::Result { ok, err } | `Result<T, E>` | Result 容器 | ❌ |
| Type::Optional(Box<Type>) | `Option<T>` | `T?` 语法糖 | ⚠️ |
| Type::Ref(Box<Type>) | `&T` | 不可变引用 | ⚠️ |
| Type::MutRef(Box<Type>) | `&mut T` | 可变引用 | ⚠️ |
| Type::Fn { params, ret } | `fn(...) -> T` | 函数类型 | ⚠️ |
| Type::Tuple(Vec<Type>) | `(T1, T2)` | 元组类型 | ✅ |
| Type::Simd { elem, width } | `wide::TxN` | SIMD 向量 | ❌ |
| Type::Self_ | `Self` | Self 占位 | ⚠️ |

---

## 七、声明 AST (decl.rs 核心结构)

| 结构体 | 说明 | 测试 |
|-------|------|:---:|
| Module | 模块顶层（imports/functions/structs/traits/impls/consts/type_aliases/magic_decls/tests） | ✅ |
| Function | 函数声明（name/generics/params/return_type/raises/where_clause/body/is_async/is_abstract/comptime/decorators/attributes/variadic/params_checker） | ✅ |
| StructDef | 结构体/枚举声明（name/generics/fields/methods/is_enum/decorators/attributes/repr_attr） | ✅ |
| TraitDef | trait 声明（name/generics/methods/fields/type_aliases） | ⚠️ |
| ImplDef | impl 块（trait_name/type_name/generics/where_clause/methods/type_aliases） | ⚠️ |
| ConstDef | 模块级常量（name/ty/value/mutable/comptime） | ⚠️ |
| TypeAlias | 类型别名（name/generics/ty/where_clause/scope） | ⚠️ |
| MagicDecl | 魔法方法声明（name/is_pub/desc） | ⚠️ |
| ImportStmt | 导入语句（path/alias/items/is_from） | ⚠️ |
| Param | 函数参数（name/ty/default/is_mut/is_owned/is_ref/is_positional_only） | ✅ |
| WhereBound | where 约束（type_param/bounds） | ⚠️ |
| Decorator | 装饰器（name/args） | ⚠️ |
| Field | 结构体字段（name/ty/default） | ✅ |
| VariadicSpec | 可变参数规格（mode/collect_ty） | ⚠️ |
| Pattern (枚举) | 模式匹配（Int/Str/Bool/Ident/Variant/Tuple/Wildcard） | ✅ |

---

## 八、覆盖率摘要

| 层面 | 总变体数 | ✅ | ⚠️ | ❌ |
|------|--------|---|---|---|
| 关键字 Token (含分类) | 46 | 8 | 29 | 9 |
| 字面量/标识符 Token | 8 | 3 | 5 | 0 |
| 运算符/标点 Token | 44 | 15 | 16 | 13 |
| 虚拟 Token | 5 | 5 | 0 | 0 |
| Expr 变体 | 43 | 17 | 18 | 8 |
| Stmt 变体 | 25 | 9 | 15 | 1 |
| Type 变体 | 20 | 5 | 10 | 5 |
| 声明结构 | 15 | 6 | 9 | 0 |

**整体估算**: ~45% 有自动化测试覆盖, ~40% 有 DEMO 示例但无自动化测试, ~15% 无任何测试。

**高风险区域** (需优先补充测试):
1. 位运算与复合赋值运算符 (10+ 个 Token 无测试)
2. 异常处理完整链路 (try/catch/finally/raise/raises)
3. 并发原语 (async/await/spawn → 端到端)
4. 类型系统边缘变体 (Never, Any, Simd, Result)
5. 管道 (|>) / 安全导航 (?.) / 闭包 → 基础用例
