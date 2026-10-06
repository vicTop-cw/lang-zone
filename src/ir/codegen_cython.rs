// LZIR → Cython 代码生成器（子编译器）
// 任务：将 IrModule 转换为合法的 Cython (.pyx) 源代码
// 覆盖全部 Stmt / ExprKind / Pattern 变体
//
// 设计原则：
// 1. 对象一律 PyObject：容器/泛型位置用 object；函数签名位置优先 C 类型（性能）
// 2. 对齐 PyO3 结构：#[pyclass]→cdef class, #[pymethods]→类内方法, #[pyfunction]→def
// 3. 函数声明统一用 def（Cython 完全支持 C 类型参数标注）——cdef 函数无法被
//    Python 层访问且限制多（不能 yield / 不能动态特性），cpdef 编译开销大且对
//    含默认参数/泛型擦除场景易出错。性能标量提升留待 I10 阶段。
// 4. 错误处理对齐 PLAN.md §1.6：Result→异常传播（Err(e)→LZError(e)、raise→raise、
//    try/catch→try/except）；Option→None 哨兵
// 5. 验证铁律：生成的 .pyx 必须能过 cythonize 编译且运行正确

use super::node::*;
use super::types::IrType;
use super::IrModule;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// `check` 块派发用的 `__Params` / `_LzKwargsMap` 垫片文本。
/// **两处发射共用这一份**（`generate()` 尾部追加、`postprocess_pyx` 的插入路径）——
/// 原先是两份逐字副本，改一处漏一处，正是这种文本最容易被读出「后端之间口径不同」。
///
/// `__contains__` 是必填项，不是锦上添花：LZ 的 `ps.kwargs.contains(k)` 在本后端和
/// str/list/dict 走同一条改写成 `k in ps.kwargs`；wrapper 只发 `__getitem__` 时，
/// Python 的 `in` 会退化成迭代协议，从 `self._d[0]` 开始试 ⇒ `KeyError: 0`
/// （实测语料 12_build_blocks/checker_call.lz 的 L3 运行异常）。
const PARAMS_SHIM: &str = concat!(
    "class _LzKwargsMap:", "\n",
    "    def __init__(self, d): self._d = dict(d or {})", "\n",
    "    def contains(self, k): return k in self._d", "\n",
    "    def __contains__(self, k): return k in self._d", "\n",
    "    def __len__(self): return len(self._d)", "\n",
    "    def __iter__(self): return iter(self._d)", "\n",
    "    def items(self): return self._d.items()", "\n",
    "    def __getitem__(self, k): return self._d[k]", "\n",
    "", "\n",
    "class __Params:", "\n",
    "    def __init__(self, kwargs=None, args=None):", "\n",
    "        self.kwargs = _LzKwargsMap(kwargs)", "\n",
    "        self.args = list(args or ())",
);

/// 类型映射上下文（决定同一 IrType 在不同位置映射为何种 Cython 类型）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypeCtx {
    /// 函数签名位置（参数/返回值）—— 优先 C 类型以获得性能
    Signature,
    /// 容器/泛型位置（List<T>, Dict<K,V> 的 T/K/V）—— 一律 object（PyObject）
    Container,
    /// 泛型参数位置 —— 运行时擦除为 object
    Generic,
    /// 结构体字段位置 —— 可用 C 类型（cdef public）
    Field,
    /// 局部变量位置 —— 推断可用 C 类型
    Local,
}

pub struct CythonCodeGen {
    indent: usize,
    buf: String,
    // ── 上下文状态 ──
    /// 所有用户自定义类型名（struct/enum/impl）
    known_types: HashSet<String>,
    /// cdef class 名集合（用于判断类型名是否需要 cdef class 引用）
    cdef_classes: HashSet<String>,
    /// 当前所在的 cdef class 名（用于 Self_ 解析）
    current_class_name: Option<String>,
    /// 函数名 → 重载签名列表（参数类型向量）
    overload_sigs: HashMap<String, Vec<Vec<IrType>>>,
    /// **实际发射**的同名函数：原名 → [(发射用的 mangle 名, 参数个数)]，按发射顺序。
    /// 分发器只能引用这里登记过的名字：`overload_sigs` 是签名登记表，同名同签名的
    /// 重复定义（`import` 平铺合并会把被导入模块的 `main` 也拉进来）会在它里
    /// 撑出两条完全一样的条目，照它发分发器就得到 `return main__1` 这种
    /// 从未发射的名字（cython: undeclared name not builtin: main__1）。
    /// 用 BTreeMap：分发器发射顺序必须与 HashMap 随机种子无关，否则同一份
    /// 源码头对尾两次转译出的 .pyx 不同 ⇒ 任何逐字节对比都会偶发红。
    emitted_fns: BTreeMap<String, Vec<(String, usize)>>,
    /// 已被 CLI **平铺合并**进本模块的 import 模块名（`merge_imported_modules`）。
    /// 这些模块在 Python 侧不存在（定义已内联进同一个 .pyx），所以：
    /// `import lz_std` 行不发射、`lz_std.Some(...)` 的限定前缀剥成 `Some(...)`。
    /// 口径对齐 Rust 后端（同一份 IR 它发的是裸 `Some(42i64)`）。
    /// 原 postprocess 第 6 条的生成期版本；lzcyc 仍走它自己那份 postprocess 副本。
    merged_modules: HashSet<String>,
    /// 自定义 enum 变体字段名映射："Enum.Variant" → [字段名]（位置字段命名为 f0/f1/…）
    variant_fields: HashMap<String, Vec<String>>,
    /// 定义了 __new__ 魔术构造的 struct 名集合（调用点路由到 `Name__new__(...)`）
    has_new_structs: HashSet<String>,
    /// impl 方法收集：目标类型名 → 方法列表（gen_struct 时注入到 cdef class 体内）
    impl_methods: HashMap<String, Vec<FnDef>>,
    /// self-def 顶层函数收集：`Point.sum` 形态 → owner 类型 → 方法列表
    /// （对齐 Rust 端 gen_self_fn_impls：生成时注入到 cdef class 体内）
    self_methods: HashMap<String, Vec<FnDef>>,
    /// 自定义 enum 名集合（`Enum.Variant` 访问形态识别用）
    enum_names: HashSet<String>,
    /// enum 名 → 变体名集合
    enum_variants: HashMap<String, HashSet<String>>,
    /// 函数形参名表（checker 派发包装器生成用：位置实参 zip 进 kwargs）
    fn_params: HashMap<String, Vec<String>>,
    /// checker 派发包装器文本（gen_expr 内收集，generate 收尾统一追加；
    /// gen_expr 持有 &cg 故用 RefCell 内部可变）
    checker_wrappers: std::cell::RefCell<Vec<String>>,
    /// 当前函数返回类型（尾表达式 return 转换 / 尾部块处理用）
    current_fn_ret_ty: Option<IrType>,
    /// 当前函数是否 `async def`（`go`/`spawn` 的分派依据：async 上下文走协程 `__go`，
    /// 同步上下文必须走线程 `__spawn`——对齐 rust 端 current_fn_is_async 的同一分派）
    current_fn_is_async: bool,
    /// 模块级变量名（Const + 顶层 let）：函数内赋值这些名字需要 global 声明
    module_var_names: HashSet<String>,
    /// 语句性 Lambda（体为赋值）的 def 映射：lambda key → def 名
    lambda_map: HashMap<String, String>,
    /// `ref` 绑定的**发射期别名**：引用名 → 它指向的位置文本（`r` → `x`）。
    /// Cython 里没有引用类型，旧实现把 `ref r = x` 发成 `r = x`（拷贝）⇒ 之后写 `r`
    /// 不回写 `x`，读到的还是旧值：`ref_binding.lz` 实测 cy 出 `100,42` 而 oracle 出
    /// `100,100`（**静默错值**，不报错）。登记别名后，`r` 的每次读与每次写都改写回
    /// `x` 这一位置，与 Rust 的 `&mut` -place 语义对齐。每个函数体开始时清空（跨函数
    /// 同名引用不得互相冒名）。已知不覆盖：把引用作为 `ref` 形参传给被调函数后再由
    /// 被调方写回——Python 传值无法回传，见 memory/bugs.md 的同名缺陷卡。
    ref_alias: HashMap<String, String>,
    /// 待注入函数体的语句性 Lambda def 文本
    pending_lambda_defs: Vec<String>,
    /// impl 目标类型是否为已知自定义类型（用于 gen_impl 决定是否已注入）
    impl_injected: HashSet<String>,
    /// defer 延迟执行栈：每进入一个块压入一层，块尾逆序（LIFO）内联 flush（对齐 Rust 端）
    deferred: Vec<Vec<Block>>,
    /// 临时变量计数（match scrutinee 等唯一名）
    tmp_counter: usize,
    /// 是否在生成器构建块内（控制 yield 语义转换）
    in_gen_build: bool,
    /// `--test` 执行面开关：为真时在文件尾发射 `_lz_run_tests()` 与 `__main__` 入口
    /// （BUG-18；缺这一段时 `lzc --backend=cython --test` 一个 test 都不执行）
    test_runner: bool,
    /// 已发射的 test 函数名（声明顺序；runner 只能引用这里登记过的名字）
    test_names: Vec<String>,
}

impl CythonCodeGen {
    pub fn new() -> Self {
        CythonCodeGen {
            indent: 0,
            buf: String::new(),
            known_types: HashSet::new(),
            cdef_classes: HashSet::new(),
            current_class_name: None,
            overload_sigs: HashMap::new(),
            emitted_fns: BTreeMap::new(),
            merged_modules: HashSet::new(),
            variant_fields: HashMap::new(),
            has_new_structs: HashSet::new(),
            impl_methods: HashMap::new(),
            self_methods: HashMap::new(),
            enum_names: HashSet::new(),
            enum_variants: HashMap::new(),
            fn_params: HashMap::new(),
            checker_wrappers: std::cell::RefCell::new(vec![]),
            current_fn_ret_ty: None,
            current_fn_is_async: false,
            module_var_names: HashSet::new(),
            lambda_map: HashMap::new(),
            ref_alias: HashMap::new(),
            pending_lambda_defs: vec![],
            impl_injected: HashSet::new(),
            deferred: Vec::new(),
            tmp_counter: 0,
            in_gen_build: false,
            test_runner: false,
            test_names: Vec::new(),
        }
    }

    /// 登记被平铺合并进本模块的 import 模块名（CLI 的 `merge_imported_modules` 调用）。
    pub fn set_merged_modules(&mut self, names: impl IntoIterator<Item = String>) {
        self.merged_modules = names.into_iter().collect();
    }

    /// `--test` 执行面：打开后在文件尾发射 `_lz_run_tests()` 与 `__main__` 入口。
    /// 关掉时**一行都不发**——纯转译产物不许夹带运行器（否则 L3 的 stdout 会被测试行污染，
    /// 跨后端 golden 立刻分叉）。
    pub fn set_test_runner(&mut self, on: bool) {
        self.test_runner = on;
    }

    /// 剥掉已合并模块的限定前缀：`lz_std.Some` → `Some`；未合并的名字原样返回。
    fn strip_merged_qualifier(&self, name: &str) -> String {
        match name.split_once('.') {
            Some((head, rest)) if self.merged_modules.contains(head) => rest.to_string(),
            _ => name.to_string(),
        }
    }

