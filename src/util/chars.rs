// Lang-Zong 编译器 — util/chars.rs
// 字符分类与基础工具：标识符边界、空白判断、数字检测
//
// 对标 Python token.py 的字符分类函数 — 所有判断集中管理，避免词法/解析器分散硬编码

/// 判断是否可作为标识符首字符（Unicode XID_Start 子集 + `_`）
pub fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

/// 判断是否可作为标识符后续字符（Unicode XID_Continue 子集 + `_`）
pub fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// 判断是否为空白字符（空格、制表符）
pub fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// 判断是否为换行符
pub fn is_newline(c: char) -> bool {
    c == '\n' || c == '\r'
}

/// 判断是否为十进制数字
pub fn is_dec_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// 判断是否为十六进制数字
pub fn is_hex_digit(c: char) -> bool {
    c.is_ascii_hexdigit()
}

/// 判断是否为八进制数字
pub fn is_oct_digit(c: char) -> bool {
    ('0'..='7').contains(&c)
}

/// 判断是否为二进制数字
pub fn is_bin_digit(c: char) -> bool {
    c == '0' || c == '1'
}

/// 判断是否为构建块空白分隔符（空格/制表符/换行，用于 `=` `:` `~` `*` 构建块前缀消歧）
/// 等价于 Python tokenize 中的 "空白间隙" 检查
pub fn is_build_ws(c: Option<char>) -> bool {
    match c {
        None => true,
        Some(ch) => is_whitespace(ch) || is_newline(ch),
    }
}

/// 判断字符是否为运算符的一部分
pub fn is_operator_char(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '.')
}

/// 判断字符是否为括号/分隔符
pub fn is_delimiter(c: char) -> bool {
    matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | ';')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ident_start() {
        assert!(is_ident_start('a'));
        assert!(is_ident_start('Z'));
        assert!(is_ident_start('_'));
        assert!(!is_ident_start('1'));
        assert!(!is_ident_start(' '));
    }

    #[test]
    fn test_ident_continue() {
        assert!(is_ident_continue('a'));
        assert!(is_ident_continue('0'));
        assert!(is_ident_continue('_'));
        assert!(!is_ident_continue(' '));
        assert!(!is_ident_continue('.'));
    }

    #[test]
    fn test_build_ws() {
        assert!(is_build_ws(None));
        assert!(is_build_ws(Some(' ')));
        assert!(is_build_ws(Some('\t')));
        assert!(is_build_ws(Some('\n')));
        assert!(!is_build_ws(Some('x')));
    }
}
