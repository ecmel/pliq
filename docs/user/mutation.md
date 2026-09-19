# Mutation and amend

[User guide](index.md) · [Collections](values.md) · [Error recovery](troubleshooting.md#error-recovery)

## Shared identity and rebinding

Arrays, dictionaries, [tables](tables.md), and strings are shared mutable values. Copying a binding
shares the container. Indexed writes and appends are visible through aliases
and functions that captured the container. Rebinding changes only that name.

```pliq
a:10 20 30;b:a
a[0]:99;b                 // => 99 20 30
a:1 2;b                   // => 99 20 30
```

Nested collections share their child containers. Replacing a row and editing
an item inside that row are different operations.

```pliq
row:1 2;m:(row;3 4)
m[0][0]:9;row             // => 9 2
m[0]:7 8;row              // => 9 2
```

## Indexed and modified assignment

`a[i]:value` replaces an item. A vector of selectors pairs with a vector of
replacements or broadcasts a scalar. Modified assignment applies an operator
before replacement. Repeated selectors apply sequentially. Array updates must
match the existing element type; for example, assigning a float to an integer
vector is an error. Dictionary entries may be replaced with a different type;
arrays stored inside them still enforce their own element types.

```pliq
a:10 20 30
a[0 2]:7 8;a              // => 7 20 8
a[1]*:2;a                 // => 7 40 8
b:1 2;b[0 0]+:10 20;b     // => 31 2
```

Array and string write indices must be nonnegative and in bounds. Null and
floating indices are errors. Reads have different rules and can return nulls.
Dictionary assignment replaces the first matching key or appends a new key.

```pliq
d:`a`b!1 2
d[`b]:20;d[`c]:30
d[`a`b`c]                 // => 1 20 30
```

Top-level modified assignment such as `a+:1` rebinds `a` to a computed value.
Use indexed modification when aliases must observe the edit.

## Append

`a,:value` appends to an array or string through all aliases. Appending an array
adds its top-level elements; enlist a value to append it as one nested item.
Self-append snapshots the source first.

```pliq
a:1 2;b:a
a,:3 4;b                  // => 1 2 3 4
rows:,(5 6);rows,:,7 8;#rows // => 2
s:"ab";s,:s;s             // => "abab"
```

Ordinary `a:a,value` builds a new outer container and rebinds the name.

## String updates

String updates accept characters. Vector selectors pair with an equal-length
string or broadcast one character. Strings count bytes, not Unicode code points.
A one-byte quoted literal is a character; enlist it to get a mutable string.

```pliq
s:"abc";alias:s
s[0]:"X";alias            // => "Xbc"
s[1 2]:"YZ";alias         // => "XYZ"
s,:"!";alias              // => "XYZ!"
one:,"a";one[0]:"b";one   // => ,"b"
```

## Functional amend

Functional amend returns a copy of the data containers, then applies updates
to the copy. It preserves the original array, dictionary, or string.

| Form | Meaning |
| --- | --- |
| `@[value;index;f]` | Apply unary `f` to selected items |
| `@[value;index;f;right]` | Apply binary `f` with `right` |
| `.[value;path;f]` | Unary amend at a nested path |
| `.[value;path;f;right]` | Binary amend at a nested path |

Use `:` as `f` to replace rather than calculate. `::` as a selector chooses all
items. Deep paths are arrays of selectors; an empty path applies to the whole
value. For vector selectors, right-hand arrays pair by position and scalars
broadcast. Repeated selectors update the running result.

```pliq
a:1 2
b:@[a;0;+;10]
b                         // => 11 2
a                         // => 1 2
@[1 2;0 0;+;10 20]        // => 31 2
.[(1 2;3 4);1 0;:;9]      // => (1 2;9 4)
```

Amend with a symbol as the first argument updates the named variable and
returns its symbol. Its aliases observe indexed changes.

```pliq
a:1 2;b:a
@[`a;0;+;10]
b                         // => 11 2
```

If the first argument is a function, three-argument `@` and `.` instead act as
[error traps](troubleshooting.md#trapping-errors).

## Failed updates and cycles

pliq validates indexed replacements before committing them. Invalid indices,
replacement lengths, types, or reference cycles leave the destination
unchanged. A cycle includes indirect paths through dictionaries or closures,
not only placing an array directly inside itself.

This guarantee applies to the update's destinations. Evaluating a right-hand
expression, selector, or callback can already have performed side effects;
the interpreter does not roll back those effects or earlier statements.
Functional amend copies data containers, but function values keep their
captured environments.
