//! Primitives that require structured arguments rather than numeric broadcasting.
use crate::eval::{Evaluator, boolean, count, index_into, items, number};
use crate::value::FunctionKind;
use crate::{Array, Dictionary, Number, Result, Value};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) fn builtins() -> HashMap<String, Value> {
    let mut env = HashMap::new();
    env.insert("round".into(), Value::function(FunctionKind::Round));
    for name in [
        "mod",
        "pow",
        "asc",
        "desc",
        "neg",
        "not",
        "null",
        "first",
        "count",
        "til",
        "key",
        "value",
        "reverse",
        "distinct",
        "where",
        "group",
        "flip",
        "type",
        "string",
        "floor",
        "reciprocal",
        "enlist",
        "sum",
        "prd",
        "sums",
        "prds",
        "min",
        "max",
        "mins",
        "maxs",
    ] {
        env.insert(name.into(), Value::function(FunctionKind::Native(name)));
    }
    env
}

// Type codes for native numbers.
pub(crate) fn type_code(v: &Value) -> i64 {
    match v {
        Value::Number(n) => -n.type_code(),
        Value::Null => 0,
        Value::Char(_) => -10,
        Value::String(_) => 10,
        Value::Symbol(_) => -11,
        Value::Dictionary(_) => 99,
        Value::Table(_) => 98,
        Value::Array(a) => a.type_code(),
        Value::Function(f) => match &f.kind {
            FunctionKind::User { .. } => 100,
            FunctionKind::Monadic(_) | FunctionKind::Native(_) | FunctionKind::Round => 101,
            FunctionKind::Verb(':') => 101,
            FunctionKind::Verb(_) => 102,
            FunctionKind::Projection(..) => 104,
            FunctionKind::Composition(..) => 105,
            FunctionKind::Derived(c, _) => match c {
                '\'' => 106,
                '/' => 107,
                '\\' => 108,
                'P' => 109,
                'R' => 110,
                'L' => 111,
                _ => 0,
            },
        },
    }
}

impl Evaluator {
    pub(crate) fn native(&mut self, name: &str, args: &[Value]) -> Result<Value> {
        if let [x, y] = args {
            return match name {
                "mod" => self.dyad('m', x, y),
                "pow" => self.dyad('p', x, y),
                _ => Err(format!("{name} expects one argument")),
            };
        }
        let [y] = args else {
            return Err(format!("invalid number of arguments to {name}"));
        };
        let op = match name {
            "neg" => '-',
            "not" => '~',
            "null" => '^',
            "first" => '*',
            "count" => '#',
            "til" | "key" => '!',
            "value" => '.',
            "reverse" => '|',
            "distinct" => '?',
            "where" => '&',
            "group" => '=',
            "flip" => '+',
            "type" => '@',
            "string" => '$',
            "floor" => '_',
            "reciprocal" => '%',
            "enlist" => ',',
            "asc" | "desc" => {
                if !matches!(y, Value::Array(_)) {
                    return Ok(y.clone());
                }
                let indices = self.monad(if name == "asc" { '<' } else { '>' }, y)?;
                return index_into(y, &indices);
            }
            "sum" | "prd" | "sums" | "prds" | "min" | "max" | "mins" | "maxs" => {
                let op = match name {
                    "sum" | "sums" => '+',
                    "prd" | "prds" => '*',
                    "min" | "mins" => '&',
                    _ => '|',
                };
                return self.apply(
                    &Value::function(FunctionKind::Derived(
                        if name.ends_with('s') { '\\' } else { '/' },
                        Value::function(FunctionKind::Verb(op)),
                    )),
                    args,
                );
            }
            _ => return Err(format!("{name} expects two arguments")),
        };
        self.monad(op, y)
    }

    pub(crate) fn flip(&mut self, y: &Value) -> Result<Value> {
        let Value::Array(rows) = y else {
            return Err("flip requires a list of rows".into());
        };
        if rows.is_empty() {
            return Ok(y.clone());
        }
        let width = rows
            .iter()
            .find_map(|row| match row {
                Value::Array(a) => Some(a.len()),
                Value::String(s) => Some(s.len()),
                _ => None,
            })
            .ok_or("flip requires at least one row array")?;
        for row in rows.iter() {
            let length = match row {
                Value::Array(a) => Some(a.len()),
                Value::String(s) => Some(s.len()),
                _ => None,
            };
            if length.is_some_and(|n| n != width) {
                return Err("length mismatch in flip".into());
            }
        }
        Value::array(
            (0..width)
                .map(|i| {
                    Value::array(rows.iter().map(|row| match row {
                        Value::Array(a) => a.get(i).unwrap(),
                        Value::String(s) => Value::Char(s.get(i).unwrap()),
                        scalar => scalar,
                    }))
                })
                .collect::<Result<Vec<_>>>()?,
        )
    }

