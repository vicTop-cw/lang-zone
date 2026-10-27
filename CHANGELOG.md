---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 9f2a11add43fbf12a546606fb2b962ab_d1133c179c3b11f1a98a525400f8a581
    ReservedCode1: bxfu1vV9/zf3BipcZW4e2TYNtBeVW+bHqK4PLgZ15h5rlYpmzg5gYQmbXmk8Fwgv2IkVjIWJL6xOPqTefv4R3SDy9zqdLM/9Smj1IZ5PwQF/DxZfqd/LcN/2BV8YJUdxQ+CcMPyvHkLc7tv5jDambxQuuLYh+RfJL0MFTNyrG4KJFQbqjnNAlnPZYtA=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 9f2a11add43fbf12a546606fb2b962ab_d1133c179c3b11f1a98a525400f8a581
    ReservedCode2: bxfu1vV9/zf3BipcZW4e2TYNtBeVW+bHqK4PLgZ15h5rlYpmzg5gYQmbXmk8Fwgv2IkVjIWJL6xOPqTefv4R3SDy9zqdLM/9Smj1IZ5PwQF/DxZfqd/LcN/2BV8YJUdxQ+CcMPyvHkLc7tv5jDambxQuuLYh+RfJL0MFTNyrG4KJFQbqjnNAlnPZYtA=
---



# CHANGELOG — Lang-Zone (LZ)

本文件记录 LZ 编译器（`lzc` / `lzcyc`）的版本变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。

## [Unreleased]

### rs-lz 后端与原生 codegen 差分对齐（2026-10-27，fist-agent lang-zone 组合轮）
- `--emit=rs-lz` 的 `rust_type` 对 `IrType.Any` 从 `Box<dyn Any>` 对齐为 `i64`（BUG-480）：无类型注解参数（`def f(x): return x + 1`）此前退化为 `Box<dyn Any>`，对之做 `+ 1i64` 触发 E0369、rs-lz 产物无法编译；原生 `emit.rs:247` 一直发 `i64`。rs-lz 侧其余两处 `Box<dyn Any>` 为 kwargs 桩（`.lz:2906` hardcode）与 `IrType.Duck`（`.lz:781`），均不受影响（`6bb52f5`）
- 定位方式：`_probes/bigsweep.sh` 64 探针差分扫描唯一真缺陷即此；同批其余 7 例为假阳性（`edge_class/edge_nums/nest/nest2/latent_variants` 为无 main 库探针报 E0601；`sweep/g65_arith_div_zero`/`sweep/g66_arith_overflow` 为刻意测除零/溢出；`x + "a"` 与 `x[0]` 在原生侧同样 E0308/E0608，属双路一致的 ill-typed 程序，非 rs-lz 独有）
- 回归测试：`tests/lz_ir_bootstrap.rs::rs_lz_unannotated_param_defaults_i64` 断言 rs-lz 发 `pub fn f(x: i64) -> i64`、不含 `Box<dyn Any>`、rustc 编译且运行输出 `42`，并对照 native 同样发 `i64`；`lz_ir_bootstrap` 25 项全绿
- 差分扫描基建：新增 `_probes/fastsweep.sh`（对 `.lz` 文件集并行跑 `--emit=rs-lz` 自检，退出码即编译运行是否通过；失败者再跑 native 编译以区分「rs-lz 独有缺陷」与「双路一致的 ill-typed 程序」）。踩坑：`--emit=rs-lz` 内置「生成 `.lzrs` → lang-zone → rustc → 运行 exe」递归自检，在 Windows 下工作文件名不可用 `in`（`in.exe` 运行报 `Os code 5 拒绝访问`，疑 Windows 保留/别名冲突），改用唯一哈希目录 + `probe.lz` 即稳定

### Option 语义族收口（2026-10-18，fist-agent lz-combo-r2 组合轮）
- `Int/F64.to_string()` 现推断为 `String`（原先因 match 顺序缺陷被 `IrType::Int/F64` 臂先行消费、回退 Any → `print` 走 Debug 加引号），`print(n.to_string())` 从 `"42"` 变 `42`（BUG-264）
- `List.get/first/last/pop` 与 `List.max/min` 的 IR 类型收口为 `Option<T>`（原先标 `T`，与 codegen 的 `.cloned()` → `Option<T>` 产物矛盾，同 BUG-281/BUG-316 语义漂移）（BUG-265、BUG-266）
- `List.max()/.min()` 在空 List 上不再 panic（原先生成 `(*recv.iter().max().unwrap())`，空集触发 `Option::unwrap on None`），改为 `.iter().max().cloned()` 保留 `Option<T>`，调用方自写 `.unwrap_or(...)`（BUG-266）
- `Dict.get(key)` 的 IR 类型收口为 `Option<V>`（原先标 `V`，与 codegen 的 `(d.get(k)).cloned()` → `Some(1)`/`None` 矛盾）（BUG-267）
- `Dict.pop(key)` 不再生成 `d.pop(key)`（BTreeMap/HashMap 无 `pop` 方法 → E0599），路由到 `d.remove(key)` 返回 `Option<V>`（BUG-268）
- 清理死代码：移除无人调用的 `list_get_unwrap_or_default`（BUG-281 遗留），build 警告 5→4

