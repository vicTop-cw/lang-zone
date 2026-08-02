# LZ 开发自动化记忆

## 最近运行: 2026-08-02 (Round 10)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 105 → 107/129

### 本轮完成 (Round 10)

1. **函数泛型参数自动 Debug 约束** (ir/codegen.rs)
   - gen_fn_generics 仅对函数泛型参数追加 Debug 约束
   - 解决 generics.lz 中 print(identity<T>) 报 E0277 (T: Debug)
   - 与 gen_generics(struct/enum/impl) 区分, 避免 E0229 回归

### 本轮分析 (未实现特性)
- **~: 构建块元组解包**: add ~: (10,20) → add(10,20) 元组场景已实现
  - 但 greet ~: {Dict} 字典拆包为 kwargs 未实现 (E0061)
  - multiply ~: factors 变量元组未实现 (block_ty 为 Any)
- **=: 构建块 return 类型冲突**: return; (单元) + 尾值 100 的闭包返回类型不匹配 (E0308)

### 关键成果
- rustc 通过率 105 → 107/129
- 泛型函数 print 正确工作

### 待处理 (均为需实现的特性, 非简单 bug)
- E0425 (11): duck typing / 安全导航 ?. / while-guard / 全局变量 / 高级运算符
- E0308 (9): 类型不匹配 (~: dict 调用 / return in closure / 非标准 ?: 三元)
- E0277 (3), E0369 (2), E0599 (2), E0382 (2)
- 主要剩余: duck typing, 安全导航 ?., 空值合并 ??, 错误传播 ?,
  iterator 关键字, while-guard, 全局可变变量, ~: dict 调用,
  __Params checker, 非标准 ?: 三元(combo_match_ternary)
