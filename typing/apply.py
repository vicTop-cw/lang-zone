import os

ROOT = r'e:\IDEProjects\AI\lang-zone'


def read(p):
    with open(p, 'r', encoding='utf-8') as f:
        return f.read()


def write(p, s):
    with open(p, 'w', encoding='utf-8', newline='') as f:
        f.write(s)


# ── src/types/def.rs ──
def edit_def_rs():
    p = os.path.join(ROOT, 'src', 'types', 'def.rs')
    s = read(p)
    s = s.replace(
        '    // ── 联合类型 ──\n    Union(Vec<Type>),           // A | B | C\n}',
        '    // ── 联合类型 ──\n    Union(Vec<Type>),           // A | B | C\n\n    // ── 交集类型 ──\n    Intersection(Vec<Type>),    // A & B & C\n}'
    )
    s = s.replace(
        '            Type::Union(_) => "Box<dyn std::any::Any>".to_string(),\n\n            Type::Var(_) => "_".to_string(), // 未解析的类型推断变量，退化到 Rust 推断',
        '            Type::Intersection(members) => {\n                let members_s: Vec<String> = members.iter().map(|m| m.to_rust_type_string()).collect();\n                format!("Box<dyn {}>", members_s.join(" + "))\n            }\n\n            Type::Union(_) => "Box<dyn std::any::Any>".to_string(),\n\n            Type::Var(_) => "_".to_string(), // 未解析的类型推断变量，退化到 Rust 推断'
    )
    write(p, s)


# ── src/parser/parser.rs ──
def edit_parser_rs():
    p = os.path.join(ROOT, 'src', 'parser', 'parser.rs')
    s = read(p)

    # 1. 添加 flatten_intersection
    s = s.replace(
        '        Type::Union(flat)\n    }\n}\n\npub struct Parser {',
        '''        Type::Union(flat)
    }
}

/// 扁平化并去重交集类型成员
fn flatten_intersection(types: Vec<Type>) -> Type {
    let mut flat = Vec::new();
    for t in types {
        if let Type::Intersection(inner) = t {
            for it in inner {
                if !flat.contains(&it) {
                    flat.push(it);
                }
            }
        } else if !flat.contains(&t) {
            flat.push(t);
        }
    }
    if flat.len() == 1 {
        flat.into_iter().next().unwrap()
    } else {
        Type::Intersection(flat)
    }
}

pub struct Parser {'''
    )

    # 2. 重构 parse_type / 新增 parse_primary_type
    sig = '    pub(super) fn parse_type(&mut self) -> Result<Type, String> {'
    start = s.find(sig)
    end_marker = '\n\n    // ─── struct / enum ───'
    end = s.find(end_marker, start)
    orig = s[start:end]

    split_at = orig.find('        // int? 语法糖')
    core = orig[len(sig) + 1:split_at]

    new_func = '''    pub(super) fn parse_type(&mut self) -> Result<Type, String> {
        let mut base = self.parse_primary_type()?;

        // int? 语法糖：int? → Option<int>, str? → Option<str>
        if self.check(&Token::Question) {
            self.advance();
            base = Type::Optional(Box::new(base));
        }

        // 交集类型：A & B & C（优先级高于 |）
        if self.check(&Token::Amp) {
            let mut types = vec![base];
            while self.check(&Token::Amp) {
                self.advance();
                types.push(self.parse_primary_type()?);
            }
            base = flatten_intersection(types);
        }

        // 联合类型：A | B | C
        if self.check(&Token::Pipe_) {
            let mut types = vec![base];
            while self.check(&Token::Pipe_) {
                self.advance();
                types.push(self.parse_type()?);
            }
            return Ok(flatten_union(types));
        }

        Ok(base)
    }

    fn parse_primary_type(&mut self) -> Result<Type, String> {
''' + core + '''    }
'''
    s = s[:start] + new_func + s[end:]
    write(p, s)