### Option / Bool 参与标量运算的 LZ 类型检查拦截（2026-10-19，fist-agent lz-combo-r2 组合轮）
- `Option<int> + int` 不再产出坏 Rust（原先 codegen 直出 `Option<i64> + i64` → E0369），`extended_check` Binary 臂补 `option_in_scalar_op`：算术/位/序比较内恰一侧为 Option 即报「需先解包」，`bool == bool` 与两侧皆 Option 的容器比较放行（BUG-269）
- `true + 1` / `true < 5` 不再产出坏 Rust（原先 `bool + i64` → E0369），同臂补 `bool_in_scalar_op`：任一侧为 Bool 即报「Bool 值不能直接参与运算」，`bool == bool` 放行（BUG-271）
- Bool 拦截带**比较链豁免**：`1 < 5 < 10` 解析为 `(1<5) < 10`（lhs 为 bool），codegen 才分解成 `(1<5) && (5<10)`；豁免使 `DEMO/13_operators/precedence.lz` 与 `CY/TESTS/15_feature_matrix/precedence.lz` 保持转绿（否则 cy_l1_transpile_all_corpus FAILED）
- 合法形态回归锁定：`f64 + int`（1.5+2=3.5）、`str + str` 拼接、`list + list` 拼接（`__cat`）均不受影响；`stdlib 自由函数（约 158 个）不接 rlib` 记为 BUG-270 待修（OPEN，潜伏、0 语料命中）
- 标量运算补类型族交叉拦截（BUG-272）：`两侧皆 Option 的算术`（`Option * Option`，收窄 BUG-269 的「两侧皆 Option 放行」到仅比较/Eq）、`浮点位/移位`（`1.5 << 2`）、`字符串跨族比较`（`"a" < 1`）、`Bool 跨族 Eq/Ne`（`true == 1`，Eq/Ne 不在 scalar_op，另开独立块）四类原先直出 E0369 的坏 Rust 现于 LZ 阶段拦截；合法形态（`Option 比较`、`list < list`、`bool == bool`、`f64 + int`）锁定不受影响；`None + 1` 因 None 落 `Any` 仍放行，记待评
- `None` 字面量参与算术/位拦截（BUG-273）：`None + 1` 生成 `LzAdd::__add__(None, 1i64)` → E0369，现于 LZ 阶段拦「None 值不能直接参与算术/位运算」。`None` 既可能是 `NoneLit` 也可能是 `Ident("None")`（词法层事实），且 `let None = 300` 可把 `None` 遮蔽为普通变量（`DEMO/boundary-coverage/edge-keyword-identifier.lz` 显式测此场景）——故 `Ident("None")` 仅当类型为 `Any`（prelude None 的推断类型）时按 None 处理，被遮蔽的 `None`（类型 Int）不拦，避免误伤遮蔽演示；`None == None` 因 codegen 不注入类型标注仍 RUSTC_FAIL，记 BUG-274 待修（OPEN）
- `None == None` / `None != None` 短路（BUG-274）：两侧皆裸 None 时 codegen 输出 `None == None` 无类型标注 → rustc E0282。`expr_gen` Binary 臂的 `x == None → x.is_none()` 守卫只覆盖「恰一侧 None」（`rhs_none != lhs_none`），两侧皆 None 时短路为布尔字面量——`Option<T>::None == Option<T>::None` 恒为 true/false 与内层类型无关，避免 E0282。`x == None` 形态仍走 `is_none()` 不受影响；遮蔽 `let None = 300` 下短路同样成立（`x == x` 恒 true）。回归用例 `scalar_op_type_family_cross_is_clean_lz_type_error` 增补 `None == None` / `None != None` 合法形态，期望输出 `true true true 3.5 false true false`

### 代码生成稳定性与标准库转译修复批次（2026-10-20 ~ 2026-10-22，BUG-341~408）
-CHANGELOG 此前记录止于 BUG-316；BUG-317~340 已在 `memory/bugs.md` 逐项记录，本段集中补齐其后（341~408）的条目。本批次跨一等函数/闭包语义、枚举/结构体/模式匹配、解析语法、rs-lz 后端四类，集中修复 LZ→Rust 转译产物编译失败与类型推断错误（共 58 个修复提交）：

**一等函数 / 闭包与 fn 值语义**
- BUG-389：一等 fn 值形参统一渲染为 `Arc<dyn Fn>`，打通 `apply(double,3)`（76689d4）
- BUG-389b：闭包返回闭包时注解用 `fn_value_type`（`Arc<dyn Fn>`）（92b2de6）
- BUG-393：impl `Fn(...)` 形参追加 `Clone` 上界，修复 LZ 值语义复用 fn 形参报 E0599（6f56c40）
- BUG-395/397：内置函数作一等 fn 值时还原引用形参（c7f33cf）
- BUG-396：泛型函数返回的闭包形参按返回签名标注（`compose |x: a|` 取代 `i64`）（bba8952）
- BUG-398：内置绑定到变量时用转发闭包保留引用形参（9f80e92）
- BUG-399：复用类型名的多义 clone 不再渲染成 `T::clone().Field`（9d2e6a9）
- BUG-400：自定义枚举含 `None` 变体时裸 `None` 解析为 `<枚举>::None`（3b3697d）
- BUG-392：impl 头自类型改用裸类型实参（去 bound/默认），修复 `struct<T: Clone = List<int>>` 生成非法 impl（a6741ef）
- BUG-386：间接 `chars()` 迭代变量保留 `char` 类型（f358366）
- BUG-362：curried closure 内层 `Arc` 包裹 + `Any` 形参注解（f9e16e7）

