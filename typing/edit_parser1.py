path = r'e:\IDEProjects\AI\lang-zone\src\parser\parser.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# 1. update imports
old_imports = '''use crate::lexer::Token;
use crate::types::Type;
use crate::ast::*;'''
new_imports = '''use crate::lexer::Token;
use crate::types::{Type, Kind};
use crate::ast::*;
use std::collections::HashMap;'''
if old_imports not in content:
    print('IMPORTS OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_imports, new_imports)

# 2. update Parser struct
old_struct = '''pub struct Parser {
    tokens: Vec<Token>,
    pub(in crate::parser) pos: usize,
    pub(in crate::parser) pending_gt: usize, // 处理嵌套泛型 >> 分裂为两个 >
    pub(super) comptime_depth: usize, // comptime 表达式前缀嵌套深度（防止递归解析）
}'''
new_struct = '''pub struct Parser {
    tokens: Vec<Token>,
    pub(in crate::parser) pos: usize,
    pub(in crate::parser) pending_gt: usize, // 处理嵌套泛型 >> 分裂为两个 >
    pub(super) comptime_depth: usize, // comptime 表达式前缀嵌套深度（防止递归解析）
    /// HKT: 当前作用域内泛型参数的 kind 栈（函数/struct/trait/impl 各有一层）
    generic_kinds: Vec<HashMap<String, Kind>>,
}'''
if old_struct not in content:
    print('STRUCT OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_struct, new_struct)

# 3. update new()
old_new = '''    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, pending_gt: 0, comptime_depth: 0 }
    }'''
new_new = '''    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, pending_gt: 0, comptime_depth: 0, generic_kinds: Vec::new() }
    }

    /// 进入一层泛型作用域（函数/struct/trait/impl）
    fn push_generic_kinds(&mut self, kinds: &[(String, Kind)]) {
        let mut map = HashMap::new();
        for (name, kind) in kinds {
            map.insert(name.clone(), kind.clone());
        }
        self.generic_kinds.push(map);
    }

    /// 离开一层泛型作用域
    fn pop_generic_kinds(&mut self) {
        self.generic_kinds.pop();
    }

    /// 查询当前作用域中泛型参数的 kind
    fn current_generic_kind(&self, name: &str) -> Option<&Kind> {
        self.generic_kinds.last().and_then(|m| m.get(name))
    }'''
if old_new not in content:
    print('NEW OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_new, new_new)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('OK')
