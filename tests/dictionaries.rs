use pliq::{Array, Dictionary, Interpreter, Number, Value};
use std::{process::Command, rc::Rc};

fn check(source: &str, expected: &str) {
    let value = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(value.to_string(), expected, "{source}");
}

#[test]
fn construction_and_lookup_preserve_key_order() {
    let setup = "prices:`USD`EUR`TRY!1 1.08 0.029;";
    for (source, expected) in [
        ("prices", "(`USD`EUR`TRY!1 1.08 0.029)"),
        ("prices[`EUR]", "1.08"),
        ("prices `EUR", "1.08"),
        ("prices@`EUR", "1.08"),
        ("prices[`TRY`USD`TRY]", "0.029 1 0.029"),
        ("prices `USD`EUR", "1 1.08"),
        ("prices[()]", "()"),
        ("prices[,`USD]", ",(1)"),
        ("!prices", "`USD`EUR`TRY"),
        (".prices", "1 1.08 0.029"),
        ("value[prices]", "1 1.08 0.029"),
        ("#prices", "3"),
    ] {
        check(&format!("{setup}{source}"), expected);
    }
    check("make:!;d:make[`USD`EUR;1 2];d `EUR", "2");
    check("values:{.x};values ((,`USD)!,1)", ",(1)");
}

#[test]
fn single_keys_accept_arbitrary_values_and_vectors_require_matching_counts() {
    check("d:(,`USD)!,1;d", "((,`USD)!,1)");
    check("d:(,`USD)!,1;!d", ",(`USD)");
    check("d:(,`USD)!,1;.d", ",(1)");
    check("d:(,`prices)!,1 2 3;d `prices", "1 2 3");
    check("d:(,`prices)!,1 2 3;#d", "1");
    check("d:(,`prices)!,1 2 3;.d", ",(1 2 3)");
    check("d:(,`prices)!,1 2 3;d `prices", "1 2 3");
    check("d:(,`USD)!,1;d `USD", "1");
    check("d:(,`USD)!,1;d `USD", "1");
    check("d:(,`empty)!,();d `empty", "()");
    check("d:(,`unit)!,`USD;d `unit", "`USD");
}

#[test]
fn dictionaries_can_hold_functions_arrays_and_nested_dictionaries() {
    check("d:`inc`twice!({x+1};{2*x});d[`inc] 4", "5");
    check("d:`inc`twice!({x+1};{2*x});d[`twice][4]", "8");
    check("d:(,`prices)!,(`USD`EUR!1 2);d[`prices][`EUR]", "2");
    check("d:`amounts`unit!(10 20;`USD);d.unit", "`USD");
    check(
        "d:`first`second!(10 20;30 40);d `second`first",
        "(30 40;10 20)",
    );
    check("lookup:{[d;k]d k};lookup[(,`USD)!,5;`USD]", "5");
    check("make:{[n](,`value)!,n};(make 3) `value", "3");
    check("d:(,`USD)!,3;f:{d x};d:(,`USD)!,4;f `USD", "3");
    check("d:(,`USD)!,3;alias:d;d:(,`USD)!,4;alias `USD", "3");
    check(
        "d:(,`USD)!,3;keys:!d;values:.d;keys:|keys;values:values+1;d `USD",
        "3",
    );
}

#[test]
fn empty_dictionaries_are_distinct_from_empty_arrays() {
    check("()!()", "(()!())");
    check("d:()!();#d", "0");
    check("d:()!();!d", "()");
    check("d:()!();.d", "()");
    check("d:()!();d[()]", "()");
    check("(()!())~()", "0");
    check("(()!())~(()!())", "1");
}

#[test]
fn dictionary_match_compares_ordered_keys_and_values() {
    check("(`a`b!1 2)~(`a`b!1 2)", "1");
    check("(`a`b!1 2)~(`b`a!2 1)", "0");
    check("(`a`b!1 2)~(`a`b!1 3)", "0");
    check("((,`a)!,1)~((,`b)!,1)", "0");
    check("((,`a)!,1)~(`a`b!1 2)", "0");
    check("((,`a)!,1)~1", "0");
    check("((,`a)!,((,`b)!,2))~((,`a)!,((,`b)!,2))", "1");
    check("d:(,`a)!,1;? (d;d)", ",(((,`a)!,1))");
}

#[test]
fn dictionary_display_preserves_nested_structure() {
    for source in [
        "()!()",
        "(,`a)!,1",
        "(,`a)!,(-1)",
        "`a`b!1 2",
        "(,`a)!,`b",
        "(,`a)!,1 2",
        "(,`a)!,()",
        "(,`a)!,((,`b)!,1)",
        "`a`b!(1 2;3 4)",
        "((,`a)!,1;(,`b)!,2)",
        "(,`a)!,,1",
        "`a`b!`USD`EUR",
        "(,`a)!,1",
    ] {
        let printed = Interpreter::new().eval(source).unwrap().to_string();
        check(&format!("({source})~({printed})"), "1");
    }
}

