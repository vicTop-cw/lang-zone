# 新测试套件结构

## 目录命名: YYYYMMDD-<描述>
## 每个目录包含: .lz 源文件 + 预期输出

### 测试矩阵

20260726-syntax/
  ├── test_eq_fn.lz      # 等式风格函数
  ├── test_sum_prod.lz   # sum/prod 声明式循环
  ├── test_path_dot.lz   # . 路径访问
  ├── test_enum.lz       # 枚举变体
  ├── test_import.lz     # import 语句
  └── test_closure.lz    # 闭包

20260726-types/
  ├── test_primitive.lz  # int, str, bool, f64
  ├── test_list.lz       # List<T> 操作
  ├── test_struct.lz     # struct 定义+方法
  ├── test_enum_def.lz   # enum 定义+匹配
  ├── test_option.lz     # Option<T>
  └── test_result.lz     # Result<T,E>

20260726-control/
  ├── test_if.lz         # if/elif/else
  ├── test_match.lz      # match 表达式
  ├── test_for.lz        # for 循环
  ├── test_while.lz      # while 循环
  ├── test_return.lz     # return 语句
  └── test_guard.lz       # guard 条件守卫

20260726-semantics/
  ├── test_borrow.lz     # 借用语义测试
  ├── test_owned.lz      # owned 所有权测试
  ├── test_strict.lz     # strict 模式验证
  └── test_unsafe.lz     # @unsafe 豁免

20260726-bridge/
  ├── test_rust_bridge.lz  # Rust 桥接
  ├── test_io.lz          # I/O 操作
  └── test_print.lz       # 打印输出

## 测试执行
lzc test.lz --check  → 生成 .rs + rustc 编译检查
预期: 0 rustc errors, 运行时输出匹配预期
