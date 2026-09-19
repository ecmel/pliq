use pliq::{Console, Interpreter, Value};

fn value(source: &str) -> Value {
    Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"))
}

fn shown(source: &str, rows: usize, columns: usize) -> String {
    value(source).view(Console { rows, columns }).to_string()
}

#[test]
fn unlimited_views_match_ordinary_display() {
    for source in ["1 2 3", "`a`b", "\"text\"", "(1 2;3 4)", "{x}", "0n"] {
        let value = value(source);
        assert_eq!(
            value.view(Console::UNLIMITED).to_string(),
            value.to_string(),
            "{source}"
        );
    }
    let Value::Table(table) = value("([] a:1 2;b:`x`y)") else {
        panic!("expected a table");
    };
    assert_eq!(
        Value::Table(table.clone())
            .view(Console::UNLIMITED)
            .to_string(),
        table.to_string()
    );
    let Value::Dictionary(dictionary) = value("`a`b!1 2") else {
        panic!("expected a dictionary");
    };
    assert_eq!(
        Value::Dictionary(dictionary.clone())
            .view(Console::UNLIMITED)
            .to_string(),
        dictionary.to_string()
    );
}

#[test]
fn values_that_fit_are_unchanged() {
    for source in ["1 2 3", "([] a:1 2;b:`x`y)", "`a`b!1 2", "()!()", "10#1"] {
        assert_eq!(
            shown(source, 12, 19),
            shown(source, 0, 0),
            "{source} should fit"
        );
    }
}

#[test]
fn wide_lines_end_in_two_dots_within_the_width() {
    assert_eq!(shown("!1000", 0, 20), "0 1 2 3 4 5 6 7 8 ..");
    assert_eq!(shown("10#1", 0, 18), "1 1 1 1 1 1 1 1 ..");
    assert_eq!(shown("!1000000", 5, 20), "0 1 2 3 4 5 6 7 8 ..");
    assert_eq!(
        shown("`a`b!(!100000;1)", 0, 20),
        "a | 0 1 2 3 4 5 6 ..\nb | 1"
    );
    let table = shown("([] a:1 2;b:(!100;!100))", 0, 20);
    for line in table.lines() {
        assert!(line.chars().count() <= 20, "{line:?} in\n{table}");
    }
    assert!(table.lines().nth(2).unwrap().ends_with(".."), "{table}");
}

#[test]
fn tall_tables_and_dictionaries_show_leading_rows_and_counts() {
    assert_eq!(
        shown("([] a:!10;b:10#`x)", 6, 0),
        "a b\n- -\n0 x\n1 x\n..\n10 rows"
    );
    assert_eq!(
        shown("([] a:!10;b:10#`x)", 12, 0).lines().count(),
        12,
        "a table that fits shows every row"
    );
    assert_eq!(
        shown("(!10)!10#`x", 5, 0),
        "0 | x\n1 | x\n2 | x\n..\n10 entries"
    );
    // Hidden rows do not widen the columns shown.
    assert_eq!(shown("([] a:1 2 1000000 4)", 5, 0), "a\n-\n1\n..\n4 rows");
    assert_eq!(
        shown("(1 2 1000000 4)!1 2 3 4", 3, 0),
        "1 | 1\n..\n4 entries"
    );
}

#[test]
fn tall_compact_values_end_in_a_dots_line() {
    let symbol = "`$\"a\\nb\\nc\\nd\"";
    assert_eq!(shown(symbol, 3, 0), "`a\nb\n..");
    assert_eq!(shown(symbol, 4, 0), "`a\nb\nc\nd");
}

#[test]
fn zero_leaves_a_dimension_unlimited() {
    assert_eq!(shown("!30", 5, 0), shown("!30", 0, 0));
    assert_eq!(
        shown("([] a:!10)", 0, 20).lines().count(),
        12,
        "no row limit"
    );
}
