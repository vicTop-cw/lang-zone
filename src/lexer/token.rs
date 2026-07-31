// Lang-Zong 编译器 — token.rs
// 词法分析: Token 类型定义
// 对齐 hermes/00-最终语法规范.md
//
// 注：Lexer 实现在 lexer.rs 中，本文件仅定义 Token 枚举与辅助函数。
// 2026-07-31：删除了零引用的冗余 Lexer 结构体（原 L122-871），该死代码含有旧版 P0 bug（unwap_or(0)）。

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── 声明关键字 ──
    Def, Struct, Enum, Trait, Impl, Const, Mut, Ref, Owned, Let,
    Iterator,      // iterator 关键字（生成器函数定义）

    // ── 控制流 ──
    If, Elif, Else, Match, Case, Guard,
    For, In, While, Loop, Break, Continue, Return, With,
    Defer,

    // ── 异常 ──
    Try, Catch, Finally, Raise, Raises,

    // ── 测试 ──
    Test, Assert, Suite, Setup, Teardown, Check,

    // ── 并发 ──
    Async, Await, Spawn, Go, Select,

    // ── 迭代 ──
    Yield,

    // ── 导入 ──
    Import, From, As,

    // ── 类型/泛型 ──
    Where, Self_, Duck,

    // ── 宏/编译期 ──
    Macro, Comptime,

    // ── 逻辑关键字 ──
    And, Or, Not, Is,

    // ── 字面量 ──
    True, False,

    // ── 字面量值 ──
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    FStrLit(String),       // f"..." 字符串插值
    RawStrLit(String),     // r"..." 原始字符串
    TripleStrLit(String),  // """..."""

    // ── 标识符 ──
    Ident(String),
    MagicMethod(String),   // __xxx__ 魔法方法

    // ── 赋值/比较 ──
    Eq,            // =
    EqEq,          // ==
    NotEq,         // !=
    Lt, Gt, Le, Ge,

    // ── 算术 ──
    Plus, Minus, Star, Slash, Percent, StarStar, // + - * / % **

    // ── 复合赋值 ──
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq, // += -= *= /= %=
    AndEq, OrEq, XorEq, ShlEq, ShrEq, PowEq,    // &= |= ^= <<= >>= **=

    // ── 位运算 ──
    Amp, Pipe_, Caret, Shl, Shr, // & | ^ << >>
    AmpAmp, PipePipe,            // && ||

    // ── 标点 ──
    Colon, Comma, Dot, DotDot, DotDotEq, DotDotDot, Semicolon,
    PathSep,       // :: 命名空间路径分隔
    Arrow,         // -> 函数返回类型 / 函数类型注解 (T) -> U
    FatArrow,      // => match case 箭头（允许内联或换行缩进）
    Pipe,          // |>
    BackPipe,      // <|
    ColonEq,       // := 海象运算符
    LParen, RParen,
    LBrack, RBrack,
    LBrace, RBrace,

    // ── 特殊符号 ──
    At,            // @ 装饰器
    Question,      // ?
    QuestionQuestion, // ??
    SafeNav,       // ?.
    Exclamation,   // !
    Underscore,    // _
    CaretOp,       // ^ 所有权转移（紧贴前导 token：后缀 move / 紧贴 XOR）
    CaretInfix,    // ^ 前置留白：强制中缀 XOR，必须带右操作数（悬空报错）
    Backtick,      // ` 代码字面量（三反引号用于宏 quote 块）
    Dollar,        // $ 宏插值符号
    Tilde,         // ~ 命名参数糖 f(x~) -> f(x = x)

    // ── 构建块专用符号 ──
    LexError(String), // 词法错误（构建块符号留白违规等），由 parse_module 拒绝
    BuildAssign,   // =: 变量构建块
    BuildIndex,    // ^: 索引构建块
    BuildCall,     // ~: 调用构建块
    BuildGen,      // *: 生成器调用构建块

    // ── 缩进虚拟 Token ──
    Indent, Dedent, Newline,

    Eof,
}

/// 判断字符是否为构建块符号所需的"留白边界"：
// 保留向后兼容：规范定义在 src/util/chars.rs
use crate::util::chars;
/// 空格 / 制表符 / 换行 / 回车，或输入边界（None）。
/// 用于强制 `=:` `~:` `*: ` 前后必须留白。
/// 
/// 规范实现在 util::chars::is_build_ws，此处为兼容性重导出。
pub fn is_build_ws(c: Option<char>) -> bool {
    chars::is_build_ws(c)
}