    pub(crate) fn nulls(&mut self, y: &Value) -> Result<Value> {
        match y {
            Value::Array(a) => a
                .iter()
                .map(|v| self.nulls(&v))
                .collect::<Result<Vec<_>>>()
                .and_then(|values| {
                    if a.is_mixed() {
                        Ok(Array::dictionary_values(values))
                    } else {
                        Array::new(values)
                    }
                })
                .map(Value::from_array),
            Value::Dictionary(d) => {
                Dictionary::from_arrays(d.keys(), items(&self.nulls(&Value::Array(d.values()))?))
                    .map(Value::Dictionary)
            }
            _ => Ok(boolean(y.is_null())),
        }
    }

    pub(crate) fn group(&mut self, y: &Value) -> Result<Value> {
        if let Value::Dictionary(d) = y {
            let Value::Dictionary(groups) = self.group(&Value::Array(d.values()))? else {
                unreachable!()
            };
            let values = groups
                .values()
                .iter()
                .map(|v| index_into(&Value::Array(d.keys()), &v))
                .collect::<Result<Vec<_>>>()
                .and_then(Array::new)?;
            return Dictionary::from_arrays(groups.keys(), values).map(Value::Dictionary);
        }
        let Value::Array(a) = y else {
            return Err("group requires an array".into());
        };
        let mut keys: Vec<Value> = Vec::new();
        let mut groups: Vec<Vec<Value>> = Vec::new();
        for (i, v) in a.iter().enumerate() {
            let group = keys.iter().position(|key| key.same(&v)).unwrap_or_else(|| {
                keys.push(v);
                groups.push(Vec::new());
                keys.len() - 1
            });
            groups[group].push(Value::Number(Number::length(i)));
        }
        Dictionary::from_arrays(
            Array::new(keys)?,
            Array::new(
                groups
                    .into_iter()
                    .map(Value::array)
                    .collect::<Result<Vec<_>>>()?,
            )?,
        )
        .map(Value::Dictionary)
    }

    pub(crate) fn stringify(&mut self, y: &Value) -> Result<Value> {
        match y {
            Value::Array(a) => a
                .iter()
                .map(|v| self.stringify(&v))
                .collect::<Result<Vec<_>>>()
                .and_then(|values| {
                    if a.is_mixed() {
                        Ok(Array::dictionary_values(values))
                    } else {
                        Array::new(values)
                    }
                })
                .map(Value::from_array),
            Value::Dictionary(d) => Dictionary::from_arrays(
                d.keys(),
                items(&self.stringify(&Value::Array(d.values()))?),
            )
            .map(Value::Dictionary),
            Value::Symbol(s) => Ok(Value::text(s.as_bytes())),
            Value::Char(c) => Ok(Value::text([*c])),
            _ => Ok(Value::text(y.to_string())),
        }
    }

