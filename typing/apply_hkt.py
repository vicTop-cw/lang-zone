#!/usr/bin/env python3
# Apply HKT edits to lang-zone sources (Edit tool is restricted to cwd).

from pathlib import Path

ROOT = Path("e:/IDEProjects/AI/lang-zone")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise RuntimeError(f"patch block not found: {label}")
    if text.count(old) != 1:
        raise RuntimeError(f"patch block not unique: {label}")
    return text.replace(old, new)


def patch_parser(src: str) -> str:
    # 1. parse_generic_params_rich signature + body
    old = """    /// 解析泛型参数，同时捕获定义点约束和默认值
    fn parse_generic_params_rich(&mut self) -> Result<(Vec<String>, Vec<(String, Vec<Type>)>, Vec<(String, Type)>), String> {
        self.expect(Token::Lt)?;
        let mut names = Vec::new();
        let mut bounds: Vec<(String, Vec<Type>)> = Vec::new();
        let mut defaults: Vec<(String, Type)> = Vec::new();

        loop {
            let name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected generic param, got {:?}", t)),
            };

            // T: Clone + Ord → 定义点约束
            if self.check(&Token::Colon) {
                self.advance();
                let mut b = Vec::new();
                loop {
                    b.push(self.parse_type()?);
                    if self.check(&Token::Plus) { self.advance(); } else { break; }
                }
                bounds.push((name.clone(), b));
            }

            // T = i64 → 默认类型参数
            if self.check(&Token::Eq) {
                self.advance();
                let default = self.parse_type()?;
                defaults.push((name.clone(), default));
            }

            names.push(name);

            if self.check(&Token::Comma) {
                self.advance();
                // 守卫：泛型参数列表不允许连续逗号（如 `<T,, U>`）
                if self.check(&Token::Comma) {
                    return Err("泛型参数列表不允许连续逗号（多余逗号）".into());
                }
                continue;
            }
            if self.check(&Token::Gt) {
                self.advance();
                break;
            }
            if self.check(&Token::Shr) {
                self.advance();
                self.pending_gt += 1;
                break;
            }
            break;
        }
        Ok((names, bounds, defaults))
    }"""
    new = """    /// 解析泛型参数，同时捕获 kind 标注、定义点约束和默认值
    fn parse_generic_params_rich(&mut self) -> Result<(Vec<String>, Vec<(String, Kind)>, Vec<(String, Vec<Type>)>, Vec<(String, Type)>), String> {
        self.expect(Token::Lt)?;
        let mut names = Vec::new();
        let mut kinds: Vec<(String, Kind)> = Vec::new();
        let mut bounds: Vec<(String, Vec<Type>)> = Vec::new();
        let mut defaults: Vec<(String, Type)> = Vec::new();

        loop {
            let name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected generic param, got {:?}", t)),
            };

            // HKT kind 标注: F[_] / F[A] → * -> *
            if self.check(&Token::LBrack) {
                self.advance();
                let mut arity = 0;
                loop {
                    if self.check(&Token::Underscore) {
                        self.advance();
                        arity += 1;
                    } else if matches!(self.peek(), Token::Ident(_)) {
                        self.advance();
                        arity += 1;
                    } else {
                        break;
                    }
                    if self.check(&Token::Comma) { self.advance(); }
                }
                self.expect(Token::RBrack)?;
                if arity == 0 {
                    return Err("类型构造器 kind 标注至少需要 one 形参，如 F[_]".into());
                }
                let params = vec![Kind::Star; arity];
                kinds.push((name.clone(), Kind::Arrow { params, ret: Box::new(Kind::Star) }));
            }

            // T: Clone + Ord → 定义点约束
            if self.check(&Token::Colon) {
                self.advance();
                let mut b = Vec::new();
                loop {
                    b.push(self.parse_type()?);
                    if self.check(&Token::Plus) { self.advance(); } else { break; }
                }
                bounds.push((name.clone(), b));
            }

            // T = i64 → 默认类型参数
            if self.check(&Token::Eq) {
                self.advance();
                let default = self.parse_type()?;
                defaults.push((name.clone(), default));
            }

            names.push(name);

            if self.check(&Token::Comma) {
                self.advance();
                // 守卫：泛型参数列表不允许连续逗号（如 `<T,, U>`）
                if self.check(&Token::Comma) {
                    return Err("泛型参数列表不允许连续逗号（多余逗号）".into());
                }
                continue;
            }
            if self.check(&Token::Gt) {
                self.advance();
                break;
            }
            if self.check(&Token::Shr) {
                self.advance();
                self.pending_gt += 1;
                break;
            }
            break;
        }
        Ok((names, kinds, bounds, defaults))
    }"""
    src = replace_once(src, old, new, "parse_generic_params_rich")

    # 2. parse_function: capture generic_kinds, push/pop, pass to Function
    old = """        // 泛型参数
        let (generics, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        // 参数
        self.expect(Token::LParen)?;"""
    new = """        // 泛型参数
        let (generics, generic_kinds, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };
        self.push_generic_kinds(&generic_kinds);

        // 参数
        self.expect(Token::LParen)?;"""
    src = replace_once(src, old, new, "parse_function generics")

    old = """        Ok(Function { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults,
            params, return_type, raises,
            where_clause, body, is_async: false, is_abstract,
            comptime: is_comptime, decorators: Vec::new(), attributes: Vec::new(), variadic, params_checker,
        })"""
    new = """        self.pop_generic_kinds();
        Ok(Function { name, generics, generic_kinds, generic_bounds, generic_defaults,
            params, return_type, raises,
            where_clause, body, is_async: false, is_abstract,
            comptime: is_comptime, decorators: Vec::new(), attributes: Vec::new(), variadic, params_checker,
        })"""
    src = replace_once(src, old, new, "parse_function Ok")

    # 3. parse_struct_like: capture generic_kinds, push/pop
    old = """        let (generics, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        // struct 声明必须使用 '='，enum 声明必须使用 ':'"""
    new = """        let (generics, generic_kinds, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };
        self.push_generic_kinds(&generic_kinds);

        // struct 声明必须使用 '='，enum 声明必须使用 ':'"""
    src = replace_once(src, old, new, "parse_struct_like generics")

    old = """            self.expect(Token::Dedent)?;
            return Ok(StructDef { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults, fields, methods, is_enum, decorators: Vec::new(), attributes: Vec::new(), repr_attr });
        }"""
    new = """            self.expect(Token::Dedent)?;
            self.pop_generic_kinds();
            return Ok(StructDef { name, generics, generic_kinds, generic_bounds, generic_defaults, fields, methods, is_enum, decorators: Vec::new(), attributes: Vec::new(), repr_attr });
        }"""
    src = replace_once(src, old, new, "parse_struct_like dedent return")

    old = """        return Ok(StructDef { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults, fields, methods, is_enum, decorators: Vec::new(), attributes: Vec::new(), repr_attr: None });
    }"""
    new = """        self.pop_generic_kinds();
        return Ok(StructDef { name, generics, generic_kinds, generic_bounds, generic_defaults, fields, methods, is_enum, decorators: Vec::new(), attributes: Vec::new(), repr_attr: None });
    }"""
    src = replace_once(src, old, new, "parse_struct_like single return")

    # 4. parse_enum_pipe_variants: signature + returns
    old = """    fn parse_enum_pipe_variants(
        &mut self,
        name: String,
        generics: Vec<String>,
        generic_bounds: Vec<(String, Vec<Type>)>,
        generic_defaults: Vec<(String, Type)>,
    ) -> Result<StructDef, String> {"""
    new = """    fn parse_enum_pipe_variants(
        &mut self,
        name: String,
        generics: Vec<String>,
        generic_kinds: Vec<(String, Kind)>,
        generic_bounds: Vec<(String, Vec<Type>)>,
        generic_defaults: Vec<(String, Type)>,
    ) -> Result<StructDef, String> {"""
    src = replace_once(src, old, new, "parse_enum_pipe_variants signature")

    old = """        // 空枚举
        if self.check(&Token::Newline) || self.check(&Token::Dedent) || self.check(&Token::Eof) {
            return Ok(StructDef { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults,
                fields, methods: Vec::new(), is_enum: true,
                decorators: Vec::new(), attributes: Vec::new(), repr_attr: None,
            });
        }"""
    new = """        // 空枚举
        if self.check(&Token::Newline) || self.check(&Token::Dedent) || self.check(&Token::Eof) {
            self.pop_generic_kinds();
            return Ok(StructDef { name, generics, generic_kinds, generic_bounds, generic_defaults,
                fields, methods: Vec::new(), is_enum: true,
                decorators: Vec::new(), attributes: Vec::new(), repr_attr: None,
            });
        }"""
    src = replace_once(src, old, new, "parse_enum_pipe_variants empty")

    old = """        Ok(StructDef { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults,
            fields, methods: Vec::new(), is_enum: true,
            decorators: Vec::new(), attributes: Vec::new(), repr_attr: None,
        })
    }"""
    new = """        self.pop_generic_kinds();
        Ok(StructDef { name, generics, generic_kinds, generic_bounds, generic_defaults,
            fields, methods: Vec::new(), is_enum: true,
            decorators: Vec::new(), attributes: Vec::new(), repr_attr: None,
        })
    }"""
    src = replace_once(src, old, new, "parse_enum_pipe_variants Ok")

    # 5. parse_trait
    old = """        let (generics, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
        self.expect(Token::Eq)?;
        self.skip_newlines();
        self.expect(Token::Indent)?;

        let mut methods = Vec::new();"""
    new = """        let (generics, generic_kinds, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };
        self.push_generic_kinds(&generic_kinds);
        self.expect(Token::Eq)?;
        self.skip_newlines();
        self.expect(Token::Indent)?;

        let mut methods = Vec::new();"""
    src = replace_once(src, old, new, "parse_trait generics")

    old = """        self.expect(Token::Dedent)?;
        Ok(TraitDef { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults, methods, fields, type_aliases })
    }"""
    new = """        self.expect(Token::Dedent)?;
        self.pop_generic_kinds();
        Ok(TraitDef { name, generics, generic_kinds, generic_bounds, generic_defaults, methods, fields, type_aliases })
    }"""
    src = replace_once(src, old, new, "parse_trait Ok")

    # 6. parse_impl
    old = """        let (mut generics, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        let first_name = match self.advance() {"""
    new = """        let (mut generics, generic_kinds, generic_bounds, generic_defaults) = if self.check(&Token::Lt) {
            self.parse_generic_params_rich()?
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };
        self.push_generic_kinds(&generic_kinds);

        let first_name = match self.advance() {"""
    src = replace_once(src, old, new, "parse_impl generics")

    old = """        Ok(ImplDef { trait_name, type_name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults, where_clause, methods, type_aliases })
    }"""
    new = """        self.pop_generic_kinds();
        Ok(ImplDef { trait_name, type_name, generics, generic_kinds, generic_bounds, generic_defaults, where_clause, methods, type_aliases })
    }"""
    src = replace_once(src, old, new, "parse_impl Ok")

    # 7. HKT type application F[A] in parse_primary_type
    old = """                    _ => Ok(Type::Generic {
                            base: Box::new(base_ty),
                            args: inner,
                        }),
                    };
                }
                base_ty
            }"""
    new = """                    _ => Ok(Type::Generic {
                            base: Box::new(base_ty),
                            args: inner,
                        }),
                    };
                }
                // HKT 类型构造器应用: F[A]（仅在当前作用域内 F 被声明为 F[_] 时）
                if self.check(&Token::LBrack) {
                    if let Some(kind) = self.current_generic_kind(&n) {
                        if matches!(kind, Kind::Arrow { .. }) {
                            self.advance(); // [
                            let mut args = Vec::new();
                            loop {
                                args.push(self.parse_type()?);
                                if self.check(&Token::Comma) { self.advance(); }
                                if self.check(&Token::RBrack) {
                                    self.advance();
                                    break;
                                }
                            }
                            return Ok(Type::Apply {
                                constructor: Box::new(Type::Constructor { name: n.clone(), arity: args.len() }),
                                args,
                            });
                        }
                    }
                }
                base_ty
            }"""
    src = replace_once(src, old, new, "parse_primary_type HKT bracket")

    return src


