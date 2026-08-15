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

