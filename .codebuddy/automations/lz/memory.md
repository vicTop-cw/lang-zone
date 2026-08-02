# LZ 开发自动化记忆

## 最近运行: 2026-08-02 (Round 9)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 97 → 105/129 (本轮 +8)

### 本轮完成 (Round 9)

1. **with 块无 as 绑定的 __exit__ 误调用** (ir/builder.rs)
   - with <普通表达式>: 无 as 绑定时也生成 __exit__ → E0599
   - 修复: 仅当 with 有 as 绑定(上下文管理器)时才生成 __exit__
   - 验证 with_defer.lz 的 standalone with 不再调用 __exit__

2. **元组解构 move (E0382)** (ir/codegen.rs)
   - let (x,y,z) = t 生成 __destruct_ = t 移动元组
   - 修复: __destruct_ / 元组模式解构对源元组 .clone()
   - 验证 primitives.lz 元组重复解构不报 E0382

### 本轮尝试(已回滚 - 需记录教训)
- **泛型自动 Debug bound**: 给所有泛型参数加 Debug 使 print 工作
  导致 struct/enum/impl 泛型 E0229 回归, 已 revert
- **关键字降级变量重命名**: 重命名 Ok/Some/None/Err 变量
  导致 Some()/Ok() 构造函数被误重命名(E0425), 已 revert
- 教训: 共享 codegen 路径(gen_generics/rename)的改动需谨慎,
  只影响目标场景, 不能影响构造函数/struct impl

### 关键成果
- rustc 通过率 97 → 105/129
- with 上下文管理器、元组重复解构等特性正确

### 待处理 (均为需实现的特性, 非简单 bug)
- E0425 (11): duck typing / 安全导航 ?. / while-guard / 全局变量 / 高级运算符
- E0308 (9): 类型不匹配 (~: dict 调用构建块 / return in closure 等)
- E0277 (4), E0382 (3), E0369 (2), E0599 (2)
- 主要剩余: duck typing, 安全导航 ?., 空值合并 ??, 错误传播 ?,
  iterator 关键字, while-guard, 全局可变变量, ~: 构建块 dict 调用
