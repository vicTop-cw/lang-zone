# ΣLang (lang-zone) 项目记忆 — AtomCode 启动即读

> 用途：下次启动自动读取，据此行动。更新于 2026-08-09。

---

## 一、项目身份

- **项目**：ΣLang / LZ（lang-zone）—— `.lz` → Rust 的源码到源码编译器。
- **定位**：LZ 是面向系统编程的静态类型语言：默认可变绑定、结构类型（duck typing）、魔法方法驱动运算符重载、构建块语法、编译期宏与 comptime。
- **核心路线（用户拍板）**：**全力走 IR 中间表示**——AST → LZIR → Rust。**只关注 IR→Rust 这条线，Cython 后端（`lzcyc`、`src/ir/codegen_cython.rs`、`src/codegen/`、CY/ 目录）不管、不碰、不为其写代码**。
- **与 sigma-lang 无关**，基于 IR 生成 Rust 代码。

## 二、构建与验证命令（每次改动后必跑）

```bash
cargo check --quiet          # 快速编译检查
cargo build --quiet          # 构建二进制 target/debug/lang-zone.exe
target/debug/lang-zone.exe DEMO/02_types/duck_demo.lz   # 编译 .lz → 生成同名 .rs
cd <目录> && rustc --edition 2021 xxx.rs -o xxx && ./xxx   # 验证生成的 Rust 可编译可运行
```

- `lang-zone.exe <file.lz>` 默认走 IR codegen，输出 `file.rs`。
- **验证铁律**：生成的 `.rs` 必须能过 `rustc` 编译且运行正确，才算完成。

## 三、架构与关键模块

```
.lz → Lexer(src/lexer) → Parser(src/parser) → AST(src/ast) → IR builder(src/ir/builder.rs) → IR codegen(src/ir/codegen.rs) → .rs
```

- `src/ir/mod.rs`：IrModule 定义、模块入口。
- `src/ir/node.rs`：IR 节点（FnDef/StructDef/DuckDef/Stmt/Expr 等）。
- `src/ir/types.rs`：IrType（Named/Option/Result/Duck/Generic…，无 List/Dict 变体，用 Named 表达）。
- `src/ir/builder.rs`：`build_ir(ast_module) -> Result<IrModule>`，AST→IR；**末尾接入 duck 结构匹配检查**。
- `src/ir/codegen.rs`：IR→Rust 字符串，`CodeGen::generate`。
- `src/ir/duck_check.rs`：duck 结构匹配编译期检查器（`check_duck_satisfaction` / `collect_duck_impls`）。
- `src/parser/parser.rs`：AST 解析，含 `parse_duck_def`、`parse_params`、`parse_single_param`。
- `src/ast/decl.rs`：AST 声明节点（DuckDef/DuckField/DuckMethod/Function/VariadicMode…）。
- 规范文档：`SYNTAX/*.md`（36 份），**写语法功能前先读对应文档**，例如：
  - `01b-duck关系约束.md`（duck 完整规范，含关系约束、参数约束、软关键字）
  - `03d-可变参数.md`（`..` 变参注入 + `/` `*` 分隔符规范）
  - `06a-struct.md` / `06b-enum.md` / `06c-trait和impl.md` / `05b-block命名块.md` 等。
- DEMO：`DEMO/01_basics/…DEMO/16_testing/`，`DEMO/02_types/duck_demo.lz`、`duck_relation.lz` 是 duck 覆盖样例。

## 四、已实现功能状态（截至 2026-08-07）

### duck 关键字（已完成一轮）
- parse + IR + Rust trait codegen（`pub trait HasArea { fn area(&self) -> f64; }`）。
- **多泛型关系语法**：`duck Mapper<T, R> = def T.map(self) -> R` 类型前缀、`A.x: f64` 字段前缀（AST/IR 用 `owner: Option<String>`）。
- **参数约束族**：`exact(N)` / `min(N)` / `max(N)` / `range(L,R)` → 统一 `(min,max)` 存 `param_range`。
- **结构匹配编译期检查**：`src/ir/duck_check.rs` 报 `error[E0600]`（缺方法/字段、参数数量、返回类型）。
- **自动 impl 生成**：结构满足 duck 的类型自动生成 `impl HasArea for Circle { fn area(&self) -> f64 { Circle::area(self) } }` 委托，使生成 Rust 可编译运行（关键突破）。
- where 约束 + 尖括号内联约束 `<T: HasArea>` 均可用。

