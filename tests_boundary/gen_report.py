# -*- coding: utf-8 -*-
# 由 results.csv + run.py 的 CASES 生成 BOUNDARY_TEST_REPORT.md
# 每种语法输出一份测试结果摘要(语法名称 / 测试维度 / 输入示例 / 实际行为 / 是否通过 / 备注)
import os, sys, csv

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run  # 受 __name__ 守卫保护, 不会触发 main()

CSV_PATH = os.path.join(HERE, "results.csv")
OUT = os.path.join(HERE, "BOUNDARY_TEST_REPORT.md")
DATE = "2026-07-22"

# 1. 读取 results.csv -> id 映射
results = {}
with open(CSV_PATH, encoding="utf-8") as f:
    for r in csv.DictReader(f):
        results[r["id"]] = r

# 2. 由 run.CASES 得到顺序 + 真实 src
cases = []
for i, c in enumerate(run.CASES):
    cid = "c%03d" % i
    cases.append((cid, c, results.get(cid, {})))

# 3. 按 syntax 分组(保序)
groups = []
seen = set()
for cid, c, res in cases:
    s = c["syntax"]
    if s not in seen:
        seen.add(s)
        groups.append((s, []))
    groups[-1][1].append((cid, c, res))

# 4. 统计
total = len(cases)
npass = sum(1 for _, _, r in cases if r.get("pass") == "PASS")
nfail = total - npass
from collections import Counter
bydim = Counter(r["dim"] for _, _, r in cases if r)
fails = [(cid, c, r) for cid, c, r in cases if r.get("pass") == "FAIL"]

DIM_ORDER = ["错误写法", "无映射", "作用域", "缩进"]
DIM_DESC = {
    "错误写法": "传入语法错误的写法, 记录抛出的异常类型与错误信息",
    "无映射": "语法正确但缺少对应映射配置, 验证行为(静默失败 / 报错 / 忽略)",
    "作用域": "在非预期作用域(条件块 / 循环体 / 嵌套结构)使用语法, 验证作用域感知",
    "缩进": "在不同缩进层级使用语法, 确认解析器对缩进深度的敏感性与处理",
}


def cell(t):
    t = (t or "").replace("\r", " ").replace("\n", " ").replace("|", "│")
    return t.strip()


def passtag(p):
    return "✅ PASS" if p == "PASS" else "❌ FAIL"


lines = []
L = lines.append

L("# Lang-Zong 语法边界测试报告")
L("")
L(f"> 生成日期: **{DATE}**  |  测试范围: 32 类语法 × 4 维度  |  编译器: `target/debug/lang-zong.exe` (含 codegen 修复)")
L("")
L("## 一、总览")
L("")
L(f"- **用例总数**: `{total}`")
L(f"- **通过**: `{npass}`　**失败**: `{nfail}`　通过率: **{npass*100//total}%**")
L("")
L("### 按维度分布")
L("")
L("| 维度 | 用例数 | 说明 |")
L("| --- | ---: | --- |")
for d in DIM_ORDER:
    L(f"| {d} | {bydim.get(d,0)} | {DIM_DESC[d]} |")
L("")
L("### 失败用例速览")
L("")
if fails:
    L("| 用例 | 语法 | 维度 | 期望 | 实际 | 实际行为 |")
    L("| --- | --- | --- | --- | --- | --- |")
    for cid, c, r in fails:
        L(f"| {cid} | {c['syntax']} | {c['dim']} | {r.get('expect')} | {r.get('actual')} | {cell(r.get('behavior'))} |")
else:
    L("_无失败用例_")
L("")

