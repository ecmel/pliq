use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet, hash_map::RandomState},
    fmt,
    hash::{BuildHasher, Hash, Hasher},
    rc::Rc,
};

use crate::{MutableString, Number, Result, compile::Proto};

mod display;
mod table;

pub use display::{Console, View};

pub(crate) type Environment = Rc<HashMap<String, Value>>;

#[derive(Clone, Debug)]
pub enum Value {
    Number(Number),
    Null,
    Char(u8),
    String(MutableString),
    /// An atomic name. Clones share text; equality compares its contents.
    Symbol(Rc<str>),
    Array(Array),
    Dictionary(Dictionary),
    Table(Table),
    Function(Rc<Function>),
}

/// Shared mutable named columns, with implicit nulls beyond shorter columns.
#[derive(Clone, Debug)]
pub struct Table(Rc<RefCell<TableColumns>>);

type TableColumns = Vec<(Rc<str>, Array)>;

/// Shared mutable dictionary with ordered keys, including repeated keys.
#[derive(Clone, Debug)]
pub struct Dictionary(Rc<RefCell<DictionaryData>>);

#[derive(Debug)]
struct DictionaryData {
    keys: Array,
    values: Array,
    index: Rc<DictionaryIndex>,
    key_depth: usize,
}

/// Index fingerprints narrow candidates; `Value::same` remains authoritative.
/// Float matching is tolerant and nontransitive, so floats cannot be HashMap keys.
#[derive(Clone, Debug, Default)]
struct DictionaryIndex {
    hasher: RandomState,
    exact: HashMap<u64, Vec<usize>>,
    floats: BTreeMap<(u64, u64), Vec<usize>>,
}

impl DictionaryIndex {
    fn fingerprint(&self, key: &Value) -> (u64, Option<f64>) {
        let mut state = self.hasher.build_hasher();
        let mut float = None;
        hash_key(key, &mut state, &mut float);
        (state.finish(), float)
    }

    fn insert(&mut self, key: &Value, position: usize) {
        let (hash, float) = self.fingerprint(key);
        let positions = match float {
            Some(number) => self.floats.entry((hash, float_order(number))).or_default(),
            None => self.exact.entry(hash).or_default(),
        };
        positions.push(position);
    }

    fn find(&self, keys: &Array, key: &Value) -> Option<usize> {
        let (hash, float) = self.fingerprint(key);
        let first_match = |positions: &[usize]| {
            positions
                .iter()
                .copied()
                .find(|&i| keys.get(i).unwrap().same(key))
        };
        match float {
            None => self
                .exact
                .get(&hash)
                .and_then(|positions| first_match(positions)),
            Some(number) => {
                let (low, high) = if number.is_finite() {
                    // Twice Number::equivalent's relative tolerance safely encloses
                    // both sides, including rounding at zero and finite boundaries.
                    let radius = number.abs() * (2.0 * crate::number::COMPARISON_TOLERANCE);
                    (
                        float_order((number - radius).next_down()),
                        float_order((number + radius).next_up()),
                    )
                } else {
                    let order = float_order(number);
                    (order, order)
                };
                self.floats
                    .range((hash, low)..=(hash, high))
                    .filter_map(|(_, positions)| first_match(positions))
                    .min()
            }
        }
    }
}

fn float_order(number: f64) -> u64 {
    let bits = if number.is_nan() {
        f64::NAN.to_bits()
    } else if number == 0.0 {
        0
    } else {
        number.to_bits()
    };
    if bits >> 63 == 0 {
        bits ^ (1 << 63)
    } else {
        !bits
    }
}

fn key_depth(value: &Value) -> usize {
    match value {
        Value::Array(a) => 1 + a.iter().map(|v| key_depth(&v)).max().unwrap_or(0),
        Value::String(_) => 1,
        _ => 0,
    }
}

// Matching values must share a fingerprint. Container identity and mutable
// function captures are deliberately excluded. The first float supplies a
// range-index discriminator; remaining floats are checked by Value::same.
fn hash_key(value: &Value, state: &mut impl Hasher, float: &mut Option<f64>) {
    match value {
        Value::Null => 0u8.hash(state),
        Value::Number(Number::Bool(b)) => {
            1u8.hash(state);
            b.hash(state);
        }
        Value::Number(Number::Int(n)) => {
            2u8.hash(state);
            n.hash(state);
        }
        Value::Number(Number::Float(n)) => {
            3u8.hash(state);
            float.get_or_insert(*n);
        }
        Value::Char(c) => {
            4u8.hash(state);
            c.hash(state);
        }
        Value::Symbol(s) => {
            5u8.hash(state);
            s.hash(state);
        }
        Value::String(s) => {
            6u8.hash(state);
            s.snapshot().hash(state);
        }
        Value::Array(a) => {
            7u8.hash(state);
            a.len().hash(state);
            for item in a.iter() {
                hash_key(&item, state, float);
            }
        }
        Value::Dictionary(d) => {
            8u8.hash(state);
            let data = d.0.borrow();
            hash_key(&Value::Array(data.keys.clone()), state, float);
            hash_key(&Value::Array(data.values.clone()), state, float);
        }
        Value::Table(t) => {
            10u8.hash(state);
            for (name, column) in t.columns() {
                name.hash(state);
                hash_key(&Value::Array(column), state, float);
            }
        }
        Value::Function(_) => 9u8.hash(state),
    }
}

