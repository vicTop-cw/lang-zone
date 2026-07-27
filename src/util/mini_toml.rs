// Lang-Zong 编译器 — mini_toml.rs
// 极简 TOML 解析器：仅支持桥接清单所需的子集
// 支持：[section] 头、key = "string"、key = { inline_table }、key = integer、# 注释
// 不支持：多行字符串、数组、日期、嵌套表等复杂特性

use std::collections::HashMap;

/// 极简 TOML 文档 = section 名 → (key → value)
pub type TomlDoc = HashMap<String, HashMap<String, TomlValue>>;

#[derive(Debug, Clone)]
pub enum TomlValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    InlineTable(HashMap<String, TomlValue>),
}

/// 解析 TOML 文本为 TomlDoc
pub fn parse(text: &str) -> Result<TomlDoc, String> {
    let mut doc: TomlDoc = HashMap::new();
    let mut current_section = String::new(); // 空 = 根表

    for line in text.lines() {
        let trimmed = line.trim();

        // 空行或注释
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // [section] 头
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len()-1].trim().to_string();
            doc.entry(current_section.clone()).or_insert_with(HashMap::new);
            continue;
        }

        // key = value
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let val_str = trimmed[eq_pos+1..].trim();

            let value = parse_value(val_str)?;
            doc.entry(current_section.clone())
                .or_insert_with(HashMap::new)
                .insert(key, value);
        }
    }

    Ok(doc)
}

/// 解析单个值
fn parse_value(s: &str) -> Result<TomlValue, String> {
    // 去尾部注释（同一行 # 后的内容）
    let s = strip_inline_comment(s);

    // 字符串 "..."
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return Ok(TomlValue::Str(s[1..s.len()-1].to_string()));
    }

    // 内联表 { k = v, ... }
    if s.starts_with('{') && s.ends_with('}') {
        return parse_inline_table(s);
    }

    // bool
    if s == "true" { return Ok(TomlValue::Bool(true)); }
    if s == "false" { return Ok(TomlValue::Bool(false)); }

    // integer
    if let Ok(n) = s.parse::<i64>() {
        return Ok(TomlValue::Int(n));
    }

    // float
    if let Ok(f) = s.parse::<f64>() {
        return Ok(TomlValue::Float(f));
    }

    // 纯字符串（无引号，如 never-2026-07-01）
    if !s.is_empty() && !s.contains(' ') && !s.contains('=') {
        return Ok(TomlValue::Str(s.to_string()));
    }

    Err(format!("无法解析 TOML 值: {}", s))
}

/// 解析内联表 { key = value, key2 = value2 }
fn parse_inline_table(s: &str) -> Result<TomlValue, String> {
    let inner = &s[1..s.len()-1];
    let mut table = HashMap::new();

    // 逐项分割（使用字节位置，与 find 返回的 byte index 一致）
    let mut pos = 0;
    let inner_bytes = inner.as_bytes();
    while pos < inner_bytes.len() {
        // 找 key = value 的边界
        let rest = &inner[pos..];
        let rest_trimmed = rest.trim_start();
        let skip = rest.len() - rest_trimmed.len();
        pos += skip;

        if pos >= inner_bytes.len() { break; }

        // 找 = 位置
        let rest = &inner[pos..];
        if let Some(eq) = rest.find('=') {
            let key = rest[..eq].trim().to_string();
            let after_eq = &rest[eq+1..];

            // 值终止：逗号或字符串结尾
            let (value_str, consumed) = extract_value_until_comma(after_eq);

            let value = parse_value(value_str.trim())?;
            table.insert(key, value);
            pos += eq + 1 + consumed;
            // 跳过逗号分隔符（若存在）
            if pos < inner_bytes.len() && inner_bytes[pos] == b',' {
                pos += 1;
            }
        } else {
            break;
        }
    }

    Ok(TomlValue::InlineTable(table))
}