    pub(crate) fn cast(&mut self, x: &Value, y: &Value) -> Result<Value> {
        if let Value::Symbol(s) = x
            && s.is_empty()
        {
            return y
                .as_text()
                .map(|s| Value::Symbol(Rc::from(s)))
                .ok_or_else(|| "symbol cast requires text".into());
        }
        if let Value::Number(_) = x
            && let Some(text) = y.as_text()
        {
            let n = count(x)?;
            let width = n.unsigned_abs();
            let bytes = text.as_bytes();
            let content = &bytes[..bytes.len().min(width)];
            let padding = std::iter::repeat_n(b' ', width.saturating_sub(content.len()));
            return Ok(if n >= 0 {
                Value::text(content.iter().copied().chain(padding).collect::<Vec<_>>())
            } else {
                Value::text(padding.chain(content.iter().copied()).collect::<Vec<_>>())
            });
        }
        if let Value::Char(code) = x {
            if code.is_ascii_uppercase() {
                let text = y.as_text().ok_or("parse cast requires text")?;
                return match code {
                    b'S' => Ok(Value::Symbol(Rc::from(text))),
                    b'B' => Ok(boolean(matches!(
                        text.trim_matches([' ', '\t', '\r']),
                        "1" | "t" | "T" | "y" | "Y"
                    ))),
                    b'J' | b'I' | b'H' => {
                        let text = text.trim_matches([' ', '\t', '\r']);
                        let n = match text {
                            "0W" | "0w" => i64::MAX,
                            "-0W" | "-0w" => -i64::MAX,
                            _ => match text.parse::<i64>() {
                                Ok(n) => n,
                                Err(_) => return Ok(Value::Null),
                            },
                        };
                        Ok(Value::Number(Number::Int(n)))
                    }
                    b'F' | b'E' => text
                        .parse::<f64>()
                        .map(|n| Value::from_number(Number::Float(n)))
                        .map_err(|_| "invalid float text".into()),
                    _ => Err("unsupported cast type".into()),
                };
            }
            if let Some(chars) = y.string_items() {
                return self.cast(x, &Value::Array(chars));
            }
            if let Value::Array(a) = y {
                return a
                    .iter()
                    .map(|v| self.cast(x, &v))
                    .collect::<Result<Vec<_>>>()
                    .and_then(|values| match code {
                        b'b' => Array::numeric(values, 1),
                        b'j' | b'i' | b'h' => Array::numeric(values, 7),
                        b'f' | b'e' => Array::numeric(values, 9),
                        _ => Array::new(values),
                    })
                    .map(Value::from_array);
            }
            return match (code, y) {
                (b'f' | b'e' | b'j' | b'i' | b'h' | b'b' | b'c' | b's', Value::Null) => {
                    Ok(Value::Null)
                }
                (b'f' | b'e', Value::Number(n)) => Ok(Value::Number(Number::Float(n.as_f64()))),
                (b'j' | b'i' | b'h', Value::Number(n)) => Ok(Value::Number(Number::Int(
                    n.as_i64()
                        .unwrap_or_else(|| n.as_f64().round_ties_even() as i64),
                ))),
                (b'b', Value::Number(n)) => Ok(boolean(!n.is_zero())),
                (b'c', Value::Number(n)) => Ok(Value::Char(
                    u8::try_from(n.to_isize().ok_or("character cast requires an integer")?)
                        .map_err(|_| "character out of range")?,
                )),
                (b'f', Value::Char(c)) => Ok(Value::Number(Number::Float(f64::from(*c)))),
                (b'j' | b'i' | b'h', Value::Char(c)) => {
                    Ok(Value::Number(Number::integer(i64::from(*c))))
                }
                (b's', Value::Symbol(_)) => Ok(y.clone()),
                _ => Err("unsupported cast type".into()),
            };
        }
        if let Value::Number(code) = x {
            let code = match code.to_isize() {
                Some(1) => b'b',
                Some(5) => b'h',
                Some(6) => b'i',
                Some(7) => b'j',
                Some(8) => b'e',
                Some(9) => b'f',
                Some(10) => b'c',
                _ => return Err("$ requires a supported cast or text padding argument".into()),
            };
            return self.cast(&Value::Char(code), y);
        }
        if matches!((x, y), (Value::Array(_), Value::Array(_))) {
            return self.matrix_product(x, y);
        }
        Err("$ requires a supported cast or text padding argument".into())
    }

    fn matrix_product(&mut self, x: &Value, y: &Value) -> Result<Value> {
        let Value::Array(xs) = x else {
            return Err("matrix product requires arrays".into());
        };
        if xs
            .iter()
            .all(|v| matches!(v, Value::Number(_) | Value::Null))
        {
            let product = self.dyad('*', x, y)?;
            return self.apply(
                &Value::function(FunctionKind::Derived(
                    '/',
                    Value::function(FunctionKind::Verb('+')),
                )),
                &[product],
            );
        }
        xs.iter()
            .map(|row| self.matrix_product(&row, y))
            .collect::<Result<Vec<_>>>()
            .and_then(Array::new)
            .map(Value::from_array)
    }

    pub(crate) fn reshape(&mut self, shape: &Array, y: &Value) -> Result<Value> {
        let dims = shape
            .iter()
            .map(|v| {
                let n = count(&v)?;
                usize::try_from(n).map_err(|_| "reshape dimensions must be nonnegative".into())
            })
            .collect::<Result<Vec<_>>>()?;
        let length = dims.iter().try_fold(1usize, |a, b| {
            a.checked_mul(*b).ok_or("reshape size overflow")
        })?;
        let flat = self.dyad('#', &Value::Number(Number::length(length)), y)?;
        fn build(dims: &[usize], flat: &Array, offset: &mut usize) -> Result<Value> {
            if let Some((n, rest)) = dims.split_first() {
                let values = (0..*n)
                    .map(|_| build(rest, flat, offset))
                    .collect::<Result<Vec<_>>>()?;
                if flat.is_mixed() {
                    Ok(Value::Array(Array::dictionary_values(values)))
                } else {
                    Value::array(values)
                }
            } else {
                let v = flat.get(*offset).unwrap();
                *offset += 1;
                Ok(v)
            }
        }
        build(&dims, &items(&flat), &mut 0)
    }

