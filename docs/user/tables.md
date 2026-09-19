# Tables

[User guide](index.md) · [Values](values.md) · [Mutation](mutation.md)

Tables are shared mutable collections of named homogeneous arrays. Columns may
have different types and lengths. A table's row count is the longest column's
length; missing cells in shorter columns read as `0n` without padding storage.

## Construction

Use `([] name:column; name:column)`. Column names are labels, not assignments.
Expressions evaluate right to left. Names must be unique and at least one column
is required. Every column must be an array: enlist scalars for a one-row table.
Byte strings are not column arrays; use symbol vectors or arrays of strings.

```pliq
t:([] name:`alice`bob; age:30 25 40)
#t                        // => 3
#t[`name]                 // => 2
t[`name;2]                // => 0n
!t                        // => `name`age
@t                        // => 98
```

Empty arrays retain their types. `([] age:(); salary:0#0.0)` has zero rows,
an integer column, and a float column. There is no scalar broadcasting.
Column labels can also be literal symbols, such as ``([] `price.usd:1.0 2.0)``,
or quoted UTF-8 text, such as ``([] "first name":`alice`bob)``.

## Screen output

Evaluating a table in the REPL or CLI displays its headers and rows. For
``([] name:`alice`bob; age:30 25 40)``, the output is:

```text
name  age
----- ---
alice  30
bob    25
0n     40
```

Symbols appear without backticks, numeric columns align right, and missing cells
print as `0n`. Empty tables show headers and a separator. Control characters in
names and symbols are escaped to keep each row on one line. Rendering does not
pad or mutate the stored columns. Nested values retain their compact syntax.

## Selection

Adjacent dot access selects columns with literal symbol names: `t.price` is
the same as ``t[`price]``. Brackets can follow it for cell access or updates,
such as `t.price[0]:42`.

| Expression | Result |
| --- | --- |
| ``t[`age]`` | Actual shared age array, with its physical length |
| ``t[`age;1]`` | Cell at index 1 in the age column |
| ``t[`age;2 0]`` | Age values at indices 2 and 0 |
| ``t[`name`age]`` | New table sharing the selected columns, in that order |
| `t[1]` | Independent one-row table |
| `t[2 0]` | Independent table with rows 2 and 0 |
| `t[()]` | Zero-row table retaining column types |
| `t[;1]` | Row 1 across all columns, as a table |

Paths are column-first. After selecting several columns, a subsequent selector
selects rows of that table. Negative, out-of-range, and null row indices yield
null-filled rows. Float indices and missing column names are errors. Repeated
column names and zero-column projections are errors. Row selections copy the
outer column containers; nested mutable elements can remain shared. A column
projection's row count is the maximum length of its selected columns.

```pliq
t:([] name:`alice`bob`cara; age:30 25 40)
s:t[&t[`age]>28]
s[`name]                  // => `alice`cara
+/s[`age]                 // => 70
```

## Shared mutation

Table aliases share column additions and replacements. Extracted columns and
columns supplied to the constructor are actual shared arrays. Appending through
an array alias changes only that array; other columns stay their original size.

```pliq
t:([] name:`alice`bob; age:30 25)
u:t
ages:t[`age]
ages[0]:31
u[`age;0]                 // => 31
ages,:40
#t                        // => 3
#t[`name]                 // => 2
t[`name;2]                // => 0n
```

Cell writes use existing array rules: indices must be within the physical
column length, and values must match its type or be null. A missing cell is
readable as null but cannot be assigned until the column has been extended.

Whole-column assignment adds a new column or replaces an existing one. Existing
columns must retain their element type; replacement may change length. This
replaces the column reference: previously extracted aliases still refer to the
old array. Reference cycles through tables, arrays, or closures are rejected.
Numeric row assignment is not supported; use a named column and row selector.

```pliq
t:([] age:30 25)
old:t[`age]
t[`age]:31 26 40
t[`senior]:t[`age]>35
old                       // => 30 25
t[`age;1]:27
t[`age]                   // => 31 27 40
```

## Append and join

`t,:otherTable` appends one or more logical rows. Names and types must match;
source columns can be in a different order. New rows start after the longest
destination column. Short columns in both tables contribute nulls, so a
successful nonempty append leaves all destination columns equally long.
Existing column aliases observe the append, including padding. The source is
unchanged unless its columns also alias destination columns. Self-append uses
the source values from before the append. A zero-row append validates the schema
but does not pad or change the destination.

```pliq
t:([] name:`alice`bob; age:30 25 40)
ages:t[`age]
t,:([] name:,`dan; age:,28)
t[`name;2]                // => 0n
t[`name;3]                // => `dan
ages                      // => 30 25 40 28
t,:([] age:35 22; name:`eve`frank)
#t                        // => 6
```

All replacements are prepared before mutation; a failed append leaves every
destination unchanged. If multiple destination columns are aliases of the same
array, their appended values must agree exactly, including nested identities;
conflicting updates are rejected. Columns shared with other tables remain shared,
so those tables also observe array changes and recompute their row counts.

`t,otherTable` returns a combined table with independent column containers.
`~` compares column names, order, physical lengths, and values: an implicit null
and a stored trailing null do not make physically different columns match.

## Scope

Use array operations on individual columns for calculations and filtering.
Keyed tables, heterogeneous row records, query keywords, and general table
arithmetic are not implemented. The Rust API can import/export Arrow record
batches, padding exported snapshots without changing the original columns.
Parquet file I/O is not yet implemented.
