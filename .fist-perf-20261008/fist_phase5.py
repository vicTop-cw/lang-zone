import importlib.util, io, sys
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
spec = importlib.util.spec_from_file_location("fistc", ".fist-perf-20261008/fist.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
c = m.Client()
D = [
 ("Dict/Set `+` 合并此前生成无法编译的代码（E0369）",
  "Rust std 未为 BTreeMap/Vec/HashSet 实现 Add（已用独立最小程序实证三者全部 E0369），"
  "而通用路径对 dict/set 直接透传 +，故 LZ 里 d = d + {...} / s = s + {...} 从来没编译过"
  "（list 之前幸免：走专用 __cat=clone()+extend 模板）。"),
 ("四类容器的原地合并（str/List/Dict/Set）",
  "登记表扩到 LocalKind::{Str,List,Dict,Set}，x = x + rhs 统一降级为原地：单条目 dict → "
  "d.insert(k,v)；多条目 → d.extend([(k,v),..])；单元素 set → s.insert(e)；多元素 → "
  "s.extend([..])；变量右操作数 → extend(rhs)（实参文本与前一致，move 语义不变）。"
  "Dict 字面量 IR 是 StructCtor{name:Dict, fields:[(_k0,k),(_v0,v),..]}，Set 字面量脱糖成 set!() 调用。"
  "附带：list 多元素由 vec![..] 改为数组字面量，再省一次分配。"),
 ("收益如实记录：不是性能问题，是能不能编译",
  "旧产物编不过，无法直接 A/B。只能与「道德等价基线」（每轮建临时表再 extend）比："
  "8000 元素 21.3 vs 22.8ms、50000 元素 28.1 vs 29.0ms —— 噪声内无差异"
  "（BTreeMap 插入 O(log n) 而临时表只1 条目）。收益 = 从编译失败变成可编译可运行，不宣称提速。"),
 ("闸门 32 -> 38 + 全量回归",
  "新增 6 例：dict 单/多条目、set 单/多元素、dict/set 变量右操作数，外加一条回归用例钉住 "
  "list 三形态不受影响（push / extend([..]) / extend(rhs)）。"
  "cargo test -j 1 --no-fail-fast = 855 passed / 0 failed（39 个二进制全绿）；"
  "demo_codegen_compile --include-ignored = 2/0（空 allowlist，348 DEMO 真编译）。提交 168e806。"
  "另记录未修缺口：空容器只能用大写 Dict()/List()/Set() 构造，小写 dict()/list()/set() "
  "未接线（生成桩函数 ⇒ rustc E0425）；lz_std 用的都是大写形式，非阻塞但错误信息不友好。"),
]
pub = c.call("publish", {"namespace":"perf-opt","project_dir":r"E:\IDEProjects\AI\lang-zone",
  "title":"阶段五：修复 Dict/Set `+` 合并的 E0369 硬缺陷（四类容器原地合并）",
  "description":"继续按运行时口径排查同类形态时发现的第四个既有缺陷：生成的 Rust 必然编译失败。"})
Q = chr(34); root = pub.split(Q+"task_id"+Q+":"+Q)[1].split(Q)[0]
print("root ->", root)
c.call("claim", {"task_id": root, "assignee":"codebuddy-agent"})
for title, deliv in D:
    p = c.call("plan", {"task_id": root, "description": title})
    if not (Q+"id"+Q+":"+Q) in p:
        print("  plan(batch):", title[:16], "->", p[:60]); continue
    cid = p.split(Q+"id"+Q+":"+Q)[1].split(Q)[0]
    print("  plan:", title[:16], "->", cid)
    for tool, args in (("claim",{"task_id":cid,"assignee":"codebuddy-agent"}),
                       ("execute",{"task_id":cid,"executor":"codebuddy-agent","deliverable":deliv}),
                       ("submit",{"task_id":cid}),("verify",{"task_id":cid,"verifier":"codebuddy-agent"})):
        print("     ", tool, c.call(tool,args)[:40])
c.call("execute",{"task_id":root,"executor":"codebuddy-agent","deliverable":
  "Dict/Set 的 + 合并此前生成无法编译的代码（Rust 未为 BTreeMap/Vec/HashSet 实现 Add，E0369）。"
  "已随 168e806 修复：四类容器统一降级为 insert/push/extend 原地操作。闸门 38/0；全量 855/0。"})
c.call("submit",{"task_id":root}); c.call("verify",{"task_id":root,"verifier":"codebuddy-agent"})
print("done:", root); print(c.call("status_summary",{"namespace":"perf-opt"})[-120:])
c.close()
