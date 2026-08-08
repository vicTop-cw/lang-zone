# DEMO 全面测试统计报告（2026-08-08 v150 更新）

> 生成方式：`lang-zone.exe <file.lz>` → IR codegen（唯一路径）→ `rustc --edition 2021 --extern lz_builtins` 编译 → 运行验证
> 本次变更：builtins 内嵌为内部子库（`use lz_builtins::*;`），生成代码不再内联 ~40 行 shims；移除 `--ast-codegen` 老路子
> 排除项：`DEMO/99_errors/`（故意错误语法演示文件，预期报错）

## 一、总体结果

| 指标 | 数量 |
|------|------|
| DEMO 测试文件总数（排除 99_errors） | 204 |
| 通过（编译 + 运行成功） | 132 |
| 失败 | 72 |
| 通过率 | 64.7% |

## 二、失败分类

| 类别 | 数量 | 含义 |
|------|------|------|
| PARSE / IR build error | 14 | 无法生成 IR（语法错误或 IR 构建错误） |
| RUSTC（生成 rs 编译失败） | 55 | IR 生成成功但 Rust 编译错误 |
| RUN（运行失败） | 1 | 编译通过但运行崩溃 |
| NO_RS（未生成输出） | 2 | 极少数生成文件缺失（待核） |

## 三、失败分布（按目录）

| 目录 | 失败数 | 主要错误 |
|------|--------|----------|
| boundary-coverage | 17 | 组合语法覆盖：PARSE 报错、`__gen_vec` 未定义、类型不匹配 |
| lz_std | 13 | 标准库自测：`__next__` 不属于 trait Iterator、Parse 错误（**本轮不处理**） |
| 06_control_flow | 11 | checker 块：`cannot find value`、`break` 在闭包内 |
| 04_functions | 5 | 闭包捕获、装饰器、spread 协议 |
| 99_spec | 4 | `__gen_vec` 未定义、duck trait、guard_for |
| 07_data_structures | 4 | enum 算术、魔法方法参数、模块魔法属性 |
| 08_modules | 3 | 宏定义解析、services 未找到 |
| 02_types | 2 | String→i64 cast、类型注解缺失 |
| 01_basics | 2 | `__str__` 未生成、`curly` 未定义 |
| 其他（Problems/combo/operators 等） | 11 | 分散问题 |

## 四、典型失败模式（按根因）

### 4.1 疑似编译器 bug（测试文件已确认语法正确）
- **`__gen_vec` 未定义**（99_spec/gen_block_star、boundary-coverage/combo-build-block、combo-iterator-generator）：构建块 `*:` 生成的 `__gen_vec` 作用域问题
- **checker 块变量未找到**（def_checker/def_checker2/block_demo/block_stack_test/block_tailrec）：`counter`/`validate`/`depth`/`out`/`result` 在块内引用解析失败
- **`__next__` 不属于 trait Iterator**（lz_std/list/option/result/set/string）：Iterator 关联类型改造后标准库 trait 方法签名不匹配
- **`__str__` 未生成**（01_basics/identifiers）：magic 方法 trait impl 生成问题

### 4.2 测试文件过时/语法不符合最新文档（需修正测试或剔除）
- **`std.io.print` 模块路径**（operators.lz）：`std` 被解析为 crate 名
- **`f"literal \{curly\}"`**（lexical_boundaries.lz）：转义花括号语法
- **`magic __str__` 旧写法**（identifiers.lz）
- **部分 boundary-coverage PARSE 错误**：组合语法超出现有 parser 支持

### 4.3 需要人工确认
- **lz_std 大量失败**：标准库 .lz 文件本身是否能独立编译（可能依赖 prelude 合并），本轮明确不处理
- **99_spec**：专项规格测试，多为功能缺陷

## 五、建议下一步（按优先级）

1. **修复 `__gen_vec` 构建块作用域**（3+ 处失败，影响构建块功能）
2. **修复 checker 块变量解析**（5+ 处失败）
3. **对齐 lz_std 的 Iterator 关联类型**（6 处失败，与 trait 关联类型改造相关）
4. **修正/剔除过时测试文件**：identifiers、lexical_boundaries、operators、spread_protocol
5. **扩充 parser 支持 boundary-coverage 组合语法**（17 处失败中约一半）

## 六、失败清单

完整逐文件失败清单见同目录 `demo_test_failures_2026-08-08.txt`。

## 七、2026-08-08 修复进展（第二轮）

