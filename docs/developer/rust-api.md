# Rust API

[Developer guide](index.md) · [Architecture](architecture.md) · [Development](development.md)

The crate exports `Interpreter`, `Value`, `Number`, `Array`, `Dictionary`,
`MutableString`, `Table`, `Console`, `View`, and `Result<T>`. The result alias is
`std::result::Result<T, String>`. Use a local path dependency from a host project;
the crate is not published to a registry.

```toml
[dependencies]
pliq = { path = "../pliq" }
```

## Evaluate source

`Interpreter::new()` and `Interpreter::default()` initialize the built-in
environment. `eval(&mut self, source: &str) -> pliq::Result<Value>` parses and
evaluates source, returning the last statement. Bindings survive subsequent
calls.

```rust
use pliq::{Interpreter, Number, Value};

fn main() -> pliq::Result<()> {
    let mut interpreter = Interpreter::new();
    interpreter.eval("a:10 20 30")?;
    let result = interpreter.eval("+/a")?;
    assert!(matches!(result, Value::Number(Number::Int(60))));
    println!("{result}");

    assert!(interpreter.eval("missing").is_err());
    assert_eq!(interpreter.eval("a[0]")?.to_string(), "10");
    Ok(())
}
```

There is no public environment setter, host-function registration API, parsed
program API, cancellation hook, or evaluation timeout. Create bindings with
language source and inspect returned values through their variants. The
runtime uses `Rc` and `RefCell`; values and interpreters are not designed to
move between threads. Create separate interpreters within separate threads
when isolation is needed.

## Value variants

| Variant                         | Contents                                |
| ------------------------------- | --------------------------------------- |
| `Value::Number(Number)`         | Integer, float, or boolean              |
| `Value::Null`                   | Single null value, displayed as `0n`             |
| `Value::Char(u8)`               | Character byte                          |
| `Value::String(MutableString)`  | Shared byte string                      |
| `Value::Symbol(Rc<str>)`        | Immutable symbol name                   |
| `Value::Array(Array)`           | Shared array                            |
| `Value::Dictionary(Dictionary)` | Shared dictionary                       |
| `Value::Table(Table)`           | Shared named array columns              |
| `Value::Function(...)`          | Opaque reference-counted function value |

Values implement `Clone`, `Debug`, and `Display`. Cloning a mutable container
shares it. `Value::array(iter)` returns a `Result` from an iterator of values; nonempty
all-character inputs become strings. Use `Value::Array(array)` to wrap an
`Array` explicitly. Internal helpers such as `Value::text`, `as_text`, and
`detached` are not public APIs.

## Console display

`value.view(Console { rows, columns })` returns a `View` whose `Display` shows
the value as the REPL does: tables and dictionaries as aligned rows, other
values in compact syntax. Zero leaves a dimension unlimited;
`Console::UNLIMITED` shows everything, as `pliq -e` does. Lines wider than
`columns` end in `..`. Output taller than `rows` ends in a `..` line, and tables
and dictionaries add their row or entry count. Only the part that fits is
rendered.

```rust
use pliq::{Console, Interpreter};

fn main() -> pliq::Result<()> {
    let table = Interpreter::new().eval("([] a:!10)")?;
    let shown = table.view(Console { rows: 5, columns: 80 }).to_string();
    assert_eq!(shown, "a\n-\n0\n..\n10 rows");
    Ok(())
}
```

## Numbers

Construct variants directly or use the `From<i64>`, `From<f64>`, and
`From<bool>` implementations. `Number::integer(i64)` and `Number::float(f64)`
are also available.

| Method / trait  | Behavior                                               |
| --------------- | ------------------------------------------------------ |
| `as_i64()`      | `Some` for integers and booleans; `None` for floats    |
| `as_f64()`      | Convert to a native float             |
| `type_code()`   | Positive numeric type code: 1, 7, or 9                 |
| `round(places)` | `Result<Number>`; decimal places in `0..=308`          |
| `FromStr`       | Parse native integer/float and infinity spellings |
| `Display`       | Print pliq numeric conventions                         |

Numeric arithmetic methods are internal. Rust `Eq`/`Ord` comparison is not the
same API as tolerant language equality; evaluate language expressions when
language operator semantics are required. The full `i64` range is available.
Use `Value::Null` for absence and `Value::from_number(number)` to convert a
native numeric result into a language value, normalizing IEEE NaN to null.
Raw `Number::Float(f64)` can hold any native float; it is not itself a nullable type.

## Arrays

`Array::numbers(Vec<Number>)` constructs a homogeneous Arrow numeric vector,
promoting booleans to integers and integers to floats as needed.
`Array::new(iterator)` and `Value::array(iterator)` return `Result` and reject
incompatible element types. `FromIterator<Value>` is no longer implemented for
`Array`.

| Method                | Result and semantics                                         |
| --------------------- | ------------------------------------------------------------ |
| `len()`, `is_empty()` | Top-level size                                               |
| `get(index: usize)`   | `Option<Value>`; clones an element                           |
| `iter()`              | Owned snapshot iterator; nested containers stay shared       |
| `as_numbers()`        | `Option<Rc<Vec<Number>>>` materialized non-null numeric snapshot (copies) |
| `set(index, value)`   | Replace through aliases; type/bounds/cycle errors return `Err`    |
| `append(value)`       | Append a scalar or an array's elements through aliases       |

`as_numbers()` returns `None` for nullable arrays; use `iter()` or `to_arrow()`
to preserve validity.

Rust getters return `None` for an absent index. This differs from language
indexing, which supplies a null. Appending an array flattens one level; wrap a
nested value with `Value::array([value])?` when appending it as a single item.

