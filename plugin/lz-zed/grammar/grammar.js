// Lang-Zone (lz) tree-sitter grammar
// ---------------------------------------------------------------------------
// Authority: E:\IDEProjects\AI\lang-zone\SYNTAX (spec v3.3, 2026-08-04/05)
//
// Design notes
// ---------------------------------------------------------------------------
// 1. This grammar is optimized for SYNTAX HIGHLIGHTING: token-level precision
//    (keywords / literals / operators / comments / identifiers), with a
//    deliberately permissive, flat structure. lz is indentation-sensitive
//    (4-space blocks); block structure is carried by the editor, not modeled
//    here, so the parser stays robust across all indent shapes.
// 2. All lexical rules follow SYNTAX/00-词法基础.md + 附录B + 12-操作符.md:
//    - keywords: the 61 reserved words (True/False are literal keywords)
//    - identifiers: [a-zA-Z_][a-zA-Z0-9_]* or Unicode letters; `_` special
//    - magic methods: __name__ (double-underscore, Python dunder style)
//    - numbers: decimal/hex 0x/octal 0o/binary 0b ints with `_` separators;
//      floats need digits on both sides of `.` (1. and .5 are illegal)
//    - strings: "…", f"…", r"…", """…""", f"""…""", r"""…""", f```…```, r```…```
//    - comments: // line, /* */ block (non-nested)
//    - `#` is an ATTRIBUTE macro marker, never a comment
// 3. Indentation, block bodies and semantic disambiguation (dict vs set,
//    ternary vs statement if, `..` rest vs range) are intentionally left to
//    GLR / not modeled; highlighting is driven by token captures, so these
//    do not affect color correctness.
// ---------------------------------------------------------------------------

