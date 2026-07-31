path = r'e:\IDEProjects\AI\lang-zone\src\ast\decl.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# 1. import Kind
old_import = 'use crate::types::Type;'
new_import = 'use crate::types::{Type, Kind};'
if old_import not in content:
    print('IMPORT OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_import, new_import)

# 2. Add generic_kinds to Function
old_func = '''pub struct Function {
    pub name: String,
    pub generics: Vec<String>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub params: Vec<Param>,'''
new_func = '''pub struct Function {
    pub name: String,
    pub generics: Vec<String>,
    /// 泛型参数 kind: F[_] -> Arrow([Star], Star)
    pub generic_kinds: Vec<(String, Kind)>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub params: Vec<Param>,'''
if old_func not in content:
    print('FUNCTION OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_func, new_func)

# 3. Add generic_kinds to StructDef
old_struct = '''pub struct StructDef {
    pub name: String,
    pub generics: Vec<String>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub fields: Vec<Field>,'''
new_struct = '''pub struct StructDef {
    pub name: String,
    pub generics: Vec<String>,
    /// 泛型参数 kind: F[_] -> Arrow([Star], Star)
    pub generic_kinds: Vec<(String, Kind)>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub fields: Vec<Field>,'''
if old_struct not in content:
    print('STRUCT OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_struct, new_struct)

# 4. Add generic_kinds to TraitDef
old_trait = '''pub struct TraitDef {
    pub name: String,
    pub generics: Vec<String>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub methods: Vec<Function>,'''
new_trait = '''pub struct TraitDef {
    pub name: String,
    pub generics: Vec<String>,
    /// 泛型参数 kind: F[_] -> Arrow([Star], Star)
    pub generic_kinds: Vec<(String, Kind)>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub methods: Vec<Function>,'''
if old_trait not in content:
    print('TRAIT OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_trait, new_trait)

# 5. Add generic_kinds to ImplDef
old_impl = '''pub struct ImplDef {
    pub trait_name: Option<String>,
    pub type_name: String,
    pub generics: Vec<String>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub where_clause: Vec<WhereBound>,'''
new_impl = '''pub struct ImplDef {
    pub trait_name: Option<String>,
    pub type_name: String,
    pub generics: Vec<String>,
    /// 泛型参数 kind: F[_] -> Arrow([Star], Star)
    pub generic_kinds: Vec<(String, Kind)>,
    /// 泛型约束在定义点: T: Clone + Ord
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认值: T = i64
    pub generic_defaults: Vec<(String, Type)>,
    pub where_clause: Vec<WhereBound>,'''
if old_impl not in content:
    print('IMPL OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_impl, new_impl)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('OK')
