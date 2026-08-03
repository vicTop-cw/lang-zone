# LZ 开发自动化记忆

## 最近运行: 2026-08-03 (Round 12)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 107 → 108/129

### 本轮完成 (Round 12)

1. **枚举变体类型推断** (ir/builder.rs)
   - Kind.A(1) 被解析为 MethodCall{receiver:Kind}, 推断为 Any
   - 修复: receiver 是已知 enum/struct 类型名时, MethodCall 返回该类型
   - 修复 [Kind.A(1), ...] 被推断为 Vec<i64> 而非 Vec<Kind> 的 E0308

2. **for-guard 过滤闭包非 Copy 元素 clone** (ir/codegen.rs)
   - for it in items if keep(it): 生成 |&it| keep(it) 移动 &Kind (E0507)
   - 修复: 非 Copy 元素且 guard 按值传参时, 生成 |it| { let it_owned=(*it).clone(); guard(it_owned) }
   - 与原始类型(|&x| Copy 解构)和字段访问(|p| 引用)区分

### 验证
- combo_for_guard_match.lz: 输出 a:1 b:"x" a:2 ✓ (枚举变体 + for守卫 + match + f-string)

### 关键成果
- rustc 通过率 107 → 108/129
- 枚举变体集合类型、for 守卫非 Copy 元素正确

### 待处理 (均为需实现的特性)
- E0425 (11): 全局变量(count) / duck typing / while-guard / 安全导航 ?. / 高级运算符
- E0308 (8): 类型不匹配 (match 语句丢弃 / 非标准 ?: 三元)
- E0277 (3): __Params Box<dyn Any>
- 主要剩余: 模块级全局变量(影响多个 while demo), duck typing, 安全导航 ?.,
  空值合并 ??, 错误传播 ?, iterator 关键字, while-guard,
  __Params checker, 非标准 ?: 三元
