from pathlib import Path

p = Path(r'e:\IDEProjects\AI\lang-zone\src\typer\mod.rs')
s = p.read_text(encoding='utf-8')

old = '''                        let key = format!("{}.{}", enum_name, method);
                        if let Some(variant) = sess.enum_variant(enum_name, method).cloned() {'''
new = '''                        if let Some(variant) = sess.enum_variant(enum_name, method).cloned() {'''
s = s.replace(old, new)

p.write_text(s, encoding='utf-8')
print('done')
