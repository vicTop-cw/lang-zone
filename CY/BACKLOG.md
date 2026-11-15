# lzcyc 挂账清单（BACKLOG）

> 约束：`src/`（主编译器 lzc）不可修改，以下事项待主编译器融入子编译器时统一处理。
> 更新：2026-09-17（第二轮：生成后处理落地，5/6 运行期缺口兜底解决）

## 一、验证基线（最新实测）

| 验证 | 结果 | 说明 |
|---|---|---|
| transpile 全量 | **54/54** | CY/TESTS 54 样例全绿（含 lz_std.lz 运行时 fixture） |
| run 全量（纯 Python 降级） | **54/54** | 全绿（test_control 样例作用域笔误已修正，checker 派发已兜底） |
| cargo test 回归 | 3/3 | testsrc/cli.rs 固化基线，偏离即红 |

产物：`CY/output/tests/*.pyx`；降级运行副本：`CY/output/pyrun/*.py`（git 已忽略）

## 二、运行期语义缺口：全部由 lzcyc 生成后处理兜底

lzcyc 新增 `postprocess_pyx`（锚点匹配、匹配不上原样保留），以下缺口在 lzcyc 侧已解决：

| 缺口 | 兜底方式 | 状态 |
|---|---|---|
| Box/Rc/Arc 下标 | prelude 类定义注入 `__getitem__`/`__setitem__` | ✅ 已兜底 |
| Option/None 字面量 | `_LZNONE` 单例垫片（is_none/is_some/unwrap/expect + __eq__ None）+ 行级 `x = None` 替换 | ✅ 已兜底 |
| enum match `_variant` | AST 收集变体序号，向变体类注入 `_variant = N` | ✅ 已兜底 |
| 构建块下标 `()(N)` | `))()(N)` → `))()[N]` | ✅ 已兜底 |
| 列表推导 filter 谓词缺调用 | `if (lambda ...)` 补 `(__cv)` | ✅ 已兜底 |
| checker 派发 `函数[checker]` | `__Params` 垫片（`_LzKwargsMap`：contains/`[k]`）+ 包装器派发（形参名 zip 位置实参进 kwargs，checker 先行检查再透传） | ✅ 已兜底 |

> 融入时：以上兜底逻辑应下沉回 lib codegen（生成正确形态），后处理退位为兼容层。下沉计划见 `docs/融入计划-2026-09-17.md` M1。

## 三、前端既有行为（已闭环）

1. ~~`99_bootstrap/import_runtime.lz`：`import lz_std` 路径解析~~ → 已由 lzcyc import 合并 + lz_std.lz fixture 解决
2. ~~`99_self_test/test_control.lz`：`未绑定变量: pos`~~ → 样例作用域笔误已修正（`let` 块级绑定改为无 let 函数级赋值，对齐文件内 total/even_sum 风格；`let` 与无 let 赋值的作用域双轨制已写入融入计划 M2.1）

## 四、挂账：lzcyc 自身待办

1. ~~宏/模板展开未接入~~ → **已完成（2026-09-17）**：`expand_macro_pipeline` 复用 lib 公开层 `lang_zone::macros::*`（提取→macro import→过滤→宏/模板交替至稳定），宏样例 run 验证通过，全量回归水位不变（transpile 51/53、run 45/53）
2. **compile 的 .pyd 环节**：本机无 C 编译器，仅验证到 cythonize；C 编译器就位后补端到端 .pyd 用例
3. **依赖收敛**：主编译器 lib 存在 3 处无条件 `serde_json` 引用（infer feature 保护不全），CY 被迫携带 default features（serde/serde_json/bincode）；融入时收敛为真正零外部依赖
4. **cython/cythonize 端到端验证受限（环境）**：本机 Python 为 Loomy 内置运行时（`AppData\Local\Loomy\python-runtime\3.13.13`），其 pip 自身损坏（`cannot import name 'CacheControlAdapter'`），无法安装 cython；且该运行时位于项目 Workspace 之外，不宜向其安装第三方包。当前验证以纯 Python 降级运行覆盖；cythonize → .pyd 端到端待有 cython 的独立 Python 环境就位后补验

## 五、降级运行器（strip_cython_syntax）能力边界

已覆盖：`cdef class`→`class`、cpdef/cdef 函数前缀与返回类型、参数 C 类型标注、
`-> ret` 返回标注、字段声明/ctypedef/cimport/from cpython/@cython.* 行注释、
`args, *args` 重名消解。

未覆盖（出现时如实报错，不做语义重写）：Box 索引、Option 实例方法、`_variant`
等运行期语义缺口（属第二节 codegen 缺口，不在剥离层修）。
