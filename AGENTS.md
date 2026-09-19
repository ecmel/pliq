# Repository Guidelines

## Project Structure & Module Organization

pliq is a Rust array language with an Apache Arrow backend.
`src/lib.rs` exports the interpreter API; `src/main.rs` implements the CLI and
REPL. The lexer/parser lives in `src/parser.rs`, name resolution into frame
slots in `src/compile.rs`, evaluation and operators in `src/eval.rs` and
`src/operators.rs`, native numeric arithmetic in `src/number.rs`,
Arrow storage and kernels in `src/arrow.rs`, runtime values and shared arrays in
`src/value.rs`, and shared byte strings in `src/string.rs`. The Unix socket
daemon and its attached REPL client live in `src/net.rs`.

Integration tests live in `tests/`; `tests/support/` contains Rust reference
arithmetic, parser mutation tests, and scripted REPL tests included by unit-test
modules. Interpreter benchmarks live in `benches/interpreter.rs`. Runnable
programs use `.pliq` files in `examples/`. Language documentation
starts at `docs/index.md`, with mdBook navigation in `docs/SUMMARY.md` and
configuration in `book.toml`. `README.md` is a short overview; testing
and verification are in `docs/developer/testing.md`.

## Build, Test, and Development Commands

Use Rust 1.88 or newer, with edition 2024.

- `cargo fetch --locked`: optionally prefetch dependencies pinned in `Cargo.lock`.
- `cargo build --release`: build `target/release/pliq`.
- `cargo run`: start the REPL when stdin is a terminal.
- `cargo run -- -e '+/!100'`: evaluate an expression.
- `cargo run -- examples/stats.pliq`: execute an example file.
- `cargo test`: run unit and integration tests.
- `cargo test --release`: verify optimized arithmetic and execution.
- `cargo fmt --check`: check formatting; use `cargo fmt` to apply it.
- `cargo clippy --all-targets -- -D warnings`: require clean lint output.
- `cargo bench`: run the interpreter benchmarks; `cargo bench -- NAME` filters them.
- `mdbook build`: build the documentation into `target/book` (requires mdBook).

## Coding Style & Naming Conventions

Follow rustfmt defaults with four-space indentation. Use `snake_case` for modules,
functions, and tests, `UpperCamelCase` for types, and `SCREAMING_SNAKE_CASE` for
constants. Use the Apache Arrow Rust crates for typed vector storage and kernels;
keep tests in Rust.

Keep arrays homogeneous and reject type-changing indexed updates.
Keep native numeric semantics explicit: `i64` integers with defined wrapping,
`f64` IEEE arithmetic, booleans, numeric promotion, and typed null behavior.
Preserve right-to-left evaluation and shared mutable arrays, dictionaries, and
byte strings: aliases and closures observe indexed
updates, while rebinding changes only the variable. Reject reference cycles and
preserve destinations on failed updates. Update examples and documentation for
syntax changes.

## Testing Guidelines

Use Rust's built-in `#[test]` framework and descriptive behavior-based names.
Add regression cases to the relevant language, numbers, strings, or CLI suite.
Arithmetic changes should also exercise the independent reference implementation and signed
boundaries. Run debug and release tests for arithmetic or evaluator changes, and
compare `cargo bench` results before and after performance changes.

With cargo-llvm-cov and llvm-tools installed:

```sh
cargo llvm-cov --all-targets --ignore-filename-regex 'tests/' --summary-only
```

No coverage threshold is enforced. Prioritize meaningful error-path and behavior
coverage.

## Commit & Pull Request Guidelines

Use concise, imperative commit subjects. PRs should explain the problem,
resulting behavior, relevant issue links, and validation commands/results.
Include input/output examples for language changes.

Maintain `CHANGELOG.md` alongside user-visible changes, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Add concise entries under
`[Unreleased]`, grouped as Added, Changed, Deprecated, Removed, Fixed, or Security,
in that order. Before the first release, use only Added and describe the current
initial feature set; do not use Changed, Deprecated, Removed, Fixed, or Security
without a previous release to compare against. Record implemented behavior, not
proposals. When releasing, move
entries to a `## [x.y.z] - YYYY-MM-DD` section, leave `[Unreleased]` in place for
future work, and update the link references at the end of the file: `[Unreleased]`
compares the newest `vx.y.z` tag with `HEAD`, each release compares its tag with
the previous one, and the first release links to its tag.

## License and Distribution

Keep `publish = false` and the MIT `LICENSE`. Do not publish package
artifacts or source as part of routine contribution work.

Release binaries are published only by pushing a `vx.y.z` tag that matches the
`Cargo.toml` version. `.github/workflows/release.yml` then tests and builds Linux
x86-64/arm64, Windows x86-64/arm64, and macOS x86-64/arm64 archives and attaches them, with
`SHA256SUMS.txt`, to a GitHub release whose notes are that version's changelog
section. Archive names (`pliq-<target>`) carry no version because
`docs/user/getting-started.md` links to them at `releases/latest/download/`; keep
that download table in step with the build matrix.
