//! Arrow storage for homogeneous numeric vectors with explicit validity.
//! Arithmetic continues to use pliq's Number rules.
use std::sync::Arc;

use arrow_array::{Array as _, ArrayRef, BooleanArray, Float64Array, Int64Array};

use crate::{Number, Result};

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
    pub(crate) fn binary(&self, other: &Self, op: char) -> Option<Result<Self>> {
        use arrow_arith::arity::binary;
        let result = match (self, other) {
            (Self::Int(a), Self::Int(b)) if matches!(op, '+' | '-' | '*') => {
                let operation = match op {
                    '+' => i64::wrapping_add,
                    '-' => i64::wrapping_sub,
                    _ => i64::wrapping_mul,
                };
                binary(a, b, operation).map(Self::Int)
            }
            (Self::Float(a), Self::Float(b)) if matches!(op, '+' | '-' | '*' | '%') => {
                binary(a, b, |x, y| match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    _ => x / y,
                })
                .map(|a: Float64Array| Self::normalize_floats(a))
            }
            _ => return None,
        };
        Some(result.map_err(|e| e.to_string()))
    }
    fn normalize_floats(array: Float64Array) -> Self {
        if array.iter().flatten().any(f64::is_nan) {
            Self::Float(Float64Array::from_iter(
                array.iter().map(|n| n.filter(|n| !n.is_nan())),
            ))
        } else {
            Self::Float(array)
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
            return Ok(Self::normalize_floats(a.clone()));
        }
        Err(format!("unsupported Arrow type: {}", array.data_type()))
    }
}