/// 提取值直到遇到逗号（考虑字符串中的逗号）
/// 返回 (value_str, byte_position_after_comma)
fn extract_value_until_comma(s: &str) -> (&str, usize) {
    let bytes = s.as_bytes();
    let mut i = 0;

    // 跳过前导空格
    while i < bytes.len() && bytes[i] == b' ' { i += 1; }

    // 字符串值
    if i < bytes.len() && bytes[i] == b'"' {
        i += 1;
        while i < bytes.len() && bytes[i] != b'"' { i += 1; }
        if i < bytes.len() { i += 1; } // 跳过结束引号
        // 找逗号
        while i < bytes.len() && bytes[i] != b',' { i += 1; }
        return (&s[..i], i);
    }

    // 内联表
    if i < bytes.len() && bytes[i] == b'{' {
        let mut depth = 1;
        i += 1;
        while i < bytes.len() {
            if bytes[i] == b'{' { depth += 1; }
            if bytes[i] == b'}' { depth -= 1; if depth == 0 { i += 1; break; } }
            i += 1;
        }
        while i < bytes.len() && bytes[i] != b',' { i += 1; }
        return (&s[..i], i);
    }

    // 普通值：到逗号或字符串结尾
    while i < bytes.len() && bytes[i] != b',' { i += 1; }
    (&s[..i], i)
}

/// 去除行内注释（# 后的内容，但不在字符串内的 #）
fn strip_inline_comment(s: &str) -> &str {
    let mut in_str = false;
    for (i, c) in s.char_indices() {
        if c == '"' { in_str = !in_str; }
        if c == '#' && !in_str {
            return &s[..i].trim_end();
        }
    }
    s
}

// ─── 便捷访问方法 ───

impl TomlValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            TomlValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            TomlValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            TomlValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            TomlValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&HashMap<String, TomlValue>> {
        match self {
            TomlValue::InlineTable(t) => Some(t),
            _ => None,
        }
    }
}

