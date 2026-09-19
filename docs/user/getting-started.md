# Getting started

[User guide](index.md) · [Next: syntax and evaluation](syntax.md)

## Download a release

These links always fetch the archives of the latest release:

| Platform       | Archive                                                                                                                                |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Linux x86-64   | [pliq-x86_64-unknown-linux-gnu.tar.gz](https://github.com/ecmel/pliq/releases/latest/download/pliq-x86_64-unknown-linux-gnu.tar.gz)   |
| Linux arm64    | [pliq-aarch64-unknown-linux-gnu.tar.gz](https://github.com/ecmel/pliq/releases/latest/download/pliq-aarch64-unknown-linux-gnu.tar.gz) |
| Windows x86-64 | [pliq-x86_64-pc-windows-msvc.zip](https://github.com/ecmel/pliq/releases/latest/download/pliq-x86_64-pc-windows-msvc.zip)             |
| Windows arm64  | [pliq-aarch64-pc-windows-msvc.zip](https://github.com/ecmel/pliq/releases/latest/download/pliq-aarch64-pc-windows-msvc.zip)           |
| macOS x86-64   | [pliq-x86_64-apple-darwin.tar.gz](https://github.com/ecmel/pliq/releases/latest/download/pliq-x86_64-apple-darwin.tar.gz)             |
| macOS arm64    | [pliq-aarch64-apple-darwin.tar.gz](https://github.com/ecmel/pliq/releases/latest/download/pliq-aarch64-apple-darwin.tar.gz)           |

[SHA256SUMS.txt](https://github.com/ecmel/pliq/releases/latest/download/SHA256SUMS.txt)
lists their checksums, and the [releases page](https://github.com/ecmel/pliq/releases)
has earlier versions. The REPL banner shows the version you have.

Each archive holds a directory of the same name with the executable, `LICENSE`,
and `README.md`. On Linux or macOS, download and unpack one from a terminal,
substituting your platform's archive name:

```sh
curl -fsSL https://github.com/ecmel/pliq/releases/latest/download/pliq-aarch64-apple-darwin.tar.gz | tar -xz
./pliq-aarch64-apple-darwin/pliq -e '+/!10'
```

The expression prints `45`. On Windows, extract the `.zip` and run `pliq.exe`.
The macOS binaries are not signed, so macOS may refuse to run one downloaded
with a browser; remove the quarantine flag with
`xattr -d com.apple.quarantine pliq`, or download it with `curl` as above. The
Linux builds link against glibc; on musl-based systems such as Alpine, build
from source instead.

Subsequent examples use `pliq` to mean the executable at its installed location
or on your `PATH`.

## Build from source

From a checkout, use Rust 1.88 or newer. pliq uses edition 2024 and the Apache Arrow
Rust crates. Cargo downloads dependencies as needed.

```sh
cargo build --release
./target/release/pliq -e '+/!10'
```

On Windows, the executable is `target\release\pliq.exe`. Run `cargo run` from a
terminal to start the REPL, or use `cargo run -- -e '+/!10'` without first
building a release binary.

## Think in arrays

A space-separated sequence of numbers is a vector. Operations combine equal
length vectors element by element. A scalar extends to every element.

```pliq
1 2 3+10                 // => 11 12 13
1 2 3*4 5 6              // => 4 10 18
```

All operators have equal precedence and evaluation proceeds right to left.
Parentheses make a different grouping explicit.

```pliq
2*3+4                    // => 14
(2*3)+4                  // => 10
```

`!10` generates integers from zero through nine. Appending `/` to `+` makes a
reduction: it combines the elements into one sum. Appending `\` keeps the
intermediate sums.

```pliq
!5                       // => 0 1 2 3 4
+/!10                    // => 45
+\1 2 3 4                // => 1 3 6 10
```

## Name values and functions

Use `:` to bind a name and braces to define a function. Brackets pass arguments;
a space can pass one argument expression. The implicit first parameter is `x`.

```pliq
square:{x*x}
square[5]                // => 25
square'1 2 3             // => 1 4 9
```

The apostrophe means *each*: call the function once for every input element.

## Write a program

Save this as `summary.pliq`:

```pliq
data:1 2 3 4 5
total:+/data
mean:total%#data
(total;mean)             // => 15 3
```

Run it with `pliq summary.pliq`. `#data` counts the elements, `%` divides, and
parentheses with semicolons collect results. A file prints the value of its last
statement. Comments start with `//` and continue to the end of the line.

## Update a collection

Indexing is zero-based. Binding another name to an array shares its mutable
identity, so both names see indexed edits.

```pliq
a:10 20 30
b:a
a[1]:99
b                        // => 10 99 30
```

Read [mutation and amend](mutation.md) before using shared collections in larger
programs. Continue with [syntax](syntax.md), consult the
[reference card](reference-card.md), or run the [included examples](examples.md).
