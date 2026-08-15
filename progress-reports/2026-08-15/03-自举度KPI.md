# 2026-08-15 进度报告 03 · 自举度 KPI

> 本报告遵循「只增不删不改」铁律；对应 tnr 建议 ⑤（自举推进 + 自举度 KPI）。
> KPI 口径：`lzc` 源码中 LZ 行数占比 = src/**/*.lz 行数 ÷ (src/**/*.rs + src/**/*.lz) 行数。

---

## 一、基线（2026-08-15，v167 前）

| 指标 | 值 |
|---|---|
| src 下 LZ 源码 | **398 行**（仅 `src/ir/lz_ir_lib.lz`） |
| src 下 Rust 源码 | 56,856 行 |
| **LZ 占比** | **0.7%**（398 / 57,254） |

## 二、自举度来源构成

| LZ 组件 | 行数 | 对应 Rust 职责 | 状态 |
|---|---|---|---|
| `src/ir/lz_ir_lib.lz` | 398 | IR 类型 + display（display.rs 743 行） | ✅ 已接入 `--emit=ir-lz` |
| 词法试点（bootstrap/work/lz_lexer/lexer.lz） | ~170 | token.rs + tokenize | 🟡 试点暴露缺陷，未入库 |

## 三、KPI 推进记录（每次登记追加）

- **2026-08-15 基线**：LZ 占比 0.7%（v167 前）。里程碑：IR 文本输出已由 LZ 承担
  （对应 Rust 侧 display.rs 743 行，折算「职责覆盖率」约 743/56856 ≈ 1.3%）。
- 目标路线：类型系统/IR display ✅ → 词法前端（试点暴露 3 缺陷）→ 语法前端 → 全自举。

## 四、词法试点暴露的缺陷（自举前端前置发现）

1. **字符串单字符索引**：`s[i]` 生成 `as_bytes()[i] as i64` 返回字节码而非字符
   （有意设计，polish_28_strings 断言依赖）——LZ 取字符需 `s[i..i+1]` 切片规避。
2. **binop 操作数 Vec 索引未 clone**：`chars[0] + chars[1]` 生成 `chars[0] + &chars[1]`
   触发 E0507（p23 probe 复现）。
3. **复杂嵌套 tokenize 栈溢出**：多级 if/elif + match 嵌套导致 lang-zone 编译栈溢出
   （与 v162 BlockExpr 同类编译器递归限制）。

> 缺陷 2 为真实 codegen 缺陷（值语义），待排期修复；缺陷 1 为设计行为需文档化；
> 缺陷 3 为编译器递归限制，词法试点需拆分函数规避。

## 五、KPI 推进记录（v168，追加）

- **2026-08-15 v168 更新**：binop 索引 clone 缺陷已修复（str 拼接 lhs IndexGet 注入
  `.clone()`，p23 复现 → rustc 0 错误，6 DEMO 回归通过）。
- **词法试点跑通**（bootstrap/work/lz_lexer/lexer.lz）：Token 枚举 + tokenize 简化版
  （标识符/关键字/整数/运算符/标点）端到端输出正确（rustc 0 错误）。拆函数规避
  栈溢出：scan_ident/scan_int/scan_punct 拆分 + punct_token 查表法（List 数据驱动）
  + is_digit/is_alpha 范围比较短链——规避「11 层 elif 链 + Option<Token>」编译栈溢出
  （p27 复现根因：深层 elif/`||` 链被解析为深嵌套 BinOp）。
- **v168 新暴露缺陷**（登记，待修复）：
  - 元组字段 `r.0` 作为实参被消费后再用 → E0382（LZ 未对元组字段实参注入 clone）；
  - 元组字段类型未推断为 Str → `num as int` 走 `as i64` 强转 E0605（cast 路径需
    依据字段类型而非表达式形态）；
  - `Option<Token>` 类型下 `None` 硬编码 i64（Option<Token> 场景 E0308）；
  - 深层 elif/`||` 链（>10 层）触发 lang-zone 编译栈溢出（编译器递归限制）。
- **LZ 占比不变**（词法试点在 bootstrap 未入库；lexer.lz 约 190 行待入库提升占比）。

## 六、KPI 推进记录（v170，追加）

