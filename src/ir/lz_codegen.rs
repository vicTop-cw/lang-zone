// Lang-Zone 编译器 — ir/lz_codegen.rs
// 自举路线 B：IrModule → LZ 构造代码（生成 .lz 源码，编译运行后输出 IR 文本）
//
// 用法：lzc file.lz --emit=ir-lz
//   → 生成 tmp.lzlz = LZ_IR_LIB（LZ 版 IR 类型 + display 函数）+ main 构造代码
//   → lang-zone 编译运行 tmp.lzlz → 输出与 --emit=ir 一致的 IR 文本
//
// 这是「用 LZ 实现 IR display」的落地：Rust 编译器只负责把 IR 数据
// 序列化为 LZ 构造调用，display 逻辑完全由 LZ 侧（lz_ir_lib.lz）承担。
//
// 扁平化策略：每个复杂 Expr/Stmt 拆成顶层 `let __tN = <构造>` 中间变量，
// 避免深层嵌套的 enum 构造调用导致 LZ 编译器（lang-zone）解析栈溢出。

use super::IrModule;
use super::node::*;
use super::types::IrType;

/// 内嵌的 LZ IR 库（类型定义 + display 函数）
pub const LZ_IR_LIB: &str = include_str!("lz_ir_lib.lz");

/// 将 IrModule 序列化为完整 LZ 源码（库 + main 构造 + display_module 调用）
pub fn ir_module_to_lz_source(module: &IrModule) -> String {
    let mut g = LzGen::new();
    let mut out = String::new();
    out.push_str("// 由 lzc --emit=ir-lz 生成（自举路线 B：LZ 实现 IR display）\n");
    out.push_str(LZ_IR_LIB);
    out.push_str("\n\n// ── main：构造当前模块的 IR 并输出 ──\n");
    out.push_str("def main() =\n");

    // 每个 item 拆成顶层 let（中间变量扁平化）
    let mut item_names: Vec<String> = Vec::new();
    for item in &module.items {
        let name = g.fresh();
        let item_src = g.item(item);
        g.lets.push(format!("    let {} = {}", name, item_src));
        item_names.push(name);
    }

    // 组装 IrModule 并输出
    out.push_str(&g.lets.join("\n"));
    out.push('\n');
    out.push_str(&format!(
        "    let m = IrModule(name: \"{}\", items: [{}], prelude: [{}], version: {})\n",
        module.name,
        item_names.join(", "),
        gen_str_list(&module.prelude),
        module.version
    ));
    // print_str（Display 输出，println!("{}")）与 --emit=ir 的 println!("{ir_module}") 一致；
    // print 会被 codegen 特判为 Debug 格式（println!("{:?}")），str 加引号转义 → 不可用
    // 尾部加 return 0：print_str 返回 ()，而 LZ main 尾表达式会被当作返回值（i64）→ E0308
    out.push_str("    print_str(display_module(m))\n    return 0\n");
    out
}

/// 扁平化生成器：产生顶层 let 序列，避免深嵌套构造
struct LzGen {
    counter: usize,
    lets: Vec<String>,
}

impl LzGen {
    fn new() -> Self {
        Self { counter: 0, lets: Vec::new() }
    }

    fn fresh(&mut self) -> String {
        self.counter += 1;
        format!("__t{}", self.counter)
    }

    /// 生成列表引用：空列表 → 带类型标注的中间变量（LZ 空列表 `[]` 无法推断
    /// 元素类型，E0308 found Vec<i64>），非空 → 内联字面量
    /// 注意：items 需先收集完毕（闭包不得捕获 self），避免借用冲突。
    fn list_ref(&mut self, elem_ty: &str, items: Vec<String>) -> String {
        if items.is_empty() {
            let name = self.fresh();
            self.lets.push(format!("    let {}: List<{}> = []", name, elem_ty));
            name
        } else {
            format!("[{}]", items.join(", "))
        }
    }

