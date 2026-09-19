# Changelog

Notable changes to pliq, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- REPL results fit a console size: never more than 1000 rows, at the
  terminal's width by default. Cut lines end in `..`, and tall tables and
  dictionaries end in `..` and their row or entry count. `\c rows columns` sets
  5 to 1000 rows and at least 20 columns, with `0` for the most, so `\c 0 0`
  shows up to 1000 rows at any width. `\c` shows the size, and `\c auto`
  restores the default. Error messages and output from `-e`, files, and piped
  input are never cut.
- Attached REPLs send their console size with each request (request mode `3`),
  so the daemon renders only what fits, and fall back to text requests with
  0.1.0 daemons.
- `Value::view(Console)` renders a value within a console size from Rust.

### Changed

- `\d` now discards pending REPL input, including on Ctrl-C; `\c` sets the
  console size.

- Calls to user-defined functions are about 5 to 9 times faster. Function
  bodies resolve their names to call-frame slots before evaluation, so calls no
  longer copy a closure's captured bindings, and primitive applications and
  literal vectors no longer rebuild their values on each evaluation.
- Numeric vector operations run as Arrow kernels for every elementwise
  arithmetic, comparison, minimum, maximum, and fill primitive, including a
  scalar on either side and mixed boolean, integer, and float types, and for
  unary `-`, `%`, `_`, and `~`. `a*2` and `a<5` on a million integers take
  under 1 ms instead of about 30 ms, with unchanged results.
- Release binaries are about half their previous size and run function calls
  about 15% faster, from link-time optimization, size-optimized code, and
  stripped symbols.

## [0.1.0] - 2026-09-19

### Added

- A Rust array language with an Apache Arrow backend, native integers, floats,
  booleans, a single `0n` null value, and numeric promotion.
- Homogeneous arrays, dictionaries with independently typed values, symbols,
  and byte strings, with shared mutable containers and indexed updates.
- Tables with named, variable-length shared columns, implicit null cells, row
  selection, atomic aligned append, and Arrow record-batch interchange.
- Aligned dictionary key/value output in the CLI and REPL.
- Aligned table output in the CLI and REPL, with column headers and explicit
  nulls for missing cells.
- Multi-selector indexed assignments in a single bracket.
- Adjacent `.name` symbol indexing, including dictionary and table access,
  chained paths, and shared indexed updates.
- Right-to-left evaluation, functions, closures, projections, iterators,
  reductions, scans, casts, and named numeric functions.
- Line comments beginning with `//`.
- A CLI for expressions, files, and standard input, plus an interactive REPL
  with persistent bindings, multiline input, and `\h`, `\c`, and `\q` commands
  for help, clearing pending input, and quitting.
- Terminal line editing, Up/Down history recall, Ctrl-R search, and persistent
  expression history shared by local and attached REPLs.
- A `--listen [HOST:]PORT` TCP daemon that evaluates an optional startup
  file and then serves length-prefixed requests, and a `--attach` REPL client.
  Attached sessions share one interpreter and its bindings, evaluate serially
  in arrival order, and receive results only on their own connection. TCP works
  on Linux, macOS, and Windows, supports IPv4 and IPv6, and uses `TCP_NODELAY`
  for small requests. The listener logs client connections and disconnections
  with their peer addresses to standard error. A bare port defaults to
  `127.0.0.1` for both listening and attaching.
- A Rust API for embedding the interpreter, including Arrow numeric and symbol
  snapshot import/export.
- User and developer documentation, runnable examples including a sales-report
  language demo, and an mdBook site.
- Rust regression tests and independent arithmetic reference checks.
- Release automation for Linux, Windows, and macOS x86-64/arm64 binaries
  with checksums, under version-free archive names that the documentation links
  to as the latest release's downloads.
- MIT licensing.

[Unreleased]: https://github.com/ecmel/pliq/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ecmel/pliq/releases/tag/v0.1.0
