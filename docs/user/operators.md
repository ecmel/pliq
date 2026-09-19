# Operator reference

[User guide](index.md) · [Reference card](reference-card.md) · [Casts](builtins.md)

Unary means one argument (`op y`); binary means two (`x op y`). Symbols choose
behavior by argument count and value type. This reference describes pliq's
implemented forms. Numeric binary operations broadcast scalars and recurse through
equal-length arrays.

## `+`: transpose and add

Unary `+` transposes a homogeneous list of rows;
array rows must have matching lengths. Nonempty input must contain at least one
array or string row. Binary `+` adds numbers or aligns dictionary values by key.

```pliq
+(1 2;3 4)                // => (1 3;2 4)
+(1 2;3 3)                // => (1 3;2 3)
1 2+10                    // => 11 12
```

## `-`: negate and subtract

Unary `-` negates numbers recursively; integer negation wraps. Binary `-`
subtracts the right operand. A leading signed numeric strand is a literal,
so parenthesize a vector when negating all its elements.

```pliq
-(1 2 3)                  // => -1 -2 -3
10-1 2 3                  // => 9 8 7
```

## `*`: first and multiply

Unary `*` returns the first array element or dictionary value. Scalars return
themselves, `*()` returns `()`, and first of an empty string is a space.
Binary `*` multiplies numbers.

```pliq
*10 20 30                 // => 10
*()                       // => ()
2*3 4                     // => 6 8
```

## `%`: reciprocal and divide

Unary `%` computes `1%y`. Binary `%` divides and produces floating results,
including infinity for nonzero values divided by zero; undefined results become `0n`.

```pliq
%4                        // => 0.25
3%2                       // => 1.5
```

## `!`: range, keys, type name, and dictionary construction

Unary `!` on a nonnegative integer generates `0` through `n-1`; on a dictionary
it returns keys; on a supported homogeneous vector it returns the type name
symbol. Binary `!` constructs a dictionary from equal-length key and value
arrays. Internal negative-`!` runtime services are absent.

```pliq
!4                        // => 0 1 2 3
!1.0 2.0                  // => `float
!`a`b!10 20               // => `a`b
```

## `&`: where and minimum

Unary `&` expands nonnegative integer counts into repeated indices. On a
dictionary it repeats keys according to the values. Binary `&` selects the
smaller of comparable numeric, character, or symbol values.

```pliq
&0 2 1                    // => 1 1 2
&`a`b!1 2                 // => `a`b`b
1 4&2 3                   // => 1 3
```

## `|`: reverse and maximum

Unary `|` reverses arrays, strings, or dictionary entry order; scalars are
unchanged. Binary `|` selects the larger comparable value.

```pliq
|1 2 3                    // => 3 2 1
1 4|2 3                   // => 2 4
```

## `^`: null test and fill

Unary `^` identifies nulls while preserving structure. Binary `^` replaces
null right-hand elements with the left-hand value; dictionaries align by key.

```pliq
^1 0n 3                   // => 0 1 0
2^1 0n 3                  // => 1 2 3
`x^`a`                    // => `a`x
```

## `#`: count, take, and reshape

Unary `#` counts top-level elements or dictionary pairs; an atom counts as one.
Binary `#` with an integer takes that many elements, cycling as necessary.
A negative count takes from the end. A dimension list reshapes the input,
cycling its top-level elements; dimensions must be nonnegative integers.

```pliq
#1 2 3                    // => 3
5#1 2 3                   // => 1 2 3 1 2
-5#1 2 3                  // => 2 3 1 2 3
2 3#1 2 3 4               // => (1 2 3;4 1 2)
```

Taking a positive number from `()` produces that many empty lists. Taking from
`""` produces spaces. Dictionary take applies the count to keys and values.

## `_`: floor, lowercase, drop, cut, and delete

Unary `_` floors numbers and lowercases ASCII text or symbols. Binary `_`
has these forms:

- Integer left: drop elements from the front, or from the end for a negative
  count. A count beyond the length yields an empty result.
- Index-vector left and array/string right: cut at ordered, in-bounds start
  indices. Elements before the first index are omitted.
- Array left and integer right: delete one element at that index.
- Dictionary right with nonnumeric left: remove the specified keys. Numeric
  left operands retain the count-based drop meaning.

```pliq
_1.9 -1.9                 // => 1 -2
_"ABC"                    // => "abc"
2_1 2 3 4                 // => 3 4
0 2_"abcd"                // => ("ab";"cd")
10 20 30_1                // => 10 30
```

## `,`: enlist and join

Unary comma wraps a value in a one-element array. Binary comma joins top-level
items, merging dictionaries when both operands are dictionaries. Right-hand
dictionary entries replace overlapping keys. For mutation through aliases use
the distinct append-assignment form `a,:value`.