    /// 收集并生成语句列表引用（先收集避免闭包借用 self）
    /// 每条 Stmt 绑定为中间变量，列表只含引用——避免长列表内联成超长行
    /// 导致 lang-zone 解析/生成栈溢出（自举试点 primitives.lz 3071 字符行触发）
    fn stmts_ref(&mut self, stmts: &[Stmt]) -> String {
        if stmts.is_empty() {
            let name = self.fresh();
            self.lets.push(format!("    let {}: List<Stmt> = []", name));
            return name;
        }
        let names: Vec<String> = stmts
            .iter()
            .map(|s| {
                let src = self.stmt(s);
                self.let_bind(src)
            })
            .collect();
        format!("[{}]", names.join(", "))
    }

    /// 生成 BlockIR 构造：块语句列表 + 块类型（对齐 Rust Block Display 的 [ty] 标注）
    fn block_ref(&mut self, b: &Block) -> String {
        let ss = self.stmts_ref(&b.stmts);
        let ty = self.irtype(&b.ty);
        format!("BlockIR.Block(stmts: {}, ty: {})", ss, ty)
    }

    /// 裸 Option.None 无法推断元素类型（LZ 硬编码 Option::<i64>::None）；
    /// 改用 lz_ir_lib.lz 的自有 Maybe* 单位变体枚举构造。
    fn opt_none(&mut self, inner_ty: &str) -> String {
        match inner_ty {
            "Expr" => "MaybeExpr.NoExpr".to_string(),
            "BlockIR" => "MaybeBlock.NoBlock".to_string(),
            "str" => "MaybeStr.NoStr".to_string(),
            "IrType" => "MaybeIrType.NoTy".to_string(),
            "Pattern" => "MaybePattern.NoPat".to_string(),
            other => {
                let name = self.fresh();
                self.lets.push(format!("    let {}: Option<{}> = Option.None", name, other));
                name
            }
        }
    }

    /// MaybeStr 构造（str 需转义）
    fn opt_str(&mut self, s: &Option<String>) -> String {
        match s {
            Some(v) => format!("MaybeStr.YesStr(value: \"{}\")", escape_lz(v)),
            None => self.opt_none("str"),
        }
    }

    /// 收集并生成表达式列表引用
    fn exprs_ref(&mut self, exprs: &[Expr]) -> String {
        let items: Vec<String> = exprs.iter().map(|e| self.expr(e)).collect();
        self.list_ref("Expr", items)
    }

    /// 生成一条顶层 let（构造表达式 + 变量名）
    fn let_bind(&mut self, expr: String) -> String {
        let name = self.fresh();
        self.lets.push(format!("    let {} = {}", name, expr));
        name
    }

    // ── Item → 构造表达式（子节点走 self.expr/stmt，扁平化）──

