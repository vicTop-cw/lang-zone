//! 内联 LZ 类型字符串解析器
//!
//! 将 `.lzi` 中的类型字符串（如 `"List<int>"`、`"fn(int, str) -> bool"`）
//! 还原为 `lang_zone::types::Type`。

use lang_zone::types::Type;

/// 解析 LZ 类型字符串，失败时返回错误信息。
pub fn parse_type(s: &str) -> Result<Type, String> {
    let mut p = Parser::new(s);
    let ty = p.parse_type()?;
    p.skip_ws();
    if !p.is_at_end() {
        return Err(format!("unexpected trailing characters at pos {}", p.pos));
    }
    Ok(ty)
}

struct Parser {
    input: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(s: &str) -> Self {
        Self { input: s.chars().collect(), pos: 0 }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        self.skip_ws();
        match self.advance() {
            Some(c) if c == expected => Ok(()),
            Some(c) => Err(format!("expected '{}' but found '{}' at pos {}", expected, c, self.pos - 1)),
            None => Err(format!("expected '{}' but reached end of input", expected)),
        }
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        self.skip_ws();
        let mut types = vec![self.parse_intersection()?];
        self.skip_ws();
        while self.peek() == Some('|') {
            self.advance();
            types.push(self.parse_intersection()?);
            self.skip_ws();
        }
        Ok(flatten_union(types))
    }

    fn parse_intersection(&mut self) -> Result<Type, String> {
        let mut types = vec![self.parse_optional()?];
        self.skip_ws();
        while self.peek() == Some('&') {
            self.advance();
            types.push(self.parse_optional()?);
            self.skip_ws();
        }
        Ok(flatten_intersection(types))
    }

fn parse_optional(&mut self) -> Result<Type, String> {
        let ty = self.parse_primary()?;
        self.skip_ws();
        if self.peek() == Some('?') {
            self.advance();
            Ok(Type::Optional(Box::new(ty)))
        } else {
            Ok(ty)
        }
    }

    fn parse_primary(&mut self) -> Result<Type, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => self.parse_paren_or_tuple(),
            Some('&') => self.parse_ref(),
            Some(c) if c.is_alphabetic() || c == '_' => self.parse_named_or_keyword(),
            Some(c) => Err(format!("unexpected character '{}' at pos {}", c, self.pos)),
            None => Err("unexpected end of type string".into()),
        }
    }

    fn parse_named_or_keyword(&mut self) -> Result<Type, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let name: String = self.input[start..self.pos].iter().collect();
        self.skip_ws();

        // 函数类型
        if name == "fn" {
            return self.parse_fn_type();
        }

        // 泛型实参
        let ty = match name.as_str() {
            "int" => Type::Int,
            "f64" | "float" => Type::F64,
            "str" => Type::Str,
            "bool" => Type::Bool,
            "Unit" => Type::Unit,
            "Never" => Type::Never,
            "Any" => Type::Any,
            "None" => Type::None_,
            "Self" => Type::Self_,
            _ => Type::Named(name),
        };

        if self.peek() == Some('<') {
            let args = self.parse_generic_args()?;
            match &ty {
                Type::Named(n) if n == "Option" => Ok(Type::Option(Box::new(single_arg(args)?))),
                Type::Named(n) if n == "Result" => {
                    if args.len() != 2 {
                        return Err("Result<T, E> requires exactly two type arguments".into());
                    }
                    let mut args = args;
                    let err = args.pop().unwrap();
                    let ok = args.pop().unwrap();
                    Ok(Type::Result { ok: Box::new(ok), err: Box::new(err) })
                }
                _ => Ok(Type::Generic { base: Box::new(ty), args }),
            }
        } else {
            Ok(ty)
        }
    }

    fn parse_generic_args(&mut self) -> Result<Vec<Type>, String> {
        self.expect('<')?;
        let mut args = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some('>') {
                break;
            }
            args.push(self.parse_type()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => { self.advance(); }
                Some('>') => break,
                Some(c) => return Err(format!("expected ',' or '>' but found '{}' at pos {}", c, self.pos)),
                None => return Err("unterminated generic args".into()),
            }
        }
        self.expect('>')?;
        Ok(args)
    }

    fn parse_fn_type(&mut self) -> Result<Type, String> {
        self.expect('(')?;
        let mut params = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(')') {
                break;
            }
            params.push(self.parse_type()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => { self.advance(); }
                Some(')') => break,
                Some(c) => return Err(format!("expected ',' or ')' but found '{}' at pos {}", c, self.pos)),
                None => return Err("unterminated function parameter list".into()),
            }
        }
        self.expect(')')?;
        self.skip_ws();
        self.expect('-')?;
        self.expect('>')?;
        let ret = self.parse_type()?;
        Ok(Type::Fn { params, ret: Box::new(ret) })
    }

    fn parse_paren_or_tuple(&mut self) -> Result<Type, String> {
        self.expect('(')?;
        self.skip_ws();
        if self.peek() == Some(')') {
            self.advance();
            return Ok(Type::Unit);
        }
        let first = self.parse_type()?;
        self.skip_ws();
        if self.peek() == Some(',') {
            let mut elems = vec![first];
            while self.peek() == Some(',') {
                self.advance();
                self.skip_ws();
                if self.peek() == Some(')') {
                    break;
                }
                elems.push(self.parse_type()?);
                self.skip_ws();
            }
            self.expect(')')?;
            Ok(Type::Tuple(elems))
        } else {
            self.expect(')')?;
            Ok(first)
        }
    }

    fn parse_ref(&mut self) -> Result<Type, String> {
        self.expect('&')?;
        self.skip_ws();
        if self.peek() == Some('m') {
            // 检查是否为 "mut"
            let start = self.pos;
            let word = self.parse_word()?;
            if word == "mut" {
                let inner = self.parse_type()?;
                return Ok(Type::MutRef(Box::new(inner)));
            } else {
                // 不是 mut，回退
                self.pos = start;
            }
        }
        let inner = self.parse_type()?;
        Ok(Type::Ref(Box::new(inner)))
    }

    fn parse_word(&mut self) -> Result<String, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        Ok(self.input[start..self.pos].iter().collect())
    }
}

