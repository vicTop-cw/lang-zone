# DEMO/boundary-coverage — 语法边界全覆盖 Demo

本目录包含 LZ 语言**所有语法边界的深度测试用例**，重点覆盖嵌套、语法组合、边界值等场景。

## 文件清单

| 文件 | 类型 | 覆盖内容 |
|:----|:----|---------|
| `nesting-control-flow.lz` | 嵌套 | if/elif/else 四层嵌套、for 三层嵌套 + if守卫、for/while 混合嵌套、match 内嵌控制流、嵌套 break/continue、循环 else |
| `nesting-expressions.lz` | 嵌套 | 五层算术表达式、六层逻辑表达式、运算符混合嵌套、函数调用链嵌套、三元表达式嵌套、方法链、管道链、推导式嵌套、match 表达式嵌套 |
| `nesting-data-types.lz` | 嵌套 | 三层嵌套 struct、嵌套泛型 struct、三层嵌套 enum、泛型枚举递归、四层模式匹配解构、嵌套集合类型、嵌套 Option/Result、struct+impl+魔法方法、enum+impl |
| `nesting-closure-lambda.lz` | 嵌套 | 三层闭包嵌套、闭包作为返回值、高阶函数组合、多层变量捕获、偏应用多洞嵌套、闭包数组、闭包内嵌 match/if |
| `combo-error-control.lz` | 组合 | try 内嵌 for、for 内嵌 try+match、try 表达式+match、catch 内嵌 if、guard+try+finally、defer+try、try+else+for、? 传播+Option、多分支 catch |
| `combo-async-await.lz` | 组合 | async+try/catch、spawn+catch、多 spawn 并发、go+defer、go+for、async+match |
| `combo-struct-method.lz` | 组合 | struct+guard+try、泛型 struct+match 方法、struct+defer、struct+match守卫、trait+impl+struct、enum+impl+闭包 |
| `combo-build-block.lz` | 组合 | =: + if/else、=: + match、=: + try、=: + 推导式、~: 调用块、^: 索引块、*: 生成器块、多层 =: 嵌套 |
| `combo-iterator-generator.lz` | 组合 | iterator+yield+for、iterator+if条件yield、iterator+try/catch+guard、yield from 委托、*: 生成器块+yield、iterator+闭包 |
| `combo-defer-guard.lz` | 组合 | defer LIFO顺序、defer+guard、guard+try+catch、defer+try+finally+guard、with+defer+try、guard let+try、嵌套 with、try+else+guard |
| `combo-pipe-lambda.lz` | 组合 | 七级管道链、管道+闭包+集合、管道+match、管道+偏应用、管道+条件函数 |
| `combo-trait-impl.lz` | 组合 | 泛型 trait+默认方法、impl 多约束、where 子句、trait+enum、trait 继承、impl+闭包方法 |
| `edge-values-boundary.lz` | 边界 | int/f64 极值、零值、空集合、None/Option 边界、各种进制字面量、科学计数法边界、字符串边界（空/unicode/raw/多行/f-string）、bool 全部组合、运算符边界（移位/幂/除/取模/取负） |
| `edge-scope-shadow.lz` | 边界 | 三层变量遮蔽、模块级+函数级+块级遮蔽、const 可见性、for 循环变量作用域、match 分支作用域、闭包捕获+遮蔽、try/catch/finally 作用域、defer 作用域、多层嵌套块作用域、if 分支作用域 |
| `edge-walrus-operator.lz` | 边界 | := 在 if/while 条件中、:= 嵌套条件、:= + match、模块级 :=、:= + for 守卫 |
| `edge-keyword-identifier.lz` | 边界 | 关键字降级（Ok/Some/None/Err/True/False 作变量）、下划线七种语义、magic 魔法方法、特殊字面量、标识符含数字/下划线、块注释边界 |

## 语法规范保证

- 所有文件均使用 **4 空格缩进**
- 布尔值使用 `True`/`False`，逻辑运算符使用 `and`/`or`/`not`
- 二元运算符**前后均留空格**，一元运算符**紧贴操作数**
- 函数调用**括号紧贴函数名**
- 枚举数据变体使用**关键字参数构造**（`Option.Some(value: 42)`）
- 魔法方法中 `__str__`/`__eq__`/`__len__`/`__getitem__` 使用 `ref self`（借用）
- `__add__`/`__sub__`/`__mul__` 使用 `self`（owned）
- 文件均无分号 `;`

## 设计原则

1. **无重复**：未覆盖 `DEMO/combo-syntax/` 和 `DEMO/99_spec/combo-syntax/` 已有组合
2. **无重复**：未覆盖 `DEMO/99_spec/` 已有边界测试
3. **互补**：与现有 DEMO 形成完整的语法覆盖矩阵
4. **可编译**：每个文件包含 `def main()` 入口，可独立编译运行