    pub fn generate(&mut self, module: &IrModule) -> &str {
        self.buf.clear();
        self.writeln("# Generated by lzcyc (IR backend)")
        ;
        self.writeln("# cython: language_level=3");
        self.writeln("# cython: boundscheck=False");
        self.writeln("");
        // ── 预扫描：收集所有自定义类型名 / 变体字段 / has_new / impl 方法 / 重载 ──
        self.prescan_module(module);
        self.writeln("import cython");
        self.writeln("import sys");
        self.writeln("import threading as _lz_threading");
        self.writeln("");
        // 运行时哨兵：move 检查
        self.writeln("class _Moved:");
        self.writeln(
            "    def __getattr__(self, name): raise RuntimeError('access to moved value')",
        );
        self.writeln("    def __bool__(self): raise RuntimeError('use of moved value')");
        self.writeln("    def __repr__(self): return '<_Moved>'");
        self.writeln("_MOVED = _Moved()");
        self.writeln("def _MovedCheck(v): raise RuntimeError('use of moved value')");
        self.writeln("");
        // 运行时：LZ 通用错误包装（Err(e) 构造 / raise 非异常值落点）
        self.writeln("class LZError(Exception):");
        self.writeln("    def __init__(self, val=None):");
        self.writeln("        self.val = val");
        self.writeln("        super().__init__(str(val))");
        self.writeln("");
        // 运行时：标签 break 模拟（BlockLabel/BreakLabel 异常脱糖）
        self.writeln("class _BreakLabel(Exception):");
        self.writeln("    def __init__(self, label, value=None):");
        self.writeln("        self.label = label");
        self.writeln("        self.value = value");
        self.writeln("        super().__init__(label)");
        self.writeln("");
        // 运行时：智能指针包装（Box/Rc/Arc → PyObject 句柄，属性访问透传内部值）
        // 用户自定义同名类型时跳过（避免与 cdef class 名冲突）
        for name in ["Box", "Rc", "Arc"] {
            if self.known_types.contains(name) {
                continue;
            }
            self.writeln(&format!("class {}:", name));
            self.writeln("    def __init__(self, v=None): self._v = v");
            self.writeln(&format!("    @staticmethod"));
            self.writeln(&format!("    def new(v=None): return {}(v)", name));
            self.writeln("    def __getattr__(self, n): return getattr(self._v, n)");
            // 下标协议：`x[0]` 取内部值 / `x[0] = v` 存内部值（指针解引用语义）
            self.writeln("    def __getitem__(self, i): return self._v");
            self.writeln("    def __setitem__(self, i, v): self._v = v");
            self.writeln(&format!("    def __repr__(self): return '{}(' + repr(self._v) + ')'", name));
            self.writeln("");
        }
        // 运行时：Option/Result 命名空间（Option.None/Some、Result.Ok/Err 伴随访问）
        self.writeln("class Option:");
        self.writeln("    @staticmethod");
        self.writeln("    def Some(v): return v");
        self.writeln("    None_ = None");
        self.writeln("");
        self.writeln("class Result:");
        self.writeln("    @staticmethod");
        self.writeln("    def Ok(v): return v");
        self.writeln("    @staticmethod");
        self.writeln("    def Err(e): return LZError(e)");
        self.writeln("");
        // 运行时：内建函数（对齐 lz_builtins 子集；map/filter 为 LZ 参数顺序）
        // go/spawn 走两条路，分派依据 = 当前函数是否 async（与 rust 端 current_fn_is_async 同形）：
        //   async 上下文 → `__go(f(...))`：调用 async def 只**创建**协程、不执行体，
        //     透传就是正确语义（跨后端实测一致的那件：CY/TESTS/15_feature_matrix/async_spawn.lz）。
        //   同步上下文 → 发射点改发 `__spawn(lambda: 体)`（见 gen_expr 的 Call 分支）。
        //     Python 的实参先求值，`__go(体)` 这个形状**结构上**不可能延后体的执行——
        //     这正是 BUG-12 记的「并发整体丢失、stdout 恰好等于顺序程序」的根因。
        self.writeln("def __go(v): return v  # async 上下文：v 是协程对象，await 时才执行");
        self.writeln("def __spawn(__fn):");
        self.writeln("    def __run():");
        self.writeln("        try:");
        self.writeln("            __fn()");
        self.writeln("        except BaseException as __e:");
        self.writeln(
            "            print('task failed: %s: %s' % (type(__e).__name__, __e), file=sys.stderr)",
        );
        self.writeln("    __t = _lz_threading.Thread(target=__run)");
        self.writeln("    __t.start()");
        self.writeln("    return __t");
        self.writeln("def __lz_expect_fail(m): raise LZError(m)");
        self.writeln("def push(seq, item):");
        self.writeln("    seq.append(item)");
        self.writeln("    return item");
        self.writeln("def pop(seq):");
        self.writeln("    return seq.pop()");
        self.writeln("def map(seq, f):");
        self.writeln("    return list(map(f, seq))");
        self.writeln("def filter(seq, f):");
        self.writeln("    return list(filter(f, seq))");
        self.writeln("def collect(it):");
        self.writeln("    return list(it)");
        self.writeln("def reverse(seq):");
        self.writeln("    return list(reversed(seq))");
        self.writeln("def sort(seq):");
        self.writeln("    return sorted(seq)");
        self.writeln("def fold(seq, init, f):");
        self.writeln("    acc = init");
        self.writeln("    for x in seq:");
        self.writeln("        acc = f(acc, x)");
        self.writeln("    return acc");
        // print 的 Debug 口径（对齐 Rust 后端的 `println!("{:?}", x)`）：
        // 字符串加引号并转义、bool→true/false、None→None、list/tuple/dict 递归，
        // 自定义类走本后端发的 `__repr__`（见 debug_repr_lines），其余交 repr()。
        // 名字必须单下划线开头：`__lz_dbg` 在 class 体内引用会被 Cython 按
        // Python「class private name」规则告警（实测 3.2.2 每条 `__repr__` 三遍），
        // 且声明"未来可能改成真改写"⇒ 双下划线是个会自己长大的雷。
        self.writeln(r#"def _lz_dbg(v):"#);
        self.writeln(r#"    if v is True: return "true""#);
        self.writeln(r#"    if v is False: return "false""#);
        self.writeln(r#"    if v is None: return "None""#);
        self.writeln(r#"    if isinstance(v, str):"#);
        self.writeln(
            r#"        return '"' + v.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t") + '"'"#,
        );
        self.writeln("    if isinstance(v, (list, tuple)):");
        self.writeln(r#"        o, c = ("[", "]") if isinstance(v, list) else ("(", ")")"#);
        self.writeln(r#"        s = ", ".join(_lz_dbg(x) for x in v)"#);
        self.writeln(r#"        if not isinstance(v, list) and len(v) == 1: s += ",""#);
        self.writeln(r#"        return o + s + c"#);
        self.writeln(r#"    if isinstance(v, dict):"#);
        self.writeln(
            r#"        return "{" + ", ".join(_lz_dbg(k) + ": " + _lz_dbg(x) for k, x in v.items()) + "}""#,
        );
        // catch 到的错误对象：Rust 侧 `e` 是 String ⇒ `{:?}` 出 `"error"`；
        // 本侧是 LZError 实例，repr() 会出 `LZError('error')`（实测 keywords.lz 跨后端分叉）。
        // 取 str(e) 再走一遍 Debug 口径，两侧逐字一致。
        self.writeln("    if isinstance(v, BaseException):");
        self.writeln("        return _lz_dbg(str(v))");
        self.writeln(r#"    return repr(v)"#);
        self.writeln("");
        // str 索引的元素口径：码点整数、越界（含负索引）给 0——与 Rust 后端逐字对齐，
        // 理由与判定见 gen_expr 的 IndexGet 分支注释。名字单下划线开头，同 `_lz_dbg`。
        self.writeln(r#"def _lz_idx_str(s, i):"#);
        self.writeln(r#"    return ord(s[i]) if 0 <= i < len(s) else 0"#);
        self.writeln("");
        // f-string 插值口径：**str 走 Display（不带引号），其余走 `_lz_dbg`**。
        // 规范行：SYNTAX/00-词法基础.md:188 `f"x={x}"` → `format!("x={}", x)`；
        // 实跑判据 tests/str_boundary.rs 的 c7_fstring（`assert msg == "name=LZ, age=30"`）。
        // Rust 侧按 `str_typed_vars` 登记表**静态**分档，这里按运行时类型分档——同一份
        // 源码两侧同形，且不必让 cy 后端复制一张登记表。bool/None/容器仍走 Debug，
        // 否则 Python 原生 `f"{True}"` 出 `True` 而 Rust 侧出 `true`（实测跨后端分叉）。
        self.writeln(r#"def _lz_fs(v):"#);
        self.writeln(r#"    return v if isinstance(v, str) else _lz_dbg(v)"#);
        self.writeln("");
        // 模块级魔法
        let fname = module.name.replace("::", ".");
        self.writeln(&format!("__name__ = \"{}\"", fname));
        self.writeln(&format!("__file__ = \"{}.lz\"", fname));
        let all_items: Vec<String> = module
            .items
            .iter()
            .filter_map(|i| match i {
                Item::FnDef(f) => {
                    // self-def（`Type.method`）不是模块级属性，不进 __all__
                    if f.name.contains('.') {
                        None
                    } else {
                        Some(f.name.clone())
                    }
                }
                Item::StructDef(s) => Some(s.name.clone()),
                Item::EnumDef(e) => Some(e.name.clone()),
                Item::Const(c) => Some(c.name.clone()),
                Item::TypeAlias(ta) => Some(ta.name.clone()),
                _ => None,
            })
            .collect();
        self.writeln(&format!(
            "__all__ = [{}]",
            all_items
                .iter()
                .map(|n| format!("\"{}\"", n))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        self.writeln("");

        let mut has_main = false;
        for item in &module.items {
            if let Item::FnDef(f) = item {
                if f.name == "main" {
                    has_main = true;
                }
                // self-def（`Type.method`）已由 prescan 注入目标类型，主循环跳过
                if f.name.contains('.') {
                    continue;
                }
            }
            self.gen_item(item);
        }
        // 生成函数重载分发器
        self.gen_overload_dispatchers();
        // 如果没有 main 函数，添加默认空 main
        if !has_main {
            self.writeln("def main():");
            self.indent += 1;
            // 顶层语句放入 main 函数体（BUG-PR-001 对齐：顶层表达式需被执行）
            if !module.top_level_stmts.is_empty() {
                gen_block(
                    self,
                    &Block {
                        stmts: module.top_level_stmts.clone(),
                        ty: IrType::Unit,
                        span: super::node::Span::unknown(),
                    },
                    false,
                );
            } else {
                self.writeln("pass");
            }
            self.indent -= 1;
        }
        // checker 派发包装器：__Params 垫片 + 包装器统一追加在文件尾
        // （模块级 def 先于任何调用执行，尾部安全；仅在有 checker 调用时生成）
        if !self.checker_wrappers.borrow().is_empty() {
            let wrappers: Vec<String> = self.checker_wrappers.borrow().clone();
            self.writeln("");
            for line in PARAMS_SHIM.lines() {
                self.writeln(line);
            }
            self.writeln("");
            for w in wrappers {
                self.writeln(&w);
                self.writeln("");
            }
        }
        if self.test_runner {
            // `--test`：测试运行器必须在**所有** def 之后（含上面追加的 checker 包装器），
            // 因为 _lz_cases 在模块级引用 test_* 名字。
            self.emit_test_runner();
        }
        &self.buf
    }

    /// 完整管线：generate + postprocess_pyx（共享后处理，下沉自 lzcyc）
    pub fn generate_full(
        &mut self,
        module: &IrModule,
        enum_variants: &[(String, usize)],
        merged_modules: &[String],
    ) -> String {
        let raw = self.generate(module).to_string();
        postprocess_pyx(&raw, enum_variants, merged_modules)
    }

    fn gen_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => self.gen_function(f),
            Item::StructDef(s) => self.gen_struct(s),
            Item::EnumDef(e) => self.gen_enum(e),
            Item::Const(c) => self.gen_const(c),
            Item::TypeAlias(ta) => self.gen_type_alias(ta),
            Item::Use(u) => self.gen_import(u),
            Item::TraitDef(t) => self.gen_trait(t),
            Item::Impl(i) => self.gen_impl(i),
            Item::Test(t) => self.gen_test(t),
            Item::CheckerBlock {
                name,
                ps_name,
                default_checker,
                body,
                captured,
            } => {
                self.gen_checker_block(
                    name,
                    ps_name.as_deref(),
                    default_checker.as_deref(),
                    body,
                    captured,
                );
            }
            Item::DuckDef(d) => self.gen_duck_def(d),
            Item::EmbedBlock { lang, src, .. } => {
                self.writeln(&format!("# #[embed({})] — Cython 后端降级为注释", lang));
                for line in src.lines() {
                    self.writeln(&format!("# {}", line));
                }
            }
        }
    }

    /// 函数签名渲染（供 gen_function / gen_method / __lz_new 复用）
    /// 返回 (参数列表字符串, 体首注释列表)
    fn render_params(&mut self, f: &FnDef, is_method: bool) -> (Vec<String>, Vec<String>) {
        let mut p: Vec<String> = vec![];
        let mut head_comments: Vec<String> = vec![];
        for param in &f.params {
            if param.variadic {
                // ..name: Tuple<T> → *args; ..name: Dict<K,V> → **kwargs
                let is_kwargs = match &param.ty {
                    IrType::Named { path, .. } => matches!(path.as_str(), "Dict" | "dict" | "HashMap"),
                    _ => false,
                } || param.name == "kwargs" || param.name.ends_with("_kwargs");
                let elem_hint = if is_kwargs {
                    format!("**{}", param.name)
                } else {
                    format!("*{}", param.name)
                };
                p.push(elem_hint);
                continue;
            }
            if is_method && (param.name == "self" || param.ty == IrType::Self_) {
                p.push("self".into());
                continue;
            }
            let ty_str = self.map_type(&param.ty, TypeCtx::Signature);
            if param.comptime {
                head_comments.push(format!("# comptime param: {}", param.name));
            }
            if !param.mods.is_empty() {
                head_comments.push(format!(
                    "# param {} mods: static={}, shared={:?}, interior={:?}, lazy={}, source={}",
                    param.name,
                    param.mods.storage_static,
                    param.mods.shared,
                    param.mods.interior,
                    param.mods.lazy,
                    param.mods.source
                ));
            }
            let mut param_str = format!("{} {}", ty_str, param.name);
            if let Some(default_val) = &param.default {
                param_str.push_str(&format!(" = {}", gen_expr(self, default_val)));
            }
            p.push(param_str);
        }
        // checker_param：追加可选参数
        if let Some(ps_name) = &f.checker_param {
            p.push(format!("{}=None", ps_name));
            head_comments.push(format!("# checker param: {}", ps_name));
        }
        (p, head_comments)
    }

    /// 返回注解渲染：**C 类型名不再上签名**（台账 BUG-13），改为行尾注释承载（见 `ret_note`）。
    /// 实测依据（Cython 3.2.2，`python -m cython -3`，探针在 `TEMP/annprobe/`）：
    /// · `-> Py_ssize_t`／`-> double`／`-> bint` ⇒ `warning: Unknown type declaration ...
    ///   in annotation, ignoring`（a_pyssize / h_double / k_bint 三件都只告警不生效）；
    /// · `-> int` 却是**真做类型检查**：`def f() -> int: return "x"` 直接
    ///   `Error compiling Cython file`（g_int_wrong，rc=1）——把 C 名换成 Python 内置名
    ///   等于给 LZ 语言没承诺的位置强加类型义务，56/77 件产物都可能因此编不过。
    /// 两条路都不成立，而本后端的函数一律 `def`（`fn_decl` 只发 def/async def，
    /// 没有 cdef/cpdef），所以这里发不出「既承重又无害」的返回注解：
    /// **不写**是唯一不说谎的形态。`raises` 仍发 `-> object`（实测无告警，且 object
    /// 对 Cython 就是「任意 Python 对象」，不新增义务）。
    fn render_ret_annotation(&self, f: &FnDef) -> String {
        if f.raises.is_some() {
            return " -> object".to_string();
        }
        String::new()
    }

    /// 返回类型的**注释**承载：签名不带类型（见 `render_ret_annotation`），但 LZ 源码里
    /// 写的 `-> T` 是事实，产物里要看得见，否则读 .pyx 的人会以为后端丢了这个信息。
    fn ret_note(&self, f: &FnDef) -> String {
        if f.raises.is_some() || f.ret_ty == IrType::Unit || f.ret_ty == IrType::Never {
            return String::new();
        }
        format!("  # ret: {}", self.map_type(&f.ret_ty, TypeCtx::Signature))
    }

    /// 声明选择：async def（异步）/ def（统一，见文件头决策 3）
    fn fn_decl(f: &FnDef) -> &'static str {
        if f.is_async {
            "async def"
        } else {
            "def"
        }
    }

    /// 生成函数体首注释（泛型 / where / intrinsics / checker）
    fn fn_head_comments(&self, f: &FnDef) -> Vec<String> {
        let mut comments = vec![];
        if !f.generics.is_empty() {
            let names: Vec<String> = f.generics.iter().map(|g| g.name.clone()).collect();
            comments.push(format!("# generic<{}>", names.join(", ")));
        }
        if !f.where_clause.is_empty() {
            let where_strs: Vec<String> = f
                .where_clause
                .iter()
                .map(|(name, bounds)| {
                    let b: Vec<String> = bounds
                        .iter()
                        .map(|b| self.map_type(b, TypeCtx::Signature))
                        .collect();
                    format!("{}: {}", name, b.join(" + "))
                })
                .collect();
            comments.push(format!("# where {}", where_strs.join(", ")));
        }
        for intr in &f.intrinsics {
            match &intr.kind {
                IntrinsicKind::Export(targets) => {
                    comments.push(format!("# @export({})", targets.join(", ")));
                }
                IntrinsicKind::Extern(targets) => {
                    comments.push(format!("# @extern({})", targets.join(", ")));
                }
                IntrinsicKind::Embed { lang, code: _ } => {
                    comments.push(format!("# @embed({})", lang));
                }
                IntrinsicKind::Memoize => comments.push("# @memoize".into()),
                IntrinsicKind::Parallel => comments.push("# @parallel".into()),
                IntrinsicKind::Curry => comments.push("# @curry".into()),
                IntrinsicKind::TailCall => comments.push("# @tailcall".into()),
                _ => {}
            }
        }
        if let Some(dc) = &f.default_checker {
            comments.push(format!("# default_checker: {}", dc));
        }
        comments
    }

    /// 分析函数体：需要 `global` 声明的模块级变量名
    /// 对齐 Rust 端 analyze_global_vars：函数内 let 声明模块级同名变量 =
    /// 写回模块变量（非 shadow）；Assign 目标命中模块级变量同理
    fn fn_global_decls(&self, f: &FnDef) -> Vec<String> {
        let mut assigned: Vec<String> = vec![];
        let mut locals: HashSet<String> = HashSet::new();
        collect_stmt_info(&f.body, &self.module_var_names, &mut assigned, &mut locals);
        assigned
            .into_iter()
            .filter(|n| self.module_var_names.contains(n) && !locals.contains(n))
            .collect()
    }

    /// 预扫描函数体中必须 hoist 成 def 的 Lambda：
    /// ① 体为 AssignExpr（闭包要写回外层变量，lambda 里没法 `nonlocal`）；
    /// ② 体为带前缀语句的 BlockExpr（lambda 装不下语句 ⇒ `lambda_block_needs_def`）。
    /// 为每个生成 `def __lambda_N(...): ...` 文本，函数体生成前注入。
    fn preprocess_lambdas(&mut self, body: &Block, raises_mode: bool, outer_bound: &HashSet<String>) {
        let mut found = vec![];
        collect_stmt_lambdas_block(body, &mut found);
        self.hoist_lambdas(found, raises_mode, outer_bound);
    }

    /// 模块级 const 初值里的块体闭包：预扫描入口是单个表达式，不是函数体
    fn preprocess_lambdas_expr(&mut self, expr: &Expr, raises_mode: bool, outer_bound: &HashSet<String>) {
        let mut found = vec![];
        collect_expr_lambdas(expr, &mut found);
        self.hoist_lambdas(found, raises_mode, outer_bound);
    }

    /// 当前函数里已绑定的名字（形参 + 体内 let/赋值目标）：块体闭包**写捕获**要不要
    /// `nonlocal` 的唯一依据（SYNTAX/03e §五）。闭包体自己的绑定不算在这里——
    /// `collect_expr_info` 对 `BlockExpr` 臂不下钻（codegen_cython.rs 的 `BlockExpr { .. } => {}`），
    /// 所以块内新绑定的名字不会混进外层集合。
    fn outer_bound_names(&self, body: &Block, params: &[Param]) -> HashSet<String> {
        let mut assigned: Vec<String> = vec![];
        let mut set = HashSet::new();
        collect_stmt_info(body, &self.module_var_names, &mut assigned, &mut set);
        for a in assigned {
            set.insert(a);
        }
        for p in params {
            set.insert(p.name.clone());
        }
        set
    }

    fn hoist_lambdas(&mut self, found: Vec<(Vec<Param>, Expr)>, raises_mode: bool, outer_bound: &HashSet<String>) {
        for (params, lam_body) in found {
            let key = format!(
                "{}::{}",
                params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(","),
                format!("{:?}", lam_body.kind)
            );
            if self.lambda_map.contains_key(&key) {
                continue;
            }
            let name = format!("__lambda_{}", self.tmp_counter);
            self.tmp_counter += 1;
            // 生成 def 文本（隔离 buf，完成后恢复）
            let saved_buf = std::mem::take(&mut self.buf);
            let saved_indent = self.indent;
            let saved_ret = self.current_fn_ret_ty.clone();
            self.indent = 0;
            self.buf.clear();
            // 嵌套 def 的尾值由下面的 `return` 显式承载，别让外层函数的返回类型上下文掺进来
            self.current_fn_ret_ty = None;
            let p: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            self.writeln(&format!("def {}({}):", name, p.join(", ")));
            self.indent += 1;
            // 写捕获（SYNTAX/03e §五「写捕获 → FnMut」）：Python 里对**外层**变量的赋值必须先声明，
            // 否则它是新局部（`nonlocal` 用在模块级变量上同样是 SyntaxError ⇒ 分派两条关键字）。
            let written = lambda_written_names(&lam_body, &self.module_var_names);
            let module_vars = self.module_var_names.clone();
            let mut globals: BTreeSet<String> = BTreeSet::new();
            let mut nonlocals: BTreeSet<String> = BTreeSet::new();
            for w in &written {
                if module_vars.contains(w) {
                    globals.insert(w.clone());
                } else if outer_bound.contains(w) {
                    nonlocals.insert(w.clone());
                }
            }
            if !globals.is_empty() {
                self.writeln(&format!(
                    "global {}",
                    globals.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            if !nonlocals.is_empty() {
                self.writeln(&format!(
                    "nonlocal {}",
                    nonlocals.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            match &lam_body.kind {
                ExprKind::AssignExpr { target, value } => {
                    if let ExprKind::Var(tn) = &target.kind {
                        self.writeln(&format!("{} = {}", tn, gen_expr(self, value)));
                        self.writeln(&format!("return {}", tn));
                    }
                }
                ExprKind::BlockExpr { block } => match gen_block_prefix(self, block, raises_mode) {
                    Some(t) => self.writeln(&format!("return {}", t)),
                    None => self.writeln("pass"),
                },
                _ => {}
            }
            let def_text = std::mem::take(&mut self.buf);
            self.current_fn_ret_ty = saved_ret;
            self.indent = saved_indent;
            self.buf = saved_buf;
            self.pending_lambda_defs.push(def_text);
            self.lambda_map.insert(key, name);
        }
    }

    /// flush 预生成的语句性 Lambda def（函数体首调用；Python def 需在调用前执行）
    fn flush_lambda_defs(&mut self) {
        let defs = std::mem::take(&mut self.pending_lambda_defs);
        for d in defs {
            for line in d.lines() {
                if !line.trim().is_empty() {
                    self.writeln(line);
                }
            }
        }
    }

    fn gen_function(&mut self, f: &FnDef) {
        // 引用别名是**函数内**局部事实：出了本函数体就不许再把 `r` 改写成别处的 `x`。
        self.ref_alias.clear();
        // 形参名表登记（checker 派发包装器用）
        self.fn_params.insert(
            f.name.clone(),
            f.params.iter().map(|p| p.name.clone()).collect(),
        );
        // ── 函数重载：同名函数有 >1 个**不同**签名时启用 mangling（Name__N）──
        // 登记表 `overload_sigs` 里同名同签名是**两条重复定义**而非重载（Rust 侧
        // 直接 E0428）。按下标取自去重后的签名集，mangle 名才和发射次数一致。
        let sig: Vec<IrType> = f.params.iter().map(|p| p.ty.clone()).collect();
        let distinct_sigs: Vec<Vec<IrType>> = self
            .overload_sigs
            .get(&f.name)
            .map(|sigs| {
                let mut out: Vec<Vec<IrType>> = Vec::new();
                for s in sigs {
                    if !out.contains(s) {
                        out.push(s.clone());
                    }
                }
                out
            })
            .unwrap_or_default();
        let fn_name = if distinct_sigs.len() > 1 {
            let idx = distinct_sigs.iter().position(|s| s == &sig).unwrap_or(0);
            format!("{}__{}", f.name, idx)
        } else {
            f.name.clone()
        };
        // 同名同签名的第二条不是重载，是重复定义（Rust 侧 E0428）。import 平铺合并
        // 造出的那一条已由 CLI 过滤（src/main.rs `merge_imports_into`）；走到这里还撞车
        // 就是源码里真有两条 `def add(...)`（例如两个 test 各定义一个）。静默取第一条
        // 会把错值编进产物 ⇒ 发一个刻意未定义的名字，让 cython 响亮失败。
        if self
            .emitted_fns
            .get(&f.name)
            .map_or(false, |em| em.iter().any(|(m, _)| m == &fn_name))
        {
            self.writeln(&format!(
                "# 重复定义：`def {}` 与已发射的同名同签名定义撞车",
                f.name
            ));
            self.writeln(&format!(
                "__lz_duplicate_def_{} = __lz_unresolved_duplicate_definition",
                mangle_ident(&f.name)
            ));
            self.writeln("");
            return;
        }
        self.emitted_fns
            .entry(f.name.clone())
            .or_default()
            .push((fn_name.clone(), f.params.len()));

        let (p, mut head_comments) = self.render_params(f, false);
        head_comments.extend(self.fn_head_comments(f));
        let ret_ann = self.render_ret_annotation(f);
        let ret_note = self.ret_note(f);

        // 预扫描语句性 Lambda（闭包写回场景）
        let lambda_outer = self.outer_bound_names(&f.body, &f.params);
        self.preprocess_lambdas(&f.body, f.raises.is_some(), &lambda_outer);
        self.write(&format!(
            "{} {}({}){}:{}",
            Self::fn_decl(f),
            fn_name,
            p.join(", "),
            ret_ann,
            ret_note
        ));
        self.writeln("");
        self.indent += 1;
        for c in &head_comments {
            self.writeln(c);
        }
        // 模块级变量写回 → global 声明（Python 语义：函数内赋值即局部，需显式 global）
        let globals = self.fn_global_decls(f);
        if !globals.is_empty() {
            self.writeln(&format!("global {}", globals.join(", ")));
        }
        // 语句性 Lambda def 注入（需在调用前定义）
        self.flush_lambda_defs();
        // 记录函数返回类型（尾表达式 return 转换上下文）
        let prev_ret = self.current_fn_ret_ty.clone();
        self.current_fn_ret_ty = Some(f.ret_ty.clone());
        let prev_async = self.current_fn_is_async;
        self.current_fn_is_async = f.is_async;
        let body = self.with_tail_return(f);
        gen_block(self, &body, f.raises.is_some());
        self.current_fn_ret_ty = prev_ret;
        self.current_fn_is_async = prev_async;
        self.lambda_map.clear();
        self.indent -= 1;
        self.writeln("");
    }

    /// 生成函数重载分发器（在模块末尾调用一次）
    ///
    /// 分支只从 `emitted_fns`（**实际发射**集合）取，不从签名登记表 `overload_sigs` 取：
    /// 登记表里同名同签名的重复定义会撑出 `Name__1` 这种从未发射的名字，照它分发
    /// 就是 `cython: undeclared name not builtin: main__1`（CY/TESTS/99_bootstrap 实测）。
    fn gen_overload_dispatchers(&mut self) {
        let overloads: Vec<(String, Vec<(String, usize)>)> = self
            .emitted_fns
            .iter()
            .filter(|(_, defs)| defs.len() > 1)
            .map(|(name, defs)| (name.clone(), defs.clone()))
            .collect();
        for (name, defs) in &overloads {
            // def name(*args):
            //     if len(args) == N0: return name__0(*args)
            //     elif ...
            //     else: raise TypeError(...)
            self.writeln(&format!("def {}(*args):", name));
            self.indent += 1;
            let mut dispatched: Vec<(usize, &String)> = Vec::new();
            for (mangled, arity) in defs {
                if let Some((_, first)) = dispatched.iter().find(|(a, _)| a == arity) {
                    self.writeln(&format!(
                        "# 未分发：{}（与 {} 同为 {} 个参数；重载分发只看 len(args)）",
                        mangled, first, arity
                    ));
                    continue;
                }
                let cond = if dispatched.is_empty() { "if" } else { "elif" };
                dispatched.push((*arity, mangled));
                self.writeln(&format!(
                    "{} len(args) == {}: return {}(*args)",
                    cond, arity, mangled
                ));
            }
            self.writeln(&format!(
                "else: raise TypeError(f\"{}: no matching overload for {{len(args)}} args\")",
                name
            ));
            self.indent -= 1;
            self.writeln("");
        }
    }

    fn gen_struct(&mut self, s: &StructDef) {
        // ── derives / generics / is_case 注释 ──
        if !s.derives.is_empty() {
            self.writeln(&format!("# @derive({})", s.derives.join(", ")));
        }
        if !s.generics.is_empty() {
            let names: Vec<String> = s.generics.iter().map(|g| g.name.clone()).collect();
            self.writeln(&format!("# generic<{}>", names.join(", ")));
        }
        if s.is_case {
            self.writeln("# case struct");
        }
        self.writeln(&format!("cdef class {}:", s.name));
        self.indent += 1;
        for f in &s.fields {
            self.writeln(&format!(
                "cdef public {} {}",
                self.map_type(&f.ty, TypeCtx::Field),
                f.name
            ));
        }
        if !s.fields.is_empty() {
            let p: Vec<String> = s
                .fields
                .iter()
                .map(|f| format!("{} {}", self.map_type(&f.ty, TypeCtx::Signature), f.name))
                .collect();
            self.writeln(&format!("def __init__(self, {}):", p.join(", ")));
            self.indent += 1;
            for f in &s.fields {
                self.writeln(&format!("self.{} = {}", f.name, f.name));
            }
            self.indent -= 1;
        } else {
            self.writeln("pass");
        }
        // __init__ 后初始化（has_init → __init__ 用户体重载字段版构造）
        if s.has_init {
            if let Some(body) = &s.init_body {
                let ip: Vec<String> = s
                    .init_params
                    .iter()
                    .filter(|(n, _)| n != "self")
                    .map(|(n, t)| format!("{} {}", self.map_type(t, TypeCtx::Signature), n))
                    .collect();
                self.writeln(&format!("def __init__(self, {}):", ip.join(", ")));
                self.indent += 1;
                gen_block(self, body, false);
                self.indent -= 1;
            }
        }
        // implicit_froms 注释
        if !s.implicit_froms.is_empty() {
            let froms: Vec<String> = s
                .implicit_froms
                .iter()
                .map(|t| self.map_type(t, TypeCtx::Signature))
                .collect();
            self.writeln(&format!("# __implicit_from__: {}", froms.join(", ")));
        }
        // ── `__repr__`：打印形态对齐 Rust `#[derive(Debug)]`。缺它时 `print(p)` 出
        //    `<模块名.Point object at 0x…>`，既带地址又泄漏门禁别名，跨后端差分无从对齐。
        let repr_fields: Vec<(Option<String>, String)> = s
            .fields
            .iter()
            .map(|f| (Some(f.name.clone()), format!("self.{}", f.name)))
            .collect();
        self.writeln("def __repr__(self):");
        self.indent += 1;
        for line in debug_repr_lines(&s.name, &repr_fields) {
            self.writeln(&line);
        }
        self.indent -= 1;
        // ── 类体内方法：struct 自身方法 + impl 注入方法 + self-def 注入方法 ──
        let prev_class = self.current_class_name.clone();
        self.current_class_name = Some(s.name.clone());
        for m in &s.methods {
            self.gen_method(m);
        }
        // impl 方法注入（inherent impl + trait impl 都按 Python 鸭子命名分发注入）
        if self.impl_methods.contains_key(&s.name) {
            let meths = self.impl_methods.remove(&s.name).unwrap_or_default();
            self.impl_injected.insert(s.name.clone());
            for m in &meths {
                self.gen_method(m);
            }
        }
        // self-def 顶层函数注入（`Point.sum` 形态，对齐 Rust 端 gen_self_fn_impls）
        if self.self_methods.contains_key(&s.name) {
            let meths = self.self_methods.remove(&s.name).unwrap_or_default();
            for m in &meths {
                self.gen_method(m);
            }
        }
        self.current_class_name = prev_class;
        self.indent -= 1;
        self.writeln("");

        // __new__ 魔术构造 → 模块级 `Name__new__(...)` 函数（对齐 Rust 端 __lz_new）
        if s.has_new {
            let np: Vec<String> = s
                .new_params
                .iter()
                .map(|(n, t)| format!("{} {}", self.map_type(t, TypeCtx::Signature), n))
                .collect();
            self.writeln(&format!("def {}__new__({}):", s.name, np.join(", ")));
            self.indent += 1;
            if let Some(body) = &s.new_body {
                gen_block(self, body, false);
            } else {
                // 占位体：按字段顺序构造，new_params 同名字段用参数，其余用类型默认值
                let args: Vec<String> = s
                    .fields
                    .iter()
                    .map(|f| {
                        if s.new_params.iter().any(|(n, _)| n == &f.name) {
                            f.name.clone()
                        } else {
                            default_value_for(&f.ty)
                        }
                    })
                    .collect();
                self.writeln(&format!("return {}({})", s.name, args.join(", ")));
            }
            self.indent -= 1;
            self.writeln("");
        }
    }

    /// ANF 尾表达式转 return：块最后一条 ExprStmt 且当前函数返回类型非
    /// Unit/Never 时改写为 Return 语句（Python 无表达式块尾值语义，显式
    /// return 才有返回值）。用于函数体 / match 臂体等值语义位置。
    fn tail_return_block(&self, block: &Block) -> Block {
        let mut b = block.clone();
        let ret = self.current_fn_ret_ty.clone().unwrap_or(IrType::Unit);
        if ret != IrType::Unit && ret != IrType::Never {
            if let Some(Stmt::ExprStmt { expr }) = b.stmts.last() {
                let e = expr.clone();
                b.stmts.pop();
                b.stmts.push(Stmt::Return { value: Some(e) });
            }
        }
        b
    }

    fn with_tail_return(&self, f: &FnDef) -> Block {
        self.tail_return_block(&f.body)
    }

    fn gen_method(&mut self, f: &FnDef) {
        // self-def/impl 来源的方法名可能带类型前缀（`Point.scaled`）→ 取最后段
        let short_name = f.name.rsplit('.').next().unwrap_or(&f.name).to_string();
        let (p, mut head_comments) = self.render_params(f, true);
        head_comments.extend(self.fn_head_comments(f));
        let ret_ann = self.render_ret_annotation(f);
        let ret_note = self.ret_note(f);

        // 无 self 参数的方法（关联函数/静态工厂 origin() 等）→ @staticmethod
        let is_static = !f
            .params
            .first()
            .map(|p| p.name == "self" || p.ty == IrType::Self_)
            .unwrap_or(false);
        if is_static {
            self.writeln("@staticmethod");
        }

        // 预扫描语句性 Lambda（闭包写回场景）
        let lambda_outer = self.outer_bound_names(&f.body, &f.params);
        self.preprocess_lambdas(&f.body, f.raises.is_some(), &lambda_outer);
        self.write(&format!(
            "{} {}({}){}:{}",
            Self::fn_decl(f),
            short_name,
            p.join(", "),
            ret_ann,
            ret_note
        ));
        self.writeln("");
        self.indent += 1;
        for c in &head_comments {
            self.writeln(c);
        }
        self.flush_lambda_defs();
        let prev_ret = self.current_fn_ret_ty.clone();
        self.current_fn_ret_ty = Some(f.ret_ty.clone());
        let prev_async = self.current_fn_is_async;
        self.current_fn_is_async = f.is_async;
        let body = self.with_tail_return(f);
        gen_block(self, &body, f.raises.is_some());
        self.current_fn_ret_ty = prev_ret;
        self.current_fn_is_async = prev_async;
        self.lambda_map.clear();
        self.indent -= 1;
    }

    fn gen_enum(&mut self, e: &EnumDef) {
        // ── derives / generics 注释 ──
        if !e.derives.is_empty() {
            self.writeln(&format!("# @derive({})", e.derives.join(", ")));
        }
        if !e.generics.is_empty() {
            let names: Vec<String> = e.generics.iter().map(|g| g.name.clone()).collect();
            self.writeln(&format!("# generic<{}>", names.join(", ")));
        }
        // ── 类层次方案（对齐 PyO3 enum / IntEnum）──
        // 注意：enum 一律用普通 Python class（非 cdef class）——无数据变体需要
        // 单例化（`Red = Red()` 覆盖类名），而 cdef class 名是 C 级类型不可赋值。
        // 变体字段用 Python 动态属性（__init__ 内 self.x = x）。
        self.writeln(&format!("class {}:", e.name));
        self.indent += 1;
        self.writeln("pass");
        self.indent -= 1;
        self.writeln("");

        // 每个变体生成一个子类（位置字段命名为 f0/f1/…，与模式解构对齐）
        // 类体首行注入 `_variant = <声明序号>`——match 解构按序号判别（对齐 `_variant == 0` 形态）
        // 无数据变体：类定义后立即单例化（`Red = Red()`）——`Enum.Variant` 引用
        // 与 `Variant == Variant` 比较、`Variant.is_warm()` 方法调用都以单例进行
        for (vi, v) in e.variants.iter().enumerate() {
            self.writeln(&format!("class {}({}):", mangle_ident(&v.name), e.name));
            self.indent += 1;
            self.writeln(&format!("_variant = {}", vi));
            if v.fields.is_empty() {
                // _variant 已构成类体，无需 pass 占位
            } else {
                // Python 动态属性（不用 cdef：普通 class 体内 cdef 非法）
                let p: Vec<String> = v
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        if f.name.is_empty() {
                            format!("f{}", i)
                        } else {
                            f.name.clone()
                        }
                    })
                    .collect();
                self.writeln(&format!("def __init__(self, {}):", p.join(", ")));
                self.indent += 1;
                for (i, f) in v.fields.iter().enumerate() {
                    let fname = if f.name.is_empty() {
                        format!("f{}", i)
                    } else {
                        f.name.clone()
                    };
                    self.writeln(&format!("self.{} = {}", fname, fname));
                }
                self.indent -= 1;
            }
            // `__repr__` 对齐 Rust Debug：具名变体 → `Rect { w: 1.0 }`，
            // 元组变体 → `Circle(0.0, 5.0)`，无数据变体 → `Red`
            let v_repr: Vec<(Option<String>, String)> = v
                .fields
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    if f.name.is_empty() {
                        (None, format!("self.f{}", i))
                    } else {
                        (Some(f.name.clone()), format!("self.{}", f.name))
                    }
                })
                .collect();
            self.writeln("def __repr__(self):");
            self.indent += 1;
            for line in debug_repr_lines(&v.name, &v_repr) {
                self.writeln(&line);
            }
            self.indent -= 1;
            self.indent -= 1;
            if v.fields.is_empty() {
                // 无数据变体单例化（类对象被实例覆盖，identity 语义保证 == / !=）
                // 变体名为 Python 关键字（如 None）→ mangle
                self.writeln(&format!(
                    "{} = {}()",
                    mangle_ident(&v.name),
                    mangle_ident(&v.name)
                ));
            }
            self.writeln("");
        }
        // enum 方法注入到基类（self 指向变体实例，方法体用 match self 解构）
        if !e.methods.is_empty() {
            let prev_class = self.current_class_name.clone();
            self.current_class_name = Some(e.name.clone());
            for m in &e.methods {
                self.gen_method(m);
            }
            self.current_class_name = prev_class;
            self.writeln("");
        }
    }

    fn gen_const(&mut self, c: &ConstDef) {
        // IrMods 修饰符注释
        if !c.mods.is_empty() {
            self.writeln(&format!(
                "# mods: static={}, shared={:?}, interior={:?}, lazy={}, source={}",
                c.mods.storage_static,
                c.mods.shared,
                c.mods.interior,
                c.mods.lazy,
                c.mods.source
            ));
        }
        // 初值里若有块体闭包，先把 hoist 出来的 def 落在绑定语句之前（模块级也是函数外的作用域，
        // 走 gen_expr 的 BlockExpr 分支会只留尾表达式 ⇒ 前缀语句静默丢失）
        self.preprocess_lambdas_expr(&c.value, false, &HashSet::new());
        self.flush_lambda_defs();
        let ty = self.map_type(&c.ty, TypeCtx::Signature);
        // 常量值是构建块/块表达式 → 前缀语句提升后绑定尾值
        if let ExprKind::BlockExpr { block } = &c.value.kind {
            let tail = gen_block_prefix(self, block, false).unwrap_or_else(|| "None".into());
            self.writeln(&format!("{} = {}  # const: {}", c.name, tail, ty));
            return;
        }
        let val = gen_expr(self, &c.value);
        // 常量一律生成 Python 模块级变量（可被外部访问；类型信息以注释保留）
        self.writeln(&format!("{} = {}  # const: {}", c.name, val, ty));
    }

    fn gen_type_alias(&mut self, ta: &TypeAliasDef) {
        let target = self.map_type(&ta.ty, TypeCtx::Signature);
        self.writeln(&format!("# type {} = {}", ta.name, target));
        // Cython 用 ctypedef 表达类型别名（仅对 C 类型有效，Python 类型用注释）
        if target != "object" && !target.contains('<') && !self.is_python_container(&target) {
            self.writeln(&format!("ctypedef {} {}", target, ta.name));
        }
    }

    /// 判断是否为 Python 容器类型（不能用 ctypedef）
    fn is_python_container(&self, ty: &str) -> bool {
        matches!(ty, "list" | "dict" | "set" | "tuple" | "str")
    }

    fn gen_import(&mut self, u: &UseStmt) {
        // 已平铺合并的模块：Python 侧没有同名模块，发射 `import lz_std` 会在导入期
        // ModuleNotFoundError（那些定义已经被 CLI 内联进同一份 .pyx）。
        if u.path
            .first()
            .map_or(false, |h| self.merged_modules.contains(h.as_str()))
        {
            return;
        }
        let path = u.path.join(".");
        if u.is_from {
            // from path import item1, item2
            // 注意：LZ 解析器仅在 `import path as alias`（is_from=false）时填充
            // UseStmt.alias；`from ... import` 不支持 per-item `as`（解析器不解析，
            // alias 恒为 None），故此处直接平铺 items，绝不附加 alias（BUG-13）。
            if u.items.is_empty() {
                self.writeln(&format!("from {} import *", path));
            } else {
                self.writeln(&format!("from {} import {}", path, u.items.join(", ")));
            }
        } else {
            // import path [as alias]
            if let Some(alias) = &u.alias {
                self.writeln(&format!("import {} as {}", path, alias));
            } else {
                self.writeln(&format!("import {}", path));
            }
        }
    }

    fn gen_trait(&mut self, t: &TraitDef) {
        // ── generics / supertraits / assoc_types 注释 ──
        if !t.generics.is_empty() {
            let names: Vec<String> = t.generics.iter().map(|g| g.name.clone()).collect();
            self.writeln(&format!("# generic<{}>", names.join(", ")));
        }
        if !t.supertraits.is_empty() {
            let sts: Vec<String> = t
                .supertraits
                .iter()
                .map(|st| self.map_type(st, TypeCtx::Signature))
                .collect();
            self.writeln(&format!("# supertraits: {}", sts.join(", ")));
        }
        if !t.assoc_types.is_empty() {
            self.writeln(&format!("# associated types: {}", t.assoc_types.join(", ")));
        }
        // ── Trait → 抽象基类（运行时标记；编译期 duck 检查已在 duck_check 完成）──
        self.writeln(&format!("class {}:", t.name));
        self.indent += 1;
        self.writeln(&format!("\"\"\"Trait: {}\"\"\"", t.name));
        for m in &t.methods {
            let p: Vec<String> = m
                .params_names
                .iter()
                .zip(m.params.iter())
                .map(|(name, ty)| {
                    if name == "self" || *ty == IrType::Self_ {
                        "self".into()
                    } else {
                        format!("{} {}", self.map_type(ty, TypeCtx::Signature), name)
                    }
                })
                .collect();
            let ret = self.map_type(&m.ret, TypeCtx::Signature);
            // trait 桩同样不发 C 类型返回注解（BUG-13：`-> double` 被 Cython 忽略并告警）；
            // 体是 `...`，注释不能挂在同一行末尾，故拆成两行。
            let note = if m.ret == IrType::Unit {
                String::new()
            } else {
                format!("  # ret: {}", ret)
            };
            self.writeln(&format!("def {}({}):{}", m.name, p.join(", "), note));
            self.indent += 1;
            self.writeln("...");
            self.indent -= 1;
        }
        self.indent -= 1;
        self.writeln("");
    }

    fn gen_impl(&mut self, i: &ImplDef) {
        // ── Impl → 方法已在 prescan 收集、gen_struct 时注入到 cdef class 体内 ──
        let target_name = match &i.for_type {
            IrType::Named { path, .. } => path.clone(),
            _ => return,
        };
        let trait_name = match &i.trait_ {
            Some(IrType::Named { path, .. }) => path.clone(),
            _ => String::new(),
        };
        if self.impl_injected.contains(&target_name) {
            // 方法已注入目标 cdef class —— 只留 trait 绑定注释
            if !trait_name.is_empty() {
                self.writeln(&format!(
                    "# impl {} for {} (methods injected into class {})",
                    trait_name, target_name, target_name
                ));
            }
            return;
        }
        // 非自定义类型目标（int/List 等标准类型 impl）→ 仅注释说明
        if !trait_name.is_empty() {
            self.writeln(&format!(
                "# impl {} for {} (std target: methods not injectable)",
                trait_name, target_name
            ));
        }
        for m in &i.methods {
            let ps: Vec<String> = m.params.iter().map(|p| p.name.clone()).collect();
            self.writeln(&format!(
                "#   fn {}({}) -> {}",
                m.name,
                ps.join(", "),
                self.map_type(&m.ret_ty, TypeCtx::Signature)
            ));
        }
        self.writeln("");
    }

    fn gen_test(&mut self, t: &TestDef) {
        // ── Test → pytest 风格函数 ──
        let safe_name: String = t
            .name
            .chars()
            .map(|c| match c {
                ' ' => '_',
                '-' => '_',
                c => c,
            })
            .collect();
        self.writeln(&format!("def test_{}():", safe_name));
        self.indent += 1;
        gen_block(self, &t.body, false);
        self.indent -= 1;
        self.writeln("");
        self.test_names.push(safe_name);
    }

    /// `--test` 的模块级运行器：逐条调用发射过的 `test_<name>`，按 Rust 面 `rustc --test`
    /// 的行形状打状态，返回失败条数（调用方据此定退出码）。
    /// 为什么形状要对齐 Rust 面：同一份 .lz 在两面跑测试时，stdout 只有「哪条过了」这一个
    /// 口径一致才可比；`running N tests` 与汇总行都不可省，否则「跑过 0 条」与「跑过 N 条」
    /// 在文本上无从区分（BUG-18 原本的失效形态就是 rc=0 + 什么都没跑）。
    fn emit_test_runner(&mut self) {
        self.writeln("");
        self.writeln("def _lz_run_tests():");
        self.indent += 1;
        if self.test_names.is_empty() {
            self.writeln("_lz_cases = []");
        } else {
            // 名字先摘出来：`&self.test_names` 的不可变借住会和循环里的
            // `self.writeln(&mut self)` 撞 E0502。
            let names = self.test_names.clone();
            self.writeln("_lz_cases = [");
            for n in names {
                self.writeln(&format!("    (\"{}\", test_{}),", n, n));
            }
            self.writeln("]");
        }
        self.writeln("print(\"running %d tests\" % len(_lz_cases))");
        self.writeln("_lz_passed = 0");
        self.writeln("_lz_failed = 0");
        self.writeln("for _lz_name, _lz_fn in _lz_cases:");
        self.indent += 1;
        self.writeln("try:");
        self.indent += 1;
        self.writeln("_lz_fn()");
        self.writeln("_lz_passed += 1");
        self.writeln("print(\"test {} ... ok\".format(_lz_name))");
        self.indent -= 1;
        self.writeln("except BaseException as _lz_e:");
        self.indent += 1;
        self.writeln("_lz_failed += 1");
        self.writeln("print(\"test {} ... FAILED\".format(_lz_name))");
        self.writeln("print(\"    {}: {}\".format(type(_lz_e).__name__, _lz_e))");
        self.indent -= 1;
        self.indent -= 1;
        self.writeln("if _lz_failed > 0:");
        self.indent += 1;
        self.writeln("print(\"test result: FAILED. %d passed; %d failed; 0 ignored; 0 measured; 0 filtered out\" % (_lz_passed, _lz_failed))");
        self.indent -= 1;
        self.writeln("else:");
        self.indent += 1;
        self.writeln("print(\"test result: ok. %d passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\" % _lz_passed)");
        self.indent -= 1;
        self.writeln("return _lz_failed");
        self.indent -= 1;
        self.writeln("");
        self.writeln("if __name__ == \"__main__\":");
        self.indent += 1;
        self.writeln("import sys as _lz_main_sys");
        self.writeln("_lz_main_sys.exit(1 if _lz_run_tests() else 0)");
        self.indent -= 1;
        self.writeln("");
    }

    fn gen_checker_block(
        &mut self,
        name: &str,
        ps_name: Option<&str>,
        default_checker: Option<&str>,
        body: &Block,
        captured: &[(String, IrType)],
    ) {
        // ── Checker 块 → 模块级函数（对齐 Rust 端 __Params）──
        let ps_param = ps_name.unwrap_or("ps");
        let mut params = vec![ps_param.to_string()];
        for (cap_name, cap_ty) in captured {
            params.push(format!(
                "{}: {}",
                cap_name,
                self.map_type(cap_ty, TypeCtx::Signature)
            ));
        }
        self.writeln(&format!("def {}({}):", name, params.join(", ")));
        self.indent += 1;
        if let Some(dc) = default_checker {
            self.writeln(&format!("# default_checker: {dc}"));
        }
        gen_block(self, body, false);
        self.indent -= 1;
        self.writeln("");
    }

    fn gen_duck_def(&mut self, d: &DuckDef) {
        // ── Duck 约束 → 运行时类型标记（编译期检查已在 duck_check 完成）──
        if !d.generics.is_empty() {
            let names: Vec<String> = d.generics.iter().map(|g| g.name.clone()).collect();
            self.writeln(&format!("# generic<{}>", names.join(", ")));
        }
        if !d.assoc_types.is_empty() {
            let ats: Vec<String> = d
                .assoc_types
                .iter()
                .map(|a| match &a.owner {
                    Some(o) => format!("{}.{}", o, a.name),
                    None => a.name.clone(),
                })
                .collect();
            self.writeln(&format!("# associated types: {}", ats.join(", ")));
        }
        for s in &d.satisfies {
            self.writeln(&format!("# satisfies {}", s));
        }
        if d.sealed {
            self.writeln("# sealed");
        }
        for rule in &d.match_rules {
            self.writeln(&format!(
                "# match /{}/ range({}, {})",
                rule.pattern, rule.range.0, rule.range.1
            ));
        }
        for pr in &d.param_reqs {
            let label = if pr.is_required { "require" } else { "optional" };
            self.writeln(&format!("# {}({})", label, pr.names.join(", ")));
        }
        self.writeln(&format!("class {}:", d.name));
        self.indent += 1;
        self.writeln(&format!("\"\"\"Duck type constraint: {}\"\"\"", d.name));
        // 方法签名存根
        for m in &d.methods {
            let ret = self.map_type(&m.ret_ty, TypeCtx::Signature);
            let owner_prefix = match &m.owner {
                Some(o) => format!("{}.", o),
                None => String::new(),
            };
            let name_str = match &m.name_pattern {
                Some(pat) => format!("{} (regex: {})", m.name, pat),
                None => m.name.clone(),
            };
            let range_str = match &m.param_range {
                Some((lo, hi)) => format!("  # param_range({}, {})", lo, hi),
                None => String::new(),
            };
            let default_str = if m.is_default { "  # default" } else { "" };
            self.writeln(&format!(
                "# def {}{}(...) -> {}{}{}",
                owner_prefix, name_str, ret, range_str, default_str
            ));
        }
        // 字段约束
        for f in &d.fields {
            let owner_prefix = match &f.owner {
                Some(o) => format!("{}.", o),
                None => String::new(),
            };
            let ty = self.map_type(&f.ty, TypeCtx::Signature);
            let rel_str = match &f.rel {
                Some((ro, rn)) => format!("  # rel {}.{}", ro, rn),
                None => String::new(),
            };
            self.writeln(&format!(
                "# field {}{}: {}{}",
                owner_prefix, f.name, ty, rel_str
            ));
        }
        self.indent -= 1;
        self.writeln("");
    }

    /// 预扫描模块：收集所有自定义类型名（struct/enum/impl）和 cdef class 集合
    /// 必须在 generate 主循环前调用一次
    fn prescan_module(&mut self, module: &IrModule) {
        use std::collections::hash_map::Entry;
        for item in &module.items {
            match item {
                Item::StructDef(s) => {
                    self.known_types.insert(s.name.clone());
                    self.cdef_classes.insert(s.name.clone());
                    if s.has_new {
                        self.has_new_structs.insert(s.name.clone());
                    }
                }
                Item::EnumDef(e) => {
                    self.known_types.insert(e.name.clone());
                    self.enum_names.insert(e.name.clone());
                    // enum 的每个变体也是类型名；记录变体字段名（位置字段 → f{i}）
                    let mut vset = HashSet::new();
                    for v in &e.variants {
                        self.known_types
                            .insert(format!("{}.{}", e.name, v.name));
                        vset.insert(v.name.clone());
                        let fnames: Vec<String> = v
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(i, f)| {
                                if f.name.is_empty() {
                                    format!("f{}", i)
                                } else {
                                    f.name.clone()
                                }
                            })
                            .collect();
                        self.variant_fields
                            .insert(format!("{}.{}", e.name, v.name), fnames);
                    }
                    self.enum_variants.insert(e.name.clone(), vset);
                }
                Item::Impl(i) => {
                    if let IrType::Named { path, .. } = &i.for_type {
                        self.known_types.insert(path.clone());
                        self.impl_methods
                            .entry(path.clone())
                            .or_default()
                            .extend(i.methods.iter().cloned());
                    }
                }
                Item::TypeAlias(ta) => {
                    self.known_types.insert(ta.name.clone());
                }
                Item::Const(c) => {
                    self.module_var_names.insert(c.name.clone());
                }
                Item::FnDef(f) => {
                    // self-def 形态（`Point.sum`）→ 注入到 owner 类型（对齐 Rust 端
                    // gen_self_fn_impls 的 impl 块分组生成）
                    if f.name.contains('.') {
                        if let Some((owner, _)) = f.name.rsplit_once('.') {
                            self.self_methods
                                .entry(owner.to_string())
                                .or_default()
                                .push(f.clone());
                        }
                        continue;
                    }
                    // 收集函数重载签名
                    let sig: Vec<IrType> = f.params.iter().map(|p| p.ty.clone()).collect();
                    match self.overload_sigs.entry(f.name.clone()) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(sig);
                        }
                        Entry::Vacant(e) => {
                            e.insert(vec![sig]);
                        }
                    }
                }
                // BUG-18：显式穷尽所有 Item 变体，杜绝 `_ => {}` 静默漏项。
                // 新增 Item 变体时此处会编译失败，强制同步（行为与原 `_ => {}` 一致，
                // 但这些变体在 prescan 阶段无需登记已知类型）。
                Item::Use(_) => {}
                Item::TraitDef(_) => {}
                Item::Test(_) => {}
                Item::CheckerBlock { .. } => {}
                Item::DuckDef(_) => {}
                Item::EmbedBlock { .. } => {}
            }
        }
        // 顶层 let 声明也是模块级变量
        for stmt in &module.top_level_stmts {
            if let Stmt::Let { name, .. } = stmt {
                self.module_var_names.insert(name.clone());
            }
        }
    }

    /// 上下文感知类型映射（核心）
    /// 函数签名位置优先 C 类型（性能），容器/泛型位置一律 object（PyObject）
    pub fn map_type(&self, ty: &IrType, ctx: TypeCtx) -> String {
        match ty {
            // ── 内建原语 ──
            IrType::Int => match ctx {
                TypeCtx::Signature | TypeCtx::Field | TypeCtx::Local => "Py_ssize_t".into(),
                TypeCtx::Container | TypeCtx::Generic => "object".into(),
            },
            IrType::Int128 => "object".into(),
            IrType::BigInt => "object".into(),
            IrType::Complex => "object".into(),
            IrType::F64 => match ctx {
                TypeCtx::Signature | TypeCtx::Field | TypeCtx::Local => "double".into(),
                TypeCtx::Container | TypeCtx::Generic => "object".into(),
            },
            IrType::Str => "str".into(),   // Python str 本身就是 PyObject
            IrType::Bool => "bint".into(), // Cython bool 也是 PyObject
            IrType::Unit | IrType::Never => "void".into(),
            IrType::Any => "object".into(),
            IrType::Self_ => {
                // 在 cdef class 内解析为当前类名，否则退化为 object
                self.current_class_name
                    .clone()
                    .unwrap_or_else(|| "object".into())
            }
            IrType::Ext => "object".into(), // 外部不透明句柄 → PyObject

            // ── 命名类型 ──
            IrType::Named { path, args } => self.map_named_type(path, args, ctx),

            // ── 容器类型 ──
            IrType::Option(_) => "object".into(),     // None = 无值
            IrType::Result { .. } => "object".into(), // 异常传播表错

            IrType::Tuple(_elems) => match ctx {
                TypeCtx::Container | TypeCtx::Generic => "object".into(),
                _ => "tuple".into(),
            },

            // ── 函数类型 ──
            IrType::Fn { .. } => "object".into(), // 闭包/Python callable

            // ── 引用类型（Cython 无原生引用 → PyObject）──
            IrType::Ref(_) | IrType::MutRef(_) => "object".into(),

            // ── Duck 类型（编译期检查，运行时为 PyObject）──
            IrType::Duck { .. } => "object".into(),

            // ── 泛型变量（运行时擦除）──
            IrType::Generic(_) => "object".into(),
        }
    }

    /// 映射 Named 类型（含用户自定义类型判断）
    fn map_named_type(&self, path: &str, _args: &[IrType], _ctx: TypeCtx) -> String {
        match path {
            // ── 标准库容器（映射到 Python 原生容器）──
            "List" | "list" => "list".into(),
            "Dict" | "dict" => "dict".into(),
            "Set" | "set" => "set".into(),

            // ── 智能指针 → 退化为 object ──
            "Box" | "Rc" | "Arc" => "object".into(),

            // ── 异步句柄 ──
            "Future" => "object".into(),

            // ── Option/Result → 异常/None 模型 ──
            "Option" | "Result" => "object".into(),

            // ── 用户自定义类型 ──
            other => {
                if self.cdef_classes.contains(other) {
                    // cdef class（struct）→ 可作为参数/返回的 C 级标注
                    other.to_string()
                } else if self.known_types.contains(other) {
                    // enum 基类/变体（普通 Python class）→ Cython 不允许普通
                    // Python class 作为类型标注 → object
                    "object".into()
                } else {
                    // 未知类型 → object（PyObject 兜底，一律 PyObject 原则）
                    "object".into()
                }
            }
        }
    }

    fn writeln(&mut self, s: &str) {
        let p = "    ".repeat(self.indent);
        self.buf.push_str(&p);
        self.buf.push_str(s);
        self.buf.push('\n');
    }
    fn write(&mut self, s: &str) {
        let p = "    ".repeat(self.indent);
        self.buf.push_str(&p);
        self.buf.push_str(s);
    }
}

/// 类型默认值（Default 表达式 / __new__ 占位构造）
fn default_value_for(ty: &IrType) -> String {
    match ty {
        IrType::Int | IrType::Never => "0".into(),
        IrType::F64 => "0.0".into(),
        IrType::Str => "\"\"".into(),
        IrType::Bool => "False".into(),
        _ => "None".into(),
    }
}

/// 安全转换调用：仅内建标量类型生成转换表达式，其余透传（自定义类型/容器
/// 在 Python 的 duck 语义下无需转换；object 不是可调用转换器）
fn safe_cast(cg: &CythonCodeGen, target: &IrType, expr_str: String) -> String {
    match target {
        IrType::Int => format!("int({})", expr_str),
        IrType::F64 => format!("float({})", expr_str),
        IrType::Str => format!("str({})", expr_str),
        IrType::Bool => format!("bool({})", expr_str),
        IrType::Complex => format!("complex({})", expr_str),
        IrType::Named { path, .. }
            if path == "int" || path == "i64" || path == "i32" =>
        {
            format!("int({})", expr_str)
        }
        IrType::Named { path, .. } if path == "f64" || path == "double" => {
            format!("float({})", expr_str)
        }
        IrType::Named { path, .. } if path == "complex" => {
            format!("complex({})", expr_str)
        }
        _ => {
            let _ = cg;
            expr_str
        }
    }
}

/// Option/Result 构造识别（enum_name 可为空：`Some(x)` 直接写时 builder 可能留空）
fn is_option_variant(enum_name: &str, variant: &str) -> bool {
    variant == "Some"
        || variant == "None"
        || enum_name == "Option"
        || enum_name == "option"
}
fn is_result_variant(enum_name: &str, variant: &str) -> bool {
    variant == "Ok"
        || variant == "Err"
        || enum_name == "Result"
        || enum_name == "result"
}

/// 判断表达式是否为 Err(...) 构造（Return 语句 raise 脱糖用）
fn is_err_ctor(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::EnumCtor { enum_name, variant, .. } => {
            is_result_variant(enum_name, variant) && variant == "Err"
        }
        ExprKind::Call { callee, .. } => {
            matches!(&callee.kind, ExprKind::Var(n) if n == "Err")
        }
        _ => false,
    }
}

/// Python 关键字判定（变体名/绑定名 mangling 用）
fn is_py_keyword(s: &str) -> bool {
    matches!(
        s,
        "None" | "True" | "False" | "and" | "or" | "not" | "if" | "else" | "elif" | "while"
            | "for" | "in" | "is" | "lambda" | "def" | "class" | "import" | "from" | "as"
            | "pass" | "break" | "continue" | "return" | "raise" | "try" | "except"
            | "finally" | "with" | "yield" | "del" | "global" | "nonlocal" | "assert"
            | "async" | "await"
    )
}

/// 标识符 mangle：Python 关键字 → 追加下划线
fn mangle_ident(s: &str) -> String {
    if is_py_keyword(s) {
        format!("{}_", s)
    } else {
        s.to_string()
    }
}

/// Range 形态的**索引位** → Python 切片文本（`a:b` / `a:` / `:b`，缺界留空）。
/// 只在索引位调用：Range 作为**值**出现时（`for i in 1..3`）仍由 StructCtor("Range")
/// 分支发 `range(...)`，两者语义不同——切片是「取子序列」，range 是「可迭代整数序」。
/// 闭区间（`inclusive`）在末位 +1，与 range 分支同一口径。
fn range_slice(cg: &CythonCodeGen, key: &Expr) -> Option<String> {
    let (name, fields) = match &key.kind {
        ExprKind::StructCtor { name, fields } => (name.as_str(), fields),
        _ => return None,
    };
    if name != "Range" {
        return None;
    }
    let start = fields
        .iter()
        .find(|(n, _)| n == "start")
        .map(|(_, v)| gen_expr(cg, v));
    let end = fields.iter().find(|(n, _)| n == "end").map(|(_, v)| gen_expr(cg, v));
    let inclusive = fields.iter().any(|(n, v)| {
        n == "inclusive" && matches!(&v.kind, ExprKind::Lit(LitKind::Bool(true)))
    });
    Some(match (start, end) {
        (Some(s), Some(e)) if inclusive => format!("{}:{} + 1", s, e),
        (Some(s), Some(e)) => format!("{}:{}", s, e),
        (Some(s), None) => format!("{}:", s),
        (None, Some(e)) => format!(":{}", e),
        _ => ":".to_string(),
    })
}

/// 把 f-string 模板里的 `{expr}` 插值裹成 `{_lz_fs(expr)}`——str 值按规范不带引号，
/// 其余按本后端的 Debug 口径（定义与规范行引用见 prelude 的 `_lz_fs`）。
/// Python 原生 f-string 两条都不是：`str(True) == "True"`，而 LZ 要 `true`
/// （实测 `DEMO/02_types/duck_field_rel.lz`：`linked: True` vs `linked: true`）。
/// `{{`/`}}` 是字面花括号原样保留；`:spec` / `!conv` 后缀留在括号外；
/// 已裹过的不重复裹（同一模板可能被生成线与后处理线各过一遍）。
fn fs_interp(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    out.push_str("{{");
                    continue;
                }
                let mut inner = String::new();
                let mut depth = 1usize;
                let mut closed = false;
                while let Some(ec) = chars.next() {
                    match ec {
                        '{' => {
                            depth += 1;
                            inner.push(ec);
                        }
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                closed = true;
                                break;
                            }
                            inner.push(ec);
                        }
                        _ => inner.push(ec),
                    }
                }
                let t = inner.trim();
                if !closed || t.is_empty() {
                    out.push('{');
                    out.push_str(&inner);
                    if closed {
                        out.push('}');
                    }
                    continue;
                }
                let (expr_part, spec_part) = match t.find(|c| c == ':' || c == '!') {
                    Some(i) => (t[..i].trim(), &t[i..]),
                    None => (t, ""),
                };
                out.push('{');
                if expr_part.is_empty() || expr_part.starts_with("_lz_fs(") {
                    out.push_str(t);
                } else {
                    out.push_str("_lz_fs(");
                    out.push_str(expr_part);
                    out.push(')');
                    out.push_str(spec_part);
                }
                out.push('}');
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    out.push_str("}}");
                } else {
                    out.push('}');
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// 静态类型是否指向**用户自定义类**（cdef class / enum 变体类）。
/// 方法名改写表（`push`→`append` 这类）只能作用于非用户类接收者 ——
/// 用户自己写的 `def push(self)` 必须原样发出去。
fn ty_is_user_class(cg: &CythonCodeGen, ty: &IrType) -> bool {
    match ty {
        IrType::Named { path, .. } => {
            cg.known_types.contains(path.as_str()) || cg.cdef_classes.contains(path.as_str())
        }
        _ => false,
    }
}

/// 生成与 Rust `#[derive(Debug)]` 同形的 `__repr__` 方法体。
/// `fields` 元素 = (字段显示名, 取值表达式)；显示名为 `None` ⇒ 位置形态（元组变体）。
/// 具名 → `Name { a: 1, b: 2 }`；位置 → `Name(1, 2)`；无字段 → `Name`。
/// 取值统一走 `_lz_dbg`：与 print 同一个 Debug 口径，嵌套对象自动递归到本方法。
fn debug_repr_lines(type_name: &str, fields: &[(Option<String>, String)]) -> Vec<String> {
    if fields.is_empty() {
        return vec![format!("return \"{}\"", type_name)];
    }
    let items: Vec<String> = fields
        .iter()
        .map(|(name, expr)| match name {
            Some(n) => format!("\"{}: \" + _lz_dbg({})", n, expr),
            None => format!("_lz_dbg({})", expr),
        })
        .collect();
    let joined = items.join(", ");
    if fields.iter().all(|(n, _)| n.is_some()) {
        vec![format!(
            "return \"{} {{ \" + \", \".join([{}]) + \" }}\"",
            type_name, joined
        )]
    } else {
        vec![format!(
            "return \"{}(\" + \", \".join([{}]) + \")\"",
            type_name, joined
        )]
    }
}

/// 替换字符串中的独立标识符（词法边界感知，UTF-8 安全）
/// 用于 guard 中引用模式绑定名 → 替换为对应的 scrutinee 子表达式
fn replace_ident(s: &str, name: &str, val: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        let first = rest.chars().next().unwrap();
        if first.is_ascii_alphabetic() || first == '_' {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let word = &rest[..end];
            if word == name {
                out.push_str(val);
            } else {
                out.push_str(word);
            }
            rest = &rest[end..];
        } else {
            out.push(first);
            rest = &rest[first.len_utf8()..];
        }
    }
    out
}

/// 提取块的尾表达式（BlockExpr 语句提升用）：最后一个 ExprStmt 的表达式。
/// 解析 checker 派发调用形态 `fn[checker]`（严格：两侧均为合法标识符）。
/// 返回 (函数名, checker 名)；非该形态返回 None。
fn parse_checker_index(s: &str) -> Option<(String, String)> {
    let close = s.strip_suffix(']')?;
    let open = close.rfind('[')?;
    let (fname, cname) = (close[..open].trim(), close[open + 1..].trim());
    let valid =
        |t: &str| !t.is_empty() && t.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) && t.chars().all(|c| c.is_alphanumeric() || c == '_');
    if valid(fname) && valid(cname) {
        Some((fname.to_string(), cname.to_string()))
    } else {
        None
    }
}

/// 块体闭包是否必须 hoist 成 def：只要尾表达式之外还有语句（或最后一条根本不是表达式语句），
/// `lambda p: 尾` 就会**静默丢掉**那些语句——丢的东西不一定报错，多半只是少一次副作用。
fn lambda_block_needs_def(block: &Block) -> bool {
    !matches!(block.stmts.as_slice(), [] | [Stmt::ExprStmt { .. }])
}

/// 闭包体会**写**哪些名字（`AssignExpr` 的目标 / 块体里对已有变量的赋值）。
/// 块体内新绑定的名字（`let`）不算：那是闭包自己的局部，写捕获判定要的是"外层已有"那一部分。
fn lambda_written_names(lam_body: &Expr, module_vars: &HashSet<String>) -> Vec<String> {
    match &lam_body.kind {
        ExprKind::AssignExpr { target, .. } => match &target.kind {
            ExprKind::Var(n) => vec![n.clone()],
            _ => vec![],
        },
        ExprKind::BlockExpr { block } => {
            let mut assigned: Vec<String> = vec![];
            let mut locals = HashSet::new();
            collect_stmt_info(block, module_vars, &mut assigned, &mut locals);
            assigned.into_iter().filter(|n| !locals.contains(n)).collect()
        }
        _ => vec![],
    }
}

/// 语句性产物（assert!/assert_eq!/panic! 特判生成的 assert/raise）不能作为值 → None
fn block_tail_expr(cg: &CythonCodeGen, block: &Block) -> Option<String> {
    match block.stmts.last() {
        Some(Stmt::ExprStmt { expr }) => {
            if let ExprKind::Call { callee, .. } = &expr.kind {
                if let ExprKind::Var(n) = &callee.kind {
                    if matches!(n.as_str(), "assert!" | "assert_eq!" | "panic!") {
                        return None;
                    }
                }
            }
            Some(gen_expr(cg, expr))
        }
        _ => None,
    }
}

/// 块表达式提升专用：只生成「尾表达式之前的语句」，把尾表达式作为**值**返回给调用方绑定。
/// 必须与 `block_tail_expr` 同一判据（尾不是 assert!/panic! 才当值）：
/// 若用 `gen_block` 连尾语句一起生成，调用方再绑定一次 ⇒ 同一条语句执行两遍
/// （实测形态：`with` 体里的 print 在产物里出现两次，L3 输出不符）。
/// 这里不做 `gen_block` 的空块 `pass` 兜底——调用方紧接着必写一行（绑定或 return），
/// 块体非空由那行保证。
fn gen_block_prefix(cg: &mut CythonCodeGen, block: &Block, raises_mode: bool) -> Option<String> {
    let tail = block_tail_expr(cg, block);
    let n = block.stmts.len() - usize::from(tail.is_some());
    cg.deferred.push(vec![]);
    for stmt in &block.stmts[..n] {
        if let Stmt::Defer { body } = stmt {
            cg.deferred.last_mut().unwrap().push(body.clone());
        } else {
            gen_stmt(cg, stmt, raises_mode);
        }
    }
    let defs = cg.deferred.pop().unwrap_or_default();
    for body in defs.iter().rev() {
        gen_block(cg, body, raises_mode);
    }
    tail
}

/// 递归扫描块内是否包含 defer（预留：函数级 try/finally 精确化时使用）
#[allow(dead_code)]
fn block_has_defer(block: &Block) -> bool {
    block.stmts.iter().any(|s| match s {
        Stmt::Defer { .. } => true,
        Stmt::If { then_branch, else_branch, .. } => {
            block_has_defer(then_branch)
                || else_branch.as_ref().map(block_has_defer).unwrap_or(false)
        }
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::WhileLet { body, .. }
        | Stmt::BlockLabel { body, .. } => block_has_defer(body),

        Stmt::Match { arms, .. } => arms.iter().any(|a| block_has_defer(&a.body)),
        Stmt::TryCatch { body, catches, else_body, finally_body } => {
            block_has_defer(body)
                || catches.iter().any(|(_, cb)| block_has_defer(cb))
                || else_body.as_ref().map(block_has_defer).unwrap_or(false)
                || finally_body.as_ref().map(block_has_defer).unwrap_or(false)
        }
        Stmt::Block { stmts: _ } => false, // 裸块语句已被展开，无需深入
        _ => false,
    })
}

// ══════════════════════════════════════════════════════════════
// 块与语句生成
// ══════════════════════════════════════════════════════════════

/// 块生成：Defer 收集（块级 LIFO，对齐 Rust 端 BUG-IR-002 方案 A）+ 空块 pass 兜底
fn gen_block(cg: &mut CythonCodeGen, block: &Block, raises_mode: bool) {
    let before = cg.buf.len();
    cg.deferred.push(vec![]);
    for stmt in &block.stmts {
        if let Stmt::Defer { body } = stmt {
            // BUG-IR-002 方案 A（内联脱糖）：仅收集 defer 体，块退出前逆序内联
            cg.deferred.last_mut().unwrap().push(body.clone());
        } else {
            gen_stmt(cg, stmt, raises_mode);
        }
    }
    // flush：块退出前逆序（LIFO）内联 defer 体
    let defs = cg.deferred.pop().unwrap_or_default();
    for body in defs.iter().rev() {
        gen_block(cg, body, raises_mode);
    }
    // 空块兜底：Python 语法要求块体非空
    if cg.buf.len() == before {
        cg.writeln("pass");
    }
}

/// 递归收集语句块信息：赋值目标名（Assign/AssignExpr 的 Var 目标）与
/// let 声明名（区分模块变量写回 vs 局部 shadow）
fn collect_stmt_info(
    block: &Block,
    module_vars: &HashSet<String>,
    assigned: &mut Vec<String>,
    locals: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        collect_stmt_info_stmt(stmt, module_vars, assigned, locals);
    }
}

fn collect_stmt_info_stmt(
    stmt: &Stmt,
    module_vars: &HashSet<String>,
    assigned: &mut Vec<String>,
    locals: &mut HashSet<String>,
) {
    match stmt {
        Stmt::Let { name, value, .. } => {
            // 函数内 let 模块级同名变量 = 写回模块变量（非 shadow，对齐 Rust 端）
            if module_vars.contains(name) {
                assigned.push(name.clone());
            } else {
                locals.insert(name.clone());
            }
            collect_expr_info(value, assigned, locals);
        }
        Stmt::Assign { target, value } => {
            if let ExprKind::Var(n) = &target.kind {
                assigned.push(n.clone());
            }
            collect_expr_info(value, assigned, locals);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                collect_expr_info(v, assigned, locals);
            }
        }
        Stmt::ExprStmt { expr } => collect_expr_info(expr, assigned, locals),
        Stmt::If { cond, then_branch, else_branch } => {
            collect_expr_info(cond, assigned, locals);
            collect_stmt_info(then_branch, module_vars, assigned, locals);
            if let Some(eb) = else_branch {
                collect_stmt_info(eb, module_vars, assigned, locals);
            }
        }
        Stmt::For { var, iter, guard, body, else_body } => {
            locals.insert(var.clone());
            collect_expr_info(iter, assigned, locals);
            if let Some(g) = guard {
                collect_expr_info(g, assigned, locals);
            }
            collect_stmt_info(body, module_vars, assigned, locals);
            if let Some(eb) = else_body {
                collect_stmt_info(eb, module_vars, assigned, locals);
            }
        }
        Stmt::While { cond, guard, body, else_body } => {
            collect_expr_info(cond, assigned, locals);
            if let Some(g) = guard {
                collect_expr_info(g, assigned, locals);
            }
            collect_stmt_info(body, module_vars, assigned, locals);
            if let Some(eb) = else_body {
                collect_stmt_info(eb, module_vars, assigned, locals);
            }
        }
        Stmt::WhileLet { expr, guard, body, .. } => {
            collect_expr_info(expr, assigned, locals);
            if let Some(g) = guard {
                collect_expr_info(g, assigned, locals);
            }
            collect_stmt_info(body, module_vars, assigned, locals);
        }
        Stmt::Match { scrutinee, arms } => {
            collect_expr_info(scrutinee, assigned, locals);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_expr_info(g, assigned, locals);
                }
                collect_stmt_info(&arm.body, module_vars, assigned, locals);
            }
        }
        Stmt::Raise { value } => collect_expr_info(value, assigned, locals),
        Stmt::Assert { cond, message } => {
            collect_expr_info(cond, assigned, locals);
            if let Some(m) = message {
                collect_expr_info(m, assigned, locals);
            }
        }
        Stmt::Yield { value } => collect_expr_info(value, assigned, locals),
        Stmt::YieldFrom { iter } => collect_expr_info(iter, assigned, locals),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                collect_expr_info(v, assigned, locals);
            }
        }
        Stmt::BlockLabel { body, .. } => collect_stmt_info(body, module_vars, assigned, locals),
        Stmt::CheckerBlock { body, .. } => collect_stmt_info(body, module_vars, assigned, locals),
        Stmt::Defer { body } => collect_stmt_info(body, module_vars, assigned, locals),
        Stmt::TryCatch { body, catches, else_body, finally_body } => {
            collect_stmt_info(body, module_vars, assigned, locals);
            for (_, cb) in catches {
                collect_stmt_info(cb, module_vars, assigned, locals);
            }
            if let Some(eb) = else_body {
                collect_stmt_info(eb, module_vars, assigned, locals);
            }
            if let Some(fb) = finally_body {
                collect_stmt_info(fb, module_vars, assigned, locals);
            }
        }
        Stmt::Block { stmts } => {
            let blk = Block {
                stmts: stmts.clone(),
                ty: IrType::Unit,
                span: super::node::Span::unknown(),
            };
            collect_stmt_info(&blk, module_vars, assigned, locals);
        }
        Stmt::Pass | Stmt::Break | Stmt::Continue | Stmt::TypeAlias { .. } => {}
    }
}

/// 递归收集表达式中的赋值目标（AssignExpr）——用于 global 分析
fn collect_expr_info(expr: &Expr, assigned: &mut Vec<String>, locals: &mut HashSet<String>) {
    match &expr.kind {
        ExprKind::AssignExpr { target, value } => {
            if let ExprKind::Var(n) = &target.kind {
                assigned.push(n.clone());
            }
            collect_expr_info(value, assigned, locals);
        }
        ExprKind::Call { callee, args, .. } => {
            collect_expr_info(callee, assigned, locals);
            for a in args {
                collect_expr_info(a, assigned, locals);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_expr_info(receiver, assigned, locals);
            for a in args {
                collect_expr_info(a, assigned, locals);
            }
        }
        ExprKind::FieldAccess { base, .. } | ExprKind::IndexGet { base, .. } => {
            collect_expr_info(base, assigned, locals)
        }
        ExprKind::IndexSet { base, key, value } => {
            collect_expr_info(base, assigned, locals);
            collect_expr_info(key, assigned, locals);
            collect_expr_info(value, assigned, locals);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_expr_info(lhs, assigned, locals);
            collect_expr_info(rhs, assigned, locals);
        }
        ExprKind::UnOp { operand, .. } => collect_expr_info(operand, assigned, locals),
        ExprKind::IfExpr { cond, then, els } => {
            collect_expr_info(cond, assigned, locals);
            collect_expr_info(then, assigned, locals);
            collect_expr_info(els, assigned, locals);
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                collect_expr_info(e, assigned, locals);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                collect_expr_info(a, assigned, locals);
            }
        }
        ExprKind::Lambda { body, .. } => collect_expr_info(body, assigned, locals),
        // 表达式内嵌块（罕见）：不影响 global 分析的正确性主路径，跳过
        ExprKind::BlockExpr { .. } => {}
        ExprKind::TupleLit(es) | ExprKind::Tuple(es) | ExprKind::ListLit(es) | ExprKind::List(es) => {
            for e in es {
                collect_expr_info(e, assigned, locals);
            }
        }
        ExprKind::Dict(entries) => {
            for (k, v) in entries {
                collect_expr_info(k, assigned, locals);
                collect_expr_info(v, assigned, locals);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_expr_info(s, assigned, locals);
            }
            collect_expr_info(end, assigned, locals);
        }
        ExprKind::Pipe { receiver, callee, args } => {
            collect_expr_info(receiver, assigned, locals);
            collect_expr_info(callee, assigned, locals);
            for a in args {
                collect_expr_info(a, assigned, locals);
            }
        }
        ExprKind::Paren(e) | ExprKind::Spread(e) | ExprKind::ImplicitConvert { source: e, .. } => {
            collect_expr_info(e, assigned, locals)
        }
        ExprKind::Cast { expr: e, .. } => collect_expr_info(e, assigned, locals),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_expr_info(a, assigned, locals);
            }
        }
        _ => {}
    }
}

