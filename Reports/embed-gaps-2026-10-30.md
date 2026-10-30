# 嵌入式统一抽象 §6「已知限制」第 1~4 条 —— 现状核查（2026-10-30 / T0r49 / ns=lang-zone）

> 被核对象：`docs/嵌入式统一抽象-实现-2026-10-21.md` §六 已知限制第 1~4 条（文档落笔于 2026-10-21）。
> 核查基准：工作树 `HEAD=31a859f` + 本车道改动（`src/bridge/cli.rs`、`tests/embed_unify_r15.rs`）。
> 本文里的 `file:line` 都是**核查当场**用 needle-grep 取的实际行号（文档 §七 自己定的规矩：叙述性引用只写文件+符号，
> 精确行号留给现场生成的锚点）。复取锚点的命令写在每条证据里。
> 判据复跑读数：`cargo test --test embed_unify_r15` **21 passed / 0 failed**；`cargo test --lib` **469 passed / 0 failed**。

## 〇 结论速览

| # | 文档声称（2026-10-21） | 核查现状（2026-10-30） | 判定 |
|---|---|---|---|
| 1 | 可序列化闭集只有 `int/float/str/bool`；闭集外签名不出垫片、逐条点名 | 闭集**仍是 4 类**，点名机制在位且实测逐条打回执。**但**文档未点名的一类静默面被实测抓到：**无返回注解的函数被当成 Unit ⇒ 返回值静默丢弃**，MANIFEST 还标 `callable` | **未修**（闭集口径成立）+ 一条文档没写的静默面 |
| 2 | `mojo`/`c`/`scala` 的分发入口未经工具链验证（本机无 mojo、无 gcc），MANIFEST `toolchain_available=no` 就是这个读数 | 本机现在**有 gcc**、仍无 mojo、**scalac 其实一直在**（Coursier `.bat`，`command -v` 看不见）。一验露三层：① `toolchain_available` 量的是 **run 命令**而不是 build 工具链，gcc 建库成功后照旧报 `no`（读数不可归因）；② **c 库产物修复前根本调不起来**（Windows 相对程序名按父进程 cwd 解析 ⇒ `os error 2`），已修；③ scala 无分发入口却照发可调用垫片，且 `.bat` 形态的 `scalac` 读数说在场、spawn 说 `program not found` | **部分**（c 已修并可端到端调用；mojo 无工具链不可验；scala 两条失真未修；读数口径未修） |
| 3 | 默认路径不起子进程，要真建库必须显式 `--embed-build` | 成立，且是**刻意设计**；实测默认跑只落盘 3 个文件、无 `.exe`，加 `--embed-build` 才 `gcc` 建库 | **未修（按设计保留）** |
| 4 | LZ 侧 `print(<垫片调用>)` 走 Debug，先绑 `let` 才 Display | 一字不差地复现：`println!("{:?}", python_calc_shout(...))`，跑起来打 `"AB"`；`let s: str = …` 那行打 `CD` | **未修** |

## 一、逐条证据

### 限制 1：可序列化闭集只有 `int/float/str/bool` —— 未修（闭集口径成立）

**代码现状**（闭集判定的六个落点，全部只认 `int/float/str/bool`）：

```
$ grep -n '不在闭集' src/embed/adapters.rs
165:  python 返回类型   unsupported = Some(format!("返回类型 {} 不在闭集 int/float/str/bool 内", r));
170:  python 形参       unsupported = Some(format!("参数 {} 的类型 {} 不在闭集内", p.name, p.lz_type));
413:  scala  泛形       unsupported = Some("带类型参数的泛形函数不在闭集内")
429:  scala  返回类型
438:  scala  形参
594:  c      形参（指针/结构体）
600:  c      返回类型
```