    pub(crate) fn cut_or_delete(&mut self, x: &Value, y: &Value) -> Result<Value> {
        if let Value::Dictionary(d) = y {
            let excluded = items(x);
            let (keys, values): (Vec<_>, Vec<_>) = d
                .keys()
                .iter()
                .zip(d.values().iter())
                .filter(|(k, _)| !excluded.iter().any(|e| e.same(k)))
                .unzip();
            return Dictionary::from_values(Array::new(keys)?, values).map(Value::Dictionary);
        }
        let Value::Array(a) = x else {
            return Err("cut requires an index list".into());
        };
        if let Value::Number(n) = y {
            let i = n
                .to_isize()
                .filter(|i| *i >= 0 && (*i as usize) < a.len())
                .ok_or("delete index out of bounds")? as usize;
            return Value::array(
                a.iter()
                    .enumerate()
                    .filter_map(|(j, v)| (i != j).then_some(v)),
            );
        }
        let Value::Array(data) = y else {
            return Err("cut requires an array".into());
        };
        let indices = a
            .iter()
            .map(|v| usize::try_from(count(&v)?).map_err(|_| "cut index out of bounds".into()))
            .collect::<Result<Vec<_>>>()?;
        if indices.windows(2).any(|w| w[0] > w[1])
            || indices.last().is_some_and(|i| *i > data.len())
        {
            return Err("cut indices must be ordered and in bounds".into());
        }
        Value::array(indices.iter().enumerate().map(|(i, start)| {
            Value::from_array(data.slice(*start, indices.get(i + 1).copied().unwrap_or(data.len())))
        }))
    }

    pub(crate) fn dictionary_dyad(&mut self, op: char, x: &Value, y: &Value) -> Result<Value> {
        match (x, y) {
            (Value::Dictionary(a), Value::Dictionary(b)) => {
                let mut keys = a.keys().iter().collect::<Vec<_>>();
                for key in b.keys().iter() {
                    if !keys.iter().any(|k| k.same(&key)) {
                        keys.push(key);
                    }
                }
                let values = keys
                    .iter()
                    .map(|k| match (a.get_value(k), b.get_value(k)) {
                        (Some(x), Some(y)) => self.dyad(op, &x, &y),
                        (Some(x), None) => Ok(x),
                        (None, Some(y)) => Ok(y),
                        _ => unreachable!(),
                    })
                    .collect::<Result<Vec<_>>>()?;
                Dictionary::from_values(Array::new(keys)?, values).map(Value::Dictionary)
            }
            (Value::Dictionary(d), y) => {
                let values = self.dyad(op, &Value::Array(d.values()), y)?;
                Dictionary::from_arrays(d.keys(), items(&values)).map(Value::Dictionary)
            }
            (x, Value::Dictionary(d)) => {
                let values = self.dyad(op, x, &Value::Array(d.values()))?;
                Dictionary::from_arrays(d.keys(), items(&values)).map(Value::Dictionary)
            }
            _ => unreachable!(),
        }
    }

    pub(crate) fn iterate(&mut self, adverb: char, f: &Value, args: &[Value]) -> Result<Value> {
        match (adverb, args) {
            ('R', [x, y]) if !matches!(y, Value::Array(_) | Value::String(_)) => {
                self.apply(f, &[x.clone(), y.clone()])
            }
            ('L', [x, y]) if !matches!(x, Value::Array(_) | Value::String(_)) => {
                self.apply(f, &[x.clone(), y.clone()])
            }
            ('R', [x, y]) => items(y)
                .iter()
                .map(|y| self.apply(f, &[x.clone(), y]))
                .collect::<Result<Vec<_>>>()
                .and_then(Array::new)
                .map(Value::from_array),
            ('L', [x, y]) => items(x)
                .iter()
                .map(|x| self.apply(f, &[x, y.clone()]))
                .collect::<Result<Vec<_>>>()
                .and_then(Array::new)
                .map(Value::from_array),
            ('P', [y]) | ('P', [_, y]) => {
                let xs = items(y);
                let mut previous = if args.len() == 2 {
                    args[0].clone()
                } else {
                    match f {
                        Value::Function(f) => match f.kind {
                            FunctionKind::Verb('*' | '%') => Value::Number(Number::integer(1)),
                            FunctionKind::Verb('-' | '+') => Value::Number(Number::integer(0)),
                            _ => xs.get(0).unwrap_or(Value::Null),
                        },
                        _ => xs.get(0).unwrap_or(Value::Null),
                    }
                };
                let mut output = Vec::new();
                for current in xs.iter() {
                    output.push(self.apply(f, &[current.clone(), previous])?);
                    previous = current;
                }
                Ok(if matches!(y, Value::Array(_)) {
                    Value::array(output)?
                } else {
                    output.pop().unwrap()
                })
            }
            _ => Err("iterator has invalid arguments".into()),
        }
    }