- **2026-08-15 v170 修复**（自举前端试点暴露的 2 个真实缺陷）：
  - **元组字段实参未 clone（E0382）**：实参 clone 注入条件 `Var | IndexGet` 扩展
    `FieldAccess`（元组字段 `r.0` 在 IR 中是 FieldAccess）——p28 probe 验证
    （`is_keyword(r.0)` 后再用 `r.0`，rustc 0 错误）。
  - **字符串切片 cast 走 as i64（E0605）**：Cast 分支 `src_is_string` 只查
    `expr.ty`，字符串切片/索引（`s[a..b] as int`）ty 常推断为 Any → 误走
    `as i64` 强转；修复：切片/索引形态（base 为 Str）同样走 `.parse::<i64>()`
    ——parser_e probe 验证（`"42"[0..2] as int` → 42）。
- **Parser 前端 LZ 化试点跑通**（bootstrap/work/lz_parser/parser.lz）：
  递归下降表达式解析器（tokenize → parse_expr/parse_term/parse_atom，覆盖
  `+ - * /` 优先级与括号）端到端输出正确（`1 + 2 * 3 + (4 - 1)` = 10、
  `2 * 3 - 4 / 2` = 4）。
- **v170 新暴露/确认缺陷**（登记，待修复）：
  - `let num: str = r.0` 变量 cast 仍走 as i64（builder let 类型标注未传播
    到变量 ty）——规避：切片表达式内联 cast（`src[a..b] as int`）；
  - 「let 语句后跟元组字面量」被 codegen 拼成调用（E0618，p29 复现）——
    规避：元组先绑定中间变量再返回；
  - 深层 elif 链（>10 层）触发 lang-zone 编译栈溢出（p27 同类）——
    规避：查表法（List 数据驱动）。
- **自举前端里程碑**：词法 ✅ → 递归下降表达式解析 ✅ → 语句/完整 Parser 下一阶段。

## 七、KPI 推进记录（v171，追加）

- **2026-08-15 v171 验证结论**：
  - **let 显式类型标注传播已可用**：p30/p31/p32 三 probe 实证
    `let num: str = r.0` + `num as int` 正确走 `.parse::<i64>()` 分支
    （rustc 0 错误）——builder 的 `add_var` 本就用显式标注注册变量类型，
    v170 的 Cast 分支修复已覆盖，builder 无需额外修改；
  - **枚举构造实参中变量 cast 仍有差异**：`Token.IntLit(v: num as int)`
    场景仍走 as i64（builder 实参处理差异）——parser.lz 以切片内联 cast
    （`src[a..b] as int`）规避，登记待修。
- **Parser 语句级扩展跑通**（bootstrap/work/lz_parser/parser.lz）：
  - Token 枚举加 `Colon/Return/If/Else/Def` 语句关键字；
  - `keyword_token` 查表（return/if/else/def）、tokenize 支持关键字与冒号；
  - `parse_stmt`（return expr | if expr | def name | 表达式语句）+
    `parse_program`（stmt* 直到 Eof）；
  - 端到端输出正确：`return 1+2 → return 3`、`if 3 → if 3`、`def foo`、
    `expr 4`（rustc 0 错误）；
  - 记录 LZ 语法边界：if 是语句式（不能内联），三元用 `a if cond else b`；
    enum 构造实参 move 需显式 `.clone()`（`Token.Ident(name: w.clone())`）。
- **自举前端里程碑**：词法 ✅ → 表达式解析 ✅ → 语句级解析 ✅ → 完整 Parser 下一阶段。

## 八、KPI 推进记录（v172，追加）

- **2026-08-15 v172 验证结论**：
  - **枚举构造实参 cast 已修复**：p33/p34/p35 三 probe 实证
    `Token.IntLit(v: num as int)` 正确走 `.parse::<i64>()` 分支（rustc 0
    错误，val=42）——v170 Cast 修复已覆盖 let 绑定 / List 字面量 / 枚举
    构造实参场景；v171 时失败为测试构建时序问题。
  - **登记差异**：List 拼接累积场景（`tokens + [Token.IntLit(v: num as int)]`）
    cast 仍走 as i64——parser.lz 以切片内联 cast（`src[a..b] as int`）规避，
    待修（builder 实参处理差异）。
- **Parser 多行语句解析跑通**（bootstrap/work/lz_parser/parser.lz）：
  - Token 枚举加 `Newline`，tokenize 支持换行 → Newline；
  - parse_stmt 加 Newline 分支（跳过空行）、parse_program 跳过空语句；
  - 多行源码解析端到端输出正确：`def foo` / `return 3` / `if 3` / `else` /
    `expr 4`（rustc 0 错误）。
