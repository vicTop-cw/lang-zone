// Lang-Zone 编译器 — ir/display.rs
// LZIR 文本输出（树形格式，用于 --emit=ir 和快照测试）

use super::node::*;
use super::types::IrType;
use std::fmt::{self};

// ── IrType Display ──

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IrType::Int => f.write_str("int"),
            IrType::F64 => f.write_str("f64"),
            IrType::Str => f.write_str("str"),
            IrType::Bool => f.write_str("bool"),
            IrType::Unit => f.write_str("()"),
            IrType::Never => f.write_str("!"),
            IrType::Any => f.write_str("?"),
            IrType::Self_ => f.write_str("Self"),
            IrType::Duck { fields } => {
                f.write_str("duck {")?;
                for (i, (n, t)) in fields.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{n}: {t}")?;
                }
                f.write_str("}")
            }
            IrType::Named { path, args } => {
                f.write_str(path)?;
                if !args.is_empty() {
                    f.write_str("<")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(">")?;
                }
                Ok(())
            }
            IrType::Option(inner) => write!(f, "Option<{inner}>"),
            IrType::Result { ok, err } => write!(f, "Result<{ok}, {err}>"),
            IrType::Tuple(elems) => {
                f.write_str("(")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{e}")?;
                }
                f.write_str(")")
            }
            IrType::Fn { params, ret } => {
                f.write_str("fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
            IrType::Ref(inner) => write!(f, "&{inner}"),
            IrType::MutRef(inner) => write!(f, "&mut {inner}"),
            IrType::Generic(name) => f.write_str(name),
        }
    }
}

// ── Span Display ──

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

// ── Expr Display ──

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}] ", self.ty)?;
        match &self.kind {
            ExprKind::Lit(lit) => write!(f, "{lit}"),
            ExprKind::Var(name) => f.write_str(name),
            ExprKind::Call { callee, args, .. } => {
                write!(f, "call {callee}")?;
                if !args.is_empty() {
                    f.write_str("(")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(")")?;
                }
                Ok(())
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                write!(f, "method {receiver}.{method}")?;
                if !args.is_empty() {
                    f.write_str("(")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(")")?;
                }
                Ok(())
            }
            ExprKind::FieldAccess { base, field } => write!(f, "field {base}.{field}"),
            ExprKind::IndexGet { base, key } => write!(f, "index {base}[{key}]"),
            ExprKind::IndexSet { base, key, value } => {
                write!(f, "index_set {base}[{key}] = {value}")
            }
            ExprKind::BinOp { op, lhs, rhs } => write!(f, "binop {lhs} {op} {rhs}"),
            ExprKind::UnOp { op, operand } => write!(f, "unop {op} {operand}"),
            ExprKind::IfExpr { cond, then, els } => write!(f, "if {cond} then {then} else {els}"),
            ExprKind::Lambda { params, body, .. } => {
                f.write_str("|")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(&p.name)?;
                }
                write!(f, "| {body}")
            }
            ExprKind::StructCtor { name, fields } => {
                write!(f, "new {name}")?;
                if !fields.is_empty() {
                    f.write_str("{ ")?;
                    for (i, (k, v)) in fields.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{k}: {v}")?;
                    }
                    f.write_str(" }")?;
                }
                Ok(())
            }
            ExprKind::EnumCtor {
                enum_name,
                variant,
                args,
            } => {
                write!(f, "{enum_name}::{variant}")?;
                if !args.is_empty() {
                    f.write_str("(")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(")")?;
                }
                Ok(())
            }
            ExprKind::GenExpr { yield_of } => write!(f, "gen {yield_of}"),
            ExprKind::Cast { expr, target } => write!(f, "cast {expr} as {target}"),
            ExprKind::MagicCall { kind, args } => {
                write!(f, "magic {kind}")?;
                if !args.is_empty() {
                    f.write_str("(")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(")")?;
                }
                Ok(())
            }
            ExprKind::BlockExpr { block } => write!(f, "block {block}"),
            ExprKind::Paren(inner) => write!(f, "({inner})"),
            ExprKind::Pipe {
                receiver,
                callee,
                args,
            } => {
                write!(f, "pipe {receiver} |> {callee}")?;
                if !args.is_empty() {
                    f.write_str("(")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(")")?;
                }
                Ok(())
            }
            ExprKind::TupleLit(elems) => {
                f.write_str("(")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{e}")?;
                }
                f.write_str(")")
            }
            ExprKind::ListLit(items) => {
                f.write_str("[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            _ => f.write_str("<expr>"),
        }
    }
}

