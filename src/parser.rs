use std::rc::Rc;

use crate::{Number, Result};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Expr {
    Number(Number),
    Null,
    Text(Vec<u8>),
    Symbol(Rc<str>),
    Array(Vec<Expr>),
    Table(Vec<(String, Expr)>),
    Strand(Vec<Expr>),
    Name(String),
    Assign(String, Box<Expr>),
    Update(String, Vec<Expr>, Box<Expr>),
    Append(Box<Expr>, Box<Expr>),
    Modify(Box<Expr>, char, Box<Expr>),
    Hole,
    Return(Box<Expr>),
    Verb(char),
    Monadic(char),
    Derived(char, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Lambda(Vec<String>, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(Number),
    Null,
    Text(Vec<u8>),
    Symbol(Rc<str>),
    Name(String),
    Op(char),
    Adverb(char),
    Colon,
    Semi,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    End,
}

fn lex(source: &str) -> Result<Vec<(Token, usize, usize)>> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut delimiters = Vec::new();
    while i < bytes.len() {
        let start = i;
        let c = bytes[i] as char;
        if c == '/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_whitespace() && (c != '\n' || matches!(delimiters.last(), Some('(' | '['))) {
            i += 1;
            continue;
        }
        let signed = c == '-'
            && bytes
                .get(i + 1)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
            && (i == 0
                || bytes[i - 1].is_ascii_whitespace()
                || matches!(bytes[i - 1], b'(' | b'[' | b';' | b':' | b'{'));
        let special_start = if c == '-' { i + 1 } else { i };
        let token = if (c != '-' || signed)
            && bytes.get(special_start) == Some(&b'0')
            && bytes
                .get(special_start + 1)
                .is_some_and(|c| matches!(c, b'N' | b'n' | b'W' | b'w'))
        {
            i = special_start + 2;
            if &source[start..i] == "0n" {
                Token::Null
            } else {
                Token::Number(source[start..i].parse()?)
            }
        } else if c == '"' {
            i += 1;
            let mut text = Vec::new();
            while i < bytes.len() && bytes[i] != b'"' {
                let mut byte = bytes[i];
                i += 1;
                if byte == b'\\' {
                    byte = match bytes.get(i) {
                        Some(b'x') => {
                            let hex = bytes
                                .get(i + 1..i + 3)
                                .and_then(|s| std::str::from_utf8(s).ok())
                                .and_then(|s| u8::from_str_radix(s, 16).ok())
                                .ok_or("invalid hexadecimal string escape")?;
                            i += 2;
                            hex
                        }
                        Some(b'n') => b'\n',
                        Some(b'r') => b'\r',
                        Some(b't') => b'\t',
                        Some(b'"') => b'"',
                        Some(b'\\') => b'\\',
                        _ => return Err(format!("invalid string escape at byte {}", i + 1)),
                    };
                    i += 1;
                }
                text.push(byte);
            }
            if bytes.get(i) != Some(&b'"') {
                return Err("unterminated string".into());
            }
            i += 1;
            Token::Text(text)
        } else if signed
            || c.is_ascii_digit()
            || (c == '.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            i += if signed { 2 } else { 1 };
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if bytes.get(i).is_some_and(|c| *c == b'e' || *c == b'E') {
                i += 1;
                if bytes.get(i).is_some_and(|c| *c == b'+' || *c == b'-') {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let n: Number = source[start..i]
                .parse()
                .map_err(|error| format!("{error} at byte {}", start + 1))?;
            Token::Number(n)
        } else if c == '`' {
            i += 1;
            let name_start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.') {
                i += 1;
            }
            Token::Symbol(Rc::from(&source[name_start..i]))
        } else if c.is_ascii_alphabetic() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                i += 1;
            }
            Token::Name(source[start..i].to_owned())
        } else {
            i += 1;
            match c {
                '+' | '-' | '*' | '%' | '!' | '&' | '|' | '^' | '#' | '_' | ',' | '=' | '<'
                | '>' | '~' | '@' | '?' | '$' | '.' => Token::Op(c),
                '/' | '\\' | '\'' => Token::Adverb(c),
                ':' if bytes.get(i) == Some(&b':') => {
                    i += 1;
                    Token::Op(':')
                }
                ':' => Token::Colon,
                ';' | '\n' => Token::Semi,
                '(' => Token::LParen,
                ')' => Token::RParen,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                _ => {
                    return Err(format!(
                        "unexpected character at byte {} (syntax is ASCII)",
                        start + 1
                    ));
                }
            }
        };
        match c {
            '(' | '[' | '{' => delimiters.push(c),
            ')' | ']' | '}' => {
                delimiters.pop();
            }
            _ => {}
        }
        out.push((token, start, i));
    }
    out.push((Token::End, source.len(), source.len()));
    Ok(out)
}

pub(crate) fn parse(source: &str) -> Result<Vec<Expr>> {
    let mut p = Parser {
        tokens: lex(source)?,
        pos: 0,
        depth: 0,
    };
    let body = p.sequence(Token::End)?;
    Ok(body)
}

struct Parser {
    tokens: Vec<(Token, usize, usize)>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }
    fn take(&mut self) -> Token {
        let token = self.peek().clone();
        if token != Token::End {
            self.pos += 1;
        }
        token
    }
    fn error(&self, message: &str) -> String {
        format!("{message} at byte {}", self.tokens[self.pos].1 + 1)
    }
    fn expect(&mut self, token: Token) -> Result<()> {
        if self.peek() != &token {
            return Err(self.error(&format!("expected {token:?}")));
        }
        self.take();
        Ok(())
    }
    fn sequence(&mut self, end: Token) -> Result<Vec<Expr>> {
        let mut body = Vec::new();
        while self.peek() == &Token::Semi {
            self.take();
        }
        while self.peek() != &end {
            body.push(self.expr()?);
            if self.peek() != &end {
                self.expect(Token::Semi)?;
            }
            while self.peek() == &Token::Semi {
                self.take();
            }
        }
        self.expect(end)?;
        Ok(body)
    }
    fn expr(&mut self) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(self.error("expression nesting limit exceeded"));
        }
        let result = self.expression();
        self.depth -= 1;
        result
    }
    fn expression(&mut self) -> Result<Expr> {
        if self.peek() == &Token::Colon
            && !matches!(
                self.tokens.get(self.pos + 1).map(|t| &t.0),
                Some(Token::Semi | Token::RBracket | Token::RParen | Token::End)
            )
        {
            self.take();
            return Ok(Expr::Return(Box::new(self.expr()?)));
        }
        if let Token::Name(name) = self.peek().clone()
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.0 == Token::Colon)
        {
            self.take();
            self.take();
            return Ok(Expr::Assign(name, Box::new(self.expr()?)));
        }
        let left = if matches!(self.peek(), Token::Op(_)) {
            let verb = self.verb()?;
            if matches!(verb, Expr::Verb(_) | Expr::Monadic(_) | Expr::Derived(_, _))
                && self.starts_expr()
            {
                let verb = if let Expr::Verb(op) = verb {
                    Expr::Monadic(op)
                } else {
                    verb
                };
                return Ok(Expr::Call(Box::new(verb), vec![self.expr()?]));
            }
            verb
        } else {
            let atom = self.atom()?;
            self.postfix(atom)?
        };
        if let Token::Op(op) = self.peek().clone()
            && op != ','
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.0 == Token::Colon)
        {
            self.take();
            self.take();
            return Ok(Expr::Modify(Box::new(left), op, Box::new(self.expr()?)));
        }
        if self.peek() == &Token::Op(',')
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.0 == Token::Colon)
        {
            self.take();
            self.take();
            return Ok(Expr::Append(Box::new(left), Box::new(self.expr()?)));
        }
        if self.peek() == &Token::Colon {
            self.take();
            let mut target = left;
            let mut indices = Vec::new();
            while let Expr::Call(function, args) = target {
                if args.is_empty() {
                    return Err(self.error("indexed assignment requires selectors"));
                }
                indices.extend(args.into_iter().rev().map(|arg| {
                    if matches!(arg, Expr::Hole) {
                        Expr::Verb(':')
                    } else {
                        arg
                    }
                }));
                if indices.len() > 128 {
                    return Err(self.error("indexed assignment nesting limit exceeded"));
                }
                target = *function;
            }
            let Expr::Name(name) = target else {
                return Err(self.error("assignment target must be a variable or indexed variable"));
            };
            indices.reverse();
            let value = Box::new(self.expr()?);
            return Ok(if indices.is_empty() {
                Expr::Assign(name, value)
            } else {
                Expr::Update(name, indices, value)
            });
        }
        if matches!(self.peek(), Token::Op(_)) {
            let verb = self.verb()?;
            let right = if self.starts_expr() {
                self.expr()?
            } else {
                Expr::Hole
            };
            Ok(Expr::Call(Box::new(verb), vec![left, right]))
        } else if self.starts_expr() {
            // A derived function in infix position receives the value on its left.
            if matches!(self.peek(), Token::Name(_) | Token::LBrace) {
                let saved = self.pos;
                let candidate = self.atom()?;
                let candidate = self.postfix(candidate)?;
                if matches!(candidate, Expr::Derived(_, _)) && self.starts_expr() {
                    return Ok(Expr::Call(Box::new(candidate), vec![left, self.expr()?]));
                }
                self.pos = saved;
            }
            // Application takes one complete expression on the right. Numeric
            // and symbol strands are already grouped by atom().
            // Operators above keep their infix meaning: use f (-2) or f (!10)
            // when an argument starts with a prefix operator.
            Ok(Expr::Call(Box::new(left), vec![self.expr()?]))
        } else {
            Ok(left)
        }
    }
    fn starts_expr(&self) -> bool {
        matches!(
            self.peek(),
            Token::Null
                | Token::Number(_)
                | Token::Colon
                | Token::Text(_)
                | Token::Symbol(_)
                | Token::Name(_)
                | Token::Op(_)
                | Token::LParen
                | Token::LBrace
        )
    }
    fn verb(&mut self) -> Result<Expr> {
        let Token::Op(op) = self.take() else {
            unreachable!()
        };
        let value = if self.peek() == &Token::Colon {
            self.take();
            Expr::Monadic(op)
        } else {
            Expr::Verb(op)
        };
        self.postfix(value)
    }
    fn postfix(&mut self, mut value: Expr) -> Result<Expr> {
        let mut depth = 0;
        loop {
            if depth > 128 {
                return Err(self.error("postfix nesting limit exceeded"));
            }
            depth += 1;
            match self.peek().clone() {
                Token::Op('.')
                    if !matches!(
                        value,
                        Expr::Verb(_) | Expr::Monadic(_) | Expr::Derived(_, _)
                    ) && self.pos > 0
                        && self.tokens[self.pos - 1].2 == self.tokens[self.pos].1
                        && self.tokens.get(self.pos + 1).is_some_and(|next| {
                            matches!(next.0, Token::Name(_)) && self.tokens[self.pos].2 == next.1
                        }) =>
                {
                    // Adjacent .name is the same selector as [`name]. Keep spaced
                    // dots as application, and leave symbol literals intact.
                    self.take();
                    let Token::Name(name) = self.take() else {
                        unreachable!()
                    };
                    value = Expr::Call(Box::new(value), vec![Expr::Symbol(Rc::from(name))]);
                }
                Token::Adverb(adverb) => {
                    self.take();
                    let adverb = if self.peek() == &Token::Colon {
                        self.take();
                        match adverb {
                            '/' => 'R',
                            '\\' => 'L',
                            '\'' => 'P',
                            _ => unreachable!(),
                        }
                    } else {
                        adverb
                    };
                    value = Expr::Derived(adverb, Box::new(value));
                }
                Token::LBracket => {
                    self.take();
                    let mut args = Vec::new();
                    if self.peek() != &Token::RBracket {
                        loop {
                            args.push(if matches!(self.peek(), Token::Semi | Token::RBracket) {
                                Expr::Hole
                            } else {
                                self.expr()?
                            });
                            if self.peek() != &Token::Semi {
                                break;
                            }
                            self.take();
                        }
                    }
                    self.expect(Token::RBracket)?;
                    value = if matches!(value, Expr::Verb('$'))
                        && args.len() >= 3
                        && args.len() % 2 == 1
                    {
                        let mut branch = args.pop().unwrap();
                        while let Some(yes) = args.pop() {
                            let condition = args.pop().unwrap();
                            branch = Expr::If(Box::new(condition), Box::new(yes), Box::new(branch));
                        }
                        branch
                    } else {
                        Expr::Call(Box::new(value), args)
                    };
                }
                _ => return Ok(value),
            }
        }
    }
    fn atom(&mut self) -> Result<Expr> {
        match self.take() {
            token @ (Token::Number(_) | Token::Null) => {
                let literal = |t| match t {
                    Token::Number(n) => Expr::Number(n),
                    Token::Null => Expr::Null,
                    _ => unreachable!(),
                };
                let mut xs = vec![literal(token)];
                while matches!(self.peek(), Token::Number(_) | Token::Null) {
                    xs.push(literal(self.take()));
                }
                if xs.len() == 1 {
                    Ok(xs.remove(0))
                } else {
                    Ok(Expr::Strand(xs))
                }
            }
            Token::Symbol(name) => {
                let mut xs = vec![Expr::Symbol(name)];
                while let Token::Symbol(name) = self.peek().clone() {
                    self.take();
                    xs.push(Expr::Symbol(name));
                }
                if xs.len() == 1 {
                    Ok(xs.remove(0))
                } else {
                    Ok(Expr::Array(xs))
                }
            }
            Token::Adverb('\'') => Ok(Expr::Monadic('\'')),
            Token::Colon => Ok(Expr::Verb(':')),
            Token::Text(text) => Ok(Expr::Text(text)),
            Token::Name(name) => Ok(Expr::Name(name)),
            Token::LParen => {
                if self.peek() == &Token::LBracket {
                    self.take();
                    self.expect(Token::RBracket)?;
                    let mut columns = Vec::new();
                    while self.peek() != &Token::RParen {
                        let name = match self.take() {
                            Token::Name(name) => name,
                            Token::Symbol(name) => name.to_string(),
                            Token::Text(name) => String::from_utf8(name)
                                .map_err(|_| self.error("table column names must be UTF-8"))?,
                            _ => return Err(self.error("expected table column name")),
                        };
                        if columns.iter().any(|(n, _)| n == &name) {
                            return Err(self.error("duplicate table column name"));
                        }
                        self.expect(Token::Colon)?;
                        columns.push((name, self.expr()?));
                        if self.peek() != &Token::Semi {
                            break;
                        }
                        self.take();
                    }
                    self.expect(Token::RParen)?;
                    return Ok(Expr::Table(columns));
                }
                if self.peek() == &Token::RParen {
                    self.take();
                    return Ok(Expr::Array(Vec::new()));
                }
                let first = self.expr()?;
                if self.peek() != &Token::Semi {
                    self.expect(Token::RParen)?;
                    return Ok(first);
                }
                let mut xs = vec![first];
                while self.peek() == &Token::Semi {
                    self.take();
                    xs.push(self.expr()?);
                }
                self.expect(Token::RParen)?;
                Ok(Expr::Array(xs))
            }
            Token::LBrace => {
                let explicit = self.peek() == &Token::LBracket;
                let mut params = Vec::new();
                if explicit {
                    self.take();
                    if self.peek() != &Token::RBracket {
                        loop {
                            let Token::Name(name) = self.take() else {
                                return Err(self.error("expected parameter name"));
                            };
                            if params.contains(&name) {
                                return Err(self.error("duplicate parameter"));
                            }
                            params.push(name);
                            if self.peek() != &Token::Semi {
                                break;
                            }
                            self.take();
                        }
                    }
                    self.expect(Token::RBracket)?;
                }
                let body = self.sequence(Token::RBrace)?;
                if body.is_empty() {
                    return Err(self.error("function body cannot be empty"));
                }
                if !explicit {
                    let arity = body.iter().map(implicit_arity).max().unwrap_or(0);
                    params = ["x", "y", "z"][..arity]
                        .iter()
                        .map(|s| (*s).to_owned())
                        .collect();
                }
                Ok(Expr::Lambda(params, body))
            }
            _ => Err(self.error("expected expression")),
        }
    }
}

