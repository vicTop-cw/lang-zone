// Lang-Zong 编译器 — macros/pattern.rs
// Token 模式匹配引擎：_ / :ident / $name / ... 模式 DSL

use crate::lexer::Token;
use std::collections::HashMap;

// ──────────────── TokenPattern ────────────────

/// Token 匹配模式，由特殊 token 序列解析而来。
#[derive(Debug, Clone, PartialEq)]
pub enum TokenPattern {
    /// `_` — 匹配任意单个 token
    Wildcard,
    /// 精确匹配某个 token
    Exact(Token),
    /// `:ident` — 匹配任意 Ident token
    TypeIdent,
    /// `:int` — 匹配任意 IntLit token
    TypeInt,
    /// `:str` — 匹配任意 StrLit token
    TypeStr,
    /// `:bool` — 匹配 True 或 False token
    TypeBool,
    /// `$name` — 匹配并捕获任意单个 token 到变量 name
    Capture(String),
    /// `...` — 前一个模式的 0 次或多次重复
    Repeat(Box<TokenPattern>),
    /// 序列 — 按顺序匹配多个子模式
    Seq(Vec<TokenPattern>),
}

/// 捕获组
#[derive(Debug, Clone, Default)]
pub struct Captures {
    pub groups: HashMap<String, Vec<Token>>,
}

impl Captures {
    pub fn new() -> Self {
        Captures { groups: HashMap::new() }
    }

    pub fn capture(&mut self, name: &str, token: Token) {
        self.groups.entry(name.to_string()).or_default().push(token);
    }

    pub fn get(&self, name: &str) -> Option<&Vec<Token>> {
        self.groups.get(name)
    }

    pub fn merge(&mut self, other: Captures) {
        for (k, v) in other.groups {
            self.groups.entry(k).or_default().extend(v);
        }
    }
}

// ──────────────── 模式解析 ────────────────

impl TokenPattern {
    /// 从 token 序列解析模式。
    ///
    /// 特殊 token：
    /// - `_` (Underscore) → Wildcard
    /// - `:ident` (Colon + Ident("ident")) → TypeIdent
    /// - `:int` (Colon + Ident("int")) → TypeInt
    /// - `:str` (Colon + Ident("str")) → TypeStr
    /// - `:bool` (Colon + Ident("bool")) → TypeBool
    /// - `$name` (Dollar + Ident) → Capture("name")
    /// - `...` (DotDotDot) → Repeat(prev)
    /// - 其他 → Exact(Token)
    pub fn parse(tokens: &[Token]) -> Result<TokenPattern, String> {
        if tokens.is_empty() {
            return Ok(TokenPattern::Seq(vec![]));
        }
        let (pattern, _) = Self::parse_seq(tokens, 0)?;
        Ok(pattern)
    }

    fn parse_seq(tokens: &[Token], start: usize) -> Result<(TokenPattern, usize), String> {
        let mut patterns: Vec<TokenPattern> = Vec::new();
        let mut i = start;
        while i < tokens.len() {
            let (pat, next) = Self::parse_one(tokens, i)?;
            patterns.push(pat);
            i = next;
        }
            if patterns.len() == 1 {
                Ok((patterns.remove(0), i))
            } else {
                // 先折叠 Repeat，再返回
                let folded = Self::fold_repeat(&patterns);
                Ok((folded, i))
            }
    }

    fn parse_one(tokens: &[Token], i: usize) -> Result<(TokenPattern, usize), String> {
        if i >= tokens.len() {
            return Err("unexpected end of pattern".to_string());
        }
        match &tokens[i] {
            // `_` → Wildcard
            Token::Underscore => {
                Ok((TokenPattern::Wildcard, i + 1))
            }
            // `:ident` / `:int` / `:str` / `:bool` → Type*
            Token::Colon => {
                if i + 1 >= tokens.len() {
                    return Err("expected type name after ':'".to_string());
                }
                match &tokens[i + 1] {
                    Token::Ident(s) => {
                        let pat = match s.as_str() {
                            "ident" => TokenPattern::TypeIdent,
                            "int" => TokenPattern::TypeInt,
                            "str" => TokenPattern::TypeStr,
                            "bool" => TokenPattern::TypeBool,
                            _ => return Err(format!("unknown type pattern ':{}'", s)),
                        };
                        Ok((pat, i + 2))
                    }
                    _ => Err("expected identifier after ':'".to_string()),
                }
            }
            // `$name` → Capture
            Token::Dollar => {
                if i + 1 >= tokens.len() {
                    return Err("expected identifier after '$'".to_string());
                }
                match &tokens[i + 1] {
                    Token::Ident(name) => {
                        Ok((TokenPattern::Capture(name.clone()), i + 2))
                    }
                    _ => Err("expected identifier after '$'".to_string()),
                }
            }
            // `...` → Repeat (修饰前一个模式)
            Token::DotDotDot => {
                Err("'...' must follow a pattern element, not stand alone".to_string())
            }
            // 其他 → Exact(Token)
            other => {
                Ok((TokenPattern::Exact(other.clone()), i + 1))
            }
        }
    }

