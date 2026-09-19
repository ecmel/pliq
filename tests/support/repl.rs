use super::*;

fn session(input: &str) -> (String, String, io::Result<()>) {
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let result = repl_io(
        &mut Interpreter::new(),
        input.as_bytes(),
        &mut output,
        &mut errors,
    );
    (
        String::from_utf8(output).unwrap(),
        String::from_utf8(errors).unwrap(),
        result,
    )
}

#[test]
fn repl_keeps_state_and_recovers_after_errors() {
    let (out, err, result) = session("\na:4\n1+`bad\na+2\n\\q\nmissing\n");
    result.unwrap();
    assert!(out.starts_with(concat!(
        "pliq ",
        env!("CARGO_PKG_VERSION"),
        " - ",
        env!("CARGO_PKG_DESCRIPTION"),
        ". \\h for help, \\q to exit.\n"
    )));
    assert!(out.contains("  4\n  "));
    assert!(out.contains("  6\n"));
    assert!(err.contains("requires numbers"));
    assert!(!err.contains("missing"));
}

#[test]
fn repl_multiline_comments_and_clear() {
    let (out, err, result) = session("f:{\nx+1 // } ignored\n}\nf 4\n(\n\\d\n7\n\\q\n");
    result.unwrap();
    assert!(out.contains(".."));
    assert!(out.contains("5\n"));
    assert!(out.contains("  7\n"));
    assert!(err.is_empty(), "{err}");
}

