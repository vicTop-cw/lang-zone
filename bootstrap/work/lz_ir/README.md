# 自举试点 1：用 LZ 翻译 LZIR 类型系统（bootstrap/work/lz_ir/）

> 日期：2026-08-15
> 目标：验证「LZ 能否表达编译器核心数据结构（递归 enum）+ 序列化逻辑」，作为路线 B（编译器自托管）的第一步试点。
> 对应：[04-当前项目自举具体方案.md](../../04-当前项目自举具体方案.md) 路线 B。

---

## 一、试点成果 ✅

### 1. LZ 成功表达 LZIR 类型系统（IrType 16 变体，递归自引用）

`ir_types.lz` 用 LZ enum 完整定义了 `src/ir/types.rs` 的 `IrType`（16 个变体）：

```lz
enum IrType:
    Int
    F64
    Str
    Bool
    Unit
    Never
    Any
    Self_
    Named(path: str, args: List<IrType>)   // 递归自引用
    Option(inner: IrType)                   // 递归自引用
    Result(ok: IrType, err: IrType)
    Tuple(elems: List<IrType>)
    FnType(params: List<IrType>, ret: IrType)
    Ref(inner: IrType)
    MutRef(inner: IrType)
    Generic(name: str)
```

**关键验证点：递归 enum 可用**——`Named(path: str, args: List<IrType>)` 自引用类型经 LZ → Rust 生成 `Box<Vec<IrType>>`，编译运行正确（`Option<Vec<Int>>` 嵌套构造输出正确）。

### 2. LZ 成功实现 display 序列化，输出与 Rust 版逐字符一致

`display_type` / `display_list` 递归渲染，与 `src/ir/display.rs` 格式完全对齐：

| LZ 版输出 | Rust 版期望 | 一致 |
|:----------|:------------|:----:|
| `Vec<int>` | `Named("Vec", [Int])` | ✅ |
| `Option<Vec<int>>` | `Option(Named("Vec", [Int]))` | ✅ |
| `fn(int, str) -> bool` | `Fn([Int, Str], Bool)` | ✅ |
| `(int, f64)` | `Tuple([Int, F64])` | ✅ |
| `Result<str, int>` | `Result(Str, Int)` | ✅ |
| `&mut Vec<int>` | `MutRef(Named("Vec", [Int]))` | ✅ |
| `T` | `Generic("T")` | ✅ |

验证链路：`ir_types.lz → lang-zone（IR codegen）→ ir_types.rs → rustc → 运行`，7 个类型全部输出正确。

---

## 二、试点暴露的 LZ 编译器差距（真实缺陷，需修复）🚩

试点过程中 rustc 报错暴露了 LZ codegen 的 2 个值语义缺陷：

### 缺陷 A：`List + List` 拼接生成非法 `Vec + Vec`（E0369）

- **现象**：LZ 源码 `out = out + [ts[i]]`（List 拼接）→ codegen 生成 `out + vec![ts[i]]`，但 `Vec` 不实现 `Add` → `E0369: cannot add Vec<IrType> to Vec<IrType>`。
- **影响面**：所有用 `list + [x]` 拼接的 LZ 代码。
- **修法方向**：`BinOpKind::Add` 对 `Vec/List` 操作数应生成 `extend`/`chain` 语义（如 `({ let mut v = lhs; v.extend(rhs); v })`）。
- **位置**：`src/ir/codegen.rs` `binop_str`（Add → `"+"`）需对 List 类型特判。

### 缺陷 B：索引元素按值传参未自动 clone（E0507）

- **现象**：LZ 源码 `display_type(ts[0])` → codegen 生成 `display_type(ts[0])`，但 `IrType` 非 Copy，从 `Vec` 索引取出会 move → `E0507: cannot move out of index of Vec<IrType>`。
- **影响面**：所有 `f(list[i])` 形式的索引传参（非 Copy 元素类型）。
- **修法方向**：`ExprKind::Call` 的实参若为 `IndexGet` 且元素类型非 Copy，自动 `.clone()`（与现有「变量实参自动 clone」逻辑对齐）。
- **位置**：`src/ir/codegen.rs` 实参 clone 注入逻辑（约 4976 行附近）。

> 试点中暂用 LZ 侧 `.clone()` / `.push()` 规避（`ir_types.lz` 已标注），缺陷本身是编译器问题，应修在 codegen 而非要求用户写 clone。

---

## 三、LZ 使用要点（试点记录，供后续翻译参考）

