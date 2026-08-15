# 自举试点 1：用 LZ 翻译 LZIR 类型系统 — 成果 + 暴露的 codegen 缺陷

> 日期：2026-08-15
> 状态：✅ 试点完成；🚩 暴露 2 个 LZ codegen 缺陷（待修复）
> 试点文件：`bootstrap/work/lz_ir/`（该目录被 .gitignore 忽略，属本地工作区；本 issue 为入库记录）

---

## 一、试点成果

**目标**：验证「LZ 能否表达编译器核心数据结构（递归 enum）+ 序列化逻辑」，作为路线 B（编译器自托管）的第一步试点。

- ✅ **递归 enum 可用**：LZ 的 `enum IrType: Named(path: str, args: List<IrType>) ...` 自引用类型经 LZ→Rust 生成 `Box<Vec<IrType>>`，编译运行正确。
- ✅ **display 序列化对齐**：7 个典型类型输出与 Rust 版 `src/ir/display.rs` 逐字符一致（`Vec<int>` / `Option<Vec<int>>` / `fn(int, str) -> bool` / `(int, f64)` / `Result<str, int>` / `&mut Vec<int>` / `T`）。
- ✅ 验证链路：`.lz → lang-zone（IR codegen）→ .rs → rustc → 运行`，符合验证铁律。

## 二、暴露的 LZ codegen 缺陷（需修复）

### 缺陷 1：`List + List` 拼接生成非法 `Vec + Vec`（E0369）

- **触发**：LZ 源码 `out = out + [ts[i]]` → codegen 生成 `out + vec![ts[i]]`，`Vec` 不实现 `Add` → `E0369`。
- **影响**：所有 `list + [x]` 拼接的 LZ 代码。
- **修法**：`src/ir/codegen.rs` `binop_str` 对 `Add` 且操作数为 `Vec/List` 时特判，生成 `extend`/`chain` 语义。

### 缺陷 2：索引元素按值传参未自动 `.clone()`（E0507）

- **触发**：LZ 源码 `display_type(ts[0])` → codegen 生成 `display_type(ts[0])`，`IrType` 非 Copy，索引取出 move → `E0507`。
- **影响**：所有 `f(list[i])` 形式（非 Copy 元素类型）。
- **修法**：`src/ir/codegen.rs` 实参 clone 注入逻辑（约 4976 行）扩展到 `IndexGet` 实参。

> 试点中暂用 LZ 侧 `.clone()` / `.push()` 规避；缺陷本身是编译器问题，应修在 codegen 而非要求用户写 clone。

## 三、LZ 使用要点（翻译参考）

| 语法点 | 正确写法 | 常见错误 |
|:-------|:---------|:---------|
| enum 变体字段 | `Named(path: str, args: List<IrType>)` | ❌ 花括号不被解析 |
| 三元表达式 | `"Int" if a.len() > 0 else "?"` | ❌ 无 `then` 关键字 |
| 可变绑定 | `let mut out = ""` | ❌ 默认不可变（E0384） |
| 空列表类型标注 | `let mut out: List<IrType> = []` | ❌ 空列表无法推断（E0282） |
| 列表推导 | `[x for x in xs]` | ❌ 无 lambda 推导 |

## 四、下一步

1. 修复缺陷 1/2（codegen 层），`ir_types.lz` 去掉规避写法还原自然代码
2. 扩展翻译 `IrModule` + `Item::FnDef/ConstDef` + `Stmt/Expr` 子集，对齐 `--emit=ir` 全量输出
3. 建立 Rust 版 `--emit=ir` vs LZ 版 IR 的 diff 对照回归
4. 路线 B 推进：lexer/parser 逐步 LZ 化，形成「LZ 前端 + Rust IR codegen」混合管线
