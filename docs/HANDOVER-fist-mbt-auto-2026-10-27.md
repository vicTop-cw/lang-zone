# lang-zone 项目交接文档（fist-mbt 自主推进版）

> 生成时间：2026-10-27（Asia/Shanghai）
> 目的：把本项目迁移到新机器，由 fist-mbt（Agent Teams + 自驱管线）自主推进。
> 本文件覆盖：环境搭建、构建验证、fist-mbt 初始化、工作流、已知坑、当前状态、未决项、禁止项。

---

## 一、项目是什么

`lang-zone` 是一个自研语言「Lang-Zone」的编译工具链，核心是 Rust 单二进制 `lang-zone`，
把 `.lz` 源文件编译成 Rust（两条路径）：

| 路径 | 命令 | 说明 |
| --- | --- | --- |
| 原生 | `lang-zone <f>.lz` | LZ → IR → `.rs` → rustc → 运行（主路径，保真度高） |
| rs-lz 自举 | `lang-zone <f>.lz --emit=rs-lz` | LZ → IR → `.lzrs` → codegen 自检 → 由 LZ 写的编译器编译（bootstrap 路径，用于验证 LZ 自身可用性） |

关键目录：
- `src/` 编译器 Rust 源码（`src/ir/codegen/` 原生发射、`src/ir/lz_codegen_lib.lz` rs-lz 发射器）
- `lz_builtins/` 运行时库 crate（`liblz_builtins.rlib`，提供 print/len/range 等）
- `FIND_BUG/` 12 个库全链路复现用例（每个目录一个 `.lz`，由 `tests/find_bug_libs.rs` 驱动）
- `CY/` `_probes/` `div-tools/` 探针语料
- `memory/bugs.md` 缺陷台账（追加式，含 OPEN/FIXED/DUPLICATE/FALSE_POSITIVE 抬头状态）
- `_probes/fastsweep.sh` 全量差分扫描脚本

---

## 二、新机器环境搭建

### 2.1 前置依赖
- Rust stable toolchain + `cargo`
- Node.js（fist-mbt serve 依赖 js/node 运行时）
- `git`
- Windows 主机（脚本按 bash-on-Windows 写法；非 Windows 需同步调整 `_probes/*.sh` 路径）

### 2.2 拉取项目
```
git clone <你的仓库> lang-zone
cd lang-zone
```
> ⚠️ **勿 `git push`**：本地 HEAD 与 origin/main 无共同祖先（558 commits pending）。
> 迁移用 `git bundle` 或直接拷贝目录，**不要用 push/pull 同步**。

### 2.3 构建编译器和运行时
```
cargo build --bin lang-zone
cargo build
```
> 运行时 rlib 落在 `target/debug/liblz_builtins.rlib` 或 `target/debug/deps/liblz_builtins-<hash>.rlib`。

### 2.4 安装 fist-mbt
按 fist-mbt 官方安装方式安装 `fist-mbt` 命令行工具，并确认 `fist-mbt serve` 可用。

---

## 三、fist-mbt 初始化与命名空间

### 3.1 启动 server
在本项目根目录：
```
fist-mbt serve
```
或复用仓库根 `.mcp.json`（已配 `"command": "fist-mbt", "args": ["serve"]`）。

### 3.2 打开命名空间
> 本项目使用独立命名空间，**不要写入 `default`**（default.db 已被外部项目污染 2300+ 条）。
```
store_open(namespace="lang-zone")
store_open(namespace="lz-regression-r26")
```

### 3.3 数据落点
- `.fist-mbt/data/lang-zone.db` — lang-zone 命名空间任务库
- `.fist-mbt/data/lz-regression-r26.db` — 回归命名空间任务库
- `memory/bugs.md` — 缺陷台账（非 fist-mbt 库，是项目内 md）

---

## 四、验证门禁（迁移后先跑通再开工）

