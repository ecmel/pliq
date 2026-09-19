use std::{collections::HashMap, rc::Rc};

use crate::{
    Array, Dictionary, MutableString, Number, Result, Table, Value,
    arrow::Operand,
    compile::{self, Node, Var},
    parser,
    value::{ArrayBuilder, Environment, Function, FunctionKind},
};

/// A persistent environment. Each evaluation returns the last statement's value.
pub struct Interpreter {
    env: Environment,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self {
            env: Rc::new(builtins()),
        }
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn eval(&mut self, source: &str) -> Result<Value> {
        let body = compile::program(&parser::parse(source)?);
        let mut evaluator = Evaluator::default();
        let result = evaluator.body(&body, &mut Scope::Global(&mut self.env));
        evaluator.returning.take().map_or(result, Ok)
    }
}

#[derive(Default)]
pub(crate) struct Evaluator {
    depth: usize,
    pub(crate) returning: Option<Value>,
    /// Slots of the active call frames, innermost last.
    stack: Vec<Option<Value>>,
    /// Names that dynamic evaluation bound in each frame without a slot.
    frames: Vec<Option<HashMap<String, Value>>>,
}

/// Where names resolve while evaluating compiled nodes.
pub(crate) enum Scope<'a> {
    Global(&'a mut Environment),
    /// A call frame whose slots start at `base` in the evaluator's stack.
    Local {
        base: usize,
        frame: usize,
        function: &'a Rc<Function>,
    },
}

pub(crate) type Arithmetic = fn(&Number, &Number) -> Result<Number>;

pub(crate) fn arithmetic(op: char) -> Option<Arithmetic> {
    match op {
        '+' => Some(Number::add),
        '-' => Some(Number::subtract),
        '*' => Some(Number::multiply),
        '%' => Some(Number::divide),
        _ => None,
    }
}

fn undefined(name: &str) -> String {
    format!("undefined name: {name}")
}

/// Assigning an anonymous function names it. Calls then bind the name to the
/// function itself for recursion, so the closure drops any captured value.
fn name_function(value: Value, name: &Rc<str>) -> Value {
    if let Value::Function(f) = &value
        && let FunctionKind::User {
            name: None,
            proto,
            captures,
            ..
        } = &f.kind
    {
        let mut captures = captures.clone();
        if let Some(i) = proto
            .free
            .iter()
            .position(|&slot| proto.names[slot] == *name)
        {
            captures[i] = None;
        }
        return Value::function(FunctionKind::User {
            name: Some(name.clone()),
            proto: proto.clone(),
            captures,
            self_slot: proto.slot(name).filter(|&slot| slot >= proto.params),
        });
    }
    value
}