    pub(crate) fn choose(&mut self, c: &Value, x: &Value, y: &Value) -> Result<Value> {
        if let Value::Array(conditions) = c {
            for v in [x, y] {
                if let Value::Array(a) = v
                    && a.len() != conditions.len()
                {
                    return Err("length mismatch in vector conditional".into());
                }
            }
            return conditions
                .iter()
                .enumerate()
                .map(|(i, c)| self.choose(&c, &element_or_scalar(x, i), &element_or_scalar(y, i)))
                .collect::<Result<Vec<_>>>()
                .and_then(Array::new)
                .map(Value::from_array);
        }
        let n = number(c)?;
        if *n != Number::integer(0) && *n != Number::integer(1) {
            return Err("vector conditional requires booleans".into());
        }
        Ok(if n.is_zero() { y.clone() } else { x.clone() })
    }

    pub(crate) fn amend_or_trap(
        &mut self,
        op: char,
        x: &Value,
        i: &Value,
        f: &Value,
        y: Option<&Value>,
    ) -> Result<Value> {
        if matches!(x, Value::Function(_)) && y.is_none() {
            let result = if op == '@' {
                self.apply(x, std::slice::from_ref(i))
            } else {
                let Value::Array(args) = i else {
                    return Err("trap requires an argument list".into());
                };
                self.apply(x, &args.iter().collect::<Vec<_>>())
            };
            if self.returning.is_some() {
                return result;
            }
            return result.or_else(|e| {
                if matches!(f, Value::Function(_)) {
                    self.apply(f, &[Value::text(e)])
                } else {
                    Ok(f.clone())
                }
            });
        }
        // Functional amend returns a copy. Shared mutation is confined to assignment.
        let result = x.detached();
        let indices = if op == '.' {
            let Value::Array(a) = i else {
                return Err("deep amend requires a path list".into());
            };
            a.iter().collect::<Vec<_>>()
        } else {
            vec![i.clone()]
        };
        if indices.is_empty() {
            return self.apply(
                f,
                &match y {
                    Some(y) => vec![result, y.clone()],
                    None => vec![result],
                },
            );
        }
        self.amend_path(&result, &indices, f, y)?;
        Ok(result)
    }

    pub(crate) fn amend_path(
        &mut self,
        target: &Value,
        path: &[Value],
        f: &Value,
        y: Option<&Value>,
    ) -> Result<()> {
        let (selector, rest) = path.split_first().ok_or("empty amend path")?;
        if let Value::Function(identity) = selector
            && matches!(identity.kind, FunctionKind::Verb(':'))
        {
            let indices = match target {
                Value::Array(a) => {
                    Value::array((0..a.len()).map(|i| Value::Number(Number::length(i))))?
                }
                Value::String(s) => {
                    Value::array((0..s.len()).map(|i| Value::Number(Number::length(i))))?
                }
                Value::Dictionary(d) => Value::Array(d.keys()),
                Value::Table(t) => Value::Array(t.names()),
                _ => return Err("amend requires an array or dictionary".into()),
            };
            let mut all_path = vec![indices];
            all_path.extend_from_slice(rest);
            return self.amend_path(target, &all_path, f, y);
        }
        if let Value::Array(indices) = selector
            && !matches!(target, Value::Dictionary(d) if d.key_is_atom(selector))
        {
            let text_values = y.and_then(Value::string_items).map(Value::Array);
            let y = text_values.as_ref().or(y);
            if let Some(Value::Array(values)) = y
                && values.len() != indices.len()
            {
                return Err("amend length mismatch".into());
            }
            for (i, index) in indices.iter().enumerate() {
                let mut child_path = vec![index];
                child_path.extend_from_slice(rest);
                let value = y.map(|y| element_or_scalar(y, i));
                self.amend_path(target, &child_path, f, value.as_ref())?;
            }
        } else if rest.is_empty() {
            let old = index_into(target, selector)?;
            let value = if let Some(y) = y {
                self.apply(f, &[old, y.clone()])?
            } else if let Value::Function(function) = f
                && let FunctionKind::Verb(op) = function.kind
            {
                self.monad(op, &old)?
            } else {
                self.apply(f, &[old])?
            };
            target.update(std::slice::from_ref(selector), &value)?;
        } else {
            self.amend_path(&index_into(target, selector)?, rest, f, y)?;
        }
        Ok(())
    }

