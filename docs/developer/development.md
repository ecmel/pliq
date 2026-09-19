# Development

[Developer guide](index.md) · [Architecture](architecture.md) · [Testing](testing.md)

## Toolchain and dependencies

Use Rust 1.88 or newer with edition 2024. The implementation uses
Apache Arrow 58 for vector storage and arithmetic. Cargo downloads dependencies
as needed. `Cargo.toml` keeps
`publish = false` to disable package registry publishing.

## Commands

Run these from the repository root:

```sh
cargo build --release
cargo run -- -e '+/!100'
cargo run -- examples/stats.pliq
cargo test
cargo test --release
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```

`cargo fmt` applies formatting. The release binary is `target/release/pliq`
(`pliq.exe` on Windows). Generated API documentation starts at
`target/doc/pliq/index.html` after `cargo doc`.

The prose documentation uses mdBook. See [documentation maintenance](documentation.md)
for installation, `mdbook build`, local preview, and GitHub Pages deployment.

See [testing](testing.md) for regression suites and verification guidance.

## Source map

| Path                                                                         | Responsibility                                                  |
| ---------------------------------------------------------------------------- | --------------------------------------------------------------- |
| [src/lib.rs](https://github.com/ecmel/pliq/blob/main/src/lib.rs)             | Public exports and `Result` alias                               |
| [src/main.rs](https://github.com/ecmel/pliq/blob/main/src/main.rs)           | CLI, line-based REPL, continuation detection                    |
| [src/net.rs](https://github.com/ecmel/pliq/blob/main/src/net.rs)             | Frame codec, TCP daemon, attached REPL client           |
| [src/parser.rs](https://github.com/ecmel/pliq/blob/main/src/parser.rs)       | Byte-oriented lexer, tokens, expression tree, parser            |
| [src/eval.rs](https://github.com/ecmel/pliq/blob/main/src/eval.rs)           | Persistent interpreter, environments, evaluation, application   |
| [src/operators.rs](https://github.com/ecmel/pliq/blob/main/src/operators.rs) | Structured primitives, casts, iterators, amend, random sampling |
| [src/number.rs](https://github.com/ecmel/pliq/blob/main/src/number.rs)       | Numeric representations, arithmetic, comparison, display        |
| [src/arrow.rs](https://github.com/ecmel/pliq/blob/main/src/arrow.rs)         | Arrow vector storage, validity, and arithmetic kernels          |
| [src/value.rs](https://github.com/ecmel/pliq/blob/main/src/value.rs)         | Values, functions, shared arrays/dictionaries, cycle checks     |
| [src/value/table.rs](https://github.com/ecmel/pliq/blob/main/src/value/table.rs) | Shared table columns, row selection, append, and Arrow batches |
| [src/value/display.rs](https://github.com/ecmel/pliq/blob/main/src/value/display.rs) | Control-character escaping, cell rendering, aligned dictionary output |
| [src/string.rs](https://github.com/ecmel/pliq/blob/main/src/string.rs)       | Shared mutable byte-string storage                              |
| [tests](https://github.com/ecmel/pliq/tree/main/tests)                       | Integration suites and unit-test support files                  |
| [examples](https://github.com/ecmel/pliq/tree/main/examples)                 | Runnable language examples                                      |
| [docs](../index.md)                                                          | User and developer documentation                                |

## Change conventions

Use rustfmt defaults, four-space indentation, `snake_case` functions/modules,
and `UpperCamelCase` types. Preserve explicit wrapping integer semantics,
floating promotion, null behavior, right-to-left evaluation, and the shared
mutation guarantees described in [architecture](architecture.md).

Keep dependencies focused on the Arrow backend. Put behavior
regressions in the relevant Rust suite. Arithmetic changes should exercise
signed boundaries and the independent reference implementation; evaluator and
arithmetic changes need both debug and release validation.

For user-visible changes, update [CHANGELOG.md](https://github.com/ecmel/pliq/blob/main/CHANGELOG.md), the affected
documentation, and any syntax examples. Use concise imperative commit subjects.
PR descriptions should state the concrete problem, resulting behavior, and
validation. Preserve the MIT [LICENSE](https://github.com/ecmel/pliq/blob/main/LICENSE) and
`publish = false` setting.