// ── LitKind Display ──

impl fmt::Display for LitKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LitKind::Int(n) => write!(f, "{n}_i64"),
            LitKind::F64(n) => write!(f, "{n}_f64"),
            LitKind::Str(s) => write!(f, "\"{s}\""),
            LitKind::FStr(s) => write!(f, "f\"{s}\""),
            LitKind::Bool(b) => write!(f, "{b}"),
            LitKind::Unit => f.write_str("()"),
            LitKind::None_ => f.write_str("None"),
        }
    }
}

// ── BinOpKind Display ──

impl fmt::Display for BinOpKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BinOpKind::Add => f.write_str("+"),
            BinOpKind::Sub => f.write_str("-"),
            BinOpKind::Mul => f.write_str("*"),
            BinOpKind::Div => f.write_str("/"),
            BinOpKind::Mod => f.write_str("%"),
            BinOpKind::Eq => f.write_str("=="),
            BinOpKind::Neq => f.write_str("!="),
            BinOpKind::Lt => f.write_str("<"),
            BinOpKind::Gt => f.write_str(">"),
            BinOpKind::Le => f.write_str("<="),
            BinOpKind::Ge => f.write_str(">="),
            BinOpKind::And => f.write_str("&&"),
            BinOpKind::Or => f.write_str("||"),
            BinOpKind::BitAnd => f.write_str("&"),
            BinOpKind::BitOr => f.write_str("|"),
            BinOpKind::Xor => f.write_str("^"),
            BinOpKind::Shl => f.write_str("<<"),
            BinOpKind::Shr => f.write_str(">>"),
            BinOpKind::Pow => f.write_str("**"),
            BinOpKind::In => f.write_str("in"),
            BinOpKind::NotIn => f.write_str("not in"),
        }
    }
}

// ── UnOpKind Display ──

impl fmt::Display for UnOpKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UnOpKind::Neg => f.write_str("-"),
            UnOpKind::Not => f.write_str("!"),
            UnOpKind::Ref => f.write_str("&"),
            UnOpKind::MutRef => f.write_str("&mut"),
            UnOpKind::Deref => f.write_str("*"),
        }
    }
}

// ── MagicKind Display ──

impl fmt::Display for MagicKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MagicKind::GetItem => f.write_str("__getitem__"),
            MagicKind::SetItem => f.write_str("__setitem__"),
            MagicKind::Call => f.write_str("__call__"),
            MagicKind::Iter => f.write_str("__iter__"),
            MagicKind::Next => f.write_str("__next__"),
            MagicKind::Display => f.write_str("__str__"),
            MagicKind::Eq => f.write_str("__eq__"),
            MagicKind::Cmp => f.write_str("__cmp__"),
            MagicKind::Drop => f.write_str("__drop__"),
            MagicKind::Rev => f.write_str("__rev__"),
            MagicKind::Len => f.write_str("__len__"),
            MagicKind::Add => f.write_str("__add__"),
            MagicKind::Sub => f.write_str("__sub__"),
            MagicKind::Mul => f.write_str("__mul__"),
            MagicKind::Neg => f.write_str("__neg__"),
            MagicKind::Not_ => f.write_str("__not__"),
            MagicKind::IntoIter => f.write_str("__into_iter__"),
            MagicKind::SizeHint => f.write_str("__size_hint__"),
            MagicKind::IterStrategy => f.write_str("__iter_strategy__"),
            MagicKind::UnpackBuildCall => f.write_str("unpack_build_call"),
        }
    }
}

// ── Pattern Display ──

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Pattern::Wildcard => f.write_str("_"),
            Pattern::Ident(name) => f.write_str(name),
            Pattern::RefMutIdent(name) => write!(f, "ref mut {name}"),
            Pattern::Lit(lit) => write!(f, "{lit}"),
            Pattern::Tuple(elems) => {
                f.write_str("(")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{e}")?;
                }
                f.write_str(")")
            }
            Pattern::Struct { name, fields } => {
                write!(f, "{name}")?;
                if !fields.is_empty() {
                    f.write_str("{ ")?;
                    for (i, (k, v)) in fields.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{k}: {v}")?;
                    }
                    f.write_str(" }")?;
                }
                Ok(())
            }
            Pattern::Enum {
                enum_name,
                variant,
                args,
            } => {
                write!(f, "{enum_name}::{variant}")?;
                if !args.is_empty() {
                    f.write_str("(")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(")")?;
                }
                Ok(())
            }
            Pattern::List(elems) => {
                f.write_str("[")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{e}")?;
                }
                f.write_str("]")
            }
            Pattern::Dict(entries) => {
                f.write_str("{")?;
                for (i, (k, p)) in entries.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "\"{k}\": {p}")?;
                }
                f.write_str("}")
            }
            Pattern::Rest(name) => match name {
                Some(n) => write!(f, "..{n}"),
                None => f.write_str(".."),
            },
            Pattern::Range {
                start,
                end,
                inclusive,
            } => {
                if *inclusive {
                    write!(f, "{start}..={end}")
                } else {
                    write!(f, "{start}..{end}")
                }
            }
        }
    }
}