| 语法点 | 正确写法 | 常见错误 |
|:-------|:---------|:---------|
| enum 变体字段 | `Named(path: str, args: List<IrType>)` | ❌ `Named { path: str }`（花括号不被解析） |
| 三元表达式 | `"Int" if a.len() > 0 else "?"` | ❌ `if cond then a else b`（无 `then` 关键字） |
| 可变绑定 | `let mut out = ""` | ❌ `let out = ""`（默认不可变） |
| 空列表类型标注 | `let mut out: List<IrType> = []` | ❌ `let mut out = []`（E0282 无法推断） |
| 列表推导 | `[x for x in xs]` | ❌ `[x -> f(x) for x in xs]`（无 lambda 推导） |

---

## 四、下一步

1. **修复缺陷 A/B（codegen 层）**：修完后 `ir_types.lz` 可去掉 `.clone()`/`.push()` 规避写法，还原为自然 LZ 代码。
2. **扩展翻译范围**：`IrModule` + `Item::FnDef`/`ConstDef` 核心 + `Stmt`/`Expr` 子集 + `display.rs` 模块级输出（`;; LZIR v1` 头 + items 列表），对齐 `--emit=ir` 全量输出。
3. **建立对照测试**：同一 .lz 输入，Rust 版 `--emit=ir` 输出 vs LZ 版 IR display 输出，做 diff 回归。
4. **推进路线 B**：词法/语法层（lexer/parser）也可逐步用 LZ 重写，最终形成「LZ 写的编译器前端 → 现有 Rust IR codegen」的混合管线。

---

## 五、文件清单

| 文件 | 说明 |
|:-----|:-----|
| `p0_recursive_enum.lz` | 可行性验证：递归 enum 自引用（`Option<Vec<Int>>`） |
| `ir_types.lz` / `ir_types.rs` | LZ 版 IrType 定义 + display（当前成果） |
| `ir_module.lz` / `ir_module.rs` | LZ 版 IrModule/Item/Stmt/Expr 核心子集 + display（试点 2） |
| `ir_compare.lz` | 对照输入（与 ir_module.lz main 构造的 IR 等价） |
| `README.md` | 本试点记录 |

---

## 六、试点 2：IR 模块 display 对照（2026-08-15）

**目标**：用 LZ 实现 IR 模块 display，与 Rust 版 `--emit=ir` 输出对照，验证 LZ 能序列化完整 IR 结构。

**对照方式**：同一输入 `ir_compare.lz`（def add + const LIMIT + def main），
Rust 版 `lang-zone.exe --emit=ir` vs LZ 版 `ir_module_lz.exe`。

**对照结论（diff 验证）**：

| 结构 | Rust 版 | LZ 版 | 一致 |
|:-----|:--------|:------|:----:|
| 模块头 | `;; LZIR v1 — module 'main'` + `;; N items` + `;; prelude: ...` | 同格式（demo 模块） | ✅ |
| FnDef 签名 | `fn add(x: int, y: int) -> int:` | 同格式 | ✅ |
| Const | `const LIMIT: int = 100_i64` | 同格式 | ✅ |
| Let 绑定 | `let xs: List<int> = [...]` | 同格式 | ✅ |
| BinOp | `binop [int] x + [int] y` | `binop x + y` | 🟡 见下 |
| Call/Index | `call print(index xs[0_i64])` | 同格式（无 ty 前缀） | 🟡 见下 |

**已知差异（设计取舍，非缺陷）**：
1. **Expr 省略 `[ty]` 前缀**：Rust display.rs 每个 Expr 带 `[ty] ` 前缀（`[int] binop ...`），
   LZ 版核心子集省略（ty 由调用方管理）——若需逐字符对齐，可在 LZ 版为
   Expr 增加 ty 字段后补前缀。
2. **FnDef body 用块包裹**：Rust 版 body 每行 `  stmt` 缩进（无外层 `{ }`），
   LZ 版 `stmt_block` 用 `{ ... }` 包裹——对齐 Rust 需改 display_item 的 body 渲染。

**结论**：LZ 已能序列化 IR 模块的核心结构（头/函数/常量/语句/表达式），
格式与 Rust 版整体对齐；上述两处差异为实现取舍，后续可逐字符对齐。

**附带收获**：试点暴露并修复了 1 个 codegen 真实缺陷——
`collect_stmt_var_refs` 的 `Stmt::For` 分支未把 for 变量加入 shadow，
多函数同名 for 变量（`for idx`）被 analyze_global_vars 误判为跨函数全局
（生成 `static mut idx`，E0530）——已在 codegen 修复（v163 随附）。
