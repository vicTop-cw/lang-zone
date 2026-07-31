// Lang-Zone 编译器 — ir/mod.rs
// LZIR-H 模块入口
//
// LZIR（Lang-Zone Intermediate Representation）是编译器的跨后端共享契约。
// 前端产出 LZIR，后端只消费 LZIR 发射目标代码。
//
// 形态：强类型树 / ANF 风格，每 Expr 携带 IrType + Span。

pub mod types;
pub mod node;
pub mod display;
pub mod builder;
pub mod codegen;
pub mod codegen_cython;

pub use builder::build_ir;

/// LZIR-H 版本号（节点兼容性标识）
pub const IR_VERSION: u32 = 1;

// ── IrModule 顶层结构 ──

/// LZIR 顶层模块 — 一个 .lz 文件编译后的 IR 根节点
#[derive(Debug, Clone)]
pub struct IrModule {
    pub name: String,
    pub directive: node::ModuleDirective,
    pub items: Vec<node::Item>,
    pub prelude: Vec<String>,
    pub version: u32,
}

impl IrModule {
    pub fn new(name: String) -> Self {
        IrModule {
            name,
            directive: node::ModuleDirective::default(),
            items: vec![],
            prelude: vec![],
            version: IR_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::macros::expand::{extract_macro_defs, MacroExpander};

    /// 编译 LZ 源码字符串 → AST → IR
    fn lz_to_ir(source: &str) -> Result<IrModule, String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();

        let (registry, _ranges) = extract_macro_defs(&tokens)
            .map_err(|e| format!("Macro error: {e}"))?;
        let expander = MacroExpander::new(registry);
        let expanded = expander.expand(&tokens)
            .map_err(|e| format!("Expand error: {e}"))?;

        let mut parser = Parser::new(expanded);
        let module = parser.parse_module()
            .map_err(|e| format!("Parse error: {e}"))?;

        builder::build_ir(&module)
            .map_err(|e| format!("IR build error: {e}"))
    }

    #[test]
    fn ir_simple_function() {
        let source = "
def add(x: int, y: int) -> int =
    x + y
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("LZIR v1"));
        assert!(text.contains("fn add"));
        assert!(text.contains("x: int"));
        assert!(text.contains("y: int"));
    }

    #[test]
    fn ir_let_binding() {
        let source = "
def demo() -> int =
    let x = 42
    let y = x + 1
    y
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("LZIR v1"));
        assert!(text.contains("fn demo"));
    }

    #[test]
    fn ir_if_else() {
        let source = "
def check_val(x: int) -> str =
    if x > 0:
        \"positive\"
    else:
        \"non-positive\"
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("fn check_val"));
    }

    #[test]
    fn ir_struct_def() {
        let source = "
struct Point =
    x: int
    y: int

def dist(p: Point) -> f64 =
    0.0
";
        let ir = lz_to_ir(source).expect("should compile");
        let text = format!("{ir}");
        assert!(text.contains("struct Point"));
    }
}