### duck 后续功能全部补齐（2026-08-07 完成，已验证 rustc 编译运行）
- **多泛型关系自动 impl**：`collect_duck_impls` 返回 `(type_name, duck_name, HashMap<duck泛型, IrType>)` 调用点绑定；`infer_duck_bindings` 用方法签名 unify 反推 duck 泛型 → 具体类型映射（含泛型类型 `Box2<T>`）；trait 方法带 owner 时给默认实现 + `_lz_duck_phantom` 防 E0392；impl 泛型带 `Clone + std::fmt::Debug` bound。
- **嵌套约束**：`duck D<T> where T: Iterable` → 检查器递归验证（`depth < 8` 防环），duck 的 where_clause 存到泛型参数 bounds。
- **字段关系**：`A.id == B.id` / `A.name: B.name` → AST/IR `DuckField.rel: Option<(owner,name)>`；trait 用关联类型 `type __Field_x;` + accessor `fn __field_x(&self) -> &Self::__Field_x`；检查器比较两侧字段类型相等；泛型函数体内生成 where 投影约束 `<A as Duck<...>>::__Field_x: PartialEq<<B as Duck<...>>::__Field_x>`。
- **关联类型**：`type I.Item` → AST/IR `assoc_types`；trait 生成 `type Item;`，impl 绑定 `type Item = <具体类型>`；方法签名渲染用 `duck_sig_type`（`I.Item` → `Self::Item`，先 substitute duck 泛型再渲染）；泛型函数体内生成 `<T as Duck<...>>::Item: std::fmt::Debug` 约束。
- **软关键字族**：`satisfies`（递归验证）/ `sealed`（成员数闭合检查）/ `default def`（可选成员，缺失不报错）/ `match "pattern" at_least(N)`（正则数量，`regex_like_match` 自实现子集：`\w`/`\d`/`(a|b)`/`+`/`*`/`?`/`.`）/ `require`/`optional`（命名参数行）；正则方法名 `def "get_\w+"(self)`。
- duck 检查器 subst 从 bound args 构建（`where T: Mapper<T,R>` 的 T/R 位置 → 实参类型），`ty_fully_bound` 判断 duck 泛型是否全绑定，未绑定保守跳过避免误报。

### 可变参数 / 参数分割（2026-08-07 完成，按 03d 文档）
- `..` 是**变参注入标记**（最多 2 次），任何 `..` 出现即注入：单 `..` 无注解 → 注入 `args`（元素 Any）；`..: Tuple<T>` → args-only；`..: Dict<K,V>` → kwargs-only；双 `..` → args + kwargs。
- `/` `*` 是 Python 式安全分隔符（纯分割、不注入、类型安全），与 `..` **互斥**（混用报 Parse error）。
- AST `VariadicMode`：`ArgsOnly { dotdot_at, elem_ty }` / `KwargsOnly { dotdot_at, value_ty }` / `Both { … }`。
- IR：注入隐式参数 `args`（`&[T]` 切片收集）、`kwargs`（`&HashMap<String, V>`），调用点自动打包。
- **`..: Tuple<T1,T2,..>` 通配**：parse_type 泛型参数内支持 `..` 占位（push Any）。
- **@overload 变长分派**：`overload_variadic`/`overload_explicit` 记录每签名是否变长 + 显式参数；`match_overload` 阶段1 固定签名精确匹配 → 阶段2 变长签名兜底（显式参数兼容即可）；收集后补登记 mangled 名的 variadic/kwargs/param_types 供调用点打包。

### 其他已有能力（勿重复造）
- block/checker 语法（`block NAME ^:` 等）、while_let、闭包、构建块、@overload 重载、泛型 where 约束、enum/struct/impl、comptime、宏、检查站 `[ps]/[chk]`。

### 其他基础设施
- 空字段 struct 构造 `Text()` → `Text {}`（Call fallback 分支修复）。
- 版本推进：`src/util/version.rs` VERSION_PATCH = 141（v0.133 起 + 8 小阶段）。

## 五、项目规则（用户全局偏好，必须遵守）

1. **输出选项/选择前，先给明确建议（带理由），再列选项**。
2. **ΣLang 长期自主运行规则**：每个小阶段 = 一次版本推进（v0.133 起）；每 10 个小阶段同步一次仓库（git 提交 + push）；每 100 个小阶段发布一次 PyPI（100 小阶段 = 0.0.1，当前 0.7.1 已发布，下次 0.7.2 需 100 小阶段）；总目标 496 个小阶段；**完全自主自由发挥演化，不弹任何询问**。
3. 提交信息：English 正文 + Conventional Commits + 末尾 `Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>`；**只在用户要求时提交**（自主运行规则下按第 2 条节奏提交）。
4. 用户说"先易后难"→ 排任务按难度递增。
5. 修改代码前先读文件（read_file），用 edit_file / write_file 改文件，禁止 sed/重定向改源码。

