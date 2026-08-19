//! LSP language server (FIST T4.5 / LZ_UPGRADE_PLAN direction D: LSP)
//! `lang-zone lsp`: stdio JSON-RPC 2.0 Language Server, supporting:
//! - initialization negotiation (initialize / initialized)
//! - document lifecycle (didOpen / didChange / didSave / didClose)
//! - diagnostics (publishDiagnostics): lexer / parser / IR three-layer errors, mapped to source lines
//! - go-to-definition (textDocument/definition): based on symbol table (AST + line location)
//! - completion (textDocument/completion): keywords + document symbols + local variables
//! - hover (textDocument/hover): symbol signature
//! - lifecycle (shutdown / exit)
//!
//! Implementation notes:
//! - Frame protocol is the LSP standard `Content-Length: N\r\n\r\n<json>`.
//! - Error location uses "token index -> source line" greedy text matching (does not touch lexer/parser).
//! - Symbol table is built from AST (function/struct/trait/enum/impl names) + text line scan (line numbers).

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};

use crate::ir::builder::build_ir;
use crate::lexer::{Lexer, Token};
use crate::parser::Parser;
use serde_json::{json, Value};

/// LSP diagnostic
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub line: usize, // 0-based
    pub char_start: usize,
    pub char_end: usize,
    pub message: String,
    pub severity: u8, // 1=Error
}

/// A symbol (function/struct/trait/enum/impl/import/local)
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: String, // Function/Struct/Trait/Enum/Impl/Import/Local
    pub line: usize,  // 0-based
    pub detail: String,
}

/// Result of a full document analysis
#[derive(Debug, Clone)]
pub struct Analysis {
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Open-document cache (uri -> text)
#[derive(Debug, Default)]
pub struct LspState {
    pub docs: HashMap<String, String>,
}

/// Run the LSP server main loop (blocking; returns 0 on exit)
pub fn run_lsp() -> i32 {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = std::io::BufWriter::new(stdout.lock());
    let mut state = LspState::default();

    loop {
        // Read one LSP frame: header lines until an empty line, then the JSON body.
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).map_err(|_| ()).is_err() || line.is_empty() {
                // EOF -> exit
                return 0;
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                content_length = rest.trim().parse::<usize>().ok();
            }
        }
        let Some(len) = content_length else {
            return 0;
        };
        let mut body = vec![0u8; len];
        if reader.read_exact(&mut body).is_err() {
            return 0;
        }
        let msg: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id").cloned();

