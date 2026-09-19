use crate::Result;
use std::{cell::RefCell, fmt, rc::Rc};

/// A mutable byte string. Clones share identity; snapshots survive subsequent edits.
/// Indices address bytes rather than Unicode code points.
#[derive(Clone, Debug)]
pub struct MutableString(pub(crate) Rc<RefCell<Rc<Vec<u8>>>>);

impl MutableString {
    pub fn new(bytes: impl AsRef<[u8]>) -> Self {
        Self(Rc::new(RefCell::new(Rc::new(bytes.as_ref().to_vec()))))
    }
    /// A new string sharing bytes until either side is edited.
    pub(crate) fn shared(bytes: Rc<Vec<u8>>) -> Self {
        Self(Rc::new(RefCell::new(bytes)))
    }
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn snapshot(&self) -> Rc<Vec<u8>> {
        self.0.borrow().clone()
    }
    pub fn get(&self, index: usize) -> Option<u8> {
        self.0.borrow().get(index).copied()
    }
    pub fn set(&self, index: usize, byte: u8) -> Result<()> {
        if index >= self.len() {
            return Err("string index out of bounds".into());
        }
        Rc::make_mut(&mut self.0.borrow_mut())[index] = byte;
        Ok(())
    }
    pub fn append(&self, bytes: impl AsRef<[u8]>) {
        Rc::make_mut(&mut self.0.borrow_mut()).extend_from_slice(bytes.as_ref());
    }
    pub(crate) fn detached(&self) -> Self {
        Self::new(self.snapshot().as_slice())
    }
}
impl fmt::Display for MutableString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        for byte in self.snapshot().iter() {
            match byte {
                b'"' => f.write_str("\\\"")?,
                b'\\' => f.write_str("\\\\")?,
                b'\n' => f.write_str("\\n")?,
                b'\r' => f.write_str("\\r")?,
                b'\t' => f.write_str("\\t")?,
                32..=126 => write!(f, "{}", char::from(*byte))?,
                _ => write!(f, "\\x{byte:02x}")?,
            }
        }
        f.write_str("\"")
    }
}