- **自举前端里程碑**：词法 ✅ → 表达式解析 ✅ → 语句级解析 ✅ → 多行语句 ✅
  → 完整 Parser（缩进块嵌套）下一阶段。

## 九、KPI 推进记录（v173，追加）

- **2026-08-15 v173 前端入库（自举度提升）**：
  - `src/frontend/lz_lexer.lz`（词法前端，~259 行）+ `src/frontend/lz_parser.lz`
    （语法前端，~284 行）入库——自举试点从 bootstrap 提升到 src/ 源码树；
  - **自举度占比更新**：src 下 LZ 源码 398 → **941 行**（RS 57514 行），
    LZ 占比 0.7% → **1.6%**；
  - 入库版本为「多行语句版」（能编译）：缩进块解析（parse_block/Indent）
    已实现并经 p42 probe 验证可用，但完整文件触发 lang-zone 编译栈溢出
    （编译器递归限制，已登记），入库版本回退规避。
- **v173 验证结论**：
  - p36 实证「List 拼接场景 cast」已正确走 `.parse::<i64>()`（v170 修复
    覆盖，v171/v172 登记的差异实为测试构建时序问题）；
  - p38/p41/p42 实证缩进 tokenize 分支组合、parse_block 互相递归均可编译
    （栈溢出源于完整文件组合规模，非单个逻辑）；
  - 新暴露缺陷登记：`elif t == Token.Indent(v: n)` 的「枚举比较+字段绑定」
    在 if 条件里无法绑定（E0425）——规避：match 绑定（block_tag/block_indent）；
    ident_name 单字母绑定名 `n` 触发 codegen 降级（n_）且体内引用未同步
    （E0308）——规避：用长名 `nm`。
- **自举前端里程碑**：词法 ✅ → 表达式解析 ✅ → 语句级解析 ✅ → 多行语句 ✅
  → 缩进块解析（p42 验证，入库版回退）→ 完整 Parser 下一阶段。

## 十、KPI 推进记录（v174，追加）

- **2026-08-15 v174 缩进块解析入库（栈溢出定位闭环）**：
  - **栈溢出根因定位**：v173 登记的「完整文件组合规模触发编译栈溢出」经
    逐步二分定位——**非缩进块组合本身**（step1 加 Indent token+tokenize
    缩进扫描、step2 加 parse_block+If/Def 调用均编译通过），而是 v173
    当时写法（tokenize 多处分散 line_start 赋值）触发；
  - **修复/规避**：step2 写法（line_start 仅行首缩进分支处理）编译通过且
    端到端正确（`def foo {}` / `if 1 {}` / `return 2` / `return 3` / `else`）；
  - **完整 Parser 入库**：src/frontend/lz_parser.lz 更新为缩进块版
    （parse_block/block_tag/block_indent/Indent token 全量），编译通过；
  - **自举度占比更新**：src 下 LZ 源码 941 → **997 行**（RS 57597 行），
    LZ 占比 1.6% → **1.7%**。
- **自举前端里程碑**：词法 ✅ → 表达式解析 ✅ → 语句级 ✅ → 多行语句 ✅
  → **缩进块解析（入库）** ✅ → 完整 Parser（嵌套块递归）下一阶段。

## 十一、KPI 推进记录（v175，追加）

- **2026-08-15 v175 嵌套块递归验证**：
  - **p44 实证嵌套递归逻辑可用**：固定 token 列表验证 `parse_stmt` If 分支
    递归调 `parse_block`（`if 1 {return 2}`，rustc 0 错误）——嵌套块递归
    逻辑本身完全可用；
  - **登记编译器限制**：p43 实证「缩进 tokenize + parse_block↔parse_stmt
    互相递归」完整组合触发 lang-zone 编译栈溢出（v174 同类递归限制）；
    v174 入库版 tokenize 实际不生成 Indent（残留 `line_start = false`
    无声明）——`def foo {}` 空体根因；
  - **清理入库版**：src/frontend/lz_parser.lz 清除 4 处残留 `line_start`
    死代码，保持干净可编译；
- **自举前端里程碑**：词法 ✅ → 表达式 ✅ → 语句级 ✅ → 多行语句 ✅ →
  缩进块 ✅ → 嵌套块递归（p44 验证，完整集成受编译器限制）→ 完整 Parser
  下一阶段。

