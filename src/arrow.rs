//! Arrow storage for homogeneous numeric vectors with explicit validity.
//! Elementwise primitives run as Arrow kernels under pliq's Number rules.
use std::{cmp::Ordering, sync::Arc};

use arrow_arith::arity::{binary, unary};
use arrow_array::{
    Array as _, ArrayRef, ArrowPrimitiveType, BooleanArray, Float64Array, Int64Array,
    PrimitiveArray,
    types::{Float64Type, Int64Type},
};
use arrow_buffer::{BooleanBuffer, NullBuffer};

use crate::{Number, Result, number::equivalent_floats};

#[derive(Clone, Debug)]
pub(crate) enum Numbers {
    Bool(BooleanArray),
    Int(Int64Array),
    Float(Float64Array),
}

impl Numbers {
    pub(crate) fn new(values: Vec<Option<Number>>, kind: i64) -> Self {
        match kind {
            9 => Self::Float(Float64Array::from_iter(
                values
                    .into_iter()
                    .map(|n| n.map(Number::as_f64).filter(|n| !n.is_nan())),
            )),
            1 => Self::Bool(BooleanArray::from_iter(
                values
                    .into_iter()
                    .map(|n| n.map(|n| n.as_i64().unwrap() != 0)),
            )),
            _ => Self::Int(Int64Array::from_iter(
                values.into_iter().map(|n| n.map(|n| n.as_i64().unwrap())),
            )),
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Bool(a) => a.len(),
            Self::Int(a) => a.len(),
            Self::Float(a) => a.len(),
        }
    }
    pub(crate) fn get(&self, i: usize) -> Option<Number> {
        match self {
            Self::Bool(a) => (!a.is_null(i)).then(|| Number::Bool(a.value(i))),
            Self::Int(a) => (!a.is_null(i)).then(|| Number::Int(a.value(i))),
            Self::Float(a) => (!a.is_null(i)).then(|| Number::Float(a.value(i))),
        }
    }
    pub(crate) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = Option<Number>> + ExactSizeIterator + '_ {
        (0..self.len()).map(|i| self.get(i))
    }
    pub(crate) fn slice(&self, offset: usize, len: usize) -> Self {
        match self {
            Self::Bool(a) => Self::Bool(a.slice(offset, len)),
            Self::Int(a) => Self::Int(a.slice(offset, len)),
            Self::Float(a) => Self::Float(a.slice(offset, len)),
        }
    }
    pub(crate) fn to_arrow(&self) -> ArrayRef {
        match self {
            Self::Bool(a) => Arc::new(a.clone()),
            Self::Int(a) => Arc::new(a.clone()),
            Self::Float(a) => Arc::new(a.clone()),
        }
    }
    pub(crate) fn null_count(&self) -> usize {
        match self {
            Self::Bool(a) => a.null_count(),
            Self::Int(a) => a.null_count(),
            Self::Float(a) => a.null_count(),
        }
    }
    pub(crate) fn from_arrow(array: ArrayRef) -> Result<Self> {
        if let Some(a) = array.as_any().downcast_ref::<BooleanArray>() {
            return Ok(Self::Bool(a.clone()));
        }
        if let Some(a) = array.as_any().downcast_ref::<Int64Array>() {
            return Ok(Self::Int(a.clone()));
        }
        if let Some(a) = array.as_any().downcast_ref::<Float64Array>() {
            return Ok(Self::Float(defined(a.clone())));
        }
        Err(format!("unsupported Arrow type: {}", array.data_type()))
    }
}

/// One side of an elementwise primitive. A scalar null acts as a vector of nulls.
pub(crate) enum Operand {
    Vector(Numbers),
    Number(Number),
    Null,
}