/// 递归收集语句块中的语句性 Lambda（体为 AssignExpr 的闭包）
fn collect_stmt_lambdas_block(block: &Block, out: &mut Vec<(Vec<Param>, Expr)>) {
    for stmt in &block.stmts {
        collect_stmt_lambdas_stmt(stmt, out);
    }
}

fn collect_stmt_lambdas_stmt(stmt: &Stmt, out: &mut Vec<(Vec<Param>, Expr)>) {
    match stmt {
        Stmt::Let { value, .. } => collect_expr_lambdas(value, out),
        Stmt::Assign { target, value } => {
            collect_expr_lambdas(target, out);
            collect_expr_lambdas(value, out);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                collect_expr_lambdas(v, out);
            }
        }
        Stmt::ExprStmt { expr } => collect_expr_lambdas(expr, out),
        Stmt::If { cond, then_branch, else_branch } => {
            collect_expr_lambdas(cond, out);
            collect_stmt_lambdas_block(then_branch, out);
            if let Some(eb) = else_branch {
                collect_stmt_lambdas_block(eb, out);
            }
        }
        Stmt::For { iter, guard, body, else_body, .. } => {
            collect_expr_lambdas(iter, out);
            if let Some(g) = guard {
                collect_expr_lambdas(g, out);
            }
            collect_stmt_lambdas_block(body, out);
            if let Some(eb) = else_body {
                collect_stmt_lambdas_block(eb, out);
            }
        }
        Stmt::While { cond, guard, body, else_body } => {
            collect_expr_lambdas(cond, out);
            if let Some(g) = guard {
                collect_expr_lambdas(g, out);
            }
            collect_stmt_lambdas_block(body, out);
            if let Some(eb) = else_body {
                collect_stmt_lambdas_block(eb, out);
            }
        }
        Stmt::WhileLet { expr, guard, body, .. } => {
            collect_expr_lambdas(expr, out);
            if let Some(g) = guard {
                collect_expr_lambdas(g, out);
            }
            collect_stmt_lambdas_block(body, out);
        }
        Stmt::Match { scrutinee, arms } => {
            collect_expr_lambdas(scrutinee, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_expr_lambdas(g, out);
                }
                collect_stmt_lambdas_block(&arm.body, out);
            }
        }
        Stmt::Raise { value } => collect_expr_lambdas(value, out),
        Stmt::Assert { cond, message } => {
            collect_expr_lambdas(cond, out);
            if let Some(m) = message {
                collect_expr_lambdas(m, out);
            }
        }
        Stmt::Yield { value } => collect_expr_lambdas(value, out),
        Stmt::YieldFrom { iter } => collect_expr_lambdas(iter, out),
        Stmt::BlockLabel { body, .. }
        | Stmt::CheckerBlock { body, .. }
        | Stmt::Defer { body } => collect_stmt_lambdas_block(body, out),
        Stmt::TryCatch { body, catches, else_body, finally_body } => {
            collect_stmt_lambdas_block(body, out);
            for (_, cb) in catches {
                collect_stmt_lambdas_block(cb, out);
            }
            if let Some(eb) = else_body {
                collect_stmt_lambdas_block(eb, out);
            }
            if let Some(fb) = finally_body {
                collect_stmt_lambdas_block(fb, out);
            }
        }
        Stmt::Block { stmts } => {
            let blk = Block {
                stmts: stmts.clone(),
                ty: IrType::Unit,
                span: super::node::Span::unknown(),
            };
            collect_stmt_lambdas_block(&blk, out);
        }
        _ => {}
    }
}

