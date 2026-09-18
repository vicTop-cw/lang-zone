# ΣLang 修复计划 — 2026-09-17

> 基于 FIND_BUG.md / TODO-STATUS.md / 实际代码状态全面盘点

## 一、基线状态

| 指标 | 数值 |
|---|---|
| 单元测试 | 420 passed / 0 failed |
| 集成测试 (find_bug_libs) | 12 passed / 0 failed |
| FIND_BUG 编译 | 54/54 文件通过 |
| DEMO 编译 | 348/348 文件通过 |
| 库转正进度 | 7/12 (option/pattern/sort/result/vector/closure/linked_list) |

## 二、已修复但文档过时（需同步 FIND_BUG.md）

| 文档标记 | 实际状态 | 本轮修复 |
|---|---|---|
| BUG-EC-002 ❌ | ✅ i128 已支持 | 测试转正 |
| core 3 例 ❌ | ✅ fn 类型已修复 | 测试转正 + unique.lz 源修复 |
| contains 参数缺 & | ✅ 本轮修复 | free-function contains 加 & |

**行动**：FIND_BUG.md 中以下项应标记 ✅：
CG-002, CG-004, PR-002, PR-005, TY-002, TY-004, IR-005, IR-002, SG-002, SG-003, SG-005, EC-002, EC-006, SB-001, SB-002, SB-003, LX-002, LX-005, PR-001

---

## 三、真实缺陷（按优先级）

### P0 — 阻塞级（类型系统核心）

| ID | 问题 | 根因 | 修复方案 |
|---|---|---|---|
| **BUG-TY-001** | duck 自引用 → E0391 | trait 方法参数引用 duck 自身时未生成 `&dyn Duck` | codegen: 检测参数类型为 duck 名 → 生成 `&dyn DuckName` |
| **BUG-IR-003** | 嵌套 def 捕获 → E0530 | 嵌套函数提升为全局 static mut，捕获语义错误 | builder: 闭包脱糖为 `Box<dyn Fn>` 捕获结构体 |

### P1 — 重要（功能可用性）

| ID | 问题 | 根因 | 修复方案 |
|---|---|---|---|
| **BUG-IR-001** | `~:` 参数位（`filter(~: _%2==0)`） | 解析侧已通，codegen 需闭包脱糖 | builder: `BuildKind::Call` 无 callee → `\|__arg\| { body }` |
| **BUG-IR-002** | defer 捕获借用冲突 E0499 | 闭包持有 &mut 至作用域结束与后续使用冲突 | 方案 A: 内联脱糖到各出口；方案 B: Rc<RefCell<T>> |
| **lib_string** | E0308/E0277 | 字符串索引/contains 语义 | 需与 json.lz 一起定字符串索引语义 |
| **lib_hashmap** | E0382 移动语义 | HashMap key 被 move 后仍使用 | 克隆 key 或重构 API |
| **lib_tree** | E0369 `Vec+i64` | 运算符重载未接通 | 检查 __add__ 接线 |
| **lib_iterator** | E0599 `&mut MapIter` | 需用户 trait 参数特性 | 扩展 duck 参数约束 |
| **json.lz** | 49 个错误跨 6 子系统 | 字符串索引语义冲突 + 多个小问题 | 先定字符串索引语义，再系统性修复 |

### P2 — 一般

| ID | 问题 | 说明 |
|---|---|---|
| BUG-LX-002 | 嵌套块注释 | 轮次 12 已修，文档未同步 |
| BUG-LX-005 | `=:` 内联形态 | 轮次 12 已修，文档未同步 |
| BUG-TY-005 | 泛型默认值报错误导 | 诊断质量，低优先级 |
| TODO × 12 | codegen 占位符 | 渐进式实现，不影响功能 |

### P3 — 提示

| ID | 问题 | 说明 |
|---|---|---|
| BUG-LX-002 | 若规范要嵌套需深度计数 | 文档明确非嵌套即可 |

---

## 四、lib_* 12 库详细计划

| 库 | 状态 | 修复策略 |
|---|---|---|
| lib_option | ✅ | — |
| lib_pattern | ✅ | — |
| lib_sort | ✅ | — |
| lib_result | ✅ | — |
| lib_vector | ✅ | — |
| lib_closure | ✅ | — |
| lib_linked_list | ✅ | — |
| **lib_hashmap** | ❌ | E0382: key 克隆 / entry API 重构 |
| **lib_iterator** | ❌ | E0599: 用户 trait 参数 + 迭代器 duck 约束 |
| **lib_string** | ❌ | 与 json.lz 联动：先定 `s[i]` 语义（i64 码点 vs 单字符 str） |
| **lib_tree** | ❌ | E0369: 运算符 / 字段类型对齐 |
| **lib_json** | ❌ | 独立专项：字符串索引 + Result 传播 + Dict 方法 + parse_f64 |

---

## 五、修复路线图

### 阶段 1：类型系统核心（P0）

```
BUG-TY-001 (duck → &dyn)
    ↓
BUG-IR-003 (闭包捕获)
    ↓
BUG-IR-001 (~: 闭包脱糖，依赖 IR-003)
```

**产出**：duck 自引用 + 嵌套函数闭包化 + 构建块参数位

### 阶段 2：库基线闭环（P1）

```
lib_hashmap (移动语义)
    ↓
lib_iterator (trait 参数)
    ↓
lib_tree (运算符)
    ↓
lib_string + json.lz (联动：先定字符串索引语义)
```

**产出**：12/12 库全绿 + json.lz 标杆用例

### 阶段 3：SYNTAX 文档实测覆盖

| 文档 | 实测用例 | 预期 |
|---|---|---|
| 10-并发与异步 | async/await + spawn | 待验证 |
| 13-指针与引用 | raw ptr / & / &mut | 待验证 |
| 14-生成器 | yield / generator | 待验证 |
| 15-测试框架 | assert 宏 | 待验证 |

### 阶段 4：自举路线 B 继续

- 当前 L1（LZ 前端组件已嵌入 Rust 编译器）
- 目标 L2（LZ 完整编译自身）— 等阶段 1-3 完成后启动

---

## 六、自举必要性再评估

**结论：不急做，阶段 1-3 优先**

| 维度 | 当前 | 自举收益 |
|---|---|---|
| 用户代码（.lz） | 编译器可编译全部生态 | 已达 L1 |
| 编译器自身（src/*.rs） | Rust 编写 | 重写成本极高 |
| 真实瓶颈 | 闭包/duck/标准库 | 自举不解决 |

**自举 = 背景任务**，每 100 小阶段评估一次，不占主力方向。

---

## 七、本轮已完成

- [x] 过时测试修复（ec002 / fold / compose / unique）
- [x] unique codegen bug（contains 参数缺 &）
- [x] lzcyc 子编译器管线下沉（postprocess_pyx）
- [x] serde_json feature gate 闭合
- [x] 420 unit + 12 integration 全绿

---

*下次推进：阶段 1 — BUG-TY-001 (duck → &dyn) 修复*