类型映射表本身也只有这四项：`src/embed/mod.rs:109` 的注释「闭集：int / float / str / bool」；
`src/embed/shim.rs` 的 `rust_param_ty` / `rust_ret_ty` 把 `_` 分支兜到 `&str` / `String`。
「不出垫片」由 `ForeignFn::callable()`（`src/embed/mod.rs`，`unsupported.is_none()`）把守，
`src/embed/shim.rs:42` 的 `.callable_methods()` 只把 callable 的翻成垫片。
「逐条点名」在 `src/main.rs:1507`（`"embed 未覆盖: {}::{} —— {}"`，循环源是 `t.unsupported_methods()`，`src/main.rs:1505`）。

**实测（探针 `target/lane-embedgaps/probe/p1b`，源码含 `-> List[int]` / `DenseMatrix` 形参）**：

```
$ ../../debug/lang-zone.exe calc.lz
embed 产物: build/embed/calc/python_calc kind=library files=3 (写盘 3 / 未变 0)
embed 未覆盖: python_calc::as_list —— 返回类型 ?List[int] 不在闭集 int/float/str/bool 内
embed 未覆盖: python_calc::handle —— 参数 p 的类型 ?DenseMatrix 不在闭集内

$ grep '^method=' build/embed/calc/python_calc/MANIFEST.txt
method=as_list params=n:int ret=?List[int] 返回类型_?List[int]_不在闭集_int/float/str/bool_内
method=handle params=p:?DenseMatrix ret=int 参数_p_的类型_?DenseMatrix_不在闭集内
method=ok params=n:int ret=int callable
```

`?` 前缀 + 原因进 MANIFEST、可调用面只剩 `ok` —— 与文档一致。

**文档没写、本轮实测补上的那一刀（静默面）**：**返回注解缺失**时既不进 unsupported，也不报错，
函数被当 Unit 处理（探针 `probe/p1`，`def as_list(n: int): return [n]`）：

```
$ grep '^method=' build/embed/calc/python_calc/MANIFEST.txt
method=needs_obj params=x:?未标注 ret=none 参数_x_的类型_?未标注_不在闭集内
method=as_list   params=n:int      ret=none callable      ← 有返回值，但标 callable、ret=none
method=with_kw   params=:?**kwargs ret=int  **kwargs_形参无法按_key=value_序列化
method=ok        params=n:int      ret=int  callable
```

LZ 侧拿到的是 `pub fn python_calc_as_list(n: i64) { … }`（`probe/p1/calc.rs:153`），
**调用成功、值被丢掉、没有任何回执**。文档 §六.1 只点名了「无注解形参 / `**kwargs` / 泛形 / 指针结构体」，
这一条不在其中 ⇒ 属于「以为被点名了、其实没有」的那类。

**最小方案（未实施）**：在 `src/embed/adapters.rs` 的 python `methods()` 里，当 `ret == None` 且函数体
出现**带值的** `return <expr>` 行时，置 `unsupported = Some("无返回注解且函数体有返回值 ⇒ 结果会被静默丢弃")`。
只加一行正则式扫描（`^\s*return\s+\S`），不影响 `def f(): pass` 这类真 Unit；
风险面是「作者本意丢弃返回值」的写法会从静默变成编译期点名，属预期收益。

### 限制 2：`mojo`/`c`/`scala` 分发入口未经工具链验证 —— 部分（c 那一路已修）

**2.0 工具链现状（与文档落笔时不同）**

```
$ for t in python mojo gcc scalac scala; do printf "%-8s " "$t"; command -v $t || echo MISSING; done
python   /g/Programs/Python/Python311/python
mojo     MISSING
gcc      /g/Programs/CodeArts IDE for Cpp/resources/app/tools/MinGW/bin/gcc    ← 文档落笔时没有
scalac   MISSING      ← 见下：git-bash 的 command -v 不认 .bat，scalac 其实是有的
scala    MISSING
```

`command -v` 的 MISSING 是**假阴性**：按编译器自己的 PATH 扫描口径（`src/embed/mod.rs:93` 收
`[".exe", ".cmd", ".bat"]`）扫一遍，scala/scalac 都在 Coursier 里以 `.bat` 形式存在：

