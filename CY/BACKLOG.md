# lzcyc 挂账清单（BACKLOG）

> 约束：`src/`（主编译器 lzc）不可修改，以下事项待主编译器融入子编译器时统一处理。
> 更新：2026-09-17

## 一、验证基线（今日实测）

| 验证 | 结果 | 说明 |
|---|---|---|
| transpile 全量 | **51/53** | CY/TESTS 53 样例 |
| run 全量（纯 Python 降级） | **45/53** | 含 cdef 剥离降级 |

产物：`CY/output/tests/*.pyx`；降级运行副本：`CY/output/pyrun/*.py`（git 已忽略）

## 二、挂账：主编译器 Cython 后端运行期语义缺口（6 个 run 失败）

lib `codegen_cython.rs` 生成形态在运行期的缺口（cythonize 与纯 Python 降级同样命中）：

| 样例 | 现象 | 缺口 |
|---|---|---|
| box_rc_arc.lz | `'Box' object is not subscriptable` | Box/Rc/Arc 包装类缺 `__getitem__`/`__setitem__`（仅 `__getattr__` 委托） |
| test_string_ops.lz | `'NoneType' object has no attribute 'is_none'` | `None_()` 返回裸 None，Option 实例方法（is_none/unwrap）无着落 |
| test_types.lz | 同上 | 同上 |
| test_struct.lz | `'Red' object has no attribute '_variant'` | enum match 解构依赖 `_variant` 字段，类层次生成时未写入 |
| call_block.lz | `'tuple' object is not callable` | 构建块 `=:` 产物调用形态 |
| checker_call.lz | `'function' object is not subscriptable` | checker 块 `__Params` 模拟形态 |

## 三、挂账：主编译器前端既有行为（2 个，与 lzc 行为一致）

1. `99_bootstrap/import_runtime.lz`：`import lz_std` 路径解析（主编译器加 `--std-dir std` 同样报"路径不存在"）
2. `99_self_test/test_control.lz`：`未绑定变量: pos`（嵌套块内 let 外部引用的作用域检查）

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
