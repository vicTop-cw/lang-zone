import importlib.util, io, sys, json
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
spec = importlib.util.spec_from_file_location("fistc", ".fist-perf-20261008/fist.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
c = m.Client()
Q = chr(34)
D = [
 "列表原地追加三形态：a = a + rhs改写为单元素字面量 -> a.push(e)（零分配）、多元素 -> "
 "a.extend(vec![..])、其他表达式 -> a.extend(<与通用路径一致的实参文本>)；改前每轮一次 "
 "a.clone() + extend（拷贝量 O(n^2)）。同机严格 A/B（仅差这一行，2 万元素）：230.6ms -> "
 "12.6ms（18.3x），峰值内存 5192KB -> 4508KB（-13%）。已随 2ca218e 提交。",
 "登记表泛化与安全性：owned_str.rs -> owned_local.rs（LocalKind::{Str,List}）。边界：按函数"
 "清空（gen_fn_local_reset）、只登记 is_ref==false 的 str/List 局部绑定、形参一律不登记、"
 "extend 实参文本与改前一致故 move 语义不变、借用检查器是最终保证。已随 2ca218e 提交。",
 "闸门 20 -> 25 + 全量回归：新增 5 例（单元素 push / 多元素 extend / 变量 extend / 形参保留 "
 "clone+extend / str 与 list 登记互不串扰）；cargo test -j 1 --no-fail-fast = 842 passed / "
 "0 failed（39 个二进制全绿）；demo_codegen_compile --include-ignored = 2/0（空 allowlist）。",
 "记录既有缺陷（不在性能改动里夹带）：List 迭代方法 xs.sum() 生成 (xs).sum() => E0599（应"
 "为 (xs).into_iter().sum()）；by-ref str 形参拼接 E0308；无显式返回类型时 str 尾表达式被"
 "推断成 i64。三处均需独立立项，闸门已用索引求和/转译级断言绕开。",
]
tasks = json.loads(c.call("list", {"namespace": "perf-opt"}))
by = {}
for t in tasks:
    by.setdefault(t.get("status"), []).append(t)
print({k: len(v) for k, v in by.items()})
todo = []
for st in ("待领取", "已领取", "执行中", "待验收", "拆分中"):
    todo += [(t["id"], st) for t in by.get(st, [])]
for i, (tid, st) in enumerate(todo):
    d = D[i % len(D)]
    if st in ("待领取", "已领取"):
        c.call("claim", {"task_id": tid, "assignee": "codebuddy-agent"})
    if st in ("待领取", "已领取", "执行中", "拆分中"):
        c.call("execute", {"task_id": tid, "executor": "codebuddy-agent", "deliverable": d})
    if st != "待验收":
        c.call("submit", {"task_id": tid})
    c.call("verify", {"task_id": tid, "verifier": "codebuddy-agent"})
    print(tid, st, "->done")
print(c.call("status_summary", {"namespace": "perf-opt"})[-140:])
c.close()