## 六、下一步方向（按难度，供后续自主选择）

> 2026-08-07 全量语法核查后，未完善清单已写入 AtomGit issue #1
> （https://atomgit.com/VictorTop/lang-zone/issues/1），按修复优先级排序：

1. **高**：管道 `|>` 实参重复 bug（`5 |> double` 生成 `double(5,5)`，现有 DEMO/05_expressions/pipe.lz 也编译失败 E0061）。
2. **高**：字典推导多 for（`{k: v for k in 1..2 for v in 10..11}` 报 `Expected RBrace, got For`；列表推导多 for 正常）。
3. **中**：trait 体内关联类型 `type Item` + `Self.Item` 引用（06c §五，parser 未实现；可复用 duck 关联类型 codegen 路径）。
4. **中**：泛型默认参数 `T = int`（03b §四，parser 跳过默认表达式，GenericParam.default 恒 None，需 AST/IR/codegen 贯通）。
5. **中**：生成器无返回类型标注时 yield 类型推断（`iterator counter(n: int) = …yield i` 生成 `Vec<()>` E0308）。
6. **难**：多类型变参位置约束 `..: Tuple<T1,T2,..>`（03d §2.3，元素类型只取 first，依赖 type-pack，成本最高，可与 P2-6 合并规划）。
7. **长期**：跨模块类型推断（P2-3，lz-infer 未接入）、模块级魔法属性 `__doc__`/`__all__`（P2-5）、`__Params.args` 异构元组化（P2-6）。

## 七、已知坑（避免踩）

- `IrType` 没有 `List`/`Dict`/`Set` 变体——用 `Named { path: "List", args }` 表达；写 IrType 匹配时别引用不存在的变体。
- `IrType::Any` 在 rust_type 映射为 `"i64"`（fallback）。
- duck 检查器在 `build_ir` 末尾运行，报错会阻止 codegen。
- 关键字实参语法是 `name: value`（`:`）或 `name~` 糖，**不是** `=`。
- 双 `..` 调用点打包：`fn_variadic` 只记位置变参（排除 kwargs），kwargs 走 `fn_kwargs`。
- 函数内局部变量勿与模块级函数/类型同名（避免 E0530/解析歧义，已有 param_renames 机制处理参数，但函数体局部变量仍可能撞名）。
- duck 方法签名里的 duck 泛型引用是 `Named(path)` 而非 `Generic`——替换要用 `duck_check::substitute`（Named path 匹配 subst），`IrType::substitute_generics` 只替换 Generic 会漏。
- 自动 impl 方法签名：先 `substitute` 替换 duck 泛型（R→Fahrenheit）再 `duck_sig_type` 处理关联类型（I.Item→Self::Item），否则 impl 里出现未定义类型 R。
- 多泛型 duck（有 owner 前缀方法）的 trait 方法必须给默认实现 `{ unimplemented!() }`，否则 impl 只覆写本类型方法会编译失败；加 `_lz_duck_phantom` 防 E0392（未使用泛型参数）。
- 字段关系 duck（`A.id == B.id`）在泛型函数体内比较两侧字段时，必须生成 where 投影约束 `<A as Duck<..>>::__Field_x: PartialEq<...>`，否则 E0369。
- 关联类型在泛型函数体内 `print` 需 where 约束 `<T as Duck<..>>::Item: std::fmt::Debug`，否则 E0277。
- 泛型函数体内访问 duck 约束泛型参数的字段（`a.field`）→ 自动转 `a.__field_field()` trait accessor（`duck_field_members` 按参数名收集，key 是参数名不是泛型名）。
- 空字段 struct 构造 `Text()` 走 Call fallback 需生成 `Text {}`（`args_s.is_empty() && is_known_type` 分支）。
- **已知缺陷（未修复）**：管道 `|>` 实参重复、字典推导多 for 报错、trait 内关联类型不支持、泛型默认参数值丢弃、生成器无返回类型 yield 推断缺失——详见 issue #1。
- ΣLang 版本推进至 v154（2026-08-08），全量回归基线 PASS 146 / FAIL 58（15 PARSE / 42 RUSTC / 1 RUN），commit 52fc0d1 已 push github+gitcode
- 2026-08-08 目录规整完成：issues/ 并入 issue/（README 补说明）；demo 测试报告移入 issue/；div-tools/ 收纳 9 个辅助脚本（含 check_doc_versions.py、fix_*.ps1）；新增 README-FOR-AI.md（AI 接手规范：唯一 IR 路线、开发期禁止缓存/增量编译、必须全量回归）与 history-work/（工作记录）
- 已清除 DEMO 与 SYNTAX 中「规范目标特性/未实现/语法冻结/待实现」字眼（用户 2026-08-08 决策：无规范目标特性，所有 DEMO 除 99_errors 都需测试修复）；99_spec 下 guard_for_3/duck_test/iterator_demo 等已实际修复通过
- 闭包写外部变量已支持：无 let 前缀可变绑定（x = v）在变量已存在时转 Stmt::Assign（TypeCtx.block_declared 记录本块首次声明，Closure 分支创建独立 closure_ctx 重置 block_declared）
- duck 类型作为参数注解（pet: Pet）→ 泛型参数 + trait bound（DuckParam0: Pet）；字段访问走 __field_X().clone()（duck_field_members 覆盖 Named duck 参数）；collect_duck_impls 在调用点为具体类型生成 impl Pet for Cat
- 宏系统已支持：lexer template 关键字、macro/template 顶层定义、import macro/from macro/as 别名、跨模块 MacroRegistry::merge、@alias.name! 展开、Tokens 类型→Str（from_ast_type + rust_type 双映射）、quote(...) 降级为字符串拼接（codegen 多参 &[..]）