**枚举 / 结构体 / 模式匹配**
- BUG-407：命名字段枚举字段访问只匹配首个变体 → E0609 no field（cf29dbe）
- BUG-353：case struct 解构生成 Rust 枚举模式而非元组展开（39f88d2）
- BUG-354：struct 模式字段顺序错误（HashMap 迭代无序）（48b4e2c）
- BUG-352：列表切片 `xs[a..b]` 类型推断返回 `List<T>` 而非 `T`（c7602e6）
- BUG-344：`_variant` 魔法字段；BUG-342 花括号块表达式；BUG-343 mut struct 字段（4f9d5ec）
- BUG-374：struct 字段默认值——IR Field 携带 AST 默认值，StructCtor 补全缺失字段（706899e）
- BUG-341：支持嵌套元组解构 `let ((a,b),c) = ...`（f7c9bc7）
- BUG-345：支持 checker 实例化调用 `fn[checker] ~: args`（f1cc49b）
- BUG-347：允许 `check` 关键字作用户标识符（81897eb）
- BUG-346：`raise` 在表达式位置 → `Expr::Panic`（a28fc19）
- BUG-348：`spawn` 非 async 函数 await E0277 / 区分 async vs non-async spawn 参数（934ccaa、687301f）
- BUG-349：dict for-loop 迭代 keys（Python 语义）（8ce7519）
- BUG-355：List map/filter 类型推断缺失打断链式调用（73f5305）
- BUG-356/357：嵌套闭包非 Copy 外部 clone + 强制移动内层 lambda（d0f2185）
- BUG-358/359：闭包体类型构造器 + 裸 Vec/Option 上的迭代器适配（2651350）
- BUG-360：为普通非递归 struct 合成 `__unapply__`（add2a3f）
- BUG-381：自由函数 `zip` 补 `.collect::<Vec<_>>()`（615e36b）
- BUG-382：`IndexGet` else 分支非 Copy 元素自动补 `.clone()`（ea8e89c）
- BUG-380：跳过 `__unapply__` 合成须同时检查 methods（9eb54ad）
- BUG-379：`.` 行首续行需区分 duck 成员声明（0477fc3）
- BUG-375/376：`parse` / `.await()` method + async main 检测（d8acd46）
- BUG-377：`with open(...)` 补 `lz_builtins::open` 文件守卫（7b4f0e9）
- BUG-383/384/385：`.or()` / `.and()` / `|_|` / `int(char)` 修复（d7a61f6）
- BUG-387：single-arg `.reduce(closure)` 路由到 `functional::reduce`（eecbc05）
- BUG-390：本地 FnDef 登记 raises 升级为 Result；裸 raises 支持 `->` 形式（1fe7361）
- BUG-388：裸 raises（无类型）解析支持（6629577）
- BUG-408：rs-lz 关键字参数 `_KwArg` 未解包 → E0422 cannot find struct `_KwArg`（d92f6d3）

**rs-lz / emit 后端**
- BUG-404：`--emit=rs-lz` 补齐 struct 提取魔法方法 `__unapply__` / `__unapply_seq__`（83b34f7）
- BUG-405：类型名接收者的方法调用改用路径分隔符 `::`（枚举构造 E0423）（596b685）
- BUG-406：嵌套块尾部误发射 return → match/if/for 等提前返回（37d9191）

> **已知回归（待并发 codegen agent 收口，不在本批次范围）**：全量 DEMO 编译门禁 `all_demos_generated_rust_compiles` 当前暴露 7 个新增回归——`DEMO/07_data_structures/self_recursive.lz`(E0308)、`DEMO/10_error_handling/panic_raise_try.lz`(E0308)、`DEMO/boundary-coverage/nesting-closure-lambda.lz`(lifetime)、`DEMO/lz_std/dict.lz`(E0308)、`DEMO/lz_std/list.lz`(E0382 moved)、`DEMO/lz_std/result.lz`(E0308)、`DEMO/lz_std/traits.lz`(E0599 Option<J> clone)。这些属 codegen 在途改动引入，需并发 agent 修复，`[tco]` 调试打印已移除以恢复门禁 stderr 纯净。

### FIND_BUG 收尾修复（2026-09-24）
- `type Item` 仅在模块未声明 Iterator trait（落 std::iter::Iterator）时补发，修复 lib_iterator E0437（本地 trait 遮蔽后无 Item 成员）
- 字符串单字符索引在拼接/比较语境直出 String 形态（新增 `gen_str_char_index_tostring` 上下文辅助），修复 lib_json E0308 与 `s[pos] != "\""` 码点十进制化导致的运行期 UnexpectedEnd；find_bug_libs 12/12 全绿
- int_overflow 边界随 BigInt 字面量特性迁移 ACCEPTED_CASES（超大整数合法），移除 99_errors/00c_lexer_overflow.lz；全量 cargo test 0 failed

### FIND_BUG 挖掘套件入库与首轮修复（2026-09-03 ~ 09-04）
- FIND_BUG 挖掘套件入库 + lz_builtins 扩展（error/functional/std）+ 测试基建（4ca253d）
- ExternBridge + EmbedBridge 两大桥接落地 + 测试断言对齐（e11517d）
- FIND_BUG 首轮修复：语法核查 11 处修正（parser/IR/codegen/语义检查）（26cd418）
- FIND_BUG 全量实测：36 编号判定落档（✅12/❌21/🟡1）+ test_all.sh 基建修正（990af98）
- FIND_BUG 二轮收口：12 库基线 7/12 转正 + find_bug_bugs.rs 回归守护套件（15绿/25挂）（96f3797）
- 三轮复验：SB-001/002/003 经 codegen 修复转正（❌21→18），守护套件 18绿/22挂（d441940）

### codegen / lexer / parser / compiler 修复
- IR codegen 接入 StdBridge 方法映射，修 SB-001/002/003（d42ff8c）
- 顶层 self-def 归属 impl，修 BUG-CG-002/TY-002（E0568）（5ec6b64）
- Option 自动 Some 包装 + ?. 链 and_then 扁平化，修 SG-002/003（6631057）
- lexer 拒绝 i64 越界字面量，修 BUG-EC-002（避免静默环绕为 i64::MIN）（928bd46）
- parser 装饰器修饰非声明显式拒绝，修 BUG-PR-005（关闭 SILENT_PASS 负向漏洞）（da00cc7）
- BUG-SG-005 列表字面量 ... 展开运算符（040d704）
- BUG-EC-006 type_name() 内省返回真实类型名（5a5583d）
- BUG-PR-002 raises 与 -> 返回类型顺序无关（937af9a）

### 卫生
- 清理全部编译 warning，恢复 0-warning 基线（1ba5cd3）
- 修正 BUG-PR-003 判定措辞：..: nums: int 按规范非法（具名收集应写 nums: List<T>）（48eac6c）

> 第一批（09-03 ~ 09-04，15 commit）见上。

### magic 系统起步（2026-09-05 ~ 09-12，18 commit）

#### magic 系统 batch 1 + guard 策略
- magic system batch 1 — auto-mut infer + 5 magic traits（455b886）
- complete guard strategy, let-bridge, and ops fix（a4ad0a0）

#### magic trait impl 生成
- generalize trait-impl generation for operator magic methods P1-0/P1-1（51c0c34）
- complete *Assign/Ord/Hash/Drop/Default trait impl generation（21d069d）
- TryFrom/TryInto impl generation and custom __clone__ semantics（bffb23f）

