# Spreadsheet engine with formulas, recalculation and structural edits

Implement the stdlib-only `gridcalc` package. Everything is single-threaded
and deterministic. This file is the complete public contract.

```python
from gridcalc import Sheet, CellError, FormulaError
```

## Grid and names

Columns are `A`..`Z`, `AA`..`ZZ` (702 columns; `Z` is followed by `AA`, `AZ`
by `BA`); rows are `1`..`9999`. A cell name is column letters then the row in
decimal without leading zeros: `A1`, `AB120`. Names passed to `Sheet` methods
are `str`, case-insensitive, without `$`; anything else raises `ValueError`.
Names returned by `Sheet` are upper-case. *Row-major order* means sorted by
row, then by column.

## Values

A cell value is one of: empty (`None`), a number (`int` or `float`, never
`bool`), text (`str`), a boolean (`bool`), or an error (`CellError`).

`CellError(code)` has a `code` attribute, one of `"#DIV/0!"`, `"#VALUE!"`,
`"#REF!"`, `"#NAME?"`, `"#NUM!"`, `"#CYCLE!"`. Two `CellError`s are equal when
their codes are equal; a `CellError` never equals a `str`.

Every number stored by `set` or produced by an operator or function is
*normalised*: a `float` with an integral value becomes the equal `int`
(`4/2` is the `int` `2`).

`FormulaError` subclasses `ValueError`.

## Sheet

`Sheet(on_evaluate=None)`

A method that raises changes nothing.

### `set(name, raw)`

- `None` or `""`: the cell becomes empty.
- `bool`, `int`, `float`: that value (a `float` that is NaN or infinite raises `ValueError`).
- a `str` starting with `=`: a formula (see "Formulas"); a formula that does not parse raises `FormulaError`.
- any other `str`: text. Text is never parsed as a number: `"5"` is text.
- any other type raises `ValueError`.

### Reads

Reads never change anything and never evaluate a formula.

- `get(name)`: the cell's value; for a formula cell the value of its formula.
- `raw(name)`: `None` for an empty cell, the value for a literal cell, and for a formula cell its *formula text* (see "Formula text").
- `text(name)`: the *display text* of the value.
- `cells()`: names of the non-empty cells in row-major order.
- `precedents(name)`: names of every cell the cell's formula references (every cell of every range, each once, row-major); `[]` if the cell holds no formula.
- `dependents(name)`: names of the formula cells whose formula references `name` directly (alone or inside a range), row-major.

### Display text

Empty is `""`; text is itself; booleans are `"TRUE"` / `"FALSE"`; an error is
its code. An `int` is its decimal digits. A `float` is rounded to 6 decimal
places, half away from zero, applied to its shortest decimal representation
(Python `repr`); then trailing zeros and a trailing `.` are dropped, and a
result of negative zero is written `0`. Examples: `2.5` is `"2.5"`, `1/3` is
`"0.333333"`, `-0.0000001` is `"0"`, `2/3` is `"0.666667"`.

## Formulas

After the `=`, tokens may be separated by spaces (only the space character).

```
expr     := concat ( ("=" | "<>" | "<" | "<=" | ">" | ">=") concat )*
concat   := additive ( "&" additive )*
additive := term ( ("+" | "-") term )*
term     := unary ( ("*" | "/") unary )*
unary    := ("-" | "+") unary | power
power    := postfix ( "^" unary )?
postfix  := primary "%"*
primary  := NUMBER | STRING | "TRUE" | "FALSE" | "#REF!" | reference
          | NAME "(" [ expr ( "," expr )* ] ")" | "(" expr ")"
```

So every binary operator except `^` associates to the left, `2^3^2` is
`2^(3^2)`, `-2^2` is `-(2^2)`, and `2^-1` is valid.

- `NUMBER`: digits, optionally followed by `.` and digits (`12`, `1.50`). No sign, no exponent; `.5` and `5.` are not numbers.
- `STRING`: in double quotes; `""` inside a string is one quote character.
- `TRUE`, `FALSE`, `#REF!`, function names and references are case-insensitive.
- `NAME`: a letter or `_` followed by letters, digits or `_`. A name followed (after optional spaces) by `(` is always a function call, even if it looks like a cell name (`A1(2)` calls a function named `A1`).
- reference: a cell `B2`, or a range `B2:C5` (two cells joined by `:` without spaces). Column letters and row may each be prefixed with `$` (`$B$2`, `$B2`, `B$2`).
- Anything else (an unknown character, a name that is neither a call nor `TRUE`/`FALSE` nor a cell inside the grid such as `A0`, `AAA1` or `A10000`, an unterminated string, a missing operand, text after a complete expression, an empty formula) is a `FormulaError`.
- A known function called with a wrong number of arguments is a `FormulaError` (see the table under "Functions").