def patch_typer(src: str) -> str:
    # expand_type
    old = """        Type::Union(ts) => Type::Union(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        _ => t.clone(),
    }
}"""
    new = """        Type::Union(ts) => Type::Union(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Constructor { name, arity } => Type::Constructor { name: name.clone(), arity: *arity },
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(expand_type(aliases, constructor)),
            args: args.iter().map(|a| expand_type(aliases, a)).collect(),
        },
        _ => t.clone(),
    }
}"""
    src = replace_once(src, old, new, "expand_type")

    # substitute
    old = """        Type::Union(ts) => Type::Union(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| substitute(subst, x)).collect()),
        _ => t.clone(),
    }
}"""
    new = """        Type::Union(ts) => Type::Union(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Constructor { name, arity } => {
            if let Some(repl) = subst.get(name) {
                return repl.clone();
            }
            Type::Constructor { name: name.clone(), arity: *arity }
        }
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(substitute(subst, constructor)),
            args: args.iter().map(|a| substitute(subst, a)).collect(),
        },
        _ => t.clone(),
    }
}"""
    src = replace_once(src, old, new, "substitute")

    # resolve_type_name
    old = """fn resolve_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(name) => Some(name.clone()),
        Type::Generic { base, .. } => match base.as_ref() {
            Type::Named(name) => Some(name.clone()),
            _ => None,
        },
        Type::Ref(inner) | Type::MutRef(inner) => resolve_type_name(inner),
        Type::Option(inner) | Type::Optional(inner) => resolve_type_name(inner),
        _ => None,
    }
}"""
    new = """fn resolve_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(name) => Some(name.clone()),
        Type::Generic { base, .. } => match base.as_ref() {
            Type::Named(name) => Some(name.clone()),
            _ => None,
        },
        Type::Apply { constructor, .. } => match constructor.as_ref() {
            Type::Named(name) | Type::Constructor { name, .. } => Some(name.clone()),
            _ => resolve_type_name(constructor),
        },
        Type::Ref(inner) | Type::MutRef(inner) => resolve_type_name(inner),
        Type::Option(inner) | Type::Optional(inner) => resolve_type_name(inner),
        _ => None,
    }
}"""
    src = replace_once(src, old, new, "resolve_type_name")

    return src


