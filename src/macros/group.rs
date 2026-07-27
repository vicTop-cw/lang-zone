// Lang-Zong 编译器 — macros/group.rs
// 宏系统核心类型：Tokens / TokenGroupKind / TokenTree / 分组分类算法

use crate::lexer::Token;

// ──────────────── 分隔符类型 ────────────────

/// 分隔符组类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Paren,    // ( )
    Bracket,  // [ ]
    Brace,    // { }
}

// ──────────────── TokenTree 层级结构 ────────────────

/// Token 树节点 —— 将扁平 token 流组织为嵌套的层级结构。
///
/// ```
/// // def foo(x: int) -> int = x + 1
/// // -> Group(Paren, [Atom(Ident("x")), Atom(Colon), Atom(Ident("int"))])
/// //   Atom(Arrow)
/// //   Atom(Ident("int"))
/// //   Atom(Eq)
/// //   Group(Brace, [Atom(Ident("x")), Atom(Plus), Atom(IntLit(1))])
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TokenTree {
    /// 单个 token
    Atom(Token),
    /// 分隔符组（含子 token 树）
    Group(Delimiter, Vec<TokenTree>),
}

impl TokenTree {
    /// 从扁平 token 序列解析为 TokenTree 列表。
    /// 自动将匹配的 `()`, `[]`, `{}` 组织为 Group 节点。
    pub fn parse_all(tokens: &[Token]) -> Result<Vec<TokenTree>, String> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let (tree, next) = Self::parse_one(tokens, i)?;
            result.push(tree);
            i = next;
        }
        Ok(result)
    }

    /// 解析单个节点（可能是 Atom 或 Group）
    fn parse_one(tokens: &[Token], i: usize) -> Result<(TokenTree, usize), String> {
        if i >= tokens.len() {
            return Err("unexpected end of input".to_string());
        }
        match &tokens[i] {
            Token::LParen => {
                let (children, next) = Self::parse_delimited(tokens, i + 1, Token::LParen, Token::RParen, Delimiter::Paren)?;
                Ok((TokenTree::Group(Delimiter::Paren, children), next))
            }
            Token::LBrack => {
                let (children, next) = Self::parse_delimited(tokens, i + 1, Token::LBrack, Token::RBrack, Delimiter::Bracket)?;
                Ok((TokenTree::Group(Delimiter::Bracket, children), next))
            }
            Token::LBrace => {
                let (children, next) = Self::parse_delimited(tokens, i + 1, Token::LBrace, Token::RBrace, Delimiter::Brace)?;
                Ok((TokenTree::Group(Delimiter::Brace, children), next))
            }
            _ => {
                Ok((TokenTree::Atom(tokens[i].clone()), i + 1))
            }
        }
    }

    /// 解析分隔符组内的内容（递归）
    fn parse_delimited(
        tokens: &[Token],
        start: usize,
        _open: Token,
        close: Token,
        _delim: Delimiter,
    ) -> Result<(Vec<TokenTree>, usize), String> {
        let mut children = Vec::new();
        let mut i = start;
        let mut depth: i32 = 1;
        while i < tokens.len() {
            if tokens[i] == close {
                depth -= 1;
                if depth == 0 {
                    return Ok((children, i + 1)); // 跳过闭合分隔符
                }
                children.push(TokenTree::Atom(tokens[i].clone()));
            } else {
                // 递归处理嵌套组
                match &tokens[i] {
                    Token::LParen => {
                        let (inner, next) = Self::parse_delimited(
                            tokens, i + 1, Token::LParen, Token::RParen, Delimiter::Paren,
                        )?;
                        children.push(TokenTree::Group(Delimiter::Paren, inner));
                        i = next;
                        continue;
                    }
                    Token::LBrack => {
                        let (inner, next) = Self::parse_delimited(
                            tokens, i + 1, Token::LBrack, Token::RBrack, Delimiter::Bracket,
                        )?;
                        children.push(TokenTree::Group(Delimiter::Bracket, inner));
                        i = next;
                        continue;
                    }
                    Token::LBrace => {
                        let (inner, next) = Self::parse_delimited(
                            tokens, i + 1, Token::LBrace, Token::RBrace, Delimiter::Brace,
                        )?;
                        children.push(TokenTree::Group(Delimiter::Brace, inner));
                        i = next;
                        continue;
                    }
                    _ => {
                        children.push(TokenTree::Atom(tokens[i].clone()));
                    }
                }
            }
            i += 1;
        }
        Err(format!("unclosed delimiter {:?}", _delim))
    }

    /// 将 TokenTree 列表展平为 Token 流
    pub fn flatten(trees: &[TokenTree]) -> Vec<Token> {
        let mut result = Vec::new();
        for tree in trees {
            tree.flatten_into(&mut result);
        }
        result
    }

    fn flatten_into(&self, output: &mut Vec<Token>) {
        match self {
            TokenTree::Atom(t) => output.push(t.clone()),
            TokenTree::Group(delim, children) => {
                let (open, close) = match delim {
                    Delimiter::Paren => (Token::LParen, Token::RParen),
                    Delimiter::Bracket => (Token::LBrack, Token::RBrack),
                    Delimiter::Brace => (Token::LBrace, Token::RBrace),
                };
                output.push(open);
                for child in children {
                    child.flatten_into(output);
                }
                output.push(close);
            }
        }
    }

    /// 查找第一个指定类型的分隔符组
    pub fn find_group(&self, delim: Delimiter) -> Option<&Vec<TokenTree>> {
        match self {
            TokenTree::Group(d, children) if *d == delim => Some(children),
            TokenTree::Group(_, children) => {
                for child in children {
                    if let Some(found) = child.find_group(delim) {
                        return Some(found);
                    }
                }
                None
            }
            TokenTree::Atom(_) => None,
        }
    }

    /// 查找最后一个指定类型的分隔符组
    pub fn find_last_group(&self, delim: Delimiter) -> Option<&Vec<TokenTree>> {
        let mut last: Option<&Vec<TokenTree>> = None;
        self.collect_groups(delim, &mut last);
        last
    }

    fn collect_groups<'a>(&'a self, delim: Delimiter, last: &mut Option<&'a Vec<TokenTree>>) {
        match self {
            TokenTree::Group(d, children) => {
                if *d == delim {
                    *last = Some(children);
                }
                for child in children {
                    child.collect_groups(delim, last);
                }
            }
            TokenTree::Atom(_) => {}
        }
    }

    /// 收集所有指定类型的分隔符组
    pub fn find_all_groups(&self, delim: Delimiter) -> Vec<&Vec<TokenTree>> {
        let mut result = Vec::new();
        self.collect_all_groups(delim, &mut result);
        result
    }

    fn collect_all_groups<'a>(&'a self, delim: Delimiter, result: &mut Vec<&'a Vec<TokenTree>>) {
        match self {
            TokenTree::Group(d, children) => {
                if *d == delim {
                    result.push(children);
                }
                for child in children {
                    child.collect_all_groups(delim, result);
                }
            }
            TokenTree::Atom(_) => {}
        }
    }

    /// 调试用的树形字符串表示
    pub fn to_tree_string(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match self {
            TokenTree::Atom(t) => format!("{}{:?}", pad, t),
            TokenTree::Group(delim, children) => {
                let name = match delim {
                    Delimiter::Paren => "Paren",
                    Delimiter::Bracket => "Bracket",
                    Delimiter::Brace => "Brace",
                };
                let mut s = format!("{}Group({}\n", pad, name);
                for child in children {
                    s.push_str(&child.to_tree_string(indent + 1));
                    s.push('\n');
                }
                s.push_str(&format!("{})", pad));
                s
            }
        }
    }
}

