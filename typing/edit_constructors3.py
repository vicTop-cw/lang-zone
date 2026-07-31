import re

files = [
    r'e:\IDEProjects\AI\lang-zone\src\parser\parser.rs',
    r'e:\IDEProjects\AI\lang-zone\src\typer\mod.rs',
    r'e:\IDEProjects\AI\lang-zone\src\strict.rs',
    r'e:\IDEProjects\AI\lang-zone\src\codegen\mod.rs',
]

for path in files:
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()

    # name: ..., generics: vec![], generic_bounds: vec![], ...
    content = re.sub(
        r'(Function\s*\{[^}]*?)generics:\s*vec!\[\],\s*generic_bounds:\s*vec!\[\],',
        r'\1generics: vec![], generic_kinds: vec![], generic_bounds: vec![],',
        content,
        flags=re.DOTALL
    )
    content = re.sub(
        r'(StructDef\s*\{[^}]*?)generics:\s*vec!\[\],\s*generic_bounds:\s*vec!\[\],',
        r'\1generics: vec![], generic_kinds: vec![], generic_bounds: vec![],',
        content,
        flags=re.DOTALL
    )
    content = re.sub(
        r'(TraitDef\s*\{[^}]*?)generics:\s*vec!\[\],\s*generic_bounds:\s*vec!\[\],',
        r'\1generics: vec![], generic_kinds: vec![], generic_bounds: vec![],',
        content,
        flags=re.DOTALL
    )
    content = re.sub(
        r'(ImplDef\s*\{[^}]*?)generics:\s*vec!\[\],\s*generic_bounds:\s*vec!\[\],',
        r'\1generics: vec![], generic_kinds: vec![], generic_bounds: vec![],',
        content,
        flags=re.DOTALL
    )

    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)
    print(f'Updated {path}')

print('OK')
