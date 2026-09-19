//! Name resolution between parsing and evaluation. Top-level code keeps names
//! for the persistent environment; each function body numbers its names as
//! frame slots and lists the outer names its closures capture.
use std::{collections::HashSet, rc::Rc};

use crate::{Array, Number, Result, Value, eval::arithmetic, parser::Expr, value::FunctionKind};

/// A name resolved for the code that uses it.
#[derive(Debug)]
pub(crate) enum Var {
    /// Top-level and dynamically evaluated code use an environment.
    Global(Rc<str>),
    /// Function bodies use a slot of the current call frame.
    Local(usize, Rc<str>),
}

impl Var {
    pub(crate) fn name(&self) -> &Rc<str> {
        match self {
            Self::Global(name) | Self::Local(_, name) => name,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Node {
    /// An immutable value shared by every evaluation.
    Const(Value),
    /// A literal vector. Each evaluation is a new array sharing this storage.
    Vector(Array),
    Text(Rc<Vec<u8>>),
    Strand(Vec<Node>),
    Array(Vec<Node>),
    /// The explicit value list of a dictionary, which may be mixed.
    Entries(Vec<Node>),
    Table(Vec<(Rc<str>, Node)>),
    Name(Var),
    Assign(Var, Box<Node>),
    Update(Var, Vec<Node>, Box<Node>),
    Append(Box<Node>, Box<Node>),
    Modify(Box<Node>, char, Box<Node>),
    Hole,
    Return(Box<Node>),
    Derived(char, Box<Node>),
    /// A primitive applied to both operands.
    Dyad(char, Box<(Node, Node)>),
    /// A unary primitive other than value applied to its operand.
    Monad(char, Box<Node>),
    /// Application; the flag records whether any argument is a hole.
    Call(Box<Node>, Vec<Node>, bool),
    If(Box<Node>, Box<Node>, Box<Node>),
    /// A function literal and where each of its captured names comes from.
    Lambda(Rc<Proto>, Vec<Var>),
}

/// A compiled function literal, shared by every closure created from it.
#[derive(Debug)]
pub(crate) struct Proto {
    /// Parameters occupy the first slots.
    pub(crate) params: usize,
    /// Every name the body binds or reads, indexed by slot.
    pub(crate) names: Vec<Rc<str>>,
    /// Slots filled from closure captures, in capture order.
    pub(crate) free: Vec<usize>,
    pub(crate) body: Vec<Node>,
    /// Bodies such as `x*y` apply their operator without a frame.
    pub(crate) arithmetic: Option<Arithmetic>,
    source: (Vec<String>, Vec<Expr>),
}

impl Proto {
    pub(crate) fn slot(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| &**n == name)
    }
    /// Functions compare structurally by parameters and body.
    pub(crate) fn same_source(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

#[derive(Debug)]
pub(crate) struct Arithmetic {
    pub(crate) operation: fn(&Number, &Number) -> Result<Number>,
    left: Operand,
    right: Operand,
}

#[derive(Debug)]
enum Operand {
    Number(Number),
    Param(usize),
}

impl Arithmetic {
    /// Both operands when they are numbers; otherwise the body runs normally.
    pub(crate) fn operands(&self, args: &[Value]) -> Option<(Number, Number)> {
        let operand = |operand: &Operand| match operand {
            Operand::Number(n) => Some(*n),
            Operand::Param(i) => match args[*i] {
                Value::Number(n) => Some(n),
                _ => None,
            },
        };
        Some((operand(&self.left)?, operand(&self.right)?))
    }
}

/// Compile top-level or dynamically evaluated code.
pub(crate) fn program(body: &[Expr]) -> Vec<Node> {
    let mut compiler = Compiler { slots: None };
    body.iter().map(|expr| compiler.expr(expr)).collect()
}

fn function(params: &[String], body: &[Expr]) -> Proto {
    let mut bound = params.iter().map(String::as_str).collect();
    let mut free = Vec::new();
    for expr in body {
        free_names(expr, &mut bound, &mut free);
    }
    let mut compiler = Compiler {
        slots: Some(params.iter().map(|p| Rc::from(p.as_str())).collect()),
    };
    let free = free.into_iter().map(|name| compiler.slot(name)).collect();
    let nodes = body.iter().map(|expr| compiler.expr(expr)).collect();
    Proto {
        params: params.len(),
        names: compiler.slots.unwrap_or_default(),
        free,
        body: nodes,
        arithmetic: arithmetic_body(params, body),
        source: (params.to_vec(), body.to_vec()),
    }
}

// Only immutable numeric leaves qualify: evaluating these cannot mutate bindings,
// invoke another function, or return early. Everything else uses the usual frame.
fn arithmetic_body(params: &[String], body: &[Expr]) -> Option<Arithmetic> {
    let [Expr::Call(function, operands)] = body else {
        return None;
    };
    let Expr::Verb(op) = **function else {
        return None;
    };
    let [left, right] = operands.as_slice() else {
        return None;
    };
    let operand = |expr: &Expr| match expr {
        Expr::Number(n) => Some(Operand::Number(*n)),
        Expr::Name(name) => params
            .iter()
            .rposition(|param| param == name)
            .map(Operand::Param),
        _ => None,
    };
    Some(Arithmetic {
        operation: arithmetic(op)?,
        left: operand(left)?,
        right: operand(right)?,
    })
}

struct Compiler {
    /// Slot names of the function being compiled; None at top level.
    slots: Option<Vec<Rc<str>>>,
}

impl Compiler {
    fn slot(&mut self, name: &str) -> usize {
        let names = self.slots.as_mut().expect("function scope");
        names.iter().position(|n| &**n == name).unwrap_or_else(|| {
            names.push(Rc::from(name));
            names.len() - 1
        })
    }
    fn var(&mut self, name: &str) -> Var {
        match &self.slots {
            None => Var::Global(Rc::from(name)),
            Some(_) => {
                let slot = self.slot(name);
                Var::Local(slot, self.slots.as_ref().unwrap()[slot].clone())
            }
        }
    }
    fn exprs(&mut self, exprs: &[Expr]) -> Vec<Node> {
        exprs.iter().map(|expr| self.expr(expr)).collect()
    }
    fn expr(&mut self, expr: &Expr) -> Node {
        match expr {
            Expr::Number(n) => Node::Const(Value::from_number(*n)),
            Expr::Null => Node::Const(Value::Null),
            Expr::Text(text) => match text.as_slice() {
                [c] => Node::Const(Value::Char(*c)),
                _ => Node::Text(Rc::new(text.clone())),
            },
            Expr::Symbol(name) => Node::Const(Value::Symbol(name.clone())),
            Expr::Strand(xs) => {
                let nodes = self.exprs(xs);
                constants(&nodes)
                    .and_then(|values| vector(strand(values)))
                    .map_or(Node::Strand(nodes), Node::Vector)
            }
            Expr::Array(xs) => {
                let nodes = self.exprs(xs);
                constants(&nodes)
                    .and_then(|values| vector(Array::new(values).map(Value::from_array)))
                    .map_or(Node::Array(nodes), Node::Vector)
            }
            Expr::Table(columns) => Node::Table(
                columns
                    .iter()
                    .map(|(name, expr)| (Rc::from(name.as_str()), self.expr(expr)))
                    .collect(),
            ),
            Expr::Name(name) => Node::Name(self.var(name)),
            Expr::Assign(name, value) => Node::Assign(self.var(name), Box::new(self.expr(value))),
            Expr::Update(name, indices, value) => Node::Update(
                self.var(name),
                self.exprs(indices),
                Box::new(self.expr(value)),
            ),
            Expr::Append(target, value) => {
                Node::Append(Box::new(self.expr(target)), Box::new(self.expr(value)))
            }
            Expr::Modify(target, op, value) => Node::Modify(
                Box::new(self.target(target)),
                *op,
                Box::new(self.expr(value)),
            ),
            Expr::Hole => Node::Hole,
            Expr::Return(value) => Node::Return(Box::new(self.expr(value))),
            Expr::Verb(op) => Node::Const(Value::function(FunctionKind::Verb(*op))),
            Expr::Monadic(op) => Node::Const(Value::function(FunctionKind::Monadic(*op))),
            Expr::Derived(adverb, function) => match self.expr(function) {
                Node::Const(function @ Value::Function(_)) => {
                    Node::Const(Value::function(FunctionKind::Derived(*adverb, function)))
                }
                function => Node::Derived(*adverb, Box::new(function)),
            },
            Expr::Call(function, args) => self.call(function, args),
            Expr::If(condition, yes, no) => Node::If(
                Box::new(self.expr(condition)),
                Box::new(self.expr(yes)),
                Box::new(self.expr(no)),
            ),
            Expr::Lambda(params, body) => {
                let proto = function(params, body);
                let sources = proto
                    .free
                    .iter()
                    .map(|&slot| self.var(&proto.names[slot]))
                    .collect();
                Node::Lambda(Rc::new(proto), sources)
            }
        }
    }
    fn call(&mut self, function: &Expr, args: &[Expr]) -> Node {
        let holes = args.iter().any(|arg| matches!(arg, Expr::Hole));
        let dictionary = matches!(function, Expr::Verb('!')) && args.len() == 2;
        let mut nodes = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            nodes.push(match arg {
                Expr::Array(entries) if dictionary && index == 1 => {
                    Node::Entries(self.exprs(entries))
                }
                arg => self.expr(arg),
            });
        }
        if !holes {
            match (function, nodes.len()) {
                (Expr::Verb(op), 2) => {
                    let y = nodes.pop().unwrap();
                    let x = nodes.pop().unwrap();
                    return Node::Dyad(*op, Box::new((x, y)));
                }
                (Expr::Monadic(op), 1) if *op != '.' => {
                    return Node::Monad(*op, Box::new(nodes.pop().unwrap()));
                }
                _ => {}
            }
        }
        Node::Call(Box::new(self.expr(function)), nodes, holes)
    }
    // Modified assignment walks the application spine of its target to find
    // selectors, so the spine keeps its general call form.
    fn target(&mut self, expr: &Expr) -> Node {
        match expr {
            Expr::Call(function, args) => Node::Call(
                Box::new(self.target(function)),
                self.exprs(args),
                args.iter().any(|arg| matches!(arg, Expr::Hole)),
            ),
            expr => self.expr(expr),
        }
    }
}

fn constants(nodes: &[Node]) -> Option<Vec<Value>> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Const(value) => Some(value.clone()),
            _ => None,
        })
        .collect()
}