```
$ python -c "…扫 PATH 上的 scala/scala.exe/scala.cmd/scala.bat…"
C:\Users\Administrator\AppData\Local\Coursier\data\bin\scala.bat
C:\Users\Administrator\AppData\Local\Coursier\data\bin\scalac.bat
```

⇒ 文档「本机只有 python 与 scalac，无 mojo、无 gcc」这句只**一半**过期：**gcc 现在有了**（c 那一路可验，见 2.2），
**mojo 仍然没有**，**scalac 一直在**（且它一验就露了 2.4 那条）。

**2.1 `toolchain_available` 读数口径 —— 未修（读数不可归因）**

该字段量的是 **`run` 命令**在场性，不是 build 工具链：

```
$ sed -n '103,106p' src/embed/artifact.rs
    s.push_str(&format!(
        "toolchain_available={}\n",
        if t.run.tool_available() { "yes" } else { "no" }
    ));
```

c 的 `run` 是 `./c_calc.exe`（`src/embed/adapters.rs:681`），含分隔符 ⇒ `tool_available` 走
`Path::new(program).exists()`（`src/embed/mod.rs:85`），相对**编译器 cwd** 求解 ⇒ 恒 false。实测两态都是 `no`：

```
# ① gcc 在场、未建库（默认路径）
$ ../../debug/lang-zone.exe calc.lz            # probe/p2c
embed 产物: build/embed/calc/c_calc kind=library files=3 (写盘 3 / 未变 0)
$ grep toolchain build/embed/calc/c_calc/MANIFEST.txt
toolchain_available=no                          ← gcc 明明在 PATH 上

# ② 加 --embed-build，gcc 真编成功、.exe 真落盘
$ ../../debug/lang-zone.exe calc.lz --embed-build
embed 建库: c_calc 完成 (gcc -O2 -o c_calc.exe __lz_dispatch_c_calc.c c_calc.c)
$ ls build/embed/calc/c_calc/
MANIFEST.txt  __lz_dispatch_c_calc.c  c_calc.c  c_calc.exe
$ grep toolchain build/embed/calc/c_calc/MANIFEST.txt
toolchain_available=no                          ← 建库成功之后仍报 no
```

⇒ 文档 §六.2「MANIFEST 里 `toolchain_available=no` 就是这个读数」这句话对 **c** 是**错的**：
它现在读的是「产物 exe 在编译器 cwd 下存不存在」，既不代表工具链缺席、也不代表建库失败。

**最小方案（未实施）**：`artifact.rs` 的 `manifest_text` 拆成两行——
`toolchain_available=` 用 `t.build.as_ref().map(|b| b.tool_available())`（无 build 步则 `n/a`）作**工具链在场**读数；
`run_available=` 保留现在这一行的语义（产物是否已在位）。两行各自可归因，且仍是确定性文本（无绝对路径、无时间戳）。

**2.2 c 库产物修复前调不起来（已修，本车道落地，见第二节）**

**2.3 scala：`dispatch=none` 却照发可调用垫片 —— 未修（失真面）**

`ScalaLang::dispatch_file` 明确返回 `None`（`src/embed/adapters.rs:484-486`，注释写着「不出假垫片」），
但 `EmbedTarget::callable_methods()`（`src/embed/mod.rs:155`）**不看** `has_dispatch_file()`（`:164`），
`src/embed/shim.rs:42` 与 `src/ir/codegen/mod.rs:651/693/712` 都只用它 ⇒ 没有分发入口的产物照样出垫片：

```
$ ../../debug/lang-zone.exe calc.lz          # probe/p2s，源：def add(a: Int, b: Int): Int = a + b
$ cat build/embed/calc/scala_calc/MANIFEST.txt
...
build=scalac scala_calc.scala
run=scala scala_calc.scala
toolchain_available=yes
dispatch=none                                 ← 没有分发入口
method=add params=a:int,b:int ret=none callable   ← 却标 callable，且 ret 解析成 none
$ grep -n 'scala_calc_add' calc.rs
170:pub fn scala_calc_add(a: i64, b: i64) {      ← 垫片照发，返回 Unit
```

