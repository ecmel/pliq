use pliq::{Array, Dictionary, Interpreter, Number, Value};
use std::rc::Rc;

fn integer(n: i128) -> Number {
    Number::Int(i64::try_from(n).unwrap())
}

fn check(source: &str, expected: &str) {
    let value = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(value.to_string(), expected, "{source}");
}

#[test]
fn array_updates_are_visible_through_aliases() {
    check("a:1 2 3;b:a;a[0]:9;b", "9 2 3");
    check("a:1 2 3;a[2]:9;a", "1 2 9");
    assert!(Interpreter::new().eval("a:1 2;b:a;a[0]:`USD;b").is_err());
    assert!(Interpreter::new().eval("a:1 2;a[0]:9 8;a").is_err());
    assert!(Interpreter::new().eval("a:1 2;a[0]:{x+1};a[0][4]").is_err());
    check("a:1 2;b:a;a:3 4;a[0]:9;(a;b)", "(9 4;1 2)");
    check("a:1 2;a[0]:9", "9");
}

#[test]
fn in_place_append_preserves_aliases_and_concatenates_one_level() {
    check("a:1 2;b:a;a,:3;b", "1 2 3");
    check("a:1 2;a,:3 4", "1 2 3 4");
    check("a:1 2;a,:();a", "1 2");
    check("a:();b:a;a,:1;b", ",(1)");
    assert!(Interpreter::new().eval("a:();a,:`USD;a,:2 3;a").is_err());
    assert!(Interpreter::new().eval("a:1 2;a,:`USD`EUR;a").is_err());
    check("a:,`USD;a,:`EUR`TRY;a", "`USD`EUR`TRY");
    assert!(Interpreter::new().eval("a:1 2;a,:,3 4;a").is_err());
    assert!(Interpreter::new().eval("a:1 2;a,:(,`x)!,3;a").is_err());
    assert!(Interpreter::new().eval("a:1 2;a,:{x+1};a[2][4]").is_err());
    check("a:1 2;b:a;a,:a;b", "1 2 1 2");
    assert!(Interpreter::new().eval("a:(1;`USD);a,:a;a").is_err());
    assert!(
        Interpreter::new()
            .eval("a:,0;child:1 2;a,:,child;child,:3;a")
            .is_err()
    );
}

#[test]
fn append_targets_nested_arrays_and_captured_containers() {
    check("a:1 2;d:(,`items)!,a;d[`items],:3;a", "1 2 3");
    check("a:1 2;m:,a;m[0],:3;a", "1 2 3");
    check("a:1 2;f:{a,:x};f[3];a", "1 2 3");
    check("a:1 2;make:{[]{a,:x}};f:make[];f[3];a", "1 2 3");
    check("f:{x,:y};a:1 2;f[a;3 4];a", "1 2 3 4");
    check("a:1 2;b:a;f:{a,:x};a:8 9;f[3];b", "1 2 3");
    check("a:1 2;b:a;a,:(a:3 4);(a;b)", "(3 4 3 4;1 2)");
    check("a:1 2;b:,3;a,:b,:4;(a;b)", "(1 2 3 4;3 4)");
}

#[test]
fn append_during_iteration_uses_the_original_elements() {
    check("a:1 2 3;f:{a,:9;x};f'a", "1 2 3");
    check("a:1 2 3;f:{a,:9;x};f'a;a", "1 2 3 9 9 9");
    check("a:1 2 3;f:{[x;y]a,:9;x+y};f/a", "6");
}

#[test]
fn failed_append_preserves_the_target_and_rejects_cycles() {
    for (setup, append, inspect, expected, error) in [
        ("a:1 2", "a,:,a", "a", "1 2", "reference cycle"),
        ("a:1 2", "a,:(3;a)", "a", "1 2", "homogeneous"),
        ("a:1 2;d:(,`a)!,a", "a,:d", "a", "1 2", "reference cycle"),
        ("a:1 2;f:{[]a}", "a,:f", "a", "1 2", "reference cycle"),
        ("a:1 2", "a,:1+`bad", "a", "1 2", "requires numbers"),
        ("a:1", "a,:2", "a", "1", "requires an array"),
        ("d:()!()", "d,:2", "d", "(()!())", "requires an array"),
    ] {
        let mut interpreter = Interpreter::new();
        interpreter.eval(setup).unwrap();
        let actual = interpreter.eval(append).unwrap_err();
        assert!(actual.contains(error), "{append}: {actual}");
        assert_eq!(interpreter.eval(inspect).unwrap().to_string(), expected);
    }
    assert!(
        Interpreter::new()
            .eval("missing,:1")
            .unwrap_err()
            .contains("undefined name")
    );
    assert!(Interpreter::new().eval("a,:").is_err());
}