/// A numeric strand promotes every number to float when any element is a float.
pub(crate) fn strand(mut values: Vec<Value>) -> Result<Value> {
    if values
        .iter()
        .any(|v| matches!(v, Value::Number(Number::Float(_))))
    {
        for value in &mut values {
            if let Value::Number(n) = value {
                *n = Number::Float(n.as_f64());
            }
        }
    }
    Value::array(values)
}

fn vector(value: Result<Value>) -> Option<Array> {
    match value {
        Ok(Value::Array(array)) => Some(array),
        _ => None,
    }
}

// Track definitely assigned names in evaluation order. Nested functions need
// their own bindings, but their free names must survive in the outer closure.
fn free_names<'a>(expr: &'a Expr, bound: &mut HashSet<&'a str>, free: &mut Vec<&'a str>) {
    match expr {
        Expr::Name(name) => read(name, bound, free),
        Expr::Assign(name, value) => {
            free_names(value, bound, free);
            bound.insert(name);
        }
        Expr::Update(name, indices, value) => {
            free_names(value, bound, free);
            for index in indices.iter().rev() {
                free_names(index, bound, free);
            }
            read(name, bound, free);
        }
        Expr::Append(target, value) | Expr::Modify(target, _, value) => {
            free_names(value, bound, free);
            free_names(target, bound, free);
        }
        Expr::Table(columns) => {
            for (_, value) in columns.iter().rev() {
                free_names(value, bound, free);
            }
        }
        Expr::Array(values) | Expr::Strand(values) => {
            for value in values.iter().rev() {
                free_names(value, bound, free);
            }
        }
        Expr::Call(function, args) => {
            for arg in args.iter().rev() {
                free_names(arg, bound, free);
            }
            free_names(function, bound, free);
        }
        Expr::Derived(_, function) | Expr::Return(function) => free_names(function, bound, free),
        Expr::If(condition, yes, no) => {
            free_names(condition, bound, free);
            let mut yes_bound = bound.clone();
            free_names(yes, &mut yes_bound, free);
            free_names(no, bound, free);
            bound.retain(|name| yes_bound.contains(name));
        }
        Expr::Lambda(params, body) => {
            let mut local = bound.clone();
            local.extend(params.iter().map(String::as_str));
            for expr in body {
                free_names(expr, &mut local, free);
            }
        }
        Expr::Null
        | Expr::Number(_)
        | Expr::Text(_)
        | Expr::Hole
        | Expr::Symbol(_)
        | Expr::Verb(_)
        | Expr::Monadic(_) => {}
    }
}

fn read<'a>(name: &'a str, bound: &HashSet<&'a str>, free: &mut Vec<&'a str>) {
    if !bound.contains(name) && !free.contains(&name) {
        free.push(name);
    }
}
