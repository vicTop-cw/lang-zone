; ============================================================
; Cypy Syntax Highlighting for Zed
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

; === Logical keywords (and, or, not, is, in) ===
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

; === Annotations (@decorator) ===
(annotation) @tag

; ============================================================
; Keywords by category
; ============================================================

; --- Imports & Modules (use @preproc for distinct yellow/green) ---
"import" @preproc
"from" @preproc
"as" @preproc

; --- Control flow (use @tag for distinct orange/red) ---
"if" @tag
"elif" @tag
"else" @tag
"match" @tag
"case" @tag
"guard" @tag
"for" @tag
"while" @tag
"return" @tag
"try" @tag
"except" @tag
"finally" @tag
"raise" @tag
"break" @tag
"continue" @tag
"yield" @tag
"defer" @tag
"with" @tag
"pass" @tag

; --- All other keywords (use @keyword purple) ---
"def" @keyword
"class" @keyword
"struct" @keyword
"enum" @keyword
"trait" @keyword
"impl" @keyword
"type" @keyword
"let" @keyword
"const" @keyword
"mut" @keyword
"implicit" @keyword
"macro" @keyword
"async" @keyword
"await" @keyword
"spawn" @keyword
"go" @keyword
"test" @keyword
"suite" @keyword
"setup" @keyword
"teardown" @keyword
"assert" @keyword
"comptime" @keyword
"lambda" @keyword
"global" @keyword
"nonlocal" @keyword
"del" @keyword

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

; --- Variable declarations: let/const/mut varName ---
(keyword_stmt
  ["let" "const" "mut"]
  (identifier) @variable)

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