两处失真叠在一起：① `ret=none` —— `fn_from_toplevel` 只认 `->` 形态的返回注解，scala 的 `: Int` 一律解析不到；
② 没有分发入口却给出调用面 ⇒ 真调起来必然打到 `scala scala_calc.scala add a=1 b=2`（把协议参数当脚本参数），
分发入口不存在 ⇒ 客户端拿到「无响应」panic。这与代码注释里「不出假垫片」的承诺相反。

**最小方案（未实施）**：`callable_methods()` 加一层 `has_dispatch_file()` 门（Library 档无分发入口 ⇒ 全员不可调用），
并把被这层门挡下的方法以 `unsupported = Some("该语言本版本不生成分发入口")` 记进 MANIFEST（保持「逐条点名」口径）；
再用 `src/embed/adapters.rs` 的 `fn_from_toplevel` 补一条 `: <Type>` 返回注解识别，让 `ret` 不再恒 `none`。
两处都在 `src/embed/`，不碰 `src/ir/`。

**2.4 scala 的建库步：读数说「在场」，spawn 说「找不到」—— 未修（读数与执行不一致）**

既然 `scalac` 在场，就给 scala 跑一次真建库（`probe/p2s`）：

```
$ ../../debug/lang-zone.exe calc.lz --embed-build
embed 产物: build/embed/calc/scala_calc kind=library files=2 (写盘 0 / 未变 2)
embed 建库: scala_calc 失败: spawn 失败: program not found
  命令: scalac scala_calc.scala
embed: 1 件失败 ⇒ 非零退出（不许报成功）
```

同一份 MANIFEST 里却写着 `toolchain_available=yes`。两边都对、因为量的不是同一件事：
`tool_available`（`src/embed/mod.rs:84-104`）按 `[".exe",".cmd",".bat"]` 扫 PATH 目录**文件名** ⇒ 认 `scalac.bat`；
而 `std::process::Command::new("scalac")` 走 CreateProcess，**`.bat` 不是可执行映像**，必须经 `cmd /C` 才能起
⇒ 恒 `program not found`。也就是说：凡是靠 `.bat` 包装装的工具链（Coursier 的 scala/scalac 是典型），
本抽象**永远建不成功**，而回执上永远写着「工具链在场」。

**最小方案（未实施，二选一，建议前者）**：
① `tool_available` 与 `build_artifact` 用**同一把尺**：`.bat`/`.cmd` 命中时，`build_artifact` 以
`Command::new("cmd").args(["/C", program, …])` spawn（Windows）；
② 或让 `tool_available` 不认 `.bat`/`.cmd` ⇒ 读数据实变 `no` + `Skipped`（诚实但把「装了却用不了」变成「没装」）。
两者都只动 `src/embed/mod.rs` 与 `src/embed/artifact.rs`，不碰 `src/ir/`。

### 限制 3：默认路径不起子进程 —— 未修（按设计保留）

**代码**：`src/main.rs:881`（注释）/ `:885`（`args.iter().any(|a| a == "--embed-build")`）；
project 模式同口径在 `src/main.rs:1339`、`:1391`；`src/cli.rs:596`（`lz build` 第三条路径）；
`src/embed/artifact.rs:156` 的 `dry` 参数是「只判工具链在场性、不真跑命令」的开关。

**实测（probe/p2c，同一棵树连跑两次）**：

```
# 默认
$ ../../debug/lang-zone.exe calc.lz
embed 产物: build/embed/calc/c_calc kind=library files=3 (写盘 3 / 未变 0)
$ ls build/embed/calc/c_calc/
MANIFEST.txt  __lz_dispatch_c_calc.c  c_calc.c          ← 没有 .exe，一个子进程都没起

# 显式
$ ../../debug/lang-zone.exe calc.lz --embed-build
embed 建库: c_calc 完成 (gcc -O2 -o c_calc.exe __lz_dispatch_c_calc.c c_calc.c)
```

判据侧由 `tests/embed_unify_r15.rs::compile_alone_never_spawns_a_toolchain` 钉着（本轮 21 条里通过）。

