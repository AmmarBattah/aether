# Minilang interpreter

Implement the stdlib-only `minilang` package. `run(source: str) -> list[str]`
parses and executes a whole program and returns the lines written by `print`,
in order. Calls are independent: no state survives between them.

Every error is a `MiniError` subclass: `MiniSyntaxError`, `MiniNameError`,
`MiniTypeError`, or `MiniRuntimeError`. An error has `line` and `column`
(1-based; tabs count as one column) of the offending token's first character,
as specified below, and `output`: the list of lines printed before the error
(empty for syntax errors). No other exception type may escape `run`; in
particular, user recursion within the limit below must not hit Python's
`RecursionError`. An implementation may raise Python's recursion limit but
must restore the previous value before `run` returns or raises.

## Lexical structure

- Whitespace (space, tab, newline) separates tokens and is otherwise ignored.
  `#` starts a comment that runs to the end of the line.
- Identifiers are ASCII `[A-Za-z_][A-Za-z0-9_]*`. Keywords cannot be used as
  identifiers: `let fn if elif else while break continue return true false nil
  and or not`.
- Integer literals are ASCII digits without leading zeros (`0` is valid, `007`
  is a syntax error at the literal); they may be arbitrarily large.
- String literals are double-quoted and end on the same line. Escapes are
  exactly `\n`, `\t`, `\"`, and `\\`; any other escape is a syntax error at
  the backslash. An unterminated string is a syntax error at its opening quote.
- Operators and punctuation: `== != <= >= ** < > + - * / % = ( ) [ ] { } , ;`.
  Any other character is a syntax error at that character.

## Grammar

```text
program    := statement*
statement  := "let" IDENT "=" expr ";"
            | target "=" expr ";"                 # target is IDENT or an index expression
            | "fn" IDENT "(" params ")" block
            | "if" expr block ( "elif" expr block )* [ "else" block ]
            | "while" expr block
            | "break" ";" | "continue" ";" | "return" [ expr ] ";"
            | expr ";"
block      := "{" statement* "}"
params     := [ IDENT ( "," IDENT )* ]
args       := [ expr ( "," expr )* ]
```

Expression precedence, lowest first:

1. `or` (left associative)
2. `and` (left associative)
3. prefix `not`
4. comparisons `== != < <= > >=`: **non-associative**; `a < b < c` is a
   syntax error at the second comparison operator
5. `+ -` (left associative)
6. `* / %` (left associative)
7. prefix `-`
8. `**`: right associative and binds tighter than a prefix `-` on its left,
   so `-2 ** 2` is `-4` and `2 ** 3 ** 2` is `512`; its right operand may
   itself start with prefix `-` (`2 ** -1` parses)
9. postfix call `f(args)` and index `x[i]`
10. primary: integer, string, `true`, `false`, `nil`, identifier,
    `( expr )`, list `[ args ]`, or anonymous function `fn ( params ) block`

A statement beginning with `fn IDENT` is a declaration; `fn (` begins an
anonymous function expression. Assigning to anything other than an
identifier or index expression is a syntax error at the `=`. A duplicate
parameter name is a syntax error at the duplicate. `return` outside a function
body, and `break`/`continue` outside a `while` body of the same function, are
syntax errors at the keyword. Syntax errors are reported before anything runs;
an unexpected token is a syntax error at that token. End of input is positioned
just past the last character (after a trailing newline: column 1 of the next line).

## Values and operators

Values are integers, strings, booleans, `nil`, lists (mutable, shared by
reference), and functions. Booleans are **not** integers.

- Truthiness: only `false` and `nil` are falsy; `0`, `""`, and `[]` are truthy.
- `and`/`or` short-circuit and return the deciding operand itself; `not`
  returns a boolean.
- `+` adds two integers, concatenates two strings, or concatenates two lists
  into a new list. `- * / % **` and prefix `-` require integers. `/` divides
  truncating toward zero and `%` takes the sign of the dividend, so
  `a == (a / b) * b + a % b`. Dividing or taking a remainder by zero is a
  runtime error, as is a negative exponent.
- `==`/`!=`: values of different types are never equal (`1 == true`,
  `0 == false`, and `nil == false` are all false). Lists are equal when they
  are the same list or have equal length and pairwise equal elements. A
  function equals only itself.
- `< <= > >=` compare two integers or two strings (code point order).
- `x[i]` indexes a list or a string (yielding a one-character string). `i`
  must be an integer and `0 <= i < len(x)`; negative indexes are out of
  range. `x[i] = v;` assigns into lists only (strings are immutable).

Operator type errors are `MiniTypeError` and arithmetic failures are
`MiniRuntimeError`, both at the operator token. Index errors are at the `[`:
`MiniTypeError` for a non-indexable value, a non-integer index, or assigning
into a string; `MiniRuntimeError` for an out-of-range index. Operands are
evaluated left to right before the operator is checked. An index assignment
evaluates the target, then the index, then the value; a name assignment
evaluates the value first.

## Scopes and functions

- The program, every block, and every function call has its own scope; each
  `while` iteration gets a fresh body scope. Builtins live in an outer scope
  that the program can shadow.
- `let` evaluates its initializer, then declares the name in the current
  scope. Declaring a name that already exists **in the same scope** is a
  `MiniNameError` at the name; shadowing an outer name is allowed. Reading or
  assigning an undeclared name is a `MiniNameError` at the identifier.
- `fn NAME(...) {...}` declares `NAME` in the current scope like `let` (so
  functions can recurse). Functions are closures over their defining scope by
  reference: they observe later assignments to captured variables, and
  assignments inside them update the captured variable.
- A call evaluates the callee, then the arguments left to right. Parameters
  are declared in the call's scope, which is also the body's top-level scope.
  Calling a non-function or passing the wrong number of arguments is a
  `MiniTypeError` at the call's `(`. A function without `return`, or with a
  bare `return;`, returns `nil`.
- At most 100 user-function calls may be active at once. The call that would
  start the 101st raises `MiniRuntimeError` at its `(`.

## Builtins and display

- `print(...)`: any number of arguments; writes one line with each argument's
  display form joined by single spaces; returns `nil`.
- `len(x)`: length of a string or list. `str(x)`: display form of `x`.
- `push(list, value)`: appends to the list in place; returns `nil`.
- Builtin argument errors (wrong count, wrong type) are `MiniTypeError` at the
  call's `(`.

Display forms: integers in decimal; strings unchanged; `true`, `false`, `nil`;
lists as `[` elements joined by `, ` `]`, where a string element is shown
double-quoted with `\\`, `\"`, `\n`, and `\t` escaped; a list that contains
itself (directly or indirectly) shows the inner occurrence as `[...]`;
functions as `<fn NAME>` or `<fn>` when anonymous; builtins as
`<builtin NAME>`.