        match method {
            "initialize" => {
                respond(
                    &mut writer,
                    id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": 2,
                            "definitionProvider": true,
                            "completionProvider": {
                                "triggerCharacters": [".", ":"],
                                "resolveProvider": false
                            },
                            "hoverProvider": true
                        },
                        "serverInfo": {
                            "name": "lang-zone-lsp",
                            "version": crate::util::version::version()
                        }
                    }),
                );
            }
            "initialized" => { /* no response */ }
            "textDocument/didOpen" => {
                if let Some(uri) = msg.pointer("/params/textDocument/uri").and_then(|v| v.as_str()) {
                    let text = msg
                        .pointer("/params/textDocument/text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    state.docs.insert(uri.to_string(), text.clone());
                    publish_diagnostics(&mut writer, uri, &text);
                }
            }
            "textDocument/didChange" => {
                if let Some(uri) = msg.pointer("/params/textDocument/uri").and_then(|v| v.as_str()) {
                    // Full-document sync (textDocumentSync=2 advertises incremental; clients may send
                    // incremental changes. Simplified: replace whole document with the last
                    // contentChanges[].text).
                    let text = msg
                        .pointer("/params/contentChanges/0/text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    state.docs.insert(uri.to_string(), text.clone());
                    publish_diagnostics(&mut writer, uri, &text);
                }
            }
            "textDocument/didSave" => {
                if let Some(uri) = msg.pointer("/params/textDocument/uri").and_then(|v| v.as_str()) {
                    if let Some(text) = state.docs.get(uri) {
                        publish_diagnostics(&mut writer, uri, text);
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = msg.pointer("/params/textDocument/uri").and_then(|v| v.as_str()) {
                    state.docs.remove(uri);
                }
            }
            "textDocument/definition" => {
                let (uri, pos) = extract_uri_position(&msg);
                if let (Some(uri), Some((line, ch))) = (uri, pos) {
                    let text = state.docs.get(&uri).cloned().unwrap_or_default();
                    let analysis = analyze(&text, &uri);
                    let name = word_at(&text, line, ch);
                    let mut result = Vec::new();
                    if let Some(n) = name {
                        // exact match first, then path-tail match
                        if let Some(sym) = analysis
                            .symbols
                            .iter()
                            .find(|s| s.name == n)
                        {
                            result.push(location_json(&uri, sym.line));
                        } else if let Some(sym) = analysis
                            .symbols
                            .iter()
                            .find(|s| s.name.split('.').last() == Some(n.as_str()))
                        {
                            result.push(location_json(&uri, sym.line));
                        }
                    }
                    respond(&mut writer, id, Value::Array(result));
                } else {
                    respond(&mut writer, id, Value::Array(vec![]));
                }
            }
            "textDocument/completion" => {
                let (uri, pos) = extract_uri_position(&msg);
                let mut items: Vec<Value> = Vec::new();
                let text = uri
                    .as_ref()
                    .and_then(|u| state.docs.get(u))
                    .cloned()
                    .unwrap_or_default();
                let analysis = analyze(&text, uri.as_deref().unwrap_or(""));
                // keyword completion
                for kw in KEYWORDS {
                    items.push(json!({ "label": kw, "kind": 14, "detail": "keyword" }));
                }
                // document symbols
                for s in &analysis.symbols {
                    items.push(json!({
                        "label": s.name,
                        "kind": symbol_kind_number(&s.kind),
                        "detail": s.detail
                    }));
                }
                // local variables (let bindings, simplified: collect let binding names)
                if let (Some(uri), Some((line, _ch))) = (uri, pos) {
                    if let Some(doc) = state.docs.get(&uri) {
                        for name in local_names_before(doc, line) {
                            items.push(json!({ "label": name, "kind": 6, "detail": "local variable" }));
                        }
                    }
                }
                respond(&mut writer, id, json!({ "isIncomplete": false, "items": items }));
            }
            "textDocument/hover" => {
                let (uri, pos) = extract_uri_position(&msg);
                let mut result: Value = Value::Null;
                if let (Some(uri), Some((line, ch))) = (uri, pos) {
                    let text = state.docs.get(&uri).cloned().unwrap_or_default();
                    let analysis = analyze(&text, &uri);
                    if let Some(n) = word_at(&text, line, ch) {
                        if let Some(sym) = analysis
                            .symbols
                            .iter()
                            .find(|s| s.name == n)
                            .or_else(|| analysis.symbols.iter().find(|s| s.name.split('.').last() == Some(n.as_str())))
                        {
                            result = json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": format!("**{}**  \n{}\n\n`{}`", sym.name, sym.kind, sym.detail)
                                }
                            });
                        }
                    }
                }
                respond(&mut writer, id, result);
            }
            "shutdown" => {
                respond(&mut writer, id, Value::Null);
            }
            "exit" => {
                return 0;
            }
            _ => {
                // unknown request: return null (per JSON-RPC); unknown notification: ignore
                if id.is_some() {
                    respond(&mut writer, id, Value::Null);
                }
            }
        }
    }
}

/// LSP completion keyword list
const KEYWORDS: &[&str] = &[
    "def", "struct", "enum", "trait", "impl", "let", "mut", "const", "ref",
    "if", "elif", "else", "match", "case", "for", "in", "while", "loop",
    "break", "continue", "return", "yield", "iterator", "import", "from", "as",
    "try", "catch", "finally", "raise", "defer", "async", "await", "spawn",
    "guard", "macro", "template", "comptime", "print", "assert", "test",
    "True", "False", "and", "or", "not", "is", "where", "self", "duck",
];

/// Map a symbol kind to the LSP CompletionItemKind number
fn symbol_kind_number(kind: &str) -> u8 {
    match kind {
        "Function" => 3,   // Function
        "Struct" => 22,    // Struct
        "Enum" => 23,      // Enum
        "Trait" => 6,      // Method; Interface=26 is more accurate but stay conservative
        "Impl" => 6,       // Method
        "Import" => 18,    // Module
        "Local" => 6,      // Variable
        _ => 6,
    }
}