**判定**：文档这条**不是缺陷而是刻意的门**，核查确认它仍然成立、且默认路径确实 0 spawn；
代价依旧是「建库失败要第二次命令才看得见」——这条限制本轮**不动**（放宽它等于把「编译器一跑就 spawn 外部程序」变成既成事实）。

### 限制 4：LZ 侧 `print(<垫片调用>)` 走 Debug —— 未修

**根因仍在原位**：`src/ir/codegen/expr_gen.rs:200` 的 `print_arg_is_display(e: &Expr) -> bool`
是个**不带 `self` 的关联函数**，只看 `e.ty`（`:212` 起的三分支：`Str` / `Ref`|`MutRef` 内层 `Str` / `Named{String|str}`），
查不到 `fn_returns`；选择点在 `src/ir/codegen/expr_gen.rs:1962`
（`if Self::print_arg_is_display(a) { "{}" } else { "{:?}" }`）。垫片调用的返回类型登记在
`src/ir/codegen/mod.rs` 的 `embed_export_names()` → `self.fn_returns.insert(name, ty)`（`:905-910` 一带），
只服务于比较/赋值侧，`print` 这条路拿不到。

**实测（probe/p4，源：`print(python_calc_shout("ab"))` + `let s: str = python_calc_shout("cd"); print(s)`）**：

```
$ ../../debug/lang-zone.exe calc.lz && grep -n 'println!' calc.rs
25:    println!("{:?}", python_calc_shout("ab".to_string()));
27:    println!("{}", s);

$ LZ_EMBED_DIR=. ./calc.exe | cat -A
"AB"$
CD$
```

⇒ 与文档 §六.4 记载逐字一致（Debug 带引号、绑 `let` 后 Display 无引号），**未修**。

**最小方案（未实施，且有已知代价）**：`print_arg_is_display` 保持纯函数不改，在调用点
（`src/ir/codegen/expr_gen.rs:1960` 那个 print 分支）加一层 `self` 侧判定：
`ExprKind::Call{callee,..}` 且 `self.fn_returns.get(callee)` 是 `IrType::Str` ⇒ 走 `{}`。
这样 `print(<str 返回的函数调用>)` 与 `print(s)` 口径统一。**代价**：改的是 print 的**公共**选择路径，
凡「返回 str 却被 print 直接打」的既有用例（含 cy golden 的 oracle）都可能由 `"x"` 变 `x`
⇒ 必须同批复核 `tests/dual_route_golden.rs`、`tests/cy_codegen_gate.rs` 与 `FIND_BUG` 下样本，
不属「零破坏」范围 —— 这正是文档当初把它入账而不修的理由，本轮沿用它（未实施）。

## 二、本车道落地的那一条：限制 2 的 c 那一路

### 症状（修复前，实测）

c 的 `run` 命令是**相对**程序名 `./c_calc.exe`（`src/embed/adapters.rs:681`），
而垫片是 `Command::new(program)` + `cmd.current_dir(&dir)`。Windows 的程序名解析由 CreateProcess 按
**父进程** cwd 做，`current_dir` 只换子进程 cwd、管不到程序名本身 ⇒ 从任何非产物目录运行都必炸：

```
$ LZ_EMBED_DIR=. ./calc.exe            # 母件在 probe 根，产物在 build/embed/calc/c_calc
thread 'main' panicked at calc.rs:95:9:
embed 产物 c_calc 调用 add 失败：无法启动 `./c_calc.exe`（系统找不到指定的文件。 (os error 2)）；
产物目录 .\build/embed/calc/c_calc
rc=101
```

即：**c 库产物不是「未经工具链验证」，是「验证了也调不起来」**。python 那一路看不出来，是因为
`program` 是裸名 `python`（走 PATH），不吃这条路径。

### 改了什么（`src/bridge/cli.rs`，EMBED_SHIM_HEADER 内，+17 行）