fn implicit_arity(expr: &Expr) -> usize {
    match expr {
        Expr::Name(name) => match name.as_str() {
            "x" => 1,
            "y" => 2,
            "z" => 3,
            _ => 0,
        },
        Expr::Array(xs) | Expr::Strand(xs) => xs.iter().map(implicit_arity).max().unwrap_or(0),
        Expr::Table(columns) => columns
            .iter()
            .map(|(_, e)| implicit_arity(e))
            .max()
            .unwrap_or(0),
        Expr::Assign(_, e) | Expr::Derived(_, e) | Expr::Return(e) => implicit_arity(e),
        Expr::Append(target, value) | Expr::Modify(target, _, value) => {
            implicit_arity(target).max(implicit_arity(value))
        }
        Expr::Update(name, indices, value) => implicit_arity(&Expr::Name(name.clone()))
            .max(indices.iter().map(implicit_arity).max().unwrap_or(0))
            .max(implicit_arity(value)),
        Expr::Call(f, args) => {
            implicit_arity(f).max(args.iter().map(implicit_arity).max().unwrap_or(0))
        }
        Expr::If(c, a, b) => implicit_arity(c)
            .max(implicit_arity(a))
            .max(implicit_arity(b)),
        _ => 0,
    }
}

#[cfg(test)]
#[path = "../tests/support/parser_robustness.rs"]
mod tests;