    fn item(&mut self, item: &Item) -> String {
        match item {
            Item::FnDef(f) => {
                let gen_items: Vec<String> = f
                    .generics
                    .iter()
                    .map(|g| format!("\"{}\"", escape_lz(&g.name)))
                    .collect();
                let generics = self.list_ref("str", gen_items);
                let params = self.param_pairs(&f.params);
                let body = self.stmts_ref(&f.body.stmts);
                format!(
                    "Item.FnDef(name: \"{}\", generics: {}, params: {}, ret: {}, body: {})",
                    f.name,
                    generics,
                    params,
                    self.irtype(&f.ret_ty),
                    body
                )
            }
            Item::Const(c) => format!(
                "Item.Const(name: \"{}\", ty: {}, value: {})",
                c.name,
                self.irtype(&c.ty),
                self.expr(&c.value)
            ),
            Item::StructDef(s) => {
                let items: Vec<String> = s
                    .fields
                    .iter()
                    .map(|fl| format!("(\"{}\", {})", fl.name, self.irtype(&fl.ty)))
                    .collect();
                let fields = self.list_ref("(str, IrType)", items);
                let generics: Vec<String> = s.generics.iter().map(|g| format!("\"{}\"", g.name)).collect();
                let generics_s = self.list_ref("str", generics);
                format!(
                    "Item.StructDef(name: \"{}\", generics: {}, fields: {})",
                    s.name, generics_s, fields
                )
            }
            Item::EnumDef(e) => {
                let variants: Vec<String> = e
                    .variants
                    .iter()
                    .map(|v| {
                        let tys: Vec<String> = v.fields.iter().map(|fl| self.irtype(&fl.ty)).collect();
                        let tys = self.list_ref("IrType", tys);
                        format!("(\"{}\", {})", v.name, tys)
                    })
                    .collect();
                let variants = self.list_ref("(str, List<IrType>)", variants);
                let generics: Vec<String> = e.generics.iter().map(|g| format!("\"{}\"", g.name)).collect();
                let generics_s = self.list_ref("str", generics);
                format!("Item.EnumDef(name: \"{}\", generics: {}, variants: {})", e.name, generics_s, variants)
            }
            Item::TraitDef(t) => {
                let supers_items: Vec<String> = t.supertraits.iter().map(|st| self.irtype(st)).collect();
                let supers = self.list_ref("IrType", supers_items);
                let methods_items: Vec<String> = t
                    .methods
                    .iter()
                    .map(|m| {
                        let params: Vec<String> = m.params.iter().map(|p| self.irtype(p)).collect();
                        let params = self.list_ref("IrType", params);
                        format!("(\"{}\", {}, {})", m.name, params, self.irtype(&m.ret))
                    })
                    .collect();
                let methods = self.list_ref("(str, List<IrType>, IrType)", methods_items);
                format!(
                    "Item.TraitDef(name: \"{}\", supertraits: {}, methods: {})",
                    t.name,
                    supers,
                    methods
                )
            }
            Item::DuckDef(d) => {
                format!("Item.DuckDef(name: \"{}\", method_count: {})", d.name, d.methods.len())
            }
            Item::Use(u) => {
                let path = u.path.join(".");
                let items = u.items.join(", ");
                if u.is_from {
                    format!("Item.UseStmt(path: [\"{}\"], alias: {}, items: [\"{}\"], is_from: true)", path, self.opt_none("str"), items)
                } else {
                    format!("Item.UseStmt(path: [\"{}\"], alias: {}, items: [], is_from: false)", path, self.opt_none("str"))
                }
            }
            Item::Test(t) => {
                let body = self.stmts_ref(&t.body.stmts);
                format!("Item.Test(name: \"{}\", body: {})", t.name, body)
            }
            Item::CheckerBlock { name, ps_name, .. } => format!(
                "Item.CheckerBlock(name: \"{}\", ps_name: {})",
                name,
                self.opt_str(ps_name)
            ),
            Item::TypeAlias(ta) => {
                format!("Item.TypeAlias(name: \"{}\", ty: {})", ta.name, self.irtype(&ta.ty))
            }
            Item::Impl(imp) => {
                let trait_s = match &imp.trait_ {
                    Some(t) => format!("MaybeIrType.YesTy(value: {})", self.irtype(t)),
                    None => self.opt_none("IrType"),
                };
                let names_items: Vec<String> = imp
                    .methods
                    .iter()
                    .map(|m| format!("\"{}\"", escape_lz(&m.name)))
                    .collect();
                let names_s = self.list_ref("str", names_items);
                format!(
                    "Item.Impl(trait_: {}, for_type: {}, method_names: {})",
                    trait_s,
                    self.irtype(&imp.for_type),
                    names_s
                )
            }
        }
    }

    // ── Stmt → 构造表达式（子节点走 self.expr，扁平化）──

