# Functions and iterators

[User guide](index.md) · [Syntax](syntax.md) · [Mutation](mutation.md)

## Definitions and calls

Braces define a function. Without an explicit parameter list, `x`, `y`, and `z`
are implicit parameters. Explicit lists can name parameters or declare zero
arguments. The function returns its final statement unless it returns early.

```pliq
square:{x*x};square[4]     // => 16
add:{[a;b]a+b};add[2;3]    // => 5
answer:{[]42};answer[]     // => 42
```

Use brackets for an explicit argument list, whitespace or `@` for one argument,
and `.` for a list of arguments. A vector is one argument unless expanded by
dot application. Too many arguments are an error.

```pliq
{x+y}.(2;3)               // => 5
{#x}[1 2 3]               // => 3
```

## Projections

Supplying fewer arguments to a user function creates a function waiting for
the remainder. Missing bracket arguments create explicit holes, filled from
left to right on later calls. Binary primitives also project when called with
one bracket argument; use `op:` for a unary primitive value.

```pliq
{x+y}[2][3]               // => 5
{x+y+z}[;2;][1;3]         // => 6
+[;2][3]                  // => 5
f:-:;f[2]                 // => -2
```

## Lexical captures and recursion

Functions capture referenced outer bindings at definition time. Rebinding an
outer name later does not replace its captured value. Assignment to a name
inside a call is local, while indexed mutation of a captured container remains
visible through aliases.

```pliq
a:10;f:{a+x};a:99
f[2]                       // => 12
make:{[a]{[b]a+b}}
addTen:make[10];addTen[5]  // => 15
```

A named function can call itself. Use a lazy conditional to stop recursion.
There is a function application depth guard; recursion is not an unbounded
loop facility.

```pliq
fact:{$[x<2;1;x*fact[x-1]]}
fact[5]                   // => 120
```

## Conditionals and return

`$[condition;yes;no]` evaluates the condition, then only the selected branch.
Numeric zero is false; nonzero numeric values and `0n` select
the true branch. Conditions must be scalar. Longer odd argument lists test
condition/result pairs in order and finish with a fallback.

```pliq
$[0;missing;42]           // => 42
$[0;2;1;4;5]              // => 4
{ :x;99}[4]               // => 4
```

Prefix `:` returns immediately from the current function. A nested function's
return does not return from its caller. `?[c;x;y]` is an eager vector choice;
both branch arguments are evaluated before it chooses elements.

## Iterators

Appending an iterator to a function creates a derived function. The input
elements are snapshotted before iteration; shared nested containers can still
observe mutation.

| Form | Operation |
| --- | --- |
| `f'` | Each; with a function argument, composition |
| `f/` | Reduction for binary functions; convergence/repetition for unary functions |
| `f\` | Scan, retaining intermediate values |
| `f/:` | Each right |
| `f\:` | Each left |
| `f':` | Each prior |

### Each and composition

Each applies a function to corresponding array elements, broadcasting scalar
arguments. Arrays used together must have equal lengths. Applying each to a
function value composes functions: `(f'g)[args]` calls `g`, then passes its
result to `f`.

```pliq
{x*x}'1 2 3               // => 1 4 9
{x+y}'[1 2;10]            // => 11 12
f:{x+1};g:{x*2};h:f'g
h[3]                      // => 7
```

### Reduce and scan

For a binary function, reduction combines elements from left to right. Without
a seed it starts at the first element. A seeded reduction starts by combining
the seed with the first element. Scan returns each accumulated result.

```pliq
-/1 2 3                   // => -4
+\1 2 3                   // => 1 3 6
10+/1 2 3                 // => 16
10+\1 2 3                 // => 11 13 16
```

An unseeded reduction or scan of the general empty list returns `()`. A seeded
reduction of an empty list returns the seed. A seeded scan of an empty list
has no intermediate elements and returns `()`.

```pliq
+/()                      // => ()
*/()                      // => ()
10+/()                    // => 10
10+\()                    // => ()
```

### Unary repetition, convergence, and while

With a unary function, a numeric left argument is a nonnegative repetition
count. A function on the left is a predicate tested before each iteration.
Scan includes the initial value for these unary forms.

```pliq
3{x+2}/1                  // => 7
3{x+2}\1                  // => 1 3 5 7
{x<5}{x+2}/1              // => 5
{x<5}{x+2}\1              // => 1 3 5
```

Without a count or predicate, unary reduction iterates until the next value
matches the current value or the initial value. It returns the current value
without adding the repeated value. Other cycles do not automatically stop;
choose an explicit bound when convergence is uncertain.

```pliq
{_x%2}/10                 // => 0
{_x%2}\10                 // => 10 5 2 1 0
```

### Each right and each left

Each right keeps the whole left argument for each element on the right. Each
left keeps the whole right argument for each element on the left. If the
iterated argument is a scalar, the function is applied once directly.

```pliq
10 20+/:1 2               // => (11 21;12 22)
10 20+\:1 2               // => (11 12;21 22)
```

### Each prior

Each prior calls a binary function with the current item on the left and the
previous item on the right. An explicit seed supplies the previous item for
the first call. Without a seed, `+`/`-` use zero, `*`/`%` use one, and other
functions use the first item.

```pliq
-':1 3 6                  // => 1 2 3
10-':1 3 6                // => -9 2 3
```
