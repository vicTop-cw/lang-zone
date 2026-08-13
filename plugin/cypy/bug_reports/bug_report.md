# Cypy 编译器 Bug 报告

> 测试基准：CYPY_SYNTAX.md v1.0.0 (2026-07-24)
> 编译器版本：cypyc 0.1.0
> 测试日期：2026-07-25

## 发现的 Bug 一览

共发现 34 个 Bug，其中 5 个 Critical、9 个 High、20 个 Medium。

### Critical (5个)
- Bug-1: 字符串中 " 转义丢失，生成无效 Cython 代码
- Bug-2: 字符串中 \ 反斜杠转义丢失
- Bug-3: \n \r \t 转义丢失
- Bug-4: struct 方法中复合赋值 self.pos = self.pos + 1 被截断为 self.pos
- Bug-5: match/case 所有分支逻辑在代码生成阶段丢失

### High (9个)
- Bug-6: 构建块 =: lambda 体压缩为单行无分隔符
- Bug-7: 构建块 ~: return 语句被错误拆分
- Bug-8: 构建块 *: yield 语句全部丢失
- Bug-9: f-string 的 f 前缀和插值丢失
- Bug-10: trait 方法中 self 参数重复
- Bug-11: impl 方法未绑定到 struct
- Bug-12: async def 丢失 async 关键字
- Bug-13: await 生成占位符 AwaitExpr(...)
- Bug-14: macro 生成占位符 BacktickBlock(...)/MacroCall(...)

### Medium (16个)
- Bug-15: enum 定义在作用域分析阶段失败
- Bug-16: _ 通配符未注册为内置符号
- Bug-17: True/False 未在 scope_analyzer 中注册
- Bug-18: guard let 变量未注册到作用域
- Bug-19: __name__ 被类型检查器报错
- Bug-20: owend 关键字未在 lexer 中定义
- Bug-21: suite/test 关键字未在 lexer 中定义
- Bug-22: while 循环不被解析器支持
- Bug-23: or 在 if 条件中不被支持
- Bug-24: {} 字典字面量被解析为 struct 字面量
- Bug-25: case _: 通配符模式不被支持
- Bug-26: case -1: 负数字面量模式不被支持
- Bug-27: 函数签名中 int | str 联合类型导致参数无法识别
- Bug-28: type 类型别名不生效
- Bug-29: match/case 第一个分支 return 总是执行
- Bug-30: val 不可变性仅编译期检查

### Phase 6 Lexer 发现的新 Bug (4个)
- Bug-31: ord() 内置函数不可用 —— 报 `Undefined name 'ord'`，Cypy 未注册 ord 为内置函数
- Bug-32: range() 内置函数不可用 —— `for _ in range(n)` 报 `Invalid indentation level`，解析器不识别 range() 调用
- Bug-33: list[T] 的 .append() 方法不可用 —— struct 字段上的 list 类型无法调用 .append() 方法
- Bug-34: continue 关键字在嵌套 if 中生成到错误层级 —— 生成的 .pyx 中 continue 被放在 `if pos < end:` 块内而非直接放在 for 循环体下

## 正常工作的特性
- comptime ✅
- implicit struct ✅
- guard ✅
- defer ✅
- let ✅
- val ✅
- |> 管道 ✅
- as 类型转换 ✅
- struct 定义 ✅
- struct 字面量构造 ✅
- def 函数定义 ✅
- 类型注解 ✅
- print ✅
- f-string 字面量（无插值） ✅