    fn stmt(&mut self, s: &Stmt) -> String {
        match s {
            Stmt::Let { name, ty, value, is_mut, is_ref, .. } => format!(
                "Stmt.Let(name: \"{}\", ty: {}, value: {}, is_mut: {}, is_ref: {})",
                name,
                self.irtype(ty),
                self.expr(value),
                if *is_mut { "true" } else { "false" },
                if *is_ref { "true" } else { "false" }
            ),
            Stmt::Assign { target, value } => format!(
                "Stmt.Assign(target: {}, value: {})",
                self.expr(target),
                self.expr(value)
            ),
            Stmt::Return { value } => match value {
                Some(v) => format!("Stmt.Return(value: MaybeExpr.YesExpr(value: {}))", self.expr(v)),
                None => format!("Stmt.Return(value: {})", self.opt_none("Expr")),
            },
            Stmt::ExprStmt { expr } => format!("Stmt.ExprStmt(expr: {})", self.expr(expr)),
            Stmt::If { cond, then_branch, else_branch } => {
                let then_s = self.block_ref(then_branch);
                let els_s = match else_branch {
                    Some(b) => {
                        let es = self.block_ref(b);
                        format!("MaybeBlock.YesBlock(value: {})", es)
                    }
                    None => self.opt_none("BlockIR"),
                };
                format!(
                    "Stmt.If(cond: {}, then: {}, els: {})",
                    self.expr(cond),
                    then_s,
                    els_s
                )
            }
            Stmt::For { var, iter, guard, body, else_body, .. } => {
                let body_s = self.block_ref(body);
                let guard_s = match guard {
                    Some(g) => format!("MaybeExpr.YesExpr(value: {})", self.expr(g)),
                    None => self.opt_none("Expr"),
                };
                let else_s = match else_body {
                    Some(eb) => {
                        let es = self.block_ref(eb);
                        format!("MaybeBlock.YesBlock(value: {})", es)
                    }
                    None => self.opt_none("BlockIR"),
                };
                format!(
                    "Stmt.For(var: \"{}\", iter: {}, guard_e: {}, body: {}, else_body: {})",
                    var,
                    self.expr(iter),
                    guard_s,
                    body_s,
                    else_s
                )
            }
            Stmt::While { cond, guard, body, else_body, .. } => {
                let body_s = self.block_ref(body);
                let guard_s = match guard {
                    Some(g) => format!("MaybeExpr.YesExpr(value: {})", self.expr(g)),
                    None => self.opt_none("Expr"),
                };
                let else_s = match else_body {
                    Some(eb) => {
                        let es = self.block_ref(eb);
                        format!("MaybeBlock.YesBlock(value: {})", es)
                    }
                    None => self.opt_none("BlockIR"),
                };
                format!(
                    "Stmt.While(cond: {}, guard_e: {}, body: {}, else_body: {})",
                    self.expr(cond),
                    guard_s,
                    body_s,
                    else_s
                )
            }
            Stmt::Block { stmts } => {
                let ss = self.stmts_ref(stmts);
                format!("Stmt.Block(stmts: {})", ss)
            }
            Stmt::Match { scrutinee, arms } => {
                let arms_items: Vec<String> = arms
                    .iter()
                    .map(|a| {
                        let body = self.block_ref(&a.body);
                        let guard = match &a.guard {
                            Some(g) => format!("MaybeExpr.YesExpr(value: {})", self.expr(g)),
                            None => self.opt_none("Expr"),
                        };
                        format!("({}, {}, {})", self.pattern(&a.pattern), guard, body)
                    })
                    .collect();
                let arms_s = self.list_ref("(Pattern, MaybeExpr, BlockIR)", arms_items);
                format!(
                    "Stmt.Match(scrutinee: {}, arms: {})",
                    self.expr(scrutinee),
                    arms_s
                )
            }
            Stmt::Yield { value } => format!("Stmt.Yield(value: {})", self.expr(value)),
            Stmt::YieldFrom { iter } => format!("Stmt.YieldFrom(iter: {})", self.expr(iter)),
            Stmt::Break => "Stmt.Break".to_string(),
            Stmt::BreakLabel { label, value } => {
                let v = match value {
                    Some(v) => format!("MaybeExpr.YesExpr(value: {})", self.expr(v)),
                    None => self.opt_none("Expr"),
                };
                format!("Stmt.BreakLabel(label: \"{}\", value: {})", label, v)
            }
            Stmt::Continue => "Stmt.Continue".to_string(),
            Stmt::BlockLabel { label, body } => {
                let ss = self.block_ref(body);
                format!("Stmt.BlockLabel(label: \"{}\", body: {})", label, ss)
            }
            Stmt::CheckerBlock { label, ps_name, .. } => format!(
                "Stmt.CheckerBlock(label: \"{}\", ps_name: {})",
                label,
                self.opt_str(ps_name)
            ),
            Stmt::Defer { body } => {
                let ss = self.block_ref(body);
                format!("Stmt.Defer(body: {})", ss)
            }
            Stmt::Raise { value } => format!("Stmt.Raise(value: {})", self.expr(value)),
            Stmt::Assert { cond, .. } => format!("Stmt.Assert(cond: {})", self.expr(cond)),
            Stmt::Pass => "Stmt.Pass".to_string(),
            Stmt::TypeAlias { name, ty } => {
                format!("Stmt.TypeAlias(name: \"{}\", ty: {})", name, self.irtype(ty))
            }
            Stmt::TryCatch { body, catches, else_body, finally_body } => {
                let ss = self.block_ref(body);
                let catches_items: Vec<String> = catches
                    .iter()
                    .map(|(pat, block)| {
                        let b = self.block_ref(block);
                        let p = match pat {
                            Some(p) => format!("MaybePattern.YesPat(value: {})", self.pattern(p)),
                            None => self.opt_none("Pattern"),
                        };
                        format!("({}, {})", p, b)
                    })
                    .collect();
                let catches_s = self.list_ref("(MaybePattern, BlockIR)", catches_items);
                let else_s = match else_body {
                    Some(eb) => {
                        let es = self.block_ref(eb);
                        format!("MaybeBlock.YesBlock(value: {})", es)
                    }
                    None => self.opt_none("BlockIR"),
                };
                let finally_s = match finally_body {
                    Some(fb) => {
                        let fs = self.block_ref(fb);
                        format!("MaybeBlock.YesBlock(value: {})", fs)
                    }
                    None => self.opt_none("BlockIR"),
                };
                format!(
                    "Stmt.TryCatch(body: {}, catches: {}, else_body: {}, finally_body: {})",
                    ss,
                    catches_s,
                    else_s,
                    finally_s
                )
            }
            Stmt::WhileLet { pattern, expr, guard, body, .. } => {
                let guard_s = match guard {
                    Some(g) => format!("MaybeExpr.YesExpr(value: {})", self.expr(g)),
                    None => self.opt_none("Expr"),
                };
                format!(
                    "Stmt.WhileLet(pattern: {}, expr: {}, guard_e: {}, body: {})",
                    self.pattern(pattern),
                    self.expr(expr),
                    guard_s,
                    self.block_ref(body)
                )
            }
        }
    }