def patch_unify(src: str) -> str:
    old = """        (Type::Generic { base: b1, args: a1 }, Type::Generic { base: b2, args: a2 }) => {
            // 构造器（List / Dict / Set …）必须对齐
            unify(ctx, &b1, &b2)?;
            if a1.len() != a2.len() {
                return Err(TypeError::Arity(a1.len(), a2.len()));
            }
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }

        (Type::Tuple(e1), Type::Tuple(e2)) => {"""
    new = """        (Type::Generic { base: b1, args: a1 }, Type::Generic { base: b2, args: a2 }) => {
            // 构造器（List / Dict / Set …）必须对齐
            unify(ctx, &b1, &b2)?;
            if a1.len() != a2.len() {
                return Err(TypeError::Arity(a1.len(), a2.len()));
            }
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }

        // HKT: Apply 与 Generic 语义等价，交叉统一
        (Type::Apply { constructor: c1, args: a1 }, Type::Apply { constructor: c2, args: a2 }) => {
            unify(ctx, c1, c2)?;
            if a1.len() != a2.len() {
                return Err(TypeError::Arity(a1.len(), a2.len()));
            }
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }
        (Type::Apply { constructor, args }, Type::Generic { base, args: args2 })
        | (Type::Generic { base, args: args2 }, Type::Apply { constructor, args }) => {
            unify(ctx, constructor, base)?;
            if args.len() != args2.len() {
                return Err(TypeError::Arity(args.len(), args2.len()));
            }
            for (x, y) in args.iter().zip(args2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }

        // 类型构造器：同名同 arity 即统一
        (Type::Constructor { name: n1, arity: a1 }, Type::Constructor { name: n2, arity: a2 }) if n1 == n2 && a1 == a2 => Ok(()),

        (Type::Tuple(e1), Type::Tuple(e2)) => {"""
    src = replace_once(src, old, new, "unify HKT")
    return src