A range covers the rectangle spanned by its two corners. It is always written
top-left corner first: if the corners are out of order on an axis, the two
column parts (or the two row parts) swap places, each keeping its own `$`
marker. `B3:A1` is the range `A1:B3`; `$B3:A1` is `A1:$B3`.

### Formula text

`raw` returns `=` followed by the text as typed, except that every reference
is written in upper case, with its `$` markers, ranges top-left first, and
`#REF!` is upper case. Everything else (spaces, the case of function names,
number spelling, string contents) is kept exactly. Text inside a string is
never a reference.

`set("C1", "= sum( b3:$a1 ) & \"a1\"")` gives `raw("C1") == "= sum( $A1:B3 ) & \"a1\""`.

### Evaluation

The value of a reference to a cell is that cell's value. A range may appear
only directly as an argument of `SUM`, `COUNT`, `AVERAGE`, `MIN`, `MAX` and
`CONCAT`; anywhere else (an operand, an argument of another function, the
result of an `IF` branch, the result of the whole formula) a range is the error
`#VALUE!`. A reference in parentheses is still a reference. `#REF!` evaluates
to the error `#REF!`.

The value of a formula cell is never empty: an empty result becomes `0`.

**Coercion**

- to a number: a number is itself; `TRUE` is `1`, `FALSE` is `0`; empty is `0`; text is the error `#VALUE!`.
- to text: text is itself; other values become their display text (empty is `""`).
- to a boolean: a boolean is itself; a number is `TRUE` unless it is `0`; empty is `FALSE`; text is the error `#VALUE!`.

**Operators.** Operands are checked left to right; the first operand that is
an error, or that cannot be coerced, gives the result (`"a" + 1/0` is
`#VALUE!`; `1/0 + "a"` is `#DIV/0!`).

- `+ - * /`, unary `-` and `+`, and `^` coerce to numbers. `x%` is `x/100`. Division by zero is `#DIV/0!`.
- `^` is Python `**`, except: zero to a negative power is `#DIV/0!`; a result that is not a real number (`(-8)^0.5`) or that overflows is `#NUM!`.
- `&` coerces to text and concatenates.
- Comparisons give a boolean and never fail on types. Two empties are equal. Otherwise an empty operand first becomes `0`, `""` or `FALSE` to match the other operand's type. Values of different types order as: every number < every text < every boolean. Numbers compare numerically; text compares after `str.lower()` by code point (`"abc" = "ABC"`); `FALSE < TRUE`.

**Errors in functions.** Unless stated otherwise a function checks its
arguments left to right, and the first argument that is an error or cannot be
coerced gives the result.

## Functions

Function names are case-insensitive. An unknown function accepts any number of
arguments and evaluates to `#NAME?`.

| Function | Arguments | Result |
| --- | --- | --- |
| `SUM`, `AVERAGE`, `MIN`, `MAX` | 1 or more | aggregate of the collected numbers; `AVERAGE` of no numbers is `#DIV/0!`, `MIN`/`MAX` of no numbers is `0` |
| `COUNT` | 1 or more | how many numbers were collected; never an error |
| `CONCAT` | 1 or more | every value coerced to text and joined |
| `IF(cond, then[, else])` | 2 or 3 | `cond` coerced to a boolean selects a branch; a missing `else` is `FALSE` |
| `IFERROR(value, fallback)` | 2 | `value` unless it is an error, otherwise `fallback` |
| `AND`, `OR` | 1 or more | every argument coerced to a boolean |
| `NOT(x)` | 1 | boolean negation |
| `ABS(x)`, `INT(x)` | 1 | absolute value; floor (towards negative infinity) |
| `ROUND(x, digits)` | 2 | `x` rounded to `digits` decimal places half away from zero on its shortest decimal representation; `digits` may be negative (`ROUND(1250, -2)` is `1300`) and must be an integer, otherwise `#VALUE!` |
| `MOD(a, b)` | 2 | `a - b*floor(a/b)` (the sign of `b`); `b = 0` is `#DIV/0!` |
| `LEN(t)`, `UPPER(t)`, `LOWER(t)` | 1 | on `t` coerced to text |
| `LEFT(t, n)`, `RIGHT(t, n)` | 2 | first / last `n` characters of `t` coerced to text (all of it if `n` is larger); `n` must be an integer `>= 0`, otherwise `#VALUE!` |
| `ISBLANK(x)` | 1 | `TRUE` only when `x` is a reference to a single empty cell |
| `ISNUMBER(x)`, `ISTEXT(x)`, `ISERROR(x)` | 1 | type test on the value of `x` (booleans and empties are neither number nor text) |