impl Operand {
    fn code(&self) -> i64 {
        match self {
            Self::Vector(Numbers::Bool(_)) => 1,
            Self::Vector(Numbers::Int(_)) => 7,
            Self::Vector(Numbers::Float(_)) => 9,
            Self::Number(n) => n.type_code(),
            Self::Null => 0,
        }
    }
    /// Booleans and integers as integers; floats have no integer column.
    fn integers(&self) -> Option<Column<Int64Type>> {
        Some(match self {
            Self::Vector(numbers) => Column::Vector(integers(numbers)?),
            Self::Number(n) => Column::Scalar(Some(n.as_i64()?)),
            Self::Null => Column::Scalar(None),
        })
    }
    fn floats(&self) -> Column<Float64Type> {
        match self {
            Self::Vector(numbers) => Column::Vector(floats(numbers)),
            Self::Number(n) => Column::Scalar(Some(n.as_f64())),
            Self::Null => Column::Scalar(None),
        }
    }
}

fn integers(numbers: &Numbers) -> Option<Int64Array> {
    match numbers {
        Numbers::Int(a) => Some(a.clone()),
        Numbers::Bool(a) => Some(Int64Array::new(
            a.values().iter().map(i64::from).collect(),
            a.nulls().cloned(),
        )),
        Numbers::Float(_) => None,
    }
}

fn floats(numbers: &Numbers) -> Float64Array {
    match numbers {
        Numbers::Float(a) => a.clone(),
        Numbers::Int(a) => unary(a, |v| v as f64),
        Numbers::Bool(a) => Float64Array::new(
            a.values().iter().map(|v| f64::from(u8::from(v))).collect(),
            a.nulls().cloned(),
        ),
    }
}

enum Column<T: ArrowPrimitiveType> {
    Vector(PrimitiveArray<T>),
    Scalar(Option<T::Native>),
}

impl<T: ArrowPrimitiveType> Column<T> {
    fn get(&self, i: usize) -> Option<T::Native> {
        match self {
            Self::Vector(a) => a.is_valid(i).then(|| a.value(i)),
            Self::Scalar(s) => *s,
        }
    }
    fn has_nulls(&self) -> bool {
        match self {
            Self::Vector(a) => a.null_count() > 0,
            Self::Scalar(s) => s.is_none(),
        }
    }
}

/// Apply a primitive element by element, or `None` for primitives that are not
/// elementwise arithmetic, comparison, or fill. Result types, promotion, and
/// nulls follow the rules for applying the primitive to each pair of elements.
pub(crate) fn dyad(op: char, x: &Operand, y: &Operand, len: usize) -> Option<Result<Numbers>> {
    let (a, b) = (x.code(), y.code());
    let code = match op {
        '=' | '<' | '>' => 1,
        '%' | 'p' => 9,
        '+' | '-' | '*' | 'm' => a.max(b).max(7),
        '&' | '|' | '^' => a.max(b),
        _ => return None,
    };
    Some(if matches!(op, '%' | 'p') || a == 9 || b == 9 {
        float_dyad(op, &x.floats(), &y.floats(), len)
    } else {
        integer_dyad(op, &x.integers()?, &y.integers()?, len, code)
    })
}

fn float_dyad(
    op: char,
    x: &Column<Float64Type>,
    y: &Column<Float64Type>,
    len: usize,
) -> Result<Numbers> {
    let arithmetic = |f: fn(f64, f64) -> f64| map(x, y, len, f).map(|a| Numbers::Float(defined(a)));
    Ok(match op {
        '+' => arithmetic(|a, b| a + b)?,
        '-' => arithmetic(|a, b| a - b)?,
        '*' => arithmetic(|a, b| a * b)?,
        '%' => arithmetic(|a, b| a / b)?,
        'p' => arithmetic(f64::powf)?,
        'm' => arithmetic(f64::rem_euclid)?,
        '&' => arithmetic(|a, b| {
            if order(a, b) == Ordering::Greater {
                b
            } else {
                a
            }
        })?,
        '|' => Numbers::Float(either(x, y, len, |a, b| {
            if order(a, b) == Ordering::Greater {
                a
            } else {
                b
            }
        })?),
        '^' => Numbers::Float(fill(x, y, len)),
        '=' => Numbers::Bool(compare(x, y, len, equivalent_floats, |a, b| !a && !b)),
        '<' => Numbers::Bool(compare(
            x,
            y,
            len,
            |a, b| !equivalent_floats(a, b) && order(a, b) == Ordering::Less,
            |a, b| !a && b,
        )),
        '>' => Numbers::Bool(compare(
            x,
            y,
            len,
            |a, b| !equivalent_floats(a, b) && order(a, b) == Ordering::Greater,
            |a, b| a && !b,
        )),
        _ => unreachable!("dyad accepts only elementwise primitives"),
    })
}