#[test]
fn repl_help_preserves_pending_input_and_bindings() {
    let (out, err, result) = session("\\h\na:4\n(a+\n  \\h  \n2)\na\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert_eq!(out.matches("\\h               Show this help").count(), 2);
    assert!(out.contains("\\c rows columns  Set it: 5 to 1000 rows"));
    assert!(out.contains("\\d               Discard pending input"));
    assert!(out.contains("\\q               Exit"));
    assert!(out.contains("..6\n  4\n"), "{out}");
}

#[test]
fn repl_multiline_returns_do_not_collide_with_commands() {
    for name in ["c", "d", "h", "q"] {
        let (out, err, result) = session(&format!("{name}:17\nf:{{\n:{name}\n99\n}}\nf[]\n\\q\n"));
        result.unwrap();
        assert!(err.is_empty(), "{name}: {err}");
        assert!(out.ends_with("  17\n  "), "{name}: {out}");
    }
}

#[test]
fn repl_clear_preserves_bindings() {
    let (out, err, result) = session("a:7\n(a:9;\n \\d \na\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("..  7\n"), "{out}");
}

#[test]
fn repl_long_commands_are_no_longer_recognized() {
    for command in ["\\quit", "\\clear", "\\help"] {
        let (out, err, result) = session(&format!("{command}\n42\n\\q\n"));
        result.unwrap();
        assert!(err.contains("error:"), "{command}: {err}");
        assert!(out.contains("  42\n"), "{command}: {out}");
    }
}

#[test]
fn repl_console_size_limits_results_but_not_errors() {
    let message = "a long error message that is wider than twenty columns";
    let (out, err, result) = session(&format!(
        "\\c\n\\c 6 20\n\\c\n!1000\n([] a:!10)\n(!10)!!10\n'\"{message}\"\n\\q\n"
    ));
    result.unwrap();
    // Piped input has no width limit until one is set.
    assert!(out.contains("  1000 0 (auto)\n"), "{out}");
    assert!(out.contains("  6 20\n"), "{out}");
    assert!(out.contains("  0 1 2 3 4 5 6 7 8 ..\n"), "{out}");
    assert!(out.contains("  a\n-\n0\n1\n..\n10 rows\n"), "{out}");
    assert!(
        out.contains("  0 | 0\n1 | 1\n2 | 2\n3 | 3\n..\n10 entries\n"),
        "{out}"
    );
    assert!(err.contains(message), "{err}");
}

#[test]
fn repl_console_size_commands_keep_pending_input() {
    let (out, err, result) =
        session("\\c 5 20\n(1+\n\\c 0 0\n2)\n!30\n\\c 5 20\n\\c auto\n\\c\n!30\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("..3\n"), "{out}");
    assert_eq!(
        out.matches(
            "0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29\n"
        )
        .count(),
        2,
        "{out}"
    );
    assert!(out.contains("  1000 0 (auto)\n"), "{out}");
}

#[test]
fn repl_results_never_exceed_1000_rows() {
    let (out, err, result) = session("\\c 0 0\n\\c\n([] a:!2000)\n(!1500)!!1500\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("  1000 0\n"), "{out}");
    let table = out.split("  a\n").nth(1).unwrap();
    let table = &table[..table.find("2000 rows").unwrap() + "2000 rows".len()];
    assert_eq!(table.lines().count(), 999, "{table}");
    assert!(table.ends_with("995\n..\n2000 rows"), "{table}");
    assert!(out.contains("997 | 997\n..\n1500 entries\n"), "{out}");
}

#[test]
fn repl_console_size_rejects_invalid_sizes() {
    let (out, err, result) =
        session("\\c 4 20\n\\c 5 19\n\\c 1001 80\n\\c 5\n\\c a b\n\\c 1 2 3\n\\c\n\\q\n");
    result.unwrap();
    assert_eq!(
        err.matches("5 to 1000 rows (0 for 1000) and at least 20 columns (0 for no limit)")
            .count(),
        3,
        "{err}"
    );
    assert_eq!(
        err.matches("usage: \\c [rows columns | auto]").count(),
        3,
        "{err}"
    );
    assert!(out.contains("  1000 0 (auto)\n"), "{out}");
}

#[test]
fn repl_eof_and_mismatched_delimiters() {
    assert!(
        session("(\n")
            .2
            .unwrap_err()
            .to_string()
            .contains("incomplete")
    );
    session("(\n\\q\n").2.unwrap();
    let (out, err, result) = session("(]\n3\n");
    result.unwrap();
    assert!(err.contains("error:"));
    assert!(out.contains("3\n"));
    for source in ["", "// (", "1 // [", "()", "[]", "{}", "([)]", ")"] {
        assert!(!incomplete(source), "{source}");
    }
    for source in ["(", "{[x]", "(\n// )\n"] {
        assert!(incomplete(source), "{source}");
    }
}

#[test]
fn repl_accepts_symbol_vectors_and_recovers_after_invalid_literals() {
    let (out, err, result) = session("codes:(`USD\n`EUR)\ncodes[1]\n`\ncodes[0]\n\\q\n");
    result.unwrap();
    assert!(out.contains("`USD`EUR\n"));
    assert!(out.contains("`EUR\n"));
    assert!(out.contains("`USD\n"));
    assert!(err.is_empty(), "{err}");
}

#[test]
fn repl_keeps_dictionaries_after_lookup_errors() {
    let (out, err, result) =
        session("prices:`USD`EUR!(\n1;1.08)\nprices `missing\n.prices\nprices `EUR\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("USD | 1\nEUR | 1.08\n"));
    assert!(out.contains("(1;1.08)\n"));
    assert!(out.contains("1.08\n"));
}

#[test]
fn repl_keeps_shared_mutations_and_recovers_from_invalid_updates() {
    let (out, err, result) =
        session("a:1 2\nb:a\na[0]:9\nb\na[0 2]:7 8\nb\nd:()!()\ne:d\nd[`a]:3\ne\n\\q\n");
    result.unwrap();
    assert_eq!(out.matches("9 2\n").count(), 2);
    assert!(out.contains("a | 3\n"));
    assert!(err.contains("index out of bounds"));
}

#[test]
fn repl_propagates_io_errors() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("broken output"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("broken flush"))
        }
    }
    let mut interpreter = Interpreter::new();
    assert!(repl_io(&mut interpreter, &b""[..], &mut Broken, &mut Vec::new()).is_err());
    assert!(
        repl_io(
            &mut interpreter,
            &b"\xff\n"[..],
            &mut Vec::new(),
            &mut Vec::new()
        )
        .is_err()
    );
    assert!(
        repl_io(
            &mut interpreter,
            &b"1+`bad\n"[..],
            &mut Vec::new(),
            &mut Broken
        )
        .is_err()
    );
}

#[test]
fn repl_delimiters_inside_strings_and_double_slash_comments_do_not_continue_input() {
    let (out, err, result) = session("\"(]\"\n1// { ignored\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("\"(]\""), "{out}");
    assert!(out.contains("1\n"), "{out}");
    assert!(!out.contains(".."), "{out}");
}

#[test]
fn repl_single_slash_keeps_iterators_and_delimiters_active() {
    assert!(incomplete("+ /("));
    assert!(!incomplete("+ // ("));
    let (out, err, result) = session("+ /(\n1 2 3\n)\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert!(out.contains(".."), "{out}");
    assert!(out.contains("6\n"), "{out}");
}

#[test]
fn repl_renders_tables_as_rows() {
    let (out, err, result) = session("t:([] name:`alice`bob;age:30 25 40)\n#t[`name]\n\\q\n");
    result.unwrap();
    assert!(err.is_empty(), "{err}");
    assert!(
        out.contains("name  age\n----- ---\nalice  30\nbob    25\n0n     40\n"),
        "{out}"
    );
    assert!(out.contains("  2\n"), "{out}");
}

#[test]
fn repl_records_complete_expressions_including_errors_but_not_cancelled_input() {
    struct Recording<'a> {
        lines: std::str::Lines<'a>,
        history: &'a mut Vec<String>,
    }
    impl input::Input for Recording<'_> {
        fn line(&mut self, _: &str, _: &mut impl Write) -> io::Result<Option<String>> {
            Ok(self.lines.next().map(str::to_owned))
        }
        fn remember(&mut self, source: &str) {
            self.history.push(source.to_owned());
        }
    }
    let mut history = Vec::new();
    repl_io(
        &mut Interpreter::new(),
        Recording {
            lines: "\n(1+\n\\h\n2)\nmissing\n(\n\\d\n(\n\\q\n".lines(),
            history: &mut history,
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(history, ["(1+\n2)\n", "missing\n"]);
}

#[test]
fn terminal_history_survives_sessions_and_preserves_multiline_entries() {
    use input::Input;
    use rustyline::history::{History, SearchDirection};

    let path = std::env::temp_dir().join(format!(
        "pliq-history-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let mut terminal = input::Terminal::with_history(Some(path.clone())).unwrap();
        terminal.remember("1+2\n");
        terminal.remember("(3+\n4)\n");
    }
    {
        let mut terminal = input::Terminal::with_history(Some(path.clone())).unwrap();
        terminal.remember("(3+\n4)\n"); // Consecutive duplicates are ignored.
        terminal.remember("5\n");
    }
    let mut editor = rustyline::DefaultEditor::new().unwrap();
    editor.load_history(&path).unwrap();
    let history = editor.history();
    assert_eq!(history.len(), 3);
    for (index, expected) in ["1+2", "(3+\n4)", "5"].into_iter().enumerate() {
        assert_eq!(
            history
                .get(index, SearchDirection::Forward)
                .unwrap()
                .unwrap()
                .entry,
            expected
        );
    }
    fs::remove_file(path).unwrap();
}
