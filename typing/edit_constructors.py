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

    # Function { name, generics, generic_bounds, ... -> add generic_kinds after generics
    content = re.sub(
        r'Function \{\n(\s+)name: ([^,]+),\n\s+generics: ([^,]+),\n\s+generic_bounds:',
        r'Function {\n\1name: \2,\n\1generics: \3,\n\1generic_kinds: Vec::new(),\n\1generic_bounds:',
        content
    )
    # Single-line Function { name, generics, generic_bounds,... }
    content = re.sub(
        r'Function \{ name: ([^,]+), generics: ([^,]+), generic_bounds:',
        r'Function { name: \1, generics: \2, generic_kinds: Vec::new(), generic_bounds:',
        content
    )

    # StructDef { name, generics, generic_bounds,
    content = re.sub(
        r'StructDef \{\n(\s+)name: ([^,]+),\n\s+generics: ([^,]+),\n\s+generic_bounds:',
        r'StructDef {\n\1name: \2,\n\1generics: \3,\n\1generic_kinds: Vec::new(),\n\1generic_bounds:',
        content
    )
    content = re.sub(
        r'StructDef \{ name: ([^,]+), generics: ([^,]+), generic_bounds:',
        r'StructDef { name: \1, generics: \2, generic_kinds: Vec::new(), generic_bounds:',
        content
    )

    # TraitDef { name, generics, generic_bounds,
    content = re.sub(
        r'TraitDef \{\n(\s+)name: ([^,]+),\n\s+generics: ([^,]+),\n\s+generic_bounds:',
        r'TraitDef {\n\1name: \2,\n\1generics: \3,\n\1generic_kinds: Vec::new(),\n\1generic_bounds:',
        content
    )
    content = re.sub(
        r'TraitDef \{ name: ([^,]+), generics: ([^,]+), generic_bounds:',
        r'TraitDef { name: \1, generics: \2, generic_kinds: Vec::new(), generic_bounds:',
        content
    )

    # ImplDef { trait_name, type_name, generics, generic_bounds,
    content = re.sub(
        r'ImplDef \{\n(\s+)trait_name: ([^,]+),\n\s+type_name: ([^,]+),\n\s+generics: ([^,]+),\n\s+generic_bounds:',
        r'ImplDef {\n\1trait_name: \2,\n\1type_name: \3,\n\1generics: \4,\n\1generic_kinds: Vec::new(),\n\1generic_bounds:',
        content
    )
    content = re.sub(
        r'ImplDef \{ trait_name: ([^,]+), type_name: ([^,]+), generics: ([^,]+), generic_bounds:',
        r'ImplDef { trait_name: \1, type_name: \2, generics: \3, generic_kinds: Vec::new(), generic_bounds:',
        content
    )

    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)
    print(f'Updated {path}')

print('OK')
