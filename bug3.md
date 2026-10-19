# BUG-3：`DEMO/lz_std/dict.lz` 基线失败（三个子问题）

> **命名澄清**：本文与 FIST 账本 `memory/bugs.md` 里的 BUG-3（BigInt const 的 E0015）
> **不是同一个 bug**，仅为 dict.lz 攻坚记录，避免上下文丢失后重复勘察。

状态（2026-09-27 更新）：
- **①②③ 已修复**：dyn E0038/E0782（③）、impl 头与方法 `K2` 的 `Ord`（①②）。
  dict.lz 从「3 个 Ord/dyn 错」降为「`str`/`String` 不一致」的深层次问题（见 ④）。
- **④ 新暴露（深层设计问题，未修）**：`Dict<str, int>` 的 `str` 键与字符串字面量/`ref str`
  的 `&str` 不一致，导致 E0308/E0277/E0631。非 Ord/dyn 范畴，需单独决策（见下）。

进度留痕：

| 子问题 | 状态 |
|---|---|
| ① `impl<K: ...>` 缺 `K: Ord` | **已修复** |
| ② `fn map<K2: ...>` 缺 `K2: Ord` | **已修复** |
| ③ `<dyn DictExt<_, _>>::lz_keys` | **已修复** |
| ④ `str`/`String` 不一致（Dict<str> 专用） | 开放（深层设计） |

另：本轮顺带发现并修复了一处**由 BUG-9 算术 clone 引入的回归**（`list.lz` E0614），见文末。

复现：

```bash
python .fist-wf-20260926/check.py DEMO/lz_std/dict.lz
```

当前三项报错：

```
error[E0277]: the trait bound `K: Ord` is not satisfied      (dict.rs:168)
error[E0277]: the trait bound `K2: Ord` is not satisfied     (dict.rs:242)
error[E0038]: the trait `DictExt` is not dyn compatible      (dict.rs:296)
```

三项互相独立，但 **dict.lz 要转绿必须三项全修**（只修前两项仍会卡在 E0038）。

---

## 子问题 ① ②：行内泛型约束缺 `Ord`

### 现象

```rust
// ① dict.rs:100 —— impl 头部的 K 没有 Ord，但体里 self.insert() 需要
impl<K: std::clone::Clone + std::fmt::Debug, V: std::clone::Clone + std::fmt::Debug>
    DictExt<K, V> for BTreeMap<K, V> {
        fn ... { self.insert(key, value); }   // ← BTreeMap::insert 要求 K: Ord
    }

// ② dict.rs:230 —— 方法自带泛型 K2 没有 Ord，但返回 BTreeMap<K2, V2>
fn map<K2: std::cmp::Eq + std::hash::Hash + Debug + Clone, V2: Debug + Clone>(&self, ...)
    -> BTreeMap<K2, V2> {
        result.insert(new_k, new_v);          // ← 要求 K2: Ord
    }
```

### 根因

`src/ir/codegen/mod.rs` 里已有一套「LZ `Dict` → `BTreeMap<K, V>`，`K` 需 `Ord`」的自动补约束机制
（`dict_using_fns` 集合 + 三处注入点，约 3432 / 4707 / 5067 行）。

**但这套机制只改写 `where` 子句**，对行内泛型约束（`impl<K: ...>` 的方法头、`<K2: ...>`）完全无效：

```rust
// 只对「已有 Hash/Eq 约束且缺 Ord」的 where 子句追加
if body_uses_dict && bounds.contains(Hash|Eq) && !bounds.contains(Ord) {
    bounds.push(Ord);
}
```

① 的 `K` 出现在 impl 头（不是 where 子句），② 的 `K2` 是方法自带泛型（也没有对应 where 子句），
于是两者都漏掉了。

### 解决方案

判定条件从「where 子句里的 Hash/Eq」扩展为**「该泛型形参是否充当 `BTreeMap` / `Dict` 的键」**：