## 八、v157 本轮进展（2026-08-09，commit c7528de 已 push）

- **全量回归基线**：PASS 189 / FAIL 15（92.6%），优于 v156 的 187/17；报告 `issue/demo-test-report-2026-08-08.md` 与失败清单已更新至 v157。
- **剩余 15 项失败分类**：macro 系 2 项（macro_real Backtick、use_macros，用户明确排除）＋ lz_std 标准库深层 13 项（traits.lz 剩余 PARSE + 12 个 RUSTC：E0423 枚举变体路径 `Ordering.Less`、E0404 Hash derive、E0782 trait 类型、E0053 `next` 类型、E0392 未用泛型、E0416 模式重绑定 `Less_`、E0277 ImplicitFrom 边界、E0390 primitive impl、E0369 `sign` 遮蔽残留、E0053 等——均为编译器特性级改造，非本轮范围）。
- **本轮修复内容（9 类）**：
  1. variadic_type_fidelity.lz type-pack 全链：`Ts...` 泛型/类型参数解析、`args.N` 元组索引（FieldAccess 数字字段→切片索引）、切片模式 `[a, ..]`、`(a,)` 单元素元组模式（has_comma 区分）、else 兜底臂、空切片自动兜底臂、slice 绑定臂体内自动 `.clone()`；**0 error 运行正常**。
  2. iter.lz 解析修复：where 关联类型 `I.Item: Add<Output = I.Item>`、`Output = I.Item` 命名泛型参数、闭包体赋值表达式（parse_expr `x = expr`→Expr::Assign）、assert! 单表达式（无 expected 时不再生成 assert_eq! 单参）、impl 泛型去重（`impl<T> Iterator for Once<T>` 不再重复 T）、`__next__`/`__size_hint__` 在 `impl Iterator` 中映射为 `next`/`size_hint`（新字段 `in_iterator_impl`）、`I.Item` 关联类型路径 `I::Item`（rust_type 中 `.`→`::`）。
  3. math.lz：内联 if 表达式（`let sign = if x < 0.0: -1.0 else 1.0`，Token::If 分支支持冒号后直接表达式）、let 变量遮蔽模块级函数（E0530 重命名后登记 param_renames 使引用同步）。
  4. string.lz：`template` 是硬关键字不可作参数名（00-词法基础 §1.8），修正测试文件参数名 template→tpl。
  5. traits.lz：where 子句 `Self.Item`（Token::Self_ 支持）、trait 关联类型带 bound（`type Iter: Iterator<Item = Self.Item>` 消费 bound）。
  6. __init__.lz：const `&str` 作 lhs 的字符串拼接（`STDLIB_NAME + " v"`）→ 生成 `.to_string() + &...`（str_concat 分支 `lhs_is_ref_str`）；**0 error 运行正常**。
  7. box.lz：模块自定义 `struct Rc<T>`/`Arc<T>` 时跳过 `use std::rc::Rc`/`Arc` 导入（E0255 修复）。
  8. lz_std 宏调用错误系列（dict/list/option/ordering/prelude/result/set/error）："unexpected end of macro invocation" 全部清零。
  9. 全量回归从 130/74 恢复到 189/15：修复 parse_expr 赋值分支引入的语句级回归（`x = 42` 默认可变绑定被解析成 `x == 42` 比较 → parse_stmt 中 Ident 后跟 Eq 时先识别为 Stmt::Let；`a[i] = v` 索引赋值 Expr::Assign 转回 Stmt::Assign）。
