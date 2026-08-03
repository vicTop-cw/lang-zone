# LZ 开发自动化记忆

## 最近运行: 2026-08-03 (Round 16)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 109 → 110/129

### 本轮完成 (Round 16)

1. **修复全局变量分析的 match 模式绑定回归** (ir/codegen.rs)
   - match_more.rs 的 case Shape.Circle(radius: r) 中 r 是模式绑定
   - 全局变量分析把 r 误判为全局 → E0530
   - 修复: collect_local_lets 收集 match 模式绑定名 (collect_pattern_bindings)
   - 验证 match_more.lz 输出 on/waiting/12.56/12.0

2. **关键字降级 (Ok/Some/None/Err 作变量名)** (ir/codegen.rs)
   - let Ok = 1 / let Some = 2 / let None = 3 报 E0530
   - 修复: 仅对实际 let 声明的降级变量重命名为 name_
   - 关键: 作用域限定 - 只有 let 声明才注册到 downgraded_vars
     (构造函数 Some(42)/Ok(1) 不被影响)
   - 验证 keyword_downgrade.lz 输出 6

### 关键改进
- rustc 通过率 109 → 110/129
- 关键字降级: 通过 let 声明追踪, 精确区分变量 vs 构造函数
- match 模式绑定纳入全局分析遮蔽集

### 待处理 (均为需实现的特性)
- E0308 (9): 类型不匹配
- E0425 (9): duck typing / 高级运算符 / catch 枚举载荷
- E0277 (3): __Params Box<dyn Any>
- 主要剩余: duck typing, catch 枚举载荷提取, iterator 关键字,
  __Params checker, 非标准 ?: 三元, 空集合默认类型
