# Values and collections

[User guide](index.md) · [Operators](operators.md) · [Mutation](mutation.md)

## Numbers

pliq stores integers as Rust `i64`, floats as IEEE 754 `f64`, and booleans as
`bool`. Integer literals outside the signed 64-bit range are errors. Integer
addition, subtraction, multiplication, and negation wrap at 64 bits. Division
returns a float. A float operand promotes numeric arithmetic to floats.

```pliq
9007199254740993+1        // => 9007199254740994
9223372036854775806*2     // => -4
3%2                       // => 1.5
0.1+0.2                   // => 0.30000000000000004
```

Float results retain binary precision. `round` changes a value explicitly;
there is no global decimal-place setting. Very small values can underflow and
floating overflow can produce infinity. Integer-to-float promotion may lose
integer precision above the exact `f64` integer range.

Numeric `=`, `<`, and `>` use relative float comparison tolerance; integers
compare exactly when both operands are integers/booleans. `~` also requires
matching numeric types. See [casts and numeric helpers](builtins.md).

## Type queries

Unary `@` returns a negative code for an atom and a positive code for a
homogeneous vector. A nested or runtime-object array has code `0`; a table has
code `98` and a dictionary has code `99`.

| Type | Atom | Vector |
| --- | --- | --- |
| Boolean | `-1` | `1` |
| Integer | `-7` | `7` |
| Float | `-9` | `9` |
| Character / byte string | `-10` | `10` |
| Symbol | `-11` | `11` |
| Null | `0` | `0` |

```pliq
@1                       // => -7
@1.0                     // => -9
@(1=1)                   // => -1
@1 2.0                   // => 9
@(1;2.0)                 // => 9
1~1.0                    // => 0
```

Numeric strands and explicit numeric lists promote all elements to a common
type: boolean, then integer, then float. `()` creates an empty integer vector
with type `7`. Slicing and reversing retain a vector's type even when empty;
the empty string keeps type `10`.

## Nulls and infinities

| Value | Meaning |
| --- | --- |
| `0n` | The single null value; explicit absence, with validity bits in Arrow vectors |
| `0W`, `-0W` | `i64::MAX` and its negation |
| `0w`, `-0w` | Positive and negative floating infinity |

Null is compatible with every array element type. Non-null elements determine
the type: `1 0n 3` is an integer vector, and `1.0 0n 3.0` is a float vector.
All-null arrays have type `0`; a cast such as `"f"$0n 0n` supplies a type.
Indexed updates preserve the destination type, even when every element becomes
null. Extracting a null always returns the same scalar `0n`.

Arithmetic propagates null. Undefined floating results, such as `0%0`, become
`0n`; there is no NaN literal or separate language NaN value. Nonzero floating
division by zero produces infinity. Unary `^` tests for null; binary `^` fills
nulls on the right with values from the left. Spaces, empty strings, empty
symbols, and `-9223372036854775808` are ordinary values, not nulls.
The former `0N` spelling is rejected.

```pliq
1%0                      // => 0w
0%0                      // => 0n
0n+1                     // => 0n
^1 0n 3                  // => 0 1 0
2^1 0n 3                 // => 1 2 3
```

`0W` is still an integer value in arithmetic, with wrapping behavior. It is not
the same representation as floating infinity `0w`.

## Arrays