    /// 合并 Repeat：如果前一个模式后紧跟 `...`，则包装为 Repeat
    pub fn fold_repeat(patterns: &[TokenPattern]) -> TokenPattern {
        let mut result: Vec<TokenPattern> = Vec::new();
        let mut i = 0;
        while i < patterns.len() {
            if i + 1 < patterns.len() && matches!(&patterns[i + 1], TokenPattern::Exact(Token::DotDotDot)) {
                let repeated = Box::new(patterns[i].clone());
                result.push(TokenPattern::Repeat(repeated));
                i += 2; // 跳过模式和 ...
            } else {
                result.push(patterns[i].clone());
                i += 1;
            }
        }
        if result.len() == 1 {
            result.remove(0)
        } else {
            TokenPattern::Seq(result)
        }
    }
}

// ──────────────── 匹配引擎 ────────────────

impl TokenPattern {
    /// 从 tokens[start..] 开始匹配，返回 (消耗的 token 数, 捕获组)。
    /// 返回 None 表示匹配失败。
    pub fn match_from(&self, tokens: &[Token], start: usize) -> Option<(usize, Captures)> {
        let mut captures = Captures::new();
        let consumed = self.match_inner(tokens, start, &mut captures)?;
        Some((consumed, captures))
    }

    fn match_inner(&self, tokens: &[Token], start: usize, captures: &mut Captures) -> Option<usize> {
        if start > tokens.len() {
            return None;
        }
        match self {
            TokenPattern::Wildcard => {
                if start < tokens.len() {
                    Some(1)
                } else {
                    None
                }
            }
            TokenPattern::Exact(expected) => {
                if start < tokens.len() && &tokens[start] == expected {
                    Some(1)
                } else {
                    None
                }
            }
            TokenPattern::TypeIdent => {
                if start < tokens.len() && matches!(&tokens[start], Token::Ident(_)) {
                    Some(1)
                } else {
                    None
                }
            }
            TokenPattern::TypeInt => {
                if start < tokens.len() && matches!(&tokens[start], Token::IntLit(_)) {
                    Some(1)
                } else {
                    None
                }
            }
            TokenPattern::TypeStr => {
                if start < tokens.len() && matches!(&tokens[start], Token::StrLit(_)) {
                    Some(1)
                } else {
                    None
                }
            }
            TokenPattern::TypeBool => {
                if start < tokens.len() && matches!(&tokens[start], Token::True | Token::False) {
                    Some(1)
                } else {
                    None
                }
            }
            TokenPattern::Capture(name) => {
                if start < tokens.len() {
                    captures.capture(name, tokens[start].clone());
                    Some(1)
                } else {
                    None
                }
            }
            TokenPattern::Repeat(inner) => {
                // 贪婪匹配：尽可能多地匹配
                let mut total = 0;
                let mut current = start;
                while let Some(n) = inner.match_inner(tokens, current, captures) {
                    total += n;
                    current += n;
                    if n == 0 { break; } // 防止无限循环
                }
                Some(total)
            }
            TokenPattern::Seq(patterns) => {
                let mut total = 0;
                let mut current = start;
                for pat in patterns {
                    match pat.match_inner(tokens, current, captures) {
                        Some(n) => {
                            total += n;
                            current += n;
                        }
                        None => return None,
                    }
                }
                Some(total)
            }
        }
    }