fn collect_expr_lambdas(expr: &Expr, out: &mut Vec<(Vec<Param>, Expr)>) {
    match &expr.kind {
        ExprKind::Lambda { params, body, .. } => {
            if matches!(&body.kind, ExprKind::AssignExpr { .. }) {
                out.push((params.clone(), (**body).clone()));
            } else if let ExprKind::BlockExpr { block } = &body.kind {
                // 块体闭包（`|a, b| =>` 后换行写语句）：Python lambda 装不下语句，
                // 走 gen_expr 的 BlockExpr 分支会**只留尾表达式**（前缀语句静默丢失）⇒ 同样要 hoist 成 def。
                if lambda_block_needs_def(block) {
                    out.push((params.clone(), (**body).clone()));
                }
                collect_expr_lambdas(body, out);
            } else {
                collect_expr_lambdas(body, out);
            }
        }
        ExprKind::Call { callee, args, .. } => {
            collect_expr_lambdas(callee, out);
            for a in args {
                collect_expr_lambdas(a, out);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_expr_lambdas(receiver, out);
            for a in args {
                collect_expr_lambdas(a, out);
            }
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_expr_lambdas(lhs, out);
            collect_expr_lambdas(rhs, out);
        }
        ExprKind::UnOp { operand, .. } => collect_expr_lambdas(operand, out),
        ExprKind::IfExpr { cond, then, els } => {
            collect_expr_lambdas(cond, out);
            collect_expr_lambdas(then, out);
            collect_expr_lambdas(els, out);
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                collect_expr_lambdas(e, out);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                collect_expr_lambdas(a, out);
            }
        }
        ExprKind::BlockExpr { block } => collect_stmt_lambdas_block(block, out),
        ExprKind::TupleLit(es) | ExprKind::Tuple(es) | ExprKind::ListLit(es) | ExprKind::List(es) => {
            for e in es {
                collect_expr_lambdas(e, out);
            }
        }
        ExprKind::Dict(entries) => {
            for (k, v) in entries {
                collect_expr_lambdas(k, out);
                collect_expr_lambdas(v, out);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_expr_lambdas(s, out);
            }
            collect_expr_lambdas(end, out);
        }
        ExprKind::Pipe { receiver, callee, args } => {
            collect_expr_lambdas(receiver, out);
            collect_expr_lambdas(callee, out);
            for a in args {
                collect_expr_lambdas(a, out);
            }
        }
        ExprKind::Paren(e) | ExprKind::Spread(e) => collect_expr_lambdas(e, out),
        ExprKind::Cast { expr: e, .. } => collect_expr_lambdas(e, out),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_expr_lambdas(a, out);
            }
        }
        ExprKind::FieldAccess { base, .. }
        | ExprKind::IndexGet { base, .. }
        | ExprKind::ImplicitConvert { source: base, .. } => collect_expr_lambdas(base, out),
        _ => {}
    }
}

