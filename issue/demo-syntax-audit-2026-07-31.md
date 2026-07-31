# Open: DEMO 文件与语法文档交叉审计（2026-07-31）

**状态**: Open
**日期**: 2026-07-31
**范围**: DEMO/ 全部 .lz 文件 + SYNTAX/ 语法文档交叉核对，不涉及源代码
**审计方式**: 子代理全量通读 + 关键文件实测编译验证 + README 清单与文件系统对账

---

## 一、P0 — 声称「已实现」实则不可编译（有实测证据）

### 1. `99_spec/README.md` 声称 38/38 编译成功，但实测 `top_level_build.lz` 生成坏代码

**证据**:
- `DEMO/99_spec/README.md:2-4`：「状态：✅ 全部通过 (2026-07-31)」「**38/38 文件编译成功**，现已纳入 `tests/compile_demos.rs` 正面测试覆盖」
- `DEMO/99_spec/README.md:12`：`top_level_build.lz | 顶层构建块 x =: body | ✅`

**实测**：编译 `DEMO/99_spec/top_level_build.lz`
```lz
config =:
    host = "localhost"
    port = 8080
    (host, port)
def main() = print(config)
```
生成的 `top_level_build.rs` 只有 `fn main() { ... }`，**`config` 从未定义**——顶层构建块被静默丢弃（与 07-31 全项目找茬 P0-1 同一根因）。README 的「编译成功」与生成物直接矛盾。

**影响**: 声称已覆盖测试的规范目标案例实际产出不可编译的 Rust；README 状态体系不可信。

---

### 2. `99_spec/README.md` 文件清单与文件系统对不上（duck_demo.lz 指向不存在的位置）

**证据**:
- `DEMO/99_spec/README.md:44`：主清单列 `duck_demo.lz | duck 类型声明 | ✅`
- 实测：`ls DEMO/99_spec/duck_demo.lz` → **No such file or directory**；该文件实际位于 `DEMO/99_errors/duck_demo.lz`
- `ls DEMO/99_spec/*.lz | wc -l` = **31**（主清单声称 32 项 + combo 6 项 = 38，与「38/38」对不上，实际 37）

**影响**: 清单引用了不存在的路径，且数量自洽性被破坏。

---

### 3. 两份 README 对 99_spec 状态互相矛盾

- `DEMO/README.md:29`：「`99_spec/` | ... | 16 | 按规范撰写、**当前未实现**的特性目标案例（被 `tests/compile_demos.rs` 跳过）」
- `DEMO/99_spec/README.md:2-4`：「**38/38 文件编译成功**，现已纳入正面测试覆盖」

同一目录，一份说「未实现、被跳过」，一份说「全部实现、纳入测试」——DEMO 顶层 README 与 99_spec 自带的 README 直接打架。

---

## 二、P1 — 99_spec 文件内容错误（编译必失败或违反文档）

### 4. 推导式示例引用未定义变量

- `DEMO/99_spec/dict_comprehension.lz:5`：`let by_id = {u.id: u.name for u in users}` — **`users` 未定义**
- `DEMO/99_spec/set_comprehension.lz:4`：`let uniq = {x for x in numbers}` — **`numbers` 未定义**
- `DEMO/99_spec/set_comprehension.lz:5`：`let vowels = {c for c in text if c in "aeiou"}` — **`text` 未定义**

文件头均标「✅ 已实现」，但示例本身无法编译。

### 5. `combo_while_guard_walrus.lz` 违反 let 不可变语义 + 跨函数引用

- `DEMO/99_spec/combo-syntax/combo_while_guard_walrus.lz:10`：`let total = 0` 后 L11 `total = total + n` — **let 不可变绑定后重新赋值**，违反 SYNTAX/02 §3.1「let 不可重赋值」
- `combo_while_guard_walrus.lz:6-8`：`def next()` 内引用 `count`，但 `count` 定义在 `main()` 内（L14）— **函数外不可见**，作用域错误
- 文件头注释「当前解析器未实现：while 分支不识别 if 守卫」与 README 标 ✅ 矛盾（同 #1 状态问题）

### 6. combo-syntax 出处引用错误的章节号

- `combo_while_guard_else.lz:3`：「出处：SYNTAX/05-控制流.md §3.4（while）+ §3.6（else）」— **§3.6 不存在**（05-控制流.md 中 while 是 §四、else 是 §4.2，§3.x 全是 for 循环）
- `combo_while_guard_walrus.lz:3`：「出处：SYNTAX/05-控制流.md §3.4/§3.5」— §3.4 是「声明式 for：sum/prod」，与 while 无关；§3.5 才是守卫

### 7. `extractor_unapply.lz` 使用位置构造，违反文档

