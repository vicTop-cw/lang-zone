import importlib.util, json, io, sys
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
spec = importlib.util.spec_from_file_location("fistc", ".fist-perf-20261008/fist.py")
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
c = m.Client()

ROOT = "T0r9"

r = c.call("execute", {"task_id": ROOT, "executor": "codebuddy-agent", "deliverable":
    "阶段一（方向A/B/C/D/E）已随 7f96abb 提交并全量验证；阶段二按用户要求把度量口径切到"
    "运行性能与运行时内存（代码行数不再作为指标），并在同一方向继续优化。"})
print("execute:", r[:140])

LEAVES = [
    ("阶段二-度量口径切换",
     "把验收口径从「产物行数/diff 行数」改为「运行时耗时 + 峰值工作集」：沿用 "
     ".fist-perf-20261008/perf/measure.py（分段计时 + 峰值工作集 + 与手写 Rust 的相对比值），"
     "并新增 ab_alloc.py / ab_str.py / ab_str_mem.py 做「两份产物仅差一行」的严格 A/B，"
     "排除机器负载漂移（复测期间锚点 rustc 段同步从 328ms 漂到 449ms）。"),
    ("阶段二-字符串原地追加",
     "发现并消除累加模式的 O(n^2)：通用路径发 s.to_string() + &\"ab\".to_string()[..]，"
     "每轮 3 次堆分配且累积串被完整拷贝两遍。新增 src/ir/codegen/owned_str.rs（by-value str 局部"
     "绑定登记表 + str_append_rhs 判定），s = s + rhs 改为 s.push_str(&rhs)，字面量右操作数剥掉"
     "多余 .to_string()。严格 A/B：40.5ms -> 19.7ms（2.06x）；端到端 362.9ms/7892KB -> "
     "142.9ms/4308KB（峰值内存 -45%），相对手写 push_str 从慢 2.49x 变为噪声内持平。"),
    ("阶段二-安全边界与闸门",
     "登记表按函数清空（gen_fn_def 内 owned_str_reset，紧邻 str_typed_vars.clear()）；"
     "只登记 is_ref==false 的 str 绑定；形参一律不登记（保守）；RHS 必须是 str 形态。"
     "tests/perf_opt_gates.rs 14 -> 20 例，新增 6 例钉「该改写的改、不该改的绝不改」："
     "字面量走 push_str 字面量且无 to_string()+add、变量走 push_str 借用且 x 后续仍可读、"
     "f-string 走 push_str 借用 format!、str+int 被类型检查拒绝、ref 形参绝不原地改写、"
     "登记表按函数隔离。回归 cargo test -j 1 --no-fail-fast = 691 passed / 2 failed"
     "（2 项为并行 agent 在途 WIP 的既有失败）。"),
    ("阶段二-提交卫生",
     "两次提交均只含本次改动：混合文件（codegen/mod.rs、stmt_gen.rs、decl_gen.rs、expr_gen.rs）"
     "按 hunk 精确分离，残差校验 RESIDUAL_MARKERS=0；并对「HEAD+仅我的改动」快照用 "
     "git checkout-index + cargo check --all-targets 验证可独立编译（0 error）。"
     "工作树中他人未提交 WIP 完整保留未暂存。"),
]
for name, deliv in LEAVES:
    pid = c.call("plan", {"task_id": ROOT, "description": name})
    tid = pid.split('"id":"')[1].split('"')[0] if '"id":"' in pid else None
    print("plan:", name, "->", tid)
    if not tid:
        print("  raw:", pid[:200])
        continue
    for step in (("claim", {"task_id": tid, "assignee": "codebuddy-agent"}),
                 ("execute", {"task_id": tid, "executor": "codebuddy-agent", "deliverable": deliv}),
                 ("submit", {"task_id": tid}),
                 ("verify", {"task_id": tid, "verifier": "codebuddy-agent"})):
        print("   ", step[0], c.call(step[0], step[1])[:90])
c.close()