# ── src/hints/unify.rs ──
def edit_unify_rs():
    p = os.path.join(ROOT, 'src', 'hints', 'unify.rs')
    s = read(p)
    s = s.replace(
        '        // ── 联合类型：任一成员能与对方统一则成功 ──\n        (Type::Union(members), other) => {',
        '''        // ── 交集类型：所有成员必须与对方统一 ──
        (Type::Intersection(members), other) => {
            for m in &members {
                unify(ctx, m, &other)?;
            }
            Ok(())
        }
        (other, Type::Intersection(members)) => {
            for m in &members {
                unify(ctx, &other, m)?;
            }
            Ok(())
        }

        // ── 联合类型：任一成员能与对方统一则成功 ──
        (Type::Union(members), other) => {'''
    )
    write(p, s)


# ── src/typing/relate.rs ──
def edit_relate_rs():
    p = os.path.join(ROOT, 'src', 'typing', 'relate.rs')
    s = read(p)
    s = s.replace(
        '        (Type::Self_, Type::Self_) => Ok(()),\n\n        _ => Err(TypingError::Conformance(sub, sup)),',
        '''        (Type::Self_, Type::Self_) => Ok(()),

        // ── 交集类型 ──
        (Type::Intersection(members), _) => {
            for m in members {
                conforms(ctx, m, sup)?;
            }
            Ok(())
        }
        (_, Type::Intersection(members)) => {
            for m in members {
                conforms(ctx, sub, m)?;
            }
            Ok(())
        }

        _ => Err(TypingError::Conformance(sub, sup)),'''
    )
    write(p, s)


# ── src/typer/mod.rs ──
def edit_typer_mod_rs():
    p = os.path.join(ROOT, 'src', 'typer', 'mod.rs')
    s = read(p)
    s = s.replace(
        '        Type::Union(ts) => Type::Union(ts.iter().map(|x| expand_type(aliases, x)).collect()),\n        _ => t.clone(),\n    }\n}\n\n/// 将类型 `t` 中所有命名参数（Named）按 `subst` 映射替换（用于泛型别名实参代入）',
        '''        Type::Union(ts) => Type::Union(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        _ => t.clone(),
    }
}

/// 将类型 `t` 中所有命名参数（Named）按 `subst` 映射替换（用于泛型别名实参代入）'''
    )
    s = s.replace(
        '        Type::Union(ts) => Type::Union(ts.iter().map(|x| substitute(subst, x)).collect()),\n        _ => t.clone(),\n    }\n}\n\n/// 合并两个分支类型：能统一则取统一结果，否则构造最小联合类型。',
        '''        Type::Union(ts) => Type::Union(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| substitute(subst, x)).collect()),
        _ => t.clone(),
    }
}

/// 合并两个分支类型：能统一则取统一结果，否则构造最小联合类型。'''
    )
    s = s.replace(
        '''            Expr::MethodCall { receiver, method, args } => {
                let recv_type = Self::infer_expr(sess, receiver)?;
                // 从 receiver 类型提取类型名
                let type_name = resolve_type_name(&recv_type);
                // Clone 方法签名以绕过 borrow checker
                let method_sig = type_name.as_ref()
                    .and_then(|tn| sess.method_registry.get(tn))
                    .and_then(|methods| methods.get(method.as_str()))
                    .cloned();''',
        '''            Expr::MethodCall { receiver, method, args } => {
                let recv_type = Self::infer_expr(sess, receiver)?;
                // 从 receiver 类型提取类型名
                let type_name = resolve_type_name(&recv_type);
                // Clone 方法签名以绕过 borrow checker
                let method_sig = match &recv_type {
                    Type::Intersection(members) => {
                        members.iter()
                            .filter_map(|m| resolve_type_name(m))
                            .filter_map(|tn| sess.method_registry.get(&tn))
                            .filter_map(|methods| methods.get(method.as_str()))
                            .next()
                            .cloned()
                    }
                    _ => type_name.as_ref()
                        .and_then(|tn| sess.method_registry.get(tn))
                        .and_then(|methods| methods.get(method.as_str()))
                        .cloned(),
                };'''
    )
    write(p, s)


