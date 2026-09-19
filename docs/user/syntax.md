# Syntax and evaluation

[User guide](index.md) · [Values](values.md) · [Functions](functions.md)

## Evaluation order

Operators have equal precedence. An expression groups from right to left;
parentheses override grouping. Function arguments are evaluated right to left
before application. Statements execute in source order and a program returns
its final statement's value.

```pliq
2*3+4                    // => 14
10-3-2                   // => 9
(10-3)-2                 // => 5
a:1;a+2                  // => 3
```

Whitespace application passes a complete expression to a function:

```pliq
square:{x*x}
square 2+3               // => 25
square[2]+3              // => 7
```

## Names and literals

Names start with an ASCII letter and continue with ASCII letters or digits.
They are case-sensitive. `_` is an operator, not a character allowed in a name.
Adjacent `.name` performs literal symbol indexing: `d.name` means ``d[`name]``.
It does not introduce dotted variable names or namespaces. Both sides of the
dot must be adjacent; `d . name`, `d. name`, and `d .name` retain binary dot
application, which evaluates `name`.

| Form | Meaning |
| --- | --- |
| `42`, `-42` | Signed 64-bit integer |
| `1.0`, `.5`, `1e-3` | 64-bit binary float |
| `0n` | Null |
| `0W`, `-0W`, `0w`, `-0w` | Integer extrema conventions and floating infinities |
| `1 2 3` | Numeric vector, also called a strand |
| `(1;2.0)` | Explicit numeric list; elements promote to float |
| `()` | Empty general list |
| `,42` | One-element list |
| `"abc"`, `"a"` | Byte string, character atom |
| `` `USD ``, `` `USD`EUR `` | Symbol, symbol vector |

Booleans are produced by comparisons and casts. Use `1=1` and `1=0` when a
boolean type is needed; numeric type-suffix literals are not supported.
See [values](values.md) for type codes and string escapes.

## Minus and whitespace

A minus immediately followed by a digit or decimal point starts a signed
number at the beginning of an expression, after whitespace, or after an opening
delimiter, semicolon, or colon. Elsewhere it is an operator.

```pliq
1 -2 3                   // => 1 -2 3
1-2                      // => -1
-2+3                     // => 1
-(2+3)                   // => -5
```

Use parentheses when a negative scalar and a vector could be confused. Put a
space between dot-apply and a numeric path: `(1 2;3 4) . 1 0`.

## Delimiters and comments

- `(...)` groups an expression; semicolons inside it make a list.
- `([] name:column; ...)` constructs a [table](tables.md).
- `f[a;b]` passes explicit arguments; omitted arguments form projections.
- `a[i]` indexes a collection; multiple selectors descend through it.
- `a.name` indexes with the literal symbol `` `name ``; suffixes can be chained
  and mixed with brackets, as in `config.servers[0].host`.
- `{...}` defines a function; `{[a;b]...}` names its parameters explicitly.
- Semicolons and newlines separate statements. Newlines directly inside
  parentheses and argument brackets act as whitespace.
- Only `//` begins a comment, continuing to the end of the line. It needs no
  preceding whitespace and is literal text inside a quoted string.
- A single `/` is an iterator, including after whitespace: `+/x` and `+ /x`
  both reduce `x` with addition.

```pliq
// A comment on its own line.
a:1 2 3                   // A comment after a statement.
+/a                       // => 6
+ /a                      // => 6
```

## Assignment and operator values

`name:value` binds a name. `a[i]:value` updates a collection. Multiple selectors
can use successive brackets or one bracket, as in `a[i;j]:value`. Modified assignment
uses an operator before the colon: `a+:1`, `a[i]*:2`. `a,:value` appends through
aliases; see [mutation](mutation.md).

A primitive can be stored as a value. `op:` explicitly selects its unary
meaning. A binary primitive supplied with one bracket argument becomes a
projection waiting for the other argument.

```pliq
f:-:;f[2]                // => -2
addOne:+[1;];addOne[4]   // => 5
```

`::` is identity. Prefix `:expression` returns early from a function.
`$[condition;yes;no]` is lazy conditional syntax; ordinary function arguments
are eager. Read [functions and control flow](functions.md) for these forms.
