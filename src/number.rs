use crate::Result;
use std::{cmp::Ordering, fmt, str::FromStr};

pub(crate) const COMPARISON_TOLERANCE: f64 = 1.0 / ((1u64 << 43) as f64);

/// Native numbers. Integer arithmetic wraps identically in debug and release;
/// floating arithmetic uses IEEE 754 binary64. Missing values are represented by Value::Null.
#[derive(Clone, Copy, Debug)]
pub enum Number {
    Bool(bool),
    Int(i64),
    Float(f64),
}

impl From<i64> for Number {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}
impl From<f64> for Number {
    fn from(n: f64) -> Self {
        Self::Float(n)
    }
}
impl From<bool> for Number {
    fn from(n: bool) -> Self {
        Self::Bool(n)
    }
}

impl Number {
    pub const fn integer(n: i64) -> Self {
        Self::Int(n)
    }
    pub const fn float(n: f64) -> Self {
        Self::Float(n)
    }
    pub fn as_i64(self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(n),
            Self::Bool(b) => Some(i64::from(b)),
            Self::Float(_) => None,
        }
    }
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Float(n) => n,
            Self::Int(n) => n as f64,
            Self::Bool(b) => u8::from(b) as f64,
        }
    }
    pub fn type_code(self) -> i64 {
        match self {
            Self::Bool(_) => 1,
            Self::Int(_) => 7,
            Self::Float(_) => 9,
        }
    }
    pub(crate) fn length(n: usize) -> Self {
        Self::Int(i64::try_from(n).expect("array length fits i64"))
    }
    pub(crate) fn is_zero(&self) -> bool {
        match self {
            Self::Int(n) => *n == 0,
            Self::Float(n) => *n == 0.0,
            Self::Bool(b) => !b,
        }
    }
    pub(crate) fn is_integer(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Bool(_))
    }
    pub(crate) fn to_isize(self) -> Option<isize> {
        self.as_i64().and_then(|n| isize::try_from(n).ok())
    }
    pub(crate) fn negated(&self) -> Result<Self> {
        Ok(match self {
            Self::Float(n) => Self::Float(-n),
            _ => Self::Int(self.as_i64().unwrap().wrapping_neg()),
        })
    }
    pub(crate) fn floor(&self) -> Result<Self> {
        Ok(match self {
            Self::Float(n) if *n == f64::NEG_INFINITY => Self::Int(-i64::MAX),
            Self::Float(n) => Self::Int(n.floor() as i64),
            _ => *self,
        })
    }
    fn binary(
        &self,
        other: &Self,
        integer: fn(i64, i64) -> i64,
        float: fn(f64, f64) -> f64,
    ) -> Self {
        if matches!(self, Self::Float(_)) || matches!(other, Self::Float(_)) {
            return Self::Float(float(self.as_f64(), other.as_f64()));
        }
        Self::Int(integer(self.as_i64().unwrap(), other.as_i64().unwrap()))
    }
    pub(crate) fn add(&self, other: &Self) -> Result<Self> {
        Ok(self.binary(other, i64::wrapping_add, |a, b| a + b))
    }
    pub(crate) fn subtract(&self, other: &Self) -> Result<Self> {
        Ok(self.binary(other, i64::wrapping_sub, |a, b| a - b))
    }
    pub(crate) fn multiply(&self, other: &Self) -> Result<Self> {
        Ok(self.binary(other, i64::wrapping_mul, |a, b| a * b))
    }
    pub(crate) fn divide(&self, other: &Self) -> Result<Self> {
        Ok(Self::Float(self.as_f64() / other.as_f64()))
    }
    pub(crate) fn remainder(&self, other: &Self) -> Result<Self> {
        if matches!(
            (self, other),
            (Self::Int(_) | Self::Bool(_), Self::Int(_) | Self::Bool(_))
        ) {
            let a = self.as_i64().unwrap();
            let b = other.as_i64().unwrap();
            if b == 0 {
                return Err("integer remainder by zero".into());
            }
            return Ok(Self::Int(a.wrapping_rem_euclid(b)));
        }
        Ok(Self::Float(self.as_f64().rem_euclid(other.as_f64())))
    }
    pub(crate) fn minimum(&self, other: &Self) -> Self {
        let value = *self.min(other);
        if matches!(self, Self::Float(_)) || matches!(other, Self::Float(_)) {
            Self::Float(value.as_f64())
        } else {
            value
        }
    }
    pub(crate) fn maximum(&self, other: &Self) -> Self {
        let value = *self.max(other);
        if matches!(self, Self::Float(_)) || matches!(other, Self::Float(_)) {
            Self::Float(value.as_f64())
        } else {
            value
        }
    }
    /// Decimal-place rounding of an IEEE float; it does not provide decimal arithmetic.
    pub fn round(&self, places: u32) -> Result<Self> {
        if places > 308 {
            return Err("round places must be between 0 and 308".into());
        }
        if !matches!(self, Self::Float(_)) {
            return Ok(*self);
        }
        let n = self.as_f64();
        let factor = 10f64.powi(places as i32);
        let scaled = n * factor;
        Ok(Self::Float(if scaled.is_finite() {
            scaled.round_ties_even() / factor
        } else {
            n
        }))
    }
    pub(crate) fn power(&self, other: &Self) -> Result<Self> {
        Ok(Self::Float(self.as_f64().powf(other.as_f64())))
    }
    /// Numeric comparison tolerance is applied to language comparisons.
    pub(crate) fn equivalent(&self, other: &Self) -> bool {
        if let (Some(a), Some(b)) = (self.as_i64(), other.as_i64()) {
            return a == b;
        }
        equivalent_floats(self.as_f64(), other.as_f64())
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.type_code() == other.type_code() && self.equivalent(other)
    }
}
/// Tolerant float equality for language comparisons.
pub(crate) fn equivalent_floats(a: f64, b: f64) -> bool {
    a == b
        || (a.is_finite()
            && b.is_finite()
            && (a - b).abs() <= COMPARISON_TOLERANCE * a.abs().max(b.abs()))
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Number {}
impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        if let (Some(a), Some(b)) = (self.as_i64(), other.as_i64()) {
            return a.cmp(&b);
        }
        let (a, b) = (self.as_f64(), other.as_f64());
        a.partial_cmp(&b).unwrap_or_else(|| a.total_cmp(&b))
    }
}
impl FromStr for Number {
    type Err = String;
    fn from_str(source: &str) -> Result<Self> {
        match source {
            "0W" => return Ok(Self::Int(i64::MAX)),
            "-0W" => return Ok(Self::Int(-i64::MAX)),
            "0w" => return Ok(Self::Float(f64::INFINITY)),
            "-0w" => return Ok(Self::Float(f64::NEG_INFINITY)),
            _ => {}
        }
        if source.contains(['.', 'e', 'E']) {
            source
                .parse::<f64>()
                .map(Self::Float)
                .map_err(|_| "invalid floating-point number".into())
        } else {
            source
                .parse::<i64>()
                .map(Self::Int)
                .map_err(|_| "invalid integer or i64 literal overflow".into())
        }
    }
}
impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{}", u8::from(*b)),
            Self::Int(i64::MAX) => f.write_str("0W"),
            Self::Int(n) if *n == -i64::MAX => f.write_str("-0W"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(n) if n.is_nan() => f.write_str("nan"),
            Self::Float(n) if *n == f64::INFINITY => f.write_str("0w"),
            Self::Float(n) if *n == f64::NEG_INFINITY => f.write_str("-0w"),
            Self::Float(n) => write!(f, "{n}"),
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/number_reference.rs"]
mod tests;