1. 让 `compute_cmp_constraints`（第 6926 行）在收集时**带上签名**。
   目前它只收集 `(name, generics, body)`：

   ```rust
   let mut fns: Vec<(String, Vec<String>, Block)> = Vec::new();
   ```

   需增加 params 类型与 ret 类型（三处 `fns.push` 都要补：`Item::FnDef` / `Item::Impl` /
   `Item::TraitDef`）。

2. 新增扫描：在签名类型里找 `Named { path: "BTreeMap" | "Dict" | "HashMap", args }`，
   取 `args[0]`；若它是 `IrType::Generic(name)`，把 `name` 并入该函数的 **`cmp_ord`**。

3. `gen_fn_generics`（第 6853 行）原本就会给 `cmp_ord` 里的泛型注入
   `PartialEq + Eq + Ord`，无需改动，①② 自动消失。

   - ① 走 impl 端：需确认 impl 头泛型也经 `gen_fn_generics`；若不走，则在 impl 泛型生成处
     对 self type `BTreeMap<K, V>` 的 `K` 直接追加 `Ord`。
   - ② 走方法端：`map` 的 `K2` 由返回类型 `BTreeMap<K2, V2>` 命中。

### 风险

- **只给「键」加 `Ord`，不要给 `V` 加**。`BTreeMap<K, V>` 的 `V` 不需要 `Ord`，误加会让
  传非 `Ord` 值类型的调用方编译失败。
- trait 声明端（约 4699 / 5067 行）与 impl 端（约 5005 行）**必须两端一致**补齐，
  否则 E0276 `impl has stricter requirements than trait`。现有 `dict_using_fns`
  就是为这个一致性存在的，扩展时保持共用同一判定源。
- 改完需回归 `DEMO/lz_std/list.lz`（同类机制此前修好的用例，不能回退）。

---

## 子问题 ③：`<dyn DictExt<_, _>>::lz_keys(self,)` —— trait 非 dyn 兼容

### 现象

```rust
// dict.rs:296
fn iter(&self) -> DictIter<K, V> {
    return DictIter { dict: self.clone(),
                      keys: <dyn DictExt<_, _>>::lz_keys(self, ),   // ← E0038
                      index: 0i64 };
}
```

```
error[E0038]: the trait `DictExt` is not dyn compatible
note: for a trait to be dyn compatible it needs to allow building a vtable
```

### 根因

codegen 把「trait 静态方法调用」渲染成了**经由 dyn trait 对象**的形式
`<dyn Trait<_, _>>::method(recv, ...)`。

而 `DictExt` 含泛型方法（如 `map<K2, V2>`）和/或 `impl Fn` 形参，
**不满足 dyn 兼容（object safety）**，因此根本无法通过 vtable 调用。

这里并不需要 dyn：调用点位于 `impl DictExt for BTreeMap` **内部**，`Self` 是具体类型
`BTreeMap<K, V>`，完全可以静态派发。

### 已实施的修复（2026-09-27）

改动点在 `codegen/mod.rs` 约 12451 行的 DictExt 强制调用分支，共两处：

1. `<dyn DictExt<_, _>>::` → `DictExt::`（静态派发）。
   旁证：紧邻的 `SetExt` 分支（约 12478 行）本来就用 `SetExt::method(recv)` 静态派发，
   只有 DictExt 遗漏在 dyn 上——改法与既有约定一致。

2. **连带发现**：仅改成 `DictExt::lz_keys` 会冒出新的
   `E0782 expected a type, found a trait`（rustc 提示 "you may have misspelled this
   associated item"）。查生成的 trait 定义确认：`DictExt` 声明的是
   **`fn keys(&self) -> Vec<K>`**（dict.rs:52），impl 端也是 `fn keys`（130 行），
   **`lz_keys` / `lz_values` / `lz_items` 根本不存在**——旧的 `keys→lz_keys` 改写是
   历史遗留的失效映射。去掉该映射、直接用 LZ 原名即可。

   全限定语法 `DictExt::keys(recv)` 本身就足以绕过 std `BTreeMap` 的同名固有方法
   （固有 `keys()` 返回迭代器而非 LZ 的 `Vec<K>`），不需要 `lz_` 前缀。

