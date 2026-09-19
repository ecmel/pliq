# Reference card

[User guide](index.md) · [Operator details](operators.md) · [Casts and functions](builtins.md)

## Syntax

| Form | Meaning |
| --- | --- |
| `a:expression` | Bind a name |
| `1 2 3`, `(1;2.0)`, `()` | Numeric strand, promoted numeric list, empty integer vector |
| `,x`, `x,y` | Enlist, join |
| `a[i]`, `a[i;j]`, `a[;j]` | Index, nested index, all rows at a column |
| `{x+y}`, `{[a;b]a+b}`, `{[]42}` | Implicit, explicit, zero-argument functions |
| `f x`, `f[x;y]`, `f.(x;y)` | One-expression call, explicit call, argument-list call |
| `f[;y]`, `+[x;]` | Projections with missing arguments |
| `op:` | Unary primitive as a function value |
| `$[c;yes;no]` | Lazy conditional |
| `?[c;x;y]` | Eager vector conditional |
| `:value` | Return from a function |
| `::` | Identity |
| `a[i]:x`, `a[i]+:x`, `a,:x` | Replace, modify, append |
| `@[a;i;f;y]`, `.[a;path;f;y]` | Functional amend, deep amend |
| `@[f;x;handler]` | Trap a one-argument call |

Operators group right to left with equal precedence. Names start with ASCII
letters and contain letters/digits. Semicolons and newlines separate statements.
Only `//` starts a comment. A single `/` is an iterator, even after whitespace.
See [syntax](syntax.md).

## Symbols

| Symbol | Unary (`op y`)                                          | Binary (`x op y`)                                       |
| ------ | ------------------------------------------------------- | ------------------------------------------------------- |
| `+`    | Transpose a list of rows; scalar rows extend            | Add                                                     |
| `-`    | Negate                                                  | Subtract                                                |
| `*`    | First                                                   | Multiply                                                |
| `%`    | Reciprocal                                              | Divide                                                  |
| `!`    | Range; dictionary keys; typed vector type name          | Dictionary from key/value arrays                        |
| `&`    | Where: repeat indices or dictionary keys                | Minimum                                                 |
| `\|`   | Reverse                                                 | Maximum                                                 |
| `^`    | Null test                                               | Fill nulls; coalesce dictionaries                       |
| `#`    | Count                                                   | Take, cycling; reshape with a dimension list            |
| `_`    | Floor; lowercase text and symbols                       | Drop; cut at indices; delete an item or dictionary keys |
| `,`    | Enlist                                                  | Join; merge dictionaries                                |
| `=`    | Group: values mapped to their indices                   | Elementwise equality                                    |
| `<`    | Grade ascending                                         | Less than                                               |
| `>`    | Grade descending                                        | Greater than                                            |
| `~`    | Logical not                                             | Match                                                   |
| `?`    | Distinct                                                | Find; roll or deal with numeric left argument           |
| `@`    | Type                                                    | Apply one argument; index                               |
| `.`    | Dictionary values; evaluate text/list; resolve a symbol | Apply an argument list; index at depth                  |
| `$`    | String conversion                                       | Supported casts; text padding; matrix product           |
| `:`    | Return from a function                                  | Assign; right identity in functional amend              |

The [operator reference](operators.md) explains supported types, extra argument
forms, and error cases.

## Iterators

| Form | Meaning |
| --- | --- |
| `f'` | Each; function composition with a function argument |
| `f/` | Reduce; unary convergence, counted repetition, or predicate loop |
| `f\` | Scan; retain intermediate results |
| `f/:` | Each right |
| `f\:` | Each left |
| `f':` | Each prior |

See [functions and iterators](functions.md). Unseeded reduction of `()` returns
`()`. A seeded empty reduction returns its seed.

## Named functions

Unary aliases: `neg`, `not`, `null`, `first`, `count`, `til`, `key`, `value`,
`reverse`, `distinct`, `where`, `group`, `flip`, `type`, `string`, `floor`,
`reciprocal`, `enlist`.

Array sorts: `asc`, `desc`. Aggregates: `sum`, `prd`, `min`, `max`.
Scans: `sums`, `prds`, `mins`, `maxs`.

Numeric helpers: `round[value;places]`, `pow[base;exponent]`, `mod[value;divisor]`.
Read the [cast catalog](builtins.md#casts) for `$` conversion codes.

## Values

| Form | Meaning |
| --- | --- |
| `0n` | Null |
| `0W`, `-0W` | Integer maximum and its negation |
| `0w`, `-0w` | Floating infinities |
| `"abc"`, `"a"`, `,"a"` | String, character, one-byte string |
| `` `USD ``, `` `USD`EUR `` | Symbol, symbol vector |
| `keys!values` | Dictionary |

Type codes: boolean `1`, integer `7`, float `9`, character/string `10`, symbol
`11`, null or nested/runtime array `0`, dictionary `99`; other atom codes are negative.

## Commands

```sh
pliq -e '+/!100'
pliq examples/stats.pliq
pliq - < examples/stats.pliq
pliq --help
```

No arguments start the REPL when stdin is a terminal. REPL commands are
`\h`, `\c`, and `\q`. See [CLI and REPL](cli.md).