// ──────────────── 单元测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── 基础解析 ───

    #[test]
    fn test_empty_doc() {
        let doc = parse("").unwrap();
        assert!(doc.is_empty());
    }

    #[test]
    fn test_comment_only() {
        let doc = parse("# this is a comment\n# another comment").unwrap();
        assert!(doc.is_empty());
    }

    #[test]
    fn test_simple_string() {
        let doc = parse(r#"key = "value""#).unwrap();
        let root = doc.get("").unwrap();
        assert_eq!(root.get("key").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn test_integer() {
        let doc = parse("count = 42").unwrap();
        let root = doc.get("").unwrap();
        assert_eq!(root.get("count").unwrap().as_int(), Some(42));
    }

    #[test]
    fn test_float() {
        let doc = parse("pi = 3.14").unwrap();
        let root = doc.get("").unwrap();
        // TOML 中 3.14 解析为 Float(3.14)，as_str() 返回 None
        assert!(root.get("pi").unwrap().as_float().is_some());
    }

    #[test]
    fn test_bool_true() {
        let doc = parse("enabled = true").unwrap();
        let root = doc.get("").unwrap();
        assert_eq!(root.get("enabled").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn test_bool_false() {
        let doc = parse("disabled = false").unwrap();
        let root = doc.get("").unwrap();
        assert_eq!(root.get("disabled").unwrap().as_bool(), Some(false));
    }

    // ─── Section 解析 ───

    #[test]
    fn test_section() {
        let text = "[meta]\ndesc = \"test\"";
        let doc = parse(text).unwrap();
        let section = doc.get("meta").unwrap();
        assert_eq!(section.get("desc").unwrap().as_str(), Some("test"));
    }

    #[test]
    fn test_multiple_sections() {
        let text = "[a]\nx = \"1\"\n[b]\ny = \"2\"";
        let doc = parse(text).unwrap();
        assert_eq!(doc.get("a").unwrap().get("x").unwrap().as_str(), Some("1"));
        assert_eq!(doc.get("b").unwrap().get("y").unwrap().as_str(), Some("2"));
    }

    #[test]
    fn test_root_and_section() {
        let text = "root = \"r\"\n[s]\nkey = \"v\"";
        let doc = parse(text).unwrap();
        assert_eq!(doc.get("").unwrap().get("root").unwrap().as_str(), Some("r"));
        assert_eq!(doc.get("s").unwrap().get("key").unwrap().as_str(), Some("v"));
    }

    // ─── 内联表解析 ───

    #[test]
    fn test_inline_table_single() {
        let doc = parse("m = { tier = 1 }").unwrap();
        let table = doc.get("").unwrap().get("m").unwrap().as_table().unwrap();
        // tier = 1 是 TOML 整数，解析为 Int(1)，用 as_int()
        assert_eq!(table.get("tier").unwrap().as_int(), Some(1));
    }

    #[test]
    fn test_inline_table_multi() {
        let doc = parse("p = { rust = \"std::io\", shim = \"fmt\" }").unwrap();
        let table = doc.get("").unwrap().get("p").unwrap().as_table().unwrap();
        assert_eq!(table.get("rust").unwrap().as_str(), Some("std::io"));
        assert_eq!(table.get("shim").unwrap().as_str(), Some("fmt"));
    }

    #[test]
    fn test_inline_table_nested_path() {
        let doc = parse("f = { rust = \"std::fs::read_to_string\", shim = \"path_ref\" }").unwrap();
        let table = doc.get("").unwrap().get("f").unwrap().as_table().unwrap();
        assert_eq!(table.get("rust").unwrap().as_str(), Some("std::fs::read_to_string"));
        assert_eq!(table.get("shim").unwrap().as_str(), Some("path_ref"));
    }

    #[test]
    fn test_inline_table_empty_shim() {
        let doc = parse("f = { rust = \"std::thread::spawn\", shim = \"\" }").unwrap();
        let table = doc.get("").unwrap().get("f").unwrap().as_table().unwrap();
        assert_eq!(table.get("shim").unwrap().as_str(), Some(""));
    }

    #[test]
    fn test_section_with_inline_tables() {
        let text = "[modules]\ncore = { tier = 1 }\nstr = { tier = 1 }";
        let doc = parse(text).unwrap();
        let modules = doc.get("modules").unwrap();
        assert_eq!(modules.get("core").unwrap().as_table().unwrap().get("tier").unwrap().as_int(), Some(1));
        assert_eq!(modules.get("str").unwrap().as_table().unwrap().get("tier").unwrap().as_int(), Some(1));
    }

    // ─── 注释处理 ───

    #[test]
    fn test_inline_comment() {
        let doc = parse("name = \"test\" # this describes name").unwrap();
        assert_eq!(doc.get("").unwrap().get("name").unwrap().as_str(), Some("test"));
    }

    #[test]
    fn test_comment_in_section() {
        let text = "[meta]\n# comment line\nvalue = \"ok\"";
        let doc = parse(text).unwrap();
        assert_eq!(doc.get("meta").unwrap().get("value").unwrap().as_str(), Some("ok"));
    }

    #[test]
    fn test_comment_preserves_inline_table() {
        let doc = parse("m = { tier = 1 } # module entry").unwrap();
        let table = doc.get("").unwrap().get("m").unwrap().as_table().unwrap();
        assert_eq!(table.get("tier").unwrap().as_int(), Some(1));
    }

    // ─── 实际清单文件格式 ───

    #[test]
    fn test_module_manifest_format() {
        let text = r#"[module]
tier = 1
rust_prefix = "std::io"

[types]
IOError = "std::io::Error"
BufReader = "std::io::BufReader"

[functions]
read_to_string = { rust = "std::fs::read_to_string", shim = "path_ref" }
write = { rust = "std::fs::write", shim = "path_ref" }

[methods]
length = "len"
isEmpty = "is_empty"
append = "push"

[aliases]
IOError = "std::io::Error"
"#;
        let doc = parse(text).unwrap();

        // [module]
        assert_eq!(doc.get("module").unwrap().get("tier").unwrap().as_int(), Some(1));
        assert_eq!(doc.get("module").unwrap().get("rust_prefix").unwrap().as_str(), Some("std::io"));

        // [types]
        let types = doc.get("types").unwrap();
        assert_eq!(types.get("IOError").unwrap().as_str(), Some("std::io::Error"));
        assert_eq!(types.get("BufReader").unwrap().as_str(), Some("std::io::BufReader"));

        // [functions]
        let funcs = doc.get("functions").unwrap();
        let f1 = funcs.get("read_to_string").unwrap().as_table().unwrap();
        assert_eq!(f1.get("rust").unwrap().as_str(), Some("std::fs::read_to_string"));
        assert_eq!(f1.get("shim").unwrap().as_str(), Some("path_ref"));
        let f2 = funcs.get("write").unwrap().as_table().unwrap();
        assert_eq!(f2.get("rust").unwrap().as_str(), Some("std::fs::write"));

        // [methods]
        let methods = doc.get("methods").unwrap();
        assert_eq!(methods.get("length").unwrap().as_str(), Some("len"));
        assert_eq!(methods.get("isEmpty").unwrap().as_str(), Some("is_empty"));
        assert_eq!(methods.get("append").unwrap().as_str(), Some("push"));

        // [aliases]
        let aliases = doc.get("aliases").unwrap();
        assert_eq!(aliases.get("IOError").unwrap().as_str(), Some("std::io::Error"));
    }

    #[test]
    fn test_bridge_top_format() {
        let text = r#"[toolchain]
tier1_channel = "stable"
tier2_channel = "nightly-2026-07-01"

[modules]
core = { tier = 1 }
io = { tier = 1 }
fs = { tier = 1 }

[tier2_gate]
enabled_by_default = false

[type_aliases]
IOError = "std::io::Error"
Never = "!"
"#;
        let doc = parse(text).unwrap();

        let toolchain = doc.get("toolchain").unwrap();
        assert_eq!(toolchain.get("tier1_channel").unwrap().as_str(), Some("stable"));
        assert_eq!(toolchain.get("tier2_channel").unwrap().as_str(), Some("nightly-2026-07-01"));

        let modules = doc.get("modules").unwrap();
        assert_eq!(modules.get("core").unwrap().as_table().unwrap().get("tier").unwrap().as_int(), Some(1));

        let gate = doc.get("tier2_gate").unwrap();
        assert_eq!(gate.get("enabled_by_default").unwrap().as_bool(), Some(false));

        let aliases = doc.get("type_aliases").unwrap();
        assert_eq!(aliases.get("Never").unwrap().as_str(), Some("!"));
    }

    // ─── 边界场景 ───

    #[test]
    fn test_key_with_underscores() {
        let doc = parse("read_to_string = \"ok\"").unwrap();
        assert_eq!(doc.get("").unwrap().get("read_to_string").unwrap().as_str(), Some("ok"));
    }

    #[test]
    fn test_value_with_colons() {
        let doc = parse(r#"prefix = "std::collections::HashMap""#).unwrap();
        assert_eq!(doc.get("").unwrap().get("prefix").unwrap().as_str(), Some("std::collections::HashMap"));
    }

    #[test]
    fn test_value_with_spaces_inside_quotes() {
        let doc = parse(r#"desc = "hello world""#).unwrap();
        assert_eq!(doc.get("").unwrap().get("desc").unwrap().as_str(), Some("hello world"));
    }

    #[test]
    fn test_value_with_punctuation() {
        let doc = parse(r#"v = "{0}.contains(&{1})""#).unwrap();
        assert_eq!(doc.get("").unwrap().get("v").unwrap().as_str(), Some("{0}.contains(&{1})"));
    }

    #[test]
    fn test_multiple_keys_in_root() {
        let text = r#"a = "1"
b = "2"
c = "3""#;
        let doc = parse(text).unwrap();
        let root = doc.get("").unwrap();
        assert_eq!(root.len(), 3);
        assert_eq!(root.get("a").unwrap().as_str(), Some("1"));
        assert_eq!(root.get("b").unwrap().as_str(), Some("2"));
        assert_eq!(root.get("c").unwrap().as_str(), Some("3"));
    }

    #[test]
    fn test_trailing_whitespace() {
        let doc = parse("key = \"value\"   \n").unwrap();
        assert_eq!(doc.get("").unwrap().get("key").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn test_special_chars_in_value() {
        let doc = parse(r#"flag = "rustc_private""#).unwrap();
        assert_eq!(doc.get("").unwrap().get("flag").unwrap().as_str(), Some("rustc_private"));
    }

    #[test]
    fn test_nightly_version_string() {
        let doc = parse(r#"nightly_required = "nightly-2026-07-01""#).unwrap();
        assert_eq!(doc.get("").unwrap().get("nightly_required").unwrap().as_str(), Some("nightly-2026-07-01"));
    }

    // ─── 错误场景 ───

    #[test]
    fn test_unparsable_value_returns_err() {
        // 空键后面的未加引号值带空格不可解析
        let result = parse("key = value with spaces");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_key_error() {
        let result = parse("= \"value\"");
        // 空键仍会被插入（键为 ""），但值是合法的
        // mini_toml 不拒绝空键，只是存储它
        assert!(result.is_ok());
    }
}