module.exports = grammar({
  name: 'lz',

  // _expr and pattern rules share literal tokens (True/False/string/number/
  // identifier/(/[/{/…); they never co-occur meaningfully in one source
  // position, so let the GLR machinery pick either branch instead of
  // failing generation.
  conflicts: $ => [
    [$._expr, $._pattern],
    [$._expr, $.variant_pattern],
    [$._expr, $._pattern, $.variant_pattern],
    [$._expr, $.tuple_pattern],
    [$._expr, $.list_pattern],
    [$._expr, $.dict_pattern],
    [$._expr, $.range_pattern],
    [$._expr, $.or_pattern],
    [$._expr, $.rest_pattern],
    [$._pattern, $.assignment_target],
    [$._pattern, $.variant_pattern],
    [$._pattern, $.tuple_pattern],
    [$._pattern, $.list_pattern],
    [$._pattern, $.dict_pattern],
    [$._pattern, $.range_pattern],
    [$._pattern, $.or_pattern],
    [$._pattern, $.rest_pattern],
    [$._pattern, $.ref_pattern],
    [$.tuple_literal, $.tuple_pattern],
    [$.list_literal, $.list_pattern],
    [$.dict_literal, $.dict_pattern],
    // `*`/`^`/`~`/`?` are both binary operators and unary/postfix suffixes;
    // GLR picks the branch that matches the surrounding context.
    [$.bit_xor_expression, $.unary_expression, $.postfix_expression],
    [$.mul_div_expression, $.unary_expression, $.postfix_expression],
    [$.bit_xor_expression, $.mul_div_expression, $.postfix_expression],
    [$.bit_xor_expression, $.bit_and_expression, $.postfix_expression],
    [$.bit_xor_expression, $.bit_or_expression, $.postfix_expression],
    [$.bit_xor_expression, $.shift_expression, $.postfix_expression],
    [$.bit_xor_expression, $.add_sub_expression, $.postfix_expression],
    [$.bit_xor_expression, $.comparison_expression, $.postfix_expression],
    [$.bit_xor_expression, $.pipe_expression, $.postfix_expression],
    [$.bit_xor_expression, $.null_coalescing_expression, $.postfix_expression],
    [$.bit_xor_expression, $.power_expression, $.postfix_expression],
    [$.bit_xor_expression, $.range_expression, $.postfix_expression],
    [$.bit_xor_expression, $.walrus_expression, $.postfix_expression],
    [$.bit_xor_expression, $.or_expression, $.postfix_expression],
    [$.bit_xor_expression, $.and_expression, $.postfix_expression],
    [$.bit_xor_expression, $.mul_div_expression, $.unary_expression, $.postfix_expression],
    [$.bit_xor_expression, $.bit_and_expression, $.unary_expression, $.postfix_expression],
    [$.bit_xor_expression, $.add_sub_expression, $.unary_expression, $.postfix_expression],
    [$.bit_xor_expression, $.shift_expression, $.postfix_expression],
    [$.bit_xor_expression, $.comparison_expression, $.postfix_expression],
    [$.bit_xor_expression, $.bit_or_expression, $.postfix_expression],
    [$.bit_xor_expression, $.power_expression, $.postfix_expression],
    [$.bit_and_expression, $.unary_expression],
    [$.bit_or_expression, $.or_pattern],
    [$.range_expression, $.range_pattern],
    [$._expr, $.test_definition],
    [$.if_statement, $.if_expression, $.range_expression],
    [$.if_statement, $.if_expression],
    [$.if_statement, $.ternary_expression, $.if_expression],
    [$.if_expression, $.walrus_expression],
    [$.if_expression, $.or_expression],
    [$.if_expression, $.and_expression],
    [$.if_expression, $.comparison_expression],
    [$.if_expression, $.pipe_expression],
    [$.if_expression, $.null_coalescing_expression],
    [$.if_expression, $.range_expression],
    [$.rest_pattern],
    [$._pattern, $.catch_statement],
    [$._pattern, $.catch_statement, $.variant_pattern],
    [$._expr, $.continue_statement],
    [$.match_expression, $.range_expression],
    [$.match_expression, $.walrus_expression],
    [$.match_expression, $.or_expression],
    [$.match_expression, $.and_expression],
    [$.match_expression, $.comparison_expression],
    [$.match_expression, $.pipe_expression],
    [$.match_expression, $.null_coalescing_expression],
    [$.match_expression, $.bit_or_expression],
    [$.match_expression, $.bit_xor_expression],
    [$.match_expression, $.bit_and_expression],
    [$.match_expression, $.ternary_expression],
    [$.match_expression, $.if_expression],
    [$.match_expression, $.if_statement],
    [$.match_expression, $.unary_expression],
    [$.match_expression, $.postfix_expression],
    [$.match_expression, $.test_definition],
    [$.match_expression, $.assignment_target],
    [$.match_expression],
    [$.bit_xor_expression, $.postfix_expression, $.await_expression],
    [$.bit_xor_expression, $.postfix_expression, $.spawn_expression],
    [$.ternary_expression, $.if_expression],
    [$.ternary_expression, $.if_statement],
    [$.ternary_expression, $.range_expression],
    [$.ternary_expression, $.walrus_expression],
    [$.ternary_expression, $.or_expression],
    [$.ternary_expression, $.and_expression],
    [$.ternary_expression, $.comparison_expression],
    [$.ternary_expression, $.pipe_expression],
    [$.ternary_expression, $.null_coalescing_expression],
    [$.ternary_expression, $.bit_or_expression],
    [$.ternary_expression, $.bit_xor_expression],
    [$.ternary_expression, $.bit_and_expression],
    [$.ternary_expression],
    [$.ternary_expression, $.if_expression, $.range_expression],
    [$.ternary_expression, $.if_expression, $.walrus_expression],
    [$.ternary_expression, $.if_expression, $.or_expression],
    [$.ternary_expression, $.if_expression, $.and_expression],
    [$.ternary_expression, $.if_expression, $.comparison_expression],
    [$.ternary_expression, $.if_expression, $.pipe_expression],
    [$.ternary_expression, $.if_expression, $.null_coalescing_expression],
    [$.if_expression],
    [$.where_clause],
    [$.type_arguments],
    [$._type, $.type_arguments],
    [$.type_parameters, $.type_arguments],
    [$.import_statement],
    [$.range_pattern],
    [$.or_pattern],
    [$.variant_pattern],
    [$.range_pattern, $.or_pattern],
    [$.range_pattern, $.rest_pattern],
    [$.duck_body, $.tuple_literal],
    [$.duck_body, $._expr],
    [$.duck_body, $.identifier],
    [$.duck_body, $.closure],
    [$.duck_body, $.list_literal],
    [$.duck_body, $.dict_literal],
    [$.duck_body],
    [$.call_args, $.tuple_literal],
    [$.call_args, $.list_literal],
    [$.call_args, $.dict_literal],
    [$._expr, $.closure],
  ],

  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  rules: {
    // ---------------------------------------------------------------------
    // Top level
    // ---------------------------------------------------------------------
    source_file: $ => repeat($._statement),

    _statement: $ => choice(
      $.attribute_macro,
      $.decorator_statement,
      $.function_definition,
      $.iterator_definition,
      $.struct_definition,
      $.enum_definition,
      $.trait_definition,
      $.impl_definition,
      $.duck_definition,
      $.type_alias,
      $.const_definition,
      $.magic_block,
      $.import_statement,
      $.macro_definition,
      $.template_definition,
      $.test_definition,
      $.suite_definition,
      $.setup_statement,
      $.teardown_statement,
      $.block_statement,
      $.comptime_statement,
      $.if_statement,
      $.elif_statement,
      $.else_statement,
      $.for_statement,
      $.while_statement,
      $.loop_statement,
      $.case_statement,
      $.guard_statement,
      $.with_statement,
      $.defer_statement,
      $.try_statement,
      $.catch_statement,
      $.finally_statement,
      $.raise_statement,
      $.raises_statement,
      $.return_statement,
      $.break_statement,
      $.continue_statement,
      $.yield_statement,
      $.pass_statement,
      $.assert_statement,
      $.check_statement,
      $.declarative_for,
      $.let_statement,
      $.assignment_statement,
      $.field_definition,
      $.expression_statement,
    ),

    // ---------------------------------------------------------------------
    // Attributes / decorators
    // ---------------------------------------------------------------------
    // #!bin macro | #!export(Rust) | #![derive(Clone)] | #
    attribute_macro: $ => token(choice(
      seq('#![', /[^\n]*/, ']'),
      seq('#!', /[^\s\n]*/),
      seq('#[', /[^\n]*/, ']'),
      '#'
    )),

    // @export(Rust) / @decorator
    decorator_statement: $ => prec.left(seq(
      '@',
      $.identifier,
      optional(seq('(', optional($.call_args), ')')),
    )),

    // ---------------------------------------------------------------------
    // Declarations
    // ---------------------------------------------------------------------
    // [async] def name[T](params) -> Type [where ...] [=]
    function_definition: $ => prec.left(seq(
      optional('async'),
      'def',
      field('name', choice($.identifier, $.magic_method)),
      optional($.type_parameters),
      optional($.parameters),
      optional($.return_annotation),
      optional($.where_clause),
      optional('='),
    )),

    // iterator name[T](params) -> Type [where ...] [=]
    iterator_definition: $ => prec.left(seq(
      'iterator',
      field('name', $.identifier),
      optional($.type_parameters),
      optional($.parameters),
      optional($.return_annotation),
      optional($.where_clause),
      optional('='),
    )),

    // struct Name[T] =
    struct_definition: $ => prec.left(seq(
      'struct',
      field('name', $.identifier),
      optional($.type_parameters),
      optional('='),
    )),

    // enum Name[T] =
    enum_definition: $ => prec.left(seq(
      'enum',
      field('name', $.identifier),
      optional($.type_parameters),
      optional('='),
    )),

    // trait Name[T] =
    trait_definition: $ => prec.left(seq(
      'trait',
      field('name', $.identifier),
      optional($.type_parameters),
      optional('='),
    )),

    // impl [T]? Trait for Type =   |   impl Type =
    impl_definition: $ => prec.left(seq(
      'impl',
      repeat1(choice(
        $.identifier, $.builtin_type, $.builtin_type_value, $.builtin_constructor,
        $.type_arguments, 'for', '.', ',', 'Self'
      )),
      optional('='),
    )),

    // duck Name =   |   duck Name:
    duck_definition: $ => prec.left(seq(
      'duck',
      field('name', $.identifier),
      optional(choice(seq('=', optional($.duck_body)), ':')),
    )),

    duck_body: $ => repeat1(choice(
      $.duck_soft_keyword,
      $.regex_literal,
      $._expr,
      $.identifier,
      $.builtin_type, $.builtin_type_value, $.builtin_constructor, $.builtin_function,
      $.magic_method,
      ':', ',', '.', '==', '!=', '..', '|', '->', '=>', '(', ')', '[', ']', '{', '}',
    )),

    // Soft keywords: reserved ONLY inside duck bodies (spec 附录B §1.13)
    duck_soft_keyword: $ => token(choice(
      'require', 'optional', 'exact', 'min', 'max', 'range',
      'at_least', 'at_most', 'satisfies', 'sealed', 'default',
      'StackType', 'RefType', 'Any'
    )),

    // type Name[T] = Type
    type_alias: $ => prec.left(seq(
      'type',
      field('name', $.identifier),
      optional($.type_parameters),
      optional(seq('=', $._type)),
    )),

    // const NAME: Type = expr
    const_definition: $ => prec.left(seq(
      'const',
      field('name', $.identifier),
      optional(seq(':', $._type)),
      optional(seq('=', $._expr)),
    )),

    // magic __name__: | magic __new__(...) | magic __implicit_from__
    magic_block: $ => prec.left(seq(
      'magic',
      optional(choice($.magic_method, $.identifier)),
      optional(seq('(', optional($.call_args), ')')),
      optional(':'),
    )),

    // ---------------------------------------------------------------------
    // Imports
    // ---------------------------------------------------------------------
    // import std.io | import std.io as Map | import macro m
    // from std.io import print, read | from std.io import *
    import_statement: $ => choice(
      seq('import', optional('macro'), $._module_path, optional(seq('as', $.identifier))),
      seq('from', $._module_path, 'import', optional(choice(
        '*',
        seq($.identifier, repeat(seq(',', $.identifier))),
      ))),
    ),

    _module_path: $ => seq(
      optional(choice('.', '..')),
      $.identifier,
      repeat(seq('.', $.identifier)),
    ),

    // ---------------------------------------------------------------------
    // Macros / comptime / tests
    // ---------------------------------------------------------------------
    // macro name(params) -> Tokens =
    macro_definition: $ => prec.left(seq(
      'macro',
      field('name', $.identifier),
      optional($.parameters),
      optional($.return_annotation),
      optional('='),
    )),

    // template name(params) -> Tokens =
    template_definition: $ => prec.left(seq(
      'template',
      field('name', $.identifier),
      optional($.parameters),
      optional($.return_annotation),
      optional('='),
    )),

    // test "name": | test name:
    test_definition: $ => prec.left(seq(
      'test',
      choice($.string, $._expr),
      optional(':'),
    )),

    // suite Name:
    suite_definition: $ => prec.left(seq(
      'suite',
      field('name', $.identifier),
      optional(':'),
    )),

    setup_statement: $ => seq('setup', optional(':')),
    teardown_statement: $ => seq('teardown', optional(':')),

    // block NAME: | block NAME[ps]/[chk]:
    block_statement: $ => prec.left(seq(
      'block',
      field('name', $.identifier),
      optional(seq('[', repeat(choice($.identifier, $.magic_method, $.string, ',', ':')), ']')),
      optional(':'),
    )),

    // comptime: | comptime expr
    comptime_statement: $ => prec.left(seq('comptime', optional(choice(':', $._expr)))),

    // ---------------------------------------------------------------------
    // Control flow statements (structure is permissive; blocks not modeled)
    // ---------------------------------------------------------------------
    if_statement: $ => prec.left(seq('if', $._expr, optional(':'))),
    elif_statement: $ => prec.left(seq('elif', $._expr, optional(':'))),
    else_statement: $ => seq('else', optional(':')),
    for_statement: $ => prec.left(seq(
      'for', $._pattern, 'in', $._expr,
      optional(seq('if', $._expr)),
      optional(':'),
    )),
    while_statement: $ => prec.left(seq(
      'while',
      optional('let'),
      optional(seq($._pattern, optional(seq('=', $._expr)))),
      optional(seq('if', $._expr)),
      optional(':'),
    )),
    loop_statement: $ => seq('loop', optional(':')),
    // case pattern [if guard] : | =>
    case_statement: $ => prec.left(seq(
      'case', $._pattern,
      optional(seq('if', $._expr)),
      optional(choice(':', '=>')),
    )),
    // guard [let PATTERN =] expr [else [:]]
    guard_statement: $ => prec.left(seq(
      'guard',
      choice(seq('let', $._pattern, '=', $._expr), $._expr),
      optional(seq('else', optional(':'))),
    )),
    with_statement: $ => prec.left(seq('with', $._expr, optional(seq('as', $._pattern)), optional(':'))),
    defer_statement: $ => prec.left(seq('defer', optional(choice(':', $._expr)))),
    try_statement: $ => seq('try', optional(':')),
    catch_statement: $ => prec.left(seq('catch', optional(choice($._pattern, $.identifier, $.builtin_type)), optional(':'))),
    finally_statement: $ => seq('finally', optional(':')),
    raise_statement: $ => prec.left(seq('raise', optional($._expr))),
    raises_statement: $ => prec.left(seq('raises', optional($._expr))),
    return_statement: $ => prec.left(seq('return', optional($._expr))),
    break_statement: $ => prec.left(seq('break', optional(choice(
      $.identifier,
      seq('with', $._expr),
      seq($.identifier, $._expr),
    )))),
    continue_statement: $ => prec.left(seq('continue', optional(choice($.identifier, $._expr)))),
    yield_statement: $ => prec.left(seq('yield', optional(seq('from', $._expr)), optional($._expr))),
    pass_statement: $ => prec(2, 'pass'),
    assert_statement: $ => prec.left(seq('assert', optional($._expr))),
    check_statement: $ => prec.left(seq('check', optional($._expr))),
    // sum x in arr: | prod i in 1..n:
    declarative_for: $ => seq(
      choice('sum', 'prod'),
      $._pattern, 'in', $._expr,
      optional(':'),
    ),

    // ---------------------------------------------------------------------
    // Bindings / assignments
    // ---------------------------------------------------------------------
    // let x = e | let x: T = e | ref r = x | owned x = e | mut x = e
    let_statement: $ => seq(
      repeat1(choice('let', 'ref', 'mut', 'owned')),
      $._pattern,
      optional(seq(':', $._type)),
      optional(seq('=', $._expr)),
    ),

    // x = e | x += e | x =: body | x ^: k | f ~: body | f *: body | _ = e
    assignment_statement: $ => seq(
      $.assignment_target,
      choice('=', '=:', '^:', '~:', '*:', '+=', '-=', '*=', '/=', '%=', '**=', '&=', '|=', '^=', '<<=', '>>='),
      $._expr,
    ),

    assignment_target: $ => choice(
      $.identifier, $.magic_method, $._pattern, $.postfix_expression,
    ),

    // struct field / duck-constraint style line:  name: Type
    field_definition: $ => seq(
      field('name', choice($.identifier, $.magic_method)),
      ':', $._type,
    ),

    expression_statement: $ => $._expr,

    // ---------------------------------------------------------------------
    // Expressions (precedence per SYNTAX/12-操作符.md §二, 0 = loosest)
    // ---------------------------------------------------------------------
    _expr: $ => choice(
      $.closure,
      $.ternary_expression,          // 1:  a if cond else b
      $.walrus_expression,           // 1:  a := b  (right)
      $.range_expression,            // 1:  a..b / a..=b
      $.if_expression,               // block-style if used as expression
      $.match_expression,
      $.or_expression,               // 2
      $.and_expression,              // 3
      $.not_expression,              // 4  (right)
      $.comparison_expression,       // 5
      $.identity_expression,         // 6  is / in / as
      $.pipe_expression,             // 7  |>
      $.null_coalescing_expression,  // 8  ??
      $.bit_or_expression,           // 9
      $.bit_xor_expression,          // 10
      $.bit_and_expression,          // 11
      $.shift_expression,            // 12
      $.add_sub_expression,          // 13
      $.mul_div_expression,          // 14
      $.power_expression,            // 15 (right)
      $.unary_expression,            // 16 (right)
      $.postfix_expression,          // 17
      $.await_expression,
      $.spawn_expression,
      $.list_literal,
      $.dict_literal,
      $.tuple_literal,
      $.string,
      $.number,
      $.regex_literal,
      $.identifier,
      $.magic_method,
      $.builtin_type,
      $.builtin_type_value,
      $.builtin_constructor,
      $.builtin_function,
      'True', 'False',
      'pass',
      '...',
    ),

    // a if cond else b
    ternary_expression: $ => prec(1, seq(
      $._expr, 'if', $._expr, 'else', $._expr,
    )),

    // block-style if used in expression position:  let x = if c: a else: b
    if_expression: $ => prec(1, seq(
      'if', $._expr, optional(':'), optional($._expr),
      repeat(seq('elif', $._expr, optional(':'), optional($._expr))),
      optional(seq('else', optional(':'), optional($._expr))),
    )),

    match_expression: $ => prec(1, seq('match', $._expr, optional(':'))),

    walrus_expression: $ => prec.right(1, seq($._expr, ':=', $._expr)),
    range_expression: $ => prec.left(1, seq($._expr, choice('..', '..='), $._expr)),

    or_expression: $ => prec.left(2, seq($._expr, choice('or', '||'), $._expr)),
    and_expression: $ => prec.left(3, seq($._expr, choice('and', '&&'), $._expr)),
    not_expression: $ => prec.right(4, seq(choice('not', '!'), $._expr)),
    comparison_expression: $ => prec.left(5, seq($._expr, choice('==', '!=', '<', '>', '<=', '>='), $._expr)),
    identity_expression: $ => prec.left(6, seq($._expr, choice('is', 'in', 'as'), $._expr)),
    pipe_expression: $ => prec.left(7, seq($._expr, '|>', $._expr)),
    null_coalescing_expression: $ => prec.left(8, seq($._expr, '??', $._expr)),
    bit_or_expression: $ => prec.left(9, seq($._expr, '|', $._expr)),
    bit_xor_expression: $ => prec.left(10, seq($._expr, '^', $._expr)),
    bit_and_expression: $ => prec.left(11, seq($._expr, '&', $._expr)),
    shift_expression: $ => prec.left(12, seq($._expr, choice('<<', '>>'), $._expr)),
    add_sub_expression: $ => prec.left(13, seq($._expr, choice('+', '-'), $._expr)),
    mul_div_expression: $ => prec.left(14, seq($._expr, choice('*', '/', '%'), $._expr)),
    power_expression: $ => prec.right(15, seq($._expr, '**', $._expr)),
    // unary: + - ~ (bitwise not) * (deref) & (ref)
    unary_expression: $ => prec.right(16, seq(choice('+', '-', '~', '*', '&'), $._expr)),

    // postfix: call / index / member / ?. / ? / ^ / ~
    postfix_expression: $ => prec(17, choice(
      seq($._expr, '.', choice($.identifier, $.magic_method)),
      seq($._expr, '?.', choice($.identifier, $.magic_method)),
      seq($._expr, '(', optional($.call_args), ')'),
      seq($._expr, '[', optional($._expr), ']'),
      seq($._expr, '?'),
      seq($._expr, '^'),
      seq($._expr, '~'),
    )),

    await_expression: $ => prec(16, seq('await', $._expr)),
    spawn_expression: $ => prec(16, seq(choice('spawn', 'go'), $._expr)),

    // |x, y| body | | | 42 | |x| => body
    closure: $ => prec.left(seq(
      '|',
      optional(seq(
        choice($.identifier, $.magic_method),
        optional(seq(':', $._type)),
        repeat(seq(',', choice($.identifier, $.magic_method), optional(seq(':', $._type)))),
      )),
      '|',
      optional('=>'),
      optional($._expr),
    )),

    call_args: $ => seq(
      choice($._expr, seq($.identifier, choice('=', ':'), $._expr)),
      repeat(seq(',', choice($._expr, seq($.identifier, choice('=', ':'), $._expr)))),
      optional(','),
    ),

    // ---------------------------------------------------------------------
    // Patterns
    // ---------------------------------------------------------------------
    _pattern: $ => choice(
      $.identifier,
      $.builtin_constructor,
      $.variant_pattern,
      $.string,
      $.number,
      prec(2, 'True'), prec(2, 'False'),
      $.tuple_pattern,
      $.list_pattern,
      $.dict_pattern,
      $.range_pattern,
      $.ref_pattern,
      $.or_pattern,
      $.rest_pattern,
    ),

    // Some(v) | Ok(x, y) | Red | User(name)
    variant_pattern: $ => seq(
      choice($.identifier, $.builtin_constructor),
      '(', optional(seq($._pattern, repeat(seq(',', $._pattern)))), optional(','), ')',
    ),

    tuple_pattern: $ => seq(
      '(', optional(seq($._pattern, repeat(seq(',', $._pattern)))), optional(','), ')',
    ),

    // [a, b, ..rest] | [_, ..]
    list_pattern: $ => seq(
      '[', optional(seq(choice($._pattern, $.rest_pattern), repeat(seq(',', choice($._pattern, $.rest_pattern))))), optional(','), ']',
    ),

    // {"k": v, ...}
    dict_pattern: $ => seq(
      '{', optional(seq($._pattern, ':', $._pattern, repeat(seq(',', $._pattern, ':', $._pattern)))), '}',
    ),

    // 1..=5 | 1..5
    range_pattern: $ => prec(1, seq($._pattern, choice('..', '..='), $._pattern)),

    // ref x | ref mut x
    ref_pattern: $ => prec(2, seq('ref', optional('mut'), $._pattern)),

    // 0 | 1  (or-pattern)
    or_pattern: $ => prec(1, seq($._pattern, '|', $._pattern)),

    // .. | ..rest
    rest_pattern: $ => seq('..', optional($.identifier)),

    // ---------------------------------------------------------------------
    // Types
    // ---------------------------------------------------------------------
    _type: $ => choice(
      $.identifier,
      $.builtin_type,
      $.builtin_type_value,
      $.builtin_constructor,
      $.generic_type,
      $.unit_type,
      $.tuple_type,
      seq($._type, '?'),     // Option shorthand: int? = Option<int>
    ),

    generic_type: $ => seq(
      choice($.identifier, $.builtin_type, $.builtin_type_value),
      $.type_arguments,
    ),

    unit_type: $ => seq('(', ')'),
    tuple_type: $ => seq('(', $._type, repeat(seq(',', $._type)), optional(','), ')'),

    // <T, U: Trait, V = Default>
    type_parameters: $ => seq(
      '<',
      repeat(choice($.identifier, $.builtin_type, $.builtin_type_value, ',', ':', '+', '=', '.', 'Self', '?')),
      '>',
    ),

    // List<int> | Option<Self> | Dict<str, List<int>>
    type_arguments: $ => seq(
      '<',
      repeat(choice($._type, ',', '..', '?')),
      optional(','),
      '>',
    ),

    return_annotation: $ => seq('->', $._type),

    // where T: Trait1 + Trait2, U: Other
    where_clause: $ => seq(
      'where',
      repeat(choice($.identifier, $.builtin_type, $.builtin_type_value, ':', '+', ',', '.', 'Self')),
    ),

    // ---------------------------------------------------------------------
    // Parameters
    // ---------------------------------------------------------------------
    parameters: $ => seq(
      '(',
      optional(seq($.parameter, repeat(seq(',', $.parameter)))),
      optional(','),
      ')',
    ),

    parameter: $ => choice(
      seq(
        optional(choice('ref', 'mut', 'owned')),
        choice($.identifier, $.magic_method),
        optional(seq(':', $._type)),
        optional(seq('=', $._expr)),
      ),
      // variadic injection: .. | ..: Tuple<..> | ..: Dict<K,V>
      seq('..', optional(seq(':', $._type))),
      // positional/keyword boundary separators: / and *
      choice('/', '*'),
    ),

    // ---------------------------------------------------------------------
    // Literals
    // ---------------------------------------------------------------------
    string: $ => token(choice(
      seq(optional(choice('f', 'r')), '"', repeat(choice(seq('\\', /[\s\S]/), /[^"\\\n]/)), '"'),
      seq(optional(choice('f', 'r')), '"""', repeat(choice(seq('\\', /[\s\S]/), /[^"]/)), '"""'),
      seq(optional(choice('f', 'r')), '```', repeat(choice(seq('\\', /[\s\S]/), /[^`]/)), '```'),
    )),

    number: $ => choice($.float, $.integer),

    float: $ => token(choice(
      seq(/[0-9][0-9_]*/, '.', /[0-9]+/, optional(seq(/[eE]/, optional(choice('+', '-')), /[0-9]+/))),
      seq(/[0-9][0-9_]*/, /[eE]/, optional(choice('+', '-')), /[0-9]+/),
    )),

    // decimal / 0x hex / 0o octal / 0b binary, `_` separators allowed
    integer: $ => token(choice(
      seq(/0[xX]/, /[0-9a-fA-F_]+/),
      seq(/0[oO]/, /[0-7_]+/),
      seq(/0[bB]/, /[01_]+/),
      /[0-9][0-9_]*/,
    )),

    // duck `match /pattern/` constraint regex (whitespace-free)
    regex_literal: $ => token(/(?:\/(?:[^\/\\\s]|\\.)+\/)/),

    // [a, b, c] | {k: v, ...} | (a, b) | (expr)
    list_literal: $ => seq(
      '[', optional(seq($._expr, repeat(seq(',', $._expr)))), optional(','), ']',
    ),
    dict_literal: $ => seq(
      '{', optional(seq($._expr, optional(seq(':', $._expr)), repeat(seq(',', $._expr, optional(seq(':', $._expr)))))), optional(','), '}',
    ),
    tuple_literal: $ => seq(
      '(', optional(seq($._expr, repeat(seq(',', $._expr)))), optional(','), ')',
    ),

    // ---------------------------------------------------------------------
    // Identifiers & builtins
    // ---------------------------------------------------------------------
    // ---------------------------------------------------------------------
    // Identifiers & builtins
    // ---------------------------------------------------------------------
    // Canonical form per spec 00 §4.1: [a-zA-Z_][a-zA-Z0-9_]*. The spec also
    // allows any Unicode letter sequence; tree-sitter CLI regex engines vary
    // in \p{L} support, so the ASCII form is authoritative here. The TextMate
    // grammar (syntaxes/lz.tmLanguage.json) keeps the Unicode form
    // (Oniguruma supports \p{L}) for full coverage.
    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    // __name__ dunder-style magic method names (ASCII form, see above)
    magic_method: $ => /__[a-zA-Z0-9_]+__/,

    // builtin types (prelude, NOT keywords — spec 00 §1.12 / 99 §2.0)
    builtin_type: $ => token(choice(
      'int', 'f64', 'bool', 'str',
      'List', 'Dict', 'Set', 'Option', 'Result', 'Tuple',
      'Box', 'Rc', 'Arc', 'Cell', 'RefCell',
      'IOError', 'Tokens',
    )),

    // builtin type-like values (prelude)
    builtin_type_value: $ => token(choice('Never', 'Unit', 'Nil', 'Number')),

    // builtin constructors (prelude)
    builtin_constructor: $ => token(choice('None', 'Some', 'Ok', 'Err')),

    // builtin functions (prelude — spec 99 §2.6 + 08)
    builtin_function: $ => token(choice(
      'print', 'panic', 'len', 'contains', 'iter', 'enumerate', 'zip',
      'map', 'filter', 'collect', 'sort', 'reverse', 'clone', 'drop',
      'format', 'hash', 'callable', 'quote', 'merge_tokens',
      'sum', 'prod',
    )),

    // ---------------------------------------------------------------------
    // Comments
    // ---------------------------------------------------------------------
    line_comment: $ => token(seq('//', /[^\n]*/)),
    block_comment: $ => token(seq('/*', repeat(choice(/[^*]/, seq('*', /[^/]/))), '*/')),
  },
});