    /// 查找所有匹配位置（不重叠），返回 (start, end, captures) 列表
    pub fn find_all(&self, tokens: &[Token]) -> Vec<(usize, usize, Captures)> {
        let mut results = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            if let Some((consumed, captures)) = self.match_from(tokens, i) {
                results.push((i, i + consumed, captures));
                if consumed == 0 {
                    i += 1; // 防止无限循环
                } else {
                    i += consumed;
                }
            } else {
                i += 1;
            }
        }
        results
    }
}

// ──────────────── 替换引擎 ────────────────

/// 替换规则：from_pattern => to_template
#[derive(Debug, Clone)]
pub struct ReplaceRule {
    pub from: TokenPattern,
    pub to: Vec<Token>,  // 模板 tokens，$name 会被替换
}

impl ReplaceRule {
    /// 解析 `from => to` 形式的 token 序列
    /// 格式: from_tokens, Arrow/FatArrow, to_tokens
    pub fn parse(tokens: &[Token]) -> Result<Vec<ReplaceRule>, String> {
        let mut rules = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            // 找到 =>
            let arrow_pos = tokens[i..].iter().position(|t| matches!(t, Token::Arrow | Token::FatArrow));
            let arrow_pos = match arrow_pos {
                Some(p) => i + p,
                None => break,
            };
            let from_tokens = &tokens[i..arrow_pos];
            let to_start = arrow_pos + 1;
            // 找到下一个规则分隔符（逗号后的 next from）
            let to_end = tokens[to_start..].iter().position(|t| {
                // 简单启发：在顶层逗号处分割
                // 更精确的做法是括号匹配，但这里简化
                matches!(t, Token::Comma)
            }).map(|p| to_start + p).unwrap_or(tokens.len());

            let to_tokens = tokens[to_start..to_end].to_vec();

            let from_pattern = TokenPattern::parse(from_tokens)?;
            rules.push(ReplaceRule { from: from_pattern, to: to_tokens });

            i = if to_end < tokens.len() { to_end + 1 } else { to_end };
        }
        Ok(rules)
    }

    /// 应用替换模板：将 $name 替换为捕获组中的值
    pub fn apply_template(template: &[Token], captures: &Captures) -> Vec<Token> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < template.len() {
            if template[i] == Token::Dollar && i + 1 < template.len() {
                if let Token::Ident(name) = &template[i + 1] {
                    if let Some(tokens) = captures.get(name) {
                        result.extend(tokens.clone());
                        i += 2;
                        continue;
                    }
                }
            }
            result.push(template[i].clone());
            i += 1;
        }
        result
    }
}

/// 执行替换：在 source 中查找所有匹配，用规则替换
pub fn apply_replace(
    source: &[Token],
    rules: &[ReplaceRule],
) -> Vec<Token> {
    if rules.is_empty() {
        return source.to_vec();
    }
    let mut result: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < source.len() {
        let mut matched = false;
        for rule in rules {
            if let Some((consumed, captures)) = rule.from.match_from(source, i) {
                let replacement = ReplaceRule::apply_template(&rule.to, &captures);
                result.extend(replacement);
                i += consumed;
                matched = true;
                break;
            }
        }
        if !matched {
            result.push(source[i].clone());
            i += 1;
        }
    }
    result
}

/// 执行移除：在 source 中查找所有匹配模式的序列并移除
pub fn apply_remove(
    source: &[Token],
    pattern: &TokenPattern,
) -> Vec<Token> {
    let mut result: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < source.len() {
        if let Some((consumed, _)) = pattern.match_from(source, i) {
            if consumed == 0 {
                result.push(source[i].clone());
                i += 1;
            } else {
                i += consumed; // 跳过匹配的部分
            }
        } else {
            result.push(source[i].clone());
            i += 1;
        }
    }
    result
}

