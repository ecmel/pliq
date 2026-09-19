use pliq::{Interpreter, Value};
use std::{process::Command, rc::Rc};

fn check(source: &str, expected: &str) {
    let value = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(value.to_string(), expected, "{source}");
}

#[test]
fn symbols_are_atomic_names_not_variable_references() {
    check("`USD", "`USD");
    check("USD:42;`USD", "`USD");
    check("currency:`USD;currency", "`USD");
    check("`asset123", "`asset123");
    check("#`USD", "1");
    check("*`USD", "`USD");
    check("`USD // comment\n`EUR", "`EUR");
    check("`USD\n`EUR", "`EUR");
}

#[test]
fn symbol_strands_and_function_calls_remain_distinct() {
    check("`USD`EUR`TRY", "`USD`EUR`TRY");
    check("`USD `EUR `TRY", "`USD`EUR`TRY");
    check("(`USD\n`EUR)", "`USD`EUR");
    check("identity:{x};identity `USD", "`USD");
    check("identity:{x};identity`USD`EUR", "`USD`EUR");
    check("size:{#x};size `USD`EUR", "2");
    check("size:{#x};size[`USD`EUR]", "2");
    check("{x=y}[`USD;`USD]", "1");
    check("{`x}[]", "`x"); // Symbols do not contribute to implicit arity.
    check("{`y`z}[]", "`y`z");
    check("{[currency]{[]currency}}[`USD][]", "`USD");
    check("$[1;`USD;missing]", "`USD");
    check("isUSD:{x=`USD};isUSD' `USD`EUR", "1 0");
}

#[test]
fn symbol_comparisons_broadcast_and_match_by_name() {
    check("`USD=`USD", "1");
    check("`USD=`usd", "0");
    check("`USD~`USD", "1");
    check("`USD`EUR~`USD`EUR", "1");
    check("`USD`EUR~`EUR`USD", "0");
    check("`USD=1", "0");
    check("1=`USD", "0");
    check("`USD~1", "0");
    check("`USD`EUR=`USD", "1 0");
    check("`USD=`USD`EUR", "1 0");
    assert!(Interpreter::new().eval("(`USD;1)=(`USD;1)").is_err());
    check("(`USD`EUR;`TRY`USD)=`USD", "(1 0;0 1)");
    check("`EUR<`USD", "1");
    check("`USD>`EUR", "1");
    check("`A<`a", "1");
    check("`asset10<`asset2", "1");
    check("=[`USD;`USD]", "1");
}

#[test]
fn structural_operations_support_homogeneous_symbols() {
    for (source, expected) in [
        ("a:`USD`EUR`TRY;a[1]", "`EUR"),
        ("`USD`EUR`TRY@2 0", "`TRY`USD"),
        ("(`USD`EUR)[-1]", "0n"),
        ("(`USD`EUR)[()]", "()"),
        ("|`USD`EUR", "`EUR`USD"),
        ("5#`USD`EUR", "`USD`EUR`USD`EUR`USD"),
        ("(-3)#`USD`EUR", "`EUR`USD`EUR"),
        ("0#`USD", "()"),
        ("1_`USD`EUR", ",(`EUR)"),
        ("(-1)_`USD`EUR", ",(`USD)"),
        ("`USD,`EUR", "`USD`EUR"),
        ("?`USD`EUR`USD", "`USD`EUR"),
        ("`USD`EUR?`EUR`GBP", "1 2"),
        ("`USD`EUR?1", "2"),
        (",/(`USD`EUR;`TRY`GBP)", "`USD`EUR`TRY`GBP"),
        ("a:`USD`EUR;b:|a;a", "`USD`EUR"),
    ] {
        check(source, expected);
    }
}

#[test]
fn sort_and_grade_use_stable_case_sensitive_lexical_order() {
    check("asc `USD`EUR`TRY", "`EUR`TRY`USD");
    check("<`USD`EUR`USD`EUR", "1 3 0 2");
    check(">`USD`EUR`USD`EUR", "0 2 1 3");
    check("asc `b`A`a", "`A`a`b");
    check("asc `USD", "`USD");
    check("<,`USD", ",(0)");
    check("asc ()", "()");
}

#[test]
fn symbol_display_round_trips_in_all_array_shapes() {
    for source in [
        "`USD",
        ",`USD",
        "`USD`EUR",
        "(`USD`EUR;`TRY`GBP)",
        ",(`USD`EUR)",
    ] {
        let printed = Interpreter::new().eval(source).unwrap().to_string();
        check(&format!("({source})~({printed})"), "1");
    }
}

#[test]
fn invalid_symbol_syntax_and_numeric_uses_report_errors() {
    for (source, message) in [
        ("`é", "ASCII"),
        ("`USDé", "ASCII"),
        ("`USD:1", "assignment target"),
        ("`USD+1", "requires numbers"),
        ("`USD*`EUR", "requires numbers"),
        ("-`USD", "requires numbers"),
        ("round[`USD;2]", "requires numbers"),
        ("$[`USD;1;2]", "expected a number"),
        ("!`USD", "expected a number"),
        ("`USD<1", "comparable"),
        ("1>`USD", "comparable"),
        ("(`USD`EUR)[`USD]", "index must be a number"),
        ("`USD[0]", "indexing requires an array"),
        ("`USD`EUR=`USD`EUR`TRY", "length mismatch"),
    ] {
        let error = Interpreter::new().eval(source).expect_err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn symbol_clones_share_text_without_an_intern_pool() {
    let mut language = Interpreter::new();
    let Value::Symbol(original) = language.eval("a:`USD").unwrap() else {
        panic!("expected symbol")
    };
    let Value::Symbol(alias) = language.eval("b:a;b").unwrap() else {
        panic!("expected symbol")
    };
    assert!(Rc::ptr_eq(&original, &alias));
    let Value::Symbol(other) = Interpreter::new().eval("`USD").unwrap() else {
        panic!("expected symbol")
    };
    assert_eq!(original, other);
    assert!(!Rc::ptr_eq(&original, &other));
    let Value::Array(numbers) = language.eval("1 2 3").unwrap() else {
        panic!("expected array")
    };
    assert!(numbers.as_numbers().is_some());
    let Value::Array(symbols) = language.eval("a,a").unwrap() else {
        panic!("expected array")
    };
    assert!(symbols.as_numbers().is_none());
    for value in symbols.iter() {
        let Value::Symbol(name) = value else {
            panic!("expected symbol")
        };
        assert_eq!(original, name);
    }
}

#[test]
fn cli_prints_symbols() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["-e", "codes:`USD`EUR`TRY;codes[1]"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"`EUR\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn mixed_symbol_arrays_are_rejected() {
    for source in [
        "`USD,42",
        "42,`USD",
        "?(`USD;1;`USD;1)",
        "(`USD;1)?`USD",
        "(`USD;())",
        "(();`USD)",
    ] {
        assert!(
            Interpreter::new()
                .eval(source)
                .unwrap_err()
                .contains("homogeneous"),
            "{source}"
        );
    }
}
