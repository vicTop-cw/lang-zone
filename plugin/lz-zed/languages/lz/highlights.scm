; ---------------------------------------------------------------------------
; Lang-Zone (lz) highlights
; Authority: E:\IDEProjects\AI\lang-zone\SYNTAX (spec v3.3)
;
; Capture names follow Zed / TextMate theme conventions so every theme maps
; them to sensible colors: keyword, keyword.control, keyword.import,
; keyword.exception, keyword.operator, constant.builtin.boolean, type.builtin,
; constant.builtin, function.builtin, function.magic, function, type, field,
; label, module, string, string.regex, number, number.float, comment,
; operator, punctuation.bracket, punctuation.delimiter, punctuation.special,
; attribute, variable.
; ---------------------------------------------------------------------------

; --- keywords: declarations (spec 00 §1.1) ---------------------------------
["def" "struct" "enum" "trait" "impl" "type" "const" "mut" "ref" "let" "owned" "magic" "duck" "iterator"] @keyword

; --- keywords: generics (spec 00 §1.7) --------------------------------------
["where" "Self"] @keyword

; --- keywords: control flow (spec 00 §1.2) ----------------------------------
["if" "elif" "else" "match" "case" "guard" "for" "while" "loop" "block" "pass" "break" "continue" "return" "with" "defer"] @keyword.control

; --- keywords: async / concurrency (spec 00 §1.4) ---------------------------
["async" "await" "spawn" "go"] @keyword.control

; --- keywords: generator (spec 00 §1.5) -------------------------------------
["yield"] @keyword.control

; --- keywords: exceptions (spec 00 §1.3) ------------------------------------
["raise" "raises" "try" "catch" "finally"] @keyword.exception

; --- keywords: imports (spec 00 §1.6) ---------------------------------------
["import" "from" "as"] @keyword.import

; --- keywords: operator words (spec 00 §1.10, 12 §1.4) ----------------------
["and" "or" "not" "is" "in"] @keyword.operator

; --- keywords: macro / comptime / templates (spec 00 §1.8) ------------------
["macro" "comptime" "template"] @keyword

; --- keywords: test framework (spec 00 §1.9, 15) ----------------------------
["test" "suite" "setup" "teardown" "assert" "check"] @keyword

; --- abstract-method marker `...` (spec 00 §1.1, 06c) -----------------------
"..." @keyword

; --- boolean literal keywords (spec 00 §1.11) -------------------------------
["True" "False"] @constant.builtin.boolean

; --- duck soft keywords (spec 附录B §1.13; only lexed inside duck bodies) ---
(duck_soft_keyword) @keyword.control

; --- builtins (prelude, NOT keywords — spec 00 §1.12, 99) -------------------
(builtin_type) @type.builtin
(builtin_type_value) @type.builtin
(builtin_constructor) @constant.builtin
(builtin_function) @function.builtin

; --- identifiers / names ----------------------------------------------------
(identifier) @variable
(magic_method) @function.magic

; declaration names
(function_definition name: (_) @function)
(iterator_definition name: (identifier) @function)
(macro_definition name: (identifier) @function)
(template_definition name: (identifier) @function)
(struct_definition name: (identifier) @type)
(enum_definition name: (identifier) @type)
(trait_definition name: (identifier) @type)
(duck_definition name: (identifier) @type)
(type_alias name: (identifier) @type)
(const_definition name: (identifier) @constant)
(suite_definition name: (identifier) @type)
(block_statement name: (identifier) @label)
(impl_definition (identifier) @type)
(field_definition name: (_) @field)

; member access / method calls: obj.method()  -> method
(postfix_expression "." (identifier) @function)
; safe navigation: obj?.field -> field
(postfix_expression "?." (identifier) @field)

; import paths: std.io.print
(import_statement (identifier) @module)

; decorators
(decorator_statement "@" @attribute)
(decorator_statement (identifier) @function)

; attribute macros: #!bin macro / #!export(...) / #![derive(...)]
(attribute_macro) @attribute

; --- literals ---------------------------------------------------------------
(string) @string
(regex_literal) @string.regex
(float) @number.float
(integer) @number

; --- comments ---------------------------------------------------------------
(line_comment) @comment
(block_comment) @comment

; --- operators --------------------------------------------------------------
["+" "-" "*" "/" "%" "**" "==" "!=" "<" ">" "<=" ">=" "&&" "||" "!" "&" "|" "^" "<<" ">>" "~" "=" "+=" "-=" "*=" "/=" "%=" "**=" "&=" "|=" "^=" "<<=" ">>=" ":=" "|>" "??" ".." "..=" "=:" "^:" "~:" "*:" "?" "?." "@"] @operator

; --- punctuation ------------------------------------------------------------
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[":" "," "."] @punctuation.delimiter
["->" "=>"] @punctuation.special