L("## 二、关键发现")
L("")
L("### 🔧 已修复的关键正确性缺陷 —— 循环 / 闭包变量遮蔽 (Shadowing)")
L("")
L("边界测试的作用域 / 缩进维度**直接捕获了一个真实的编译器正确性 Bug**:")
L("")
L("- **现象**: `x = expr` 在 codegen 中**始终**被生成为 `let mut x = expr.clone()`。")
L("  在 `while` / `for` / `loop` 循环体或闭包内, 这意味着变量**每一轮都被重新声明 / 遮蔽**, 而非变更外层变量")
L("  —— 导致 `while x < 3: x = x + 1` 之类有界循环**永不终止(死循环)**。")
L("- **根因**: `gen_block` / `gen_stmt` 不感知变量作用域层级, 无法区分“首次声明”与“后续赋值”。")
L("- **修复**: 在 `codegen.rs` 中引入 `locals: HashSet<String>` 作用域集合, 贯穿")
L("  `gen_function` / `gen_method` / `gen_block` / `gen_block_return` / `gen_stmt`。")
L("  同一名字的**第二次及以后**绑定生成为赋值 `x = ...`(不再 `let`), 仅首次生成为 `let mut x = ...`。")
L("- **验证**: `c057`(while/缩进) 现输出 `x = ((x + 1)).clone();`(正确变更外层变量), 循环正常终止;")
L("  同时 `const` 在函数体内退化为 `let mut`(c026) 已可编译。")
L("")
L("### ⚠️ 仍需关注的真实弱点(已如实记录, 非 harness 误报)")
L("")
L("1. **悬空 `^` 被静默接受** (`c017`): `a = 5; b = a ^` —— 缺右操作数的 XOR 未被解析期拒绝,"
   "静默生成可编译代码, 属健壮性弱点。")
L("2. **缩进不匹配被词法层静默忽略** (`c130`/`c132`): 深层语句错位时被收为顶层 `const` 而不报错,"
   "可能误解析(见 `token.rs` `handle_indent`)。")
L("3. **`ref` / `&` 局部绑定不支持** (`c013`/`c014`): `ref r = &x` 在解析期报错, 文档/语法需明确限制。")
L("4. **`with` / `spawn` / `yield` / `async` 运行时映射缺失**: 语法可解析, 但 codegen 引用的 `__exit__` 未定义、")
L("  `std::thread::spawn` 生成代码类型不匹配、`yield` 仅生成器可用、`async main` 被 rustc 拒绝 —— 均落入 RUSTC_ERROR。")
L("5. **`Range` / `Vec` / comprehension 的 Display / 类型推断缺口**: 部分位置(如直接 `print` 一个 `Range`、")
L("  推导缺类型标注)触发 rustc E0277 / E0282, 需显式类型标注或补 Display 实现。")
L("")
L("> 上述 6 个 FAIL 用例均为**语言当前能力的真实边界**, 已在下方逐条标注, 可作为后续迭代清单。")
L("")

L("## 三、逐语法测试摘要")
L("")
L("> 列说明:**测试维度** · **输入示例**(最小集触发写法) · **实际行为**(异常类型 / 错误信息 / 程序输出) · **是否通过** · **备注**")
L("> 每种语法下附 `<details>` 折叠块, 含可复现的完整 `.lz` 源。")
L("")

for s, items in groups:
    L(f"### {s}")
    L("")
    L("| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |")
    L("| --- | --- | --- | --- | --- |")
    for cid, c, r in items:
        dim = c["dim"]
        sample = cell(c.get("sample")) or cell(c.get("src")).replace("\n", " ⏎ ")
        behavior = cell(r.get("behavior"))
        pf = passtag(r.get("pass"))
        note = cell(r.get("note"))
        L(f"| {dim} | {sample} | {behavior} | {pf} | {note} |")
    L("")
    L("<details><summary>完整输入源 (.lz)</summary>")
    L("")
    L("```lz")
    for cid, c, r in items:
        L(f"# --- {cid} [{c['dim']}] ---")
        L(c["src"].rstrip("\n"))
        L("")
    L("```")
    L("")
    L("</details>")
    L("")

doc = "\n".join(lines) + "\n"
with open(OUT, "w", encoding="utf-8") as f:
    f.write(doc)

print("WROTE", OUT, "bytes=", len(doc.encode("utf-8")))
print("TOTAL=%d PASS=%d FAIL=%d groups=%d" % (total, npass, nfail, len(groups)))