**改动位置**：搜索生成 `<dyn {Trait}<_, _>>::{method}({recv}, ...)` 的分支。
它与此前修 BUG-8 时动过的 `static_type_call` 分支同源（MethodCall 臂，约 11291 行起），
在该分支里对输出做替换即可。

**注意**：该分支同时承接合法的 `Type::assoc_fn()` 调用（如 `__Params::new()`），
改动时只替换 `<dyn ...>::` 这一种形态，不要影响正常的 `Type::method` 输出。

### 风险

- 若调用点**确实**需要跨具体类型动态派发（当前 dict.lz 不需要），去掉 dyn 会改变语义。
  建议先只在「trait 非 dyn 兼容」时降级为静态派发，避免误伤合法 dyn 场景。
- 修完需回归 `DEMO/lz_std/list.lz`、`DEMO/lz_std/string.lz`（同样大量使用 Ext trait）。

---

## 子问题 ④：`str` / `String` 不一致（Dict<str> 专用，深层设计问题）

### 现象（修复 ①②③ 后 `dict.lz` 残存的报错）

dict.lz 的 `main` 测试用的是 `Dict<str, int>`，修复 ①②③ 后残存约 6 个错：

```
error[E0308]: ... expected `Vec<&str>`, found `Vec<String>`          (dict.rs:534)
error[E0308]: ... expected `Vec<&str>`, found `Vec<i64>`              (dict.rs:540)
error[E0277]: can't compare `&str` with `i64`                        (dict.rs:544)
error[E0631]: ... expected fn(&String, &_), found fn(&str, &_)       (dict.rs:629 / 646)
```

### 根因（与 Ord/dyn 无关，是 LZ `str` 表示的固有张力）

codegen 里**两套 `str` 映射并存**：

- 类型层 `IrType::Str`（以及 LZ 关键字 `str`）→ Rust `String`
  （`codegen/mod.rs` 2449 `"str" => "String"`、2631 `IrType::Str => "String"`）。
  于是 `Dict<str, int>` → `BTreeMap<String, i64>`，`keys()` → `Vec<String>`，
  方法签名 `fn(&K, &V)` → `fn(&String, &i64)`。
- **引用 / 字面量层**刻意走 `&str`：
  - `ref str` / `&str` 引用 → `&str`（2755-2762、3613-3621、7344-7350 多处
    「特殊处理 Str 引用：生成 &str 而非 &String」）。
  - 字符串字面量 `"..."` → Rust `&'static str`（Rust 默认行为）。

于是 `Dict<str, int>` 里：键类型是 `String`，但闭包参数 `ref str` 是 `&str`、
字面量/数组 `["a","b"]` 是 `&str` —— 三方对不齐 → E0631 / E0308 / E0277。

> 这套「`str`→`String` + `ref str`→`&str` + 字面量 `&str`」是**既有设计取舍**：
> 选 `String` 是为躲 `&str` 的生命周期（结构字段/返回值），特判 `&str` 是为让字符串
> 字面量能传给 `ref str` 形参（`string.lz` 依赖此约定，见 3657-3664 注释）。
> `Dict<str>` 同时踩中两边，故冲突。

### 为什么不在本轮顺手修

把 `str` 统一成 `&str`（让 `Dict<str,int>`→`BTreeMap<&str,i64>`、`keys()`→`Vec<&str>`、
字面量/闭包全 `&str`）在类型上完全自洽、能消掉 ④，但 `&str` 会引入生命周期：
结构字段 `DictIter { dict: BTreeMap<&str, V> }` 需要带生命周期参数，会牵动
`DictIter<K,V>`→`DictIter<'a, K, V>` 全链路改写，并把 `string.lz` 等大量用例的
`&str` 约定一并改变——属于**编译器级的表示层决策**，风险高、影响广，不适合夹在
「修 Ord 约束」里顺手做。

### 建议处置

- **不并入本次修复**。①②③ 的 Ord/dyn 目标已达成，dict.lz 的「可编译性」从 3 个
  Ord/dyn 错收敛到 1 类独立的设计层问题。