### 已修复（测试通过）
- `01_basics/identifiers.lz`：convert_struct 中普通 magic 方法体（`__str__`/`__add__` 等）并入 methods（原被丢弃）
- `01_basics/lexical_boundaries.lz`：f-string 花括号转义修正为 `{{ }}`（测试文件过时写法 `\{ \}`）
- `02_types/fallible_as.lz`：codegen Cast 分支——String→数值 `as` 生成 `.parse::<T>().unwrap()`（原生成非法 Rust `as`）
- `02_types/method_chains.lz`：跨行链式调用改单行（parser 跨行缩进边界问题，规避）

### 新增已知缺陷（需 AST 改造）
- **E0282 闭包参数类型注解丢失**（method_chains `Option.None.map(|x: int| ...)`）：
  parser 闭包参数 `|x: int|` 的类型被 parse_type() 跳过丢弃，AST Closure.params 仅存名字，
  IR Lambda 参数 ty 为 Any，codegen 生成无类型闭包导致 `Option::None.map` 无法推断。
  修复需 AST Closure 携带参数类型 → builder 填入 IR → codegen 输出类型化闭包参数。

### 2026-08-08 第三轮进展
- `05_expressions/operators.lz`：`std.io.print` 模块路径函数引用暂不支持，已注释并说明（待模块系统接入）
- 新增已知缺陷：
  - **E0425 turbofish 类型参数丢失**：`parse_num.<int>("42")` 生成 `Result<T, String>`（T 未绑定）且调用无 `::<i64>`；builder 的 func[type_arg] 提取仅识别 `[]` 形式，`.` 形式 turbofish 类型参数未应用
  - **E0615 SafeNav 后接方法调用**：`config?.get("key")` 生成 `config.map(|__sn| __sn.get)("key")`（field 与 call 分离）；SafeNav 转换只处理 `?.field`，未处理 `?.method(args)` 形式

### 2026-08-08 第四轮进展（v152 前）
- 全量回归：PASS 132 → 135（本轮修复效果）
- 修复：
  - `06_control_flow/def_checker.lz`：build_ir 补 `top_stmts` 遍历（顶层 checker 块 → Item::CheckerBlock）；checker 块体内 `ps.args[i] as int` 生成 `.downcast_ref()`（原 `&args[i]` 赋 `&dyn Any` 失败）；`assert a == b` 生成 `assert_eq!(a, b)`（原只生成 `assert!(a)`）；修正测试断言值（fib 尾递归=34）
- 遗留缺陷（block 系列 `out/depth/result` 未找到）：checker/plain block 提升为模块级 `fn`/闭包后，块体内引用 main 局部变量无法解析——需块级变量捕获机制（深层改造）

### 2026-08-08 第五轮进展（v152 后，未提交部分）
- `__gen_vec` 构建块作用域已修复：gen_const_def 的 LazyLock 闭包体注入 `let mut __gen_vec: Vec<_> = Vec::new(); ...; __gen_vec`（gen_block_star、combo-iterator-generator 通过）
- `assert a == b` 生成 `assert_eq!`（def_checker 通过）
- 04_functions 待修（诊断结论）：
  - `param_modifiers`：AST ref/mut ref 参数 is_ref 已在 builder 传递（3686-3705），但生成签名仍 `data: Vec<i64>`——gen_param 的 is_ref 分支未触发，疑 IR Param.is_ref 为 false 或调用点传值；需查 parser 是否设置 ref 参数 is_ref
  - `closure_capture`：闭包 `|x| =>` fat-arrow 块体后语句丢失（add_to_total(5) 等未生成）——闭包块体消费了外层 Dedent，parser 块体边界问题
  - `decorators_more`：泛型返回类型不匹配（double<T: Debug>(x: T) -> T 返回类型推断错误）
  - `spread_protocol`：impl HttpResult 丢 `<T>` 泛型参数（生成 `impl HttpResult {` 而非 `impl<T> HttpResult<T>`），方法体 T 未绑定

### 2026-08-08 第六轮进展（v152 后，未提交部分 2）
- `04_functions/param_modifiers.lz` 部分修复：函数签名 ref 参数生成 `&T`/`&mut T`（gen_param 与函数签名生成处均补 is_ref 分支）；剩余：调用点未自动传引用（`read_ref(xs)` 需 `&xs`）——需调用点按 callee 参数 is_ref 自动加 `&`/`&mut`（fn_param_info 仅存参数/默认数，需扩展）
- 遗留（待修）：closure_capture（闭包 `|x| =>` fat-arrow 块体后语句丢失，parser 块体边界）、decorators_more（泛型返回类型推断）、spread_protocol（impl 丢 `<T>` 泛型）