- `DEMO/99_spec/extractor_unapply.lz:9`：`let p = Point(3, 4)` — 位置构造
- SYNTAX/06a-struct.md §4.1：struct 实例必须**关键字参数**构造 `Point(x: 3, y: 4)`
- 文档未定义位置构造；示例与规范矛盾

### 8. `duck_test.lz` 使用文档不存在的匿名 inline duck 语法

- `DEMO/99_spec/duck_test.lz:5`：`def process(p: duck { name: str, age: int }) = ...` — 匿名 `duck {...}` 内联语法
- SYNTAX/01b-duck关系约束.md 只定义 `duck Name =` 具名定义形式，无匿名内联形式

---

## 三、P1 — 99_errors 文件问题

### 9. `duck_demo.lz` 目录定位与自述矛盾 + 构造字段名不匹配

- `DEMO/99_errors/duck_demo.lz:3`：「当前状态：99_spec（编译器未实现，被 parse 测试跳过）」— **文件放在 99_errors/，注释自称属于 99_spec**
- `DEMO/99_errors/duck_demo.lz:226`：`let ct = Counter(value: 10, count: 0)` — 构造字段名 `count` 与 struct 定义 `._count` 不匹配（应为 `_count`，见同一文件的 `._count` 私有字段约定）

### 10. `02_variable_errors.lz` 注释与文档矛盾

- `DEMO/99_errors/02_variable_errors.lz:14`：「// let owned s: str = "hi"  // ❌ owned 只能修饰形参」
- SYNTAX/02 §六：**解析器忽略 owned**，实际不报错而退化为 `let s`
- 注释声称的错误边界与实际行为不符（错误用例不再是错误）

### 11. `duck_demo.lz` 多处异体字/错字

`DEMO/99_errors/duck_demo.lz:191,194,199,200,241`：`滿足`（应为 满足）、`演示`（应为 演示）、`签名`（应为 签名）——异体字混入（「滿」「演」「签」为异体/错字），影响检索与一致性。

---

## 四、P2 — 语法文档补充发现（与 DEMO 交叉核对引出）

### 12. `SYNTAX/overview/缺失语法特性报告.md:103` 文档自身示例用错语法

```lz
struct Point:      // ❌ 文档自己的示例用了冒号
```
SYNTAX/06a-struct.md §七明确 `struct Bad:` 是错误语法（必须 `=`）——规范文档自身的示例违反自身规范。

### 13. SYNTAX 文档内部命名混用（Lang-Zone / Lang-Zong）

- `SYNTAX/00-词法基础.md:5` 使用 **Lang-Zone**
- `SYNTAX/06a-struct.md:3,5` 使用 **Lang-Zong**

同一规范目录内两个名字并存（07-31 全项目找茬已报过全局命名问题，此为 SYNTAX 内部的具体分布证据）。

### 14. `DEMO/README.md` 计数全面过时

- L29：99_spec 记为 **16** 个文件，实际 31 主 + 6 combo = 37
- L31：自称「文件数 2026-07-29 刷新」，但目录内容已 07-31 变化
- L138：「合计 49 ❌ 边界全覆盖」，与 99_errors 实际 15 个文件、99_spec 存在未编译文件的现状不匹配

---

## 五、修复建议（按优先级）

1. **P0**：以实测为准重写 `99_spec/README.md`——去掉「38/38 编译成功」声明；修复或移除 `top_level_build.lz`（顶层构建块要么真正实现要么标为未实现）；清单与实际文件系统对账
2. **P0**：`DEMO/README.md` 与 `99_spec/README.md` 的 99_spec 状态描述统一
3. **P1**：修正 99_spec 内未定义变量（#4）、let 重赋值与跨函数引用（#5）、构造字段名（#9）
4. **P1**：修正 combo-syntax 出处章节引用（#6），指向真实章节号
5. **P1**：`duck_test.lz` 匿名 duck 语法、`extractor_unapply.lz` 位置构造——要么与文档对齐，要么在文档中补充定义
6. **P2**：`缺失语法特性报告.md:103` 的 `struct Point:` 改 `=`
7. **P2**：异体字清理（#11）、SYNTAX 内部命名统一（#13）、DEMO README 计数刷新（#14）

## 六、验收标准（修正后核对）

1. `99_spec/README.md` 清单与 `ls DEMO/99_spec/*.lz` 完全一致，无指向不存在路径的条目
2. 99_spec 每个标 ✅ 的文件实测编译产物可用（不引用未定义变量）
3. 两份 README 对 99_spec 的状态描述一致
4. combo-syntax 出处引用章节号在 05-控制流.md 中真实存在
5. 99_errors/duck_demo.lz 目录定位、自述状态、字段名与文档一致