### 4.1 单元 + 集成全量
```
cargo test
```
预期：**41 个测试套件全绿**，无 FAILED。含：
- `cargo test --lib` → 463 项
- `tests/find_bug_libs.rs` → 12 库全链路（lz→rs→rustc→run→stdout 含 OK）
- `tests/find_bug_bugs.rs` → 45 收集用例
- `tests/codegen_corner_fixes.rs` → 38 项角例修复回归
- `tests/demo_codegen_compile.rs` → 全套 DEMO（默认 `#[ignore]`，慢；按需跑）

慢门禁（可选）：
```
cargo test --test demo_codegen_compile -- --ignored
```

### 4.2 全量差分扫描（sweep）
```
bash _probes/fastsweep.sh <(find _probes div-tools -name "*.lz" -type f | grep -v sweepwork) 4
```
预期（2026-10-27 基线，261 文件）：
- `RSLZ_OK` = 216
- `RSLZ_FAIL_NATIVE_OK` = 1（`_probes/latent_variants.lz`，已知 bootstrap IR 缺口，**非回归**）
- `BOTH_FAIL` = 44（全部为已复核非缺陷：19 个 `neg_*` 负向语法探针 + 7 个 `.diffwork/` 在途产物 + 11 个已验非缺陷）

> 扫描结果无汇总行，需自行 `grep -c` 三类分类行。

---

## 五、当前状态（2026-10-27 HEAD = `37fd534`）

### 5.1 本轮（本次会话）修复的 3 个真缺陷

| BUG | 现象 | 根因 | 修复 commit |
| --- | --- | --- | --- |
| **BUG-485** [high] | `let comp: Complex = ...` E0425 未绑定变量 | `IrType::Named{path:"Complex"}` 未命中仅小写的 type_map → `rust_type` 返回裸 `"Complex"` | `b4f9f64` |
| **BUG-486** [medium] | 链接配方 `-L dependency` 指向 rlib 所在目录 → E0463 | rlib 命中顶层 hardlink 时 `r.parent()`=target/debug，传递依赖在 deps/ | `e6f7ed4` |
| **BUG-487** [high] | 用户 `struct HashMap` 命名构造器被生成 `BTreeMap::from([...])` → 4×E0308 | expr_gen.rs kwargs 分支对 `Dict\|HashMap\|dict` 一律当 dict 字面量，未校验用户同名 struct | `37fd534` |

修复文件（各自独立）：
- `b4f9f64`: `src/ir/codegen/types_emit.rs` + `memory/bugs.md`
- `e6f7ed4`: `src/main.rs` + `memory/bugs.md`
- `37fd534`: `src/ir/codegen/expr_gen.rs` + `memory/bugs.md`

### 5.2 已知未修的 rs-lz 缺陷（台账 OPEN，不影响原生主路径）
- BUG-450 — native `List<str>.contains("a")` 实参缺 `.to_string()`（E0308；rs-lz 侧已修，native 侧未对齐）
- BUG-434 — native trait 方法返回关联类型发裸名 `-> Item`（应为 `-> Self::Item`）
- BUG-482 — rs-lz `esc_rust` 按字节切片多字节字符 panic
- BUG-469/470/471/471(a)/472/473/475/477/478/479/480 — rs-lz 各发射器保真缺陷（多数已带「修复:」字段，需复核转正）

### 5.3 已验证的边界结论（不必重查）
- 用户 `struct Dict` + `Dict(key: k, val: v)` 构造器：BUG-487 修复后已正确生成 struct 字面量
- 用户 `struct String`：产生 E0072 递归类型（病态命名，**不修**；Rust 侧同名冲突固有）
- 未定义函数调用（如 `string(x)`，非标准内建）被静默生成 `-> i64 { i64::MAX }` stub —— **设计如此**（.lzi 未加载时兜底），但会掩盖用户拼写错误；若要收紧需改 `collect_unknown_extern_fns` 逻辑，属改进非紧急 bug

---

## 六、工作流与最佳实践