/// Extract uri and position (line, character) from a JSON message
fn extract_uri_position(msg: &Value) -> (Option<String>, Option<(usize, usize)>) {
    let uri = msg
        .pointer("/params/textDocument/uri")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let line = msg
        .pointer("/params/position/line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let ch = msg
        .pointer("/params/position/character")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    match (line, ch) {
        (Some(l), Some(c)) => (uri, Some((l, c))),
        _ => (uri, None),
    }
}

/// Build an LSP Location JSON
fn location_json(uri: &str, line: usize) -> Value {
    json!({
        "uri": uri,
        "range": {
            "start": { "line": line, "character": 0 },
            "end": { "line": line, "character": 0 }
        }
    })
}

/// Send a response
fn respond(writer: &mut impl Write, id: Option<Value>, result: Value) {
    let msg = json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result });
    write_frame(writer, &msg);
}

/// Send a server notification
fn notify(writer: &mut impl Write, method: &str, params: Value) {
    let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    write_frame(writer, &msg);
}

/// Write one LSP frame (Content-Length header + JSON body)
fn write_frame(writer: &mut impl Write, msg: &Value) {
    let body = serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string());
    let _ = write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = writer.flush();
}

/// Publish diagnostics for a document
fn publish_diagnostics(writer: &mut impl Write, uri: &str, text: &str) {
    let analysis = analyze(text, uri);
    let items: Vec<Value> = analysis
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "range": {
                    "start": { "line": d.line, "character": d.char_start },
                    "end": { "line": d.line, "character": d.char_end.max(d.char_start + 1) }
                },
                "severity": d.severity,
                "source": "lang-zone",
                "message": d.message
            })
        })
        .collect();
    notify(writer, "textDocument/publishDiagnostics", json!({ "uri": uri, "diagnostics": items }));
}

/// Full analysis of a document: symbol table + diagnostics
pub fn analyze(source: &str, _uri: &str) -> Analysis {
    let mut symbols = Vec::new();
    let mut diagnostics = Vec::new();

    let tokens = Lexer::new(source).tokenize();

    // -- diagnostic 1: lexer errors --
    for (i, tok) in tokens.iter().enumerate() {
        if let Token::LexError(msg) = tok {
            diagnostics.push(Diagnostic {
                line: token_index_to_line(source, &tokens, i),
                char_start: 0,
                char_end: 0,
                message: format!("lex error: {}", msg),
                severity: 1,
            });
        }
    }

    // -- diagnostics 2+3: parser + IR --
    let mut parser = Parser::new(tokens.clone());
    match parser.parse_module() {
        Ok(module) => {
            // symbol table (AST names) + line location
            collect_symbols_from_ast(&module, source, &mut symbols);
            // IR build errors
            if let Err(e) = build_ir(&module) {
                diagnostics.push(Diagnostic {
                    line: 0,
                    char_start: 0,
                    char_end: 0,
                    message: format!("ir error: {}", e),
                    severity: 1,
                });
            }
        }
        Err(e) => {
            diagnostics.push(Diagnostic {
                line: parse_error_line(source, &tokens, &e),
                char_start: 0,
                char_end: 0,
                message: format!("parse error: {}", e),
                severity: 1,
            });
        }
    }

    // local variable symbols (let bindings)
    collect_local_symbols(source, &mut symbols);

    Analysis {
        symbols,
        diagnostics,
    }
}