Ordinary arrays have one element type: numbers, strings, functions, dictionaries, or
other arrays. Nested arrays must have matching child types; their lengths may
differ. Ordinary mixed array literals such as ``(1;`a)`` or `(1;1 2)` are errors.
Use parentheses and semicolons for nested structures, and unary comma to enlist
one item. `(42)` is a grouped scalar; `,42` is a one-element array.

```pliq
#42                      // => 1
#,42                     // => 1
@,42                     // => 7
(1 2;3 4)+10             // => (11 12;13 14)
```

Arithmetic and comparisons recursively pair equal-length arrays and broadcast
scalars. Length mismatches are errors. Operations such as take, reverse, and
join create independent outer containers; nested mutable values may remain
shared. Indexed assignment and append must preserve the element type. Rebinding
can change a variable to a new type.

## Strings and characters

Quoted literals contain bytes. A one-byte literal is a character atom; a longer
literal or `""` is a string. Enlist a character to create a one-byte string.
Indexing and length count bytes, including for UTF-8 text.

| Escape | Byte |
| --- | --- |
| `\n` | Newline |
| `\r` | Carriage return |
| `\t` | Tab |
| `\"` | Double quote |
| `\\` | Backslash |
| `\xHH` | Two hexadecimal digits specifying any byte |

```pliq
@"a"                     // => -10
@,"a"                    // => 10
#"abc"                   // => 3
"abc"[2 0]               // => "ca"
3#""                     // => "   "
```

Strings share indexed writes and appends through aliases. Reverse,
concatenation, and vector selection create independent strings. Text-to-symbol
and text parsing operations require valid UTF-8 text, although byte storage and
escaped display can preserve arbitrary bytes.

## Symbols

A backtick introduces a symbol. Literal symbol names contain ASCII letters,
digits, and dots. Symbols are case-sensitive values, not variable references;
adjacent symbols form a vector. Empty symbols are allowed. `` `$text `` converts
text into a symbol and can represent names beyond the literal character set.

```pliq
`USD=`EUR                 // => 0
_`USD`EUR                 // => `usd`eur
`$"USD"                   // => `USD
```

Symbol comparisons and grading use case-sensitive lexical order. Resolving a
symbol as a variable is an explicit operation: ``a:42; .`a`` returns `42`.

## Dictionaries

`keys!values` constructs an ordered dictionary. Both operands must be arrays
of equal length; enlist both sides for a singleton dictionary. Keys can be
symbols, numbers, or compound values. Keys are copied recursively at insertion
so later mutation cannot change their lookup identity.

```pliq
prices:`USD`EUR!1 1.08
prices[`EUR]              // => 1.08
!prices                   // => `USD`EUR
.prices                   // => 1 1.08
d:(,`items)!,10 20
d[`items]                 // => 10 20
```

Adjacent dot access is shorthand for symbol indexing: `prices.EUR` means
``prices[`EUR]``. It works with any value supporting symbol indexing, including
table columns, and supports assignment and modified assignment. Names after
the dot use the variable-name rules (ASCII letter, then letters or digits).
Dynamic keys and keys containing dots or other characters use brackets:
``d[`price.usd]`` selects one key, while `d.price.usd` selects a nested path.

Dictionary values may have different types. An empty dictionary acquires its
key type on the first insertion; later keys must match it. Each entry can hold
any value, and replacement may change that entry's type:

```pliq
d:()!()
d.name                    // => 0n
d.name:"alice"
d.count:0
d.count+:1
d.name                    // => "alice"
d.count                   // => 1
d.count:`unknown          // replacement may change type
```

A parenthesized value list directly supplied to binary `!` also permits mixed
entries: ``d:`name`count!("alice";0)`` or ``![`name`count;("alice";0)]``.
Its entries evaluate right to left and preserve their individual types.
Ordinary array literals remain homogeneous, so `("alice";0)` by itself is an
error. To pass mixed values through a function, use an existing dictionary's
value list, such as `.d`.

Nested paths require existing intermediate containers; they are not created
automatically. For example, initialize `config:()!()` and
`config.database:()!()` before assigning `config.database.port:5432`.
Dot updates preserve the same aliasing and failure behavior as bracket updates.

Duplicate keys remain in construction order; lookup and replacement select the
first match. New-key assignment appends a pair. A dictionary with compound keys
can look up one compound key or a list of keys:

```pliq
d:(1 2;3 4)!10 20
d[1 2]                    // => 10
d[(3 4;1 2)]              // => 20 10
```

`!d` returns keys; `.d` returns values. Their outer arrays are independent of
the dictionary, while nested values in `.d` can remain shared. Mixed value
lists and multi-key lookups have type code `0`; homogeneous selections retain
typed arrays. Mixed value lists allow different element types, including when
updating their independent outer storage. Numeric fields of different types
are not promoted merely by insertion or replacement. Joining
dictionaries replaces overlapping values with those from the right and appends
new keys. Numeric dictionary operations align shared keys and retain unmatched
entries. [Tables](tables.md) hold named columns of potentially different types
and lengths. Dictionary operations still reject invalid operand combinations;
for example, adding a number to a string field is an error.

In the CLI and REPL, dictionaries display aligned key/value rows. For
`` `alice`bob!30 25 ``, the output is:

```text
alice | 30
bob   | 25
```

Rows retain insertion order, including duplicate keys. Symbols display without
backticks (an empty symbol retains its backtick); nested values use compact pliq
syntax. Control characters are escaped to keep each entry on one line. An empty
dictionary displays as `(()!())`. Rendering does not modify keys or values.

## Indexing

Indices start at zero and must be integers or booleans. Float indices are
errors even if they have integral values. A vector of indices selects a vector.
Negative, out-of-range, and null read indices produce missing values;
negative indices do not count backward from the end.

```pliq
(10 20 30)[2 0]           // => 30 10
(10 20)[-1]               // => 0n
(10 20)[0n]               // => 0n
(1.0 2.0)[0n]             // => 0n
"abc"[0n]                 // => 0n
```

Missing dictionary keys also produce `0n`, regardless of the value type.
A string selection containing a missing character returns a nullable character
array instead of a byte string. Byte strings themselves cannot store nulls.

Multiple selectors descend into nested values. An omitted selector selects all
elements at that level. Dot application accepts a path as a list.

```pliq
(1 2;3 4)[1;0]            // => 3
(1 2;3 4)[;1]             // => 2 4
(1 2;3 4) . 1 0           // => 3
```

Writes have stricter index checks; see [mutation](mutation.md).

## Display

Tables print as aligned rows and column headers in the CLI and REPL, with `0n`
for missing cells. Dictionaries display aligned key/value rows. Other values
and Rust `Value::Display` print pliq syntax: vectors with
spaces, nested lists with semicolons, symbols with backticks, and escaped
strings. Single-item containers use enlistment so they stay distinct from atoms.
Floats can print without a decimal point when integral; use `@` to inspect their
type. Booleans print as `0` or `1`.

Display is intended for reading results. It does not preserve every numeric
type on re-parsing and does not serialize functions.
