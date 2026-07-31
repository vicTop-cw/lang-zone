$syntax = @'
// Minimal AST: imports/exports/function signatures/expressions

#[derive(Debug, Clone, PartialEq)]
pub struct ImportStmt {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub items: Vec<String>,
    pub is_from: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportTarget {
    Rust,
    Python,
    C,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportDecl {
    pub name: String,
    pub targets: Vec<ExportTarget>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Ident(String),
    BinOp(BinOp, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuncSig {
    pub name: String,
    pub params: Vec<(String, String)>,
    /// LZ return type string (e.g. "int" / "List<int>" / None)
    pub ret: Option<String>,
    /// Function body expression, if present.
    pub body: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleImportExport {
    pub imports: Vec<ImportStmt>,
    pub exports: Vec<ExportDecl>,
    pub funcs: Vec<FuncSig>,
}
'@

Set-Content -Path "src/syntax.rs" -Value $syntax -Encoding UTF8
