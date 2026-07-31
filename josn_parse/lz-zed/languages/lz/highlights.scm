; ============================================================
; LZ Syntax Highlighting for Zed
; Capture order: general → specific (last matching wins)
; ============================================================

; === Comments ===
(comment) @comment

; === Strings ===
(string) @string

; === Numbers ===
(number) @number

; === Constants ===
(constant) @constant.builtin

; === Logical keywords (and, or, not, is, in, as, from) ===
(logic_kw) @keyword.operator

; === Self / Super ===
(self_kw) @variable.builtin

; === Operators ===
(operator) @operator

; === Punctuation ===
(punctuation) @punctuation

; === Wildcard ===
(wildcard) @variable.special

; === Ellipsis ===
(punctuation) @punctuation.special

; === Annotations (@decorator, @macro!) ===
(annotation) @tag

; ============================================================
; Keywords by category
; Zed only supports: @keyword, @preproc, @label, @constant,
;   @function, @type, @variable, @tag, @operator, etc.
; @conditional, @repeat, @exception, @include are NOT supported!
; ============================================================

; --- Imports & Modules (use @preproc for distinct color) ---
"import" @preproc
"export" @preproc
"use" @preproc
"mod" @preproc
"pub" @preproc
"from" @preproc
"as" @preproc

; --- Control flow (use @tag for distinct orange/red color) ---
"if" @tag
"elif" @tag
"else" @tag
"match" @tag
"case" @tag
"guard" @tag
"for" @tag
"while" @tag
"loop" @tag
"return" @tag
"try" @tag
"catch" @tag
"finally" @tag
"raise" @tag
"throw" @tag
"throws" @tag
"raises" @tag
"panic" @tag
"break" @tag
"continue" @tag
"yield" @tag
"defer" @tag
"with" @tag

; --- All other keywords (use @keyword) ---
"def" @keyword
"iterator" @keyword
"class" @keyword
"struct" @keyword
"enum" @keyword
"trait" @keyword
"duck" @keyword
"type" @keyword
"let" @keyword
"var" @keyword
"const" @keyword
"impl" @keyword
"macro" @keyword
"template" @keyword
"alias" @keyword
"Self" @type
"async" @keyword
"await" @keyword
"spawn" @keyword
"go" @keyword
"comptime" @keyword
"unsafe" @keyword
"extern" @keyword
"abstract" @keyword
"override" @keyword
"virtual" @keyword
"final" @keyword
"sealed" @keyword
"static" @keyword
"mut" @keyword
"ref" @keyword
"owned" @keyword
"magic" @keyword
"pass" @keyword
"test" @keyword
"suite" @keyword
"setup" @keyword
"teardown" @keyword
"assert" @keyword
"check" @keyword
"where" @keyword
"do" @keyword
"new" @keyword
"del" @keyword
"sizeof" @keyword
"typeof" @keyword
"unless" @keyword
"until" @keyword
"when" @keyword
"then" @keyword
"begin" @keyword
"end" @keyword
"require" @keyword
"include" @keyword
"extend" @keyword
"mixin" @keyword
"except" @keyword
"ensure" @keyword

; ============================================================
; DEFAULT identifier — must come BEFORE specific overrides
; because Tree-sitter uses last-match-wins priority
; ============================================================
(identifier) @variable

; ============================================================
; Declaration names — OVERRIDE the default @variable above
; ============================================================

; --- Function names: def funcName ---
(keyword_stmt
  "def"
  (identifier) @function)

; --- Class / Struct / Enum / Trait names ---
(keyword_stmt
  ["class" "struct" "enum" "trait"]
  (identifier) @type)

; --- Variable declarations: let/var/mut/const/static/ref varName ---
(keyword_stmt
  ["let" "var" "mut" "const" "static" "ref"]
  (identifier) @variable)

; --- Module / Import names: mod/import/export/use modName ---
(keyword_stmt
  ["mod" "import" "export" "use"]
  (identifier) @module)

; --- Type alias: type TypeName ---
(keyword_stmt
  "type"
  (identifier) @type)

; ============================================================
; Identifier-based patterns — OVERRIDE the default @variable
; ============================================================

; --- Types (CamelCase identifiers) ---
((identifier) @type
  (#match? @type "^[A-Z][a-zA-Z0-9_]*$"))

; --- Built-in types (lowercase) ---
((identifier) @type.builtin
  (#match? @type.builtin "^(int|float|bool|str|string|list|dict|set|tuple|range|slice|bytes|bytearray|memoryview|complex|object|Any|Callable|Iterable|Iterator|Generator|Coroutine|Union|Optional|Self|Exception|ValueError|TypeError|KeyError|IndexError|IOError|RuntimeError|NotImplementedError|StopIteration|AssertionError|AttributeError|ImportError|MemoryError|OverflowError|RecursionError|ReferenceError|SystemError|SystemExit|ZeroDivisionError|FileNotFoundError|PermissionError|TimeoutError|BufferError|ArithmeticError|LookupError|OSError|EOFError|FloatingPointError|GeneratorExit|KeyboardInterrupt|ModuleNotFoundError|NotADirectoryError|InterruptedError|IsADirectoryError|ProcessLookupError|BlockingIOError|ChildProcessError|ConnectionError|BrokenPipeError|ConnectionAbortedError|ConnectionRefusedError|ConnectionResetError|FileExistsError|TabError|UnicodeError|UnicodeDecodeError|UnicodeEncodeError|UnicodeTranslateError|Warning|DeprecationWarning|FutureWarning|ImportWarning|PendingDeprecationWarning|ResourceWarning|RuntimeWarning|SyntaxWarning|UnicodeWarning|UserWarning|BytesWarning)$"))

; --- Magic methods (__xxx__) ---
((identifier) @function
  (#match? @function "^__[a-zA-Z0-9_]+__$"))