/// Collect symbols from the AST (name + line location)
fn collect_symbols_from_ast(module: &crate::ast::Module, source: &str, out: &mut Vec<Symbol>) {
    for f in &module.functions {
        if let Some(line) = find_decl_line(source, "def", &f.name) {
            out.push(Symbol {
                name: f.name.clone(),
                kind: "Function".into(),
                line,
                detail: fn_signature(f),
            });
        }
    }
    for s in &module.structs {
        let line = if s.is_enum {
            find_decl_line(source, "enum", &s.name)
        } else {
            find_decl_line(source, "struct", &s.name)
        };
        if let Some(line) = line {
            out.push(Symbol {
                name: s.name.clone(),
                kind: if s.is_enum { "Enum".into() } else { "Struct".into() },
                line,
                detail: if s.is_enum {
                    format!("enum {}", s.name)
                } else {
                    format!("struct {}", s.name)
                },
            });
        }
    }
    for t in &module.traits {
        if let Some(line) = find_decl_line(source, "trait", &t.name) {
            out.push(Symbol {
                name: t.name.clone(),
                kind: "Trait".into(),
                line,
                detail: format!("trait {}", t.name),
            });
        }
    }
    for im in &module.impls {
        let target = im.type_name.clone();
        if let Some(line) = find_impl_line(source) {
            out.push(Symbol {
                name: target,
                kind: "Impl".into(),
                line,
                detail: if let Some(tr) = &im.trait_name {
                    format!("impl {} for {}", tr, im.type_name)
                } else {
                    format!("impl {}", im.type_name)
                },
            });
        }
    }
    for imp in &module.imports {
        let name = if imp.is_from {
            imp.items
                .first()
                .cloned()
                .unwrap_or_else(|| imp.path.join("."))
        } else {
            imp.path.join(".")
        };
        if let Some(line) = find_import_line(source, &name) {
            out.push(Symbol {
                name,
                kind: "Import".into(),
                line,
                detail: "import".to_string(),
            });
        }
    }
}

/// Function signature summary (for hover)
fn fn_signature(f: &crate::ast::Function) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let mut s = p.name.clone();
            s.push_str(": ");
            s.push_str(&p.ty.to_string());
            s
        })
        .collect();
    format!("def {}({})", f.name, params.join(", "))
}

/// Collect let bindings (simple line-level scan, for local-variable completion)
fn collect_local_symbols(source: &str, out: &mut Vec<Symbol>) {
    for (idx, line) in source.lines().enumerate() {
        let t = line.trim_start();
        // support `let x = ...`, `mut x = ...` and `x =:` builder block bindings
        let name = if let Some(rest) = t.strip_prefix("let ") {
            rest.split([' ', ':', '=']).next().unwrap_or("")
        } else if let Some(rest) = t.strip_prefix("mut ") {
            rest.split([' ', ':', '=']).next().unwrap_or("")
        } else if let Some(rest) = t.strip_prefix("const ") {
            rest.split([' ', ':', '=']).next().unwrap_or("")
        } else {
            ""
        };
        if !name.is_empty() && is_ident(name) {
            out.push(Symbol {
                name: name.to_string(),
                kind: "Local".into(),
                line: idx,
                detail: "local binding".to_string(),
            });
        }
    }
}

/// Collect local variable names before the cursor line (completion context filter)
fn local_names_before(source: &str, line: usize) -> Vec<String> {
    let mut names = Vec::new();
    for (idx, l) in source.lines().enumerate() {
        if idx >= line {
            break;
        }
        let t = l.trim_start();
        let name = if let Some(rest) = t.strip_prefix("let ") {
            rest.split([' ', ':', '=']).next().unwrap_or("")
        } else if let Some(rest) = t.strip_prefix("mut ") {
            rest.split([' ', ':', '=']).next().unwrap_or("")
        } else if let Some(rest) = t.strip_prefix("const ") {
            rest.split([' ', ':', '=']).next().unwrap_or("")
        } else {
            ""
        };
        if !name.is_empty() && is_ident(name) {
            names.push(name.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Find the source line of a `keyword name` declaration (0-based)
fn find_decl_line(source: &str, keyword: &str, name: &str) -> Option<usize> {
    let pat_short = format!("{} {}", keyword, name);
    let pat_long = format!("{}{} ", keyword, name);
    for (idx, line) in source.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with(&pat_short)
            || t.starts_with(&pat_long)
            || (t == pat_short.trim_end())
        {
            // exclude comments
            if !t.starts_with("//") && !t.starts_with("/*") {
                return Some(idx);
            }
        }
    }
    // fallback: none found
    None
}

/// Find the line of an `impl` block
fn find_impl_line(source: &str) -> Option<usize> {
    for (idx, line) in source.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("impl ") && !t.starts_with("//") {
            return Some(idx);
        }
    }
    None
}

/// Find the line of an import (reverse lookup by imported target name)
fn find_import_line(source: &str, target: &str) -> Option<usize> {
    for (idx, line) in source.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("import ") || t.starts_with("from ") {
            if t.contains(target) && !t.starts_with("//") {
                return Some(idx);
            }
        }
    }
    None
}

/// Simple identifier check
fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => chars.all(|c| c.is_alphanumeric() || c == '_'),
        _ => false,
    }
}

