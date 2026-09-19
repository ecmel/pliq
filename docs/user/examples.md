# Example programs

[User guide](index.md) · [Getting started](getting-started.md)

Run an example from the repository root:

```sh
cargo run -- examples/stats.pliq
```

Each file starts a fresh interpreter and prints its final expression.

## Included programs

| File | Demonstrates |
| --- | --- |
| [demo.pliq](https://github.com/ecmel/pliq/blob/main/examples/demo.pliq) | Sales report combining shared tables, dictionary lookup, closures, each, filtering, grading, reductions, and scans |
| [tables.pliq](https://github.com/ecmel/pliq/blob/main/examples/tables.pliq) | Shared columns, implicit nulls, row append, and filtering |
| [arrays.pliq](https://github.com/ecmel/pliq/blob/main/examples/arrays.pliq) | Select even values, square them, and compute running totals |
| [stats.pliq](https://github.com/ecmel/pliq/blob/main/examples/stats.pliq) | Sum, mean, and population variance |
| [factorial.pliq](https://github.com/ecmel/pliq/blob/main/examples/factorial.pliq) | Recursion with a lazy conditional and each |
| [numbers.pliq](https://github.com/ecmel/pliq/blob/main/examples/numbers.pliq) | Floating arithmetic, large integers, and powers |
| [dictionaries.pliq](https://github.com/ecmel/pliq/blob/main/examples/dictionaries.pliq) | Align currency labels and total converted amounts |
| [symbols.pliq](https://github.com/ecmel/pliq/blob/main/examples/symbols.pliq) | Select values by symbolic labels |
| [mutation.pliq](https://github.com/ecmel/pliq/blob/main/examples/mutation.pliq) | Aliases, captured arrays, nested updates, and append |
| [strings.pliq](https://github.com/ecmel/pliq/blob/main/examples/strings.pliq) | Shared byte strings and captured bindings |

## Sales report demo

Run `cargo run -- examples/demo.pliq` for a combined language tour.
The script builds a table from existing arrays, updates it through an array
alias, and appends a sale. It uses illustrative currency rates in a dictionary,
vector arithmetic, and a closure to calculate revenue and percentages. Each
classifies sales with a lazy conditional. Filtering and grading select and sort
multi-unit sales, and a scan adds running totals. Missing notes display as null.
Percentages refer to all sales before filtering; running totals cover the report.

## Select, transform, and aggregate

`mod` tests parity. Comparing with zero builds a boolean vector; `&` converts
that vector to indices. `@` selects the values and each applies a function.

```pliq
numbers:!10
evens:numbers@&0=mod[numbers;2]
evens                     // => 0 2 4 6 8
squares:{x*x}'evens
squares                   // => 0 4 16 36 64
+/squares                 // => 120
```

## Mean and population variance

Compute deviations from the mean, square them, then divide their total by
the number of values. This example assumes nonempty numeric input.

```pliq
data:1 2 3 4 5
total:+/data
mean:total%#data
deviation:data-mean
variance:(+/deviation*deviation)%#data
(total;mean;variance)     // => 15 3 2
```

## Labels as keys

A dictionary lookup can select rates in the same order as another
dictionary's keys. Dot obtains values and `!` obtains keys.

```pliq
rates:`USD`EUR`TRY!1 1.08 0.029
amounts:`USD`EUR`TRY!100 200 300
round[+/(.amounts)*rates[!amounts];2] // => 324.7
```

## A closure over shared state

A closure captures the array binding. Its indexed updates remain visible to
the caller. Rebinding the caller's variable later would leave the closure
holding the originally captured array.

```pliq
state:,0
increment:{[]state[0]+:1;state[0]}
increment[]               // => 1
increment[]               // => 2
state[0]                  // => 2
```

For more input/output pairs, see the [operator reference](operators.md).
