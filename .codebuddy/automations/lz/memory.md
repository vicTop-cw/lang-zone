# LZ 开发自动化记忆

## 最近运行: 2026-08-02 (Round 8)

### 状态
- 所有测试通过: 292 单元测试 + 1 编译测试(129 demo) + 8 IR 快照 = 301 ✅
- rustc 实际编译通过率: 103 → 104/129 (持续提升)

### 本轮完成 (Round 8)

1. **字符串拼接 String + String → String + &str** (codegen.rs)
   - "num: " + str(42) 生成 String + String 报 E0308
   - RHS 用 &{}[..] 切为 &str, 兼容 format!/.to_string()/变量

2. **str/int/float/bool/len 内建返回类型推断** (builder.rs)
   - str(42) 返回类型从 Any 改为 Str, 使字符串拼接分支正确触发

### 关键尝试 (本轮走过的弯路 - 需记录)
- **match 语句尾值丢弃**: 尝试让语句级 match 的 arm 尾值丢弃为 ()
  导致 13 个 demo 回归 (值型 match 被错误丢弃), 已完全 revert
  教训: match 代码gen 是共享路径, 不应粗暴丢弃 arm 值

### 提交记录 (Round 8)
- 8c7ff85: 字符串拼接 + 内建类型推断
- abd7f3e: revert match 语句丢弃 (修复回归)
- dbcd9c1: match 语句尝试(已revert)
- 已推送到 github + gitcode

### 累计 (Round 7 + 8 的核心成果, 均已推送)
- for 元组解构 (enumerate/zip)
- 泛型内联 trait 约束 + Ordered/Equatable/Hashable 映射
- Rc/Arc/Box clone 返回类型
- 标量 const 解引用 (lazy_static_names)
- 独立 magic 块 (magic __str__/__add__ → impl T)
- stmt_has_await 过期 IR 引用修复
- 字符串拼接 + 内建类型推断

### 关键成果
- rustc 实际通过率 90 → 104/129
- E0614/E0608/E0282 大幅减少

### 待处理 (均为需要实现的特性, 非简单 bug)
- E0425 (11): duck typing / 安全导航 ?. / while-guard / 全局变量
- E0308 (9): 类型不匹配 (多为未实现特性)
- E0277 (4): trait 不满足
- 主要剩余: duck typing, 安全导航 ?., 空值合并 ??, 错误传播 ?,
  ownership move ^, while-guard 等规范目标特性未实现