// ── Block Display ──

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.stmts.is_empty() {
            return write!(f, "{{ }} [{}]", self.ty);
        }
        writeln!(f, "{{ [{}]", self.ty)?;
        for stmt in &self.stmts {
            writeln!(f, "  {stmt}")?;
        }
        f.write_str("}")
    }
}

// ── Stmt Display ──

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Stmt::Let {
                name,
                ty,
                value,
                is_mut,
            } => {
                let kw = if *is_mut { "let mut" } else { "let" };
                write!(f, "{kw} {name}: {ty} = {value}")
            }
            Stmt::Assign { target, value } => write!(f, "{target} = {value}"),
            Stmt::Return { value } => match value {
                Some(v) => write!(f, "return {v}"),
                None => f.write_str("return"),
            },
            Stmt::ExprStmt { expr } => write!(f, "{expr}"),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                write!(f, "if {cond} {then_branch}")?;
                if let Some(els) = else_branch {
                    write!(f, " else {els}")?;
                }
                Ok(())
            }
            Stmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
            } => {
                let guard_s = guard
                    .as_ref()
                    .map(|g| format!(" if {g}"))
                    .unwrap_or_default();
                let else_s = else_body
                    .as_ref()
                    .map(|b| format!(" else {b}"))
                    .unwrap_or_default();
                write!(f, "for {var} in {iter}{guard_s} {body}{else_s}")
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
            } => {
                let guard_s = guard
                    .as_ref()
                    .map(|g| format!(" if {g}"))
                    .unwrap_or_default();
                let else_s = else_body
                    .as_ref()
                    .map(|b| format!(" else {b}"))
                    .unwrap_or_default();
                write!(f, "while {cond}{guard_s} {body}{else_s}")
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                let guard_s = guard
                    .as_ref()
                    .map(|g| format!(" if {g}"))
                    .unwrap_or_default();
                write!(f, "while let {pattern} = {expr}{guard_s} {body}")
            }
            Stmt::Match { scrutinee, arms } => {
                write!(f, "match {scrutinee} {{")?;
                for arm in arms {
                    let guard_s = arm
                        .guard
                        .as_ref()
                        .map(|g| format!(" if {g}"))
                        .unwrap_or_default();
                    write!(f, " {}{} => {}", arm.pattern, guard_s, arm.body)?;
                }
                f.write_str(" }")
            }
            Stmt::Break => f.write_str("break"),
            Stmt::BreakLabel { label, value } => {
                write!(f, "break '{label}")?;
                if let Some(v) = value {
                    write!(f, " {v}")?;
                }
                Ok(())
            }
            Stmt::Continue => f.write_str("continue"),
            Stmt::BlockLabel { label, body } => write!(f, "block '{label} {body}"),
            Stmt::CheckerBlock {
                label,
                ps_name,
                default_checker: _,
                body: _,
            } => write!(f, "block '{label}[ps:{ps_name:?}]"),
            Stmt::Yield { value } => write!(f, "yield {value}"),
            Stmt::YieldFrom { iter } => write!(f, "yield from {iter}"),
            Stmt::Defer { body } => write!(f, "defer {body}"),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                write!(f, "try {body}")?;
                for (pat, block) in catches {
                    match pat {
                        Some(p) => write!(f, " catch({p}) {block}")?,
                        None => write!(f, " catch {block}")?,
                    }
                }
                if let Some(els) = else_body {
                    write!(f, " else {els}")?;
                }
                if let Some(fin) = finally_body {
                    write!(f, " finally {fin}")?;
                }
                Ok(())
            }
            Stmt::Block { stmts } => {
                if stmts.is_empty() {
                    return f.write_str("{ }");
                }
                writeln!(f, "{{")?;
                for s in stmts {
                    writeln!(f, "  {s}")?;
                }
                f.write_str("}")
            }
            _ => f.write_str("<stmt>"),
        }
    }
}