def patch_relate(src: str) -> str:
    old = """        // ── 泛型（含归一化后的 Option/Optional/Result）：base 相等 + 实参协变 ──
        (Type::Generic { base: sb, args: sa }, Type::Generic { base: ub, args: ua }) => {
            conforms(ctx, sb, ub)?;
            if sa.len() != ua.len() {
                return Err(TypingError::Arity(sa.len(), ua.len()));
            }
            for (s, u) in sa.iter().zip(ua.iter()) {
                conforms(ctx, s, u)?; // 协变
            }
            Ok(())
        }

        // ── 元组：等长 + 逐元素协变 ──"""
    new = """        // ── 泛型（含归一化后的 Option/Optional/Result）：base 相等 + 实参协变 ──
        (Type::Generic { base: sb, args: sa }, Type::Generic { base: ub, args: ua }) => {
            conforms(ctx, sb, ub)?;
            if sa.len() != ua.len() {
                return Err(TypingError::Arity(sa.len(), ua.len()));
            }
            for (s, u) in sa.iter().zip(ua.iter()) {
                conforms(ctx, s, u)?; // 协变
            }
            Ok(())
        }

        // ── HKT: Apply 与 Generic 同构 ──
        (Type::Apply { constructor: sc, args: sa }, Type::Apply { constructor: uc, args: ua }) => {
            conforms(ctx, sc, uc)?;
            if sa.len() != ua.len() {
                return Err(TypingError::Arity(sa.len(), ua.len()));
            }
            for (s, u) in sa.iter().zip(ua.iter()) {
                conforms(ctx, s, u)?;
            }
            Ok(())
        }
        (Type::Apply { constructor, args: sa }, Type::Generic { base, args: ua })
        | (Type::Generic { base, args: sa }, Type::Apply { constructor, args: ua }) => {
            conforms(ctx, constructor, base)?;
            if sa.len() != ua.len() {
                return Err(TypingError::Arity(sa.len(), ua.len()));
            }
            for (s, u) in sa.iter().zip(ua.iter()) {
                conforms(ctx, s, u)?;
            }
            Ok(())
        }

        // ── 元组：等长 + 逐元素协变 ──"""
    src = replace_once(src, old, new, "relate HKT")
    return src