/// IfExpr 语句化：`if cond: { stmts } else: { stmts }` 形态的 IfExpr（builder 将
/// LZ 的 if/elif 语句生成 ExprStmt(IfExpr)）在语句位置提升为 Python if/else。
/// 返回 true 表示已按语句生成。
fn gen_ifexpr_stmt(cg: &mut CythonCodeGen, expr: &Expr, raises_mode: bool) -> bool {
    let ExprKind::IfExpr { cond, then, els } = &expr.kind else {
        return false;
    };
    // then 需为可语句化的形态：BlockExpr / 嵌套 IfExpr / panic!（raise 产物）
    let then_is_panic = matches!(&then.kind, ExprKind::Call { callee, .. }
        if matches!(&callee.kind, ExprKind::Var(n) if n == "panic!"));
    match &then.kind {
        ExprKind::BlockExpr { .. } | ExprKind::IfExpr { .. } => {}
        _ if then_is_panic => {}
        _ => return false,
    }
    cg.writeln(&format!("if {}:", gen_expr(cg, cond)));
    cg.indent += 1;
    if then_is_panic {
        // panic! 特判产物是 raise 语句，直接输出
        cg.writeln(&gen_expr(cg, then));
    } else {
        match &then.kind {
            ExprKind::BlockExpr { block } => gen_block(cg, block, raises_mode),
            ExprKind::IfExpr { .. } => {
                // 嵌套 IfExpr（嵌套 if 语句）递归语句化
                if !gen_ifexpr_stmt(cg, then, raises_mode) {
                    cg.writeln("pass");
                }
            }
            _ => {
                cg.writeln("pass");
            }
        }
    }
    cg.indent -= 1;
    match &els.kind {
        ExprKind::BlockExpr { block } => {
            cg.writeln("else:");
            cg.indent += 1;
            gen_block(cg, block, raises_mode);
            cg.indent -= 1;
        }
        ExprKind::IfExpr { .. } => {
            // elif 链：递归语句化
            cg.writeln("else:");
            cg.indent += 1;
            if !gen_ifexpr_stmt(cg, els, raises_mode) {
                cg.writeln("pass");
            }
            cg.indent -= 1;
        }
        _ => {
            // else 为普通表达式（None 哨兵）→ 无可执行语句，省略 else
        }
    }
    true
}

/// IfExpr 的块提升值提取（Let 等值位置）：把 then/els 块的语句平铺 + 尾值绑定
/// 不适用时返回 None
#[allow(dead_code)]
fn ifexpr_block_tails(expr: &Expr) -> Option<(&Block, &Block)> {
    let ExprKind::IfExpr { then, els, .. } = &expr.kind else {
        return None;
    };
    let ExprKind::BlockExpr { block: tb } = &then.kind else {
        return None;
    };
    let ExprKind::BlockExpr { block: eb } = &els.kind else {
        return None;
    };
    Some((tb, eb))
}

fn gen_stmt(cg: &mut CythonCodeGen, stmt: &Stmt, raises_mode: bool) {
    match stmt {
        Stmt::Let {
            name,
            value,
            is_mut,
            is_ref,
            mods,
            ..
        } => {
            // 关键字降级：None 作为绑定名 → None_（Python 关键字）
            let name = if name == "None" { "None_" } else { name.as_str() };
            // 绑定位解析：该名若已是某个 `ref` 的别名（`ref r = x`），对它的一切再赋值都要
            // 落到它指向的位置上。IR 把「对已有可变变量的赋值」也发成 `Stmt::Let`
            // （实测 pyx：`r = 100  # mut`），所以写侧必须在这里解析，
            // 只改 `ExprKind::Var` 的读侧不够。
            let bind_to = cg
                .ref_alias
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_string());
            // IrMods 修饰符注释
            if !mods.is_empty() {
                cg.writeln(&format!(
                    "# mods: static={}, shared={:?}, interior={:?}, lazy={}, source={}",
                    mods.storage_static,
                    mods.shared,
                    mods.interior,
                    mods.lazy,
                    mods.source
                ));
            }
            // BlockExpr 语句提升：let x = { stmts; expr } → 前缀语句 + x = expr
            if let ExprKind::BlockExpr { block } = &value.kind {
                let tail = gen_block_prefix(cg, block, raises_mode).unwrap_or_else(|| "None".into());
                cg.writeln(&format!("{} = {}", name, tail));
                return;
            }
            // IfExpr 块值提升：let x = if c: {..} else: {..} → if/else 内分别赋值
            // （then 为 panic! 时分支内直接 raise，无赋值）
            if let ExprKind::IfExpr { cond, then, els } = &value.kind {
                let then_panic = matches!(&then.kind, ExprKind::Call { callee, .. }
                    if matches!(&callee.kind, ExprKind::Var(n) if n == "panic!"));
                let (tb, eb) = match (&then.kind, &els.kind) {
                    (ExprKind::BlockExpr { block: t }, ExprKind::BlockExpr { block: e }) => {
                        (Some(t), Some(e))
                    }
                    _ if then_panic => (None, None),
                    _ => {
                        // 普通三元 → 表达式路径
                        cg.writeln(&format!("{} = {}", name, gen_expr(cg, value)));
                        return;
                    }
                };
                cg.writeln(&format!("if {}:", gen_expr(cg, cond)));
                cg.indent += 1;
                if let Some(b) = tb {
                    let t_tail =
                        gen_block_prefix(cg, b, raises_mode).unwrap_or_else(|| "None".into());
                    cg.writeln(&format!("{} = {}", name, t_tail));
                } else if then_panic {
                    cg.writeln(&gen_expr(cg, then));
                } else {
                    cg.writeln(&format!("{} = {}", name, gen_expr(cg, then)));
                }
                cg.indent -= 1;
                cg.writeln("else:");
                cg.indent += 1;
                if let Some(b) = eb {
                    let e_tail =
                        gen_block_prefix(cg, b, raises_mode).unwrap_or_else(|| "None".into());
                    cg.writeln(&format!("{} = {}", name, e_tail));
                } else {
                    // then 为 panic（raise 语句已输出）时 else 为普通表达式赋值
                    let mut e_s = gen_expr(cg, els);
                    if let Some(stripped) = e_s.strip_prefix("raise ") {
                        e_s = stripped.to_string();
                    }
                    cg.writeln(&format!("{} = {}", bind_to, e_s));
                }
                cg.indent -= 1;
                return;
            }
            if *is_ref {
                // ref 是**别名**，不是拷贝：旧实现发 `r = x` 后，`r = 100` 只改到 r，
                // `print(x)` 仍是旧值 ⇒ ref_binding.lz 实测 cy 出 `100,42`、oracle 出
                // `100,100`（静默错值）。Cython 没有引用类型，这里改成发射期别名：
                // 绑到已有位置（裸名/字段访问）就把该名登记为指向它；
                // 绑到字面量/表达式则先落一个隐式存放位 `__ref_<名>` 再登记。
                let place = match &value.kind {
                    ExprKind::Var(_) | ExprKind::FieldAccess { .. } => gen_expr(cg, value),
                    _ => {
                        let hidden = format!("__ref_{}", name);
                        let v = gen_expr(cg, value);
                        cg.writeln(&format!("{} = {}", hidden, v));
                        hidden
                    }
                };
                cg.ref_alias.insert(name.to_string(), place.clone());
                cg.writeln(&format!("# ref {} = {}", name, place));
            } else if *is_mut {
                cg.writeln(&format!("{} = {}  # mut", bind_to, gen_expr(cg, value)));
            } else {
                cg.writeln(&format!("{} = {}", bind_to, gen_expr(cg, value)));
            }
        }
        Stmt::Assign { target, value } => {
            // BlockExpr 语句提升
            if let ExprKind::BlockExpr { block } = &value.kind {
                let tail = gen_block_prefix(cg, block, raises_mode).unwrap_or_else(|| "None".into());
                cg.writeln(&format!("{} = {}", gen_expr(cg, target), tail));
                return;
            }
            cg.writeln(&format!(
                "{} = {}",
                gen_expr(cg, target),
                gen_expr(cg, value)
            ));
        }
        Stmt::Return { value } => {
            // BlockExpr 语句提升
            if let Some(v) = value {
                if let ExprKind::BlockExpr { block } = &v.kind {
                    let tail = gen_block_prefix(cg, block, raises_mode);
                    match tail {
                        Some(t) => cg.writeln(&format!("return {}", t)),
                        None => cg.writeln("return"),
                    }
                    return;
                }
                // IfExpr 块值提升：return if c: {..} else: {..} → if/else 内分别 return
                if let ExprKind::IfExpr { cond, then, els } = &v.kind {
                    let tb = match &then.kind {
                        ExprKind::BlockExpr { block } => Some(block),
                        _ => None,
                    };
                    let eb = match &els.kind {
                        ExprKind::BlockExpr { block } => Some(block),
                        _ => None,
                    };
                    if tb.is_some() || eb.is_some() {
                        // raise 语句性产物不能作为 return 值 → 直接输出语句
                        let emit_tail = |s: String| -> String {
                            if s.starts_with("raise ") {
                                s
                            } else {
                                format!("return {}", s)
                            }
                        };
                        cg.writeln(&format!("if {}:", gen_expr(cg, cond)));
                        cg.indent += 1;
                        if let Some(b) = tb {
                            let b_tail = gen_block_prefix(cg, b, raises_mode);
                            match b_tail {
                                Some(t) => cg.writeln(&emit_tail(t)),
                                None => cg.writeln("return"),
                            }
                        } else {
                            let s = gen_expr(cg, then);
                            cg.writeln(&emit_tail(s));
                        }
                        cg.indent -= 1;
                        cg.writeln("else:");
                        cg.indent += 1;
                        if let Some(b) = eb {
                            let b_tail = gen_block_prefix(cg, b, raises_mode);
                            match b_tail {
                                Some(t) => cg.writeln(&emit_tail(t)),
                                None => cg.writeln("return"),
                            }
                        } else {
                            let s = gen_expr(cg, els);
                            cg.writeln(&emit_tail(s));
                        }
                        cg.indent -= 1;
                        return;
                    }
                }
            }
            match value {
                Some(v) => {
                    let v_s = gen_expr(cg, v);
                    // 异常传播模型：return Err(e) → raise（可被 try/except 捕获）
                    if is_err_ctor(v) {
                        let e_str = match &v.kind {
                            ExprKind::EnumCtor { args, .. } => args
                                .first()
                                .map(|a| gen_expr(cg, a))
                                .unwrap_or_else(|| "None".into()),
                            _ => gen_expr(cg, v),
                        };
                        cg.writeln(&format!("raise LZError({})", e_str));
                    } else if v_s.starts_with("raise ") {
                        // panic!/raise 产物是语句，直接输出
                        cg.writeln(&v_s);
                    } else if raises_mode {
                        // raises 函数：return 值直接裸返回（Ok 包装由异常模型消除）
                        cg.writeln(&format!("return {}", v_s));
                    } else {
                        cg.writeln(&format!("return {}", v_s));
                    }
                }
                None => {
                    if raises_mode {
                        cg.writeln("return None");
                    } else {
                        cg.writeln("return");
                    }
                }
            }
        }
        Stmt::ExprStmt { expr } => {
            // BlockExpr 语句提升：{ stmts; expr } → 前缀语句 + 尾表达式语句
            if let ExprKind::BlockExpr { block } = &expr.kind {
                if let Some(tail) = gen_block_prefix(cg, block, raises_mode) {
                    if !tail.is_empty() {
                        cg.writeln(&tail);
                    }
                }
                return;
            }
            // IfExpr 语句化（builder 将 LZ if/elif 语句生成为 ExprStmt(IfExpr)）
            if matches!(&expr.kind, ExprKind::IfExpr { .. }) && gen_ifexpr_stmt(cg, expr, raises_mode) {
                return;
            }
            // 嵌套 def 被提升为模块级 item 后，语句位留下 Lit(Unit) 占位。
            // Rust 后端对它什么都不发；这里发裸 `None` 是纯噪声（还常落在块首行）。
            if matches!(&expr.kind, ExprKind::Lit(LitKind::Unit)) {
                return;
            }
            let s = gen_expr(cg, expr);
            if !s.is_empty() {
                cg.writeln(&s);
            }
        }
        Stmt::Break => cg.writeln("break"),
        Stmt::Continue => cg.writeln("continue"),
        Stmt::Block { stmts } => {
            // 裸块：独立作用域 → 复用 gen_block（defer 边界对齐）
            let blk = Block {
                stmts: stmts.clone(),
                ty: IrType::Unit,
                span: super::node::Span::unknown(),
            };
            gen_block(cg, &blk, raises_mode);
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            cg.writeln(&format!("if {}:", gen_expr(cg, cond)));
            cg.indent += 1;
            gen_block(cg, then_branch, raises_mode);
            cg.indent -= 1;
            if let Some(eb) = else_branch {
                cg.writeln("else:");
                cg.indent += 1;
                gen_block(cg, eb, raises_mode);
                cg.indent -= 1;
            }
        }
        Stmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            // Python for-else 原生对应 LZ for-else（无 break 时执行）
            cg.writeln(&format!("for {} in {}:", var, gen_expr(cg, iter)));
            cg.indent += 1;
            if let Some(g) = guard {
                cg.writeln(&format!("if not ({}): continue", gen_expr(cg, g)));
            }
            gen_block(cg, body, raises_mode);
            cg.indent -= 1;
            if let Some(eb) = else_body {
                cg.writeln("else:");
                cg.indent += 1;
                gen_block(cg, eb, raises_mode);
                cg.indent -= 1;
            }
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            cg.writeln(&format!("while {}:", gen_expr(cg, cond)));
            cg.indent += 1;
            if let Some(g) = guard {
                cg.writeln(&format!("if not ({}): continue", gen_expr(cg, g)));
            }
            gen_block(cg, body, raises_mode);
            cg.indent -= 1;
            if let Some(eb) = else_body {
                cg.writeln("else:");
                cg.indent += 1;
                gen_block(cg, eb, raises_mode);
                cg.indent -= 1;
            }
        }
        Stmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
        } => {
            // while let pat = expr → while True + 手工解构 + 不匹配 break
            //   while True:
            //       __wlv = <expr>
            //       if not (<pat cond on __wlv> and <guard>): break
            //       <bindings>
            //       body
            let scrut = format!("__wlv_{}", cg.tmp_counter);
            cg.tmp_counter += 1;
            cg.writeln("while True:");
            cg.indent += 1;
            cg.writeln(&format!("{} = {}", scrut, gen_expr(cg, expr)));
            let (pat_cond, bindings) = gen_pattern(cg, pattern, &scrut);
            // guard 引用的绑定名替换为 scrutinee 子表达式
            let cond = match guard {
                Some(g) => {
                    let mut gs = gen_expr(cg, g);
                    for (bname, bval) in &bindings {
                        gs = replace_ident(&gs, bname, bval);
                    }
                    format!("not (({}) and ({}))", pat_cond, gs)
                }
                None => format!("not ({})", pat_cond),
            };
            cg.writeln(&format!("if {}:", cond));
            cg.indent += 1;
            cg.writeln("break");
            cg.indent -= 1;
            for (bname, bval) in &bindings {
                cg.writeln(&format!("{} = {}", bname, bval));
            }
            gen_block(cg, body, raises_mode);
            cg.indent -= 1;
        }
        Stmt::Match { scrutinee, arms } => {
            // match scrut → 唯一临时变量保存被匹配值，if/elif 链 + 模式解构
            let scrut = format!("__scrut_{}", cg.tmp_counter);
            cg.tmp_counter += 1;
            cg.writeln(&format!("{} = {}", scrut, gen_expr(cg, scrutinee)));
            if arms.is_empty() {
                cg.writeln("pass");
                return;
            }
            for (i, arm) in arms.iter().enumerate() {
                let cond = if i == 0 { "if" } else { "elif" };
                let (pat_cond, bindings) = gen_pattern(cg, &arm.pattern, &scrut);
                // guard 守卫条件：guard 引用的模式绑定名在条件判断时尚未绑定，
                // 替换为对应的 scrutinee 子表达式（绑定值无副作用，语义等价）
                let guard_cond = match &arm.guard {
                    Some(g) => {
                        let mut gs = gen_expr(cg, g);
                        for (bname, bval) in &bindings {
                            gs = replace_ident(&gs, bname, bval);
                        }
                        format!("({}) and ({})", pat_cond, gs)
                    }
                    None => pat_cond,
                };
                cg.writeln(&format!("{} {}:", cond, guard_cond));
                cg.indent += 1;
                // 绑定赋值放在臂体内（进入臂才绑定）
                for (bname, bval) in &bindings {
                    cg.writeln(&format!("{} = {}", bname, bval));
                }
                // 臂体尾表达式转 return（match 作为函数尾值场景）
                let arm_body = cg.tail_return_block(&arm.body);
                gen_block(cg, &arm_body, raises_mode);
                cg.indent -= 1;
            }
            // 无匹配兜底（穷尽匹配下不可达；保证语法完整）
            cg.writeln("else:");
            cg.indent += 1;
            cg.writeln("pass");
            cg.indent -= 1;
        }
        Stmt::Raise { value } => {
            // 异常传播模型：raise 统一生成 Python raise（raises_mode 下 Rust 端
            // 生成的 return Err(...) 由 Stmt::Return 分支处理，此处为真 raise）
            let tmp = format!("__raise_{}", cg.tmp_counter);
            cg.tmp_counter += 1;
            let v = gen_expr(cg, value);
            cg.writeln(&format!("{} = {}", tmp, v));
            cg.writeln(&format!("raise {} if isinstance({}, BaseException) else LZError({})", tmp, tmp, tmp));
        }
        Stmt::Assert { cond, message } => {
            if let Some(msg) = message {
                cg.writeln(&format!(
                    "assert {}, {}",
                    gen_expr(cg, cond),
                    gen_expr(cg, msg)
                ));
            } else {
                cg.writeln(&format!("assert {}", gen_expr(cg, cond)));
            }
        }
        Stmt::Yield { value } => cg.writeln(&format!("yield {}", gen_expr(cg, value))),
        Stmt::YieldFrom { iter } => {
            cg.writeln(&format!("yield from {}", gen_expr(cg, iter)));
        }
        Stmt::BreakLabel { label, value } => {
            match value {
                Some(v) => cg.writeln(&format!(
                    "raise _BreakLabel('{}', {})",
                    label,
                    gen_expr(cg, v)
                )),
                None => cg.writeln(&format!("raise _BreakLabel('{}')", label)),
            }
        }
        Stmt::BlockLabel { label, body } => {
            // 'label: { body } → try/except _BreakLabel 捕获内层 break 'label
            cg.writeln("try:");
            cg.indent += 1;
            gen_block(cg, body, raises_mode);
            cg.indent -= 1;
            cg.writeln(&format!("except _BreakLabel as __bl_{}:", label));
            cg.indent += 1;
            cg.writeln(&format!("if __bl_{}.label != '{}':", label, label));
            cg.indent += 1;
            cg.writeln("raise");
            cg.indent -= 1;
            cg.indent -= 1;
        }
        Stmt::CheckerBlock {
            label,
            ps_name,
            default_checker,
            body,
        } => {
            let ps_param = ps_name.as_deref().unwrap_or("ps");
            cg.writeln(&format!("def {}({}):", label, ps_param));
            cg.indent += 1;
            if let Some(dc) = default_checker {
                cg.writeln(&format!("# default_checker: {dc}"));
            }
            gen_block(cg, body, raises_mode);
            cg.indent -= 1;
        }
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            // try/catch → Python try/except/else/finally（异常传播模型）
            cg.writeln("try:");
            cg.indent += 1;
            gen_block(cg, body, raises_mode);
            cg.indent -= 1;
            for (pat, cb) in catches.iter() {
                let head = match pat {
                    None => "except BaseException:".to_string(),
                    Some(p) => match p {
                        Pattern::Ident(n) | Pattern::RefMutIdent(n) => {
                            format!("except BaseException as {}:", n)
                        }
                        // 变体 catch（ParseError.BadFormat(e) 等）→ 统一 BaseException
                        // 绑定（类型过滤在异常模型降级下不可精确区分）
                        Pattern::Enum { args, .. } => match args.first() {
                            Some(Pattern::Ident(n)) | Some(Pattern::RefMutIdent(n)) => {
                                format!("except BaseException as {}:", n)
                            }
                            _ => "except BaseException:".to_string(),
                        },
                        _ => "except BaseException:".to_string(),
                    },
                };
                cg.writeln(&head);
                cg.indent += 1;
                gen_block(cg, cb, raises_mode);
                cg.indent -= 1;
            }
            if let Some(eb) = else_body {
                cg.writeln("else:");
                cg.indent += 1;
                gen_block(cg, eb, raises_mode);
                cg.indent -= 1;
            }
            if let Some(fb) = finally_body {
                cg.writeln("finally:");
                cg.indent += 1;
                gen_block(cg, fb, raises_mode);
                cg.indent -= 1;
            }
        }
        Stmt::Pass => cg.writeln("pass"),
        Stmt::TypeAlias { name, ty } => {
            let t = cg.map_type(ty, TypeCtx::Signature);
            cg.writeln(&format!("# type {} = {}", name, t));
        }
        Stmt::Defer { .. } => {
            // 已由 gen_block 统一收集（此处不应到达）
            cg.writeln("pass  # defer (handled by block flush)");
        }
    }
}