#[test]
fn invalid_keys_lengths_and_lookups_return_errors() {
    for (source, message) in [
        ("`a`b!1", "requires key and value arrays"),
        ("`a`b!1 2 3", "length mismatch"),
        ("(,`a)!()", "length mismatch"),
        (".[1;2]", "argument list"),
        (". 1", ". requires"),
        (".(`a)", "undefined name"),
        ("$[(,`a)!,1;2;3]", "expected a number"),
    ] {
        let error = Interpreter::new().eval(source).expect_err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn numeric_keys_construct_dictionaries_and_mod_is_explicit() {
    check(".1", "0.1");
    check(".5+1.25", "1.75");
    check("mod[7;2]", "1");
    check("2 3!5 7", "(2 3!5 7)");
    check("(2 3!5 7)[3]", "7");
    check("(2 3!5 7)[4]", "0n");
    check("(`a`a!1 2)[`a]", "1");
    check("d:(,`a)!,0.5;d `a", "0.5");
}

#[test]
fn rust_api_preserves_packed_values_and_checks_construction() {
    let values = Array::numbers(vec![Number::Int(1), Number::Int(2)]);
    let dictionary = Dictionary::new(vec![Rc::from("a"), Rc::from("b")], values.clone()).unwrap();
    assert_eq!(dictionary.len(), 2);
    assert!(!dictionary.is_empty());
    assert_eq!(dictionary.keys().get(0).unwrap().to_string(), "`a");
    assert_eq!(dictionary.get("b").unwrap().to_string(), "2");
    assert!(dictionary.get("missing").is_none());
    assert_eq!(
        dictionary.values().to_arrow().unwrap().to_data().buffers()[0].as_ptr(),
        values.to_arrow().unwrap().to_data().buffers()[0].as_ptr()
    );
    let alias = dictionary.clone();
    drop(dictionary);
    assert_eq!(alias.get("a").unwrap().to_string(), "1");
    assert!(Dictionary::new(vec![Rc::from("a")], values.clone()).is_err());
    assert!(Dictionary::new(vec![Rc::from("a"), Rc::from("a")], values).is_ok());
    let empty = Dictionary::new(vec![], Array::numbers(vec![])).unwrap();
    assert!(empty.is_empty());
    assert_eq!(Value::Dictionary(empty).to_string(), "(()!())");
}

#[test]
fn cli_evaluates_dictionary_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["-e", "prices:`USD`EUR`TRY!1 1.08 0.029;prices[`EUR]"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"1.08\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn dot_access_uses_literal_symbol_keys_and_composes_with_postfixes() {
    for (source, expected) in [
        ("d:`a`b!10 20;a:`b;d.a", "10"),
        ("d:`a`b!10 20;d.missing", "0n"),
        ("d:(,`child)!,((,`value)!,42);d.child.value", "42"),
        ("d:(,`items)!,10 20;d.items[1]", "20"),
        ("d:(,`child)!,((,`value)!,42);d[`child].value", "42"),
        ("((,`a)!,7).a", "7"),
        ("d:(,`inc)!,{x+1};d.inc[4]", "5"),
        ("d:(,`items)!,10 20;+/d.items", "30"),
        ("d:(,`price.usd)!,7;d[`price.usd]", "7"),
        ("d:(,`a1)!,7;d.a1", "7"),
        ("d:(,`a)!,7;key:,`a;d . key", "7"),
        ("d:(,`a)!,7;key:,`a;d. key", "7"),
        ("d:(,`a)!,7;key:,`a;d .key", "7"),
        ("d:(,`a)!,7;key:,`a;(d// comment\n.key)", "7"),
        ("d:(,`a)!,7;.d", ",(7)"),
        ("d:(,`a)!,7;#.d", "1"),
        (".5+1.5", "2"),
        ("f:{x};f.a", "`a"),
    ] {
        check(source, expected);
    }
}

#[test]
fn dot_updates_infer_empty_dictionary_types_and_preserve_shared_identity() {
    check("d:()!();d.name", "0n");
    check("d:()!();alias:d;d.name:42;d.count:10;alias.name", "42");
    check("d:()!();d.name:42;d.name+:1;d.name", "43");
    check("d:()!();d.items:10 20;d.items,:30;d.items", "10 20 30");
    check("d:()!();d.child:()!();d.child.value:42;d.child.value", "42");
    check("d:()!();d.items:10 20;d.items[0]:99;d.items", "99 20");
    check("d:()!();d.name:1;f:{d.name};d.name:2;f[]", "2");

    let mut interpreter = Interpreter::new();
    interpreter.eval("d:()!();alias:d").unwrap();
    assert!(interpreter.eval("d.child.value:42").is_err());
    assert_eq!(interpreter.eval("#alias").unwrap().to_string(), "0");
    assert!(interpreter.eval("d.self:d").is_err());
    assert_eq!(interpreter.eval("#alias").unwrap().to_string(), "0");
    interpreter.eval("d.name:42").unwrap();
    interpreter.eval("d.label:\"hello\"").unwrap();
    assert_eq!(interpreter.eval("alias.name").unwrap().to_string(), "42");
    assert_eq!(interpreter.eval("#alias").unwrap().to_string(), "2");
}

#[test]
fn mixed_entries_support_records_replacement_and_literal_construction() {
    for (source, expected) in [
        ("d:()!();d.name:\"alice\";d.count:0;d.name", "\"alice\""),
        ("d:()!();d.name:\"alice\";d.count:0;d.count+:1;d.count", "1"),
        ("d:`name`count!(\"alice\";0);.d", "(\"alice\";0)"),
        (
            "d:`name`count!(\"alice\";0);d[`count`name`missing]",
            "(0;\"alice\";0n)",
        ),
        ("d:`name`count!(\"alice\";0);@d[`count`count]", "7"),
        (
            "d:`name`count!(\"alice\";0);alias:d;f:{d.count};d.count:`zero;f[]",
            "`zero",
        ),
        ("d:()!();d.a:1;d.b:2.5;@d.a", "-7"),
        ("d:()!();d.a:1;d.a:2.5;@d.a", "-9"),
        ("d:`a`b!(0n;1);d.a:\"text\";d.b", "1"),
        ("a:,0;d:`left`right!((a[0]:2);(a[0]:1));a[0]", "2"),
        ("d:![`name`count;(\"alice\";0)];d.count", "0"),
        ("d:`a`a!(\"first\";2);d.a:3;d[`a]", "3"),
    ] {
        check(source, expected);
    }
}

#[test]
fn mixed_dictionary_operations_preserve_entries_and_outer_snapshots() {
    let setup = "d:`name`count`items!(\"alice\";0;1 2);";
    for (expression, expected) in [
        ("v:.d;d.count:9;v[1]", "0"),
        ("v:.d;v[0]:42;d.name", "\"alice\""),
        ("v:.d;v[2][0]:9;d.items", "9 2"),
        ("r:|d;r.name", "\"alice\""),
        ("r:2#d;r.count", "0"),
        ("r:1_d;r.items", "1 2"),
        ("r:(,`items)_d;r.name", "\"alice\""),
        ("r:d,((,`count)!,`zero);r.count", "`zero"),
        ("r:d,((,`other)!,`ok);r.other", "`ok"),
        ("r:^d;r.items", "0 0"),
        ("d~`name`count`items!(\"alice\";0;1 2)", "1"),
        ("r:2 3#.d;#r", "2"),
        ("v:{x}'(.d);v[1]", "0"),
    ] {
        check(&format!("{setup}{expression}"), expected);
    }
    check("d:`a`b!(1;2.5);r:d+1;@r.a", "-7");
    check("d:`a`b!(1;2.5);.d", "(1;2.5)");
    check("d:`a`b!(1 2;`x`y);d[`a`b;0]", "(1;`x)");
    check(
        "a:(,`count)!,1;b:(,`name)!,\"alice\";r:a+b;r.name",
        "\"alice\"",
    );
    let value = Interpreter::new()
        .eval("`name`count!(\"alice\";0)")
        .unwrap();
    check(&format!("d:{};d.name", value), "\"alice\"");
}

#[test]
fn mixed_dictionaries_keep_cycles_key_types_and_array_types_checked() {
    let mut interpreter = Interpreter::new();
    interpreter
        .eval("d:`name`count`items!(\"alice\";0;1 2);alias:d")
        .unwrap();
    for expression in [
        "d[0]:42",
        "d.self:d",
        "d.self:{d}",
        "d.items[0]:`wrong",
        "d[`count`items;0]:7",
        "d.child.value:42",
        "([] mixed:.d)",
        "t:([] a:1 2);t.other:.d",
        "(.d)!1 2 3",
    ] {
        assert!(interpreter.eval(expression).is_err(), "{expression}");
        assert_eq!(interpreter.eval("alias.count").unwrap().to_string(), "0");
        assert_eq!(interpreter.eval("alias.items").unwrap().to_string(), "1 2");
        assert_eq!(interpreter.eval("#alias").unwrap().to_string(), "3");
    }
    // The first edit succeeds, but the second would create a cycle: roll back both.
    interpreter.eval("replacements:`count`self!(7;d)").unwrap();
    assert!(interpreter.eval("d[`count`self]:.replacements").is_err());
    assert_eq!(interpreter.eval("alias.count").unwrap().to_string(), "0");
    assert_eq!(interpreter.eval("#alias").unwrap().to_string(), "3");
}

#[test]
fn rust_dictionary_mixed_values_preserve_types_and_snapshots() {
    let keys = Array::new([Value::Symbol("name".into()), Value::Symbol("count".into())]).unwrap();
    let dictionary = Dictionary::from_values(
        keys,
        [Value::Symbol("alice".into()), Value::Number(Number::Int(0))],
    )
    .unwrap();
    let snapshot = dictionary.values();
    assert!(snapshot.to_arrow().is_err());
    dictionary
        .insert("name".into(), Value::Number(Number::Float(2.5)))
        .unwrap();
    assert!(matches!(
        dictionary.get("count"),
        Some(Value::Number(Number::Int(0)))
    ));
    assert!(matches!(snapshot.get(0), Some(Value::Symbol(_))));
    dictionary
        .insert("name".into(), Value::Number(Number::Int(2)))
        .unwrap();
    assert!(dictionary.values().to_arrow().is_ok());
}
