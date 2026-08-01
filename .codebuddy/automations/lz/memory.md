# LZ 开发自动化记忆

## 最近运行: 2026-08-01 16:54

### 状态
- IR→rustc 编译通过率: **76/129 (58.9%)**，较上次(74/129, 57.4%)提升 +1.5%
- LZ→IR 编译通过率: 129/129 (100.0%)

### 本次完成的修复 (Task 3: E0425-B P1 自由函数→方法调用映射)

1. **`map` 自由函数**: `map(collection, fn)` → `collection.into_iter().map(fn)`
2. **`filter` 自由函数**: `filter(iterator, fn)` → `iterator.filter(fn)` + 闭包参数添加 `&` 前缀 (filter 需要 `&Item`)
3. **`collect` 自由函数**: `collect(iterator)` → `iterator.collect::<Vec<_>>()`
4. **`sum` 自由函数**: `sum(collection)` → `collection.iter().sum()`
5. **`max`/`min` 自由函数**: `max(c)` → `*(&c).iter().max().unwrap()`
6. **`any`/`all` 自由函数**: `any(c, fn)` → `c.iter().any(fn)`
7. **`sorted`/`reversed` 自由函数**: `sorted(c)` → `{ let mut _tmp = c.clone(); _tmp.sort(); _tmp }`
8. **新增 `strip_lambda_type_with_ref`**: strip 闭包类型标注并添加 `&` 引用模式，用于 filter

### 修复的测试文件
- ✅ `99_spec/pipe_spec.lz` — `map`/`filter`/`collect` 管道链
- ✅ `99_spec/parallel_decorator.lz` — `sum` 函数

### 错误分布变化
- E0425: 19 → **17** files (-2, pipe_spec + parallel_decorator 修复)
- 其他错误码不变

### 关键修改文件
- `src/ir/codegen.rs`: prelude 映射扩展 (sum/map/filter/collect/max/min/any/all/sorted/reversed) + strip_lambda_type_with_ref

### 待处理高优先级问题
- E0425: 17 files (大部分为文档/占位符，含 turbofish `<int,str>` 解析、undefined 变量等)
- E0308: 5 files (closure 类型推断、combo-syntax)
- E0277: 4 files (__Params Box<dyn Any>)
- callable_objects.lz: __call__ 机制
- import_demo.lz: 多文件导入