#[test]
fn rust_append_preserves_numeric_packing_and_read_snapshots() {
    let array = Array::numbers(vec![integer(1), integer(2)]);
    let alias = array.clone();
    let snapshot = array.as_numbers().unwrap();
    let iter = array.iter();
    array.append(Value::Number(integer(3))).unwrap();
    assert_eq!(alias.len(), 3);
    assert_eq!(snapshot.len(), 2);
    assert_eq!(iter.len(), 2);
    array.append(Value::Array(alias.clone())).unwrap();
    assert_eq!(
        &array.as_numbers().unwrap()[..],
        &[
            integer(1),
            integer(2),
            integer(3),
            integer(1),
            integer(2),
            integer(3)
        ]
    );
    assert!(
        array
            .append(Value::array([Value::Array(alias)]).unwrap())
            .is_err()
    );
    assert_eq!(array.len(), 6);

    let values = Array::numbers(vec![integer(1)]);
    let dictionary = Dictionary::new(vec![Rc::from("a")], values.clone()).unwrap();
    values.append(Value::Number(integer(2))).unwrap();
    dictionary
        .values()
        .append(Value::Number(integer(3)))
        .unwrap();
    assert_eq!(dictionary.len(), 1);
    assert_eq!(dictionary.values().len(), 1);
}

#[test]
fn vector_updates_pair_values_broadcast_scalars_and_keep_order() {
    check("a:1 2 3;a[0 2]:8 9;a", "8 2 9");
    check("a:1 2 3;a[0 2]:9;a", "9 2 9");
    check("a:1 2;a[1 0]:a;a", "2 1");
    check("a:1 2;a[0 0]:8 9;a", "9 2");
    assert!(Interpreter::new().eval("a:1 2;a[,0]:,7 8;a").is_err());
    check("a:1 2;a[()]:9;a[()]:();a", "1 2");
}

#[test]
fn dictionary_updates_replace_and_append_keys_through_aliases() {
    check("d:`a`b!1 2;e:d;d[`a]:9;e", "(`a`b!9 2)");
    check("d:(,`a)!,1;e:d;d[`b]:2;e", "(`a`b!1 2)");
    check("d:()!();d[`a]:1 2;d[`a]", "1 2");
    check("d:(,`a)!,1;d[`b`a`b]:2 3 4;d", "(`a`b!3 4)");
    check("d:()!();d[`a`b]:7;d", "(`a`b!7 7)");
    check("d:(,`a)!,1;d[`a]:`USD;d.a", "`USD");
    check("d:(,`a)!,1;d[()]:();d", "((,`a)!,1)");
}

#[test]
fn nested_updates_preserve_shared_children() {
    check("row:1 2;m:(row;row);m[0][1]:9;m", "(1 9;1 9)");
    check("m:(1 2;3 4);m[0 1][0]:7 8;m", "(7 2;8 4)");
    check("m:(1 2;3 4);m[0 1][0 1]:(5 6;7 8);m", "(5 6;7 8)");
    check("a:1 2;d:(,`items)!,a;d[`items][0]:9;a", "9 2");
    check("d:(,`a)!,1;m:,d;m[0][`b]:2;d", "(`a`b!1 2)");
    check(
        "d:(,`child)!,((,`a)!,1);e:d[`child];d[`child][`b]:2;e",
        "(`a`b!1 2)",
    );
}

#[test]
fn indexed_assignment_follows_right_to_left_evaluation() {
    check("a:0 0 0;i:0;a[i:i+1]:i:i+1;a,i", "0 0 1 2");
    check("a:(0 0;0 0);i:0;a[i:i+1][i]:9;a", "(0 0;9 0)");
    check("a:1 2;b:a;a[0]:(a:3 4)[1];(a;b)", "(4 4;1 2)");
    check("a:1 2;b:3 4;a[0]:b[1]:9;(a;b)", "(9 2;3 9)");
}

