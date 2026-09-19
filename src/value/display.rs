use super::*;

pub(super) fn single_line(text: &str) -> String {
    let mut result = String::new();
    for c in text.chars() {
        if c.is_control() {
            result.extend(c.escape_default());
        } else {
            result.push(c);
        }
    }
    result
}

pub(super) fn cell(value: &Value) -> String {
    match value {
        Value::Symbol(name) if !name.is_empty() => single_line(name),
        value => single_line(&value.to_string()),
    }
}

/// Render ordered key/value rows while keeping nested values in compact syntax.
impl fmt::Display for Dictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "(()!())");
        }
        let rows: Vec<_> = self
            .keys()
            .iter()
            .zip(self.values().iter())
            .map(|(key, value)| (cell(&key), cell(&value)))
            .collect();
        let width = rows
            .iter()
            .map(|(key, _)| key.chars().count())
            .max()
            .unwrap_or(0);
        for (i, (key, value)) in rows.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{key:<width$} | {value}")?;
        }
        Ok(())
    }
}
