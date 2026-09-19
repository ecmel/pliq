mod arrow;
mod compile;
mod eval;
mod number;
mod parser;
mod value;

pub use eval::Interpreter;
pub use number::Number;
pub use value::{Array, Console, Dictionary, Table, Value, View};

pub type Result<T> = std::result::Result<T, String>;

mod operators;

mod string;
pub use string::MutableString;