// ──────────────── Tokens 类型 ────────────────

/// Token 序列 —— 一段尚未解析为 AST 的代码片段。
/// 宏系统的核心数据类型。
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub tokens: Vec<Token>,
    pub kind: TokenGroupKind,
    /// 可选的树形表示（由 token_stream() 调用填充）
    pub tree: Option<Vec<TokenTree>>,
}

impl Tokens {
    pub fn new(tokens: Vec<Token>) -> Self {
        let kind = classify_token_group(&tokens);
        Tokens { tokens, kind, tree: None }
    }

    pub fn empty() -> Self {
        Tokens { tokens: vec![], kind: TokenGroupKind::Any, tree: None }
    }

    /// 创建带有树形结构的 Tokens
    pub fn with_tree(tokens: Vec<Token>, tree: Vec<TokenTree>) -> Self {
        let kind = classify_token_group(&tokens);
        Tokens { tokens, kind, tree: Some(tree) }
    }

    /// 拼接两个 Tokens（拼接后树形结构失效）
    pub fn concat(mut self, other: Tokens) -> Tokens {
        self.tokens.extend(other.tokens);
        self.kind = classify_token_group(&self.tokens);
        self.tree = None; // 拼接后树失效
        self
    }

    /// Token 数量
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// 取第一个 token
    pub fn first(&self) -> Tokens {
        if self.tokens.is_empty() {
            Tokens::empty()
        } else {
            Tokens::new(vec![self.tokens[0].clone()])
        }
    }