/// Take the word under the cursor (only if the char at the cursor is a word char)
fn word_at(source: &str, line: usize, character: usize) -> Option<String> {
    let text = source.lines().nth(line)?;
    let chars: Vec<char> = text.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '.';
    let _anchor = chars.get(character).copied().filter(|c| is_word(*c))?;
    let mut start = character;
    while start > 0 && chars.get(start - 1).copied().map(is_word).unwrap_or(false) {
        start -= 1;
    }
    let mut end = character;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    let w: String = chars[start..end].iter().collect();
    if w.is_empty() {
        None
    } else {
        Some(w)
    }
}

// ------------------------- error line location -------------------------

/// Map a parser error message's token index to a source line
fn parse_error_line(source: &str, tokens: &[Token], err: &str) -> usize {
    // Error formats: `Expected X, got Y at pos N` / `Parse error: ... at pos N`
    let pos = err
        .rsplit_once("at pos ")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .and_then(|n| n.parse::<usize>().ok());
    let line = match pos {
        // parser's "at pos N" usually points to the expected position (tokens consumed);
        // minus 1 yields the actual index of the failing token; keep pos=0 unchanged.
        Some(p) => token_index_to_line(source, tokens, p.saturating_sub(1)),
        None => 0,
    };
    // The parser error index may point at an indentation marker on the next line;
    // if the located line is blank, fall back one line.
    if let Some(text) = source.lines().nth(line) {
        if text.trim().is_empty() && line > 0 {
            return line - 1;
        }
    }
    line
}

/// Map a token index to a 0-based line number (greedy text matching; fallback 0)
pub fn token_index_to_line(source: &str, tokens: &[Token], target: usize) -> usize {
    if target >= tokens.len() {
        return source.lines().count().saturating_sub(1);
    }
    let mut byte_pos: usize = 0;
    for (i, tok) in tokens.iter().enumerate() {
        if i == target {
            return line_of_byte(source, byte_pos);
        }
        if matches!(tok, Token::Newline) {
            // newline token: advance to the start of the next line (key for line +1)
            byte_pos = next_line_start(source, byte_pos);
            continue;
        }
        if is_virtual_token(tok) {
            continue;
        }
        if let Some(text) = token_source_text(tok) {
            if let Some(found) = find_text_after(source, byte_pos, &text) {
                byte_pos = found + text.len();
            }
            // not found: keep byte_pos unchanged (loose matching for the next token)
        }
    }
    // fallback: position after the last real token
    line_of_byte(source, byte_pos)
}

/// Advance from `from` to the start of the next line (just after the first '\n')
fn next_line_start(source: &str, from: usize) -> usize {
    let bytes = source.as_bytes();
    let mut pos = from.min(bytes.len());
    while pos < bytes.len() && bytes[pos] != b'\n' {
        pos += 1;
    }
    if pos < bytes.len() {
        pos + 1
    } else {
        pos
    }
}

