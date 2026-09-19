use super::*;

use std::fmt::Write as _;

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

/// A table or dictionary cell. Beyond `limit` characters the text is cut
/// short, keeping one extra character so the line still reads as too wide.
pub(super) fn cell(value: &Value, limit: usize) -> String {
    match value {
        Value::Symbol(name) if !name.is_empty() => single_line(name),
        value if limit == usize::MAX => single_line(&value.to_string()),
        value => {
            let mut text = Bounded {
                text: String::new(),
                left: limit.saturating_add(1),
            };
            let _ = write!(text, "{value}");
            single_line(&text.text)
        }
    }
}

/// Collects characters until its capacity runs out, then stops the writer.
struct Bounded {
    text: String,
    left: usize,
}

impl fmt::Write for Bounded {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if self.left == 0 {
                return Err(fmt::Error);
            }
            self.text.push(c);
            self.left -= 1;
        }
        Ok(())
    }
}

/// Limits for showing a value on a console. Zero leaves a dimension unlimited.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Console {
    pub rows: usize,
    pub columns: usize,
}

impl Console {
    pub const UNLIMITED: Self = Self {
        rows: 0,
        columns: 0,
    };
}

/// A value as a console shows it: tables and dictionaries as aligned rows,
/// other values in compact syntax. Lines wider than the console end in `..`.
/// Taller output ends in a `..` line; tables and dictionaries add their row or
/// entry count. Only the part that fits is rendered.
pub struct View<'a> {
    value: &'a Value,
    console: Console,
}

impl Value {
    pub fn view(&self, console: Console) -> View<'_> {
        View {
            value: self,
            console,
        }
    }
}

impl fmt::Display for View<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let limit = |n| if n == 0 { usize::MAX } else { n };
        let rows = limit(self.console.rows);
        let columns = limit(self.console.columns);
        if rows == usize::MAX && columns == usize::MAX {
            return match self.value {
                Value::Table(table) => table.render(f, rows, columns),
                Value::Dictionary(dictionary) => dictionary.render(f, rows, columns),
                value => write!(f, "{value}"),
            };
        }
        let mut clip = Clip {
            out: f,
            rows,
            columns,
            line: String::new(),
            width: 0,
            cut: false,
            lines: 0,
            stop_at_cut: false,
            stopped: false,
        };
        let result = match self.value {
            Value::Table(table) => table.render(&mut clip, rows, columns),
            Value::Dictionary(dictionary) => dictionary.render(&mut clip, rows, columns),
            value => {
                // Compact values are one line; stop rendering once it is cut.
                clip.stop_at_cut = true;
                write!(clip, "{value}")
            }
        };
        if !clip.stopped {
            result?;
        }
        clip.out.write_str(&clip.line)
    }
}

/// Writes lines of at most `columns` characters, ending cut lines in `..`, and
/// replaces everything from line `rows` on with a `..` line when more follows.
struct Clip<'a, W: fmt::Write> {
    out: &'a mut W,
    rows: usize,
    columns: usize,
    /// The line being written, flushed at the next newline.
    line: String,
    width: usize,
    cut: bool,
    /// Lines already written to `out`.
    lines: usize,
    stop_at_cut: bool,
    stopped: bool,
}

impl<W: fmt::Write> fmt::Write for Clip<'_, W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if c == '\n' {
                if self.lines + 1 >= self.rows {
                    self.line.clear();
                    self.line.push_str("..");
                    self.stopped = true;
                    return Err(fmt::Error);
                }
                self.line.push('\n');
                self.out.write_str(&self.line)?;
                self.line.clear();
                self.lines += 1;
                self.width = 0;
                self.cut = false;
            } else if self.cut {
                continue;
            } else if self.width == self.columns {
                let keep = self.columns.saturating_sub(2);
                let end = self
                    .line
                    .char_indices()
                    .nth(keep)
                    .map_or(self.line.len(), |(i, _)| i);
                self.line.truncate(end);
                self.line.push_str("..");
                self.cut = true;
                if self.stop_at_cut {
                    self.stopped = true;
                    return Err(fmt::Error);
                }
            } else {
                self.line.push(c);
                self.width += 1;
            }
        }
        Ok(())
    }
}

/// `count` followed by `one` or `many`, for row and entry totals.
pub(super) fn total(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

impl Dictionary {
    /// Aligned key/value rows; with fewer console rows than entries, the first
    /// entries followed by `..` and the entry count.
    fn render(&self, f: &mut impl fmt::Write, rows: usize, columns: usize) -> fmt::Result {
        let (keys, values) = {
            let data = self.0.borrow();
            (data.keys.clone(), data.values.clone())
        };
        let count = keys.len();
        if count == 0 {
            return write!(f, "(()!())");
        }
        let shown = if count <= rows {
            count
        } else {
            rows.saturating_sub(2)
        };
        let entries: Vec<_> = keys
            .iter()
            .zip(values.iter())
            .take(shown)
            .map(|(key, value)| (cell(&key, columns), cell(&value, columns)))
            .collect();
        let width = entries
            .iter()
            .map(|(key, _)| key.chars().count())
            .max()
            .unwrap_or(0);
        for (i, (key, value)) in entries.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{key:<width$} | {value}")?;
        }
        if shown < count {
            if shown > 0 {
                writeln!(f)?;
            }
            write!(f, "..\n{}", total(count, "entry", "entries"))?;
        }
        Ok(())
    }
}

/// Render ordered key/value rows while keeping nested values in compact syntax.
impl fmt::Display for Dictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.render(f, usize::MAX, usize::MAX)
    }
}