```rust
use pliq::{Array, Number, Value};

fn main() -> pliq::Result<()> {
    let array = Array::numbers(vec![Number::Int(1), Number::Int(2)]);
    let alias = array.clone();
    let before = array.as_numbers().expect("numeric storage");

    array.set(0, Value::Number(Number::Int(9)))?;
    array.append(Value::Number(Number::Int(3)))?;
    assert_eq!(Value::Array(alias).to_string(), "9 2 3");
    assert_eq!(before[0].as_i64(), Some(1));
    assert_eq!(before.len(), 2);
    Ok(())
}
```

## Dictionaries

`Dictionary::Display` renders aligned key/value rows for screen output, preserving
entry order and duplicate keys. `Value::Display` retains compact dictionary syntax
for nested values. Neither rendering changes the dictionary.

`Dictionary::new(Vec<Rc<str>>, Array)` constructs symbol keys.
`Dictionary::from_arrays(Array, Array)` accepts homogeneous keys and a value
array, including an existing mixed dictionary value list.
`Dictionary::from_values(Array, impl IntoIterator<Item = Value>)` accepts
independently typed values without numeric promotion. All constructors return a
`Result` and reject unequal lengths. Constructors detach keys recursively and
snapshot the outer value array; nested mutable values remain shared.

| Method                       | Result and semantics                                        |
| ---------------------------- | ----------------------------------------------------------- |
| `len()`, `is_empty()`        | Pair count, including duplicate keys                        |
| `keys()`                     | Detached key array                                          |
| `values()`                   | Independent outer snapshot; nested values can remain shared |
| `get(&str)`                  | Symbol-key lookup returning `Option<Value>`                 |
| `get_value(&Value)`          | Arbitrary-key lookup returning `Option<Value>`              |
| `insert(Rc<str>, Value)`     | Insert or replace first matching symbol key                 |
| `insert_value(Value, Value)` | Insert or replace first matching arbitrary key              |

Both insertion methods return `Result<()>`, reject cycles and incompatible key
types, and mutate through dictionary aliases. Values may have different types;
replacement may change an entry's type. Mixed value snapshots have type code
`0` and cannot be exported as Arrow vectors. Language lookup adds null behavior and vector selection;
the Rust getters perform one lookup and return `None` when missing.

```rust
use std::rc::Rc;
use pliq::{Array, Dictionary, Number, Value};

fn main() -> pliq::Result<()> {
    let dictionary = Dictionary::new(
        vec![Rc::<str>::from("count")],
        Array::numbers(vec![Number::Int(1)]),
    )?;
    let alias = dictionary.clone();
    dictionary.insert(Rc::from("count"), Value::Number(Number::Int(2)))?;
    assert_eq!(alias.get("count").unwrap().to_string(), "2");
    assert!(alias.get("missing").is_none());
    Ok(())
}
```

## Mutable byte strings

`MutableString::new(bytes)` accepts `AsRef<[u8]>`, including invalid UTF-8.
Wrap it in `Value::String` for display or use its byte-level API directly.

| Method                | Result and semantics                       |
| --------------------- | ------------------------------------------ |
| `len()`, `is_empty()` | Byte count                                 |
| `get(index)`          | `Option<u8>`                               |
| `snapshot()`          | Stable `Rc<Vec<u8>>`                       |
| `set(index, byte)`    | `Result<()>`; writes through aliases       |
| `append(bytes)`       | Append bytes through aliases; returns `()` |

```rust
use pliq::{MutableString, Value};

fn main() -> pliq::Result<()> {
    let text = MutableString::new(b"abc");
    let before = text.snapshot();
    let alias = text.clone();
    text.set(0, b'X')?;
    text.append(b"!");
    assert_eq!(before.as_slice(), b"abc");
    assert_eq!(alias.snapshot().as_slice(), b"Xbc!");
    assert_eq!(Value::String(alias).to_string(), "\"Xbc!\"");
    Ok(())
}
```

Use [generated Rust documentation](development.md#commands) for the exported
signatures and the [architecture guide](architecture.md) for storage invariants.

See [Arrow backend](arrow.md) for `Array::from_arrow` and `Array::to_arrow`.

## Tables

`Table::Display` renders aligned headers and rows, as used by the CLI and REPL.
`Value::Display` retains compact syntax for embedding tables inside other values.

`Value::Table(Table)` stores named shared `Array` columns. `Table::new` accepts
`Vec<(Rc<str>, Array)>`, requires unique names and at least one column, and
preserves array identity without requiring equal column lengths.

| Method | Behavior |
| --- | --- |
| `len()`, `is_empty()` | Logical row count is the longest column |
| `names()` | Independent symbol array of column names |
| `column(name)` | Optional shared array with its actual physical length |
| `columns()` | Ordered name/array pairs sharing column identity |
| `set_column(name, array)` | Add or replace a column; existing element type must match |
| `append(&source)` | Atomically append aligned rows, preserving destination array identities |
| `to_arrow()` | Rectangular `RecordBatch` snapshot; pads short columns with nulls |
| `from_arrow(&batch)` | Import supported arrays; reject duplicate names and zero-column schemas |

Arrow interchange supports the same types as `Array` interchange. Export shares
buffers for full-length columns and allocates padded short columns. It never
pads the original table. Imported fields retain their names and supported array
types, but Arrow field/schema metadata and non-nullability constraints are not
retained. Exports mark fields nullable. Float NaNs normalize to nulls, as with
`Array::from_arrow`. Tables containing other runtime column types fail export.
See [table semantics](../user/tables.md) for aliasing and append rules.
