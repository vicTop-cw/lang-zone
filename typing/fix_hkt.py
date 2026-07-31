#!/usr/bin/env python3
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


def fix_parser(src: str) -> str:
    # n moved in base_ty default arm
    src = replace_once(src, """                    _ => Type::Named(n),""", """                    _ => Type::Named(n.clone()),""", "n clone")
    # pass generic_kinds to parse_enum_pipe_variants
    src = replace_once(src, """            return self.parse_enum_pipe_variants(name, generics, generic_bounds, generic_defaults);""",
                       """            return self.parse_enum_pipe_variants(name, generics, generic_kinds, generic_bounds, generic_defaults);""", "enum pipe call")
    return src


def fix_unify(src: str) -> str:
    src = replace_once(src, """        // HKT: Apply 与 Generic 语义等价，交叉统一
        (Type::Apply { constructor: c1, args: a1 }, Type::Apply { constructor: c2, args: a2 }) => {
            unify(ctx, c1, c2)?;""",
                       """        // HKT: Apply 与 Generic 语义等价，交叉统一
        (Type::Apply { constructor: c1, args: a1 }, Type::Apply { constructor: c2, args: a2 }) => {
            unify(ctx, &*c1, &*c2)?;""", "unify apply-apply")
    src = replace_once(src, """        (Type::Apply { constructor, args }, Type::Generic { base, args: args2 })
        | (Type::Generic { base, args: args2 }, Type::Apply { constructor, args }) => {
            unify(ctx, constructor, base)?;""",
                       """        (Type::Apply { constructor, args }, Type::Generic { base, args: args2 })
        | (Type::Generic { base, args: args2 }, Type::Apply { constructor, args }) => {
            unify(ctx, &*constructor, base)?;""", "unify apply-generic")
    return src


def fix_variance(src: str) -> str:
    old = """        Type::Intersection(types) =>
            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),
    }
}"""
    new = """        Type::Intersection(types) =>
            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),
        Type::Constructor { .. } => Variance::Irrelevant,
        Type::Apply { constructor, args } => {
            let c = walk(constructor, param);
            args.iter().map(|t| walk(t, param)).fold(c, combine2)
        }
    }
}"""
    return replace_once(src, old, new, "variance")


def fix_unify_base(src: str) -> str:
    return replace_once(src, "unify(ctx, &*constructor, base)?;", "unify(ctx, &*constructor, &*base)?;", "unify base borrow")


def main():
    unify_path = ROOT / "src/hints/unify.rs"
    write(unify_path, fix_unify_base(read(unify_path)))

    print("HKT fixes applied.")


if __name__ == "__main__":
    main()
