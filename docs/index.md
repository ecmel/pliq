# pliq documentation

pliq is an array language in Rust with an Apache Arrow backend.
Numbers use native integers, floats, and booleans. Arrays, dictionaries, and
byte strings support shared mutation.

These pages document the implemented language.

## Start here

- [Getting started](user/getting-started.md): download or build pliq and write a first program.
- [Reference card](user/reference-card.md): syntax, operators, functions, and commands.
- [Examples](user/examples.md): small programs with explanations.

## User guide

Read the [user guide index](user/index.md), or jump to a topic:

| Topic                                                 | Contents                                                 |
| ----------------------------------------------------- | -------------------------------------------------------- |
| [CLI and REPL](user/cli.md)                           | Files, expressions, standard input, interactive commands |
| [Syntax and evaluation](user/syntax.md)               | Names, literals, spacing, precedence, assignment         |
| [Values and collections](user/values.md)              | Numbers, nulls, strings, arrays, dictionaries, indexing  |
| [Operators](user/operators.md)                        | Every supported symbol and its argument forms            |
| [Casts and named functions](user/builtins.md)         | Text conversion, numeric conversion, aggregates, helpers |
| [Functions and iterators](user/functions.md)          | Closures, projections, each, reduce, scan, control flow  |
| [Mutation and amend](user/mutation.md)                | Aliases, indexed writes, append, functional updates      |
| [Errors and troubleshooting](user/troubleshooting.md) | Error handling, common mistakes, runtime limits          |

## Developer guide

- [Development](developer/development.md): toolchain, commands, repository conventions.
- [Architecture](developer/architecture.md): parser, evaluator, storage, mutation.
- [Rust API](developer/rust-api.md): embedding and public value/container APIs.
- [Testing](developer/testing.md): test suites, regression checks, coverage.
- [Releases](developer/releases.md): versioning, archives, distribution.
- [Maintaining documentation](developer/documentation.md): page structure and verification.

See also the [developer index](developer/index.md), [changelog](https://github.com/ecmel/pliq/blob/main/CHANGELOG.md),
and [repository overview](https://github.com/ecmel/pliq/blob/main/README.md).
