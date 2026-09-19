use super::display::{cell, single_line};
use super::*;

impl Table {
    /// Construct a table without copying or padding its shared columns.
    pub fn new(columns: Vec<(Rc<str>, Array)>) -> Result<Self> {
        if columns.is_empty() {
            return Err("tables require at least one column".into());
        }
        let mut names = HashSet::new();
        for (name, column) in &columns {
            if column.is_mixed() {
                return Err("table columns require homogeneous elements".into());
            }
            if !names.insert(name.clone()) {
                return Err(format!("duplicate table column: {name}"));
            }
        }
        Ok(Self(Rc::new(RefCell::new(columns))))
    }

    /// Number of logical rows, including implicit nulls in shorter columns.
    pub fn len(&self) -> usize {
        self.0
            .borrow()
            .iter()
            .map(|(_, a)| a.len())
            .max()
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn names(&self) -> Array {
        Array::with_type(
            self.0
                .borrow()
                .iter()
                .map(|(n, _)| Value::Symbol(n.clone()))
                .collect(),
            ElementType::Symbol,
        )
        .expect("column names are symbols")
    }

    /// Return the actual shared column; its physical length may be less than len().
    pub fn column(&self, name: &str) -> Option<Array> {
        self.0
            .borrow()
            .iter()
            .find(|(n, _)| n.as_ref() == name)
            .map(|(_, a)| a.clone())
    }

    pub fn columns(&self) -> Vec<(Rc<str>, Array)> {
        self.0.borrow().clone()
    }

    /// Add or replace a column. Replacement preserves type, but may change length.
    /// Previously extracted aliases retain the old column on replacement.
    pub fn set_column(&self, name: Rc<str>, column: Array) -> Result<()> {
        if column.is_mixed() {
            return Err("table columns require homogeneous elements".into());
        }
        Value::Array(column.clone()).check_cycle(&Value::Table(self.clone()))?;
        let mut columns = self.0.borrow_mut();
        if let Some((_, old)) = columns.iter_mut().find(|(n, _)| *n == name) {
            if old.element_type() != column.element_type() {
                return Err("table column replacement requires matching element type".into());
            }
            *old = column;
        } else {
            columns.push((name, column));
        }
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Self {
        Self::new(
            self.columns()
                .into_iter()
                .map(|(n, a)| (n, a.snapshot()))
                .collect(),
        )
        .expect("existing table schema")
    }

    pub(super) fn detached(&self) -> Self {
        Self::new(
            self.columns()
                .into_iter()
                .map(|(n, a)| (n, a.detached()))
                .collect(),
        )
        .expect("existing table schema")
    }

    pub(crate) fn lookup(&self, index: &Value) -> Result<Value> {
        match index {
            Value::Symbol(name) => self
                .column(name)
                .map(Value::Array)
                .ok_or_else(|| format!("missing table column: {name}")),
            Value::Function(f) if matches!(f.kind, FunctionKind::Verb(':')) => {
                Ok(Value::Table(self.clone()))
            }
            Value::Array(indices) if indices.element_type() == ElementType::Symbol => {
                let columns = indices
                    .iter()
                    .map(|v| {
                        let Value::Symbol(name) = v else {
                            return Err("column names cannot be null".into());
                        };
                        let column = self
                            .column(&name)
                            .ok_or_else(|| format!("missing table column: {name}"))?;
                        Ok((name, column))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Self::new(columns).map(Value::Table)
            }
            Value::Array(indices) => self
                .select_rows(&indices.iter().collect::<Vec<_>>())
                .map(Value::Table),
            Value::Number(_) | Value::Null => self
                .select_rows(std::slice::from_ref(index))
                .map(Value::Table),
            _ => Err("table index must be a column name or integer row index".into()),
        }
    }

    fn select_rows(&self, indices: &[Value]) -> Result<Self> {
        let indices = indices
            .iter()
            .map(|v| match v {
                Value::Null => Ok(None),
                Value::Number(n) => n
                    .as_i64()
                    .map(|i| usize::try_from(i).ok())
                    .ok_or_else(|| "table row index must be an integer".to_owned()),
                _ => Err("table row indices must be integers or nulls".into()),
            })
            .collect::<Result<Vec<_>>>()?;
        Self::new(
            self.columns()
                .into_iter()
                .map(|(name, column)| {
                    let values = indices
                        .iter()
                        .map(|i| i.and_then(|i| column.get(i)).unwrap_or(Value::Null))
                        .collect();
                    Ok((name, column.rebuild(values)?))
                })
                .collect::<Result<_>>()?,
        )
    }

    /// Append logical rows, aligning both tables' shorter columns with nulls.
    /// Builds every replacement before updating any shared column.
    pub fn append(&self, source: &Self) -> Result<()> {
        let columns = self.columns();
        let incoming = source.columns();
        if columns.len() != incoming.len() {
            return Err("table append requires matching column names".into());
        }
        let start = self.len();
        let rows = source.len();
        let end = start.checked_add(rows).ok_or("table row count overflow")?;
        let mut replacements: Vec<(Array, Array)> = Vec::new();
        for (name, column) in columns {
            let other = incoming
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, a)| a)
                .ok_or_else(|| format!("table append missing column: {name}"))?;
            if column.element_type() != other.element_type() {
                return Err(format!("table append type mismatch for column: {name}"));
            }
            if rows == 0 {
                continue;
            }
            let mut values: Vec<_> = column.iter().collect();
            values.resize(start, Value::Null);
            for value in other.iter() {
                value.check_cycle(&Value::Array(column.clone()))?;
                values.push(value);
            }
            values.resize(end, Value::Null);
            let replacement = column.rebuild(values)?;
            if let Some((_, previous)) = replacements
                .iter()
                .find(|(a, _)| Rc::ptr_eq(&a.0, &column.0))
            {
                // Duplicate column aliases must receive exactly the same values, including
                // float bits and nested mutable identities, not tolerant language equality.
                if !previous
                    .iter()
                    .zip(replacement.iter())
                    .all(|(a, b)| match (&a, &b) {
                        (Value::Number(Number::Float(a)), Value::Number(Number::Float(b))) => {
                            a.to_bits() == b.to_bits()
                        }
                        _ if a.identity().is_some() || b.identity().is_some() => {
                            a.identity() == b.identity()
                        }
                        _ => a.same(&b),
                    })
                {
                    return Err("table append has conflicting values for shared columns".into());
                }
            } else {
                replacements.push((column, replacement));
            }
        }
        let backups: Vec<_> = replacements
            .iter()
            .map(|(column, _)| (column.clone(), column.0.borrow().clone()))
            .collect();
        for (column, replacement) in replacements {
            // Earlier replacements can create paths through other destination columns.
            // Check against that graph and roll back the entire append on failure.
            if let Err(error) =
                Value::Array(replacement.clone()).check_cycle(&Value::Array(column.clone()))
            {
                for (column, storage) in backups {
                    *column.0.borrow_mut() = storage;
                }
                return Err(error);
            }
            *column.0.borrow_mut() = replacement.0.borrow().clone();
        }
        Ok(())
    }

    /// Export a rectangular Arrow snapshot, padding short columns without mutation.
    /// The same column types as Array::to_arrow are supported.
    pub fn to_arrow(&self) -> Result<arrow_array::RecordBatch> {
        use arrow_schema::{Field, Schema};
        use std::sync::Arc;
        let rows = self.len();
        let mut fields = Vec::new();
        let mut arrays = Vec::new();
        for (name, column) in self.columns() {
            let array = if column.len() == rows {
                column.to_arrow()?
            } else {
                let mut values: Vec<_> = column.iter().collect();
                values.resize(rows, Value::Null);
                column.rebuild(values)?.to_arrow()?
            };
            fields.push(Field::new(name.as_ref(), array.data_type().clone(), true));
            arrays.push(array);
        }
        arrow_array::RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays)
            .map_err(|e| e.to_string())
    }

    /// Import a batch using Array::from_arrow's supported types and normalization.
    pub fn from_arrow(batch: &arrow_array::RecordBatch) -> Result<Self> {
        Self::new(
            batch
                .schema()
                .fields()
                .iter()
                .zip(batch.columns())
                .map(|(field, array)| {
                    Ok((
                        Rc::from(field.name().as_str()),
                        Array::from_arrow(array.clone())?,
                    ))
                })
                .collect::<Result<_>>()?,
        )
    }
}

/// Render a table as aligned rows without padding or changing its shared arrays.
impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rows = self.len();
        let columns: Vec<_> = self
            .columns()
            .into_iter()
            .map(|(name, array)| {
                let header = if name.is_empty() {
                    "\"\"".to_owned()
                } else {
                    single_line(&name)
                };
                let cells: Vec<_> = (0..rows)
                    .map(|row| cell(&array.get(row).unwrap_or(Value::Null)))
                    .collect();
                let width = cells
                    .iter()
                    .map(|s| s.chars().count())
                    .max()
                    .unwrap_or(0)
                    .max(header.chars().count());
                let numeric = matches!(array.element_type(), ElementType::Number(_));
                (header, cells, width, numeric)
            })
            .collect();
        for (i, (header, _, width, _)) in columns.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            if i + 1 == columns.len() {
                write!(f, "{header}")?;
            } else {
                write!(f, "{header:<width$}")?;
            }
        }
        writeln!(f)?;
        for (i, (_, _, width, _)) in columns.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", "-".repeat(*width))?;
        }
        for row in 0..rows {
            writeln!(f)?;
            for (i, (_, cells, width, numeric)) in columns.iter().enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                let cell = &cells[row];
                if *numeric {
                    write!(f, "{cell:>width$}")?;
                } else if i + 1 == columns.len() {
                    write!(f, "{cell}")?;
                } else {
                    write!(f, "{cell:<width$}")?;
                }
            }
        }
        Ok(())
    }
}