fn flatten_intersection(types: Vec<Type>) -> Type {
    let mut flat = Vec::new();
    for t in types {
        if let Type::Intersection(inner) = t {
            for it in inner {
                if !flat.contains(&it) {
                    flat.push(it);
                }
            }
        } else if !flat.contains(&t) {
            flat.push(t);
        }
    }
    if flat.len() == 1 {
        flat.into_iter().next().unwrap()
    } else {
        Type::Intersection(flat)
    }
}

fn flatten_union(types: Vec<Type>) -> Type {
    let mut flat = Vec::new();
    for t in types {
        if let Type::Union(inner) = t {
            for it in inner {
                if !flat.contains(&it) {
                    flat.push(it);
                }
            }
        } else if !flat.contains(&t) {
            flat.push(t);
        }
    }
    if flat.len() == 1 {
        flat.into_iter().next().unwrap()
    } else {
        Type::Union(flat)
    }
}

fn single_arg(args: Vec<Type>) -> Result<Type, String> {
    if args.len() != 1 {
        return Err("expected exactly one type argument".into());
    }
    Ok(args.into_iter().next().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_primitives() {
        assert_eq!(parse_type("int").unwrap(), Type::Int);
        assert_eq!(parse_type("f64").unwrap(), Type::F64);
        assert_eq!(parse_type("str").unwrap(), Type::Str);
        assert_eq!(parse_type("bool").unwrap(), Type::Bool);
        assert_eq!(parse_type("Unit").unwrap(), Type::Unit);
    }

    #[test]
    fn parse_generic() {
        assert_eq!(
            parse_type("List<int>").unwrap(),
            Type::Generic { base: Box::new(Type::Named("List".into())), args: vec![Type::Int] }
        );
    }

    #[test]
    fn parse_option() {
        assert_eq!(
            parse_type("int?").unwrap(),
            Type::Optional(Box::new(Type::Int))
        );
        assert_eq!(
            parse_type("Option<int>").unwrap(),
            Type::Option(Box::new(Type::Int))
        );
    }

    #[test]
    fn parse_result() {
        assert_eq!(
            parse_type("Result<int, str>").unwrap(),
            Type::Result { ok: Box::new(Type::Int), err: Box::new(Type::Str) }
        );
    }

    #[test]
    fn parse_fn() {
        assert_eq!(
            parse_type("fn(int, int) -> int").unwrap(),
            Type::Fn { params: vec![Type::Int, Type::Int], ret: Box::new(Type::Int) }
        );
    }

    #[test]
    fn parse_tuple() {
        assert_eq!(
            parse_type("(int, str)").unwrap(),
            Type::Tuple(vec![Type::Int, Type::Str])
        );
    }

    #[test]
    fn parse_ref() {
        assert_eq!(
            parse_type("&int").unwrap(),
            Type::Ref(Box::new(Type::Int))
        );
        assert_eq!(
            parse_type("&mut int").unwrap(),
            Type::MutRef(Box::new(Type::Int))
        );
    }
}
