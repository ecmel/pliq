# pliq

An array language in Rust with an Apache Arrow backend. Native integers,
floats, and booleans work with shared mutable arrays, dictionaries, tables, and byte strings.

```text
1 2 3+10                  // 11 12 13
+/!10                     // 45
+\1 2 3 4                 // 1 3 6 10
square:{x*x};square'1 2 3 // 1 4 9
```

## Download

Prebuilt binaries for Linux, Windows, and macOS on x86-64 and arm64 are on the
[latest release](https://github.com/ecmel/pliq/releases/latest).
[Getting started](docs/user/getting-started.md#download-a-release) links to
each platform's archive.

## Run

Building from source requires Rust 1.88 or newer.

```sh
cargo build --release
cargo run -- -e '+/!100'
cargo run -- examples/stats.pliq
cargo run
```

The last command starts the REPL when stdin is a terminal.
Use `\h` for help, `\c` to discard pending input, and `\q` to exit.

## Documentation

- [Documentation home](docs/index.md)
- [Getting started](docs/user/getting-started.md)
- [Language reference card](docs/user/reference-card.md)
- [Operator reference](docs/user/operators.md)
- [Example programs](docs/user/examples.md)
- [Rust API and embedding](docs/developer/rust-api.md)
- [Development and testing](docs/developer/development.md)
- [Build and preview the docs](docs/developer/documentation.md)
- [Changelog](CHANGELOG.md)

Licensed under the [MIT License](LICENSE).