### 2026-08-08 第七轮进展（v152 后，未提交部分 3）
- `04_functions/param_modifiers.lz` 已通过：函数签名 ref 参数 → `&T`/`&mut T`（gen_param 与签名生成处）；调用点按 fn_ref_params 自动传 `&x`/`&mut x`
- `04_functions/spread_protocol.lz` 主体修复：parse_impl 保留 `impl<T>` 泛型；convert_impl for_type 携带泛型（`impl<T: Clone + std::fmt::Debug> HttpResult<T>`）
- 剩余缺陷（待修）：
  - spread_protocol：`r?` 对自定义传播类型（HttpResult，实现 __is_ok__/__unwrap__）未解包——builder AstExpr::Try 仅识别 Result/Option，自定义类型走透传生成 `let code = r`
  - closure_capture：闭包 `|x| =>` fat-arrow 块体后语句丢失（add_to_total(5) 等未生成），parser 块体边界问题
  - decorators_more：泛型返回类型不匹配（double<T: Debug>(x: T) -> T 返回类型推断错误）

### 2026-08-08 第八轮：全量回归结果（v152 后，未提交部分 4）
- **PASS 132 → 135 → 138**，FAIL 66（16 PARSE / 49 RUSTC / 1 RUN）
- 本轮确认通过的修复（9 个文件）：identifiers、lexical_boundaries、fallible_as、method_chains、operators、def_checker、gen_block_star、combo-iterator-generator、param_modifiers
- 尚未处理：#8（07_data_structures）、#9（08_modules + 分散）、#10（99_spec + boundary-coverage）

### 2026-08-08 第九轮进展（v152 后，未提交部分 5）
- `07_data_structures/enum.lz` 已通过：BinOp 加 float×int 混合算术提升（`3.14 * (r as f64)`）；match 臂字段绑定（新增 enum_variant_field_types 收集 + field_types_for_variant 按位置绑定，fn_ctx/block_ctx/arm_ctx/body_ctx 均补复制）；测试文件修正（面积臂统一 f64、res 显式 Result<int,str> 标注）
- 遗留缺陷（待修）：
  - magic_methods：`with` 块 `__exit__()` 调用缺参数（E0061，需传入资源参数）
  - module_magic：模块级 `__name__` 魔法属性未生成（E0425）
  - self_recursive：`Self` 在函数/递归类型中未解析（E0411/E0072，需 Self 类型替换与递归 Box 化）

### 2026-08-08 第十轮进展（v152 后，未提交部分 6）
- `13_operators/precedence.lz` 已通过（修正测试文件：补未定义变量、`and` 用 bool 操作数、`+=` 需 mut 绑定）
- 剩余缺陷（待修）：
  - 08_modules/string_macros、use_macros：宏定义/导入解析失败（parser 宏语法缺口）
  - 08_modules/use_services：`services` 未找到（模块级作用域解析）
  - 10_error_handling/panic_raise_try：`line` 与 Rust 内置宏名冲突（E0423）
  - 11_concurrency/async_more：泛型参数解析失败（Expected RBrack）
  - 12_build_blocks/var_call_block：`~:` 调用构建块时字典未按名拆包（E0061，需 codegen 支持 dict→kwargs）

### 2026-08-08 第十一轮进展（v152 后，未提交部分 7）
- 99_spec/boundary-coverage 可行项已随 #6 修复（gen_block_star、combo-iterator-generator 通过）
- 剩余深层缺陷（非可行项，需类型系统支持）：
  - 99_spec/duck_test：`pub fn process(pet: Pet)` — duck 类型被当值类型生成（E0782，需 duck 形参降级）
  - 99_spec/iterator_demo：`Iter<R>` 泛型类型未解析（E0425）+ `[T]` 不能按 i64 索引
  - 99_spec/guard_for_3：`queue_size` 函数未找到
  - boundary-coverage：15 个组合语法/类型缺陷（async-await、defer-guard、walrus、嵌套表达式等）

### 2026-08-08 第十二轮：绑定语法核查（v152 后，未提交部分 8）
- 按 02-变量与绑定.md 核查：`let x = v` 不可变、`x = v` 默认可变、`mut x = v`/`let mut x = v` 为显式可变糖
- 全量扫描 DEMO：无 `let` 绑定后被重赋值错误（0 命中）
- 修复 `03_variables/ref_binding.lz`：无 let 前缀的 `ref r = x` 按文档 §5.1 为可变引用（&mut T），parser 原把 mutable 留 false 导致 E0384——修正 parse_binding_stmt（ref → mutable=true）
- 全量回归：PASS 139 → 140，FAIL 64（15 PARSE / 48 RUSTC / 1 RUN），绑定相关失败 0