#### magic 方法 wire
- wire __contains__/in, __iadd__/+= and Bool inference（aa6aed1）
- wire __invert__, __bool__ truth chain, with-statement constructor chain, __from__ → From impl（25b3ad2）
- __into__ → Into impl, __from__ implicit conversion at let sites, guard keyword param fix（95f5a15）
- gap magics __int__/__float__/__abs__ + __from__ implicit conversion at call args（7c8175b）
- __from__ return-site trigger (P1-2) + __cast__ dispatch for `x as T`（cddbaba）
- __try_cast__ fallible dispatch, __pos__/__deref__ unary magics, __from__ cycle detection（d107075）
- __implicit_from__ blanket + __into_iter__/__rev__ dispatch + .rev() semantics（3050f86）
- wire __implicit_copy__, __implicit_default__, __implicit_to__ traits（0734680）

#### magic docs/status + __init__ 修复
- docs: mark __unapply__ as ✅ (explicit impl on struct works)（0dd1da1）
- docs: correct __init__ status to 🔸 (body generated, call-site not auto-wired)（38a20ca）
- fix(codegen): __init__ 注入点仅当无额外参数时触发（754af9b）

#### 卫生
- 清理生成物并统一 IR 路线；补 for 解构测试与语法文档（000506f）
- clean PROBE temp files（5d59693）

> 本段为分批补条目第二批（09-05 ~ 09-12，18 commit）。剩余 09-12 后 ~65 commit 待后续轮次补。

### lzcyc 子编译器 CLI 落地与 M1.x 下沉 / codegen 修复 / 装饰器系统 / --emit=rs-lz / 卫生（2026-09-13 ~ 09-18，42 commit）

#### lzcyc 子编译器 CLI 落地与 M1.x 下沉
- 39025a3 feat: lzcyc 子编译器 CLI 落地 — transpile/compile/run 全链路
- b05e705 fix: lzcyc 运行期缺口兜底 + import 合并 + 主源码入库补漏
- b258aa1 feat: checker 派发兜底 — run 基线 53/54
- 31f8788 feat: 全绿基线 54/54 + 主编译器融入计划
- 0ea9858 Merge gitcode/feature/lzcyc-cli
- aefc10d Merge gitcode/feature/lzcyc-cli
- b5eec92 feat(cython): codegen 重构扩展 + _variant 下沉（M1.1）
- 2116beb feat(cython): Box/Rc/Arc 下标下沉（M1.2）
- 2b472ba feat(cython): Option 方法族下沉（M1.3）— 垫片退位
- 0eff34d fix(cython): 构建块元组解包下标修复（M1.4）
- 2910a27 feat(cython): 推导式 filter 谓词补调用下沉（M1.5）
- 9679488 fix(lzcyc): postprocess filter 分支退位（M1.5 配套）
- 082820b feat(cython): checker 派发下沉（M1.6）
- b236432 docs: 融入计划 M1 主体完成（6/7 下沉）
- 5750756 Merge origin/master into feature/m1-variant-sink

#### codegen / cython 修复
- 56f7ac1 fix(codegen): Eq/Neq 字符串对齐 — E0277 根治
- 4abcfbd fix(codegen): str 引用语义修复收尾（Ref(Str)→&str + &&str 防御）+ lz_std 基线登记
- 54919a8 fix(cython): gen_expr Lit 分支补全（merge 配套）
- cd86f33 fix(cython): 合并后补 complex/Int128/BigInt 字面量渲染（merge 配套）
- d5d21cf fix(codegen): 字符串索引取全绿基线版 — 修 ir_pass_fixes E0308/E0277
- 595c33e fix(codegen): 修复 3 个 codegen 缺陷
- d43d32a fix: 全绿基线 — lib_json/lib_string/lib_hashmap/lib_tree 编译错误消除
- 4c8c245 fix: wrap_ok logic, DictExt generics, parse_f64, string index
- ccb63e8 fix: 修复 lib_hashmap 字符索引类型不匹配
- 1e40069 feat: 修复 --emit=rs-lz golden 对比测试 - E0384/E0382 - auto_mut 切 List<str>→str

#### 装饰器系统推进
- 75eee02 fix: macro/template + 运算符双重语义 + 分层递归下降优先级解析
- 7a3ac49 fix: @memoize 约束检查 + @parallel 收窄 + @init async 分支
- 5615958 Merge gitcode/feature/lz-decorator-improve：6 个内置装饰器 + @case
- 3177627 feat: 新增 @case 装饰器（struct 双用法，自动配 __unapply__/__unapply_seq__）
- 89e85c1 feat: 实现 5 个内置装饰器 + 修复 @derive 参数 + 未知装饰器 fallback
- 2e67f34 feat(moddec): 修饰符装饰器全链路（P0 完成；P1 新轴 codegen 待续）
- 8dddb06 feat(moddec): T04 新语义轴 codegen（L4 转绿、@lazy 真惰性）
- ea5832c Merge gitcode/feature/lz-decorator-improve
- 60a3d9a Merge gitcode/master

#### --emit=rs-lz + moddec golden
- 6ce2ddc feat: 推进 D2 缺口 — 补 5 种 Item codegen + 打通 --emit=rs-lz
- 19ea60e chore(moddec): 补提 golden 快照 + .gitignore 例外

#### 卫生 / docs / 清理
- e739ff4 chore: clean build artifacts, fix absolute path in __file__/__path__
- 024b332 fix: 删除冗余的 grammars/lz 和 tree-sitter-lz 目录
- 12787fb fix: 合并 tree-sitter 查询文件并删除冗余目录
- 2d7b552 fix: 清理 15 个编译警告（unreachable pattern、unused variables/doc comments）
- c92c26b docs: 同步函数类型注解 fn(...) -> ... 语法规范到 SYNTAX 文档体系
- 2a869cc docs: 生成任务提示词 - lib_string 回归 + 编译警告清理

> 第三批（09-13 ~ 09-18，42 commit）见上。剩余 09-18 后 22 commit 由第四批补全。

