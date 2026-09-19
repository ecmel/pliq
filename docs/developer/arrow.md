# Arrow backend

[Developer guide](index.md) · [Rust API](rust-api.md) · [Architecture](architecture.md)

pliq uses Apache Arrow 58's native Rust implementation. `arrow-array` owns vector
buffers, `arrow-buffer` supplies validity and bit-packed buffers, and
`arrow-arith` supplies typed kernels. Rust 1.88 remains the minimum.

## Storage and operations

Boolean, integer, float, and symbol vectors use `BooleanArray`, `Int64Array`,
`Float64Array`, and `StringArray`. Booleans are bit-packed; integers and floats
occupy eight bytes per value plus an optional validity bitmap. Numeric lists
promote to one type: boolean to integer to float. Promotion can lose integer
precision when converting to float. Incompatible element types are errors.

Nested arrays and arrays of runtime objects remain homogeneous, but use typed
Rust storage to preserve shared child identity. Byte strings retain their
existing shared byte storage. These values do not yet have Arrow import/export.
Tables hold named shared arrays and support Arrow record-batch interchange.
Parquet file I/O, device I/O, and streaming are not implemented.

Elementwise numeric primitives run as Arrow kernels on numeric vectors, with
a number or `0n` on either side broadcast across the vector: `+ - * %`, `mod`,
`pow`, minimum `&`, maximum `|`, fill `^`, and comparisons `= < >`, plus unary
`-`, `%`, `_`, and `~`. Operands of mixed types convert to the result's domain
first: booleans to integers, and anything with a float, `%`, or `pow` to
floats. Results keep the rules for applying the primitive to each pair of
elements: integer arithmetic wraps, undefined float results and integer `mod`
by zero are null, arithmetic and `&` propagate nulls, `|` and `^` keep the
valid side, comparisons treat null as less than any number, and float equality
is tolerant. Nested arrays and arrays of runtime values still apply the
primitive element by element, reaching the kernels for any numeric children.

Numeric reductions and scans iterate Arrow snapshots directly. There is no
expression fusion or automatic parallel execution, and no performance
improvement is claimed without measurement; `cargo bench` includes vector
arithmetic, scalar broadcasting, comparison, and mixed-type cases.

Cloning an array retains shared mutable identity. Indexed updates and appends
must preserve its element type and build replacement storage before committing.
They can copy the whole vector; batching is preferable to individual appends.
Arrow exports and iterators keep stable snapshots across updates. Slices share
Arrow buffers, so a small slice can retain a large backing allocation.

`()` is an empty integer vector. Slicing and reversing preserve element types.
An empty dictionary infers its key type on first insertion; later keys must
match it. Dictionary values may differ in type and replacement may change an
entry's type. Homogeneous value storage uses the existing typed representation;
mixed values use a runtime value list and cannot be exported as an Arrow vector. Nested arrays must have matching child types, including typed
empty children, but may have different lengths.

## Rust interchange

Use the same Arrow major version in a host application:

```rust
use std::sync::Arc;
use arrow_array::{Array as _, Int64Array};
use pliq::{Array, Number, Value};

fn main() -> pliq::Result<()> {
    let input = Arc::new(Int64Array::from(vec![Some(1), None, Some(3)]));
    let values = Array::from_arrow(input)?;
    let snapshot = values.to_arrow()?;
    values.set(0, Value::Number(Number::Int(9)))?;
    assert!(snapshot.is_null(1));
    assert_eq!(snapshot.as_any().downcast_ref::<Int64Array>().unwrap().value(0), 1);
    assert_eq!(values.get(0).unwrap().to_string(), "9");
    Ok(())
}
```

`Array::from_arrow(ArrayRef)` and `Array::to_arrow()` return `Result` and share
supported buffers. Import currently accepts Boolean, Int64, Float64, Utf8, and Null;
Utf8 represents symbols, not byte strings. Other Arrow types are rejected.
`as_numbers()` allocates a `Rc<Vec<Number>>` snapshot for non-null numeric
vectors and returns `None` for nullable vectors.

Nulls are represented by `Value::Null` in scalars and validity bits in numeric
and symbol vectors. Nullable Boolean and Utf8 arrays are supported, along with
Arrow Null arrays. Valid `i64::MIN` values remain ordinary integers. Imported
IEEE NaN values are normalized to nulls, allocating a new float buffer when
necessary; other supported imports share their buffers. Empty symbols remain
ordinary symbols.
