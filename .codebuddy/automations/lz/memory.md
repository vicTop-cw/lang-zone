# LZ 开发自动化记忆

## 最近运行: 2026-08-02 (Round 11)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 107/129 (稳定)

### 本轮完成 (Round 11)

1. **函数泛型参数自动 Debug 约束** (上一轮延续, codegen.rs)
   - gen_fn_generics 仅对函数泛型参数追加 Debug
   - 与 gen_generics(struct/enum/impl) 分离, 避免 E0229
   - generics.lz 的 print(identity<T>) 正确 (E0277)

2. **~: dict 调用解包 (尝试后回滚)**
   - 尝试为 greet ~: {Dict} 实现 dict→kwargs 解包
   - 需要 fn_param_names 映射 + 修改 codegen 的 UnpackBuildCall
   - 发现 codegen 的 __t.{idx} 元组机制不适用于 HashMap, 需不同 codegen
   - 已完全回滚, 保持元组解包正常工作
   - 教训: dict→kwargs 解包需要独立的 codegen 机制, 与元组不同

### 关键成果
- rustc 通过率 105 → 107/129 (本轮累计)
- 泛型函数 print 正确工作

### 待处理 (均为需实现的特性)
- E0425 (11): duck typing / 安全导航 ?. / while-guard / 全局变量 / 高级运算符
- E0308 (9): 类型不匹配
- E0277 (3): __Params Box<dyn Any> / 泛型 trait
- E0369 (2): __Params Any 比较
- E0061 (1): ~: dict 调用解包
- 主要剩余: duck typing, 安全导航 ?., 空值合并 ??, 错误传播 ?,
  iterator 关键字, while-guard, 全局可变变量, ~: dict 调用,
  __Params checker, 非标准 ?: 三元