// ══════════════════════════════════════════════════════════════
// 表达式生成
// ══════════════════════════════════════════════════════════════

/// 字符串字面量转义（单双引号统一双引号；真实控制字符转义）
fn escape_str_literal(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// 查找 Block 中第一个 yield 表达式的 IR 类型（生成器构建块 func *: 收集器类型用）
fn first_yield_type(block: &Block) -> Option<IrType> {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Yield { value } => return Some(value.ty.clone()),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(t) = first_yield_type(then_branch) {
                    return Some(t);
                }
                if let Some(e) = else_branch {
                    if let Some(t) = first_yield_type(e) {
                        return Some(t);
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                if let Some(t) = first_yield_type(body) {
                    return Some(t);
                }
            }
            Stmt::ExprStmt { expr } => {
                if let Some(t) = expr_first_yield_type(expr) {
                    return Some(t);
                }
            }
            Stmt::Let { value, .. } => {
                if let Some(t) = expr_first_yield_type(value) {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    None
}

/// 查找 Expr 中第一个 yield 表达式的 IR 类型
fn expr_first_yield_type(expr: &Expr) -> Option<IrType> {
    match &expr.kind {
        ExprKind::BlockExpr { block } => first_yield_type(block),
        ExprKind::GenBuild { block, .. } => first_yield_type(block),
        ExprKind::IfExpr { then, els, .. } => {
            expr_first_yield_type(then).or_else(|| expr_first_yield_type(els))
        }
        ExprKind::Call { callee, args, .. } => {
            expr_first_yield_type(callee).or_else(|| args.iter().find_map(expr_first_yield_type))
        }
        ExprKind::Lambda { body, .. } => expr_first_yield_type(body),
        _ => None,
    }
}

fn gen_expr(cg: &CythonCodeGen, expr: &Expr) -> String {
    // float→complex 提升：将 f64 表达式提升为 Python complex 字面量
    fn gen_complex_promote(cg: &CythonCodeGen, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Lit(LitKind::F64(f)) => format!("({} + 0j)", f),
            _ => format!("({} + 0j)", gen_expr(cg, expr)),
        }
    }
    match &expr.kind {
        ExprKind::Lit(lit) => match lit {
            LitKind::Int(n) => n.to_string(),
            LitKind::Int128(n) => n.to_string(),
            // BigInt：Python 原生 int 任意精度，字符串形式直出
            LitKind::BigInt(s) => s.clone(),
            LitKind::Complex(re, im) => format!("({} + {}j)", re, im),
            LitKind::F64(f) => {
                // 特殊浮点值：NaN/inf/-inf 需用 float() 构造（Python 无对应字面量）
                if f.is_nan() {
                    "float('nan')".to_string()
                } else if f.is_infinite() {
                    if *f > 0.0 { "float('inf')".to_string() } else { "float('-inf')".to_string() }
                } else {
                    let s = f.to_string();
                    if s.contains('.') || s.contains('e') {
                        s
                    } else {
                        format!("{}.0", s)
                    }
                }
            }
            LitKind::Str(s) => format!("\"{}\"", escape_str_literal(s)),
            LitKind::FStr(s) => format!("f\"{}\"", fs_interp(&escape_str_literal(s))),
            LitKind::Bool(b) => {
                if *b {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            }
            LitKind::None_ => "None".to_string(),
            LitKind::Unit => "None".to_string(),
        },
        ExprKind::Var(name) => {
            // `ref r = x` 之后每次出现裸名 `r`，都改写成它指向的位置（发射期别名，
            // 动机与边界见 CythonCodeGen::ref_alias 字段注释）。写侧两条路都要覆盖：
            // 真 `Stmt::Assign` 的 target 经这里（自然变成 `x = 100`），而 IR 把
            // 「对已有可变变量的赋值」发成 `Stmt::Let`，那条在 Let 的 `bind_to` 处解析。
            if let Some(place) = cg.ref_alias.get(name.as_str()) {
                return place.clone();
            }
            match name.as_str() {
                // LZ 内建输出函数 → Python print
                "println" | "print" => "print".to_string(),
                // 已合并模块的限定名 → 裸名（`lz_std.Some` → `Some`）
                _ => cg.strip_merged_qualifier(name),
            }
        }
        ExprKind::Default => default_value_for(&expr.ty),
        ExprKind::Spread(inner) => format!("*{}", gen_expr(cg, inner)),
        ExprKind::Call {
            callee,
            args,
            type_args: _,
        } => {
            let f = gen_expr(cg, callee);
            // ── print/println 走 Rust 的 `{:?}`（Debug）口径 ──
            // 逐实参套 `_lz_dbg(...)`：字符串带引号、bool→true/false、None→None、容器递归。
            // Rust 端 `print(a, b)` 发 `println!("{:?} {:?}", a, b)`（空格连接，见
            // src/ir/codegen/mod.rs 的 print 分支），Python `print(x, y)` 也是空格连接 ⇒
            // 只需包装参数。两套 stdout 一旦不一致，跨后端差分（L3 的 oracle）就无从谈起。
            if f == "print" {
                let a: Vec<String> = args
                    .iter()
                    .map(|e| format!("_lz_dbg({})", gen_expr(cg, e)))
                    .collect();
                return format!("print({})", a.join(", "));
            }
            // ── go/spawn：同步上下文必须把体裹成 thunk 交给线程 ──
            // IR 把 `go e`/`spawn e` 统一降成 `__go(e)`（src/ir/builder.rs:6331），
            // 而 `__go(v)` 的 v 在调用前就求值完了 ⇒ 垫片无论怎么写都只能同步跑（BUG-12）。
            // rust 面在同样位置发 `std::thread::spawn(move || e)`，实测失败隔离行为：
            // 任务体里 assert 失败**不打断主流程**（stdout 仍有 "after go"、退出码 0），
            // 语料 CY/TESTS/11_concurrency/go_failure_isolated.lz 钉的就是这一条。
            // async 上下文不动：那里的体是协程对象，`await` 由 Python 驱动，跨后端 stdout 已实测一致。
            if f == "__go" && !cg.current_fn_is_async {
                let body = args
                    .first()
                    .map(|a| gen_expr(cg, a))
                    .unwrap_or_else(|| "None".into());
                return format!("__spawn(lambda: {})", body);
            }
            // ── has_new struct 构造路由：T(...) → T__new__(...)（对齐 Rust 端）──
            let routed = match &callee.kind {
                ExprKind::Var(n) if cg.has_new_structs.contains(n) => format!("{}__new__", n),
                _ => f.clone(),
            };
            // ── 内建断言降级：assert_eq!(a, b) → assert a == b；assert!(e) → assert e ──
            if f == "assert_eq!" {
                if args.len() >= 2 {
                    return format!(
                        "assert {} == {}",
                        gen_expr(cg, &args[0]),
                        gen_expr(cg, &args[1])
                    );
                }
                if args.len() == 1 {
                    return format!("assert {}", gen_expr(cg, &args[0]));
                }
            }
            if f == "assert!" {
                if let Some(a) = args.first() {
                    return format!("assert {}", gen_expr(cg, a));
                }
            }
            // panic!(msg) → raise LZError(msg)
            if f == "panic!" || f == "panic" {
                let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
                return format!("raise LZError({})", a.join(", "));
            }
            // callee 为 Lambda → 括号包裹（避免立即调用 `()` 绑定进 lambda 体）
            if matches!(&callee.kind, ExprKind::Lambda { .. }) {
                let lambda_s = gen_expr(cg, callee);
                let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
                return format!("({})({})", lambda_s, a.join(", "));
            }
            // ── Option/Result 构造（Call 形态）：Some(x)/Ok(x)/Err(x)/None ──
            match f.as_str() {
                "Some" => {
                    return args
                        .first()
                        .map(|a| gen_expr(cg, a))
                        .unwrap_or_else(|| "None".into());
                }
                "Ok" => {
                    return args
                        .first()
                        .map(|a| gen_expr(cg, a))
                        .unwrap_or_else(|| "None".into());
                }
                "Err" => {
                    let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
                    return format!("LZError({})", a.join(", "));
                }
                "None" => return "None".to_string(),
                // set!(items...) → Python set（Rust 端映射 HashSet）
                "set!" => {
                    let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
                    if a.is_empty() {
                        return "set()".to_string();
                    }
                    return format!("{{{}}}", a.join(", "));
                }
                // Exception(msg) → LZError（Rust 端 panic 基；Cython 异常模型统一 LZError）
                "Exception" => {
                    let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
                    return format!("LZError({})", a.join(", "));
                }
                // eprintln!(args) → print(args, file=sys.stderr)
                "eprintln!" => {
                    let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
                    return format!("print({}, file=sys.stderr)", a.join(", "));
                }
                _ => {}
            }
            // 推导式宏族：comp!/comp_outer!/comp_leaf!/dict_comp_*!/set_comp_*!
            // comp!(f, iter[, filter]) → list comp；dict_comp → dict；set_comp → set
            if f.ends_with('!') && f.contains("comp") && args.len() >= 2 {
                let kind = f.trim_end_matches('!');
                let is_dict = kind.contains("dict_comp");
                let is_set = !is_dict && kind.contains("set_comp");
                let f_s = gen_expr(cg, &args[0]);
                let it = gen_expr(cg, &args[1]);
                let filter = if args.len() >= 3 {
                    let fe = gen_expr(cg, &args[2]);
                    // filter 为 lambda 时需补谓词调用（对齐映射位 `(__cv)` 形态，
                    // 否则 lambda 对象恒真，过滤失效）
                    let is_lambda = matches!(&args[2].kind, ExprKind::Lambda { .. });
                    if is_lambda {
                        format!(" if ({}){}", fe, "(__cv)")
                    } else {
                        format!(" if ({})", fe)
                    }
                } else {
                    String::new()
                };
                let inner = format!("({})(__cv) for __cv in ({}){}", f_s, it, filter);
                // dict_comp 的 f 返回 (k, v) 二元组
                if is_dict {
                    return format!("dict([{}])", inner);
                }
                if is_set {
                    return format!("{{{}}}", inner);
                }
                return format!("[{}]", inner);
            }
            // ── 关键字参数 _KwArg 内联：f(_KwArg(name="x", value=3)) → f(x = 3) ──
            let a: Vec<String> = args
                .iter()
                .map(|arg| {
                    if let ExprKind::StructCtor { name, fields } = &arg.kind {
                        if name == "_KwArg" {
                            let k = fields.iter().find(|(n, _)| n == "name").and_then(
                                |(_, v)| match &v.kind {
                                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                                    _ => None,
                                },
                            );
                            let v = fields
                                .iter()
                                .find(|(n, _)| n == "value")
                                .map(|(_, e)| gen_expr(cg, e));
                            if let (Some(k), Some(v)) = (k, v) {
                                return format!("{} = {}", k, v);
                            }
                        }
                    }
                    gen_expr(cg, arg)
                })
                .collect();
            // ── type_args 泛型参数：Cython 无泛型运行时，静默擦除（注释放括号内
            // 会吞掉闭合括号，故不生成）──
            // checker 派发：callee 为 `fn[checker]` 形态 → 改写为包装器调用
            // （包装器在 generate 收尾统一追加，形参名 zip 位置实参进 kwargs，
            //   checker 先行检查再透传；对齐 SYNTAX/03c-检查站 __Params 接口）
            let routed = match parse_checker_index(&routed) {
                Some((fname, cname)) => {
                    let params = cg.fn_params.get(&fname).cloned().unwrap_or_default();
                    let zip_list = if params.is_empty() {
                        "()".to_string()
                    } else {
                        let names = params
                            .iter()
                            .map(|p| format!("'{p}'"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("({names},)")
                    };
                    let wrapper = format!("__lz_checked_{fname}_{cname}");
                    let text = format!(
                        "def {wrapper}(*__args, **__kwargs):\n    __params = __Params(kwargs=dict(list(zip({zip_list}, __args)), **__kwargs))\n    {cname}(__params)\n    return {fname}(*__args, **__kwargs)"
                    );
                    if !cg.checker_wrappers.borrow().contains(&text) {
                        cg.checker_wrappers.borrow_mut().push(text);
                    }
                    wrapper
                }
                None => routed,
            };
            format!("{}({})", routed, a.join(", "))
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            // Enum.Variant(args) 构造形态（builder 将 `Shape.Circle(radius: 5)`
            // 生成 MethodCall）→ 直接引用变体类构造
            if let ExprKind::Var(b) = &receiver.kind {
                // 内建 Option/Result 伴随调用：Option.Some(v) → v；Result.Ok(v) → v
                if b == "Option" || b == "Result" {
                    if method == "Some" || method == "Ok" {
                        return args
                            .first()
                            .map(|a| gen_expr(cg, a))
                            .unwrap_or_else(|| "None".into());
                    }
                    if method == "Err" {
                        let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
                        return format!("LZError({})", a.join(", "));
                    }
                    if method == "None" {
                        return "None".to_string();
                    }
                }
                // 已平铺合并模块的限定调用：`lz_std.Some(42)` → `Some(42)`。
                // 那些定义就在同一份 .pyx 里，Python 侧不存在叫 lz_std 的模块。
                if cg.merged_modules.contains(b.as_str()) {
                    let a: Vec<String> = args.iter().map(|arg| gen_expr(cg, arg)).collect();
                    return format!("{}({})", mangle_ident(method), a.join(", "));
                }
                if let Some(vs) = cg.enum_variants.get(b) {
                    if vs.contains(method) {
                        // _KwArg 内联（与 Call 分支一致的关字参数处理）
                        let a: Vec<String> = args
                            .iter()
                            .map(|arg| {
                                if let ExprKind::StructCtor { name, fields } = &arg.kind {
                                    if name == "_KwArg" {
                                        let k = fields
                                            .iter()
                                            .find(|(n, _)| n == "name")
                                            .and_then(|(_, v)| match &v.kind {
                                                ExprKind::Lit(LitKind::Str(s)) => {
                                                    Some(s.clone())
                                                }
                                                _ => None,
                                            });
                                        let v = fields
                                            .iter()
                                            .find(|(n, _)| n == "value")
                                            .map(|(_, e)| gen_expr(cg, e));
                                        if let (Some(k), Some(v)) = (k, v) {
                                            return format!("{} = {}", k, v);
                                        }
                                    }
                                }
                                gen_expr(cg, arg)
                            })
                            .collect();
                        if a.is_empty() {
                            // 无数据变体 → 单例引用
                            return mangle_ident(method);
                        }
                        return format!("{}({})", mangle_ident(method), a.join(", "));
                    }
                }
            }
            // await 调用形态（`fut.await()` → `await fut`，async 上下文）
            if method == "await" && args.is_empty() {
                return format!("await {}", gen_expr(cg, receiver));
            }
            let r = gen_expr(cg, receiver);
            // Option/Result 方法族 → 裸 None 哨兵语义（对齐 PLAN §1.6 的 `is None` 模型；
            // 裸 None 无实例方法，故生成语义等价表达式而非方法调用）
            match method.as_str() {
                "is_none" => return format!("({} is None)", r),
                "is_some" => return format!("({} is not None)", r),
                // unwrap/expect：有值透传；None 时的 panic 语义由运行层 raise（见 BACKLOG）
                "unwrap" => return r,
                // LZ 的 `x?` 在 IR 里就是 `try_into()`（Rust 后端发 `.unwrap()`）⇒ 同一口径透传。
                // 落到通用兜底会得到 `x.try_into()`：AttributeError: 'int' object has no attribute 'try_into'
                // （DEMO/13_operators/precedence.lz 实测）。
                "try_into" if args.is_empty() => return r,
                // LZ 的 Vec 方法名 `push` → Python list 的 `append`；
                // prelude 只有自由函数 `push(seq,item)`，方法形态 `xs.push(v)` 落到通用兜底就是
                // AttributeError: type object 'list' has no attribute 'push'
                // （DEMO/01_basics/polish_18_iterator.lz 实测）。
                // 反向闸门：接收者是用户类实例时保留原方法名（用户自己定义的 push 不许改写）。
                "push" if args.len() == 1 && !ty_is_user_class(cg, &receiver.ty) => {
                    let v = gen_expr(cg, &args[0]);
                    return format!("{}.append({})", r, v);
                }
                // `.clone()`：with 脱糖在**共享 IR** 里就发了它（builder.rs:7864 把 guard
                // 作为 `alias.__exit__(alias.clone())` 的实参），cy 侧没有 Clone trait ⇒
                // 落到通用兜底就是 AttributeError: 'MyResource' object has no attribute 'clone'
                // （DEMO/06_control_flow/with_defer.lz 实测）。透传与本后端已声明的引用模型一致
                // （见 UnOp Ref/MutRef「Python 对象本为引用 ⇒ 透传」），不是新口径。
                "clone" if args.is_empty() => return r,
                // `.len()`：Rust 后端把 `s.len()` 原样交给 rustc（str/slice 的内建关联方法），
                // Python 侧 str/list/dict 都没有 `.len()` 方法 ⇒
                // AttributeError: type object 'str' has no attribute 'len'
                // （DEMO/03_variables/owned_binding.lz、param_modifiers.lz 实测；cy 语料此前零覆盖）。
                // 规范口径是自由函数：`len(x)` 走 `__len__`（SYNTAX/99-内置预导入库.md:128）
                // ⇒ 方法形态改写成自由函数，与本后端的 `len(x)` 同一条实现路径。
                // 反向闸门：用户类自定义 len 保留方法形态（与 push 同一纪律）。
                "len" if args.is_empty() && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("len({})", r);
                }
                // 同一族的其余成员：LZ/Rust 侧的内建方法形态在 Python 上没有同名方法，
                // 落到通用兜底一律 AttributeError（本表每条都带实测来源，不猜）。
                // `.length()` 是**规范口径**的实例方法（SYNTAX/99-内置预导入库.md:152
                // 「`.length()` 是 List/str 的实例方法」），Rust 后端把它改名成 `len`
                // （codegen/mod.rs `"length" => "len"`）⇒ cy 同改写为自由函数。
                // 实测：TEMP/probe6/p_str_len.lz、p_list_len.lz（rust=3,3,3 / cy AttributeError:
                // type object 'list' has no attribute 'length'）。
                "length" if args.is_empty() && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("len({})", r);
                }
                // `.is_empty()`：Rust str/Vec/HashMap 的内建方法（同 `.len()` 走的是「另一后端
                // 直漏给 rustc」这条路，两后端的点名单里都看不见它 ⇒ 表差集判据的盲区，靠探针才现形）。
                // 实测：TEMP/probe6/p_is_empty.lz（rust=false,false / cy AttributeError:
                // type object 'list' has no attribute 'is_empty'）。
                "is_empty" if args.is_empty() && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("(len({}) == 0)", r);
                }
                // `starts_with`/`ends_with`/`trim`：Rust 侧改名表里就写着它们（`"trim" => "trim"`
                // 之类），Python 方法是另一个名字 ⇒ 不改写就是 AttributeError。
                // 实测来源：DEMO/01_basics/strings.lz、polish_28_strings.lz 用过这三个（TEMP/diff_method_tables.py 清单）。
                "starts_with" if args.len() == 1 && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("{}.startswith({})", r, gen_expr(cg, &args[0]));
                }
                "ends_with" if args.len() == 1 && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("{}.endswith({})", r, gen_expr(cg, &args[0]));
                }
                "trim" if args.is_empty() && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("{}.strip()", r);
                }
                // `contains`：Rust 侧对结构体接收者有特殊派发（`"contains" if recv_is_struct …`），
                // 内建接收者上 Python 没有 `.contains()` 方法，只有 `in` 运算符
                // （str/list/dict-keys 三者语义一致）。实测来源：DEMO/01_basics/dicts.lz。
                "contains" if args.len() == 1 && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("({} in {})", gen_expr(cg, &args[0]), r);
                }
                // `remove(i)`/`remove(k)`：LZ 侧按**位置/键**移除（DEMO/01_basics/lists.lz
                // 「zs.remove(1) // 移除位置 1」；dicts.lz 对 Dict 同义）。
                // Python 的 `list.remove(v)` 按**值**移除、`dict` 根本没有 remove ⇒
                // 不改写会静默移错元素（lists.lz 实测 AssertionError）或
                // AttributeError: type object 'dict' has no attribute 'remove'（dicts.lz 实测）。
                // `.pop(i)`/`.pop(k)` 在 list/dict 上都正是「按位置/按键移除并返回被移除项」，
                // 与 Rust 侧 `"remove" => "remove"`（Vec::remove 按索引）同口径。
                "remove" if args.len() == 1 && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("{}.pop({})", r, gen_expr(cg, &args[0]));
                }
                // `to_lower()`/`to_upper()`：Rust 后端点名改名到 to_lowercase/to_uppercase
                // （`"to_lower" => "to_lowercase"`），Python str 的方法名是 lower/upper。
                // 实测：DEMO/01_basics/strings.lz 的 `cleaned.to_lower()`（AttributeError:
                // 'str' object has no attribute 'to_lower'）。
                "to_lower" if args.is_empty() && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("{}.lower()", r);
                }
                "to_upper" if args.is_empty() && !ty_is_user_class(cg, &receiver.ty) => {
                    return format!("{}.upper()", r);
                }
                "expect" => {
                    let m = args.first().map(|a| gen_expr(cg, a)).unwrap_or_default();
                    if m.is_empty() {
                        return r;
                    }
                    return format!("({} if {} is not None else __lz_expect_fail({}))", r, r, m);
                }
                _ => {}
            }
            let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
            format!("{}.{}({})", r, method, a.join(", "))
        }
        ExprKind::FieldAccess { base, field } => {
            // 内建 Option/Result 伴随访问：Option.None → None
            if let ExprKind::Var(b) = &base.kind {
                if (b == "Option" || b == "Result") && field == "None" {
                    return "None".to_string();
                }
                // 已平铺合并模块的限定访问：`lz_std.VERSION` → `VERSION`
                if cg.merged_modules.contains(b.as_str()) {
                    return mangle_ident(field);
                }
            }
            // Enum.Variant 访问形态 → 直接引用变体（无数据变体为单例实例，
            // 有数据变体为类，可继续 Call 构造）。变体名为关键字 → mangle
            if let ExprKind::Var(b) = &base.kind {
                if let Some(vs) = cg.enum_variants.get(b) {
                    if vs.contains(field) {
                        return mangle_ident(field);
                    }
                }
            }
            // 元组位置字段 `.0` / `.1`（Rust 形态）→ Python 下标 `[0]` / `[1]`
            if !field.is_empty() && field.chars().all(|c| c.is_ascii_digit()) {
                return format!("{}[{}]", gen_expr(cg, base), field);
            }
            format!("{}.{}", gen_expr(cg, base), field)
        }
        ExprKind::IndexGet { base, key } => {
            // 切片形态的索引：`xs[1..3]` 在 IR 里 key 是 StructCtor("Range")，而 Python 的
            // `[]` 收到 range 对象会 TypeError: list indices must be integers or slices,
            // not range（DEMO/01_basics/lists.lz 实测）。规范口径写明序列切片按
            // **Python 语义**（SYNTAX/01-类型系统.md:436）⇒ 这里发切片语法而非 range()。
            if let Some(sl) = range_slice(cg, key) {
                return format!("{}[{}]", gen_expr(cg, base), sl);
            }
            // str 索引的元素类型：Rust 后端发 `chars().collect()[i] as i64`（码点整数，
            // 越界给 0——实测 TEMP/xdiff-probe7/rs/strings/strings.rs:38），Python 原生给
            // 单字符 str ⇒ `assert "abc"[0] == 97` 在 cy 面红（DEMO/01_basics/strings.lz:114）。
            // 两侧同为**码点**口径（都不是 UTF-8 字节），非 ASCII 也一致。
            if matches!(base.ty, IrType::Str) {
                return format!(
                    "_lz_idx_str({}, {})",
                    gen_expr(cg, base),
                    gen_expr(cg, key)
                );
            }
            format!("{}[{}]", gen_expr(cg, base), gen_expr(cg, key))
        }
        ExprKind::BinOp { op, lhs, rhs } => {
            let o = match op {
                BinOpKind::Add => "+",
                BinOpKind::Sub => "-",
                // LZ 整数除法语义对齐 Rust（截断整除）→ //（真除 / 会产生 float）
                BinOpKind::Div => {
                    if matches!(lhs.ty, IrType::Int) {
                        "//"
                    } else {
                        "/"
                    }
                }
                BinOpKind::Mul => "*",
                BinOpKind::Mod => "%",
                BinOpKind::Eq => "==",
                BinOpKind::Neq => "!=",
                BinOpKind::Lt => "<",
                BinOpKind::Gt => ">",
                BinOpKind::Le => "<=",
                BinOpKind::Ge => ">=",
                BinOpKind::And => "and",
                BinOpKind::Or => "or",
                BinOpKind::BitAnd => "&",
                BinOpKind::BitOr => "|",
                BinOpKind::Xor => "^",
                BinOpKind::Shl => "<<",
                BinOpKind::Shr => ">>",
                BinOpKind::Pow => "**",
                BinOpKind::In => "in",
                BinOpKind::NotIn => "not in",
            };
            // float→complex 提升：当 BinOp 整体类型为 Complex 而某侧为 f64 时，
            // 将 f64 字面量/表达式提升为 complex(re, im) 形式
            let is_complex_binop = matches!(expr.ty, IrType::Complex);
            let lhs_str = if is_complex_binop && matches!(lhs.ty, IrType::F64) {
                gen_complex_promote(cg, lhs)
            } else {
                gen_expr(cg, lhs)
            };
            let rhs_str = if is_complex_binop && matches!(rhs.ty, IrType::F64) {
                gen_complex_promote(cg, rhs)
            } else {
                gen_expr(cg, rhs)
            };
            format!("{} {} {}", lhs_str, o, rhs_str)
        }
        ExprKind::UnOp { op, operand } => {
            let o = match op {
                UnOpKind::Neg => "-",
                UnOpKind::Not => "not ",
                // Python 对象本为引用：Ref/MutRef 透传；Deref 透传（无指针）
                UnOpKind::Ref | UnOpKind::MutRef | UnOpKind::Deref => "",
            };
            let inner = gen_expr(cg, operand);
            if o.is_empty() {
                inner
            } else {
                format!("{}{}", o, inner)
            }
        }
        ExprKind::IfExpr { cond, then, els } => {
            // 三元表达式：分支若为 raise 产物（panic! 深嵌套降级）→ 剥离 raise
            // 前缀，保留 LZError(...) 值形态（表达式合法性优先）
            let t = gen_expr(cg, then);
            let e = gen_expr(cg, els);
            let t = t.strip_prefix("raise ").unwrap_or(&t).to_string();
            let e = e.strip_prefix("raise ").unwrap_or(&e).to_string();
            format!(
                "({} if {} else {})",
                t,
                gen_expr(cg, cond),
                e
            )
        }
        ExprKind::StructCtor { name, fields } => {
            // ── 内建特殊构造形态（对齐 Rust 端 StructCtor 分支）──
            match name.as_str() {
                "_KwArg" => {
                    // 关键字参数包装 → 提取 value（Call 侧已内联，此为直接出现形态）
                    return fields
                        .iter()
                        .find(|(n, _)| n == "value")
                        .map(|(_, v)| gen_expr(cg, v))
                        .unwrap_or_else(|| "None".into());
                }
                "_Walrus" => {
                    // := walrus → Python 海象运算符 (bind := val)
                    let bind = fields
                        .iter()
                        .find(|(n, _)| n == "_bind")
                        .map(|(_, v)| gen_expr(cg, v))
                        .unwrap_or_default();
                    let val = fields
                        .iter()
                        .find(|(n, _)| n == "_val")
                        .map(|(_, v)| gen_expr(cg, v))
                        .unwrap_or_default();
                    return format!("({} := {})", bind, val);
                }
                "None" => return "None".to_string(),
                "Dict" | "HashMap" => {
                    if fields.is_empty() {
                        return "{}".to_string();
                    }
                    // 带条目：_kN/_vN 交替字段 → {k: v, ...}
                    let mut pairs = vec![];
                    let mut i = 0;
                    while i < fields.len() {
                        let key = fields.iter().find(|(n, _)| n == &format!("_k{}", i));
                        let val = fields.iter().find(|(n, _)| n == &format!("_v{}", i));
                        if let (Some((_, k)), Some((_, v))) = (key, val) {
                            pairs.push(format!("{}: {}", gen_expr(cg, k), gen_expr(cg, v)));
                        }
                        i += 1;
                    }
                    return format!("{{{}}}", pairs.join(", "));
                }
                "Range" => {
                    let start = fields.iter().find(|(n, _)| n == "start");
                    let end = fields.iter().find(|(n, _)| n == "end");
                    let inclusive = fields.iter().any(|(n, v)| {
                        n == "inclusive" && matches!(&v.kind, ExprKind::Lit(LitKind::Bool(true)))
                    });
                    return match (start, end) {
                        (Some((_, s)), Some((_, e))) if inclusive => {
                            format!("range({}, {} + 1)", gen_expr(cg, s), gen_expr(cg, e))
                        }
                        (Some((_, s)), Some((_, e))) => {
                            format!("range({}, {})", gen_expr(cg, s), gen_expr(cg, e))
                        }
                        // 单边区间（无界）：Python 无原生无限 range，用 i64 上界近似
                        (Some((_, s)), None) => {
                            format!("range({}, 9223372036854775807)", gen_expr(cg, s))
                        }
                        (None, Some((_, e))) => format!("range(0, {})", gen_expr(cg, e)),
                        _ => "range(0, 0)".to_string(),
                    };
                }
                "List" | "Vec" | "list" => {
                    if fields.is_empty() {
                        return "[]".to_string();
                    }
                    let items: Vec<String> =
                        fields.iter().map(|(_, v)| gen_expr(cg, v)).collect();
                    return format!("[{}]", items.join(", "));
                }
                "Set" | "HashSet" | "set" => {
                    if fields.is_empty() {
                        return "set()".to_string();
                    }
                    let items: Vec<String> =
                        fields.iter().map(|(_, v)| gen_expr(cg, v)).collect();
                    return format!("{{{}}}", items.join(", "));
                }
                _ => {}
            }
            // has_new struct 位置构造 → Name__new__(...)（对齐 Rust 端分派）
            if cg.has_new_structs.contains(name) {
                let values: Vec<String> =
                    fields.iter().map(|(_, v)| gen_expr(cg, v)).collect();
                return format!("{}__new__({})", name, values.join(", "));
            }
            let f: Vec<String> = fields
                .iter()
                .map(|(n, e)| format!("{} = {}", n, gen_expr(cg, e)))
                .collect();
            format!("{}({})", name, f.join(", "))
        }
        ExprKind::EnumCtor {
            enum_name,
            variant,
            args,
        } => {
            let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
            // ── Option/Result 构造 → None/异常模型（PLAN.md §1.6）──
            if is_option_variant(enum_name, variant) {
                match variant.as_str() {
                    "None" => "None".to_string(),
                    _ => a.first().cloned().unwrap_or_else(|| "None".into()),
                }
            } else if is_result_variant(enum_name, variant) {
                match variant.as_str() {
                    "Ok" => a.first().cloned().unwrap_or_else(|| "None".into()),
                    // Err(e) 表达式上下文 → LZError 包装（可 raise / 可携带）
                    _ => format!("LZError({})", a.join(", ")),
                }
            } else {
                // 自定义 enum：变体是顶层子类，直接按位置构造
                format!("{}({})", mangle_ident(variant), a.join(", "))
            }
        }
        ExprKind::Lambda {
            params,
            body,
            is_move,
            ret_ty: _,
        } => {
            // Python lambda 无类型标注；默认值支持；is_move 无意义（引用语义）
            let _ = is_move;
            let p: Vec<String> = params
                .iter()
                .map(|p| match &p.default {
                    Some(d) => format!("{}={}", p.name, gen_expr(cg, d)),
                    None => p.name.clone(),
                })
                .collect();
            // 语句性 Lambda（体为赋值）：引用预生成的 def（闭包写回语义）
            if let ExprKind::AssignExpr { target, value } = &body.kind {
                let key = format!(
                    "{}::{}",
                    params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(","),
                    format!("{}", format!("{:?}", body.kind))
                );
                if let Some(name) = cg.lambda_map.get(&key) {
                    return name.clone();
                }
                // fallback（未预扫描覆盖）：walrus 化保语法（弱化写回语义）
                let t = match &target.kind {
                    ExprKind::Var(n) => n.clone(),
                    _ => String::new(),
                };
                let v = gen_expr(cg, value);
                if !t.is_empty() {
                    return format!("lambda {}: ({} := {})", p.join(", "), t, v);
                }
                let _ = target;
            }
            // 块体闭包：引用预扫描 hoist 出来的 def（`lambda` 装不下语句，直接生成会丢前缀语句）
            if let ExprKind::BlockExpr { block } = &body.kind {
                if lambda_block_needs_def(block) {
                    let key = format!(
                        "{}::{:?}",
                        params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(","),
                        body.kind
                    );
                    if let Some(name) = cg.lambda_map.get(&key) {
                        return name.clone();
                    }
                }
            }
            let b = gen_expr(cg, body);
            format!("lambda {}: {}", p.join(", "), b)
        }
        ExprKind::IndexSet { base, key, value } => {
            format!(
                "{}[{}] = {}",
                gen_expr(cg, base),
                gen_expr(cg, key),
                gen_expr(cg, value)
            )
        }
        ExprKind::GenExpr { yield_of } => {
            // 生成器表达式 `*: expr` → yield 形式（外层函数标记 is_iterator）
            format!("(yield {})", gen_expr(cg, yield_of))
        }
        ExprKind::AssignExpr { target, value } => {
            format!("{} = {}", gen_expr(cg, target), gen_expr(cg, value))
        }
        ExprKind::GenBuild { callee, block } => {
            // 生成器构建块 func *: { yield ... }
            // 有 callee：收集 yield 参数包（闭包收集器 __bb），逐包调用 callee → list
            // 无 callee：仅收集参数包返回 list（迭代器语义）
            let elem_ty = first_yield_type(block).unwrap_or(IrType::Unit);
            let _elem_cy = cg.map_type(&elem_ty, TypeCtx::Local);
            // 生成 block 体（收集 yield 参数到 __bb）
            let mut child = CythonCodeGen::new();
            child.indent = cg.indent + 2;
            child.known_types = cg.known_types.clone();
            child.cdef_classes = cg.cdef_classes.clone();
            child.enum_variants = cg.enum_variants.clone();
            child.variant_fields = cg.variant_fields.clone();
            child.has_new_structs = cg.has_new_structs.clone();
            child.lambda_map = cg.lambda_map.clone();
            child.checker_wrappers = cg.checker_wrappers.clone();
                child.fn_params = cg.fn_params.clone();
                child.current_class_name = cg.current_class_name.clone();
                child.current_fn_ret_ty = cg.current_fn_ret_ty.clone();
                child.module_var_names = cg.module_var_names.clone();
                child.overload_sigs = cg.overload_sigs.clone();
                child.emitted_fns = cg.emitted_fns.clone();
                child.merged_modules = cg.merged_modules.clone();
                child.impl_methods = cg.impl_methods.clone();
                child.self_methods = cg.self_methods.clone();
                child.enum_names = cg.enum_names.clone();
                child.in_gen_build = true;
            // 收集 yield 语句：转换为 __bb.append(...)
            for stmt in &block.stmts {
                match stmt {
                    Stmt::Yield { value } => {
                        let v = gen_expr(&child, value);
                        child.writeln(&format!("__bb.append({})", v));
                    }
                    Stmt::YieldFrom { iter } => {
                        let v = gen_expr(&child, iter);
                        child.writeln(&format!("__bb.extend({})", v));
                    }
                    _ => {
                        // 其他语句正常生成
                        gen_stmt(&mut child, stmt, false);
                    }
                }
            }
            let body_s = child.buf;
            // build block 出现在**表达式位置**（`xs = add *:`），而 Python 没有
            // "语句表达式"。改前发的是
            //     {
            //         __bb: list = []
            //         (lambda: None)(
            //             __bb.append((1, 2))
            //         )
            //         [...] for __p in __bb]}
            // ——`{…}` 是集合/字典字面量，里面不能放语句，整段直接 cython 失败
            // （`Expected '}', found '='`）。这里改成等价且合法的写法：
            // 用带默认形参的 IIFE 把收集器 `__bb` 局部化，body 内每条都是表达式
            // （append/extend 都返回 None，元组里逐个求值即可完成副作用），
            // 末尾取 `[-1]` 得到列表；闭包语义保留（可引用外层局部变量）。
            let has_non_expression_stmt = block.stmts.iter().any(|s| {
                !matches!(
                    s,
                    Stmt::Yield { .. } | Stmt::YieldFrom { .. } | Stmt::ExprStmt { .. }
                )
            });
            let body_items: Vec<String> = body_s
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            let body_joined = if has_non_expression_stmt || body_items.is_empty() {
                // 非表达式语句（如 block 内 `let`）在本后端没有等价 lowering。
                // 发一个**刻意未定义**的标记名：cython 会当场报 undeclared name，
                // 失败面留在编译期，而不是产出语法合法但语义错的代码。
                "__lz_genbuild_unsupported_block_stmt".to_string()
            } else {
                body_items.join(", ")
            };
            let tail = match callee {
                Some(callee_expr) => {
                    let callee_s = gen_expr(cg, callee_expr);
                    match &elem_ty {
                        IrType::Tuple(elems) => {
                            let binds: Vec<String> =
                                (0..elems.len()).map(|i| format!("__a{}", i)).collect();
                            // 循环目标必须是**与参数个数一致的解构元组**：改前写
                            // `for __p in __bb` 却用 `__a0, __a1` 求值 ⇒ 名字从未绑定，
                            // 运行期 NameError（且 cython 侧报 undeclared name）。
                            let (pat, call) = if binds.len() == 1 {
                                (
                                    format!("({},)", binds[0]),
                                    format!("{}({})", callee_s, binds[0]),
                                )
                            } else {
                                (
                                    format!("({})", binds.join(", ")),
                                    format!("{}({})", callee_s, binds.join(", ")),
                                )
                            };
                            format!("[{} for {} in __bb]", call, pat)
                        }
                        IrType::Unit => format!("[{}() for _ in __bb]", callee_s),
                        _ => format!("[{}(__p) for __p in __bb]", callee_s),
                    }
                }
                None => "__bb".to_string(),
            };
            format!("(lambda __bb: (None, {}, {})[-1])([])", body_joined, tail)
        }
        ExprKind::Cast { expr, target } => {
            let inner = gen_expr(cg, expr);
            safe_cast(cg, target, inner)
        }
        ExprKind::MagicCall { kind, args } => {
            let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
            let first = a.first().cloned().unwrap_or_default();
            let second = a.get(1).cloned().unwrap_or_default();
            match kind {
                MagicKind::GetItem => format!("{}[{}]", first, second),
                MagicKind::SetItem => format!("{}[{}] = {}", first, second, a.get(2).cloned().unwrap_or_default()),
                MagicKind::Display => format!("str({})", first),
                MagicKind::Len => format!("len({})", first),
                MagicKind::Eq => format!("{} == {}", first, second),
                MagicKind::Cmp => format!("{} < {}", first, second),
                MagicKind::Iter => format!("iter({})", first),
                MagicKind::Next => format!("next({})", first),
                MagicKind::Call => format!("{}({})", first, a.iter().skip(1).cloned().collect::<Vec<_>>().join(", ")),
                MagicKind::Add => format!("{} + {}", first, second),
                MagicKind::Sub => format!("{} - {}", first, second),
                MagicKind::Mul => format!("{} * {}", first, second),
                MagicKind::Neg => format!("-{}", first),
                MagicKind::Not_ => format!("not {}", first),
                MagicKind::Drop => format!("# __drop__({})", first),
                MagicKind::Rev => format!("reversed({})", first),
                MagicKind::IntoIter => format!("iter({})", first),
                MagicKind::SizeHint => "# __size_hint__()".to_string(),
                MagicKind::IterStrategy => "# __iter_strategy__()".to_string(),
                MagicKind::UnpackBuildCall => {
                    // ~: 构建块元组解包：args[0]=闭包立即调用表达式, args[1]=元素索引
                    // Python 元组用下标访问（packed 为复杂表达式，括号包裹）；
                    // 索引从 IR 直接提取裸整数（对齐 Rust 端，避免类型后缀）
                    let idx = match args.get(1).map(|a| &a.kind) {
                        Some(ExprKind::Lit(LitKind::Int(n))) => n.to_string(),
                        _ => a.get(1).cloned().unwrap_or_else(|| "0".into()),
                    };
                    format!("({})[{}]", first, idx)
                }
            }
        }
        ExprKind::BlockExpr { block } => {
            // 深嵌套位置（无法语句提升）：降级为尾表达式。
            // 不生成注释——后续拼接的 `()`/`[0]` 会被行内注释吞掉
            let tail = block_tail_expr(cg, block).unwrap_or_else(|| "None".into());
            format!("({})", tail)
        }
        ExprKind::TupleLit(exprs) | ExprKind::Tuple(exprs) => {
            let e: Vec<String> = exprs.iter().map(|e| gen_expr(cg, e)).collect();
            if e.is_empty() {
                "()".to_string()
            } else if e.len() == 1 {
                format!("({},)", e[0])
            } else {
                format!("({})", e.join(", "))
            }
        }
        ExprKind::ListLit(exprs) | ExprKind::List(exprs) => {
            // Spread 元素 `...a` → `*a`（Python 原生列表解包）
            let e: Vec<String> = exprs.iter().map(|e| gen_expr(cg, e)).collect();
            format!("[{}]", e.join(", "))
        }
        ExprKind::Dict(entries) => {
            let e: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", gen_expr(cg, k), gen_expr(cg, v)))
                .collect();
            format!("{{{}}}", e.join(", "))
        }
        ExprKind::Range {
            start,
            end,
            inclusive,
        } => {
            let s = match start {
                Some(s) => gen_expr(cg, s),
                None => "0".into(),
            };
            let e = gen_expr(cg, end);
            if *inclusive {
                format!("range({}, {} + 1)", s, e)
            } else {
                format!("range({}, {})", s, e)
            }
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            let r = gen_expr(cg, receiver);
            let c = gen_expr(cg, callee);
            let a: Vec<String> = args.iter().map(|a| gen_expr(cg, a)).collect();
            format!("{}({}, {})", c, a.join(", "), r)
        }
        ExprKind::Paren(expr) => format!("({})", gen_expr(cg, expr)),
        ExprKind::ImplicitConvert { source, target_ty } => {
            let inner = gen_expr(cg, source);
            safe_cast(cg, target_ty, inner)
        }
    }
}