```pliq
,42                       // => ,(42)
1 2,3 4                   // => 1 2 3 4
"ab","cd"                 // => "abcd"
```

## `=`: group and equality

Unary `=` groups an array into a dictionary whose keys are distinct values and
whose values are vectors of original indices. Grouping dictionary values
returns the corresponding original keys. Binary `=` compares elements.

```pliq
(=1 2 1)[1]               // => 0 2
1 2 3=2                   // => 0 1 0
1=1.0                     // => 1
```

## `<` and `>`: grade and comparison

Unary grade returns indices in ascending (`<`) or descending (`>`) order;
dictionary grade returns keys ordered by their values. Sorting is stable.
Nulls sort before non-null values; nested arrays compare
lexicographically. Binary forms perform less-than and greater-than comparisons.

```pliq
<30 10 20                 // => 1 2 0
>30 10 20                 // => 0 2 1
<`a`b!2 1                 // => `b`a
1 2 3>2                   // => 0 0 1
```

Use `asc` and `desc` to sort arrays into values instead of obtaining indices.

## `~`: not and match

Unary `~` tests numeric zero recursively. Binary `~` compares whole values and
returns one boolean; numeric types, nested shapes, and dictionary order matter.
Function matching uses pliq syntax and captured values.

```pliq
~0 1 2                    // => 1 0 0
1 2~1 2                   // => 1
1~1.0                     // => 0
```

## `?`: distinct, find, sampling, and vector choice

Unary `?` retains each distinct array item in first-occurrence order.
Binary `x?y` finds the first index of each item of `y` in `x`; a missing item
returns the length of `x`.

With a numeric left argument, `n?y` samples with replacement for nonnegative
`n`, or deals without repeating population positions for negative `n`.
The population is `0` through `y-1` for a positive integer `y`, or the items of
an array. Deal cannot exceed the population. Random state is not exposed.

```pliq
?3 1 3 2                  // => 3 1 2
10 20 30?20 99            // => 1 3
#5?10                     // => 5
#?(-5?10)                 // => 5
```

Three-argument `?[c;x;y]` selects `x` for boolean true and `y` for false,
broadcasting scalar branches across an array condition. Arguments are eager;
use `$[...]` for lazy branches.

```pliq
?[0<1 -1 1;10 20 30;40 50 60] // => 10 50 30
```

## `@`: type, apply, index, amend, and trap

Unary `@` returns a [type code](values.md#type-queries). Binary `f@y` applies
one argument, or indexes when the left operand is a collection.

```pliq
@1                        // => -7
{x*x}@4                   // => 16
10 20 30@2 0              // => 30 10
```

`@[value;index;function]` and `@[value;index;function;right]` perform
[functional amend](mutation.md#functional-amend). With a function as the first
argument, `@[function;argument;handler]` [traps errors](troubleshooting.md#trapping-errors).

## `.`: values, evaluation, apply, deep index, amend, and trap

Unary `.` obtains dictionary values, evaluates source text, resolves a symbol
as a variable.
Direct text evaluation uses pliq's parser and current environment.

Binary dot passes a list of arguments to a function or descends through a
collection with a list of selectors.

```pliq
a:2;."a+3"                // => 5
a:42;.`a                  // => 42
{x+y}.(2;3)               // => 5
(1 2;3 4) . 1 0           // => 3
```

Three- and four-argument dot support deep amend. `.[function;arguments;handler]`
traps a call with an argument list. These forms are detailed in
[mutation](mutation.md) and [errors](troubleshooting.md).

## `$`: string conversion, casts, padding, matrix product, and conditional

Unary `$` converts values to strings, element by element for arrays. Binary `$`
casts according to a character or numeric type code, pads/truncates text with
an integer width, or multiplies numeric vectors/matrices. See the complete
[supported cast catalog](builtins.md#casts).

```pliq
$123                      // => "123"
"J"$"123"                 // => 123
5$"abc"                   // => "abc  "
1.0 2.0$3.0 4.0           // => 11
```

`$[c;yes;no]` is a lazy conditional; longer odd argument lists add branches.
See [control flow](functions.md#conditionals-and-return).

## `:` and `::`: assignment, return, and identity

`a:value` binds a name; indexed and modified assignment are described in
[mutation](mutation.md). Prefix `:value` returns early from the current function.
In an amend argument, `:` chooses the replacement on the right. `::` is identity:
with one argument it returns that argument, and with two it returns the right.
It also acts as an all-elements selector in supported indexing/amend forms.

```pliq
{ :x;99}[4]               // => 4
::[3]                     // => 3
@[1 2;::;:;9]             // => 9 9
```

## `'`, `/`, and `\`: iterators and error signal

Suffix `'`, `/`, `\`, `/:`, `\:`, and `':` derive functions. Their argument
forms are listed in [functions and iterators](functions.md#iterators).
Prefix apostrophe signals an error, for example `'"bad input"`.
