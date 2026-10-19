import io, re, sys
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

def rw(path, pairs):
    s = io.open(path, encoding="utf-8").read()
    for old, new in pairs:
        assert old in s, (path, old[:60])
        s = s.replace(old, new, 1)
    io.open(path, "w", encoding="utf-8", newline="").write(s)
    print("patched", path)

# 1) mod.rs：模块名与字段
rw("src/ir/codegen/mod.rs", [
    ("mod owned_str;", "mod owned_local;"),
    ("""    owned_str_vars: owned_str::OwnedStrSet,""",
     """    owned_locals: owned_local::OwnedLocals,"""),
    ("""            owned_str_vars: std::cell::RefCell::new(std::collections::HashSet::new()),""",
     """            owned_locals: std::cell::RefCell::new(std::collections::HashMap::new()),"""),
])

# 2) decl_gen.rs：函数入口清空
rw("src/ir/codegen/decl_gen.rs", [("self.owned_str_reset();", "self.owned_local_reset();")])

# 3) stmt_gen.rs：import + 改写块 + 登记块
rw("src/ir/codegen/stmt_gen.rs", [
    ("use super::owned_str;", "use super::owned_local;"),
    ("owned_str::push_str_arg", "owned_local::push_str_arg"),
])
print("step1 ok")