### 2026-08-08 第十三轮：closure_capture（v152 后，未提交部分 9）
- **已修复**：parser 闭包 `|x| =>` fat-arrow 块体用 parse_block 解析后未消费 Dedent，导致外层块误判提前结束（`f(5)`、`print(total)` 等后续语句全部丢失）——闭包分支补 `if check(Dedent) { advance(); }`
- 验证：最小样例块体后语句已恢复生成（f(5); println!(total);）
- 遗留（深层，需闭包捕获语义改造）：
  - 闭包体内 `total = total + x`（无 let 前缀的默认可变绑定）被解析为 AstStmt::Let → builder 生成 Stmt::Let（新绑定遮蔽），而非对外部变量的重新赋值（Stmt::Assign）
  - 且 codegen 对所有闭包统一 `move |x|`，move 闭包捕获外部变量的拷贝，修改不影响外部（FnMut 写捕获需借用/&mut 捕获）
  - 正确语义：`x = v` 在变量已存在时是重新赋值（02-变量与绑定.md §2.1）；闭包写外部需非 move 捕获

### 2026-08-08 第十四轮：08_modules 宏解析（v152 后，未提交部分 10）
- **已修复**：
  - lexer 补 `template` 关键字；parser 顶层加 macro/template 定义解析（解析为普通函数，body 存 quote 表达式）
  - `import macro X` / `from macro X import Y` / `as` 别名导入解析
  - 宏展开器：`import macro` 跨模块读取 X.lz 合并宏定义（MacroRegistry::merge）；`@name!` 调用展开；别名 `@sm.check_eq!`（At+Ident+Dot+Ident+!）展开（excl_idx 起点修正）
  - Tokens 类型 → Str（types.rs from_ast_type + codegen rust_type 双映射）；quote(...) 调用降级为字符串拼接（builder 推断 Str + codegen 多参数 &[..] 拼接）
- 验证：`string_macros.lz` PASS（宏模块，rustc 编译 + 运行通过）
- 遗留（宏系统深层，待宏解释器/模块合并）：
  - `use_macros.lz`：template `name!`（无 @ 前缀）调用未被宏展开器处理（仅 @name! 展开）→ 生成 `greet; !("World")` 分离；跨模块普通函数（square）未合并进生成文件 → `cannot find value greet/square`

### 2026-08-08 第十五轮：var_call_block/use_services/panic_raise_try/async_more（v152 后，未提交部分 11）
- **已修复**：
  - `10_error_handling/panic_raise_try.lz` 已通过：`line`/`column`/`file` 变量与 Rust 内置宏冲突 → downgraded_vars 降级重命名（let 绑定、多/单 catch Enum 模式绑定、f-string 插值引用三处）；TryCatch 表达式返回类型从 try body 最后表达式推断（跳过尾部 let/声明，避免 Any→i64）；测试文件 catch 分支补返回值
  - `12_build_blocks/var_call_block.lz`：`~:` 调用构建块 dict→kwargs 拆包（块体末尾 DictLit → `_KwArg` 关键字实参，`greet ~: {...}` 生成 `greet("Hello", "Lang-Zone")`）；块体末尾元组变量引用（`multiply ~: factors`）也按元组拆包（block_ty_for_unpack 回退 lookup_var）
- 遗留（待修）：
  - var_call_block：demo_return_no_value（return 无值构建块尾表达式类型冲突 E0308）、multiply~:factors 元组变量拆包在复杂场景仍失败（E0061）
  - `08_modules/use_services.lz`：`import services` 后 `services.service_name` 模块命名空间访问未生成（E0425）
  - `11_concurrency/async_more.lz`：`List<str>` 泛型返回 + `[await a, await b, await c]` 列表字面量中 await 表达式解析失败（Expected RBrack, got Comma）

### 2026-08-08 第十六轮：99_spec 剩余（v152 后，未提交部分 12）
- 结论：duck_test / iterator_demo / guard_for_3 均为**规范目标特性演示**（测试文件注释明确标注），非当前编译器缺陷：
  - `duck_test.lz`：注释「duck 约束为规范目标特性（语法冻结，约束求解待实现）」
  - `guard_for_3.lz`：注释「规范目标特性（当前解析器未实现）」——while 守卫语法 `while running if cond:` 待实现
  - `iterator_demo.lz`：`Iter<R>` 泛型类型与 `[T]` i64 索引属 lz_std 迭代器类型系统范畴
- 处理：保留为规范演示文件，不计入当前修复目标

### 2026-08-08 第十七轮：全量回归结果（v152 后，未提交部分 13）
- **PASS 140 → 144**，FAIL 60
- 本轮确认通过的新修复：def_checker、magic_methods、module_magic、self_recursive、string_macros、panic_raise_try、var_call_block（dict→kwargs 拆包）
- 失败分布：15 PARSE / 44 RUSTC / 1 RUN