def patch_subst(src: str) -> str:
    old = """        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|t| zonk(ctx, t)).collect()),
        other => other.clone(),
    }
}"""
    new = """        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|t| zonk(ctx, t)).collect()),
        Type::Constructor { name, arity } => Type::Constructor { name: name.clone(), arity: *arity },
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(zonk(ctx, constructor)),
            args: args.iter().map(|a| zonk(ctx, a)).collect(),
        },
        other => other.clone(),
    }
}"""
    src = replace_once(src, old, new, "zonk HKT")

    old = """        Type::Intersection(ts) => ts.iter().all(|t| is_resolved(ctx, t)),
        _ => true,
    }
}"""
    new = """        Type::Intersection(ts) => ts.iter().all(|t| is_resolved(ctx, t)),
        Type::Apply { constructor, args } => is_resolved(ctx, constructor) && args.iter().all(|a| is_resolved(ctx, a)),
        Type::Constructor { .. } => true,
        _ => true,
    }
}"""
    src = replace_once(src, old, new, "is_resolved HKT")
    return src


def patch_lzi_infer(src: str) -> str:
    old = """        Type::Named(name) => name.clone(),
        Type::Var(_) => "?".into(),
        Type::Generic { base, args } => {"""
    new = """        Type::Named(name) => name.clone(),
        Type::Var(_) => "?".into(),
        Type::Constructor { name, .. } => name.clone(),
        Type::Apply { constructor, args } => {
            let base_s = type_to_lz_string(constructor);
            let args_s: Vec<String> = args.iter().map(type_to_lz_string).collect();
            format!("{}<{}>", base_s, args_s.join(", "))
        }
        Type::Generic { base, args } => {"""
    src = replace_once(src, old, new, "type_to_lz_string HKT")
    return src


def main():
    parser_path = ROOT / "src/parser/parser.rs"
    write(parser_path, patch_parser(read(parser_path)))

    typer_path = ROOT / "src/typer/mod.rs"
    write(typer_path, patch_typer(read(typer_path)))

    unify_path = ROOT / "src/hints/unify.rs"
    write(unify_path, patch_unify(read(unify_path)))

    relate_path = ROOT / "src/typing/relate.rs"
    write(relate_path, patch_relate(read(relate_path)))

    subst_path = ROOT / "src/hints/subst.rs"
    write(subst_path, patch_subst(read(subst_path)))

    lzi_path = ROOT / "lz-infer/src/infer.rs"
    write(lzi_path, patch_lzi_infer(read(lzi_path)))

    print("HKT patches applied.")


if __name__ == "__main__":
    main()