    /// 去掉第一个 token 的剩余部分
    pub fn rest(&self) -> Tokens {
        if self.tokens.len() <= 1 {
            Tokens::empty()
        } else {
            Tokens::new(self.tokens[1..].to_vec())
        }
    }

    /// 调试用的字符串表示
    pub fn to_string(&self) -> String {
        self.tokens.iter()
            .map(|t| match t {
                Token::Ident(s) => s.clone(),
                Token::IntLit(n) => n.to_string(),
                Token::FloatLit(f) => f.to_string(),
                Token::StrLit(s) => format!("\"{}\"", s),
                Token::Plus => "+".to_string(),
                Token::Minus => "-".to_string(),
                Token::Star => "*".to_string(),
                Token::Slash => "/".to_string(),
                Token::Eq => "=".to_string(),
                Token::EqEq => "==".to_string(),
                Token::Colon => ":".to_string(),
                Token::Comma => ",".to_string(),
                Token::Dot => ".".to_string(),
                Token::PathSep => "::".to_string(),
                Token::Arrow => "->".to_string(),
                Token::LParen => "(".to_string(),
                Token::RParen => ")".to_string(),
                Token::LBrack => "[".to_string(),
                Token::RBrack => "]".to_string(),
                Token::LBrace => "{".to_string(),
                Token::RBrace => "}".to_string(),
                Token::Lt => "<".to_string(),
                Token::Gt => ">".to_string(),
                Token::Semicolon => ";".to_string(),
                Token::At => "@".to_string(),
                Token::Exclamation => "!".to_string(),
                Token::Dollar => "$".to_string(),
                Token::Def => "def".to_string(),
                Token::Return => "return".to_string(),
                Token::If => "if".to_string(),
                Token::Else => "else".to_string(),
                Token::For => "for".to_string(),
                Token::While => "while".to_string(),
                Token::Match => "match".to_string(),
                Token::Newline => "\n".to_string(),
                Token::Indent => "  ".to_string(),
                _ => format!("{:?}", t).to_lowercase(),
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

// ──────────────── TokenGroupKind ────────────────

/// Token 分组类型 —— 宏内部仅做此分类，不进行完整 AST 构建。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenGroupKind {
    /// 表达式 tokens：1 + 2, foo(), x.y
    Expr,
    /// 声明 tokens：def/struct/enum/trait/impl/const
    Decl,
    /// 语句 tokens：return x, if ..., for ...
    Stmt,
    /// 模式 tokens：Some(x), (a, b), _
    Pattern,
    /// 类型标注 tokens：int, List<str>, Option<int>
    Type,
    /// 未分类（默认）
    Any,
}

// ──────────────── 分类算法 ────────────────

/// 对 Token 序列进行分组分类。
/// 根据首 token 判定属于 Expr / Decl / Stmt / Pattern / Type 中的哪一类。
pub fn classify_token_group(tokens: &[Token]) -> TokenGroupKind {
    // 去掉前导空白 token
    let start = tokens.iter().position(|t| !is_whitespace_token(t));
    let trimmed = match start {
        Some(i) => &tokens[i..],
        None => return TokenGroupKind::Any,
    };

    if trimmed.is_empty() {
        return TokenGroupKind::Any;
    }

    // 声明开头的关键字
    if matches_decl_start(trimmed) {
        return TokenGroupKind::Decl;
    }

    // 语句开头的关键字
    if matches_stmt_start(trimmed) {
        return TokenGroupKind::Stmt;
    }

    // 模式（在 match 上下文中，由调用方判断）
    // 这里只做启发式：Ident + LParen 或 _ 或字面量
    if matches_pattern_start(trimmed) {
        return TokenGroupKind::Pattern;
    }

    // 类型标注
    if matches_type_start(trimmed) {
        return TokenGroupKind::Type;
    }

    // 默认：表达式
    TokenGroupKind::Expr
}

fn is_whitespace_token(t: &Token) -> bool {
    matches!(t, Token::Newline | Token::Indent | Token::Dedent)
}

/// 声明首 token：def / struct / enum / trait / impl / const / macro
fn matches_decl_start(tokens: &[Token]) -> bool {
    matches!(
        tokens.first(),
        Some(Token::Def) | Some(Token::Struct) | Some(Token::Enum)
        | Some(Token::Trait) | Some(Token::Impl) | Some(Token::Const)
        | Some(Token::Macro)
    )
}

/// 语句首 token：return / if / for / while / loop / match / let / guard / try / raise / yield / defer / break / continue
fn matches_stmt_start(tokens: &[Token]) -> bool {
    matches!(
        tokens.first(),
        Some(Token::Return) | Some(Token::If) | Some(Token::For)
        | Some(Token::While) | Some(Token::Loop) | Some(Token::Match)
        | Some(Token::Let) | Some(Token::Guard) | Some(Token::Try)
        | Some(Token::Raise) | Some(Token::Yield) | Some(Token::Defer)
        | Some(Token::Break) | Some(Token::Continue)
        | Some(Token::Assert)
    )
}

/// 模式首 token：_ 或 标识符(（在 match case 上下文中由调用方确认）
fn matches_pattern_start(tokens: &[Token]) -> bool {
    match tokens.first() {
        Some(Token::Underscore) => true,
        Some(Token::Ident(_)) => {
            // 模式一般是 Ident(LParen)，且不应是类型关键字
            tokens.len() > 1 && matches!(tokens.get(1), Some(Token::LParen))
            || tokens.len() == 1
        }
        // 字面量模式仅在 match 上下文中有效，这里不做启发式判断
        _ => false,
    }
}

/// 类型首 token：int / f64 / str / bool / List / Dict / Set / Option / Result
/// 或 标识符< (泛型类型)
fn matches_type_start(tokens: &[Token]) -> bool {
    match tokens.first() {
        // 内置类型关键字
        Some(Token::Ident(s)) if is_type_keyword(s) => true,
        // 泛型类型: Ident + LT
        Some(Token::Ident(_)) if tokens.len() > 1 && matches!(tokens.get(1), Some(Token::Lt)) => true,
        _ => false,
    }
}

fn is_type_keyword(s: &str) -> bool {
    matches!(s, "int" | "f64" | "str" | "bool"
        | "List" | "Dict" | "Set" | "Option" | "Result"
        | "Self" | "self")
}

// ──────────────── 反引号块前缀 ────────────────

/// 反引号块的模式前缀
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktickPrefix {
    /// 普通 ```  (不插值、不展开)
    None,
    /// f```  (插值模式：$(expr) 求值)
    F,
    /// r```  (原始模式：全部保留原样)
    R,
}

// ──────────────── 单元测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_expr() {
        let tokens = vec![Token::IntLit(1), Token::Plus, Token::IntLit(2)];
        assert_eq!(classify_token_group(&tokens), TokenGroupKind::Expr);
    }

    #[test]
    fn test_classify_decl() {
        let tokens = vec![Token::Def, Token::Ident("foo".into()), Token::LParen, Token::RParen];
        assert_eq!(classify_token_group(&tokens), TokenGroupKind::Decl);

        let tokens2 = vec![Token::Struct, Token::Ident("Bar".into())];
        assert_eq!(classify_token_group(&tokens2), TokenGroupKind::Decl);
    }

    #[test]
    fn test_classify_stmt() {
        let tokens = vec![Token::Return, Token::Ident("x".into())];
        assert_eq!(classify_token_group(&tokens), TokenGroupKind::Stmt);

        let tokens2 = vec![Token::If, Token::IntLit(1)];
        assert_eq!(classify_token_group(&tokens2), TokenGroupKind::Stmt);
    }

    #[test]
    fn test_classify_pattern() {
        let tokens = vec![Token::Ident("Some".into()), Token::LParen, Token::Ident("x".into()), Token::RParen];
        assert_eq!(classify_token_group(&tokens), TokenGroupKind::Pattern);

        let tokens2 = vec![Token::Underscore];
        assert_eq!(classify_token_group(&tokens2), TokenGroupKind::Pattern);
    }

    #[test]
    fn test_classify_type() {
        let tokens = vec![Token::Ident("List".into()), Token::Lt, Token::Ident("int".into()), Token::Gt];
        assert_eq!(classify_token_group(&tokens), TokenGroupKind::Type);
    }

    #[test]
    fn test_classify_skip_whitespace() {
        let tokens = vec![Token::Newline, Token::Newline, Token::Return, Token::Ident("x".into())];
        assert_eq!(classify_token_group(&tokens), TokenGroupKind::Stmt);
    }

    #[test]
    fn test_tokens_concat() {
        let a = Tokens::new(vec![Token::Ident("foo".into())]);
        let b = Tokens::new(vec![Token::LParen, Token::RParen]);
        let c = a.concat(b);
        assert_eq!(c.tokens.len(), 3);
    }

    #[test]
    fn test_tokens_first_rest() {
        let t = Tokens::new(vec![
            Token::Ident("a".into()),
            Token::Ident("b".into()),
            Token::Ident("c".into()),
        ]);
        assert_eq!(t.first().to_string(), "a");
        assert_eq!(t.rest().to_string(), "bc");
        assert!(t.rest().rest().rest().is_empty());
    }

    #[test]
    fn test_tokens_empty() {
        let t = Tokens::empty();
        assert!(t.is_empty());
        assert_eq!(t.first(), Tokens::empty());
        assert_eq!(t.rest(), Tokens::empty());
    }

    // ── TokenTree 测试 ──

    #[test]
    fn test_token_tree_parse_simple() {
        let tokens = vec![
            Token::Ident("x".into()),
            Token::Plus,
            Token::IntLit(1),
        ];
        let tree = TokenTree::parse_all(&tokens).unwrap();
        assert_eq!(tree.len(), 3);
        assert_eq!(tree[0], TokenTree::Atom(Token::Ident("x".into())));
        assert_eq!(tree[1], TokenTree::Atom(Token::Plus));
        assert_eq!(tree[2], TokenTree::Atom(Token::IntLit(1)));
    }

    #[test]
    fn test_token_tree_parse_paren_group() {
        let tokens = vec![
            Token::Ident("foo".into()),
            Token::LParen,
            Token::Ident("x".into()),
            Token::Comma,
            Token::IntLit(42),
            Token::RParen,
        ];
        let tree = TokenTree::parse_all(&tokens).unwrap();
        assert_eq!(tree.len(), 2);
        assert!(matches!(&tree[0], TokenTree::Atom(Token::Ident(s)) if s == "foo"));
        assert!(matches!(&tree[1], TokenTree::Group(Delimiter::Paren, _)));
        if let TokenTree::Group(Delimiter::Paren, children) = &tree[1] {
            assert_eq!(children.len(), 3);
        }
    }

    #[test]
    fn test_token_tree_nested_groups() {
        let tokens = vec![
            Token::LParen,
            Token::IntLit(1),
            Token::Plus,
            Token::LParen,
            Token::IntLit(2),
            Token::Star,
            Token::IntLit(3),
            Token::RParen,
            Token::RParen,
        ];
        let tree = TokenTree::parse_all(&tokens).unwrap();
        assert_eq!(tree.len(), 1);
        assert!(matches!(&tree[0], TokenTree::Group(Delimiter::Paren, _)));
        if let TokenTree::Group(Delimiter::Paren, children) = &tree[0] {
            // 1 + (2 * 3) → 3 个顶层节点: Atom(1), Atom(+), Group(Paren, [2, *, 3])
            assert_eq!(children.len(), 3);
            assert!(matches!(&children[2], TokenTree::Group(Delimiter::Paren, _)));
        }
    }

    #[test]
    fn test_token_tree_flatten() {
        let tokens = vec![
            Token::Ident("foo".into()),
            Token::LParen,
            Token::IntLit(1),
            Token::Comma,
            Token::IntLit(2),
            Token::RParen,
        ];
        let tree = TokenTree::parse_all(&tokens).unwrap();
        let flat = TokenTree::flatten(&tree);
        assert_eq!(flat, tokens);
    }

    #[test]
    fn test_token_tree_find_group() {
        let tokens = vec![
            Token::Def,
            Token::Ident("foo".into()),
            Token::LParen,
            Token::Ident("x".into()),
            Token::Colon,
            Token::Ident("int".into()),
            Token::RParen,
            Token::Eq,
            Token::LBrace,
            Token::Ident("x".into()),
            Token::Plus,
            Token::IntLit(1),
            Token::RBrace,
        ];
        let tree = TokenTree::parse_all(&tokens).unwrap();
        // 在顶层查找第一个 Group
        match &tree[0] {
            TokenTree::Group(_, _) => {}
            _ => {
                // 遍历所有节点查找 Brace
                let all_braces: Vec<_> = tree.iter()
                    .flat_map(|t| t.find_all_groups(Delimiter::Brace))
                    .collect();
                assert!(!all_braces.is_empty());
            }
        }
    }
}
