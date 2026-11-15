# lzcyc 自举规划

## 目标

用 LZ 语言写编译器前端（Lexer → Parser → Type Checker），
编译为 `.pyd`，使 `lzcyc.exe` 成为仅调用 Python `lz_compiler.pyd` 的薄启动器。

## 架构

```
lzcyc.exe (Rust 薄层 ~200 行)
  │  python -c "import lz_compiler; lz_compiler.transpile(...)"
  ▼
lz_compiler/ (全部用 LZ 写 → 编译为 .pyd)
├── __init__.pyx    入口
├── lexer.lz        词法分析器: str → List[Token]
├── parser.lz       语法分析器: List[Token] → AST
├── ast.lz          AST 类型定义
├── typer.lz        类型推断 (简化)
├── codegen.lz      Cython 代码生成 (可选的后续步骤)
└── lz_compile.lz   主控入口
```

## 阶段

| 阶段 | 内容 | 预估 LZ 行数 |
|:----:|------|:------------:|
| **P1** | Lexer: Token 类型 + 字符流 → Token 流 | ~300 |
| **P2** | Parser: 递归下降, 支持 def/let/if/for/while/match | ~500 |
| **P3** | AST 类型 + Typer 类型推断 | ~400 |
| **P4** | 整合: lz_compile.lz 串联全过程 | ~200 |
| **P5** | 薄化 Rust: lzcyc.exe 只做参数解析 | ~50 |
| **P6** | 自举验证: 用 LZ 编译器编译自身 | ✅ |

## 关键决策

- Token/AST 用 Python list/dict 模拟（不用自定义类）
- 缩进用空格计数生成 INDENT/DEDENT
- 错误返回 `(result, errors)` 元组
- 逐步替换：先 P1-P3 输出 AST dict → 当前 Rust codegen 仍可用