impl Evaluator {
    pub(crate) fn body(&mut self, body: &[Node], scope: &mut Scope) -> Result<Value> {
        let Some((last, init)) = body.split_last() else {
            return Value::array([]);
        };
        for node in init {
            self.eval(node, scope)?;
        }
        self.eval(last, scope)
    }
    fn eval(&mut self, node: &Node, scope: &mut Scope) -> Result<Value> {
        match node {
            Node::Const(value) => Ok(value.clone()),
            Node::Name(var) => self.read(var, scope),
            Node::Dyad(op, operands) => {
                let y = self.eval(&operands.1, scope)?;
                let x = self.eval(&operands.0, scope)?;
                self.nested(|this| this.dyad(*op, &x, &y))
            }
            Node::Monad(op, operand) => {
                let y = self.eval(operand, scope)?;
                self.nested(|this| this.monad(*op, &y))
            }
            Node::Call(function, args, false) => match args.as_slice() {
                [] => {
                    let function = self.eval(function, scope)?;
                    self.call(&function, &[], scope)
                }
                [x] => {
                    let x = self.eval(x, scope)?;
                    let function = self.eval(function, scope)?;
                    self.call(&function, &[x], scope)
                }
                [x, y] => {
                    let y = self.eval(y, scope)?;
                    let x = self.eval(x, scope)?;
                    let function = self.eval(function, scope)?;
                    self.call(&function, &[x, y], scope)
                }
                args => {
                    let mut values = Vec::with_capacity(args.len());
                    for arg in args.iter().rev() {
                        values.push(self.eval(arg, scope)?);
                    }
                    values.reverse();
                    let function = self.eval(function, scope)?;
                    self.call(&function, &values, scope)
                }
            },
            Node::Call(function, args, true) => self.project(function, args, scope),
            Node::If(condition, yes, no) => {
                let condition = self.eval(condition, scope)?;
                self.eval(
                    if matches!(condition, Value::Null) || !number(&condition)?.is_zero() {
                        yes
                    } else {
                        no
                    },
                    scope,
                )
            }
            Node::Assign(var, value) => {
                let value = name_function(self.eval(value, scope)?, var.name());
                self.store(var, value.clone(), scope);
                Ok(value)
            }
            Node::Lambda(proto, sources) => Ok(Value::function(FunctionKind::User {
                name: None,
                proto: proto.clone(),
                captures: sources.iter().map(|var| self.lookup(var, scope)).collect(),
                self_slot: None,
            })),
            Node::Vector(array) => Ok(Value::Array(array.snapshot())),
            Node::Text(text) => Ok(Value::String(MutableString::shared(text.clone()))),
            Node::Return(value) => {
                let value = self.eval(value, scope)?;
                self.returning = Some(value);
                // The enclosing call takes the value; this message is never shown.
                Err(String::new())
            }
            Node::Derived(adverb, function) => {
                let function = self.eval(function, scope)?;
                if !matches!(function, Value::Function(_)) {
                    return Err("adverbs require a function".into());
                }
                Ok(Value::function(FunctionKind::Derived(*adverb, function)))
            }
            Node::Hole => Err("argument placeholder outside a call".into()),
            Node::Strand(xs) => compile::strand(
                xs.iter()
                    .map(|x| self.eval(x, scope))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Node::Array(xs) => {
                let values = self.values(xs, scope)?;
                Ok(Value::from_array(Array::new(values)?))
            }
            Node::Entries(xs) => {
                let values = self.values(xs, scope)?;
                Ok(Value::Array(Array::dictionary_values(values)))
            }
            Node::Table(columns) => self.table(columns, scope),
            Node::Update(var, indices, value) => self.update(var, indices, value, scope),
            Node::Append(target, value) => self.append(target, value, scope),
            Node::Modify(target, op, value) => self.modify(target, *op, value, scope),
        }
    }
    /// Evaluate list elements right to left, returning them in list order.
    fn values(&mut self, xs: &[Node], scope: &mut Scope) -> Result<Vec<Value>> {
        let mut values = Vec::with_capacity(xs.len());
        for x in xs.iter().rev() {
            values.push(self.eval(x, scope)?);
        }
        values.reverse();
        Ok(values)
    }
    fn lookup(&self, var: &Var, scope: &Scope) -> Option<Value> {
        match (var, scope) {
            (Var::Local(slot, _), Scope::Local { base, .. }) => self.stack[base + slot].clone(),
            (Var::Global(name), Scope::Global(env)) => env.get(&**name).cloned(),
            _ => unreachable!("names resolve in the scope that compiled them"),
        }
    }
    fn read(&self, var: &Var, scope: &Scope) -> Result<Value> {
        self.lookup(var, scope).ok_or_else(|| undefined(var.name()))
    }
    fn store(&mut self, var: &Var, value: Value, scope: &mut Scope) {
        match (var, scope) {
            (Var::Local(slot, _), Scope::Local { base, .. }) => {
                self.stack[*base + slot] = Some(value);
            }
            (Var::Global(name), Scope::Global(env)) => {
                Rc::make_mut(env).insert(name.to_string(), value);
            }
            _ => unreachable!("names resolve in the scope that compiled them"),
        }
    }
    /// Look up a name given as data, as `value` and amend by symbol do.
    fn lookup_name(&self, name: &str, scope: &Scope) -> Option<Value> {
        match scope {
            Scope::Global(env) => env.get(name).cloned(),
            Scope::Local {
                base,
                frame,
                function,
            } => {
                let FunctionKind::User {
                    name: own, proto, ..
                } = &function.kind
                else {
                    unreachable!("frames belong to user functions")
                };
                if let Some(slot) = proto.slot(name) {
                    return self.stack[base + slot].clone();
                }
                self.frames[*frame]
                    .as_ref()
                    .and_then(|extra| extra.get(name).cloned())
                    .or_else(|| {
                        (own.as_deref() == Some(name)).then(|| Value::Function(Rc::clone(function)))
                    })
            }
        }
    }
    fn assign_name(&mut self, name: &str, value: Value, scope: &mut Scope) {
        match scope {
            Scope::Global(env) => {
                Rc::make_mut(env).insert(name.to_owned(), value);
            }
            Scope::Local {
                base,
                frame,
                function,
            } => {
                let FunctionKind::User { proto, .. } = &function.kind else {
                    unreachable!("frames belong to user functions")
                };
                match proto.slot(name) {
                    Some(slot) => self.stack[*base + slot] = Some(value),
                    None => {
                        self.frames[*frame]
                            .get_or_insert_default()
                            .insert(name.to_owned(), value);
                    }
                }
            }
        }
    }
    /// Evaluate source text in the current scope. Inside a function, the frame's
    /// bindings form an environment for the evaluation and are then written back.
    fn evaluate(&mut self, source: &str, scope: &mut Scope) -> Result<Value> {
        let body = compile::program(&parser::parse(source)?);
        let (base, frame, function) = match scope {
            Scope::Global(env) => return self.body(&body, &mut Scope::Global(env)),
            Scope::Local {
                base,
                frame,
                function,
            } => (*base, *frame, *function),
        };
        let FunctionKind::User { name, proto, .. } = &function.kind else {
            unreachable!("frames belong to user functions")
        };
        let mut env = HashMap::new();
        // Parameters and assignments shadow the function's own name.
        if let Some(name) = name {
            env.insert(name.to_string(), Value::Function(function.clone()));
        }
        for (name, value) in proto.names.iter().zip(&self.stack[base..]) {
            if let Some(value) = value {
                env.insert(name.to_string(), value.clone());
            }
        }
        let mut extra = self.frames[frame].take().unwrap_or_default();
        env.extend(extra.drain());
        let mut env = Rc::new(env);
        let result = self.body(&body, &mut Scope::Global(&mut env));
        for (name, value) in Rc::unwrap_or_clone(env) {
            match proto.slot(&name) {
                Some(slot) => self.stack[base + slot] = Some(value),
                None => {
                    extra.insert(name, value);
                }
            }
        }
        self.frames[frame] = Some(extra);
        result
    }
    fn call(&mut self, function: &Value, args: &[Value], scope: &mut Scope) -> Result<Value> {
        if let Value::Function(f) = function {
            match (&f.kind, args) {
                (FunctionKind::Monadic('.') | FunctionKind::Native("value"), [arg]) => {
                    if let Value::Symbol(name) = arg {
                        return self.lookup_name(name, scope).ok_or_else(|| undefined(name));
                    }
                    if let Some(source) = arg.as_text() {
                        return self.evaluate(&source, scope);
                    }
                }
                (
                    FunctionKind::Verb(op @ ('@' | '.')),
                    [Value::Symbol(name), selector, f, rest @ ..],
                ) if rest.len() <= 1 => {
                    return self.amend_name(*op, name, selector, f, rest.first(), scope);
                }
                _ => {}
            }
        }
        self.apply(function, args)
    }
    fn amend_name(
        &mut self,
        op: char,
        name: &Rc<str>,
        selector: &Value,
        f: &Value,
        y: Option<&Value>,
        scope: &mut Scope,
    ) -> Result<Value> {
        let target = self
            .lookup_name(name, scope)
            .ok_or_else(|| undefined(name))?;
        let amended = self.amend_or_trap(op, &target, selector, f, y)?;
        let selectors = if op == '.' {
            items(selector).iter().collect::<Vec<_>>()
        } else {
            vec![selector.clone()]
        };
        if selectors.is_empty() {
            self.assign_name(name, amended, scope);
        } else {
            target.update(&selectors, &index_depth(&amended, &selectors)?)?;
        }
        Ok(Value::Symbol(name.clone()))
    }
    /// Apply with argument holes: index containers or project functions.
    fn project(&mut self, function: &Node, args: &[Node], scope: &mut Scope) -> Result<Value> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args.iter().rev() {
            values.push(match arg {
                Node::Hole => None,
                arg => Some(self.eval(arg, scope)?),
            });
        }
        values.reverse();
        let function = self.eval(function, scope)?;
        if matches!(
            function,
            Value::Array(_) | Value::String(_) | Value::Dictionary(_) | Value::Table(_)
        ) {
            let indices = values
                .into_iter()
                .map(|v| v.unwrap_or_else(|| Value::function(FunctionKind::Verb(':'))))
                .collect::<Vec<_>>();
            index_depth(&function, &indices)
        } else {
            Ok(Value::function(FunctionKind::Projection(function, values)))
        }
    }
    fn table(&mut self, columns: &[(Rc<str>, Node)], scope: &mut Scope) -> Result<Value> {
        let mut values = Vec::with_capacity(columns.len());
        for (name, node) in columns.iter().rev() {
            let Value::Array(column) = self.eval(node, scope)? else {
                return Err(format!("table column {name} must be an array"));
            };
            values.push((name.clone(), column));
        }
        values.reverse();
        Table::new(values).map(Value::Table)
    }
    fn update(
        &mut self,
        var: &Var,
        indices: &[Node],
        value: &Node,
        scope: &mut Scope,
    ) -> Result<Value> {
        let value = self.eval(value, scope)?;
        let mut selectors = Vec::with_capacity(indices.len());
        for index in indices.iter().rev() {
            selectors.push(self.eval(index, scope)?);
        }
        selectors.reverse();
        self.read(var, scope)?.update(&selectors, &value)?;
        Ok(value)
    }
    fn append(&mut self, target: &Node, value: &Node, scope: &mut Scope) -> Result<Value> {
        let value = self.eval(value, scope)?;
        let target = self.eval(target, scope)?;
        match &target {
            Value::Array(array) => array.append(value)?,
            Value::Table(table) => match value {
                Value::Table(source) => table.append(&source)?,
                _ => return Err("table append requires a table".into()),
            },
            Value::String(string) => {
                let bytes = match value {
                    Value::Char(c) => vec![c],
                    Value::String(s) => s.snapshot().as_ref().clone(),
                    _ => return Err("string append requires a character or string".into()),
                };
                string.append(bytes);
            }
            _ => return Err("in-place append requires an array or string".into()),
        }
        Ok(target)
    }
    fn modify(
        &mut self,
        target: &Node,
        op: char,
        value: &Node,
        scope: &mut Scope,
    ) -> Result<Value> {
        let right = self.eval(value, scope)?;
        let mut base = target;
        let mut selectors = Vec::new();
        while let Node::Call(function, args, _) = base {
            for arg in args.iter().rev() {
                selectors.push(self.eval(arg, scope)?);
            }
            base = function;
        }
        selectors.reverse();
        let Node::Name(var) = base else {
            return Err("assignment target must be a variable or indexed variable".into());
        };
        let container = self.read(var, scope)?;
        if selectors.is_empty() {
            let result = self.dyad(op, &container, &right)?;
            self.store(var, result.clone(), scope);
            Ok(result)
        } else {
            let amended = container.detached();
            self.amend_path(
                &amended,
                &selectors,
                &Value::function(FunctionKind::Verb(op)),
                Some(&right),
            )?;
            let replacement = index_depth(&amended, &selectors)?;
            container.update(&selectors, &replacement)?;
            Ok(replacement)
        }
    }
    /// Function and primitive applications share one depth limit.
    fn nested(&mut self, run: impl FnOnce(&mut Self) -> Result<Value>) -> Result<Value> {
        if self.depth >= 128 {
            return Err("function call depth limit exceeded".into());
        }
        self.depth += 1;
        let result = run(self);
        self.depth -= 1;
        result
    }
    pub(crate) fn apply(&mut self, function: &Value, args: &[Value]) -> Result<Value> {
        self.nested(|this| this.apply_inner(function, args))
    }
    fn apply_inner(&mut self, function: &Value, args: &[Value]) -> Result<Value> {
        if let Value::Dictionary(dictionary) = function {
            return match args {
                [] => Ok(Value::Dictionary(dictionary.clone())),
                _ => index_depth(&Value::Dictionary(dictionary.clone()), args),
            };
        }
        let Value::Function(function) = function else {
            return match args {
                [] => Ok(function.clone()),
                _ => index_depth(function, args),
            };
        };
        match &function.kind {
            FunctionKind::Verb(':') => match args {
                [x] => Ok(x.clone()),
                [_, y] => Ok(y.clone()),
                _ => Err("identity expects one or two arguments".into()),
            },
            FunctionKind::Verb(op) => match args {
                [y] => Ok(Value::function(FunctionKind::Projection(
                    Value::Function(function.clone()),
                    vec![Some(y.clone()), None],
                ))),
                [x, y] => self.dyad(*op, x, y),
                [x, i, f] if matches!(op, '@' | '.') => self.amend_or_trap(*op, x, i, f, None),
                [x, i, f, y] if matches!(op, '@' | '.') => {
                    self.amend_or_trap(*op, x, i, f, Some(y))
                }
                [c, x, y] if *op == '?' => self.choose(c, x, y),
                _ => Err(format!("{op} expects one or two arguments")),
            },
            FunctionKind::Native(name) => self.native(name, args),
            FunctionKind::Monadic(op) => match args {
                [y] => self.monad(*op, y),
                _ => Err("unary primitive expects one argument".into()),
            },
            FunctionKind::Projection(f, slots) => {
                let mut remaining = args.iter();
                let mut filled = slots.clone();
                for slot in &mut filled {
                    if slot.is_none() {
                        *slot = remaining.next().cloned();
                    }
                }
                if remaining.next().is_some() {
                    return Err("too many projection arguments".into());
                }
                if filled.iter().any(Option::is_none) {
                    Ok(Value::function(FunctionKind::Projection(f.clone(), filled)))
                } else {
                    self.apply(f, &filled.into_iter().flatten().collect::<Vec<_>>())
                }
            }
            FunctionKind::Composition(f, g) => {
                let value = self.apply(g, args)?;
                self.apply(f, &[value])
            }
            FunctionKind::Round => {
                let [value, places] = args else {
                    return Err("round expects [value;places]".into());
                };
                let places = number(places)?
                    .to_isize()
                    .filter(|p| (0..=308).contains(p))
                    .ok_or("round places must be an integer between 0 and 308")?;
                self.round(value, places as u32)
            }
            FunctionKind::User {
                proto,
                captures,
                self_slot,
                ..
            } => {
                if args.len() < proto.params {
                    let mut slots = args.iter().cloned().map(Some).collect::<Vec<_>>();
                    slots.resize(proto.params, None);
                    return Ok(Value::function(FunctionKind::Projection(
                        Value::Function(function.clone()),
                        slots,
                    )));
                }
                if proto.params != args.len() {
                    return Err(format!(
                        "function expects {} arguments, got {}",
                        proto.params,
                        args.len()
                    ));
                }
                if let Some(arithmetic) = &proto.arithmetic
                    && let Some((x, y)) = arithmetic.operands(args)
                {
                    // The lambda call already consumed one depth level. Its
                    // primitive call consumes another, even without a frame.
                    if self.depth >= 128 {
                        return Err("function call depth limit exceeded".into());
                    }
                    return (arithmetic.operation)(&x, &y).map(Value::from_number);
                }
                let base = self.stack.len();
                self.stack.extend(args.iter().cloned().map(Some));
                self.stack.resize(base + proto.names.len(), None);
                for (&slot, value) in proto.free.iter().zip(captures) {
                    self.stack[base + slot] = value.clone();
                }
                if let Some(slot) = self_slot {
                    self.stack[base + slot] = Some(Value::Function(function.clone()));
                }
                let frame = self.frames.len();
                self.frames.push(None);
                let result = self.body(
                    &proto.body,
                    &mut Scope::Local {
                        base,
                        frame,
                        function,
                    },
                );
                self.frames.pop();
                self.stack.truncate(base);
                self.returning.take().map_or(result, Ok)
            }
            FunctionKind::Derived(adverb, f) => match adverb {
                '/' | '\\' => self.fold(f, args, *adverb == '\\'),
                '\'' if matches!(args, [Value::Function(_)]) => Ok(Value::function(
                    FunctionKind::Composition(f.clone(), args[0].clone()),
                )),
                '\'' => {
                    let function = if args.len() == 1 {
                        match f {
                            Value::Function(f) => match f.kind {
                                FunctionKind::Verb(op) => {
                                    Value::function(FunctionKind::Monadic(op))
                                }
                                _ => Value::Function(f.clone()),
                            },
                            _ => f.clone(),
                        }
                    } else {
                        f.clone()
                    };
                    self.each(&function, args)
                }
                'R' | 'L' | 'P' => self.iterate(*adverb, f, args),
                _ => unreachable!(),
            },
        }
    }
    fn fold(&mut self, f: &Value, args: &[Value], scan: bool) -> Result<Value> {
        if unary_function(f) {
            return self.unary_fold(f, args, scan);
        }
        let (seed, input) = match args {
            [y] => (None, y),
            [x, y] => (Some(x.clone()), y),
            _ => {
                return Err(
                    "reduce and scan expect an array, optionally preceded by a seed".into(),
                );
            }
        };
        if let Value::Function(function) = f
            && let FunctionKind::Verb(op) = function.kind
            && let Some(operation) = arithmetic(op)
            && let Value::Array(array) = input
            && let Some(numbers) = array.numeric_snapshot()
            && numbers.iter().all(|n| n.is_some())
            && (seed.is_none() || matches!(seed, Some(Value::Number(_))))
        {
            let mut iter = numbers.iter().map(Option::unwrap);
            let mut output = Vec::with_capacity(if scan { numbers.len() } else { 0 });
            let mut acc = if let Some(Value::Number(seed)) = seed {
                seed
            } else if let Some(first) = iter.next() {
                if scan {
                    output.push(first);
                }
                first
            } else {
                return Value::array([]);
            };
            if iter.len() > 0 && self.depth >= 128 {
                return Err("function call depth limit exceeded".into());
            }
            for number in iter {
                acc = operation(&acc, &number)?;
                if scan {
                    output.push(acc);
                }
            }
            return Ok(if scan {
                Value::Array(Array::numbers(output))
            } else {
                Value::from_number(acc)
            });
        }
        let xs = items(input);
        let mut iter = xs.iter();
        let mut output = ArrayBuilder::new(if scan { xs.len() } else { 0 });
        let mut acc = if let Some(seed) = seed {
            seed
        } else if let Some(first) = iter.next() {
            if scan {
                output.push(first.clone());
            }
            first
        } else {
            // An unseeded reduction or scan of no elements returns an empty vector.
            return Value::array([]);
        };
        for x in iter {
            acc = self.apply(f, &[acc, x])?;
            if scan {
                output.push(acc.clone());
            }
        }
        if scan && matches!(input, Value::Array(_) | Value::String(_)) {
            Ok(Value::Array(output.finish()?))
        } else {
            Ok(acc)
        }
    }
    fn each(&mut self, f: &Value, args: &[Value]) -> Result<Value> {
        if args.iter().any(|v| matches!(v, Value::String(_))) {
            let args = args
                .iter()
                .map(|v| {
                    v.string_items()
                        .map(Value::Array)
                        .unwrap_or_else(|| v.clone())
                })
                .collect::<Vec<_>>();
            return self.each(f, &args);
        }
        let result_array = |values| {
            if args
                .iter()
                .any(|v| matches!(v, Value::Array(a) if a.is_mixed()))
            {
                Ok(Array::dictionary_values(values))
            } else {
                Array::new(values)
            }
        };
        match args {
            [Value::Array(xs)] => xs
                .iter()
                .map(|x| self.apply(f, &[x]))
                .collect::<Result<Vec<_>>>()
                .and_then(result_array)
                .map(Value::from_array),
            [x] => self.apply(f, std::slice::from_ref(x)),
            [Value::Array(xs), Value::Array(ys)] => {
                if xs.len() != ys.len() {
                    return Err("length mismatch in each".into());
                }
                xs.iter()
                    .zip(ys.iter())
                    .map(|(x, y)| self.apply(f, &[x, y]))
                    .collect::<Result<Vec<_>>>()
                    .and_then(result_array)
                    .map(Value::from_array)
            }
            [Value::Array(xs), y] => xs
                .iter()
                .map(|x| self.apply(f, &[x, y.clone()]))
                .collect::<Result<Vec<_>>>()
                .and_then(result_array)
                .map(Value::from_array),
            [x, Value::Array(ys)] => ys
                .iter()
                .map(|y| self.apply(f, &[x.clone(), y]))
                .collect::<Result<Vec<_>>>()
                .and_then(result_array)
                .map(Value::from_array),
            [x, y] => self.apply(f, &[x.clone(), y.clone()]),
            _ => Err("each expects one or two arguments".into()),
        }
    }
    fn unary_numeric(&mut self, op: char, y: &Value) -> Result<Value> {
        // Empty and all-null results keep the untyped form element-wise
        // evaluation gives them.
        if let Value::Array(xs) = y
            && let Some(numbers) = xs.numeric_snapshot()
            && let Some(result) = crate::arrow::monad(op, &numbers)
            && result.null_count() < result.len()
        {
            return Ok(Value::Array(Array::from_numbers(result)));
        }
        match y {
            Value::Array(xs) => xs
                .iter()
                .map(|x| self.unary_numeric(op, &x))
                .collect::<Result<Vec<_>>>()
                .and_then(|values| {
                    if xs.is_mixed() {
                        Ok(Array::dictionary_values(values))
                    } else {
                        Array::new(values)
                    }
                })
                .map(Value::from_array),
            Value::Symbol(s) if op == '_' => Ok(Value::Symbol(Rc::from(s.to_ascii_lowercase()))),
            Value::Char(c) if op == '_' => Ok(Value::Char(c.to_ascii_lowercase())),
            Value::Char(_) if op == '~' => Ok(boolean(false)),
            Value::Null => Ok(if op == '~' {
                boolean(false)
            } else {
                Value::Null
            }),
            Value::Dictionary(d) => Dictionary::from_arrays(
                d.keys(),
                items(&self.unary_numeric(op, &Value::Array(d.values()))?),
            )
            .map(Value::Dictionary),
            Value::Number(n) => Ok(Value::from_number(match op {
                '+' => *n,
                '-' => n.negated()?,
                '%' => Number::integer(1).divide(n)?,
                '_' => n.floor()?,
                '~' => return Ok(boolean(n.is_zero())),
                _ => unreachable!(),
            })),
            _ => Err(format!("{op} requires numbers")),
        }
    }
    fn round(&mut self, value: &Value, places: u32) -> Result<Value> {
        match value {
            Value::Null => Ok(Value::Null),
            Value::Number(n) => n.round(places).map(Value::from_number),
            Value::Array(xs) => xs
                .iter()
                .map(|x| self.round(&x, places))
                .collect::<Result<Vec<_>>>()
                .and_then(Array::new)
                .map(Value::from_array),
            _ => Err("round requires numbers".into()),
        }
    }
    pub(crate) fn monad(&mut self, op: char, y: &Value) -> Result<Value> {
        if let Value::String(s) = y {
            match op {
                '@' => return Ok(Value::Number(Number::Int(10))),
                '!' => return Ok(Value::Symbol(Rc::from("char"))),
                '*' if s.is_empty() => return Ok(Value::Null),
                ',' => return Ok(Value::Array(Array::new([y.clone()])?)),
                _ => {}
            }
            let result = self.monad(op, &Value::Array(y.string_items().unwrap()))?;
            return Ok(if matches!(op, '|' | '_' | '?') {
                Value::text(
                    items(&result)
                        .iter()
                        .map(|v| {
                            if let Value::Char(c) = v {
                                c
                            } else {
                                unreachable!()
                            }
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                result
            });
        }
        match op {
            '-' | '%' | '_' | '~' => self.unary_numeric(op, y),
            '+' => self.flip(y),
            '^' => self.nulls(y),
            '=' => self.group(y),
            '@' => Ok(Value::Number(Number::integer(type_code(y)))),
            '$' => self.stringify(y),
            ':' => Ok(y.clone()),
            '\'' => Err(y.as_text().unwrap_or_else(|| y.to_string())),
            '*' => match y {
                Value::Array(xs) => Ok(xs.get(0).unwrap_or(Value::array([])?)),
                Value::Dictionary(d) => self.monad('*', &Value::Array(d.values())),
                _ => Ok(y.clone()),
            },
            ',' => Ok(Value::array([y.clone()])?),
            '#' => Ok(Value::Number(Number::length(match y {
                Value::Array(xs) => xs.len(),
                Value::Dictionary(d) => d.len(),
                Value::Table(t) => t.len(),
                _ => 1,
            }))),
            '!' => {
                if let Value::Table(t) = y {
                    return Ok(Value::Array(t.names()));
                }
                if let Value::Dictionary(d) = y {
                    return Ok(Value::from_array(d.keys()));
                }
                if matches!(y, Value::Array(_)) {
                    let name = match type_code(y) {
                        1 => "boolean",
                        7 => "long",
                        9 => "float",
                        10 => "char",
                        11 => "symbol",
                        _ => return Err("! requires a typed vector, dictionary, or count".into()),
                    };
                    return Ok(Value::Symbol(Rc::from(name)));
                }
                let n = count(y)?;
                if n < 0 {
                    return Err("range requires a nonnegative integer".into());
                }
                Ok(Value::Array(Array::new(
                    (0..n).map(|n| Value::Number(Number::length(n as usize))),
                )?))
            }
            '|' => match y {
                Value::Array(a) => Ok(Value::Array(a.reversed())),
                Value::Dictionary(d) => {
                    Dictionary::from_arrays(d.keys().reversed(), d.values().reversed())
                        .map(Value::Dictionary)
                }
                _ => Ok(y.clone()),
            },
            '.' => match y {
                Value::Dictionary(d) => Ok(Value::from_array(d.values())),
                Value::Array(_) | Value::Char(_) if y.as_text().is_some() => {
                    let body = compile::program(&parser::parse(&y.as_text().unwrap())?);
                    self.body(&body, &mut Scope::Global(&mut Rc::new(builtins())))
                }
                Value::Array(a) if !a.is_empty() => {
                    self.apply(&a.get(0).unwrap(), &a.iter().skip(1).collect::<Vec<_>>())
                }
                _ => Err(". requires a dictionary, source text, or an evaluation list".into()),
            },
            '&' => {
                if let Value::Dictionary(d) = y {
                    let indices = self.monad('&', &Value::Array(d.values()))?;
                    return index_into(&Value::Array(d.keys()), &indices);
                }
                if !matches!(y, Value::Array(_)) {
                    return Err("where requires an array".into());
                }
                let mut out = Vec::new();
                for (i, x) in items(y).iter().enumerate() {
                    let n = count(&x)?;
                    if n < 0 {
                        return Err("where requires nonnegative integer counts".into());
                    }
                    out.extend(std::iter::repeat_n(Number::length(i), n as usize));
                }
                Ok(Value::Array(Array::numbers(out)))
            }
            '<' | '>' => {
                if let Value::Dictionary(d) = y {
                    let indices = self.monad(op, &Value::Array(d.values()))?;
                    return index_into(&Value::Array(d.keys()), &indices);
                }
                let Value::Array(xs) = y else {
                    return Err("grade requires an array".into());
                };
                let values = xs.iter().collect::<Vec<_>>();
                let mut indices = (0..values.len()).collect::<Vec<_>>();
                indices.sort_by(|a, b| {
                    let order = crate::operators::compare(&values[*a], &values[*b]);
                    if op == '>' { order.reverse() } else { order }
                });
                Ok(Value::array(
                    indices
                        .into_iter()
                        .map(|i| Value::Number(Number::length(i))),
                )?)
            }
            '?' => {
                if !matches!(y, Value::Array(_)) {
                    return Err("distinct requires an array".into());
                }
                let mut unique: Vec<Value> = Vec::new();
                for x in items(y).iter() {
                    let mut seen = false;
                    for previous in &unique {
                        if previous.same(&x) {
                            seen = true;
                            break;
                        }
                    }
                    if !seen {
                        unique.push(x);
                    }
                }
                Ok(Value::array(unique)?)
            }
            _ => Err(format!("{op} has no one-argument meaning")),
        }
    }
    pub(crate) fn dyad(&mut self, op: char, x: &Value, y: &Value) -> Result<Value> {
        // Scalar arithmetic reaches the same rule below; skip the structural checks.
        if let (Value::Number(x), Value::Number(y)) = (x, y)
            && matches!(
                op,
                '+' | '-' | '*' | '%' | 'm' | 'p' | '&' | '|' | '=' | '<' | '>'
            )
        {
            return number_dyad(op, x, y);
        }
        if !matches!(op, '$' | '@' | '.' | '~' | ':')
            && (matches!(x, Value::String(_)) || matches!(y, Value::String(_)))
        {
            if op == '#'
                && let Value::String(s) = y
                && s.is_empty()
            {
                return Ok(Value::text(vec![b' '; count(x)?.unsigned_abs()]));
            }
            let a = x
                .string_items()
                .map(Value::Array)
                .unwrap_or_else(|| x.clone());
            let b = y
                .string_items()
                .map(Value::Array)
                .unwrap_or_else(|| y.clone());
            let result = self.dyad(op, &a, &b)?;
            return Ok(match result {
                Value::Array(a) if !a.is_empty() => Value::from_array(a),
                Value::Array(a) if matches!(op, '#' | '_' | ',' | '^') => {
                    let _ = a;
                    Value::text([])
                }
                other => other,
            });
        }
        if op == '!' {
            let (Value::Array(keys), Value::Array(values)) = (x, y) else {
                return Err("! dictionary construction requires key and value arrays".into());
            };
            return Dictionary::from_arrays(keys.clone(), values.clone()).map(Value::Dictionary);
        }
        if matches!(
            op,
            '+' | '-' | '*' | '%' | '&' | '|' | '=' | '<' | '>' | '^'
        ) && (matches!(x, Value::Dictionary(_)) || matches!(y, Value::Dictionary(_)))
        {
            return self.dictionary_dyad(op, x, y);
        }
        match op {
            '~' => return Ok(boolean(x.same(y))),
            ',' => {
                if let (Value::Table(a), Value::Table(b)) = (x, y) {
                    let result = a.snapshot();
                    result.append(b)?;
                    return Ok(Value::Table(result));
                }
                if matches!(x, Value::Table(_)) || matches!(y, Value::Table(_)) {
                    return Err("table join requires two tables".into());
                }
                if let (Value::Dictionary(a), Value::Dictionary(b)) = (x, y) {
                    let result = Dictionary::from_arrays(a.keys(), a.values())?;
                    for (k, v) in b.keys().iter().zip(b.values().iter()) {
                        result.insert_value(k, v)?;
                    }
                    return Ok(Value::Dictionary(result));
                }
                let xs = items(x);
                let ys = items(y);
                return Value::array(xs.iter().chain(ys.iter()));
            }
            '@' => return self.apply(x, std::slice::from_ref(y)),
            '.' => {
                return match y {
                    Value::Array(a) => self.apply(x, &a.iter().collect::<Vec<_>>()),
                    _ => Err(". requires an argument list".into()),
                };
            }
            '$' => return self.cast(x, y),
            ':' => return Ok(y.clone()),
            '^' if !matches!(x, Value::Array(_)) && !matches!(y, Value::Array(_)) => {
                return Ok(if y.is_null() { x.clone() } else { y.clone() });
            }
            '#' => {
                if let Value::Dictionary(d) = y {
                    let keys = self.dyad('#', x, &Value::Array(d.keys()))?;
                    let values = self.dyad('#', x, &Value::Array(d.values()))?;
                    return Dictionary::from_arrays(items(&keys), items(&values))
                        .map(Value::Dictionary);
                }
                if let Value::Array(shape) = x {
                    return self.reshape(shape, y);
                }
                let n = count(x)?;
                let xs = items(y);
                let length = n.unsigned_abs();
                if length == 0 {
                    return Ok(Value::Array(xs.slice(0, 0)));
                }
                if xs.is_empty() && length != 0 {
                    return Value::array(std::iter::repeat_n(Value::array([])?, length));
                }
                let offset = if n < 0 && !xs.is_empty() {
                    (xs.len() - length % xs.len()) % xs.len()
                } else {
                    0
                };
                return xs
                    .rebuild(
                        (0..length)
                            .map(|i| xs.get((offset + i) % xs.len()).unwrap())
                            .collect(),
                    )
                    .map(Value::Array);
            }
            '_' => {
                if let Value::Dictionary(d) = y
                    && matches!(x, Value::Number(_))
                {
                    let keys = self.dyad('_', x, &Value::Array(d.keys()))?;
                    let values = self.dyad('_', x, &Value::Array(d.values()))?;
                    return Dictionary::from_arrays(items(&keys), items(&values))
                        .map(Value::Dictionary);
                }
                if !matches!(x, Value::Number(_)) {
                    return self.cut_or_delete(x, y);
                }
                let n = count(x)?;
                let xs = items(y);
                let nabs = n.unsigned_abs().min(xs.len());
                return Ok(Value::Array(if n >= 0 {
                    xs.slice(nabs, xs.len())
                } else {
                    xs.slice(0, xs.len() - nabs)
                }));
            }
            '?' => {
                if let Value::Number(_) = x {
                    return self.roll(x, y);
                }
                let xs = items(x);
                let find = |needle: &Value| -> Result<Value> {
                    for (i, value) in xs.iter().enumerate() {
                        if value.same(needle) {
                            return Ok(Value::Number(Number::length(i)));
                        }
                    }
                    Ok(Value::Number(Number::length(xs.len())))
                };
                return match y {
                    Value::Array(ys) => ys
                        .iter()
                        .map(|y| find(&y))
                        .collect::<Result<Vec<_>>>()
                        .and_then(Array::new)
                        .map(Value::from_array),
                    _ => find(y),
                };
            }
            _ => {}
        }
        let result_array = |values: Vec<Value>| {
            let kind = |v: &Value| match v {
                Value::Number(n) => Some(n.type_code()),
                Value::Array(a) if matches!(a.type_code(), 1 | 7 | 9) => Some(a.type_code()),
                Value::Null => Some(0),
                Value::Array(a) if a.iter().all(|v| v.is_null()) => Some(0),
                _ => None,
            };
            if let (Some(a), Some(b)) = (kind(x), kind(y)) {
                let code = match op {
                    '=' | '<' | '>' => 1,
                    '%' | 'p' => 9,
                    '+' | '-' | '*' | 'm' => a.max(b).max(7),
                    '&' | '|' | '^' => a.max(b),
                    _ => 0,
                };
                if code != 0 {
                    return Array::numeric(values, code);
                }
            }
            if matches!(x, Value::Array(a) if a.is_mixed())
                || matches!(y, Value::Array(a) if a.is_mixed())
            {
                Ok(Array::dictionary_values(values))
            } else {
                Array::new(values)
            }
        };
        if let (Value::Array(xs), Value::Array(ys)) = (x, y)
            && xs.len() != ys.len()
        {
            return Err(format!("length mismatch: {} and {}", xs.len(), ys.len()));
        }
        if let Some(result) = vector_dyad(op, x, y) {
            return result;
        }
        match (x, y) {
            (Value::Array(xs), Value::Array(ys)) => xs
                .iter()
                .zip(ys.iter())
                .map(|(x, y)| self.dyad(op, &x, &y))
                .collect::<Result<Vec<_>>>()
                .and_then(result_array)
                .map(Value::from_array),
            (Value::Array(xs), y) => xs
                .iter()
                .map(|x| self.dyad(op, &x, y))
                .collect::<Result<Vec<_>>>()
                .and_then(result_array)
                .map(Value::from_array),
            (x, Value::Array(ys)) => ys
                .iter()
                .map(|y| self.dyad(op, x, &y))
                .collect::<Result<Vec<_>>>()
                .and_then(result_array)
                .map(Value::from_array),
            (Value::Number(x), Value::Number(y)) => number_dyad(op, x, y),
            (Value::Null, _) | (_, Value::Null) => match op {
                '=' => Ok(boolean(matches!((x, y), (Value::Null, Value::Null)))),
                '<' => Ok(boolean(
                    matches!(x, Value::Null) && !matches!(y, Value::Null),
                )),
                '>' => Ok(boolean(
                    !matches!(x, Value::Null) && matches!(y, Value::Null),
                )),
                '&' => Ok(Value::Null),
                '|' => Ok(if matches!(x, Value::Null) {
                    y.clone()
                } else {
                    x.clone()
                }),
                _ => Ok(Value::Null),
            },
            (Value::Char(a), Value::Char(b)) if matches!(op, '=' | '<' | '>' | '&' | '|') => {
                Ok(match op {
                    '=' => boolean(a == b),
                    '<' => boolean(a < b),
                    '>' => boolean(a > b),
                    '&' => Value::Char(*a.min(b)),
                    '|' => Value::Char(*a.max(b)),
                    _ => unreachable!(),
                })
            }
            (Value::Symbol(x), Value::Symbol(y)) if matches!(op, '=' | '<' | '>' | '&' | '|') => {
                if op == '&' {
                    return Ok(Value::Symbol(x.min(y).clone()));
                }
                if op == '|' {
                    return Ok(Value::Symbol(x.max(y).clone()));
                }
                Ok(boolean(match op {
                    '=' => x == y,
                    '<' => x < y,
                    '>' => x > y,
                    _ => unreachable!(),
                }))
            }
            (Value::Symbol(_), Value::Number(_)) | (Value::Number(_), Value::Symbol(_))
                if op == '=' =>
            {
                Ok(boolean(false))
            }
            _ if matches!(op, '=' | '<' | '>') => {
                Err(format!("{op} requires comparable numbers or symbols"))
            }
            _ => Err(format!("{op} requires numbers or arrays of numbers")),
        }
    }
}

/// Elementwise numeric primitives on Arrow vectors, broadcasting scalars.
fn vector_dyad(op: char, x: &Value, y: &Value) -> Option<Result<Value>> {
    let len = match (x, y) {
        (Value::Array(a), _) | (_, Value::Array(a)) => a.len(),
        _ => return None,
    };
    let operand = |value: &Value| match value {
        Value::Array(a) => a.numeric_snapshot().map(Operand::Vector),
        Value::Number(n) => Some(Operand::Number(*n)),
        Value::Null => Some(Operand::Null),
        _ => None,
    };
    let result = crate::arrow::dyad(op, &operand(x)?, &operand(y)?, len)?;
    Some(result.map(|numbers| Value::Array(Array::from_numbers(numbers))))
}

fn number_dyad(op: char, x: &Number, y: &Number) -> Result<Value> {
    if op == 'm' && x.is_integer() && y.is_integer() && y.is_zero() {
        return Ok(Value::Null);
    }
    Ok(Value::from_number(match op {
        '+' => x.add(y)?,
        '-' => x.subtract(y)?,
        '*' => x.multiply(y)?,
        '%' => x.divide(y)?,
        'm' => x.remainder(y)?,
        'p' => x.power(y)?,
        '&' => x.minimum(y),
        '|' => x.maximum(y),
        '=' => return Ok(boolean(x.equivalent(y))),
        '<' => return Ok(boolean(!x.equivalent(y) && x < y)),
        '>' => return Ok(boolean(!x.equivalent(y) && x > y)),
        _ => return Err(format!("unknown operator: {op}")),
    }))
}

pub(crate) fn boolean(b: bool) -> Value {
    Value::Number(Number::Bool(b))
}
pub(crate) fn number(value: &Value) -> Result<&Number> {
    if let Value::Number(n) = value {
        Ok(n)
    } else {
        Err("expected a number".into())
    }
}
pub(crate) fn count(value: &Value) -> Result<isize> {
    let n = number(value)?;
    if !n.is_integer() {
        return Err("expected an integer".into());
    }
    n.to_isize()
        .ok_or_else(|| "count out of range for this platform".into())
}
pub(crate) fn items(value: &Value) -> Array {
    if let Some(chars) = value.string_items() {
        return chars;
    }
    if let Value::Array(xs) = value {
        xs.clone()
    } else {
        Array::new([value.clone()]).expect("singleton is homogeneous")
    }
}
pub(crate) fn index_into(array: &Value, index: &Value) -> Result<Value> {
    if let Value::String(string) = array {
        return match index {
            Value::Null => Ok(Value::Null),
            Value::Number(n) => {
                let i = n
                    .as_i64()
                    .and_then(|i| isize::try_from(i).ok())
                    .ok_or("string index must be an integer")?;
                Ok(if i < 0 { None } else { string.get(i as usize) }
                    .map(Value::Char)
                    .unwrap_or(Value::Null))
            }
            Value::Array(indices) => {
                let values = indices
                    .iter()
                    .map(|i| index_into(array, &i))
                    .collect::<Result<Vec<_>>>()?;
                if values.iter().all(|v| matches!(v, Value::Char(_))) {
                    Ok(Value::text(
                        values
                            .into_iter()
                            .map(|v| {
                                if let Value::Char(c) = v {
                                    c
                                } else {
                                    unreachable!()
                                }
                            })
                            .collect::<Vec<_>>(),
                    ))
                } else {
                    Ok(Value::array(values)?)
                }
            }
            Value::Function(f) if matches!(f.kind, FunctionKind::Verb(':')) => Ok(array.clone()),
            _ => Err("string index must be an integer or array".into()),
        };
    }
    if let Value::Table(t) = array {
        return t.lookup(index);
    }
    if let Value::Dictionary(d) = array {
        return d.lookup(index);
    }
    let Value::Array(xs) = array else {
        return Err("indexing requires an array".into());
    };
    match index {
        Value::Null => Ok(Value::Null),
        Value::Array(indices) if indices.is_empty() => Ok(Value::Array(xs.slice(0, 0))),
        Value::Array(indices) => indices
            .iter()
            .map(|i| index_into(array, &i))
            .collect::<Result<Vec<_>>>()
            .and_then(|values| xs.rebuild(values))
            .map(Value::from_array),
        Value::Number(n) => {
            let i = n
                .as_i64()
                .and_then(|i| isize::try_from(i).ok())
                .ok_or("index out of bounds or not an integer")?;
            Ok(if i < 0 { None } else { xs.get(i as usize) }.unwrap_or(Value::Null))
        }
        Value::Function(f) if matches!(f.kind, FunctionKind::Verb(':')) => Ok(array.clone()),
        _ => Err("index must be a number or array of numbers".into()),
    }
}

pub(crate) fn index_depth(value: &Value, indices: &[Value]) -> Result<Value> {
    let Some((first, rest)) = indices.split_first() else {
        return Ok(value.clone());
    };
    let selected = index_into(value, first)?;
    if rest.is_empty() {
        return Ok(selected);
    }
    if matches!(value, Value::Table(_)) {
        return index_depth(&selected, rest);
    }
    if matches!(
        first,
        Value::Array(_) | Value::String(_) | Value::Function(_)
    ) && !matches!(value, Value::Dictionary(d) if d.key_is_atom(first))
    {
        return items(&selected)
            .iter()
            .map(|v| index_depth(&v, rest))
            .collect::<Result<Vec<_>>>()
            .and_then(|values| {
                if matches!(value, Value::Dictionary(_))
                    || matches!(&selected, Value::Array(a) if a.is_mixed())
                {
                    Ok(Array::dictionary_values(values))
                } else {
                    Array::new(values)
                }
            })
            .map(Value::from_array);
    }
    index_depth(&selected, rest)
}

use crate::operators::{builtins, type_code};

pub(crate) fn unary_function(f: &Value) -> bool {
    matches!(f, Value::Function(f) if match &f.kind {
        FunctionKind::User { proto, .. } => proto.params == 1,
        FunctionKind::Monadic(_) => true,
        FunctionKind::Native(name) => !matches!(*name, "mod" | "pow"),
        FunctionKind::Projection(_, args) => args.iter().filter(|v| v.is_none()).count() == 1,
        _ => false,
    })
}