// ══════════════════════════════════════════════════════════════
// 模式生成
// ══════════════════════════════════════════════════════════════

/// 拼接模式条件（过滤恒真项，避免 `x and True` 冗余）
fn join_conds(conds: Vec<String>) -> String {
    let real: Vec<String> = conds.into_iter().filter(|c| c != "True").collect();
    if real.is_empty() {
        "True".into()
    } else {
        real.join(" and ")
    }
}

/// 生成模式匹配条件 + 绑定变量列表
/// 返回 (条件表达式, [(绑定名, 值表达式)])
/// 注意：条件用 Python `and`（非 &&）；绑定值来自 scrutinee 的属性/下标访问
fn gen_pattern(
    cg: &CythonCodeGen,
    pat: &Pattern,
    scrutinee: &str,
) -> (String, Vec<(String, String)>) {
    match pat {
        Pattern::Wildcard => ("True".into(), vec![]),
        Pattern::Ident(name) => {
            // `None` 模式 → None 检查（None 不是可绑定标识符）
            if name == "None" {
                return (format!("{} is None", scrutinee), vec![]);
            }
            (
                "True".into(),
                vec![(name.clone(), scrutinee.to_string())],
            )
        }
        Pattern::RefMutIdent(name) => (
            "True".into(),
            vec![(name.clone(), scrutinee.to_string())],
        ),
        Pattern::Lit(lit) => {
            let lit_str = match lit {
                LitKind::Int(n) => n.to_string(),
                LitKind::Int128(n) => n.to_string(),
                LitKind::BigInt(s) => s.clone(),
                LitKind::Complex(re, im) => format!("({} + {}j)", re, im),
                LitKind::F64(f) => f.to_string(),
                LitKind::Str(s) => format!("\"{}\"", escape_str_literal(s)),
                LitKind::FStr(s) => format!("f\"{}\"", fs_interp(&escape_str_literal(s))),
                LitKind::Bool(b) => if *b { "True" } else { "False" }.into(),
                LitKind::None_ | LitKind::Unit => "None".into(),
            };
            (format!("{} == {}", scrutinee, lit_str), vec![])
        }
        Pattern::Tuple(elems) => {
            let mut conds = vec![format!(
                "isinstance({}, tuple) and len({}) == {}",
                scrutinee, scrutinee, elems.len()
            )];
            let mut bindings = vec![];
            for (i, elem) in elems.iter().enumerate() {
                let sub_scrut = format!("{}[{}]", scrutinee, i);
                let (c, b) = gen_pattern(cg, elem, &sub_scrut);
                conds.push(c);
                bindings.extend(b);
            }
            (join_conds(conds), bindings)
        }
        Pattern::List(elems) => {
            // 含 Rest 的列表模式：len 下界检查 + rest 切片绑定
            let has_rest = elems
                .iter()
                .any(|e| matches!(e, Pattern::Rest(_)));
            let min_len = if has_rest { elems.len() - 1 } else { elems.len() };
            let len_cond = if has_rest {
                format!("len({}) >= {}", scrutinee, min_len)
            } else {
                format!("len({}) == {}", scrutinee, min_len)
            };
            let mut conds = vec![format!("isinstance({}, list) and {}", scrutinee, len_cond)];
            let mut bindings = vec![];
            for (i, elem) in elems.iter().enumerate() {
                match elem {
                    Pattern::Rest(name) => {
                        // ..rest → scrutinee[i:]
                        let rest_str = format!("{}[{}:]", scrutinee, i);
                        if let Some(n) = name {
                            bindings.push((n.clone(), rest_str));
                        }
                        // rest 之后不再有普通元素
                        break;
                    }
                    _ => {
                        let sub_scrut = format!("{}[{}]", scrutinee, i);
                        let (c, b) = gen_pattern(cg, elem, &sub_scrut);
                        conds.push(c);
                        bindings.extend(b);
                    }
                }
            }
            (join_conds(conds), bindings)
        }
        Pattern::Dict(entries) => {
            let mut conds = vec![format!("isinstance({}, dict)", scrutinee)];
            let mut bindings = vec![];
            for (key, val) in entries {
                let sub_scrut = format!("{}[\"{}\"]", scrutinee, key);
                // 键必须存在（先用 in 检查，避免 KeyError）
                conds.push(format!("\"{}\" in {}", key, scrutinee));
                let (c, b) = gen_pattern(cg, val, &sub_scrut);
                conds.push(c);
                bindings.extend(b);
            }
            (join_conds(conds), bindings)
        }
        Pattern::Rest(name) => match name {
            Some(n) => (
                "True".into(),
                vec![(n.clone(), scrutinee.to_string())],
            ),
            None => ("True".into(), vec![]),
        },
        Pattern::Range {
            start,
            end,
            inclusive,
        } => {
            let end_cond = if *inclusive {
                format!("{} <= {} <= {}", start, scrutinee, end)
            } else {
                format!("{} <= {} < {}", start, scrutinee, end)
            };
            (end_cond, vec![])
        }
        Pattern::Struct { name, fields } => {
            let mut conds = vec![format!("isinstance({}, {})", scrutinee, name)];
            let mut bindings = vec![];
            for (fname, fpat) in fields {
                let sub_scrut = format!("{}.{}", scrutinee, fname);
                let (c, b) = gen_pattern(cg, fpat, &sub_scrut);
                conds.push(c);
                bindings.extend(b);
            }
            (join_conds(conds), bindings)
        }
        Pattern::Enum {
            enum_name,
            variant,
            args,
        } => {
            // ── Option/Result 模式 → None/异常模型（PLAN.md §1.6）──
            if is_option_variant(enum_name, variant) {
                return match variant.as_str() {
                    "None" => (format!("{} is None", scrutinee), vec![]),
                    _ => {
                        // Some(p) → scrutinee is not None + 绑定
                        let mut bindings = vec![];
                        let mut conds = vec![format!("{} is not None", scrutinee)];
                        if let Some(arg) = args.first() {
                            let (c, b) = gen_pattern(cg, arg, scrutinee);
                            if c != "True" {
                                conds.push(c);
                            }
                            bindings.extend(b);
                        }
                        (join_conds(conds), bindings)
                    }
                };
            }
            if is_result_variant(enum_name, variant) {
                return match variant.as_str() {
                    "Ok" => {
                        let mut bindings = vec![];
                        let mut conds =
                            vec![format!("not isinstance({}, BaseException)", scrutinee)];
                        if let Some(arg) = args.first() {
                            let (c, b) = gen_pattern(cg, arg, scrutinee);
                            if c != "True" {
                                conds.push(c);
                            }
                            bindings.extend(b);
                        }
                        (join_conds(conds), bindings)
                    }
                    _ => {
                        let mut bindings = vec![];
                        let mut conds =
                            vec![format!("isinstance({}, BaseException)", scrutinee)];
                        if let Some(arg) = args.first() {
                            let (c, b) = gen_pattern(cg, arg, scrutinee);
                            if c != "True" {
                                conds.push(c);
                            }
                            bindings.extend(b);
                        }
                        (join_conds(conds), bindings)
                    }
                };
            }
            // ── 自定义 enum：变体是顶层子类，字段按 prescan 映射访问 ──
            let key = format!("{}.{}", enum_name, variant);
            // 无数据变体已单例化（`Red = Red()`），用 identity 比较（isinstance 会炸：
            // 单例实例不是 type）
            // 无数据变体已单例化（`Red = Red()`），用 identity 比较（isinstance 会炸：
            // 单例实例不是 type）。判定优先用 prescan 的 variant_fields；跨模块 import
            // 合并时 variant_fields 可能缺失，退化为按模式参数个数判定：无数据变体在
            // match 中写成 `Enum.Variant` 不带参数，args 必为空（BUG-B11）。
            let unit_variant = cg
                .variant_fields
                .get(&key)
                .map(|f| f.is_empty())
                .unwrap_or(args.is_empty());
            if unit_variant {
                return (format!("{} is {}", scrutinee, mangle_ident(variant)), vec![]);
            }
            let mut conds = vec![format!("isinstance({}, {})", scrutinee, mangle_ident(variant))];
            let mut bindings = vec![];
            let fnames = cg.variant_fields.get(&key);
            for (i, arg) in args.iter().enumerate() {
                let sub_scrut = match fnames {
                    Some(fs) if i < fs.len() => format!("{}.{}", scrutinee, fs[i]),
                    _ => format!("{}.f{}", scrutinee, i),
                };
                let (c, b) = gen_pattern(cg, arg, &sub_scrut);
                conds.push(c);
                bindings.extend(b);
            }
            (join_conds(conds), bindings)
        }
    }
}