新增 `__lz_resolve_program(dir, program)`：绝对路径原样；**含分隔符的相对路径**拼成 `<产物目录>/<程序>`；
裸名字（`python` / `mojo` / `scala` / 未知语言 canon）原样交给 PATH。
两个 runner（`__lz_call` 协议调用、`__lz_exec` 直接执行）都改用它 spawn。
`current_dir(&dir)` 保留（相对数据路径仍按产物目录解析）。

改动面：`src/bridge/cli.rs`（垫片模板文本，唯一子进程代码生成点）——
**没有**新增 `Command::new` 调用点，`embed_adds_exactly_one_spawn_site_and_it_lives_in_the_shim`
的「垫片内恰好 2 处 spawn」不变量仍成立（本轮该条通过）。

### 新增判据（`tests/embed_unify_r15.rs`，+53 行）

`c_library_artifact_is_invocable_from_an_unrelated_cwd`：
编一份 `#[embed(c)]` 的 `int add(int a,int b)`，`--embed-build`（gcc 在场就走真 gcc）；
产物 exe 不在位（无 gcc 的机器）时用 `rustc` 造一个守协议的替身 —— 本条测的是**程序路径怎么解析**，
不是「gcc 能不能编」（`Built`/`Failed` 两分支由 `src/embed/artifact.rs` 的
`build_step_actually_builds_when_tool_present` 钉着，本轮 `--lib` 里通过）。
然后**在母件自己的 cwd**（产物在 `build/embed/...` 子目录）运行，断言打印 `3`。

### 红/绿对照

| | 读数 |
|---|---|
| 修复前（盘上旧 `calc.rs`，`Command::new(program)`） | `rc=101`，panic `无法启动 ./c_calc.exe（os error 2）` |
| 修复后（同一探针重新生成） | `$ LZ_EMBED_DIR=. ./calc.exe` → `3`，`rc=0` |

修复后生成的垫片（probe/p2c2，`:104` / `:148` 两处 runner）：

```
    let mut cmd = Command::new(__lz_resolve_program(&dir, program));
```

## 三、复跑读数（串行，`CARGO_TARGET_DIR=target/lane-embedgaps`）

```
cargo test --test embed_unify_r15      →  21 passed; 0 failed; 0 ignored   （改动前 20/0，本轮 +1 条）
cargo test --lib                       → 469 passed; 0 failed; 0 ignored
```

改动前后 gate 都是全绿；新增的那条在修复前必红（红面见上表第一行，来自同一探针的旧产物）。

## 四、四条之外的现场观测（不在本题范围，只点名不代归因）

1. **c 的形参名带声明处的空白**：`int add(int a, int b)` → MANIFEST `params= a:int, b:int`、
   生成的分发入口 `__lz_argv_get(argv, argc, " a")`（probe/p2c3 里 `int   add(int   a…)` 得到 `"   a"`）。
   生产者与消费者由同一份 `ForeignFn` 生成 ⇒ 当前**自洽**、端到端仍是 `OK:3`；但协议文本里出现带前导空格的
   key 本身是解析残留（`src/embed/adapters.rs:567-578` 的 `pname` 回溯没 trim），
   外部按自然名 `a=1` 传参会被 `atol("")` 静默吃成 `0`。最小方案：`pname` 回溯后 `.trim()`，MANIFEST 与两处 key 同步。
2. **`src/embed/mod.rs:17-19` 的模块头注释仍写「LineProtocol／stdin 一行请求」**，与实现（argv + 单行 `OK:`/`ERR:`）
   和文档 §三 相反；`src/embed/shim.rs` 头部同一处已改成 argv。属注释陈旧，一行可修。
3. **`tool_available` 认 `.bat`、spawn 不认**：见 §2.4（scala 那一路 `toolchain_available=yes` 但 `program not found`）。
   这条与限制 2 同源，单列以防被读成「scala 只是没装」。
4. `probe/p2s` / `probe/p2c3` / `probe/p1b` 等探针目录都在 `target/lane-embedgaps/probe/` 下（构建产物区，
   不进版管）；要复现按本文命令原样跑即可。
