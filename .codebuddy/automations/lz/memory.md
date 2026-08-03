# LZ 开发自动化记忆

## 最近运行: 2026-08-03 (Round 18)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 110/129

### 本轮完成 (Round 18)

1. **Box/Rc/Arc 指针下标 0 解引用 (E0608)** (ir/codegen.rs)
   - boxed[0] 生成 boxed[0i64] 报 cannot index Box<i64>
   - 修复: Box/Rc/Arc 基址的下标 0 → (*base) 解引用
   - 关键: gen_expr(IntLit(0)) 渲染为 0i64 而非 0, 用 key.kind 判断
   - 验证: primitives.lz (输出 42/3.14/hello/true),
     box_rc_arc.lz (42), rc_arc_more.lz (1, 10 10, 20 20)

### 关键改进
- E0608 从 3 → 0 (消除指针索引错误)
- 修复 3 个指针 demo (primitives/box_rc_arc/rc_arc_more)
- 教训: 需先重建二进制再重新生成 .rs, 否则编译的是陈旧文件

### 待处理 (均为需实现的特性)
- E0425 (9): duck typing / 高级运算符 / catch 枚举载荷 / while-guard
- E0308 (8): 类型不匹配
- E0277 (3): __Params Box<dyn Any>
- 主要剩余: duck typing, catch 枚举载荷提取, iterator 关键字,
  __Params checker, 非标准 ?: 三元