impl Dictionary {
    pub fn new(keys: Vec<Rc<str>>, values: Array) -> Result<Self> {
        Self::from_arrays(Array::new(keys.into_iter().map(Value::Symbol))?, values)
    }
    /// Construct a dictionary from independently typed values.
    pub fn from_values(keys: Array, values: impl IntoIterator<Item = Value>) -> Result<Self> {
        Self::from_arrays(keys, Array::dictionary_values(values.into_iter().collect()))
    }
    pub fn from_arrays(keys: Array, values: Array) -> Result<Self> {
        if keys.is_mixed() {
            return Err("dictionary keys require homogeneous elements".into());
        }
        if keys.len() != values.len() {
            return Err(format!(
                "dictionary length mismatch: {} keys and {} values",
                keys.len(),
                values.len()
            ));
        }
        // Keys are snapshotted recursively so mutations cannot change lookup identity.
        let keys = keys.detached();
        let mut index = DictionaryIndex::default();
        let mut depth = 0;
        for (position, key) in keys.iter().enumerate() {
            index.insert(&key, position);
            depth = depth.max(key_depth(&key));
        }
        Ok(Self(Rc::new(RefCell::new(DictionaryData {
            keys,
            values: values.snapshot(),
            index: Rc::new(index),
            key_depth: depth,
        }))))
    }
    pub fn len(&self) -> usize {
        self.0.borrow().keys.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn keys(&self) -> Array {
        self.0.borrow().keys.detached()
    }
    pub fn values(&self) -> Array {
        self.0.borrow().values.snapshot()
    }
    pub fn get(&self, key: &str) -> Option<Value> {
        self.get_value(&Value::Symbol(Rc::from(key)))
    }
    pub fn get_value(&self, key: &Value) -> Option<Value> {
        let data = self.0.borrow();
        data.index
            .find(&data.keys, key)
            .and_then(|i| data.values.get(i))
    }
    pub fn insert(&self, key: Rc<str>, value: Value) -> Result<()> {
        self.insert_value(Value::Symbol(key), value)
    }
    pub fn insert_value(&self, key: Value, value: Value) -> Result<()> {
        value.check_cycle(&Value::Dictionary(self.clone()))?;
        key.check_cycle(&Value::Dictionary(self.clone()))?;
        let mut data = self.0.borrow_mut();
        if data.keys.is_empty() {
            let key = key.detached();
            let keys = Array::new([key.clone()])?;
            let values = Array::new([value])?;
            Rc::make_mut(&mut data.index).insert(&key, 0);
            data.key_depth = key_depth(&key);
            data.keys = keys;
            data.values = values;
            return Ok(());
        }
        if let Some(index) = data.index.find(&data.keys, &key) {
            let mut values = data.values.iter().collect::<Vec<_>>();
            values[index] = value;
            data.values = Array::dictionary_values(values);
        } else {
            if !data.keys.compatible(&key) {
                return Err("dictionary update requires matching key types".into());
            }
            let key = key.detached();
            let position = data.keys.len();
            Rc::make_mut(&mut data.index).insert(&key, position);
            data.key_depth = data.key_depth.max(key_depth(&key));
            data.values = Array::dictionary_values(data.values.iter().chain([value]).collect());
            data.keys.push(key)?;
        }
        Ok(())
    }
    pub(crate) fn key_is_atom(&self, key: &Value) -> bool {
        if self.get_value(key).is_some() {
            return true;
        }
        key_depth(key) <= self.0.borrow().key_depth
    }
    pub(crate) fn lookup(&self, key: &Value) -> Result<Value> {
        if !self.key_is_atom(key) {
            if let Some(keys) = key.string_items() {
                return keys
                    .iter()
                    .map(|k| self.lookup(&k))
                    .collect::<Result<Vec<_>>>()
                    .and_then(|values| self.values().rebuild(values))
                    .map(Value::from_array);
            }
            if let Value::Array(keys) = key {
                return keys
                    .iter()
                    .map(|k| self.lookup(&k))
                    .collect::<Result<Vec<_>>>()
                    .and_then(|values| self.values().rebuild(values))
                    .map(Value::from_array);
            }
        }
        Ok(self.get_value(key).unwrap_or(Value::Null))
    }
}

/// Shared mutable array. Ordinary arrays are homogeneous; dictionary value lists may be mixed.
/// Numeric and symbol vectors use Arrow buffers.
#[derive(Clone, Debug)]
pub struct Array(Rc<RefCell<Storage>>);

#[derive(Clone, Debug)]
enum Storage {
    Numbers(crate::arrow::Numbers),
    Symbols(arrow_array::StringArray),
    Values(Rc<Vec<Value>>, Rc<ElementType>),
}

impl Array {
    pub fn numbers(values: Vec<Number>) -> Self {
        Self::new(values.into_iter().map(Value::Number)).expect("numbers are homogeneous")
    }
    /// Construct a homogeneous array, promoting booleans to integers and
    /// integers to floats when necessary. Incompatible element types fail.
    pub fn new(values: impl IntoIterator<Item = Value>) -> Result<Self> {
        let values: Vec<_> = values.into_iter().collect();
        let kind = values
            .iter()
            .filter(|v| !v.is_null())
            .map(value_type)
            .reduce(|a, b| match (&a, &b) {
                (ElementType::Number(x), ElementType::Number(y)) => {
                    ElementType::Number((*x).max(*y))
                }
                _ => a,
            })
            .unwrap_or(if values.is_empty() {
                ElementType::Number(7)
            } else {
                ElementType::Null
            });
        Self::with_type(values, kind)
    }
    /// Dictionary value lists preserve each entry's type, without numeric promotion.
    pub(crate) fn dictionary_values(values: Vec<Value>) -> Self {
        let kind = values
            .iter()
            .find(|v| !v.is_null())
            .map(value_type)
            .unwrap_or(if values.is_empty() {
                ElementType::Number(7)
            } else {
                ElementType::Null
            });
        if values.iter().all(|v| v.is_null() || value_type(v) == kind) {
            Self::with_type(values, kind).expect("matching value types")
        } else {
            Self::from_storage(Storage::Values(
                Rc::new(values),
                Rc::new(ElementType::Mixed),
            ))
        }
    }
    pub(crate) fn is_mixed(&self) -> bool {
        self.element_type() == ElementType::Mixed
    }
    fn with_type(values: Vec<Value>, kind: ElementType) -> Result<Self> {
        if kind != ElementType::Mixed
            && values.iter().any(|v| {
                !v.is_null()
                    && value_type(v) != kind
                    && !matches!((&kind, v), (ElementType::Number(_), Value::Number(_)))
            })
        {
            return Err("arrays require homogeneous elements".into());
        }
        match kind {
            ElementType::Number(code) => Ok(Self::from_storage(Storage::Numbers(
                crate::arrow::Numbers::new(
                    values
                        .into_iter()
                        .map(|v| match v {
                            Value::Number(n) => Some(n),
                            _ => None,
                        })
                        .collect(),
                    code,
                ),
            ))),
            ElementType::Symbol => Ok(Self::from_storage(Storage::Symbols(
                arrow_array::StringArray::from_iter(values.iter().map(|v| match v {
                    Value::Symbol(s) => Some(s.as_ref()),
                    _ => None,
                })),
            ))),
            kind => Ok(Self::from_storage(Storage::Values(
                Rc::new(values),
                Rc::new(kind),
            ))),
        }
    }
    pub(crate) fn rebuild(&self, values: Vec<Value>) -> Result<Self> {
        if self.is_mixed() {
            return Ok(Self::dictionary_values(values));
        }
        Self::with_type(values, self.element_type())
    }
    pub(crate) fn numeric(values: Vec<Value>, code: i64) -> Result<Self> {
        Self::with_type(values, ElementType::Number(code))
    }
    fn from_storage(storage: Storage) -> Self {
        Self(Rc::new(RefCell::new(storage)))
    }
    pub(crate) fn type_code(&self) -> i64 {
        match self.element_type() {
            ElementType::Number(n) => n,
            ElementType::Symbol => 11,
            ElementType::Char => 10,
            _ => 0,
        }
    }
    fn detached(&self) -> Self {
        let storage = match &*self.0.borrow() {
            Storage::Values(xs, kind) => Storage::Values(
                Rc::new(xs.iter().map(Value::detached).collect()),
                kind.clone(),
            ),
            storage => storage.clone(),
        };
        Self::from_storage(storage)
    }
    /// A new array sharing this array's current storage.
    pub(crate) fn snapshot(&self) -> Self {
        Self::from_storage(self.0.borrow().clone())
    }
    /// Materialize a non-null numeric snapshot; nullable arrays return None.
    /// Prefer `iter` or `to_arrow` to preserve nulls.
    pub fn as_numbers(&self) -> Option<Rc<Vec<Number>>> {
        self.numeric_snapshot()
            .and_then(|xs| xs.iter().collect::<Option<Vec<_>>>())
            .map(Rc::new)
    }
    pub(crate) fn numeric_snapshot(&self) -> Option<crate::arrow::Numbers> {
        match &*self.0.borrow() {
            Storage::Numbers(xs) => Some(xs.clone()),
            _ => None,
        }
    }
    pub(crate) fn from_numbers(numbers: crate::arrow::Numbers) -> Self {
        Self::from_storage(Storage::Numbers(numbers))
    }
    /// Export a stable Arrow snapshot without copying numeric or symbol buffers.
    /// Nested and other runtime values are not yet supported by this boundary.
    pub fn to_arrow(&self) -> Result<arrow_array::ArrayRef> {
        match &*self.0.borrow() {
            Storage::Numbers(xs) => Ok(xs.to_arrow()),
            Storage::Symbols(xs) => Ok(std::sync::Arc::new(xs.clone())),
            Storage::Values(xs, kind) if **kind == ElementType::Null => {
                Ok(std::sync::Arc::new(arrow_array::NullArray::new(xs.len())))
            }
            Storage::Values(_, _) => {
                Err("Arrow export requires numeric, symbol, or null vectors".into())
            }
        }
    }
    /// Import nullable Boolean, Int64, Float64, Utf8 (symbol), or Null arrays.
    pub fn from_arrow(array: arrow_array::ArrayRef) -> Result<Self> {
        use arrow_array::Array as _;
        if let Some(xs) = array.as_any().downcast_ref::<arrow_array::StringArray>() {
            return Ok(Self::from_storage(Storage::Symbols(xs.clone())));
        }
        if array.as_any().is::<arrow_array::NullArray>() {
            return Self::with_type(vec![Value::Null; array.len()], ElementType::Null);
        }
        crate::arrow::Numbers::from_arrow(array).map(|xs| Self::from_storage(Storage::Numbers(xs)))
    }
    pub fn len(&self) -> usize {
        use arrow_array::Array as _;
        match &*self.0.borrow() {
            Storage::Numbers(xs) => xs.len(),
            Storage::Symbols(xs) => xs.len(),
            Storage::Values(xs, _) => xs.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<Value> {
        if index >= self.len() {
            return None;
        }
        Some(storage_value(&self.0.borrow(), index))
    }
    pub(crate) fn index(&self, number: &Number) -> Result<usize> {
        let n = number
            .to_isize()
            .ok_or("index out of bounds or not an integer")?;
        usize::try_from(n)
            .ok()
            .filter(|i| *i < self.len())
            .ok_or_else(|| "index out of bounds".into())
    }
    /// Iterate a snapshot of the elements; nested containers remain shared.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Value> + ExactSizeIterator + use<> {
        let storage = self.0.borrow().clone();
        (0..self.len()).map(move |i| storage_value(&storage, i))
    }
    /// Replace an element without changing the array's element type.
    pub fn set(&self, index: usize, value: Value) -> Result<()> {
        if index >= self.len() {
            return Err("index out of bounds".into());
        }
        value.check_cycle(&Value::Array(self.clone()))?;
        self.replace(index, value)
    }
    fn compatible(&self, value: &Value) -> bool {
        self.is_mixed() || value.is_null() || self.element_type() == value_type(value)
    }
    fn element_type(&self) -> ElementType {
        match &*self.0.borrow() {
            Storage::Numbers(xs) => ElementType::Number(match xs {
                crate::arrow::Numbers::Bool(_) => 1,
                crate::arrow::Numbers::Int(_) => 7,
                crate::arrow::Numbers::Float(_) => 9,
            }),
            Storage::Symbols(_) => ElementType::Symbol,
            Storage::Values(_, kind) => (**kind).clone(),
        }
    }
    pub fn append(&self, value: Value) -> Result<()> {
        let values = match value {
            Value::Array(source) => source.iter().collect::<Vec<_>>(),
            value => vec![value],
        };
        if values.is_empty() {
            return Ok(());
        }
        for value in &values {
            value.check_cycle(&Value::Array(self.clone()))?;
            if !self.compatible(value) {
                return Err("array update requires matching element type".into());
            }
        }
        let result = Self::with_type(self.iter().chain(values).collect(), self.element_type())?;
        *self.0.borrow_mut() = result.0.borrow().clone();
        Ok(())
    }
    fn replace(&self, index: usize, value: Value) -> Result<()> {
        if !self.compatible(&value) {
            return Err("array update requires matching element type".into());
        }
        let values = self
            .iter()
            .enumerate()
            .map(|(i, v)| if i == index { value.clone() } else { v });
        let result = Self::with_type(values.collect(), self.element_type())?;
        *self.0.borrow_mut() = result.0.borrow().clone();
        Ok(())
    }
    fn push(&self, value: Value) -> Result<()> {
        if !self.compatible(&value) {
            return Err("array update requires matching element type".into());
        }
        let result = Self::with_type(
            self.iter().chain(std::iter::once(value)).collect(),
            self.element_type(),
        )?;
        *self.0.borrow_mut() = result.0.borrow().clone();
        Ok(())
    }
    pub(crate) fn reversed(&self) -> Self {
        self.snapshot().into_reversed()
    }
    pub(crate) fn into_reversed(self) -> Self {
        if self.is_empty() {
            return self;
        }
        let reversed = Self::with_type(self.iter().rev().collect(), self.element_type())
            .expect("reversal preserves element types");
        *self.0.borrow_mut() = reversed.0.borrow().clone();
        self
    }
    pub(crate) fn slice(&self, start: usize, end: usize) -> Self {
        let storage = match &*self.0.borrow() {
            Storage::Numbers(xs) => Storage::Numbers(xs.slice(start, end - start)),
            Storage::Symbols(xs) => Storage::Symbols(xs.slice(start, end - start)),
            Storage::Values(xs, kind) => {
                Storage::Values(Rc::new(xs[start..end].to_vec()), kind.clone())
            }
        };
        Self::from_storage(storage)
    }
}

fn storage_value(storage: &Storage, index: usize) -> Value {
    use arrow_array::Array as _;
    match storage {
        Storage::Numbers(xs) => xs.get(index).map(Value::Number).unwrap_or(Value::Null),
        Storage::Symbols(xs) => {
            if xs.is_null(index) {
                Value::Null
            } else {
                Value::Symbol(Rc::from(xs.value(index)))
            }
        }
        Storage::Values(xs, _) => xs[index].clone(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ElementType {
    Number(i64),
    Symbol,
    Char,
    String,
    Array(Box<ElementType>),
    Dictionary,
    Table,
    Function,
    Null,
    Mixed,
}
fn value_type(value: &Value) -> ElementType {
    match value {
        Value::Number(n) => ElementType::Number(n.type_code()),
        Value::Symbol(_) => ElementType::Symbol,
        Value::Char(_) => ElementType::Char,
        Value::String(_) => ElementType::String,
        Value::Array(a) => ElementType::Array(Box::new(a.element_type())),
        Value::Dictionary(_) => ElementType::Dictionary,
        Value::Table(_) => ElementType::Table,
        Value::Function(_) => ElementType::Function,
        Value::Null => ElementType::Null,
    }
}

enum Key {
    Index(usize),
    Dictionary(Value),
    Column(Rc<str>),
}

struct Edit {
    target: Value,
    key: Key,
    value: Value,
}

impl Edit {
    fn apply(&self) -> Result<()> {
        match (&self.target, &self.key) {
            (Value::Array(a), Key::Index(i)) => a.set(*i, self.value.clone()),
            (Value::String(s), Key::Index(i)) => match self.value {
                Value::Char(c) => s.set(*i, c),
                _ => Err("string updates require characters".into()),
            },
            (Value::Dictionary(d), Key::Dictionary(key)) => {
                d.insert_value(key.clone(), self.value.clone())
            }
            (Value::Table(t), Key::Column(name)) => match &self.value {
                Value::Array(a) => t.set_column(name.clone(), a.clone()),
                _ => Err("table columns must be arrays".into()),
            },
            _ => unreachable!(),
        }
    }
}

enum Backup {
    Array(Array, Storage),
    String(MutableString, Rc<Vec<u8>>),
    Dictionary(Dictionary, DictionaryData),
    Table(Table, TableColumns),
}

impl Backup {
    fn new(value: &Value) -> Self {
        match value {
            Value::Array(a) => Self::Array(a.clone(), a.0.borrow().clone()),
            Value::Table(t) => Self::Table(t.clone(), t.columns()),
            Value::String(s) => Self::String(s.clone(), s.snapshot()),
            Value::Dictionary(d) => {
                let data = d.0.borrow();
                Self::Dictionary(
                    d.clone(),
                    DictionaryData {
                        keys: data.keys.snapshot(),
                        values: data.values.snapshot(),
                        index: data.index.clone(),
                        key_depth: data.key_depth,
                    },
                )
            }
            _ => unreachable!(),
        }
    }
    fn restore(self) {
        match self {
            Self::Array(a, storage) => *a.0.borrow_mut() = storage,
            Self::String(s, bytes) => *s.0.borrow_mut() = bytes,
            Self::Dictionary(d, data) => *d.0.borrow_mut() = data,
            Self::Table(t, columns) => *t.0.borrow_mut() = columns,
        }
    }
}

pub(crate) struct ArrayBuilder(Vec<Value>);
impl ArrayBuilder {
    pub(crate) fn new(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }
    pub(crate) fn push(&mut self, value: Value) {
        self.0.push(value);
    }
    pub(crate) fn finish(self) -> Result<Array> {
        Array::new(self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Function {
    pub(crate) kind: FunctionKind,
}

#[derive(Clone, Debug)]
pub(crate) enum FunctionKind {
    Verb(char),
    Monadic(char),
    Round,
    Native(&'static str),
    Projection(Value, Vec<Option<Value>>),
    Composition(Value, Value),
    Derived(char, Value),
    User {
        name: Option<Rc<str>>,
        proto: Rc<Proto>,
        /// Values for `proto.free`; names unbound at creation are None.
        captures: Vec<Option<Value>>,
        /// The slot that calls bind to the function itself.
        self_slot: Option<usize>,
    },
}

impl Value {
    /// Convert a native number to a language value; undefined floats become null.
    pub fn from_number(number: Number) -> Self {
        if matches!(number, Number::Float(n) if n.is_nan()) {
            Self::Null
        } else {
            Self::Number(number)
        }
    }
    pub(crate) fn text(text: impl AsRef<[u8]>) -> Self {
        Self::String(MutableString::new(text))
    }
    pub(crate) fn as_text(&self) -> Option<String> {
        let bytes = match self {
            Self::Char(c) => vec![*c],
            Self::String(s) => s.snapshot().as_ref().clone(),
            Self::Array(a) => a
                .iter()
                .map(|v| if let Self::Char(c) = v { Some(c) } else { None })
                .collect::<Option<Vec<_>>>()?,
            _ => return None,
        };
        String::from_utf8(bytes).ok()
    }
    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    pub(crate) fn detached(&self) -> Self {
        match self {
            Self::String(s) => Self::String(s.detached()),
            Self::Array(a) => Self::Array(a.detached()),
            Self::Table(t) => Self::Table(t.detached()),
            Self::Dictionary(d) => Self::Dictionary(
                Dictionary::from_arrays(d.keys(), d.values().detached())
                    .expect("matching dictionary lengths"),
            ),
            _ => self.clone(),
        }
    }
    pub fn array(values: impl IntoIterator<Item = Value>) -> Result<Self> {
        Array::new(values).map(Self::from_array)
    }
    pub(crate) fn from_array(array: Array) -> Self {
        if !array.is_empty() && array.iter().all(|v| matches!(v, Self::Char(_))) {
            Self::text(
                array
                    .iter()
                    .map(|v| {
                        if let Self::Char(c) = v {
                            c
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            Self::Array(array)
        }
    }
    pub(crate) fn string_items(&self) -> Option<Array> {
        if let Self::String(s) = self {
            Some(
                Array::new(s.snapshot().iter().copied().map(Self::Char))
                    .expect("characters are homogeneous"),
            )
        } else {
            None
        }
    }
    pub(crate) fn function(kind: FunctionKind) -> Self {
        Self::Function(Rc::new(Function { kind }))
    }
    fn identity(&self) -> Option<(u8, usize)> {
        match self {
            Self::Array(a) => Some((0, Rc::as_ptr(&a.0) as usize)),
            Self::Dictionary(d) => Some((1, Rc::as_ptr(&d.0) as usize)),
            Self::Function(f) => Some((2, Rc::as_ptr(f) as usize)),
            Self::String(s) => Some((3, Rc::as_ptr(&s.0) as usize)),
            Self::Table(t) => Some((4, Rc::as_ptr(&t.0) as usize)),
            _ => None,
        }
    }
    fn check_cycle(&self, target: &Value) -> Result<()> {
        if self.identity().is_none() {
            return Ok(());
        }
        let mut pending = vec![self.clone()];
        let mut visited = HashSet::new();
        while let Some(value) = pending.pop() {
            let Some(id) = value.identity() else { continue };
            if Some(id) == target.identity() {
                return Err("mutation would create a reference cycle".into());
            }
            if !visited.insert(id) {
                continue;
            }
            match value {
                Self::String(_) => {}
                Self::Table(t) => {
                    pending.extend(t.columns().into_iter().map(|(_, a)| Self::Array(a)))
                }
                Self::Array(a) => {
                    if a.numeric_snapshot().is_none() {
                        pending.extend(a.iter().filter(|value| value.identity().is_some()));
                    }
                }
                Self::Dictionary(d) => {
                    let data = d.0.borrow();
                    pending.extend(data.keys.iter());
                    if data.values.numeric_snapshot().is_none() {
                        pending.extend(
                            data.values
                                .iter()
                                .filter(|value| value.identity().is_some()),
                        );
                    }
                }
                Self::Function(f) => match &f.kind {
                    FunctionKind::User { captures, .. } => {
                        pending.extend(captures.iter().flatten().cloned())
                    }
                    FunctionKind::Derived(_, value) => pending.push(value.clone()),
                    FunctionKind::Projection(f, args) => {
                        pending.push(f.clone());
                        pending.extend(args.iter().flatten().cloned());
                    }
                    FunctionKind::Composition(f, g) => {
                        pending.push(f.clone());
                        pending.push(g.clone());
                    }
                    _ => {}
                },
                _ => unreachable!(),
            }
        }
        Ok(())
    }
    pub(crate) fn update(&self, indices: &[Value], value: &Value) -> Result<()> {
        let mut edits = Vec::new();
        self.plan_update(indices, value, &mut edits)?;
        if let [edit] = edits.as_slice() {
            return edit.apply();
        }
        // Preserve all destinations for rollback, including aliases and new keys.
        // A single update constructs replacement storage before committing.
        let mut seen = HashSet::new();
        let mut backups = Vec::new();
        for edit in &edits {
            if seen.insert(edit.target.identity()) {
                backups.push(Backup::new(&edit.target));
            }
        }
        for edit in edits {
            if let Err(error) = edit.apply() {
                for backup in backups {
                    backup.restore();
                }
                return Err(error);
            }
        }
        Ok(())
    }
    fn plan_update(&self, indices: &[Value], value: &Value, edits: &mut Vec<Edit>) -> Result<()> {
        let (index, rest) = indices.split_first().expect("indexed assignment");
        if !matches!(
            self,
            Self::Array(_) | Self::String(_) | Self::Dictionary(_) | Self::Table(_)
        ) {
            return Err("indexed assignment requires an array or dictionary".into());
        }
        if let Self::Function(identity) = index
            && matches!(identity.kind, FunctionKind::Verb(':'))
        {
            let all = match self {
                Self::Array(a) => {
                    Self::array((0..a.len()).map(|i| Self::Number(Number::length(i))))?
                }
                Self::String(s) => {
                    Self::array((0..s.len()).map(|i| Self::Number(Number::length(i))))?
                }
                Self::Dictionary(d) => Self::Array(d.keys()),
                Self::Table(t) => Self::Array(t.names()),
                _ => unreachable!(),
            };
            let mut selectors = vec![all];
            selectors.extend_from_slice(rest);
            return self.plan_update(&selectors, value, edits);
        }
        if let Self::Dictionary(d) = self
            && !d.key_is_atom(index)
            && let Some(chars) = index.string_items()
        {
            let mut selectors = vec![Self::Array(chars)];
            selectors.extend_from_slice(rest);
            return self.plan_update(&selectors, value, edits);
        }
        if let Self::Dictionary(d) = self
            && d.key_is_atom(index)
        {
            return self.plan_element(index, rest, value, edits);
        }
        if let Self::Array(selectors) = index {
            if let Some(chars) = value.string_items() {
                return self.plan_update(indices, &Self::Array(chars), edits);
            }
            if let Self::Array(values) = value
                && values.len() != selectors.len()
            {
                return Err("indexed assignment length mismatch".into());
            }
            for (i, selector) in selectors.iter().enumerate() {
                if matches!(selector, Self::Array(_)) && !matches!(self, Self::Dictionary(_)) {
                    return Err("assignment indices must be a flat array".into());
                }
                let value = match value {
                    Self::Array(values) => values.get(i).expect("matching lengths"),
                    value => value.clone(),
                };
                self.plan_element(&selector, rest, &value, edits)?;
            }
            Ok(())
        } else {
            self.plan_element(index, rest, value, edits)
        }
    }
    fn plan_element(
        &self,
        index: &Value,
        rest: &[Value],
        value: &Value,
        edits: &mut Vec<Edit>,
    ) -> Result<()> {
        let key = match (self, index) {
            (Self::Array(a), Self::Number(n)) => Key::Index(a.index(n)?),
            (Self::String(s), Self::Number(n)) => Key::Index(
                n.to_isize()
                    .and_then(|i| usize::try_from(i).ok())
                    .filter(|i| *i < s.len())
                    .ok_or("string index out of bounds")?,
            ),
            (Self::String(_), _) => return Err("string index must be an integer".into()),
            (Self::Dictionary(_), key) => Key::Dictionary(key.clone()),
            (Self::Table(_), Self::Symbol(name)) => Key::Column(name.clone()),
            (Self::Table(_), _) => return Err("table updates require column names".into()),
            (Self::Array(_), _) => return Err("array assignment index must be a number".into()),
            _ => unreachable!(),
        };
        if rest.is_empty() {
            edits.push(Edit {
                target: self.clone(),
                key,
                value: value.clone(),
            });
            Ok(())
        } else {
            let child = match (self, key) {
                (Self::Table(t), Key::Column(name)) => Self::Array(
                    t.column(&name)
                        .ok_or_else(|| format!("missing table column: {name}"))?,
                ),
                (Self::Array(a), Key::Index(i)) => a.get(i).expect("validated index"),
                (Self::String(s), Key::Index(i)) => Self::Char(s.get(i).expect("validated index")),
                (Self::Dictionary(d), Key::Dictionary(s)) => d
                    .get_value(&s)
                    .ok_or_else(|| format!("missing dictionary key: `{s}"))?,
                _ => unreachable!(),
            };
            child.plan_update(rest, value, edits)
        }
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Table(a), Self::Table(b)) => {
                let a = a.columns();
                let b = b.columns();
                a.len() == b.len()
                    && a.iter().zip(&b).all(|((an, av), (bn, bv))| {
                        an == bn && Self::Array(av.clone()).same(&Self::Array(bv.clone()))
                    })
            }
            (Self::Null, Self::Null) => true,
            (Self::Char(a), Self::Char(b)) => a == b,
            (Self::String(a), Self::String(b)) => a.snapshot() == b.snapshot(),
            (Self::Number(a), Self::Number(b)) => a.same(b),
            (Self::Symbol(a), Self::Symbol(b)) => a == b,
            (Self::Dictionary(a), Self::Dictionary(b)) => {
                let a = a.0.borrow();
                let b = b.0.borrow();
                Value::Array(a.keys.clone()).same(&Value::Array(b.keys.clone()))
                    && a.values
                        .iter()
                        .zip(b.values.iter())
                        .all(|(a, b)| a.same(&b))
            }
            (Self::Array(a), Self::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.same(&b))
            }
            (Self::Function(a), Self::Function(b)) => {
                Rc::ptr_eq(a, b)
                    || match (&a.kind, &b.kind) {
                        (FunctionKind::Verb(a), FunctionKind::Verb(b))
                        | (FunctionKind::Monadic(a), FunctionKind::Monadic(b)) => a == b,
                        (FunctionKind::Round, FunctionKind::Round) => true,
                        (FunctionKind::Native(a), FunctionKind::Native(b)) => a == b,
                        (FunctionKind::Derived(a, f), FunctionKind::Derived(b, g)) => {
                            a == b && f.same(g)
                        }
                        (FunctionKind::Composition(f, g), FunctionKind::Composition(h, i)) => {
                            f.same(h) && g.same(i)
                        }
                        (FunctionKind::Projection(f, a), FunctionKind::Projection(g, b)) => {
                            f.same(g)
                                && a.len() == b.len()
                                && a.iter().zip(b).all(|(a, b)| match (a, b) {
                                    (Some(a), Some(b)) => a.same(b),
                                    (None, None) => true,
                                    _ => false,
                                })
                        }
                        (
                            FunctionKind::User {
                                proto: a,
                                captures: ac,
                                ..
                            },
                            FunctionKind::User {
                                proto: b,
                                captures: bc,
                                ..
                            },
                        ) => {
                            (Rc::ptr_eq(a, b) || a.same_source(b))
                                && ac.len() == bc.len()
                                && ac.iter().zip(bc).all(|pair| match pair {
                                    (Some(a), Some(b)) => a.same(b),
                                    (None, None) => true,
                                    _ => false,
                                })
                        }
                        _ => false,
                    }
            }
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table(t) => {
                write!(f, "([]")?;
                for (i, (name, column)) in t.columns().into_iter().enumerate() {
                    if i > 0 {
                        write!(f, ";")?;
                    }
                    if name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                        && name.bytes().all(|b| b.is_ascii_alphanumeric())
                    {
                        write!(f, "{name}")?;
                    } else {
                        write!(f, "{}", MutableString::new(name.as_bytes()))?;
                    }
                    write!(f, ":{}", Self::Array(column))?;
                }
                write!(f, ")")
            }
            Self::Null => write!(f, "0n"),
            Self::Char(c) => write!(f, "{}", MutableString::new([*c])),
            Self::String(s) if s.len() == 1 => write!(f, ",{s}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Symbol(name) => write!(f, "`{name}"),
            Self::Dictionary(d) if d.len() == 1 => write!(
                f,
                "((,{})!,{})",
                d.keys().get(0).unwrap(),
                d.values().get(0).unwrap()
            ),
            Self::Dictionary(d) => {
                write!(f, "({}!{})", Self::Array(d.keys()), Self::Array(d.values()))
            }
            Self::Function(_) => write!(f, "<fn>"),
            Self::Array(xs) if xs.is_empty() => write!(f, "()"),
            Self::Array(xs) if xs.iter().all(|v| matches!(v, Self::Char(_))) => {
                write!(f, "{:?}", self.as_text().unwrap_or_default())
            }
            Self::Array(xs) if xs.len() == 1 => write!(f, ",({})", xs.get(0).unwrap()),
            Self::Array(xs) => {
                let numeric =
                    !xs.is_mixed() && xs.iter().all(|v| matches!(v, Self::Number(_) | Self::Null));
                let symbols = !numeric && xs.iter().all(|x| matches!(x, Self::Symbol(_)));
                if !numeric && !symbols {
                    write!(f, "(")?;
                }
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        write!(
                            f,
                            "{}",
                            if numeric {
                                " "
                            } else if symbols {
                                ""
                            } else {
                                ";"
                            }
                        )?;
                    }
                    write!(f, "{x}")?;
                }
                if !numeric && !symbols {
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/dictionary_index.rs"]
mod dictionary_index_tests;