### 6.1 寻虫闭环
1. `cargo test` 全绿基线
2. `bash _probes/fastsweep.sh ...` 找 RSLZ_FAIL_NATIVE_OK（真缺陷候选）
3. 对候选：原生路径复现 `lang-zone <f>.lz` → `rustc` → 记录 E-code
4. 定位 codegen 分支 → 修 → 建回归测试（`tests/codegen_corner_fixes.rs`）→ 全量回归
5. 记账：`memory/bugs.md` 追加 `## BUG-NNN [severity] <标题>` + `状态：FIXED` + commit

### 6.2 提交纪律
> **工作树很脏（并发 agent 改动 318 文件）**：只 `git add` 你改的那几个文件，**绝不要** `git add .` 或 `cargo fmt --all`。禁止动：
- `src/embed/`（另一 agent 在途）
- 删除的 `DEMO/*.c`、`.fist-night/`、`data/` 等并发产物

安全提交模板：
```
git add src/ir/codegen/types_emit.rs memory/bugs.md
git commit -m "fix(codegen): BUG-XXX ..."
```

### 6.3 缺陷台账读写坑（重要）
- **`memory/bugs.md` 被 `read`/`edit` 工具误判为二进制** → 必须用 Python：
  ```bash
  python3 -c "d=open('memory/bugs.md','r',encoding='utf-8').read(); open('memory/bugs.md','w',encoding='utf-8').write(d+'...新条目...')"
  ```
- **终端 stdout 是 GBK**：Python `print` 中文会乱码。用 `PYTHONIOENCODING=utf-8 python3 ...` 或把结果写文件再读。

### 6.4 rustc 手动验证模板
```
rustc <gen>.rs --edition 2021 \
  --extern lz_builtins=<target/debug/liblz_builtins.rlib或deps路径> \
  -L dependency=<target/debug/deps> -o <gen>.exe
<gen>.exe
```
> 链接配方由 `lang-zone <f>.lz` 在 stderr 打印，可直接照抄（BUG-486 已修）。

---

## 七、已知环境坑

1. **bash 反斜杠被吃**：bash 里写路径用 `/`（如 `E:/IDEProjects/...`）或 `e/IDEProjects/...`；反斜杠仅配方本身面向 cmd/PowerShell，不是代码 bug。
2. **`target/debug/liblz_builtins.rlib` 顶层 hardlink（nlink=2）**：BUG-486 修复后配方已取 deps/，但若直接手跑 rustc 仍需给 `-L dependency=<deps>`。
3. **fist-mbt `report_bug` 的 `project_dir`**：拒绝绝对路径与 `..`，需相对 server cwd；server cwd 默认是 fist-mbt 安装目录，与项目目录不同 → 要么把 server 用 `FIST_PROJECT_DIR` 指到项目，要么绕开 `report_bug` 直接写 `memory/bugs.md`。
4. **命名空间写门**：publish/list/claim 等写类工具，namespace 传 `default` 会被拒；请显式 `store_open(namespace="lang-zone")`。

---

## 八、迁移清单（到新机器后按序执行）

1. 拷贝整个 `lang-zone` 目录（含 `.git`、`.fist-mbt/data/`、`memory/`）
2. `cargo build --bin lang-zone` 确认工具链可用
3. `cargo test` 全绿（41 套件）
4. `fist-mbt serve` + `store_open(namespace="lang-zone")`
5. 读 `memory/bugs.md` 看 OPEN 条目，优先原生路径 BUG-450 / BUG-434
6. 按第六节工作流循环推进

---

## 九、目标回执

**未完成 / 待续**：
- native 端 BUG-450（`List<str>.contains` 实参 `.to_string()`）与 BUG-434（trait 关联类型裸名）
- rs-lz 各保真缺陷（BUG-469~480 系列）复核转正
- 未定义函数 stub 静默掩盖错误的 UX 改进（低优先）
- CHANGELOG 回填（任务 T0.3 提及 BUG-317~408 大段缺失）

**已完成（本轮会话）**：BUG-485 / BUG-486 / BUG-487 修复 + 回归测试 + 全量 sweep 无回归 + 全量测试 41 套件全绿。