- ④ 应作为独立议题：要么（A）把 LZ `str` 整体改成 `&str` 并补生命周期（彻底解，
  工作量大）；要么（B）保留 `String` 但让 `ref str` 与字面量在「上下文期望 `String`」
  时自动 `.to_string()`（局部 coercion，需类型上下文，较脆弱）。
- 在 `KNOWN_RUSTC_FAILURES` 里保留 `dict.lz`，注释更新为「①②③ 已修，剩 ④
  str/String 设计问题」。

---

## 实施顺序建议

1. 先做 ③（改动最局部，一个分支替换），单独验证 E0038 消失。
2. 再做 ①②（需改 `compute_cmp_constraints` 数据结构，影响三个调用点），验证 E0277 消失。
3. 两项都绿后，从 `tests/demo_codegen_compile.rs` 的 `KNOWN_RUSTC_FAILURES` 移除
   `"DEMO/lz_std/dict.lz"`，收紧闸门。
4. 全量回归：`cargo test -j 1 --test demo_codegen_compile` 与 `--test find_bug_bugs`。

> 注：`cargo test` 全量并行在 Windows 会因页文件不足（os error 1455）报大量虚假错误，
> 务必用 `-j 1` 或单目标。

---

## 附 1：本轮已修复项（留痕）

| 用例 | 根因 | 状态 |
|---|---|---|
| `ty004_params_type_erase` | Call 臂用**未剥离哨兵**的 `field` 重新渲染 `Type::method()`，覆盖算对的 `callee_s` → 生成 `__Params::␁PATH␁new()` | 已修复 |
| `sb001_time_method` | 同一哨兵使 `"␁PATH␁Duration"` 与 STD_MODULES 已知名比对**静默失配**，`time::Duration::fromMillis` 未映射 | 已修复 |
| `DEMO/04_functions/spread_protocol.lz` | `?` 作用于 `Option` 但函数返回 `Result`（E0277），改为 `.ok_or_else(\|\| LzError::new("None"))?` | 已修复，已移出基线 |
| `dict.lz` 子问题 ①②③ | ③ `<dyn DictExt<_, _>>::lz_keys` 非 dyn 兼容（E0038）+ `lz_keys` 方法名不存在（E0782）；①② impl 头 `K` 与方法 `map` 的 `K2` 缺 `Ord`（E0277） | 已修复 |

## 附 2：顺带修复的回归（重要教训）

修 ③ 时跑全量验证，发现 **`DEMO/lz_std/list.lz` 已被我此前的 BUG-9 改动弄坏**，
而魔法 demo 闸门**不包含 list.lz**，所以一直没暴露。

- 症状：`E0614 type i64 cannot be dereferenced`
  → `move \|x: &i64\| { *(x).clone() % 2i64 == 0i64 }`
- 根因：BUG-9 为避免非 Copy 算术移动而给操作数加 `.clone()`，但 `is_copy_ty`
  只认 `Int/F64/Bool`，把**引用 `&i64`** 也当成需 clone 的类型。
  `(&i64).clone()` 经自动解引用得到 `i64`，外层再 `*` 解引用即炸。
- 修法：`is_copy_ty` 补上 `Ref(_) / MutRef(_)`——引用本身即 Copy，无需 clone。
- **教训**：魔法 demo 闸门覆盖面有限，改动算术/表达式这类横切逻辑后，
  必须跑 `-- --ignored` 的全量 DEMO 闸门（`cargo test -j 1 --test demo_codegen_compile -- --ignored`）。

**哨兵（BUG-8 引入）是系统性隐患**：`\u{1}PATH\u{1}` 前缀会静默破坏任何
「拿字段名比对已知名」的分支。已逐个剥离的位置：FieldAccess 渲染臂、Call 臂的
FieldAccess callee、STD_MODULES 块。后续若再现同类怪现象，优先怀疑尚有未剥离的比对点。

闸门现状：`find_bug_bugs` 46 passed / 1 failed（`core_compose_ok`，E0282，需真类型推断）；
`demo_codegen_compile` 全绿。