### raises 全链路 / TryCatch 异常处理 / codegen 作用域与模式修复 / 字符串索引语义 / str→String 收尾 / Cython GenBuild / parser+docs（2026-09-19 ~ 09-23，22 commit）

#### raises 全链路透传
- 199cad2 feat(codegen): raises 支持全链路 — gen_expr fn_raises 透传 IfExpr/BlockExpr
- d89dd4f feat(codegen): expr_cs_list/gen_call 透传 fn_raises — 嵌套调用 raise 正确生成 return Err
- ca9070b fix(codegen): raises 函数 Unit 尾表达式/return 包 Ok — 移除 wrap_ok 的 Unit 排除

#### TryCatch 异常处理增强
- ac33b44 feat(codegen): TryCatch 全套增强 + Defer/CheckerBlock 骨架 + Item.CheckerBlock/Item.DuckDef 分支
- 5f514fc feat(codegen): TryCatch 多 catch 前置注释生成 — 跳过的 catch 输出 // catch <pattern> skipped
- 7d02a4f feat(codegen): TryCatch Result 基检测函数 + gen_stmt 注释标记接入
- 9460bb6 feat(codegen): gen_stmt BreakLabel/BlockLabel/WhileLet 分支实现

#### codegen 作用域与模式修复
- 36f5a4a fix(codegen): convert_ast_pattern Tuple/List/Dict filter_map → map + unwrap_or(Wildcard) 保留位置
- 5c28a92 fix(codegen): E0164 Variant args 保留 Wildcard + EnumCtor struct 模式 + lexer _ token
- dbeb540 fix(codegen): gen_stmt 补全 Yield/YieldFrom 分支对齐原生 codegen
- 2e556e6 fix(codegen): for 循环体 declared 快照恢复 — 修复块作用域变量泄漏
- 4d162e4 fix(codegen): while/while-let 循环体 declared 快照恢复 — 对齐 for 循环块作用域隔离
- 785f0e3 fix(codegen): compute_use_counts 补 StructDef 内联方法 — 修复 lib_hashmap E0382

#### 字符串单字符索引语义对齐
- 007fd9b feat(ir): 字符串单字符索引类型推断与比较对齐
- b6aba3e fix(codegen): 字符串单字符索引返回字节码 i64 — 移除 idx_wants_string 的 expr.ty 条件

#### str→String 类型修复收尾
- 448d133 fix(codegen): str → String 类型修复收尾
- 419949c fix: str→String 类型修复 (lz_codegen_lib.lz + lz_ir_lib.lz)
- 386d902 fix: 补充 stmt_uses_dict 剩余变体 + module_uses_dict 漏检修复

#### Cython GenBuild 代码生成
- ffe8e4a feat(cython): GenBuild 代码生成 + str 引用语义修复 + E0277 测试转正

#### parser / docs / test 卫生
- 33b5ca2 fix(parser): 支持 _ 作为函数参数名 — 修复 identifiers.lz 解析回归
- 6beae89 docs(readme): 状态段同步至 2026-09-23 — 补 09-05 后 magic/装饰器/lzcyc/raises 进展
- 2c87319 test(lz-infer): update ignore comments for pending inference capabilities

> 第四批（09-19 ~ 09-23，22 commit）见上。[Unreleased] 段已补全 09-03 ~ 09-23 共 97 commit（第一批 15 + 第二批 18 + 第三批 42 + 第四批 22）。

## [v0.1.180] - 2026-08-29

### 阶段 A 收口 + Result/Option 泛型桥接 + lib_iterator 转正（J1–J4）
- builder.rs 跨表示桥接：`let` 注解 `Named("Result"/"Option")` 与函数签名 Result/Option 变体互通 → 泛型绑定不再零绑定（修 lib_result and_then）
- `infer_generic_binding` 对 Named(Result/Option) 与 IrType::Result/Option 变体做显式配对推断
- FIND_BUG/lib_iterator：`collect/sum` 参数 `RangeIter` → `Iterator` 放宽，用例全链路转正（ignored 12→10）
- version.rs bump v0.1.165 → **v0.1.180**（此前 version.rs 滞后于 git 版本）
- 清理根目录临时调试文件（err*.txt / build_*.txt / b3/b4.txt 等 16 个），`.diffwork/`、`*.lzcache` 纳入 .gitignore
- 回归 514 passed / 0 failed / 10 ignored（基线 512 + iterator 转正 +2）；批测 325/325 正例 + 3/3 反例 100%

## [v0.1.179] - 2026-08-28

### 扩展语义检查器 + 负向防线 25/25（ERROR_BUG）
- builder.rs 新增 check_extended_semantics / ExWalker 全套扩展语义检查：fn 形参逐参类型检查、闭包实参与 fn 形参匹配、泛型替换（ex_subst）、duck 约束、match 穷尽性与分支类型一致性、未绑定捕获等
- ERROR_BUG 负向测试集 25 用例（8/24 基线 0/25 全漏报）→ **25/25 全部拦截**
- 新增 tests/error_bug_libs.rs 回归守护（任一用例被放行即红）
- 变体载荷解析优先限定名（Enum.Variant）+ scrutinee 枚举消歧：修复跨枚举同名变体（IrType.Tuple vs Pattern.Tuple）导致的误报（.lzlz 自举库实测暴露）
- 参数类型报错补调用方函数名（诊断改进）

### codegen 修复与自举 gate 三连修
- 字符串字面量转义 escape_default（json.lz E0308）；字符串字面量参数直接生成 &str；下标 as usize 括号；字符串字面量 raise 豁免 raises 声明（消息式错误，语义见 reject_more 更新）
- collect_unknown_extern_fns 将 Item::Use 导入名记为已知：修复增量拼接产物中未知函数桩与缓存模块真身重复定义（E0428，incremental_golden 自 v178 起存量失败）
- LZ_BUILTIN_FN_NAMES 补 print_str / print_val：修复 .lzlz 自举 gate（--emit=ir-lz）被 i64 桩遮蔽内置函数（E0308）
- 上述三修使 lz_ir_bootstrap 5/5 转绿、incremental_golden 转绿

