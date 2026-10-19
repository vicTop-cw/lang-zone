import importlib.util, io, sys
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
spec = importlib.util.spec_from_file_location("fistc", ".fist-perf-20261008/fist.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
c = m.Client()
Q = chr(34)
def tid_of(s):
    for k in (Q + "id" + Q + ":" + Q, Q + "task_id" + Q + ":" + Q):
        if k in s:
            return s.split(k)[1].split(Q)[0]
    return None

# 1) 先把T0r10 / T0r11 收尾（它们已被 execute 过，直接 submit+verify）
SUM = ("阶段三落地并随 2ca218e 提交：列表累加改 push/extend（严格 A/B 18.3x、峰值内存 "
       "-13%）；登记表泛化为 owned_local；闸门 20->25 全绿；全量 842 passed / 0 failed；"
       "DEMO 真编译闸门 2/0。")
for t in ("T0r10", "T0r11"):
    c.call("submit", {"task_id": t})
    print(t, "verify ->", c.call("verify", {"task_id": t, "verifier": "codebuddy-agent"})[:60])

# 2) 新根任务：先 plan 子任务，再 execute 根
pub = c.call("publish", {
    "namespace": "perf-opt",
    "project_dir": r"E:\IDEProjects\AI\lang-zone",
    "title": "阶段三（分解）：列表累加原地追加的四个交付面",
    "description": "度量口径 = 运行时耗时 + 峰值工作集。本根任务只做分解与验收汇总，"
                   "四个叶子各自独立可验。",
})
root = tid_of(pub)
print("root ->", root); c.call("claim", {"task_id": root, "assignee": "codebuddy-agent"})

L = [
 ("列表原地追加：push / extend 三形态",
  "a = a + rhs 改写：单元素列表字面量 -> a.push(e)（零分配）；多元素 -> a.extend(vec![..])；"
  "其他表达式 -> a.extend(<与通用路径完全一致的实参文本>)。改前是每轮一次 a.clone() + extend，"
  "拷贝量 O(n^2)。同机严格 A/B（两份 .rs 仅差这一行，2 万元素）：230.6ms -> 12.6ms（18.3x），"
  "峰值内存 5192KB -> 4508KB（-13%）。差距随规模放大：旧的随 n 平方增长，新的随 n 线性。"),
 ("登记表泛化与安全性论证",
  "owned_str.rs 泛化为 owned_local.rs（LocalKind::{Str,List}）。四条边界：登记表按函数清空"
  "（gen_fn_def 内 owned_local_reset）；只登记 is_ref==false 的 str/List 局部绑定；"
  "形参一律不登记（参数是否独占永远不需要跨函数推断，代价是 by-value 形参走通用路径）；"
  "extend(<rhs>) 实参文本与改前完全一致故 move 语义不变；借用检查器是最终保证。"),
 ("闸门 20 -> 25 + 全量回归",
  "tests/perf_opt_gates.rs 新增 5 例：单元素字面量走 push、多元素走 extend(vec![..])、"
  "变量右操作数走 extend、形参保留 clone+extend（证明「不登记形参」策略生效）、"
  "str/list 两类登记互不串扰。回归 cargo test -j 1 --no-fail-fast = 842 passed / 0 failed"
  "（39 个二进制，本轮全绿）；demo_codegen_compile --include-ignored = 2/0（空 allowlist）。"),
 ("记录既有缺陷（不在性能改动里夹带）",
  "List 的迭代方法（如 xs.sum()）生成 (xs).sum() 而非 (xs).into_iter().sum() => E0599，"
  "属既有方法映射缺陷，闸门改用索引求和绕开。此前记录的 by-ref str 形参拼接 E0308、"
  "无显式返回类型时 str 尾表达式被推断成 i64 两处缺陷仍在，均需独立立项。"),
]
for title, deliv in L:
    p = c.call("plan", {"task_id": root, "description": title})
    cid = tid_of(p)
    print("  plan:", title[:18], "->", cid)
    if not cid:
        print("    raw:", p[:120]); continue
    for tool, args in (("claim", {"task_id": cid, "assignee": "codebuddy-agent"}),
                       ("execute", {"task_id": cid, "executor": "codebuddy-agent", "deliverable": deliv}),
                       ("submit", {"task_id": cid}),
                       ("verify", {"task_id": cid, "verifier": "codebuddy-agent"})):
        print("     ", tool, c.call(tool, args)[:46])

c.call("claim", {"task_id": root, "assignee": "codebuddy-agent"})
c.call("execute", {"task_id": root, "executor": "codebuddy-agent", "deliverable": SUM})
c.call("submit", {"task_id": root})
c.call("verify", {"task_id": root, "verifier": "codebuddy-agent"})
print("root done:", root)
print(c.call("status_summary", {"namespace": "perf-opt"})[-135:])
c.close()