fn integer_dyad(
    op: char,
    x: &Column<Int64Type>,
    y: &Column<Int64Type>,
    len: usize,
    code: i64,
) -> Result<Numbers> {
    // Minimum, maximum, and fill of booleans stay boolean.
    let typed = |a: Int64Array| {
        if code == 1 {
            Numbers::Bool(booleans(&a))
        } else {
            Numbers::Int(a)
        }
    };
    Ok(match op {
        '+' => typed(map(x, y, len, i64::wrapping_add)?),
        '-' => typed(map(x, y, len, i64::wrapping_sub)?),
        '*' => typed(map(x, y, len, i64::wrapping_mul)?),
        // An integer remainder by zero is null.
        'm' => typed(nonzero_divisors(
            map(
                x,
                y,
                len,
                |a, b| if b == 0 { 0 } else { a.wrapping_rem_euclid(b) },
            )?,
            y,
        )),
        '&' => typed(map(x, y, len, i64::min)?),
        '|' => typed(either(x, y, len, i64::max)?),
        '^' => typed(fill(x, y, len)),
        '=' => Numbers::Bool(compare(x, y, len, |a, b| a == b, |a, b| !a && !b)),
        '<' => Numbers::Bool(compare(x, y, len, |a, b| a < b, |a, b| !a && b)),
        '>' => Numbers::Bool(compare(x, y, len, |a, b| a > b, |a, b| a && !b)),
        _ => unreachable!("dyad accepts only elementwise primitives"),
    })
}

/// Apply a unary numeric primitive (`-`, `%`, `_`, or `~`) to each element.
pub(crate) fn monad(op: char, x: &Numbers) -> Option<Numbers> {
    Some(match (op, x) {
        ('-', Numbers::Float(a)) => Numbers::Float(unary(a, |v: f64| -v)),
        ('-', _) => Numbers::Int(unary(&integers(x)?, i64::wrapping_neg)),
        ('%', _) => Numbers::Float(unary(&floats(x), |v: f64| 1.0 / v)),
        ('_', Numbers::Float(a)) => Numbers::Int(unary(a, |v: f64| {
            if v == f64::NEG_INFINITY {
                -i64::MAX
            } else {
                v.floor() as i64
            }
        })),
        ('_', _) => x.clone(),
        // Not is false for nulls rather than null.
        ('~', _) => {
            let zero = |i| match x {
                Numbers::Bool(a) => a.is_valid(i) && !a.value(i),
                Numbers::Int(a) => a.is_valid(i) && a.value(i) == 0,
                Numbers::Float(a) => a.is_valid(i) && a.value(i) == 0.0,
            };
            Numbers::Bool(BooleanArray::new(
                BooleanBuffer::collect_bool(x.len(), zero),
                None,
            ))
        }
        _ => return None,
    })
}

/// Combine elements where both sides are valid; any null gives null.
fn map<T: ArrowPrimitiveType>(
    x: &Column<T>,
    y: &Column<T>,
    len: usize,
    f: impl Fn(T::Native, T::Native) -> T::Native,
) -> Result<PrimitiveArray<T>> {
    Ok(match (x, y) {
        (Column::Vector(a), Column::Vector(b)) => binary(a, b, f).map_err(|e| e.to_string())?,
        (Column::Vector(a), Column::Scalar(Some(s))) => unary(a, |v| f(v, *s)),
        (Column::Scalar(Some(s)), Column::Vector(b)) => unary(b, |v| f(*s, v)),
        (Column::Scalar(Some(s)), Column::Scalar(Some(t))) => {
            PrimitiveArray::from_value(f(*s, *t), len)
        }
        _ => PrimitiveArray::new_null(len),
    })
}

