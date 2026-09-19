use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
};

use pliq::{Console, Interpreter};

mod input;
mod net;

const REPL_HELP: &str = r#"  \h               Show this help; keep pending input and bindings
  \c               Show the console size for results
  \c rows columns  Set it: 5 to 1000 rows, 20 or more columns; 0 for the most
  \c auto          Show up to 1000 rows at the terminal's width (the default)
  \d               Discard pending input; keep bindings
  \q               Exit
"#;

/// The most rows a REPL result shows. Scrollback handles tall output, so only
/// the width follows the terminal.
pub(crate) const ROWS: usize = 1000;
const MIN_ROWS: usize = 5;
pub(crate) const MIN_COLUMNS: usize = 20;

/// The console size for REPL results.
#[derive(Clone, Copy, Default)]
pub(crate) enum Size {
    /// The input's own size: the terminal's width, or no width limit for piped input.
    #[default]
    Auto,
    /// A size from `\c`; zero rows means the most.
    Fixed(Console),
}

impl Size {
    /// The size for the next result, never over `ROWS` rows.
    pub(crate) fn console(self, input: &mut impl input::Input) -> Console {
        let console = match self {
            Self::Auto => input.console(),
            Self::Fixed(console) => console,
        };
        Console {
            rows: if console.rows == 0 {
                ROWS
            } else {
                console.rows.min(ROWS)
            },
            ..console
        }
    }

    /// Apply the arguments of a `\c` command, returning any text to show.
    pub(crate) fn command(
        &mut self,
        args: &str,
        input: &mut impl input::Input,
    ) -> Result<Option<String>, String> {
        let usage = || Err("usage: \\c [rows columns | auto]".into());
        match args.split_whitespace().collect::<Vec<_>>().as_slice() {
            [] => {
                let console = self.console(input);
                let auto = if matches!(self, Self::Auto) {
                    " (auto)"
                } else {
                    ""
                };
                Ok(Some(format!("{} {}{auto}", console.rows, console.columns)))
            }
            ["auto"] => {
                *self = Self::Auto;
                Ok(None)
            }
            [rows, columns] => {
                let (Ok(rows), Ok(columns)) = (rows.parse::<usize>(), columns.parse::<usize>())
                else {
                    return usage();
                };
                if !(rows == 0 || (MIN_ROWS..=ROWS).contains(&rows))
                    || !(columns == 0 || columns >= MIN_COLUMNS)
                {
                    return Err(format!(
                        "console size needs {MIN_ROWS} to {ROWS} rows (0 for {ROWS}) and at least {MIN_COLUMNS} columns (0 for no limit)"
                    ));
                }
                *self = Self::Fixed(Console { rows, columns });
                Ok(None)
            }
            _ => usage(),
        }
    }
}

/// The arguments of a `\c` command line.
pub(crate) fn console_command(line: &str) -> Option<&str> {
    let args = line.strip_prefix("\\c")?;
    (args.is_empty() || args.starts_with(char::is_whitespace)).then_some(args)
}

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
                "pliq - an array language\n\nUsage: pliq                  Start the REPL (or read piped stdin)\n       pliq -e 'expression'  Evaluate an expression\n       pliq path.pliq        Run a file\n       pliq -                Read stdin\n       pliq --listen [HOST:]PORT [path.pliq]  Run a file, then serve TCP\n       pliq --attach [HOST:]PORT    Attach a REPL to a running daemon\n\nNumbers use native i64 integers and f64 floats. Use round[value;places] for explicit rounding.\nREPL commands: \\h (help), \\c (console size), \\d (discard input), \\q (quit)\nSee README.md for syntax and operators."
            );
        }
        [flag, source] if flag == "-e" => {
            println!("{}", interpreter.eval(source)?.view(Console::UNLIMITED));
        }
        [flag, address] if flag == "--listen" => listen_mode(&mut interpreter, address, None)?,
        [flag, address, path] if flag == "--listen" => {
            listen_mode(&mut interpreter, address, Some(path))?;
        }
        [flag, address] if flag == "--attach" => attach_mode(address)?,
        [path] if !path.starts_with('-') => {
            let source = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            println!("{}", interpreter.eval(&source)?.view(Console::UNLIMITED));
        }
        [] if io::stdin().is_terminal() => repl(&mut interpreter)?,
        [] | [_] if args.is_empty() || args[0] == "-" => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|e| e.to_string())?;
            println!("{}", interpreter.eval(&source)?.view(Console::UNLIMITED));
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
    let mut size = Size::default();
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
            "\\d" => {
                source.clear();
                continue;
            }
            "" if source.is_empty() => continue,
            command => {
                if let Some(args) = console_command(command) {
                    match size.command(args, &mut input) {
                        Ok(Some(text)) => writeln!(output, "{text}")?,
                        Ok(None) => {}
                        Err(error) => writeln!(errors, "error: {error}")?,
                    }
                    continue;
                }
            }
        }
        source.push_str(&line);
        source.push('\n');
        if incomplete(&source) {
            continue;
        }
        input.remember(&source);
        match interpreter.eval(&source) {
            Ok(value) => writeln!(output, "{}", value.view(size.console(&mut input)))?,
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