// ── 生成后处理（下沉自 lzcyc/CY/src/main.rs） ───────────────────────────
// 各项均以生成代码的稳定形态为锚点，匹配不上则原样保留：
// 1. enum 无数据变体注入 `_variant = <序号>`（match 解构依赖）
// 2. Box/Rc/Arc 补 `__getitem__`/`__setitem__`（`x[0]` 取/存内部值）
// 3. Option 垫片：`None_()` 返回带方法的 `_LZNONE` 单例；行级 `x = None` → `x = _LZNONE`
// 4. 构建块下标：`(lambda : (...)))()(N)` → `...))()[N]`
// 5. 列表推导 filter 谓词补调用：`for __cv in ... if (lambda ...)` → `...(__cv)`
// 6. 已合并 import 的限定前缀剥离：`lz_std.X` → `X`，并删除对应 `import X` 行
// 7. checker 派发：`fn[checker](...)` 调用改写为包装器；注入 `__Params` 垫片

pub fn postprocess_pyx(
    code: &str,
    enum_variants: &[(String, usize)],
    merged_modules: &[String],
) -> String {
    let mut lines: Vec<String> = code.lines().map(String::from).collect();

    // 1) _variant 注入
    for i in 0..lines.len() {
        let t = lines[i].trim_start().to_string();
        if let Some(rest) = t.strip_prefix("class ") {
            if let Some(paren) = rest.find('(') {
                let cname = rest[..paren].trim();
                if let Some((_, idx)) = enum_variants.iter().find(|(n, _)| n == cname) {
                    if i + 1 < lines.len() && lines[i + 1].trim() == "pass" {
                        let indent = lines[i + 1].len() - lines[i + 1].trim_start().len();
                        lines[i + 1] =
                            format!("{}{}", " ".repeat(indent), format!("_variant = {idx}"));
                    }
                }
            }
        }
    }
    let mut code = lines.join("\n");

    // 2) Box/Rc/Arc 下标增强
    for cls in ["Box", "Rc", "Arc"] {
        let anchor = format!(
            "class {cls}:\n    def __init__(self, v=None): self._v = v\n    @staticmethod\n    def new(v=None): return {cls}(v)\n    def __getattr__(self, n): return getattr(self._v, n)"
        );
        let enhanced = format!(
            "{anchor}\n    def __getitem__(self, i): return self._v\n    def __setitem__(self, i, v): self._v = v"
        );
        if code.contains(&anchor) {
            code = code.replace(&anchor, &enhanced);
        }
    }

    // 3a) Option 垫片：None_ 裸 None → 单例方法对象
    let opt_anchor = "class Option:\n    @staticmethod\n    def Some(v): return v\n    None_ = None";
    let opt_shim = "class Option:\n    @staticmethod\n    def Some(v): return v\n    @staticmethod\n    def None_(): return _LZNONE\n\nclass _LzNoneCls:\n    def is_none(self): return True\n    def is_some(self): return False\n    def unwrap(self): raise LZError('unwrap None')\n    def expect(self, m): raise LZError(m)\n    def __repr__(self): return 'None'\n    def __eq__(self, o): return o is None or isinstance(o, _LzNoneCls)\n    def __bool__(self): return False\n\n_LZNONE = _LzNoneCls()";
    if code.contains(opt_anchor) {
        code = code.replace(opt_anchor, opt_shim);
    }

    // 3b) 行级裸 None 字面量赋值 → _LZNONE
    {
        let mut out = Vec::with_capacity(lines.len());
        for l in code.lines() {
            let t = l.trim_start();
            let is_bare_none = !t.starts_with('#')
                && !t.starts_with("def ")
                && !t.contains('(')
                && t.ends_with("= None")
                && {
                    let head = t[..t.len() - 6].trim_end();
                    head.ends_with(|c: char| c.is_alphanumeric() || c == '_')
                        && head
                            .split_whitespace()
                            .last()
                            .map(|w| w.chars().all(|c| c.is_alphanumeric() || c == '_'))
                            .unwrap_or(false)
                };
            if is_bare_none {
                let indent = l.len() - t.len();
                let name = t[..t.len() - 6].trim_end();
                out.push(format!("{}{} = _LZNONE", " ".repeat(indent), name));
            } else {
                out.push(l.to_string());
            }
        }
        code = out.join("\n");
    }

    // 4) 构建块下标修复：`))()(N)` → `))()[N]`
    loop {
        let Some(p) = code.find("))()(") else { break };
        let after = &code[p + 5..];
        let Some(close) = after.find(')') else { break };
        let idx = &after[..close];
        if idx.is_empty() || !idx.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        code = format!("{}))()[{}]{}", &code[..p], idx, &after[close + 1..]);
    }

    // 5) 列表推导 filter 谓词补调用
    {
        let mut out = Vec::with_capacity(code.lines().count());
        for l in code.lines() {
            if l.contains("for __cv in") && l.contains("if (lambda") {
                if let Some(lf) = l.find("if (lambda") {
                    let bytes: Vec<char> = l.chars().collect();
                    let start = lf + 3;
                    if let Some(&c0) = bytes.get(start) {
                        if c0 == '(' {
                            let mut depth = 0i32;
                            let mut end = None;
                            for j in start..bytes.len() {
                                match bytes[j] {
                                    '(' => depth += 1,
                                    ')' => {
                                        depth -= 1;
                                        if depth == 0 {
                                            end = Some(j);
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if let Some(e) = end {
                                let head: String = bytes[..=e].iter().collect();
                                let tail: String = bytes[e + 1..].iter().collect();
                                out.push(format!("{head}(__cv){tail}"));
                                continue;
                            }
                        }
                    }
                }
            }
            out.push(l.to_string());
        }
        code = out.join("\n");
    }

    // 6) 已合并 import：删除 `import X` 行 + 剥离 `X.` 限定前缀
    for m in merged_modules {
        code = code.replace(&format!("{m}."), "");
        let mut out = Vec::with_capacity(code.lines().count());
        let import_stmt = format!("import {m}");
        for l in code.lines() {
            if l.trim() == import_stmt {
                continue;
            }
            out.push(l.to_string());
        }
        code = out.join("\n");
    }

    // 7) checker 派发
    {
        use std::collections::BTreeSet;
        let mut wrappers: Vec<String> = Vec::new();
        let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
        let mut out = Vec::with_capacity(code.lines().count());
        for l in code.lines() {
            let mut fixed = l.to_string();
            if !fixed.trim_start().starts_with('#') && fixed.contains('[') {
                let chars: Vec<char> = fixed.chars().collect();
                let mut i = 0;
                let mut rebuilt = String::new();
                while i < chars.len() {
                    if chars[i] == '[' {
                        let head_end = i;
                        let mut fs = i;
                        while fs > 0 {
                            let c = chars[fs - 1];
                            if c.is_alphanumeric() || c == '_' {
                                fs -= 1;
                            } else {
                                break;
                            }
                        }
                        let mut j = i + 1;
                        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                            j += 1;
                        }
                        if j < chars.len()
                            && chars[j] == ']'
                            && j + 1 < chars.len()
                            && chars[j + 1] == '('
                            && head_end > fs
                        {
                            let fname: String = chars[fs..head_end].iter().collect();
                            let checker: String = chars[i + 1..j].iter().collect();
                            let valid = |s: &str| {
                                !s.is_empty()
                                    && s.chars().next().map(|c| c.is_alphabetic() || c == '_')
                                        .unwrap_or(false)
                            };
                            if valid(&fname)
                                && valid(&checker)
                                && code.contains(&format!("def {checker}("))
                                && code.contains(&format!("def {fname}("))
                            {
                                let wrapper = format!("__lz_checked_{fname}_{checker}");
                                let fname_len = head_end - fs;
                                rebuilt.truncate(rebuilt.len() - fname_len);
                                rebuilt.push_str(&wrapper);
                                pairs.insert((fname.clone(), checker.clone()));
                                i = j + 1;
                                continue;
                            }
                        }
                    }
                    rebuilt.push(chars[i]);
                    i += 1;
                }
                fixed = rebuilt;
            }
            out.push(fixed);
        }
        code = out.join("\n");

        for (fname, checker) in &pairs {
            let params = extract_def_params(&code, fname);
            let zip_list = if params.is_empty() {
                "()".to_string()
            } else {
                let names = params
                    .iter()
                    .map(|p| format!("'{p}'"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({names},)")
            };
            wrappers.push(format!(
                "def __lz_checked_{fname}_{checker}(*__args, **__kwargs):\n    __params = __Params(kwargs=dict(list(zip({zip_list}, __args)), **__kwargs))\n    {checker}(__params)\n    return {fname}(*__args, **__kwargs)"
            ));
        }
        if !wrappers.is_empty() {
            let shim = PARAMS_SHIM;
            let block = format!("{}\n{}", shim, wrappers.join("\n\n"));
            let anchor = "\ndef main(";
            match code.find(anchor) {
                Some(p) => {
                    code = format!("{}\n\n{}\n{}", &code[..p], block, &code[p + 1..]);
                }
                None => code = format!("{code}\n\n{block}\n"),
            }
        }
    }

    code
}

/// 从生成代码中提取 `def fname(...)` 的形参名列表
pub fn extract_def_params(code: &str, fname: &str) -> Vec<String> {
    let mut out = Vec::new();
    for l in code.lines() {
        let t = l.trim_start();
        let head = format!("def {fname}(");
        if t.starts_with(&head) {
            if let Some(open) = t.find('(') {
                if let Some(close) = t.rfind(')') {
                    if close > open {
                        for seg in t[open + 1..close].split(',') {
                            let seg_t = seg.trim();
                            if seg_t.is_empty() || seg_t.starts_with('*') {
                                continue;
                            }
                            let head_t = seg_t.split('=').next().unwrap_or(seg_t).trim();
                            if head_t.contains(':') {
                                continue;
                            }
                            if let Some(last) = head_t.split_whitespace().last() {
                                if last == "self" || last == "ps" {
                                    continue;
                                }
                                out.push(last.to_string());
                            }
                        }
                    }
                }
            }
            break;
        }
    }
    out
}