/// Combine elements where both sides are valid; otherwise keep the valid side.
fn either<T: ArrowPrimitiveType>(
    x: &Column<T>,
    y: &Column<T>,
    len: usize,
    f: impl Fn(T::Native, T::Native) -> T::Native,
) -> Result<PrimitiveArray<T>> {
    if !x.has_nulls() && !y.has_nulls() {
        return map(x, y, len, f);
    }
    Ok((0..len)
        .map(|i| match (x.get(i), y.get(i)) {
            (Some(a), Some(b)) => Some(f(a, b)),
            (a, b) => a.or(b),
        })
        .collect())
}

/// Each element of `y`, or of `x` where `y` is null.
fn fill<T: ArrowPrimitiveType>(x: &Column<T>, y: &Column<T>, len: usize) -> PrimitiveArray<T> {
    match y {
        Column::Vector(b) if b.null_count() == 0 => b.clone(),
        Column::Scalar(Some(t)) => PrimitiveArray::from_value(*t, len),
        _ => (0..len).map(|i| y.get(i).or_else(|| x.get(i))).collect(),
    }
}

/// Compare elements with `f`; where either is null, `null` decides from
/// whether each side is valid.
fn compare<T: ArrowPrimitiveType>(
    x: &Column<T>,
    y: &Column<T>,
    len: usize,
    f: impl Fn(T::Native, T::Native) -> bool,
    null: impl Fn(bool, bool) -> bool,
) -> BooleanArray {
    let values = match (x, y) {
        _ if x.has_nulls() || y.has_nulls() => {
            BooleanBuffer::collect_bool(len, |i| match (x.get(i), y.get(i)) {
                (Some(a), Some(b)) => f(a, b),
                (a, b) => null(a.is_some(), b.is_some()),
            })
        }
        (Column::Vector(a), Column::Vector(b)) => {
            let (a, b) = (a.values(), b.values());
            BooleanBuffer::collect_bool(len, |i| f(a[i], b[i]))
        }
        (Column::Vector(a), Column::Scalar(Some(t))) => {
            let a = a.values();
            BooleanBuffer::collect_bool(len, |i| f(a[i], *t))
        }
        (Column::Scalar(Some(s)), Column::Vector(b)) => {
            let b = b.values();
            BooleanBuffer::collect_bool(len, |i| f(*s, b[i]))
        }
        _ => BooleanBuffer::collect_bool(len, |i| match (x.get(i), y.get(i)) {
            (Some(a), Some(b)) => f(a, b),
            (a, b) => null(a.is_some(), b.is_some()),
        }),
    };
    BooleanArray::new(values, None)
}

/// Float ordering as pliq compares numbers.
fn order(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or_else(|| a.total_cmp(&b))
}

/// Undefined float results become nulls.
fn defined(a: Float64Array) -> Float64Array {
    let values = a.values();
    if !values.iter().any(|v| v.is_nan()) {
        return a;
    }
    let valid = NullBuffer::new(BooleanBuffer::collect_bool(a.len(), |i| {
        !values[i].is_nan()
    }));
    Float64Array::new(values.clone(), NullBuffer::union(a.nulls(), Some(&valid)))
}

fn booleans(a: &Int64Array) -> BooleanArray {
    let values = a.values();
    BooleanArray::new(
        BooleanBuffer::collect_bool(a.len(), |i| values[i] != 0),
        a.nulls().cloned(),
    )
}

fn nonzero_divisors(result: Int64Array, divisors: &Column<Int64Type>) -> Int64Array {
    let valid = match divisors {
        Column::Scalar(Some(0)) => return Int64Array::new_null(result.len()),
        Column::Vector(b) if b.values().contains(&0) => {
            let b = b.values();
            NullBuffer::new(BooleanBuffer::collect_bool(result.len(), |i| b[i] != 0))
        }
        _ => return result,
    };
    let nulls = NullBuffer::union(result.nulls(), Some(&valid));
    Int64Array::new(result.values().clone(), nulls)
}
