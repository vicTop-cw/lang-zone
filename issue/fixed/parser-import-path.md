# parser 不支持 `import a.b.c` / `from..import` / `as` 别名

- **Status**: Open → **候选关闭**（2026-07-29 审计：import_demo.lz + import_more.lz 已解析 ✅）
- **Severity**: P1
- **Category**: parser（顶层声明）
- **Parent**: [parser-new-syntax-regression.md](parser-new-syntax-regression.md)
- **Owner**: engineering

## Summary

`DEMO/08_modules/` 已使用 v3.1 的模块导入语法（点号分层路径、`from..import` 选择导入、`as` 别名、`bridge` 导入），解析器当前不支持，报 `Unexpected token at top level: Dot`。

## Evidence

复现（`DEMO/08_modules/import_demo.lz` 头部）：

```lz
import std.io
import std.collections.HashMap
import std.collections.HashMap as Map
from std.io import print, read
from std.collections import List, Dict
```

运行 `lzc DEMO/08_modules/import_demo.lz` → `Parse error: Unexpected token at top level: Dot`

受影响 demo：
- `08_modules/import_demo.lz`
- `08_modules/import_more.lz`

## Impact

模块化是 v3.1 核心特性；导入语法不支持会阻塞所有依赖 `import` 的 demo 与标准库桥接（`import std.*` / `import lz.std.bridge.*`）。

## Recommendation

在顶层声明解析中支持：
1. 点号分层模块路径 `import a.b.c`（`.` 分隔，对齐 SYNTAX v3.1；`::` 已废弃）。
2. `from <mod> import <names>` 选择导入。
3. `as <alias>` 别名。
4. `import lz.std.bridge.rust.<crate>` 等桥接导入（经 `BridgeRegistry`）。

修复后 `08_modules/import_demo.lz`、`import_more.lz` 应转绿。