// ── Item Display ──

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Item::FnDef(fn_def) => {
                write!(f, "fn {}", fn_def.name)?;
                if !fn_def.generics.is_empty() {
                    f.write_str("<")?;
                    for (i, g) in fn_def.generics.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        f.write_str(&g.name)?;
                    }
                    f.write_str(">")?;
                }
                f.write_str("(")?;
                for (i, p) in fn_def.params.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    if p.is_mut {
                        f.write_str("mut ")?;
                    }
                    if p.is_owned {
                        f.write_str("owned ")?;
                    }
                    if p.is_ref {
                        f.write_str("ref ")?;
                    }
                    write!(f, "{}: {}", p.name, p.ty)?;
                }
                writeln!(f, ") -> {}:", fn_def.ret_ty)?;
                // Body
                for stmt in &fn_def.body.stmts {
                    writeln!(f, "  {stmt}")?;
                }
                Ok(())
            }
            Item::StructDef(s) => {
                write!(f, "struct {}", s.name)?;
                if !s.generics.is_empty() {
                    f.write_str("<")?;
                    for (i, g) in s.generics.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        f.write_str(&g.name)?;
                    }
                    f.write_str(">")?;
                }
                writeln!(f, " {{")?;
                for field in &s.fields {
                    writeln!(f, "  {}: {}", field.name, field.ty)?;
                }
                f.write_str("}")
            }
            Item::EnumDef(e) => {
                write!(f, "enum {}", e.name)?;
                if !e.generics.is_empty() {
                    f.write_str("<")?;
                    for (i, g) in e.generics.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        f.write_str(&g.name)?;
                    }
                    f.write_str(">")?;
                }
                writeln!(f, " {{")?;
                for v in &e.variants {
                    if v.fields.is_empty() {
                        writeln!(f, "  {}", v.name)?;
                    } else {
                        f.write_str("  ")?;
                        f.write_str(&v.name)?;
                        f.write_str("(")?;
                        for (i, t) in v.fields.iter().enumerate() {
                            if i > 0 {
                                f.write_str(", ")?;
                            }
                            write!(f, "{}", t.ty)?;
                        }
                        writeln!(f, ")")?;
                    }
                }
                f.write_str("}")
            }
            Item::TraitDef(t) => {
                write!(f, "trait {}", t.name)?;
                if !t.supertraits.is_empty() {
                    f.write_str(" : ")?;
                    for (i, st) in t.supertraits.iter().enumerate() {
                        if i > 0 {
                            f.write_str(" + ")?;
                        }
                        write!(f, "{st}")?;
                    }
                }
                writeln!(f, " {{")?;
                for m in &t.methods {
                    f.write_str("  fn ")?;
                    f.write_str(&m.name)?;
                    f.write_str("(")?;
                    for (i, p) in m.params.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{p}")?;
                    }
                    writeln!(f, ") -> {}", m.ret)?;
                }
                f.write_str("}")
            }
            Item::Impl(imp) => {
                f.write_str("impl ")?;
                if let Some(tr) = &imp.trait_ {
                    write!(f, "{tr} for ")?;
                }
                write!(f, "{}", imp.for_type)?;
                writeln!(f, " {{")?;
                for m in &imp.methods {
                    writeln!(f, "  fn {} ...", m.name)?;
                }
                f.write_str("}")
            }
            Item::Use(u) => {
                let path = u.path.join(".");
                if u.is_from {
                    write!(f, "from {path} import {}", u.items.join(", "))
                } else {
                    write!(f, "import {path}")
                }
            }
            Item::Const(c) => write!(f, "const {}: {} = {}", c.name, c.ty, c.value),
            Item::TypeAlias(ta) => write!(f, "type {} = {}", ta.name, ta.ty),
            Item::Test(t) => write!(f, "test {} {:#?}", t.name, t.body),
            Item::CheckerBlock { name, ps_name, .. } => {
                write!(f, "checker block '{name}[ps:{ps_name:?}]")
            }
            Item::DuckDef(d) => {
                write!(f, "duck {} {{ {} methods }}", d.name, d.methods.len())
            }
        }
    }
}

// ── IrModule Display ──

impl fmt::Display for super::IrModule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, ";; LZIR v{} — module '{}'", self.version, self.name)?;
        writeln!(f, ";; {} items", self.items.len())?;

        if !self.prelude.is_empty() {
            writeln!(f, ";; prelude: {}", self.prelude.join(", "))?;
        }
        writeln!(f)?;

        for item in &self.items {
            writeln!(f, "{item}")?;
            writeln!(f)?;
        }
        Ok(())
    }
}
