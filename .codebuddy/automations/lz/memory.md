# LZ 开发自动化记忆

## 最近运行: 2026-08-03 (Round 14)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 108/129 (稳定)

### 本轮尝试 (模块级全局变量 - 已完全回滚)

**实现**: 检测跨函数引用的变量(count), 生成 static mut + unsafe 访问
- combo_while_walrus.lz 验证通过 (输出 1 2 3 4 5)
- 但导致 7 个 demo 回归 (closures_more/composite/pipe/match_more/var_call_block/iterator_demo/keyword_downgrade)
- E0530 从 1→7, E0614 新增 3

**回滚原因**: 跨函数全局检测过于激进
- collect_var_refs 收集了函数名/闭包捕获/装饰器参数等被误判为全局的变量
- 如 composite.rs 的 @math sq(x,y) 中 x 被误提升为 static mut
- 需要精确的作用域分析才能区分真全局 vs 参数/捕获/装饰器产物

**决定**: 完全回滚, 恢复 108 基线, 无净变更
- 教训: 跨函数全局检测需配合精确作用域分析(区分参数/闭包捕获/装饰器),
  不能简单用"函数内未声明"启发式

### 保留成果 (上一轮 Round 13, 已推送)
- 安全导航 ?. 实现 (null_safe.lz 输出 "x")
- Any 闭包参数省略类型注解

### 待处理 (均为需实现的特性)
- E0425 (9): 模块级全局变量(需精确作用域分析) / duck typing / 高级运算符
- E0308 (10): 类型不匹配
- E0530 (1): 关键字降级
- 主要剩余: 全局变量(高影响但需精确分析), duck typing, catch 枚举载荷,
  iterator 关键字, __Params checker, 非标准 ?: 三元