/// Byte offset -> 0-based line number
pub fn line_of_byte(source: &str, byte_pos: usize) -> usize {
    let byte_pos = byte_pos.min(source.len());
    source.as_bytes()[..byte_pos]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

/// Virtual tokens (do not consume source text)
fn is_virtual_token(tok: &Token) -> bool {
    matches!(tok, Token::Indent | Token::Dedent | Token::Newline)
}

/// Source text representation of a token (for greedy location; None if not representable)
fn token_source_text(tok: &Token) -> Option<String> {
    use Token::*;
    Some(match tok {
        IntLit(v) => v.to_string(),
        Ident(s) | MagicMethod(s) => s.clone(),
        StrLit(s) => format!("\"{}\"", s),
        FStrLit(s) => format!("f\"{}\"", s),
        RawStrLit(s) => format!("r\"{}\"", s),
        Def => "def".into(),
        Struct => "struct".into(),
        Enum => "enum".into(),
        Trait => "trait".into(),
        Impl => "impl".into(),
        Const => "const".into(),
        Mut => "mut".into(),
        Ref => "ref".into(),
        Owned => "owned".into(),
        Let => "let".into(),
        Iterator => "iterator".into(),
        If => "if".into(),
        Elif => "elif".into(),
        Else => "else".into(),
        Match => "match".into(),
        Case => "case".into(),
        Guard => "guard".into(),
        For => "for".into(),
        In => "in".into(),
        While => "while".into(),
        Loop => "loop".into(),
        Break => "break".into(),
        Continue => "continue".into(),
        Return => "return".into(),
        With => "with".into(),
        Defer => "defer".into(),
        Block => "block".into(),
        Try => "try".into(),
        Catch => "catch".into(),
        Finally => "finally".into(),
        Raise => "raise".into(),
        Raises => "raises".into(),
        Test => "test".into(),
        Assert => "assert".into(),
        Suite => "suite".into(),
        Setup => "setup".into(),
        Teardown => "teardown".into(),
        Check => "check".into(),
        Async => "async".into(),
        Await => "await".into(),
        Spawn => "spawn".into(),
        Go => "go".into(),
        Select => "select".into(),
        Yield => "yield".into(),
        Import => "import".into(),
        From => "from".into(),
        As => "as".into(),
        Where => "where".into(),
        Self_ => "self".into(),
        Duck => "duck".into(),
        Macro => "macro".into(),
        Template => "template".into(),
        Comptime => "comptime".into(),
        And => "and".into(),
        Or => "or".into(),
        Not => "not".into(),
        Is => "is".into(),
        True => "True".into(),
        False => "False".into(),
        Eq => "=".into(),
        EqEq => "==".into(),
        NotEq => "!=".into(),
        Lt => "<".into(),
        Gt => ">".into(),
        Le => "<=".into(),
        Ge => ">=".into(),
        Plus => "+".into(),
        Minus => "-".into(),
        Star => "*".into(),
        Slash => "/".into(),
        Percent => "%".into(),
        StarStar => "**".into(),
        PlusEq => "+=".into(),
        MinusEq => "-=".into(),
        StarEq => "*=".into(),
        SlashEq => "/=".into(),
        PercentEq => "%=".into(),
        AndEq => "&=".into(),
        OrEq => "|=".into(),
        XorEq => "^=".into(),
        ShlEq => "<<=".into(),
        ShrEq => ">>=".into(),
        PowEq => "**=".into(),
        Amp => "&".into(),
        Pipe_ => "|".into(),
        Caret => "^".into(),
        Shl => "<<".into(),
        Shr => ">>".into(),
        AmpAmp => "&&".into(),
        PipePipe => "||".into(),
        Colon => ":".into(),
        ColonEq => ":=".into(),
        Comma => ",".into(),
        Dot => ".".into(),
        DotDot => "..".into(),
        DotDotEq => "..=".into(),
        DotDotDot => "...".into(),
        Semicolon => ";".into(),
        PathSep => "::".into(),
        Arrow => "->".into(),
        FatArrow => "=>".into(),
        Pipe => "|>".into(),
        BackPipe => "<|".into(),
        Exclamation => "!".into(),
        Question => "?".into(),
        QuestionQuestion => "??".into(),
        SafeNav => "?.".into(),
        At => "@".into(),
        Underscore => "_".into(),
        Dollar => "$".into(),
        Tilde => "~".into(),
        LParen => "(".into(),
        RParen => ")".into(),
        LBrack => "[".into(),
        RBrack => "]".into(),
        LBrace => "{".into(),
        RBrace => "}".into(),
        Backtick => "`".into(),
        BuildAssign => "=:".into(),
        // remaining (floats, triple-quoted strings, etc.) are hard to reverse-map -> None
        _ => return None,
    })
}

/// Find `text` starting at or after `start`, skipping whitespace and comments (None if absent)
fn find_text_after(source: &str, start: usize, text: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut pos = start.min(bytes.len());
    loop {
        // skip whitespace
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t' || bytes[pos] == b'\r' || bytes[pos] == b'\n') {
            pos += 1;
        }
        if pos >= bytes.len() {
            return None;
        }
        // skip line comments
        if pos + 1 < bytes.len() && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        // skip block comments
        if pos + 1 < bytes.len() && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < bytes.len() && !(bytes[pos] == b'*' && bytes[pos + 1] == b'/') {
                pos += 1;
            }
            pos = (pos + 2).min(bytes.len());
            continue;
        }
        break;
    }
    if source[pos..].starts_with(text) {
        Some(pos)
    } else {
        // try from the next non-whitespace char (prevents scan getting stuck)
        let next = pos + 1;
        if next < bytes.len() {
            find_text_after(source, next, text)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_of_byte_counts_newlines() {
        assert_eq!(line_of_byte("abc", 2), 0);
        assert_eq!(line_of_byte("a\nb\nc", 4), 2); // pos 4 = 'c' -> line 2 (0-based)
        assert_eq!(line_of_byte("a\nb\nc", 0), 0);
        assert_eq!(line_of_byte("", 0), 0);
    }

    #[test]
    fn word_at_works() {
        let src = "def main() = print(hello_world)";
        assert_eq!(word_at(src, 0, 5), Some("main".to_string()));
        assert_eq!(word_at(src, 0, 8), None); // "("
        assert_eq!(word_at(src, 0, 23), Some("hello_world".to_string()));

        let src2 = "    print(add(1, 2))";
        assert_eq!(word_at(src2, 0, 12), Some("add".to_string()));
        assert_eq!(word_at(src2, 0, 10), Some("add".to_string()));
        assert_eq!(word_at(src2, 0, 13), None);
    }

    #[test]
    fn analyze_clean_source_no_diagnostics() {
        let src = "def main() =\n    print(1)\n";
        let a = analyze(src, "file:///test.lz");
        assert!(a.diagnostics.is_empty(), "expected no diagnostics, got {:?}", a.diagnostics);
        assert!(a.symbols.iter().any(|s| s.name == "main" && s.kind == "Function"));
    }

    #[test]
    fn analyze_bad_source_has_diagnostic() {
        // missing colon after `def` triggers a parse error
        let src = "def main() =\n    print(1)\ndef broken( = )\n";
        let a = analyze(src, "file:///test.lz");
        assert!(!a.diagnostics.is_empty(), "expected parse diagnostic");
    }

    #[test]
    fn symbols_include_struct_trait_enum() {
        let src = "struct Point =\n    x: int\n\ntrait Draw =\n    def draw(self) -> () = ...\n\nenum Color:\n    Red\n\ndef main() =\n    print(1)\n";
        let a = analyze(src, "file:///test.lz");
        let names: Vec<&str> = a.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Point"), "symbols: {:?}", names);
        assert!(names.contains(&"Draw"), "symbols: {:?}", names);
        assert!(names.contains(&"Color"), "symbols: {:?}", names);
    }

    #[test]
    fn token_index_to_line_rough() {
        let src = "def main() =\n    print(1)\n    x = 2\n";
        let tokens = Lexer::new(src).tokenize();
        // locate the Ident("x") index and verify it maps to line 2 (0-based)
        let idx = tokens
            .iter()
            .position(|t| matches!(t, Token::Ident(s) if s == "x"))
            .unwrap();
        let line = token_index_to_line(src, &tokens, idx);
        assert_eq!(line, 2, "tokens={:?}, idx={}", tokens, idx);
    }

    #[test]
    fn token_index_to_line_second_line() {
        let src = "def main() =\n    print(1)\n    x = 2\n";
        let tokens = Lexer::new(src).tokenize();
        let idx = tokens
            .iter()
            .position(|t| matches!(t, Token::Ident(s) if s == "print"))
            .unwrap();
        let line = token_index_to_line(src, &tokens, idx);
        assert_eq!(line, 1, "tokens={:?}, idx={}", tokens, idx);
    }
}