#[test]
fn literals_are_new_containers_on_every_evaluation() {
    check("f:{[] v:1 2 3;r:v[0];v[0]:9;r};(f[];f[])", "1 1");
    check("f:{[] v:`a`b;v,:`c;#v};(f[];f[])", "3 3");
    check("f:{[] e:();e,:1;#e};(f[];f[])", "1 1");
    check("f:{[] v:(+;-);v[0]:*;v[0][2;3]};(f[];f[])", "6 6");
    check("f:{[] s:\"ab\";r:s[0];s[0]:\"z\";r};(f[];f[])", "\"aa\"");
}

#[test]
fn closures_and_function_arguments_share_mutable_containers() {
    check("a:1 2;f:{[]a[0]};a[0]:9;f[]", "9");
    check("a:1 2;f:{a[0]:x};f[9];a", "9 2");
    check("a:1 2;make:{[]{a[0]:x}};f:make[];f[9];a", "9 2");
    check("a:1 2;f:{a[0]:x};b:a;a:3 4;f[9];(a;b)", "(3 4;9 2)");
    check("f:{x[0]:9};a:1 2;f[a];a", "9 2");
    check("f:{x[y]:z};a:1 2;f[a;1;9];a", "1 9");
    check("d:(,`a)!,1;f:{d[`a]:x};f[9];d", "((,`a)!,9)");
    check("a:1 2;f:{[]a:3 4;a[0]:9;a};(f[];a)", "(9 4;1 2)");
    check("a:1 2;f:{[]a[0]:(a:3 4)[1];a};(f[];a)", "(4 4;1 2)");
}

#[test]
fn derived_arrays_and_dictionary_views_have_independent_outer_storage() {
    check("a:1 2;b:|a;b[0]:9;a", "1 2");
    check("a:1 2;b:a+0;b[0]:9;a", "1 2");
    check("a:1 2;b:a,3;b[0]:9;a", "1 2");
    check("a:1 2;b:1#a;b[0]:9;a", "1 2");
    check("a:1 2;b:1_a;b[0]:9;a", "1 2");
    check("a:1 2;d:`a`b!a;a[0]:9;d", "(`a`b!1 2)");
    check("d:`a`b!1 2;k:!d;v:.d;k[0]:`c;v[0]:9;d", "(`a`b!1 2)");
    check("d:`a`b!1 2;v:.d;d[`a]:9;v", "1 2");
    check("a:1 2;d:(,`items)!,a;v:.d;v[0][0]:9;a", "9 2");
}

#[test]
fn iteration_survives_updates_from_callbacks() {
    check("a:1 2 3;f:{a[1]:9;x};f'a", "1 2 3");
    check("a:1 2 3;f:{a[1]:9;x};f'a;a", "1 9 3");
    check("a:1 2 3;f:{[x;y]a[2]:9;x+y};f/a", "6");
}

#[test]
fn invalid_updates_leave_all_destinations_unchanged() {
    for (setup, update, inspect, expected, error) in [
        ("a:1 2", "a[0 2]:8 9", "a", "1 2", "index out of bounds"),
        ("a:1 2", "a[0 1]:,9", "a", "1 2", "length mismatch"),
        ("a:1 2", "a[-3]:9", "a", "1 2", "index out of bounds"),
        ("a:1 2", "a[0.5]:9", "a", "1 2", "not an integer"),
        ("a:1 2", "a[1e20]:9", "a", "1 2", "index out of bounds"),
        ("a:1 2", "a[`a]:9", "a", "1 2", "must be a number"),
        ("a:1 2", "a[,(,0)]:9", "a", "1 2", "flat array"),
        ("a:1", "a[0]:9", "a", "1", "requires an array or dictionary"),
        (
            "a:1 2",
            "a[0][0]:9",
            "a",
            "1 2",
            "requires an array or dictionary",
        ),
        (
            "a:(1 2;,3)",
            "a[0 1][1]:9",
            "a",
            "(1 2;,(3))",
            "index out of bounds",
        ),
        (
            "d:(,`a)!,1",
            "d[`b][0]:9",
            "d",
            "((,`a)!,1)",
            "missing dictionary key",
        ),
        (
            "d:(,`a)!,1",
            "d[`a`b]:,9",
            "d",
            "((,`a)!,1)",
            "length mismatch",
        ),
        ("a:1 2", "a[0]:1+`bad", "a", "1 2", "requires numbers"),
    ] {
        let mut interpreter = Interpreter::new();
        interpreter.eval(setup).unwrap();
        let actual = interpreter.eval(update).expect_err(update);
        assert!(actual.contains(error), "{update}: {actual}");
        assert_eq!(
            interpreter.eval(inspect).unwrap().to_string(),
            expected,
            "{update}"
        );
    }
    let mut interpreter = Interpreter::new();
    interpreter.eval("a:1 2;i:0").unwrap();
    assert!(interpreter.eval("a[i:9]:7").is_err());
    assert_eq!(interpreter.eval("i").unwrap().to_string(), "9");
    assert!(
        interpreter
            .eval("missing[0]:1")
            .unwrap_err()
            .contains("undefined name")
    );
}