    // ── Expr → 构造表达式（复杂节点拆中间变量，扁平化）──

    fn expr(&mut self, e: &Expr) -> String {
        let ty = self.irtype(&e.ty);
        let src = match &e.kind {
            ExprKind::Lit(lit) => gen_lit(lit, &ty),
            ExprKind::Var(name) => format!("Expr.Var(name: \"{}\", ty: {})", name, ty),
            ExprKind::Call { callee, args, .. } => {
                let args_s = self.exprs_ref(args);
                format!(
                    "Expr.Call(callee: {}, args: {}, ty: {})",
                    self.expr(callee),
                    args_s,
                    ty
                )
            }
            ExprKind::MethodCall { receiver, method, args, .. } => {
                let args_s = self.exprs_ref(args);
                format!(
                    "Expr.MethodCall(receiver: {}, method: \"{}\", args: {}, ty: {})",
                    self.expr(receiver),
                    method,
                    args_s,
                    ty
                )
            }
            ExprKind::FieldAccess { base, field } => format!(
                "Expr.FieldAccess(base: {}, field: \"{}\", ty: {})",
                self.expr(base),
                field,
                ty
            ),
            ExprKind::IndexGet { base, key } => format!(
                "Expr.IndexGet(base: {}, key: {}, ty: {})",
                self.expr(base),
                self.expr(key),
                ty
            ),
            ExprKind::IndexSet { base, key, value } => format!(
                "Expr.IndexSet(base: {}, key: {}, value: {}, ty: {})",
                self.expr(base),
                self.expr(key),
                self.expr(value),
                ty
            ),
            ExprKind::BinOp { op, lhs, rhs } => format!(
                "Expr.BinOp(op: \"{}\", lhs: {}, rhs: {}, ty: {})",
                binop_str(op),
                self.expr(lhs),
                self.expr(rhs),
                ty
            ),
            ExprKind::UnOp { op, operand } => format!(
                "Expr.UnOp(op: \"{}\", operand: {}, ty: {})",
                unop_str(op),
                self.expr(operand),
                ty
            ),
            ExprKind::StructCtor { name, fields } => {
                let fields_items: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("(\"{}\", {})", k, self.expr(v)))
                    .collect();
                let fields_s = self.list_ref("(str, Expr)", fields_items);
                format!("Expr.StructCtor(name: \"{}\", fields: {}, ty: {})", name, fields_s, ty)
            }
            ExprKind::EnumCtor { enum_name, variant, args } => {
                let args_s = self.exprs_ref(args);
                format!(
                    "Expr.EnumCtor(enum_name: \"{}\", variant: \"{}\", args: {}, ty: {})",
                    enum_name,
                    variant,
                    args_s,
                    ty
                )
            }
            ExprKind::Cast { expr, target } => format!(
                "Expr.Cast(inner: {}, target: {}, ty: {})",
                self.expr(expr),
                self.irtype(target),
                ty
            ),
            ExprKind::MagicCall { kind, args } => {
                let args_s = self.exprs_ref(args);
                format!(
                    "Expr.MagicCall(magic: \"{}\", args: {}, ty: {})",
                    magic_str(kind),
                    args_s,
                    ty
                )
            }
            ExprKind::IfExpr { cond, then, els } => format!(
                "Expr.IfExpr(cond: {}, then: {}, els: {}, ty: {})",
                self.expr(cond),
                self.expr(then),
                self.expr(els),
                ty
            ),
            ExprKind::Pipe { receiver, callee, args } => {
                let args_s = self.exprs_ref(args);
                format!(
                    "Expr.Pipe(receiver: {}, callee: {}, args: {}, ty: {})",
                    self.expr(receiver),
                    self.expr(callee),
                    args_s,
                    ty
                )
            }
            ExprKind::TupleLit(elems) | ExprKind::Tuple(elems) => {
                let elems_s = self.exprs_ref(elems);
                format!("Expr.TupleLit(elems: {}, ty: {})", elems_s, ty)
            }
            ExprKind::ListLit(elems) | ExprKind::List(elems) => {
                let elems_s = self.exprs_ref(elems);
                format!("Expr.ListLit(items: {}, ty: {})", elems_s, ty)
            }
            ExprKind::Dict(pairs) => {
                let elems_items: Vec<String> = pairs
                    .iter()
                    .map(|(k, v)| {
                        format!("({}, {})", self.expr(k), self.expr(v))
                    })
                    .collect();
                let elems_s = self.list_ref("(Expr, Expr)", elems_items);
                format!("Expr.Dict(pairs: {}, ty: {})", elems_s, ty)
            }
            ExprKind::Range { start, end, inclusive } => {
                // 注意：LZ 版 Range 不携带 start（display.rs 对 Range 落 `_ => "<expr>"`，
                // start 不参与显示；同时避免 Expr ↔ MaybeExpr 直接互递归 E0072）
                let _ = start;
                format!(
                    "Expr.Range(end: {}, inclusive: {}, ty: {})",
                    self.expr(end),
                    inclusive,
                    ty
                )
            }
            ExprKind::Paren(inner) => format!("Expr.Paren(inner: {}, ty: {})", self.expr(inner), ty),
            ExprKind::BlockExpr { block } => {
                let stmts = self.stmts_ref(&block.stmts);
                format!("Expr.BlockExpr(stmts: {}, ty: {})", stmts, ty)
            }
            ExprKind::GenExpr { yield_of } => {
                format!(
                    "Expr.GenExpr(yield_of: {}, ty: {})",
                    self.expr(yield_of),
                    ty
                )
            }
            ExprKind::AssignExpr { target, value } => format!(
                "Expr.AssignExpr(target: {}, value: {}, ty: {})",
                self.expr(target),
                self.expr(value),
                ty
            ),
            ExprKind::ImplicitConvert { source, target_ty } => format!(
                "Expr.ImplicitConvert(source: {}, target_ty: {}, ty: {})",
                self.expr(source),
                self.irtype(target_ty),
                ty
            ),
            ExprKind::Lambda { params, body, .. } => {
                let param_items: Vec<String> = params
                    .iter()
                    .map(|p| format!("\"{}\"", escape_lz(&p.name)))
                    .collect();
                let params_s = self.list_ref("str", param_items);
                format!(
                    "Expr.Lambda(params: {}, body: {}, ty: {})",
                    params_s,
                    self.expr(body),
                    ty
                )
            }
        };
        // 简单节点（字面量/变量）直接内联；含子节点的复杂构造拆中间变量
        if is_simple_expr(&e.kind) {
            src
        } else {
            self.let_bind(src)
        }
    }

    fn irtype(&mut self, t: &IrType) -> String {
        match t {
            IrType::Int => "IrType.Int".to_string(),
            IrType::F64 => "IrType.F64".to_string(),
            IrType::Str => "IrType.Str".to_string(),
            IrType::Bool => "IrType.Bool".to_string(),
            IrType::Unit => "IrType.Unit".to_string(),
            IrType::Never => "IrType.Never".to_string(),
            IrType::Any => "IrType.Any".to_string(),
            IrType::Self_ => "IrType.Self_".to_string(),
            IrType::Named { path, args } => {
                let args_items: Vec<String> = args.iter().map(|a| self.irtype(a)).collect();
                let args_s = self.list_ref("IrType", args_items);
                format!("IrType.Named(path: \"{}\", args: {})", escape_lz(path), args_s)
            }
            IrType::Option(inner) => format!("IrType.Opt(inner: {})", self.irtype(inner)),
            IrType::Result { ok, err } => format!(
                "IrType.Res(ok: {}, err: {})",
                self.irtype(ok),
                self.irtype(err)
            ),
            IrType::Tuple(elems) => {
                let elems_items: Vec<String> = elems.iter().map(|e| self.irtype(e)).collect();
                let elems_s = self.list_ref("IrType", elems_items);
                format!("IrType.Tuple(elems: {})", elems_s)
            }
            IrType::Fn { params, ret } => {
                let params_items: Vec<String> = params.iter().map(|p| self.irtype(p)).collect();
                let params_s = self.list_ref("IrType", params_items);
                format!("IrType.FnType(params: {}, ret: {})", params_s, self.irtype(ret))
            }
            IrType::Ref(inner) => format!("IrType.Ref(inner: {})", self.irtype(inner)),
            IrType::MutRef(inner) => format!("IrType.MutRef(inner: {})", self.irtype(inner)),
            IrType::Duck { fields } => {
                let fields_items: Vec<String> = fields
                    .iter()
                    .map(|(n, ty)| format!("(\"{}\", {})", escape_lz(n), self.irtype(ty)))
                    .collect();
                let fields_s = self.list_ref("(str, IrType)", fields_items);
                format!("IrType.Duck(fields: {})", fields_s)
            }
            IrType::Generic(name) => format!("IrType.Generic(name: \"{}\")", escape_lz(name)),
        }
    }

    fn param_pairs(&mut self, params: &[Param]) -> String {
        let items: Vec<String> = params
            .iter()
            .map(|p| {
                format!(
                    "(\"{}\", {}, {}, {}, {})",
                    p.name,
                    self.irtype(&p.ty),
                    if p.is_ref { "true" } else { "false" },
                    if p.is_mut { "true" } else { "false" },
                    if p.is_owned { "true" } else { "false" }
                )
            })
            .collect();
        self.list_ref("(str, IrType, bool, bool, bool)", items)
    }

    /// Pattern → 构造表达式（列表走 self.list_ref，空列表类型标注避免 Vec<i64> 推断）
    fn pattern(&mut self, p: &Pattern) -> String {
        match p {
            Pattern::Wildcard => "Pattern.Wildcard".to_string(),
            Pattern::Ident(name) => format!("Pattern.Ident(name: \"{}\")", name),
            Pattern::RefMutIdent(name) => format!("Pattern.RefMutIdent(name: \"{}\")", name),
            Pattern::Lit(LitKind::Int(n)) => format!("Pattern.LitInt(v: {})", n),
            Pattern::Lit(LitKind::Str(s)) => format!("Pattern.LitStr(s: \"{}\")", escape_lz(s)),
            Pattern::Lit(LitKind::Bool(b)) => format!("Pattern.LitBool(b: {})", b),
            Pattern::Lit(LitKind::F64(n)) => format!("Pattern.LitF64(v: {})", n),
            Pattern::Lit(_) => "Pattern.Wildcard".to_string(),
            Pattern::Tuple(elems) => {
                let elems_items: Vec<String> = elems.iter().map(|e| self.pattern(e)).collect();
                let elems_s = self.list_ref("Pattern", elems_items);
                format!("Pattern.Tuple(elems: {})", elems_s)
            }
            Pattern::List(elems) => {
                let elems_items: Vec<String> = elems.iter().map(|e| self.pattern(e)).collect();
                let elems_s = self.list_ref("Pattern", elems_items);
                format!("Pattern.List(elems: {})", elems_s)
            }
            Pattern::Dict(_) => "Pattern.Wildcard".to_string(),
            Pattern::Rest(name) => match name {
                Some(n) => format!("Pattern.Rest(name: MaybeStr.YesStr(value: \"{}\"))", escape_lz(n)),
                None => format!("Pattern.Rest(name: {})", self.opt_none("str")),
            },
            Pattern::Range { start, end, inclusive } => {
                format!("Pattern.Range(start: {}, end: {}, inclusive: {})", start, end, inclusive)
            }
            Pattern::Struct { name, fields } => {
                let fields_items: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("(\"{}\", {})", k, self.pattern(v)))
                    .collect();
                let fields_s = self.list_ref("(str, Pattern)", fields_items);
                format!("Pattern.Struct(name: \"{}\", fields: {})", name, fields_s)
            }
            Pattern::Enum { enum_name, variant, args } => {
                let args_items: Vec<String> = args.iter().map(|a| self.pattern(a)).collect();
                let args_s = self.list_ref("Pattern", args_items);
                format!(
                    "Pattern.Enum(enum_name: \"{}\", variant: \"{}\", args: {})",
                    enum_name,
                    variant,
                    args_s
                )
            }
        }
    }
}

