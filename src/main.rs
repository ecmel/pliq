use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
};

use pliq::{Interpreter, Value};

mod input;
mod net;

struct Output(Value);

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Value::Table(table) => write!(f, "{table}"),
            Value::Dictionary(dictionary) => write!(f, "{dictionary}"),
            value => write!(f, "{value}"),
        }
    }
}

const REPL_HELP: &str = r#"  \h  Show this help; keep pending input and bindings
  \c  Discard pending input; keep bindings
  \q  Exit
"#;

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--precision") {
        return Err(
            "--precision is not supported; use round[value;places] for explicit rounding".into(),
        );
    }
    let mut interpreter = Interpreter::new();
    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "pliq - an array language\n\nUsage: pliq                  Start the REPL (or read piped stdin)\n       pliq -e 'expression'  Evaluate an expression\n       pliq path.pliq        Run a file\n       pliq -                Read stdin\n       pliq --listen [HOST:]PORT [path.pliq]  Run a file, then serve TCP\n       pliq --attach [HOST:]PORT    Attach a REPL to a running daemon\n\nNumbers use native i64 integers and f64 floats. Use round[value;places] for explicit rounding.\nREPL commands: \\h (help), \\c (clear input), \\q (quit)\nSee README.md for syntax and operators."
            );
        }
        [flag, source] if flag == "-e" => println!("{}", Output(interpreter.eval(source)?)),
        [flag, address] if flag == "--listen" => listen_mode(&mut interpreter, address, None)?,
        [flag, address, path] if flag == "--listen" => {
            listen_mode(&mut interpreter, address, Some(path))?;
        }
        [flag, address] if flag == "--attach" => attach_mode(address)?,
        [path] if !path.starts_with('-') => {
            let source = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            println!("{}", Output(interpreter.eval(&source)?));
        }
        [] if io::stdin().is_terminal() => repl(&mut interpreter)?,
        [] | [_] if args.is_empty() || args[0] == "-" => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|e| e.to_string())?;
            println!("{}", Output(interpreter.eval(&source)?));
        }
        _ => {
            return Err(
                "usage: pliq [-e 'expression' | path.pliq | - | --listen [HOST:]PORT [path.pliq] | --attach [HOST:]PORT | --help]"
                    .into(),
            );
        }
    }
    Ok(())
}

/// Run an optional startup script, then serve the address until stopped.
fn listen_mode(
    interpreter: &mut Interpreter,
    address: &str,
    script: Option<&str>,
) -> Result<(), String> {
    if let Some(path) = script {
        let source = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        interpreter.eval(&source)?;
    }
    net::listen(interpreter, address, &mut io::stdout().lock()).map_err(|e| e.to_string())
}

fn attach_mode(address: &str) -> Result<(), String> {
    if io::stdin().is_terminal() {
        return net::attach(
            address,
            input::Terminal::new().map_err(|e| e.to_string())?,
            &mut io::stdout().lock(),
            &mut io::stderr().lock(),
        )
        .map_err(|e| e.to_string());
    }
    net::attach(
        address,
        io::stdin().lock(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
    .map_err(|e| e.to_string())
}

fn repl(interpreter: &mut Interpreter) -> Result<(), String> {
    repl_io(
        interpreter,
        input::Terminal::new().map_err(|e| e.to_string())?,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
    .map_err(|e| e.to_string())
}

fn repl_io(
    interpreter: &mut Interpreter,
    mut input: impl input::Input,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        output,
        "pliq {} - {}. \\h for help, \\q to exit.",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_DESCRIPTION")
    )?;
    let mut source = String::new();
    loop {
        let prompt = if source.is_empty() { "  " } else { ".." };
        let Some(line) = input.line(prompt, output)? else {
            break;
        };
        match line.trim() {
            "\\q" => {
                source.clear();
                break;
            }
            "\\h" => {
                write!(output, "{REPL_HELP}")?;
                continue;
            }
            "\\c" => {
                source.clear();
                continue;
            }
            "" if source.is_empty() => continue,
            _ => {}
        }
        source.push_str(&line);
        source.push('\n');
        if incomplete(&source) {
            continue;
        }
        input.remember(&source);
        match interpreter.eval(&source) {
            Ok(value) => writeln!(output, "{}", Output(value))?,
            Err(error) => writeln!(errors, "error: {error}")?,
        }
        source.clear();
    }
    if !source.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete expression at end of input",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/repl.rs"]
mod tests;

fn incomplete(source: &str) -> bool {
    let mut stack = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    for line in source.lines() {
        let bytes = line.as_bytes();
        for (i, c) in bytes.iter().copied().enumerate() {
            if quoted {
                if escaped {
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == b'"' {
                    quoted = false;
                }
                continue;
            }
            if c == b'"' {
                quoted = true;
                continue;
            }
            if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
                break;
            }
            match c {
                b'(' => stack.push(b')'),
                b'[' => stack.push(b']'),
                b'{' => stack.push(b'}'),
                b')' | b']' | b'}' if stack.pop() != Some(c) => return false,
                _ => {}
            }
        }
    }
    quoted || !stack.is_empty()
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
