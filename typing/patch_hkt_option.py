#!/usr/bin/env python3
"""Patch HKT unify to canonicalize Option/Optional/Result and fix occurs-check for Apply."""
import pathlib
import sys

ROOT = pathlib.Path("e:/IDEProjects/AI/lang-zone")


def patch_unify():
    path = ROOT / "src/hints/unify.rs"
    text = path.read_text(encoding="utf-8")

    insert_after = '''use crate::types::def::Type;
use crate::hints::tyvar::{InferCtx, TypeError};
'''
    if "to_canonical" in text:
        print("unify.rs: to_canonical already present, skipping")
        return

    canonical = '''
/// 将类型简写归一化为规范形式，使 Option/Optional/Result 能参与 HKT 统一。
///
/// - `Option(T)` → `Generic { base: Named("Option"), args: [T] }`
/// - `Optional(T)` → `Generic { base: Named("Option"), args: [T] }`（T? 语法糖）
/// - `Result { ok, err }` → `Generic { base: Named("Result"), args: [ok, err] }`
fn to_canonical(t: &Type) -> Type {
    match t {
        Type::Option(inner) => Type::Generic {
            base: Box::new(Type::Named("Option".into())),
            args: vec![*inner.clone()],
        },
        Type::Optional(inner) => Type::Generic {
            base: Box::new(Type::Named("Option".into())),
            args: vec![*inner.clone()],
        },
        Type::Result { ok, err } => Type::Generic {
            base: Box::new(Type::Named("Result".into())),
            args: vec![*ok.clone(), *err.clone()],
        },
        other => other.clone(),
    }
}
'''

    old_start = '''/// 统一 a 与 b。推断变量在统一过程中被绑定，具体类型按结构递归。
pub fn unify(ctx: &mut InferCtx, a: &Type, b: &Type) -> Result<(), TypeError> {
    // 取得所有权形式后按结构匹配；递归统一内部类型时统一以 & 传参
    let a = ctx.prune(a);
    let b = ctx.prune(b);

    match (a, b) {'''

    new_start = '''/// 统一 a 与 b。推断变量在统一过程中被绑定，具体类型按结构递归。
pub fn unify(ctx: &mut InferCtx, a: &Type, b: &Type) -> Result<(), TypeError> {
    // 取得所有权形式后按结构匹配；递归统一内部类型时统一以 & 传参
    let a = ctx.prune(a);
    let b = ctx.prune(b);

    // 归一化：让 Option/Optional/Result 与 Generic/Apply 使用同一套统一规则
    let a = to_canonical(&a);
    let b = to_canonical(&b);

    match (a, b) {'''

    if insert_after not in text:
        raise RuntimeError("unify.rs: anchor not found")
    text = text.replace(insert_after, insert_after + canonical)
    if old_start not in text:
        raise RuntimeError("unify.rs: unify start anchor not found")
    text = text.replace(old_start, new_start)
    path.write_text(text, encoding="utf-8")
    print("unify.rs: patched")


def patch_tyvar():
    path = ROOT / "src/hints/tyvar.rs"
    text = path.read_text(encoding="utf-8")

    old_occurs = '''    pub fn occurs(&self, v: TyVar, t: &Type) -> bool {
        let target = self.find(v);
        match t {
            Type::Var(w) => self.find(*w) == target,
            Type::Option(inner) | Type::Optional(inner)
            | Type::Ref(inner) | Type::MutRef(inner) =>
                self.occurs(v, inner),
            Type::Result { ok, err } =>
                self.occurs(v, ok) || self.occurs(v, err),
            Type::Generic { args, .. } | Type::Tuple(args) =>
                args.iter().any(|a| self.occurs(v, a)),
            Type::Fn { params, ret } =>
                params.iter().any(|p| self.occurs(v, p)) || self.occurs(v, ret),
            Type::Simd { elem, .. } =>
                self.occurs(v, elem),
            Type::Intersection(args) =>
                args.iter().any(|a| self.occurs(v, a)),
            _ => false,
        }
    }'''

    new_occurs = '''    pub fn occurs(&self, v: TyVar, t: &Type) -> bool {
        let target = self.find(v);
        match t {
            Type::Var(w) => self.find(*w) == target,
            Type::Option(inner) | Type::Optional(inner)
            | Type::Ref(inner) | Type::MutRef(inner) =>
                self.occurs(v, inner),
            Type::Result { ok, err } =>
                self.occurs(v, ok) || self.occurs(v, err),
            Type::Generic { args, .. } | Type::Tuple(args) |
            Type::Union(args) | Type::Futures(args) |
            Type::Intersection(args) =>
                args.iter().any(|a| self.occurs(v, a)),
            Type::Record(fields) =>
                fields.iter().any(|(_, t)| self.occurs(v, t)),
            Type::Apply { constructor, args } =>
                self.occurs(v, constructor) || args.iter().any(|a| self.occurs(v, a)),
            Type::Fn { params, ret } =>
                params.iter().any(|p| self.occurs(v, p)) || self.occurs(v, ret),
            Type::Simd { elem, .. } | Type::Future(elem) =>
                self.occurs(v, elem),
            _ => false,
        }
    }'''

    if "Type::Apply" in text:
        print("tyvar.rs: Apply already handled in occurs, skipping")
        return

    if old_occurs not in text:
        raise RuntimeError("tyvar.rs: old occurs body not found")
    text = text.replace(old_occurs, new_occurs)
    path.write_text(text, encoding="utf-8")
    print("tyvar.rs: patched")


if __name__ == "__main__":
    patch_unify()
    patch_tyvar()
