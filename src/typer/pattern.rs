// Lang-Zone 编译器 — typer/pattern.rs
// （由 typer/mod.rs move-only 拆出，逻辑零改动）

use super::*;

impl Typer {
    /// 推断模式中的标识符类型
    pub(crate) fn infer_pattern(
        sess: &mut InferSession,
        pattern: &crate::ast::Pattern,
        expected_ty: Option<&Type>,
    ) -> Result<(), TypeError> {
        match pattern {
            crate::ast::Pattern::Ident(name) => {
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(name.clone(), ty);
            }
            crate::ast::Pattern::Variant(_, sub) => {
                if let Some(Type::Tuple(elems)) = expected_ty {
                    if elems.len() == sub.len() {
                        for (p, t) in sub.iter().zip(elems.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                        return Ok(());
                    }
                }
                if let Some(Type::Record(fields)) = expected_ty {
                    if fields.len() == sub.len() {
                        for ((_, t), p) in fields.iter().zip(sub.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                        return Ok(());
                    }
                }
                if sub.len() == 1 {
                    Self::infer_pattern(sess, &sub[0], expected_ty)?;
                } else {
                    for p in sub {
                        Self::infer_pattern(sess, p, None)?;
                    }
                }
            }
            crate::ast::Pattern::Tuple(ps) => {
                if let Some(Type::Tuple(elems)) = expected_ty {
                    if elems.len() == ps.len() {
                        for (p, t) in ps.iter().zip(elems.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                    } else {
                        for p in ps {
                            Self::infer_pattern(sess, p, None)?;
                        }
                    }
                } else {
                    for p in ps {
                        Self::infer_pattern(sess, p, None)?;
                    }
                }
            }
            crate::ast::Pattern::Array(elems) => {
                if let Some(Type::Tuple(elems_ty)) = expected_ty {
                    if elems_ty.len() == elems.len() {
                        for (p, t) in elems.iter().zip(elems_ty.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                    } else {
                        for p in elems {
                            Self::infer_pattern(sess, p, None)?;
                        }
                    }
                } else {
                    for p in elems {
                        Self::infer_pattern(sess, p, None)?;
                    }
                }
            }
            // P5: AS 模式 — 递归推断内层 pattern，同时绑定 as_name
            crate::ast::Pattern::As(inner, as_name) => {
                Self::infer_pattern(sess, inner, expected_ty)?;
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(as_name.clone(), ty);
            }
            // P6: 类型模式 — 绑定变量到环境
            crate::ast::Pattern::Type {
                type_name: _,
                binding,
            } => {
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(binding.clone(), ty);
            }
            // P7: 范围模式 — 无变量绑定
            crate::ast::Pattern::Range { .. } => {}
            // P1~P4 已有模式无新绑定
            crate::ast::Pattern::Slice(Some(name)) => {
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(name.clone(), ty);
            }
            crate::ast::Pattern::Slice(None) => {}
            crate::ast::Pattern::Dict { pairs, rest } => {
                for (k, v) in pairs {
                    Self::infer_pattern(sess, k, None)?;
                    Self::infer_pattern(sess, v, None)?;
                }
                if let Some(name) = rest {
                    let tv = sess.ctx.fresh_ty(0);
                    sess.env.insert(name.clone(), tv);
                }
            }
            crate::ast::Pattern::StructVariant { fields, .. } => {
                if let Some(Type::Record(map)) = expected_ty {
                    for (fname, p) in fields {
                        let fty = map.iter().find(|(n, _)| n == fname).map(|(_, t)| t);
                        Self::infer_pattern(sess, p, fty)?;
                    }
                } else {
                    for (_, p) in fields {
                        Self::infer_pattern(sess, p, None)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
