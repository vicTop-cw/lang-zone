// Lang-Zone 编译器 — lexer/numbers.rs
// （由 lexer/lexer.rs move-only 拆出，逻辑零改动）

use super::*;

impl Lexer {
    pub(crate) fn read_number(&mut self, first: char) -> Token {
        let mut num = String::from(first);
        let mut is_float = false;

        // 处理进制前缀 0x 0o 0b
        if first == '0' {
            match self.peek() {
                Some('x') | Some('X') => {
                    num.push(self.advance().unwrap());
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() {
                            num.push(self.advance().unwrap());
                        } else if c == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    match i64::from_str_radix(&num[2..].replace('_', ""), 16) {
                        Ok(val) => return Token::IntLit(val),
                        Err(_) => {
                            let hex_str = &num[2..].replace('_', "");
                            // 如果值在 u64 范围内，作为 i64 返回（允许负数表示）
                            if let Ok(val) = u64::from_str_radix(hex_str, 16) {
                                return Token::IntLit(val as i64);
                            }
                            return Token::LexError(format!("无效的十六进制数字: {}", num));
                        }
                    }
                }
                Some('o') | Some('O') => {
                    num.push(self.advance().unwrap());
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() && c < '8' {
                            num.push(self.advance().unwrap());
                        } else if c == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    match i64::from_str_radix(&num[2..].replace('_', ""), 8) {
                        Ok(val) => return Token::IntLit(val),
                        Err(_) => {
                            return Token::LexError(format!("八进制值溢出 i64 范围: {}", num))
                        }
                    }
                }
                Some('b') | Some('B') => {
                    num.push(self.advance().unwrap());
                    while let Some(c) = self.peek() {
                        if c == '0' || c == '1' {
                            num.push(self.advance().unwrap());
                        } else if c == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    match i64::from_str_radix(&num[2..].replace('_', ""), 2) {
                        Ok(val) => return Token::IntLit(val),
                        Err(_) => {
                            return Token::LexError(format!("二进制值溢出 i64 范围: {}", num))
                        }
                    }
                }
                _ => {}
            }
        }

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                num.push(self.advance().unwrap());
            } else if c == '_' {
                self.advance();
            } else if c == '.' && !is_float && self.peek_n(1).map_or(false, |c| c.is_ascii_digit())
            {
                is_float = true;
                num.push(self.advance().unwrap());
            } else if (c == 'e' || c == 'E') && !is_float {
                is_float = true;
                num.push(self.advance().unwrap());
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        num.push(self.advance().unwrap());
                    }
                }
            } else if c == 'e' || c == 'E' {
                num.push(self.advance().unwrap());
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        num.push(self.advance().unwrap());
                    }
                }
            } else {
                break;
            }
        }
        // 复数字面量后缀：浮点后紧跟 `i` 或 `I` → ComplexLit(0.0, imag)
        if is_float {
            let imag = num.parse::<f64>().unwrap_or(0.0);
            if let Some(c) = self.peek() {
                if c == 'i' || c == 'I' {
                    self.advance();
                    return Token::ComplexLit(0.0, imag);
                }
            }
        }
        // G2: 数字后紧跟字母/下划线（如 `12abc`）→ 非法数字字面量
        if let Some(c) = self.peek() {
            if c.is_alphabetic() || c == '_' {
                return Token::LexError(format!("非法数字字面量: {}（后跟 `{}`）", num, c));
            }
        }
        if is_float {
            match num.parse::<f64>() {
                Ok(v) => Token::FloatLit(v),
                Err(_) => {
                    // 检查是否形如 "123e"（指数无尾数）
                    if num.ends_with('e')
                        || num.ends_with('E')
                        || num.ends_with("e+")
                        || num.ends_with("E+")
                        || num.ends_with("e-")
                        || num.ends_with("E-")
                    {
                        Token::LexError(format!("科学计数法缺少指数: {}", num))
                    } else {
                        Token::LexError(format!("无效的浮点数: {}", num))
                    }
                }
            }
        } else {
            match num.parse::<i64>() {
                Ok(v) => Token::IntLit(v),
                Err(_) => {
                    // i64::MAX = 9223372036854775807，其 +1 = 9223372036854775808 超出 i64 正数范围。
                    // 该值仅在作为一元负号操作数（即源码 `-9223372036854775808` == i64::MIN）
                    // 时合法，透传为 i64::MIN 哨兵；其余情形（裸 `9223372036854775808` 或二元减
                    // 操作数）一律拒绝，避免被静默环绕成 i64::MIN（BUG-EC-002）。
                    // 超出 i128 范围的值尝试解析为 BigInt（LZ 支持 BigInt 基础类型）。
                    if num == "9223372036854775808" {
                        // read_number 在此分支时所有数字已读完，self.pos 指向末位之后。
                        // 首位数字的位置 = self.pos - num.len()，其前字符位于 -1，再前 -2。
                        let first_digit_pos = self.pos - num.len();
                        let before_first = self.chars.get(first_digit_pos.wrapping_sub(1)).copied();
                        let is_unary_minus = match before_first {
                            Some('-') => {
                                let before_minus =
                                    self.chars.get(first_digit_pos.wrapping_sub(2)).copied();
                                match before_minus {
                                    None => true,
                                    Some(c)
                                        if c.is_whitespace()
                                            || c == '('
                                            || c == '['
                                            || c == '{'
                                            || c == '='
                                            || c == ':'
                                            || c == ','
                                            || c == '+'
                                            || c == '-'
                                            || c == '*'
                                            || c == '/'
                                            || c == '<'
                                            || c == '>'
                                            || c == '|'
                                            || c == '&' =>
                                    {
                                        true
                                    }
                                    _ => false,
                                }
                            }
                            _ => false,
                        };
                        if is_unary_minus {
                            Token::IntLit(i64::MIN)
                        } else {
                            // 尝试解析为 i128，溢出则回退到 BigInt
                            match num.parse::<i128>() {
                                Ok(v) => Token::Int128Lit(v),
                                Err(_) => Token::BigIntLit(num.clone()),
                            }
                        }
                    } else {
                        // 尝试解析为 i128，溢出则回退到 BigInt
                        match num.parse::<i128>() {
                            Ok(v) => Token::Int128Lit(v),
                            Err(_) => Token::BigIntLit(num.clone()),
                        }
                    }
                }
            }
        }
    }
}
