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

    # Function { name, generics, generic_bounds, generic_defaults, ...
    content = re.sub(
        r'Function \{\s*name,\s*generics,\s*generic_bounds,\s*generic_defaults,',
        r'Function { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults,',
        content
    )

    # StructDef { name, generics, generic_bounds, generic_defaults, ...
    content = re.sub(
        r'StructDef \{\s*name,\s*generics,\s*generic_bounds,\s*generic_defaults,',
        r'StructDef { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults,',
        content
    )

    # TraitDef { name, generics, generic_bounds, generic_defaults, ...
    content = re.sub(
        r'TraitDef \{\s*name,\s*generics,\s*generic_bounds,\s*generic_defaults,',
        r'TraitDef { name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults,',
        content
    )

    # ImplDef { trait_name, type_name, generics, generic_bounds, generic_defaults, ...
    content = re.sub(
        r'ImplDef \{\s*trait_name,\s*type_name,\s*generics,\s*generic_bounds,\s*generic_defaults,',
        r'ImplDef { trait_name, type_name, generics, generic_kinds: Vec::new(), generic_bounds, generic_defaults,',
        content
    )

    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)
    print(f'Updated {path}')

print('OK')