/// 简单表达式（字面量/变量）可内联，避免不必要的中间变量
fn is_simple_expr(kind: &ExprKind) -> bool {
    matches!(kind, ExprKind::Lit(_) | ExprKind::Var(_))
}

fn gen_lit(lit: &LitKind, ty: &str) -> String {
    match lit {
        LitKind::Int(n) => format!("Expr.LitInt(v: {}, ty: {})", n, ty),
        // LZ 中 `3` 是 int（3_i64），f64 字面量需带小数点（3.0）
        LitKind::F64(n) => {
            let s = format!("{}", n);
            let s = if s.contains('.') || s.contains('e') || s.contains('E') {
                s
            } else {
                format!("{}.0", s)
            };
            format!("Expr.LitF64(v: {}, ty: {})", s, ty)
        }
        LitKind::Str(s) => format!("Expr.LitStr(s: \"{}\", ty: {})", escape_lz(s), ty),
        LitKind::FStr(s) => format!("Expr.LitFStr(s: \"{}\", ty: {})", escape_lz(s), ty),
        LitKind::Bool(b) => format!("Expr.LitBool(b: {}, ty: {})", b, ty),
        LitKind::Unit => format!("Expr.LitUnit(ty: {})", ty),
        LitKind::None_ => format!("Expr.LitNone(ty: {})", ty),
    }
}