### 找缺推进与测试基建
- FIND_BUG 12 库用例入库：全链路 0/12（阶段推进：解析层 fn 类型注解 5/5 修复、lib_pattern 至运行关）
- tests/find_bug_libs.rs 修正路径约定（按目录实际 .lz 定位）+ rlib 查找兑底；12 用例按失败阶段 #[ignore] 分级，修复一个转正一个
- tests/reject_more.rs 口径对齐：字符串 raise 免 raises 声明入 ACCEPTED_CASES 锁定，类型化 raise 未声明仍拒绝
- DEMO 下 124 个 .pyx 生成产物清理；自举/stdlib 中间产物入库（lz_ir_lib.rs 等）
- **全量回归 512 passed / 0 failed / 12 ignored（12 个 ignore 为 12 库转正队列）**，v178 存量隐藏失败（incremental_golden / lz_ir_bootstrap×3 / reject_more 口径）全部清零

## [v0.1.165] - 2026-08-20

### IR→rustc 通过率提升（74.9% → 76.6%，本轮修复）
- E0308 类型不匹配（p22_str_index / p27_opt_elif 类）：
  - codegen/mod.rs 新增 `current_expected_ty`（RefCell<Option<IrType>>）+ `option_none_elem()`，`Option::None` 类型参数选择优先实参期望类型 → 函数返回类型 → 表达式自身类型 → 默认 i64
  - 字符串单字符索引按当前函数返回类型生成：返回 str/String 时生成 `String`（char 安全，越界 `'\0'`），否则生成 char 码点 i64（兼容 string_index_unicode 场景）
- E0599 方法不存在（p24_slice_method 类）：内置 str/List 接收者的 `.slice(a, b)` 映射为 `lz_slice`（用户自定义 struct 的 slice 保留原样）
- E0605/E0308 String→数值（p41_full_tokenize 类）：`as int` 对 String/Any 类型走 `.parse()` 而非 Rust `as`（`as` 不允许 String→数值）
- 新增 tests/ir_pass_fixes.rs：4 用例全链路（lz→rs→rustc→run）覆盖上述四类修复
- 全量回归 511 tests passed / 0 failed（基线 507 + 新增 4）；未触碰 Cy 后端
- 增量重跑原 21 个 rustc 失败用例：修复 6 个（含 2 个 diffwork 噪音），IR→rustc 通过率 266/355 → 272/355（76.6%）
- 遗留：E0425 use/import/extern 作用域注入（7 例）+ E0609 Option 模式解构（1 例）+ diffwork 内部噪音（7 例），下轮继续

## [v0.1.164] - 2026-08-20

### G6 D2 codegen 补缺（impl 块 / 列表推导 / 生成器 / match）
- codegen 补齐四类语法 IR→rustc 生成：impl 块（inherent / trait / 泛型）、列表推导（多 for / guard / 嵌套 / 函数调用 / 集合 / 字典）、生成器（yield / yield from）、match（值表达式 / 元组模式 / 范围模式 / 守卫）
- semantic_check.rs 新增 check_impls 语义校验：impl 未知 trait、impl 目标类型不存在、trait 抽象方法缺失、impl 多余方法均拒绝；match guard 变量先 bind 再检查；多 for 推导 extra 变量预注册后校验 output/cond/key/value
- 新增 DEMO/g6/ 四类 demo（g6_impl / g6_listcomp / g6_generator / g6_match），编译 + rustc 运行验证
- 新增 tests/g6_codegen.rs：17 用例（正向 10 + 拒绝 7），覆盖四类特性运行与语义拒绝
- 全量回归 507 tests passed / 0 failed（基线 490 + 新增 17）
- 未触碰 Cy 后端（codegen_cython.rs / CY/）与无关文件

### I6 收口（2026-08-20，无代码修复，仅测量与文档）
- 差异校验：docs/I6-差异校验-2026-08-20.md（12 例代表性语料 + 1 综合绑定项目；Rust codegen 产物 vs 绑定输出（registry/ledger/PyO3 结构基准）结构、行为、映射全部一致，无修复项）
- IR→rustc 通过率重测：docs/IR-rustc-通过率重测-2026-08-20.md（355 例 evaluable → 266 通过，**74.9%**，旧数据 56.6% 已刷新；rustc 失败 21：E0308×10 / E0425×7 / E0599×2 / E0609×1 / E0605×1；lz 失败 68）
- 提升方向：E0308 类型收敛 → E0425 use/extern 作用域 → builtins 方法别名 → 语义误报收敛

## [v0.1.163] - 2026-08-19

