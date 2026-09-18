; ---------------------------------------------------------------------------
; Lang-Zone (lz) indentation queries
; Authority: E:\IDEProjects\AI\lang-zone\SYNTAX (spec v3.3) — indentation is
; 4 spaces, block bodies follow a `:` (or a declaration `=`) on the same line.
;
; The grammar is intentionally flat (blocks are not modeled), so indentation
; is driven by begin/outdent markers:
;   - `:` block headers and `=` declaration headers -> indent the next line
;   - continuation keywords (else/elif/catch/finally/case) -> outdent
;
; NOTE: anonymous-token child patterns like `(if_statement ":" @indent.begin)`
; are NOT used: this grammar's generated node-types do not expose `:`/`=` as
; children of statement nodes, which makes such patterns "Impossible" and
; fails the whole query (breaking language load). Top-level string patterns
; match the tokens directly and compile cleanly.
; ---------------------------------------------------------------------------

; --- `:` block headers ------------------------------------------------------
":" @indent.begin

; --- `=` declaration headers (fields/bodies always start on the next line) --
"=" @indent.begin

; --- continuation lines dedent to the parent block --------------------------
(else_statement) @outdent
(elif_statement) @outdent
(catch_statement) @outdent
(finally_statement) @outdent
(case_statement) @outdent