# ── src/semantic.rs ──
def edit_semantic_rs():
    p = os.path.join(ROOT, 'src', 'semantic.rs')
    s = read(p)
    s = s.replace(
        '        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|t| subst_self(t, self_name)).collect()),\n        Type::Fn { params, ret } => Type::Fn {\n            params: params.iter().map(|t| subst_self(t, self_name)).collect(),\n            ret: Box::new(subst_self(ret, self_name)),\n        },\n        other => other.clone(),\n    }\n}\n\n/// 返回类型的可读描述（None 视为单元类型 `()`）。',
        '''        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|t| subst_self(t, self_name)).collect()),
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|t| subst_self(t, self_name)).collect(),
            ret: Box::new(subst_self(ret, self_name)),
        },
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|t| subst_self(t, self_name)).collect()),
        other => other.clone(),
    }
}

/// 返回类型的可读描述（None 视为单元类型 `()`）。'''
    )
    write(p, s)


# ── src/hints/subst.rs ──
def edit_subst_rs():
    p = os.path.join(ROOT, 'src', 'hints', 'subst.rs')
    s = read(p)
    s = s.replace(
        '        Type::Simd { elem, width } => Type::Simd {\n            elem: Box::new(zonk(ctx, elem)),\n            width: *width,\n        },\n        other => other.clone(),\n    }\n}\n\n/// 便捷包装：对一组类型批量 zonk',
        '''        Type::Simd { elem, width } => Type::Simd {
            elem: Box::new(zonk(ctx, elem)),
            width: *width,
        },
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|t| zonk(ctx, t)).collect()),
        other => other.clone(),
    }
}

/// 便捷包装：对一组类型批量 zonk'''
    )
    s = s.replace(
        '        Type::Simd { elem, .. } => is_resolved(ctx, elem),\n        _ => true,\n    }\n}',
        '''        Type::Simd { elem, .. } => is_resolved(ctx, elem),
        Type::Intersection(ts) => ts.iter().all(|t| is_resolved(ctx, t)),
        _ => true,
    }
}'''
    )
    write(p, s)


# ── src/hints/tyvar.rs ──
def edit_tyvar_rs():
    p = os.path.join(ROOT, 'src', 'hints', 'tyvar.rs')
    s = read(p)
    s = s.replace(
        '            Type::Simd { elem, .. } =>\n                self.occurs(v, elem),\n            _ => false,\n        }\n    }\n}',
        '''            Type::Simd { elem, .. } =>
                self.occurs(v, elem),
            Type::Intersection(args) =>
                args.iter().any(|a| self.occurs(v, a)),
            _ => false,
        }
    }
}'''
    )
    write(p, s)


# ── src/typing/variance.rs ──
def edit_variance_rs():
    p = os.path.join(ROOT, 'src', 'typing', 'variance.rs')
    s = read(p)
    s = s.replace(
        '        // 联合类型：各成员均为协变位置\n        Type::Union(types) =>\n            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),\n    }\n}',
        '''        // 联合类型：各成员均为协变位置
        Type::Union(types) =>
            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),

        // 交集类型：与联合类型一致，各成员均为协变位置
        Type::Intersection(types) =>
            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),
    }
}'''
    )
    write(p, s)


# ── src/typing/bounds.rs ──
def edit_bounds_rs():
    p = os.path.join(ROOT, 'src', 'typing', 'bounds.rs')
    s = read(p)
    s = s.replace(
        '    // 7. 命名类型：委托 satisfies（若有 provider + env）\n    if let Type::Named(_) = ty {',
        '''    // 7. 交集类型：所有成员都必须满足
    if let Type::Intersection(members) = ty {
        for m in members {
            check_trait(m, trait_name, provider, env)?;
        }
        return Ok(());
    }

    // 8. 命名类型：委托 satisfies（若有 provider + env）
    if let Type::Named(_) = ty {'''
    )
    write(p, s)


