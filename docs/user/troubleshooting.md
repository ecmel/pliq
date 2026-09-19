# Errors and troubleshooting

[User guide](index.md) · [CLI](cli.md)

## Common mistakes

| Symptom | Cause and next step |
| --- | --- |
| `undefined name` | Bind the name first. Names are case-sensitive; symbols require a backtick. |
| `length mismatch` | Paired arrays have different lengths. Use a scalar for broadcasting or reshape explicitly. |
| Index error for `1.0` | Indices require integer or boolean values. Use an integer literal or an explicit cast. |
| Error on a negative/null write index | Writes require valid positions even though corresponding reads return nulls. |
| `dictionary length mismatch` | Keys and values need equal lengths. Enlist both operands for a singleton dictionary. |
| A function prints instead of a numeric result | A missing argument created a projection. Use `op:` for a unary primitive value. |
| `unexpected character ... syntax is ASCII` | Variable syntax uses ASCII letters/digits; put arbitrary text inside a string. |
| `mutation would create a reference cycle` | A replacement reaches its destination through a container or captured function. Store acyclic data. |
| Decimal output has extra digits | Floats are binary approximations. Use `round` where explicit rounding is appropriate. |
| A REPL prompt changes to `..` | Complete the open delimiter/string, or use `\c`. |

The minus sign is spacing-sensitive. `1 -2` is a vector containing a negative
literal; `1-2` is subtraction. Both `+/x` and `+ /x` reduce; only `//` starts a
comment. Replace old single-slash comments with `//`. See [syntax](syntax.md).

## Trapping errors

`@[function;argument;handler]` calls a function with one argument. If the call
fails, a handler function receives the error text, or a non-function handler
is returned as a fallback. `.[function;arguments;handler]` passes an argument
list. Successful calls return their normal results.

```pliq
@[{x+1};2;99]                // => 3
@[{x+`a};2;99]               // => 99
@[{x+`a};2;{[error]`failed}] // => `failed
.[{x+y};(2;3);99]            // => 5
```

Prefix apostrophe raises an error. The error text is available to a trap:

```pliq
@[{[x]'"bad input"};0;{[error]error}] // => "bad input"
```

A return from a function is control flow, not an error to be swallowed by a
trap. Errors while evaluating the trap's own arguments occur before the
protected call and are not caught by it.

## Error recovery

The Rust API returns `Result<Value, String>`. The CLI prints errors to stderr
and fails in file/expression/stdin mode. The REPL prints an error and accepts
the next input, retaining its environment.

Parsing happens before execution, so a parse error prevents that source unit
from executing. Runtime errors can occur after earlier statements or argument
side effects. Indexed-update validation protects the update destinations, but
an evaluation is not a transaction. To discard all state, create a new
`Interpreter` or restart the process.

## Limits

The parser and evaluator guard deeply nested syntax and function application
at approximately 128 nested levels. The precise point depends on syntax and
internal calls, so this is not a guarantee of 128 user recursion steps.
There is no general execution timeout, array-size quota, or cancellation API.
In a `--listen` daemon these limits are shared: one oversized allocation can
exhaust memory for every attached session.
Unbounded unary iteration can continue indefinitely if it does not converge.

Keyed tables, query syntax, temporal values, and runtime file/handle operators
are not implemented. See [tables](tables.md) for supported table operations.
See the [operator reference](operators.md) for supported argument forms.

## Useful reports

For a reproducible issue, retain the smallest failing expression, its expected
and actual output, the executable/build version, and any required preceding
bindings. The [testing guide](../developer/testing.md) explains how to add a
regression test.