- **Collecting** (`SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT`, `CONCAT`): arguments are taken in order and the cells of a range in row-major order.
  - For the five numeric ones, a *reference* argument (a cell or a range) contributes only the cells holding numbers: text, booleans and empty cells are skipped. Any other argument is coerced to a number (so `SUM(TRUE, 2)` is `3` and `SUM("a")` is `#VALUE!`), except in `COUNT`, where it counts only if it is a number.
  - The first error met, in a referenced cell or in another argument, is the result — except in `COUNT`, which ignores errors.
  - `CONCAT` coerces every value to text, referenced or not; the first error met is the result.
- `IF` evaluates only the chosen branch, and `IFERROR` evaluates `fallback` only when needed: an error in a branch that is not taken has no effect. An error in `cond` is the result of `IF`.
- `ISBLANK`, `ISNUMBER`, `ISTEXT` and `ISERROR` never return an error: `ISNUMBER(1/0)` is `FALSE`, `ISERROR(1/0)` is `TRUE`.

## Cycles

A formula *depends on* every cell it references, including references in
branches that are never evaluated (`=IF(FALSE, A1, 1)` in `A1` references
itself). A formula cell that is part of a dependency cycle, or that depends
directly or transitively on a cell that is, has the value `#CYCLE!` and is
not evaluated at all (so `IFERROR` cannot hide it). When a later mutation
removes the cycle, those cells get ordinary values again.

## Recalculation

Values are computed inside the mutating call. `on_evaluate(name)`, when given,
is called once for every evaluation of a formula cell, with the cell's name.

- `set(name, raw)` evaluates `name` itself if it now holds a formula, and every formula cell that depends on `name` directly or transitively — even when the new content equals the old.
- `copy(src, dst)` evaluates like `set` on `dst`.
- A structural edit evaluates every formula cell.

In each case every such cell is evaluated exactly once, after all the cells it
depends on that are evaluated in the same call, and no other cell is
evaluated. Cells whose value is `#CYCLE!` are never evaluated. `on_evaluate`
does not raise.

## Structural edits

- `insert_rows(before, count=1)`: rows `before` and below move down by `count`.
- `delete_rows(first, count=1)`: rows `first`..`first+count-1` are removed; rows below move up by `count`.
- `insert_cols(before, count=1)`, `delete_cols(first, count=1)`: the same for columns, named by letters (`"B"`, case-insensitive).

`before`/`first` must be a row (`int`, not `bool`) or a column inside the
grid, `count` an `int` (not `bool`) `>= 1`, and a deleted band must lie inside
the grid; otherwise `ValueError`.

Cells move with their row or column, and every reference in every formula is
rewritten to keep pointing at the same cells. `$` markers are kept and have no
effect on structural edits.

- Insert: a cell reference, or a range corner, at or after `before` moves by `count`. So a range grows when rows are inserted inside it, moves when they are inserted at its first row, and is unchanged when they are inserted just after its last row.
- Insert that would move a non-empty cell or any reference in a formula past row `9999` or column `ZZ` raises `ValueError`.
- Delete: a reference to a deleted cell becomes `#REF!` in the formula text. A range that lies entirely in the deleted band becomes `#REF!`; otherwise it shrinks to its surviving part (a corner inside the band moves to the nearest surviving row or column of the range). References beyond the band move by `count`.

## Copy

`copy(src, dst)` sets `dst` to the content of `src` (an empty `src` empties
`dst`). When `src` holds a formula, its references are shifted by the row and
column distance from `src` to `dst`; a part prefixed with `$` does not shift.
A cell reference shifted outside the grid becomes `#REF!`; a range with either
corner shifted outside the grid becomes `#REF!` as a whole. A shifted range is
then written top-left first by the rule above.

`A1` holding `=B1+$B1+B$1+$B$1`, copied to `C3`, gives `=D3+$B3+D$1+$B$1`.
