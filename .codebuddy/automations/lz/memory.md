# LZ 开发自动化记忆

## 最近运行: 2026-08-03 (Round 17)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 110 → 111/129

### 本轮完成 (Round 17)

1. **空集合/None 默认类型注解 (E0282)** (ir/codegen.rs)
   - let b: List = [] (裸 List) → Vec<_> 无法推断元素类型
   - let x = None → Option<_> 无法推断
   - 修复: 空容器/None 默认元素类型为 i64
     - rust_type: List/Vec/Option/Dict/Set 空 args → <i64>
     - ty_str: 空容器用 rust_type(ty) 而非硬编码 Vec<_>
     - None (LitKind::None_ / StructCtor None / Var("None")) → : Option<i64>
   - 验证 ir-edge-empty-collections.lz 输出 [] [] None None

### 关键改进
- rustc 通过率 110 → 111/129
- E0282 从 1 → 0
- None 的 IR 表示是 Var("None") 而非 LitKind::None_ (关键调试发现)

### 待处理 (均为需实现的特性)
- E0308 (9): 类型不匹配
- E0425 (9): duck typing / 高级运算符 / catch 枚举载荷
- E0277 (3): __Params Box<dyn Any>
- 主要剩余: duck typing, catch 枚举载荷提取, iterator 关键字,
  __Params checker, 非标准 ?: 三元