    pub(crate) fn unary_fold(&mut self, f: &Value, args: &[Value], scan: bool) -> Result<Value> {
        let (control, mut current) = match args {
            [x] => (None, x.clone()),
            [control, x] => (Some(control), x.clone()),
            _ => return Err("unary iteration expects one or two arguments".into()),
        };
        let first = current.clone();
        let mut output = if scan {
            vec![current.clone()]
        } else {
            Vec::new()
        };
        let mut remaining = match control {
            Some(Value::Number(_)) => Some(
                usize::try_from(count(control.unwrap())?)
                    .map_err(|_| "iteration count must be nonnegative")?,
            ),
            _ => None,
        };
        loop {
            if let Some(n) = remaining.as_mut() {
                if *n == 0 {
                    break;
                }
                *n -= 1;
            } else if let Some(test) = control {
                let condition = self.apply(test, &[current.clone()])?;
                if number(&condition)?.is_zero() {
                    break;
                }
            }
            let next = self.apply(f, &[current.clone()])?;
            if control.is_none() && (next.same(&current) || next.same(&first)) {
                break;
            }
            current = next;
            if scan {
                output.push(current.clone());
            }
        }
        Ok(if scan { Value::array(output)? } else { current })
    }

    pub(crate) fn roll(&mut self, x: &Value, y: &Value) -> Result<Value> {
        static RANDOM: AtomicU64 = AtomicU64::new(0);
        let mut state = RANDOM.load(Ordering::Relaxed);
        if state == 0 {
            state = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
                | 1;
        }
        let n = count(x)?;
        let bound = match y {
            Value::Number(_) => usize::try_from(count(y)?)
                .ok()
                .filter(|n| *n > 0)
                .ok_or("roll requires a positive bound")?,
            Value::Array(a) => a.len(),
            _ => return Err("roll requires a count or array".into()),
        };
        if bound == 0 && n != 0 {
            return Err("cannot sample an empty array".into());
        }
        if n < 0 && n.unsigned_abs() > bound {
            return Err("deal exceeds population".into());
        }
        let mut selected = std::collections::HashSet::new();
        let mut output = Vec::new();
        for _ in 0..n.unsigned_abs() {
            let i = loop {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                // Reject the incomplete remainder interval instead of biasing small indices.
                let limit = u64::MAX - u64::MAX % bound as u64;
                if state >= limit {
                    continue;
                }
                let i = (state % bound as u64) as usize;
                if n >= 0 || selected.insert(i) {
                    break i;
                }
            };
            output.push(match y {
                Value::Array(a) => a.get(i).unwrap(),
                _ => Value::Number(Number::length(i)),
            });
        }
        RANDOM.store(state, Ordering::Relaxed);
        Value::array(output)
    }
}

fn element_or_scalar(v: &Value, i: usize) -> Value {
    match v {
        Value::Array(a) => a.get(i).unwrap(),
        _ => v.clone(),
    }
}

/// Stable total ordering for grading data, with numeric nulls before numbers.
pub(crate) fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Number(a), Value::Number(b)) => a.cmp(b),
        (Value::Symbol(a), Value::Symbol(b)) => a.cmp(b),
        (Value::Char(a), Value::Char(b)) => a.cmp(b),
        (Value::String(a), Value::String(b)) => a.snapshot().cmp(&b.snapshot()),
        (Value::Array(a), Value::Array(b)) => {
            for (a, b) in a.iter().zip(b.iter()) {
                let order = compare(&a, &b);
                if order != Ordering::Equal {
                    return order;
                }
            }
            a.len().cmp(&b.len())
        }
        _ => type_code(a).abs().cmp(&type_code(b).abs()),
    }
}