- **新增已知坑**：
  - `parse_expr` 的赋值表达式分支（为闭包体 `|x| = total = total + x` 支持）会抢先消费 `=`，**语句级** `x = 42`（默认可变绑定）必须在 parse_stmt 中先于 parse_expr 识别为 Stmt::Let，`a[i] = v`/`obj.f = v` 的 Expr::Assign 需转回 Stmt::Assign——否则 builder 把 Assign 转 BinOp `==` 生成比较（E0425/E0369 回归）。
  - `parse_generic_params` 中 `impl<T> ... for Once<T>` 的 `Once<T>` 类型参数若与已有泛型同名会重复收集 → 生成 `impl<T,T>`；解析时对已存在名字去重。
  - `for (k, v) in ...` 元组解构循环变量需分别收集为局部变量（`collect_for_var_bindings`），否则 analyze_global_vars 把未收集名字误判为跨函数全局变量（E0530 static mut 冲突）。
  - lz_std 是标准库自测（#!bin lz），多数文件 rustc 仍有深层错误（E0423/E0277/E0404 等），非解析层可修，需编译器特性级改造。
- v157（2026-08-09）全量回归 PASS 189/FAIL 15（92.6%），commit c7528de 已 push；剩余失败：macro 2（排除）+ lz_std 深层 13（traits.lz 剩余 PARSE + 12 RUSTC：E0423/E0404/E0782/E0053/E0392/E0416/E0277/E0390/E0369，需编译器特性级改造）；本轮新增坑：parse_expr 赋值分支抢先消费 `=`，语句级 `x = 42` 须在 parse_stmt 先识别为 Stmt::Let、`a[i]=v` 的 Expr::Assign 转回 Stmt::Assign（否则生成 == 比较回归）；for (k,v) 解构变量须分别收集（collect_for_var_bindings 防 E0530）；impl<T> for Once<T> 泛型去重。
- codegen Paren(BinOp) 不再剥离括号（(a+b)/c 剥离会改变优先级，math.lz sqrt 断言失败）；仅 UnOp 剥离。
- comptime 已实现：Expr::Comptime/Stmt::Comptime 在 ir/builder.rs 中调 ComptimeEvaluator 求值内联；TypeCtx.comptime_consts 存顶层 const 求值结果（comptime 内可引用 const），需在 convert_block/convert_fn_def 等子 ctx 继承。
- template 机制：TemplateRegistry+extract_template_defs+TemplateExpander（name! 调用，main.rs/project.rs 在 MacroExpander 后接入）；quote 内建重新 lex StrLit 时需 trim_start（前导空格产生 Indent 报错）并过滤 Eof/Semicolon；Binary Plus 后 merge_str_lits 合并相邻 StrLit。
- #!bin macro：lexer 识别整行产生单个 Token::Macro，extract_macro_defs 在 macro 后跟 Newline/Eof 时消费该 token（否则残留到 Parser 报 Expected macro name）；多参数非属性宏（check_eq 两参）由 expand_macro 用 split_top_level_args 按顶层逗号拆分绑定。
- comptime 编译期函数调用：ComptimeEvaluator Call 分支查 module.functions 求值纯函数（参数位置绑定、继承 symtab）；内建 len()/push/索引支持；comptime_value_to_lit 返回 ExprKind 支持 List/Tuple 内联 vec![...]（查找表焊死）。TypeCtx.comptime_module 需在子 ctx 继承。
- 宏展开要点：expand_macro 按调用形式分派（attr 有值→属性宏；多参数→split_top_level_args 拆分绑定），不依赖 is_attr（参数个数≥2 误判 bug）；宏体必须单行表达式（parse_macro_body 不处理跨行 + 链）；quote 重新 lex 的字符串片段与参数 token 不合并成单一标识符（`"get_" + name` 生成两个 Ident）——模板生成完整函数名参数。
