from pathlib import Path

p = Path(r'e:\IDEProjects\AI\lang-zone\src\typer\mod.rs')
s = p.read_text(encoding='utf-8')

s = s.replace(
    'return apply_subst(subst, ret)',
    'return substitute(subst, ret)')
# Wait we want apply_subst used, so replace substitute with apply_subst in enum_self_type_with_subst.
# Let's revert any previous wrong replacement by doing targeted.

s = s.replace(
    '''    if let Some(ret) = &variant.return_type {
        substitute(subst, ret)''',
    '''    if let Some(ret) = &variant.return_type {
        apply_subst(subst, ret)''')

s = s.replace(
    'if let Some(variant) = sess.enum_variants.get(&format!("{}.{}", enum_name, field)).cloned() {',
    'if let Some(variant) = sess.enum_variant(enum_name, field).cloned() {')

s = s.replace(
    'if let Some(variant) = sess.enum_variants.get(&key).cloned() {',
    'if let Some(variant) = sess.enum_variant(enum_name, method).cloned() {')

s = s.replace(
    'if let Some(variant) = sess.enum_variants.get(&format!("{}.{}", enum_name, variant_name)).cloned() {',
    'if let Some(variant) = sess.enum_variant(&enum_name, &variant_name).cloned() {')

p.write_text(s, encoding='utf-8')
print('done')
