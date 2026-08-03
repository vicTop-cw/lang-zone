# LZ 开发自动化记忆

## 最近运行: 2026-08-03 (Round 15)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 108/129 (稳定, 无回归)

### 本轮完成 (Round 15)

1. **模块级全局变量 (跨函数共享)** (ir/codegen.rs)
   - 检测在函数 A 引用但未在 A 声明、在另一函数声明的变量(count)
   - 生成 static mut + unsafe 访问
   - 关键: 精确作用域分析 - 闭包参数遮蔽 (Lambda 参数加入 shadow 集)
   - 解决上一轮回归: composite/pipe/match_more 不再被误判
   - 验证: combo_while_walrus.lz (1 2 3 4 5), while_walrus_guard_1.lz 编译通过

### 关键改进 (vs 上一轮失败尝试)
- 上一轮 naive 检测导致 7 个 demo 回归 (闭包参数被误判为全局)
- 本轮加入 shadow 集: 遍历 Lambda 时, 闭包参数加入遮蔽集, 不作为全局候选
- collect_var_refs 接受 shadow 参数, 精确区分闭包参数 vs 真自由变量

### 待处理 (均为需实现的特性)
- E0425 (11): duck typing / 高级运算符 / catch 枚举载荷(msg)
- E0308 (8): 类型不匹配
- E0277 (3): __Params Box<dyn Any>
- 主要剩余: duck typing, catch 枚举载荷提取, iterator 关键字,
  __Params checker, 非标准 ?: 三元, 空集合默认类型
