# lz-zed — Lang-Zone (lz) 语言的 Zed 语法高亮插件

为 [Lang-Zone](https://github.com/lang-zone) 语言（扩展名 `.lz`）提供 Zed 编辑器语法高亮。
语法基准为 `E:\IDEProjects\AI\lang-zone\SYNTAX`（权威语法规范 v3.3，2026-08-04/05），
本插件的词法规则与文档逐项对照（见 [词法元素对照清单](docs/LEXICAL-MATRIX.md)）。

## 特性

- 全部 **59 个关键字** + `True`/`False`/`...` 保留字（共 62 项）高亮（声明/控制流/异常/异步/宏/测试/运算符关键字分类着色）
- **duck 软关键字**（14 个，附录B §1.13）：仅在 `duck` 约束体内生效，体外保持普通标识符
- 内建类型 / 构造器 / 函数（48 个，prelude）按 `@type.builtin` / `@constant.builtin` / `@function.builtin` 着色
- 字符串全形态：`"…"` / `f"…"`（插值）/ `r"…"` / `"""…"""` / `f"""…"""` / `r"""…"""` / 反引号 quote 块
- 数字：十进制 / `0x` / `0o` / `0b` / 下划线 / 浮点 / 科学计数
- 注释：`//` 与 `/* */`（`#` 按规范为属性宏，不作注释）
- 运算符全集（45 个，含 `|>` `:=` `=:` `^:` `~:` `*:` `?.` `??` `..=` 等）
- 魔法方法 `__name__`、Unicode 标识符、`_` 通配、闭包 `|x| body`、模式、属性宏 `#!`/`#[]`、装饰器 `@`
- 缩进感知（4 空格块体自动缩进/对齐），大纲面板（def/struct/enum/trait/…）
- 测试套件：17 个 fixture + 快照断言 + 193 项词法元素三方覆盖校验（零依赖 Node）

## 目录结构

```
lz-zed/
├── extension.toml            # Zed 扩展清单（语言注册：.lz 关联、注释、括号等）
├── languages/lz/
│   ├── config.toml           # 语言配置（与 extension.toml 同步）
│   ├── highlights.scm        # 高亮查询（tree-sitter capture）
│   ├── indents.scm           # 缩进规则
│   └── outline.scm           # 大纲规则
├── grammar/
│   ├── grammar.js            # tree-sitter 语法定义（唯一词法源）
│   ├── tree-sitter.json      # tree-sitter CLI 配置
│   └── package.json          # npx 依赖声明
├── grammars/                 # 构建产物（lz.wasm）输出目录
├── syntaxes/
│   └── lz.tmLanguage.json    # TextMate 语法（测试基准 + 高亮规范可执行定义）
├── test/
│   ├── run-tests.js          # 测试运行器（快照 + 覆盖率，零依赖）
│   ├── fixtures/*.lz         # 17 个测试样例（覆盖全部语法元素）
│   └── expected/*.snap.json  # 高亮快照（--update 生成）
├── docs/
│   ├── LEXICAL-MATRIX.md     # 词法元素三方对照清单
│   └── TEST-REPORT.md        # 测试运行记录（由运行器生成）
├── demo/showcase.lz          # 综合展示文件（目视验证用）
├── showcase.html             # 浏览器端高亮预览（无需 Zed）
├── verify-all.ps1            # 一键验证（刷新快照 + 全量校验）
├── build.ps1 / build.sh      # 构建脚本
└── README.md
```

## 环境要求

| 项 | 要求 |
|----|------|
| Zed | 当前最新稳定版（Windows / macOS / Linux） |
| Node.js | ≥ 18（构建 grammar 与运行测试；测试套件零依赖） |
| tree-sitter CLI | 构建 grammar 时用（`npx --yes tree-sitter` 自动获取，或 `npm i -g tree-sitter-cli`） |
| 编译器 | WASM 构建如需要 Emscripten（`npm i -g @emscripten/emscripten` 或按 <https://emscripten.org> 安装） |

## 一、构建 grammar（一次性）

```powershell
cd E:\IDEProjects\AI\lang-zone\plugin\lz-zed
powershell -ExecutionPolicy Bypass -File build.ps1     # Windows
# 或
./build.sh                                              # macOS / Linux
```

脚本依次执行：

1. `tree-sitter generate` → 在 `grammar/` 下生成 `src/parser.c` 等
2. `tree-sitter build --wasm` → 产出 `grammars/lz.wasm`
3. `node test/run-tests.js` → 快照 + 覆盖率测试，全部通过才结束

也可以手动执行：

```powershell
cd grammar
npx --yes tree-sitter generate
npx --yes tree-sitter build --wasm -o ..\grammars\lz.wasm
cd ..
node test\run-tests.js
```

## 二、安装到 Zed（本地开发加载）

### 方式 1：命令面板（推荐）

1. 启动 Zed，按 `Ctrl+Shift+P`（macOS：`Cmd+Shift+P`）打开命令面板
2. 输入并执行 **`zed: install dev extension`**
3. 在弹出的文件选择器中选中本目录：`E:\IDEProjects\AI\lang-zone\plugin\lz-zed`（目录本身，不是子目录）
4. 状态栏出现 `lz` 语言提示即安装成功

> 开发模式加载的扩展在每次 `zed: reload dev extensions`（或重启 Zed）后生效；修改 `languages/lz/*.scm` 或 grammar 后需重新构建并重载。

### 方式 2：CLI

```powershell
zed --dev --install-extension E:\IDEProjects\AI\lang-zone\plugin\lz-zed
```

（`zed` 是否在 PATH 取决于安装方式；找不到时在 Zed 内用命令面板方式。）

## 三、验证高亮

### 3.1 语言识别

打开 `demo/showcase.lz`：编辑器**状态栏**应显示语言名为 `lz`（而非 "Plain Text" 或其它语言）。若未识别：

- 确认扩展已启用（方式 1 加载后，命令面板 `zed: extension manager` 中能看到 `lz`）
- 确认文件扩展名为 `.lz`

### 3.2 目视检查

用 `demo/showcase.lz` 逐项检查（对应词法元素）：

| 检查项 | 样例位置 | 预期 |
|--------|----------|------|
| 关键字 | `def` `struct` `match` `for` 等 | 关键字色（如紫/橙） |
| 字符串 | `"User(...)"` `f"user={...}"` | 字符串色，f-string 前缀同色 |
| 数字 | `0xDEAD_BEEF` `3.14159` `6.02e23` | 数字色 |
| 注释 | `// 注释` `/* */` | 注释色（灰/斜体） |
| 运算符 | `|>` `??` `^` `=:` `?.` | 运算符色 |
| 内建 | `List<int>` `print` `Some` | 内建类型/函数/常量色 |
| 魔法方法 | `__init__` `__str__` | 方法色 |

也可以直接打开 `showcase.html`（浏览器，无需 Zed）查看同一文件的着色效果，并支持选择任意 `.lz` 文件预览。

### 3.3 自动化测试（快照 + 覆盖率）

```powershell
node test\run-tests.js            # 校验：快照一致 + 193 项词法元素三方映射完整
node test\run-tests.js --update   # 首次或修改样例后重新生成快照
```

> 一键执行（刷新快照 + 全量校验）：`powershell -ExecutionPolicy Bypass -File verify-all.ps1`

- 结果写入 `docs/TEST-REPORT.md`（每个 fixture 的行数/token/scope 统计 + 覆盖率表格）
- 退出码 0 = 全部通过；1 = 存在失败项（详见报告）

## 四、卸载

1. 命令面板执行 **`zed: extension manager`**（或 `zed: installed extensions`）
2. 找到 `lz`，点击 Uninstall（本地开发加载的扩展同样在此移除）
3. 或 CLI：`zed --uninstall-extension lz`

## 五、常见问题排查

| 问题 | 原因与解决 |
|------|-----------|
| `tree-sitter generate` 报错 | Node/CLI 版本过旧：`npm i -g tree-sitter-cli@latest` 后重试；若报正则不支持 `\p{L}`，按 `grammar.js` 头部注释替换为 ASCII 字符类（语法规范本身以 ASCII 为主） |
| `tree-sitter build --wasm` 失败 | 需要 Emscripten：`npm i -g @emscripten/emscripten` 后重试；macOS 可 `brew install emscripten` |
| 已加载但 `.lz` 仍显示 Plain Text | ① 确认选中的是扩展根目录（含 `extension.toml` 的目录）；② 执行 `zed: reload dev extensions`；③ 确认文件后缀为 `.lz` |
| 高亮有误标/漏标 | 先跑 `node test\run-tests.js` 确认词法层是否与规范一致；若样例问题可 `--update` 后人工复核；若规则问题，修改 `languages/lz/highlights.scm` 或 `grammar/grammar.js` 后重新构建 |
| `grammars/lz.wasm` 缺失 | 未执行构建（见 §一），或构建目录被清理；重新 `build.ps1` |
| 状态栏显示但无颜色 | 主题映射问题：换 `One Dark`/`One Light` 等标准主题验证；capture 名均为 Zed 标准名（keyword/string/number/…），自定义主题需包含对应映射 |
| 需要发布到扩展市场 | 本插件暂以本地开发方式交付；发布需 Zed 账号与授权，另行处理 |

## 六、语法实现说明（与 SYNTAX 文档的关系）

- **词法基准**：`00-词法基础.md`（关键字/字面量/标识符/运算符/注释）、`附录B`（关键字全集/软关键字）、`12-操作符.md`（运算符全集/优先级）、`03e`（闭包）、`05`（控制流）、`08`（宏）、`15`（测试框架）、`99`（内建清单）
- **高亮机制**：Zed 使用 tree-sitter + `highlights.scm` 查询；`grammar.js` 为词法规则唯一来源，`syntaxes/lz.tmLanguage.json` 为同规范的 TextMate 等价定义（用作测试基准与可执行规范，支持未来 TextMate 语法能力的编辑器）
- **缩进敏感结构**：lz 为 4 空格缩进语言，块结构由编辑器维护；grammar 保持扁平结构以提升高亮鲁棒性（详见 `grammar.js` 头部设计说明）
- **对照清单**：`docs/LEXICAL-MATRIX.md` 逐项列出 每个词法元素 → 高亮规则 → 测试样例，可由测试运行器自动复核

## 七、不做范围（与目标约定一致）

- 不实现 LSP / 语义高亮 / 补全 / 格式化（语言服务另立项）
- 不修改 SYNTAX 语法文档本身
- 不发布 Zed 官方扩展市场
- 不保证 Zed 历史版本兼容（面向当前稳定版）

## License

MIT
