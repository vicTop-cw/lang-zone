// Lang-Zone 编译器 — tests/lexer_parser_core.rs
// 测试强化（阶段2b / FIST 任务 T4.4）：词法/语法核心路径单元测试
//
// 覆盖：
// - 词法边界：数字（int/float/hex/溢出）、字符串（含转义/未终止）、运算符、注释、关键字
// - 语法核心：def/struct/import/顶层语句/if-else/while/for/try-catch 的结构解析
// - AST 结构断言：函数名/参数/返回类型/结构体字段/导入路径

use lang_zone::ast::Module;
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;

fn lex(src: &str) -> Vec<lang_zone::lexer::Token> {
    Lexer::new(src).tokenize()
}

fn parse(src: &str) -> Result<Module, String> {
    let toks = lex(src);
    let mut p = Parser::new(toks);
    p.parse_module()
}

fn parse_ok(src: &str) -> Module {
    parse(src).expect("parse should succeed")
}

#[test]
fn lex_numbers_and_strings() {
    let toks = lex(r#"42 3.14 0x1F "hello" "a\nb" 'c' true false"#);
    let kinds: Vec<String> = toks.iter().map(|t| format!("{:?}", t)).collect();
    // 至少包含数字与字符串字面量 token
    assert!(kinds.iter().any(|k| k.contains("42")), "缺整数: {kinds:?}");
    assert!(kinds.iter().any(|k| k.contains("3.14")), "缺浮点: {kinds:?}");
    assert!(kinds.iter().any(|k| k.contains("hello")), "缺字符串: {kinds:?}");
}

#[test]
fn lex_rejects_invalid_int_overflow() {
    let toks = lex("99999999999999999999999999999");
    // 溢出整数被 lexer 拒绝（LexError 进入 token 流），不得成为合法数值 token
    assert!(
        toks.iter().any(|t| format!("{:?}", t).contains("LexError")),
        "溢出整数应产生 LexError: {toks:?}"
    );
    assert!(
        !toks.iter().any(|t| format!("{:?}", t).starts_with("IntLit(")),
        "溢出整数不应成为合法 IntLit token: {toks:?}"
    );
}

#[test]
fn lex_rejects_unterminated_string() {
    let toks = lex("\"abc");
    assert!(
        toks.iter().any(|t| format!("{:?}", t).contains("error") || format!("{:?}", t).contains("Error")),
        "未终止字符串应产生错误 token: {toks:?}"
    );
}

#[test]
fn lex_comments_and_operators() {
    let toks = lex("// 行注释\nx = 1 // 尾注释\n");
    let kinds: Vec<String> = toks.iter().map(|t| format!("{:?}", t)).collect();
    // 注释内容不应出现在 token 流
    assert!(!kinds.iter().any(|k| k.contains("行注释")), "注释被泄漏: {kinds:?}");
    // 运算符应识别（语言用 `=` 赋值，token 为 Eq）
    assert!(kinds.iter().any(|k| k.contains("Eq")), "缺赋值运算符 Eq: {kinds:?}");
}

#[test]
fn parse_def_signature() {
    let m = parse_ok("def add(a: int, b: int) -> int =\n    a + b\n");
    assert_eq!(m.functions.len(), 1, "应解析出 1 个函数");
    let f = &m.functions[0];
    assert_eq!(f.name, "add");
    assert_eq!(f.params.len(), 2);
    // 类型标注在语法层规范化为底层类型（int → i64，float → f64）
    let rt = f.return_type.as_ref().map(|t| t.to_string()).unwrap_or_default();
    assert!(
        rt == "int" || rt == "i64",
        "返回类型应为 int/i64，实际: {rt}"
    );
}

#[test]
fn parse_struct_fields() {
    let m = parse_ok("struct Rect =\n    w: float\n    h: float\n");
    assert_eq!(m.structs.len(), 1, "应解析出 1 个结构体");
    let s = &m.structs[0];
    assert_eq!(s.name, "Rect");
    assert_eq!(s.fields.len(), 2, "Rect 应有 2 字段");
    assert_eq!(s.fields[0].name, "w");
}

#[test]
fn parse_import_path() {
    let m = parse_ok("import std.math\nimport services.client\n");
    assert_eq!(m.imports.len(), 2, "应解析出 2 条导入");
    let first = &m.imports[0];
    let joined: Vec<String> = first.path.iter().map(|s| s.clone()).collect();
    assert_eq!(joined, vec!["std", "math"]);
}

#[test]
fn parse_top_level_statements_and_control_flow() {
    let m = parse_ok(
        "def main() =\n    let x = 10\n    if x > 5:\n        print(\"big\")\n    else:\n        print(\"small\")\n    while x > 0:\n        x = x - 1\n    for i in 0..5:\n        print(i)\n",
    );
    assert_eq!(m.functions.len(), 1);
    assert!(!m.functions[0].body.is_empty(), "main 应有函数体");
}

#[test]
fn parse_try_catch() {
    let m = parse_ok(
        "def main() =\n    try:\n        let v = 1\n    catch e:\n        print(e)\n",
    );
    assert_eq!(m.functions.len(), 1);
}

#[test]
fn parse_enum_like() {
    let m = parse_ok("enum Color =\n    Red\n    Green\n    Blue\n");
    let enums: Vec<&_> = m.structs.iter().filter(|s| s.is_enum).collect();
    assert_eq!(enums.len(), 1, "应解析出 1 个枚举");
    assert_eq!(enums[0].name, "Color");
}

#[test]
fn parse_rejects_malformed() {
    let cases = [
        "def f( = 1\n",                 // 参数缺名
        "def f() =\n    let = 1\n",      // let 缺变量名
        "struct = \n",                   // struct 缺名
        "def f() =\n    if x > 1\n        print(1)\n", // if 缺冒号
        "def f() =\n    print(1 + )\n",  // 表达式缺操作数
    ];
    for (i, src) in cases.iter().enumerate() {
        assert!(parse(src).is_err(), "case#{i} 应被拒绝: {src}");
    }
}
