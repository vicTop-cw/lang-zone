import importlib.util, io, sys, json
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
spec = importlib.util.spec_from_file_location("fistc", ".fist-perf-20261008/fist.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
c = m.Client(); Q = chr(34)
def tid_of(s):
    for k in (Q+"id"+Q+":"+Q, Q+"task_id"+Q+":"+Q):
        if k in s: return s.split(k)[1].split(Q)[0]
    return None

pub = c.call("publish", {"namespace":"perf-opt","project_dir":r"E:\IDEProjects\AI\lang-zone",
  "title":"阶段四：修复三处既有缺陷（List 归约 / 返回类型推断 / 只读 ref 形参赋值）",
  "description":"前三轮记录为「已知缺陷，本轮修复」。三者都是编译期硬失败或生成非法代码。"})
root = tid_of(pub); print("root ->", root)
c.call("claim", {"task_id": root, "assignee":"codebuddy-agent"})
L = [
 ("缺陷A：List 归约方法走迭代器链",
  "xs.sum()/product() 此前 E0599：方法调用里 `if method==\"sum\" { return ({recv}).sum() }` "
  "不区分真迭代器与 List，而 Rust 的 Vec/HashSet 既不是 Iterator 也没有 sum/product。"
  "修复：List/Set 接收者走 ({recv}).iter().copied().sum::<T>()，T 取元素类型（非硬编码 "
  "i64，否则 List<float>.sum() ⇒ E0308）；用 iter() 而非 into_iter()（后者 move 接收者，"
  "LZ 值语义允许重复读）；渲染与自由函数 sum(collection) 完全一致。"),
 ("缺陷C：无返回注解时的返回类型推断",
  "ret_ty 在体转换之前算好，而 let 绑定类型登记发生在体转换中 ⇒ 尾表达式引用顶层 let "
  "变量时 lookup_var 落空 → Any → i64 ⇒ def accum() = ... s（str 尾）E0308。"
  "修复：用一次性探测副本 fn_ctx.clone() 预登记顶层 let 再推断。"
  "踩坑：直接登记进 fn_ctx 会让 convert_block 预扫描（!vars.contains 才登记 "
  "block_declared）误判为已声明而跳过真实声明 ⇒ E0425（实测），用副本彻底避免。"
  "只扫顶层不下钻嵌套块，避免与块内遮蔽打架。"),
 ("缺陷B：只读 ref 形参赋值给精确诊断",
  "ref x: T 在 Rust 侧是 &T，赋值在 LZ 语义下本就非法，但此前放行后生成 "
  "s = format!(..)/s = s + 1 ⇒ E0308/E0384 且诊断指向生成代码。"
  "修复：赋值点（Let 重绑定 + Assign 两路径）检测「目标是只读 ref 形参」→ 发compile_error! "
  "精确中文诊断。登记复用既有 owned_local 登记表（新增 LocalKind::ReadonlyBorrow），"
  "不引入新字段。刻意不改发射为 &mut T：ref self 是 std 协议 trait 的常规写法，"
  "改签名会与 trait 不匹配（E0053/E0195）波及所有扩展 trait；只读取用法完全不受影响。"),
 ("闸门与回归",
  "tests/perf_opt_gates.rs 25 -> 32 passed / 0 failed，新增 8 例含反向用例：显式注解与 int "
  "尾表达式不变、只读 ref 形参只读取仍工作且绝不渲染成 &mut、sum 后接收者仍可读。"
  "cargo test -j 1 --no-fail-fast = 849 passed / 0 failed（39 个二进制全绿）；"
  "demo_codegen_compile --include-ignored = 2/0（空allowlist，348 DEMO 真编译）；"
  "cargo build 0 error / 0 warning。提交 e5c072d（5 文件按 hunk 分离，RESIDUAL_MARKERS=0）。"
  "仍存的分层缺口（未修、非回归）：调用点把推断出的返回类型视为 Any（fn_returns 只登记"
  "显式注解），故 println(推断返回的 str) 走 Debug 带引号；缺陷 C 修的是函数自身签名。"),
]
for title, deliv in L:
    p = c.call("plan", {"task_id": root, "description": title})
    cid = tid_of(p)
    if isinstance(cid, str) and cid.startswith("["): cid = cid.strip("[]").split(",")[0].strip('"')
    print("  plan:", title[:18], "->", cid)
    if not cid:
        print("    raw:", p[:120]); continue
    for tool, args in (("claim",{"task_id":cid,"assignee":"codebuddy-agent"}),
                       ("execute",{"task_id":cid,"executor":"codebuddy-agent","deliverable":deliv}),
                       ("submit",{"task_id":cid}),
                       ("verify",{"task_id":cid,"verifier":"codebuddy-agent"})):
        print("     ", tool, c.call(tool,args)[:44])
c.call("execute",{"task_id":root,"executor":"codebuddy-agent","deliverable":
  "三处既有缺陷全部修复并随 e5c072d 提交：List 归约走迭代器链、返回类型推断用探测副本、"
  "只读 ref 形参赋值给精确诊断。闸门 32/0；全量 849 passed / 0 failed；DEMO 2/0。"})
c.call("submit",{"task_id":root}); c.call("verify",{"task_id":root,"verifier":"codebuddy-agent"})
print("done:", root); print(c.call("status_summary",{"namespace":"perf-opt"})[-130:])
c.close()