### 桥接缺口补全 + G2 语义检测 + Cy 后端
- G4 bridge 接线：python.rs 正向桥接（Mojo 式 from python import shim + resolve/gen + 单测）
- G5 调用台账：ledger.rs 追加式 TSV + `lzc emit-bridge-report` CLI 审计
- G8 PyO3 依赖注入：pyo3 0.22.6 + py-bridge feature 隔离 + docs/py-bridge-struct.md（Cy 对齐基准）
- G7 embed 属性宏完整落地：`#[embed(rust)]`/`#[embed(py)]` lexer/parser 解析 → builder 语义展开（IntrinsicKind::Embed + 代码段字面量提取 + 诊断）→ codegen 原样内嵌 + registry 登记；DEMO/embed_demo.lz 编译运行验证（`hello from embed` / 42）
- I3/I4 接线收尾：extern/export 在解析生成时自动注册 BridgeRegistry（generate_with_bridge + CLI 注入 registry + ledger flush）；extern_demo 2 symbols、export_demo 1 symbol 运行验证
- G2 错误检测推进：semantic_check.rs 修复 is_bound 漏判内置类型名、builtin_type_names 扩充、FnSig param_count_min（默认参数）、泛型/arity 放宽、Never 返回类型豁免 raise 声明、BuildBlock 标识符先绑定；builder.rs 泛型调用无法推断时拒绝（neg_generic_missing_t）；ir_snapshots 43/43 全绿；syntax_probes 反例 25/25 全部拒绝
- 新增 tests/bridge_embed.rs：embed 运行生效 / embed 缺代码段拒绝 / extern 登记 2 / export 登记 1
- Cy 后端完成：src/ir/codegen_cython.rs + CY/ 规范 + tests/cython_backend.rs + DEMO/*.pyx 生成物
- 全量回归 490 tests passed / 0 failed（基线 486 + 新增 4）
- docs/补缺计划-2026-08-19.md（单一事实源主计划）；过时文档归档 docs/obsolete/

## [v0.1.162] - 2026-08-18

### Ext 类型 + extern L1 + G3 字符串切片 + FIST T4
- `#[extern(lang)]` 外部声明（L1）：lexer/parser 通用装饰器、Ext 类型、`__lz_ext_call` 分发器、ExtHandle
- G3 字符串切片 char 安全（chars().collect()），非 ASCII 不 panic
- FIST T4：incr.rs 模块级缓存+依赖图+级联失效+rayon、hotreload.rs watch、lsp.rs；IR 序列化 IR_MAGIC+IR_VERSION+ModuleDep
- 提交 a4a6a25 / ec37e78；全量回归 392 tests

## [v0.1.161] - 2026-08-17

### 稳定自举 + CLI 子命令集成

- 完成稳定自举（三代收敛）：宿主编译器处理自举源集 13 个 .lz，连续两代 .rs manifest 与运行输出逐字节一致
- 前端自举链：LZ 写的 `frontend_self.lz` 编译自身源码，三代收敛（f2.rs == f3.rs）
- 新增 CLI 子命令：`lz create` / `lz build`（含 `--incremental`）/ `lz peek` / `lz check` / `lz push`
- 语法特性矩阵：36 份 SYNTAX/ 文档 → 40+ 特性清单；DEMO 261/261；bootstrap closed 13/13 RC=0
- cargo test 全量 320/0（基线不回归）

### 已知缺口

- D2 codegen 缺口（impl 块/列表推导/生成器/match 模式）
- 行级覆盖率报告（cargo llvm-cov）待网络安装工具
- 跨平台安装包超出当前范围

## [v0.1.160] - 2026-08-17

### 自举 100% 里程碑

- 达成自举 100%（v160）：cargo test 320/0、DEMO 261/261、bootstrap closed 13/13 RC=0
- `--emit=rs-lz` 与 Rust codegen 逐字符一致

## [自举 50% 里程碑] - 2026-08-17

- 自举进度过半，前端自举链收敛验证通过

## [v0.1.x] - 2026-07-31 起

### IR 路线决策

- 全力走 IR 中间表示路线：代码生成统一以 LZIR 为中间层（AST → LZIR → 目标语言）
- 旧 `src/codegen/`（AST → Rust 直接 codegen）视为遗留，逐步退役
- 双后端：Rust（`lzc`）与 Cython/Python（`lzcyc`）
*（内容由AI生成，仅供参考）*
*（内容由AI生成，仅供参考）*

- float×int 混合比较提升（BUG-275）：\1.5 == 1\ 生成 \ == 1i64\（f64 == i64）→ rustc E0308。根因是 f64 提升只挂在 \rith\（Add/Sub/Mul/Div/Mod）分支，比较（Lt/Le/Gt/Ge）与 Eq/Ne 走不到 \({} as f64)\ 提升分支，且整数/浮点字面量不随 \current_expected_ty\ 改自身类型。新增 \
um_mixed = arith || is_comparison || Eq/Ne\ 让提升分支对比较也生效，\1.5 == 1 → 1.5f64 == (1i64 as f64)\。TEMP/probe/sweep3/f64_cmp_reg.lz 验证 1.5>2 false、3<2.5 false、1==1 true。
- str/List 重复（\*\）（BUG-276/277）：\"ab" * 2\ 生成 \String * i64\、\[1,2] * 2\ 生成 \Vec * i64\ → rustc E0369。根因是 Mul 未像 Add 那样做集合/字符串特判，str/List 侧落到 f64 提升后被当原生 \*\ 直出。在 Mul 分支前置拦截：str/String × int → \__rep.repeat(n)\、List/Vec × int → 循环 extend 重复，次数取 \max(0)\ 对齐 Python 语义（\"ab" * -1 → ""\、\[1] * 0 → []\）。\"ab"*2 → abab\、\[1,2]*2 → [1,2,1,2]\ 验证通过。

- type alias 元组索引（BUG-278）：\	ype Pair = (int, int); let p: Pair = (1,2); print(p[0])\ 生成 \p[0i64 as usize]\ → rustc E0608。根因是 \p: Pair\ 的 IR 类型是 Named("Pair")，IndexGet 的元组分支只匹配 IrType::Tuple(_)。CodeGen 增加 type_aliases 映射（模块级 gen_type_alias_def 与函数内 Stmt::TypeAlias 都登记），新增 resolve_type_alias 递归解析，IndexGet 元组判定改用解析结果。\p[0] → p.0\。
- 裸 Vec filter 方法闭包参数（BUG-279）：\[1,2,3].filter(|x| => x > 2)\ 生成 \\|x\| { x > 2i64 }\ → rustc E0308（Rust Iterator::filter 闭包参数是 &Item）。裸 Vec filter 分支套 strip_lambda_type_with_ref 加 & 模式（body 含 \*x\ 时保留裸参），map 分支保持 owned 参数不变。

- 裸 Vec 的 reduce/enumerate 方法路由（BUG-280/281）：\.reduce(\|acc,x\| => acc+x, 0)\ 生成 \.reduce(...)\ → E0599，改为 \into_iter().fold(init, closure)\；\.enumerate()\ → E0599、自由函数缺 \.collect()\ → E0608，改为 \into_iter().enumerate().map(\|(i,x)\| (i as i64, x)).collect()\，并在 infer.rs 登记返回类型 \List<Tuple(Int, T)>\。
- 元组索引后字段 clone（BUG-282）：\[0][1]\（b: List<(int, str)>）\let s = b[0].1\ → E0507（从借用 move String）。IndexGet 元组分支在 base 是 IndexGet 且字段非 Copy 时追加 \.clone()\，新增 is_copy_ty 辅助。

### 2026-10-19 - BUG-284/285/286：yield 嵌套与 Vec.next() 类型

- **BUG-284**：def evens(n): while ...: if i%2==0: yield i —— AST 预扫描
  st_body_has_yield / scan_iterator_yield_ty 未下钻 Stmt::Expr(Expr::If)
  分支体 → 元素类型回退 Unit → Vec<()>（E0599）。新增 st_expr_has_yield /
  scan_expr_yield_ty 下钻 If/Match/BlockExpr/Closure。
- **BUG-285**：match x: 0 => yield 100 ... —— IR 层 lock_has_yield /
  irst_yield_type 未下钻 Stmt::Match 各分支 Block → 返回类型 i64 且
  __gen_vec 未声明（E0425）。补 Stmt::Match 分支。
- **BUG-286**：生成器返回值 Vec<T>.next() 未在 lookup_builtin_method_ret
  登记 → 回退 Any → yield g.next() 归一成 i64 → Vec<i64> 却 push
  Option<i64>（E0308）。List/Vec 分支补 
ext → Option<elem>。

新增回归：generator_if_in_while / generator_match_arms / generator_yield_next_of_gen。
五门核验：corner 27 ✓、demo ✓、ir_snapshots ✓、cy_corpus ✓、perf_opt 44 ✓。- **BUG-287**：ewrite_iterator_returns 把生成器内所有 eturn 重写为
  Stmt::Raise(())（等价 Python StopIteration）。但 Stmt::Raise 的 codegen
  只要 in_generator 就 push(Err(..))，无视函数是否有 aises 子句 →
  非 raises 生成器 Vec<i64> 里 push Err(()) → E0308
  （TEMP/probe/sweep6/gen_return.lz）。修法：Stmt::Raise 在 in_generator
  时检查 current_fn_raises.is_some()——有 aises 才 push Err，否则直接
  eturn __gen_vec（提前结束收集）。

新增回归：generator_return_stops。全门核验：corner 30 ✓、demo ✓、
ir_snapshots ✓、cy_corpus ✓、perf_opt 44 ✓。- **BUG-288**：生成器 + raises 类型错配（3 处）：
  - `convert_decl.rs`：`def gen() raises str:` 无返回注解时 `ok_ty` 取
    `unwrap_or(Unit)` → `Vec<Result<(),String>>` 与 `push(Ok(1i64))` E0308。
    修法：无注解时取 `scan_iterator_yield_ty` 推断出的 `ret_ty`。
  - `mod.rs`：生成器 + raises 应为 `Vec<Result<T,E>>`（每次 yield 一个
    Result），旧逻辑先包 `Vec<T>` 再包 `Result` → `Result<Vec<T>,E>`，
    调用点 `let g: Result<Vec<T>,E>` 与实际 codegen 的 `Vec<Result<T,E>>`
    不符 → `g.next()` E0599。
  - `current_is_iterator` 只按 `func.is_iterator` 设置，漏掉 body 含 yield
    的函数 → `return` 值触发隐式转换（`"boom"` → `i64`），随后
    `rewrite_iterator_returns` 转成 `Stmt::Raise` 时值已被污染 →
    `push(Err(<i64 as ImplicitFrom<String>>::__implicit_from__("boom")))`
    E0277。修法：`current_is_iterator = func.is_iterator ||
    ast_body_has_yield(&func.body)`。

新增回归：generator_raises、generator_raises_return。全门核验：corner 32 ✓、
demo ✓、ir_snapshots ✓、cy_corpus ✓、perf_opt 44 ✓。- **BUG-289**：生成器内 try/catch yield 漏检（3 处）：
  - `helpers.rs block_has_yield`：无 `Stmt::TryCatch` 分支 → codegen 生成
    `-> Itor<i64>` 而非 `-> Vec<i64>`，且 `__gen_vec` 未声明 → E0425。
  - `helpers.rs first_yield_type`：无 `Stmt::TryCatch` 分支 → 生成器构建块
    收集器元素类型推断漏检。
  - `convert_decl.rs ast_expr_has_yield / scan_expr_yield_ty`：无
    `AstExpr::TryCatch` 分支 → `scan_iterator_yield_ty` 漏检。
  修法：三处均补 `Stmt::TryCatch` / `AstExpr::TryCatch` 分支，递归下钻
  body/catches/else_body/finally_body。新增回归 generator_try_yield。- **BUG-290**：生成器 `yield None; yield Some(1)` 元素类型推断错。
  `scan_iterator_yield_ty` 对 `NoneLit` 返回 `Any` → 归一 `i64` →
  `Vec<i64>` 却 `push(Option::None)` E0308。修法：yield None 返回
  `Option(Any)`（归一 `Option<i64>`），生成器返回类型 `Vec<Option<i64>>`。
  新增回归 generator_yield_none。- **BUG-292**：生成器 `if true: yield 1; return` 分支体内 `return` 未重写为
  `raise`。LZ 语句位置的 if 是 `Stmt::ExprStmt` 包裹 `ExprKind::IfExpr`，
  原 `ir_block_has_yield` / `rewrite_iterator_returns` 均未下钻
  `Stmt::ExprStmt` → codegen 生成 `panic!("generator return")`。
  修法：两处均补 `Stmt::ExprStmt` 分支，经新增 `ir_expr_has_yield` /
  `rewrite_expr_returns` 递归下钻 `ExprKind::IfExpr` / `BlockExpr`。
  新增回归 generator_if_return。- **BUG-293**：raises 生成器 `if: yield "ok"; return` 裸 `return` 重写为
  `raise ()`（value=Unit），codegen 一律 `push(Err(..))` →
  `push(Err(()))` 与 `Vec<Result<String,String>>` E0308。修法：
  `Stmt::Raise` 生成器分支仅当 raises 存在且 value 非 Unit 才 push Err，
  Unit 视为成功终止。新增回归 generator_raises_if_return。- **BUG-294**：struct impl 方法生成器 `let g = c.gen(); g.next()`，
  IR 侧接收者类型为 Any → codegen .next() 特判 recv_is_list=false 未转
  `into_iter().next()` → E0599。修法：.next() 特判增加 Any 接收者。
  新增回归 generator_struct_method。