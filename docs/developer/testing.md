# Testing and verification

[Developer guide](index.md)

## Standard checks

```sh
cargo test
cargo test --release
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The ordinary suites need no external runtimes or services. Run debug
and release tests for arithmetic/evaluator changes, because optimization and
overflow settings can expose different defects. Tests use Rust's built-in
framework; keep reference calculations and regression tests in Rust.

## Suite map

| Suite                                                                                                      | Coverage                                                                               |
| ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| [language.rs](https://github.com/ecmel/pliq/blob/main/tests/language.rs)                                   | Evaluation order, functions, closures, projections, iterators, limits                  |
| [arrow.rs](https://github.com/ecmel/pliq/blob/main/tests/arrow.rs) | Arrow buffers, null conversion, type errors, snapshots, kernel boundaries |
| [operators.rs](https://github.com/ecmel/pliq/blob/main/tests/operators.rs) | Operator semantics and amend regressions |
| [numbers.rs](https://github.com/ecmel/pliq/blob/main/tests/numbers.rs)                                     | Numeric types, nulls, promotion, storage, precision                                    |
| [strings.rs](https://github.com/ecmel/pliq/blob/main/tests/strings.rs)                                     | Byte strings, escapes, aliases, invalid updates                                        |
| [symbols.rs](https://github.com/ecmel/pliq/blob/main/tests/symbols.rs)                                     | Symbol values, display, ordering, invalid syntax                                       |
| [dictionaries.rs](https://github.com/ecmel/pliq/blob/main/tests/dictionaries.rs)                           | Construction, lookup, keys, display, Rust API                                          |
| [mutation.rs](https://github.com/ecmel/pliq/blob/main/tests/mutation.rs)                                   | Shared updates, nested containers, cycle rejection, failure preservation               |
| [console.rs](https://github.com/ecmel/pliq/blob/main/tests/console.rs)                                     | Console-size views: cut lines, row and entry counts, unlimited display                 |
| [cli.rs](https://github.com/ecmel/pliq/blob/main/tests/cli.rs)                                             | Command arguments, stdin/files, output, exit status                                    |
| [support/number_reference.rs](https://github.com/ecmel/pliq/blob/main/tests/support/number_reference.rs)   | Independent arithmetic reference and signed boundaries, included by numeric unit tests |
| [support/parser_robustness.rs](https://github.com/ecmel/pliq/blob/main/tests/support/parser_robustness.rs) | Mutated parser inputs, included by parser unit tests                                   |
| [support/repl.rs](https://github.com/ecmel/pliq/blob/main/tests/support/repl.rs)                           | Scripted REPL behavior and console-size commands, included by CLI unit tests           |
| [support/net.rs](https://github.com/ecmel/pliq/blob/main/tests/support/net.rs)                             | Frame codec, socket serving, shared sessions, attached clients, included by transport unit tests |

## Add a regression

For a language defect, add the expression and expected pliq display result to
the relevant regression suite. Confirm the failure before changing behavior.
Use a separate `@` type query when the result's display hides a type difference.

For mutations, also verify aliases, captured values, nested destinations, and
failure preservation where relevant. For arithmetic, extend the independent
reference cases and signed boundaries. Random sampling tests should assert
bounds, count, and uniqueness of dealt positions rather than a particular
random sequence.

## Benchmarks

[benches/interpreter.rs](https://github.com/ecmel/pliq/blob/main/benches/interpreter.rs)
times representative programs: user-function calls, closures, recursion,
evaluated text, and whole-array primitives. It uses no benchmark framework.

```sh
cargo bench
cargo bench -- closure
```

Each benchmark runs its setup code once, then evaluates its program repeatedly
in the same interpreter for about half a second (5 to 100 samples) and reports
the median and minimum time. Every run checks the program's result, so a
benchmark cannot time an error by mistake. `cargo test --benches` runs each
program once as a smoke test.

To measure a change, run `cargo bench` on the base commit and on the change on
an otherwise idle machine, then compare medians. Function-call benchmarks
measure the evaluator; whole-array benchmarks mostly measure Arrow kernels.
Add a benchmark with its expected result when a change targets a workload the
suite does not cover.

## Coverage

With `cargo-llvm-cov` and LLVM tools already installed:

```sh
cargo llvm-cov --all-targets --ignore-filename-regex 'tests/' --summary-only
```

There is no enforced percentage threshold. Prefer meaningful behavior and
error-path checks over tests that simply mirror an implementation branch.
See [documentation maintenance](documentation.md) for checking prose examples.