# ── lz-infer/src/infer.rs ──
def edit_infer_rs():
    p = os.path.join(ROOT, 'lz-infer', 'src', 'infer.rs')
    s = read(p)
    s = s.replace(
        '        Type::Union(members) => {\n            let members_s: Vec<String> = members.iter().map(type_to_lz_string).collect();\n            members_s.join(" | ")\n        }\n    }\n}',
        '''        Type::Intersection(members) => {
            let members_s: Vec<String> = members.iter().map(type_to_lz_string).collect();
            members_s.join(" & ")
        }
        Type::Union(members) => {
            let members_s: Vec<String> = members.iter().map(type_to_lz_string).collect();
            members_s.join(" | ")
        }
    }
}'''
    )
    write(p, s)


# ── lz-infer/src/type_parser.rs ──
def edit_type_parser_rs():
    p = os.path.join(ROOT, 'lz-infer', 'src', 'type_parser.rs')
    s = read(p)
    s = s.replace(
        '    fn parse_type(&mut self) -> Result<Type, String> {\n        self.skip_ws();\n        let ty = self.parse_union()?;\n        self.skip_ws();\n        Ok(ty)\n    }\n\n    fn parse_union(&mut self) -> Result<Type, String> {',
        '''    fn parse_type(&mut self) -> Result<Type, String> {
        self.skip_ws();
        let ty = self.parse_intersection()?;
        self.skip_ws();
        Ok(ty)
    }

    fn parse_intersection(&mut self) -> Result<Type, String> {
        let mut types = vec![self.parse_optional()?];
        self.skip_ws();
        while self.peek() == Some('&') {
            self.advance();
            types.push(self.parse_optional()?);
            self.skip_ws();
        }
        Ok(flatten_intersection(types))
    }

    fn parse_union(&mut self) -> Result<Type, String> {'''
    )
    s = s.replace(
        'fn flatten_union(types: Vec<Type>) -> Type {\n    let mut flat = Vec::new();',
        '''fn flatten_intersection(types: Vec<Type>) -> Type {
    let mut flat = Vec::new();
    for t in types {
        if let Type::Intersection(inner) = t {
            for it in inner {
                if !flat.contains(&it) {
                    flat.push(it);
                }
            }
        } else if !flat.contains(&t) {
            flat.push(t);
        }
    }
    if flat.len() == 1 {
        flat.into_iter().next().unwrap()
    } else {
        Type::Intersection(flat)
    }
}

fn flatten_union(types: Vec<Type>) -> Type {
    let mut flat = Vec::new();'''
    )
    write(p, s)


# ── lz-infer/tests/integration.rs ──
def edit_integration_rs():
    p = os.path.join(ROOT, 'lz-infer', 'tests', 'integration.rs')
    s = read(p)
    append = '''

#[test]
fn infer_intersection_type() {
    let tmp = std::env::temp_dir().join("lz_infer_intersection.lz");
    fs::write(
        &tmp,
        "def both(x: Clone & Debug) -> Clone & Debug = x\\n",
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let both = &module.functions["both"];
    assert_eq!(both.params[0].ty, "Clone & Debug");
    assert_eq!(both.return_type.as_deref(), Some("Clone & Debug"));
}

#[test]
fn type_parser_intersection() {
    use lz_infer::type_parser::parse_type;
    use lang_zong::types::Type;

    assert_eq!(
        parse_type("A & B").unwrap(),
        Type::Intersection(vec![Type::Named("A".into()), Type::Named("B".into())])
    );
    assert_eq!(
        parse_type("A & B & A").unwrap(),
        Type::Intersection(vec![Type::Named("A".into()), Type::Named("B".into())])
    );
    assert_eq!(
        parse_type("A | B & C").unwrap(),
        Type::Union(vec![
            Type::Named("A".into()),
            Type::Intersection(vec![Type::Named("B".into()), Type::Named("C".into())])
        ])
    );
}
'''
    s = s.rstrip() + append
    write(p, s)


if __name__ == '__main__':
    edit_def_rs()
    edit_parser_rs()
    edit_unify_rs()
    edit_relate_rs()
    edit_typer_mod_rs()
    edit_semantic_rs()
    edit_subst_rs()
    edit_tyvar_rs()
    edit_variance_rs()
    edit_bounds_rs()
    edit_infer_rs()
    edit_type_parser_rs()
    edit_integration_rs()
    print('done')