#[test]
fn cycles_through_arrays_dictionaries_and_closures_are_rejected() {
    for (setup, update, inspect, expected) in [
        ("a:,0", "a[0]:a", "a", ",(0)"),
        ("a:,0;b:,a", "a[0]:b", "a", ",(0)"),
        ("d:()!()", "d[`self]:d", "d", "(()!())"),
        ("a:,0;d:(,`a)!,a", "a[0]:d", "a", ",(0)"),
        ("a:,0;f:{[]a}", "a[0]:f", "a", ",(0)"),
        ("a:,0;f:{[]a}", "a[0]:f'", "a", ",(0)"),
        ("d:()!();f:{[]d}", "d[`f]:f", "d", "(()!())"),
        ("d:()!()", "d[`ok`self]:(d;d)", "d", "(()!())"),
    ] {
        let mut interpreter = Interpreter::new();
        interpreter.eval(setup).unwrap();
        assert!(
            interpreter
                .eval(update)
                .unwrap_err()
                .contains("reference cycle"),
            "{update}"
        );
        assert_eq!(
            interpreter.eval(inspect).unwrap().to_string(),
            expected,
            "{update}"
        );
    }
}

#[test]
fn rust_mutation_api_preserves_aliases_packing_and_read_snapshots() {
    let array = Array::numbers(vec![integer(1), integer(2)]);
    let alias = array.clone();
    array.set(0, Value::Number(integer(9))).unwrap();
    assert_eq!(alias.get(0).unwrap().to_string(), "9");
    let snapshot = array.as_numbers().unwrap();
    let iter = array.iter();
    array.set(1, Value::Number(integer(8))).unwrap();
    assert_eq!(snapshot[1], integer(2));
    assert_eq!(iter.map(|v| v.to_string()).collect::<Vec<_>>(), ["9", "2"]);
    assert_eq!(alias.get(1).unwrap().to_string(), "8");
    assert!(array.set(0, Value::Symbol(Rc::from("USD"))).is_err());
    assert!(array.as_numbers().is_some());
    assert_eq!(alias.get(0).unwrap().to_string(), "9");
    assert!(array.set(2, Value::Number(integer(0))).is_err());
    assert!(array.set(0, Value::Array(alias)).is_err());

    let dictionary =
        Dictionary::new(vec![Rc::from("a")], Array::numbers(vec![integer(1)])).unwrap();
    let alias = dictionary.clone();
    dictionary
        .insert(Rc::from("b"), Value::Number(integer(2)))
        .unwrap();
    dictionary
        .insert(Rc::from("a"), Value::Number(integer(9)))
        .unwrap();
    assert_eq!(alias.len(), 2);
    assert_eq!(alias.get("a").unwrap().to_string(), "9");
    assert_eq!(alias.get("b").unwrap().to_string(), "2");
    assert!(
        dictionary
            .insert(Rc::from("self"), Value::Dictionary(alias))
            .is_err()
    );
    assert_eq!(dictionary.len(), 2);
}

#[test]
fn replacing_elements_releases_old_values() {
    let mut interpreter = Interpreter::new();
    let Value::Function(function) = interpreter.eval("{x+1}").unwrap() else {
        panic!("expected function");
    };
    let weak = Rc::downgrade(&function);
    let array = Array::new([Value::Function(function)]).unwrap();
    assert!(weak.upgrade().is_some());
    array.set(0, interpreter.eval("{x+2}").unwrap()).unwrap();
    assert!(weak.upgrade().is_none());
}

#[test]
fn invalid_assignment_targets_are_reported() {
    for source in [
        "1:2",
        "(1 2)[0]:3",
        "a[]:1",
        "a[0;1]:2",
        "a[0]:",
        "{x}[0]:1",
    ] {
        assert!(Interpreter::new().eval(source).is_err(), "{source}");
    }
    let source = format!("a{}:1", "[0]".repeat(129));
    assert!(
        Interpreter::new()
            .eval(&source)
            .unwrap_err()
            .contains("nesting limit")
    );
}