fn gen_str_list(xs: &[String]) -> String {
    xs.iter().map(|s| format!("\"{}\"", escape_lz(s))).collect::<Vec<_>>().join(", ")
}

/// 转义 LZ 字符串字面量中的引号
fn escape_lz(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn binop_str(op: &super::node::BinOpKind) -> &'static str {
    use super::node::BinOpKind::*;
    match op {
        Add => "+", Sub => "-", Mul => "*", Div => "/", Mod => "%", Pow => "**",
        Eq => "==", Neq => "!=", Lt => "<", Gt => ">", Le => "<=", Ge => ">=",
        And => "&&", Or => "||", BitAnd => "&", BitOr => "|", Xor => "^",
        Shl => "<<", Shr => ">>", In => "in", NotIn => "not in",
    }
}

fn unop_str(op: &super::node::UnOpKind) -> &'static str {
    use super::node::UnOpKind::*;
    match op {
        Neg => "-", Not => "!", Ref => "&", MutRef => "&mut", Deref => "*",
    }
}

fn magic_str(kind: &super::node::MagicKind) -> &'static str {
    use super::node::MagicKind::*;
    match kind {
        GetItem => "__getitem__", SetItem => "__setitem__", Call => "__call__",
        Iter => "__iter__", Next => "__next__", Display => "__str__", Eq => "__eq__",
        Cmp => "__cmp__", Drop => "__drop__", Rev => "__rev__", Len => "__len__",
        Add => "__add__", Sub => "__sub__", Mul => "__mul__", Neg => "__neg__",
        Not_ => "__not__", IntoIter => "__into_iter__", SizeHint => "__size_hint__",
        IterStrategy => "__iter_strategy__", UnpackBuildCall => "unpack_build_call",
    }
}