// ──────────────── 单元测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wildcard() {
        let pat = TokenPattern::parse(&[Token::Underscore]).unwrap();
        assert_eq!(pat, TokenPattern::Wildcard);
    }

    #[test]
    fn test_parse_type_ident() {
        let pat = TokenPattern::parse(&[Token::Colon, Token::Ident("ident".into())]).unwrap();
        assert_eq!(pat, TokenPattern::TypeIdent);
    }

    #[test]
    fn test_parse_capture() {
        let pat = TokenPattern::parse(&[Token::Dollar, Token::Ident("x".into())]).unwrap();
        assert_eq!(pat, TokenPattern::Capture("x".into()));
    }

    #[test]
    fn test_match_wildcard() {
        let pat = TokenPattern::Wildcard;
        let tokens = vec![Token::Ident("foo".into())];
        let (n, caps) = pat.match_from(&tokens, 0).unwrap();
        assert_eq!(n, 1);
        assert!(caps.get("x").is_none());
    }

    #[test]
    fn test_match_exact() {
        let pat = TokenPattern::Exact(Token::Plus);
        let tokens = vec![Token::Plus, Token::IntLit(1)];
        let (n, _) = pat.match_from(&tokens, 0).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn test_match_exact_fail() {
        let pat = TokenPattern::Exact(Token::Plus);
        let tokens = vec![Token::Minus, Token::IntLit(1)];
        assert!(pat.match_from(&tokens, 0).is_none());
    }

    #[test]
    fn test_match_type_ident() {
        let pat = TokenPattern::TypeIdent;
        let tokens = vec![Token::Ident("foo".into())];
        assert!(pat.match_from(&tokens, 0).is_some());

        // IntLit 不是 Ident
        let tokens2 = vec![Token::IntLit(42)];
        assert!(pat.match_from(&tokens2, 0).is_none());
    }

    #[test]
    fn test_match_capture() {
        let pat = TokenPattern::Capture("var".into());
        let tokens = vec![Token::Ident("hello".into())];
        let (n, caps) = pat.match_from(&tokens, 0).unwrap();
        assert_eq!(n, 1);
        assert_eq!(caps.get("var").unwrap(), &vec![Token::Ident("hello".into())]);
    }

    #[test]
    fn test_match_seq() {
        let pat = TokenPattern::Seq(vec![
            TokenPattern::Capture("a".into()),
            TokenPattern::Exact(Token::Plus),
            TokenPattern::Capture("b".into()),
        ]);
        let tokens = vec![
            Token::Ident("x".into()),
            Token::Plus,
            Token::Ident("y".into()),
        ];
        let (n, caps) = pat.match_from(&tokens, 0).unwrap();
        assert_eq!(n, 3);
        assert_eq!(caps.get("a").unwrap(), &vec![Token::Ident("x".into())]);
        assert_eq!(caps.get("b").unwrap(), &vec![Token::Ident("y".into())]);
    }

    #[test]
    fn test_apply_remove() {
        let source = vec![
            Token::Ident("print".into()),
            Token::LParen,
            Token::Ident("x".into()),
            Token::RParen,
            Token::Semicolon,
        ];
        // 移除 "print(_)"
        let pattern = TokenPattern::Seq(vec![
            TokenPattern::Exact(Token::Ident("print".into())),
            TokenPattern::Exact(Token::LParen),
            TokenPattern::Wildcard,
            TokenPattern::Exact(Token::RParen),
        ]);
        let result = apply_remove(&source, &pattern);
        assert_eq!(result, vec![Token::Semicolon]);
    }

    #[test]
    fn test_apply_replace_simple() {
        let source = vec![
            Token::Ident("old".into()),
            Token::Dot,
            Token::Ident("field".into()),
        ];
        let rules = vec![ReplaceRule {
            from: TokenPattern::Exact(Token::Ident("old".into())),
            to: vec![Token::Ident("new".into())],
        }];
        let result = apply_replace(&source, &rules);
        assert_eq!(result[0], Token::Ident("new".into()));
    }

    #[test]
    fn test_apply_replace_with_capture() {
        // 交换 a + b → b + a
        let source = vec![
            Token::Ident("a".into()),
            Token::Plus,
            Token::Ident("b".into()),
        ];
        let from = TokenPattern::Seq(vec![
            TokenPattern::Capture("a".into()),
            TokenPattern::Exact(Token::Plus),
            TokenPattern::Capture("b".into()),
        ]);
        let to = vec![
            Token::Dollar, Token::Ident("b".into()),   // $b
            Token::Plus,
            Token::Dollar, Token::Ident("a".into()),   // $a
        ];
        let rules = vec![ReplaceRule { from, to }];
        let result = apply_replace(&source, &rules);
        // 期望: b + a
        assert_eq!(result[0], Token::Ident("b".into()));
        assert_eq!(result[1], Token::Plus);
        assert_eq!(result[2], Token::Ident("a".into()));
    }
}
