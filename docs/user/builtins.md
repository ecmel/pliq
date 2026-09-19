# Casts and named functions

[User guide](index.md) · [Operators](operators.md) · [Numeric semantics](values.md#numbers)

## Casts

Binary `$` uses a character on the left to choose a conversion. Lowercase codes
convert values; uppercase codes parse text. This table is the implemented
catalog, including aliases that do not introduce additional storage types.

| Code | Input | Result |
| --- | --- | --- |
| `"b"` | Number | Boolean: false for zero, true otherwise |
| `"j"`, `"i"`, `"h"` | Number or character | `i64` integer; a character becomes its byte code |
| `"f"` | Number or character | `f64` float; a character becomes its byte code |
| `"e"` | Number | `f64` float |
| `"c"` | Integer or boolean in `0..255` | Character |
| `"s"` | Symbol | The same symbol |
| `"B"` | Text | Parsed boolean |
| `"J"`, `"I"`, `"H"` | Text | Parsed `i64` integer, or `0n` on invalid input |
| `"F"`, `"E"` | Text | Parsed `f64` float; invalid text is an error |
| `"S"` | Text | Symbol |
| Empty symbol (a single backtick) | Text | Symbol |

Lowercase casts recurse through arrays. Converting a string uses its character
elements. Casts preserve `0n`. Integer conversion of a float rounds ties to even
and uses Rust's saturating float-to-integer conversion for values
outside the integer range. `h`, `i`, and `e` are aliases here; they do not provide
narrow integer or 32-bit float storage.

```pliq
"f"$"a"                   // => 97
@"f"$"a"                  // => -9
"j"$2.5 3.5               // => 2 4
"c"$65 66                 // => "AB"
"J"$"bad"                 // => 0n
"J"$" 12 "                // => 12
"S"$"USD"                 // => `USD
```

Boolean text parsing accepts exactly `1`, `t`, `T`, `y`, or `Y` as true;
everything else, including `true` and `false`, is false. Integer and boolean
parsing remove surrounding spaces, tabs, and carriage returns, but not
newlines. Integer parsing also recognizes `0W`/`0w` and `-0W`/`-0w`; invalid,
empty, and out-of-range text returns `0n`.

Numeric codes `1`, `5`, `6`, `7`, `8`, `9`, and `10` select the corresponding
lowercase boolean, integer, float, or character cast when the right operand is
not text. With text on the right, an integer left operand is a width instead.
Negative type codes and casts outside this catalog are not implemented.

## String conversion and padding

Unary `$` converts values to display strings. It acts element by element on
arrays and preserves dictionary keys while converting values. Symbols convert
to their name text.

Binary `$` with an integer width and text truncates to the width in bytes and
pads with spaces. Positive widths align left; negative widths align right.

```pliq
$123                      // => "123"
$`USD                     // => "USD"
5$"abc"                   // => "abc  "
-5$"abc"                  // => "  abc"
3$"abcdef"                // => "abc"
```

Byte truncation can split a multi-byte UTF-8 character. Use these widths as
byte widths rather than terminal display-column widths.

## Matrix product

Binary `$` with numeric arrays computes a dot product for vectors and supports
matrix products for compatible nested arrays. Element arithmetic follows the
usual numeric promotion rules.

```pliq
1.0 2.0$3.0 4.0                     // => 11
(1.0 2.0;3.0 4.0)$(5.0 6.0;7.0 8.0) // => (19 22;43 50)
```

## Unary aliases

Each named function below takes one argument and uses the indicated unary
primitive. Names are ordinary bindings and can be rebound in an interpreter.

| Names | Primitive | Operation |
| --- | --- | --- |
| `neg` | `-` | Negate |
| `not` | `~` | Test numeric zero |
| `null` | `^` | Test null |
| `first` | `*` | First element |
| `count` | `#` | Count |
| `til`, `key` | `!` | Range, keys, or vector type name |
| `value` | `.` | Values or evaluation |
| `reverse` | `\|` | Reverse |
| `distinct` | `?` | Distinct elements |
| `where` | `&` | Expand counts |
| `group` | `=` | Group indices |
| `flip` | `+` | Transpose |
| `type` | `@` | Type code |
| `string` | `$` | String conversion |
| `floor` | `_` | Floor or lowercase |
| `reciprocal` | `%` | Reciprocal |
| `enlist` | `,` | One-element list |

`asc` and `desc` return array elements in sorted order. Their current
implementation returns non-array values, including strings and dictionaries,
unchanged. To obtain grade indices or dictionary keys use `<` or `>`.

## Aggregates and scans

| Reduction | Scan | Primitive |
| --- | --- | --- |
| `sum` | `sums` | `+` |
| `prd` | `prds` | `*` |
| `min` | `mins` | `&` |
| `max` | `maxs` | `\|` |

These functions take one input. The scan retains intermediate results.
Unseeded reductions of `()` return `()`. Use symbolic forms such as `10+/x`
when a seed is needed.

```pliq
sum 1 2 3                 // => 6
sums 1 2 3                // => 1 3 6
min 3 1 2                 // => 1
maxs 3 1 4                // => 3 3 4
```

## Additional numeric functions

| Function | Meaning |
| --- | --- |
| `round[value;places]` | Round floats to 0–308 decimal places, ties to even |
| `pow[base;exponent]` | Floating power, including fractional exponents |
| `mod[value;divisor]` | Euclidean remainder; zero divisor produces numeric null |

These functions recurse over arrays; paired arrays must have equal lengths.
`round` leaves integer and boolean values unchanged. `pow` always produces
floats. `mod` preserves integer arithmetic for integer/boolean pairs and
promotes to floating arithmetic when needed.

```pliq
round[1%3;5]              // => 0.33333
round[1.255;2]            // => 1.25
pow[2;0.5]                // => 1.4142135623730951
mod[-7;3]                 // => 2
mod[7;0]                  // => 0n
```

Rounding uses binary floating-point scaling, so decimal ties may already have
an approximation error.
