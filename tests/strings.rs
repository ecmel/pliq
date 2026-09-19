use pliq::{Interpreter, MutableString, Value};

fn check(source: &str, expected: &str) {
    let actual = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(actual.to_string(), expected, "{source}");
}

#[test]
fn strings_are_a_distinct_shared_mutable_type() {
    let mut interpreter = Interpreter::new();
    let Value::String(string) = interpreter.eval("s:\"abc\";s").unwrap() else {
        panic!("not a string")
    };
    let snapshot = string.snapshot();
    interpreter
        .eval("alias:s;read:{[]s};s[0]:\"X\";s[1 2]:\"YZ\";s,:\"!\"")
        .unwrap();
    assert_eq!(snapshot.as_slice(), b"abc");
    assert_eq!(string.snapshot().as_slice(), b"XYZ!");
    assert_eq!(interpreter.eval("alias").unwrap().to_string(), "\"XYZ!\"");
    assert_eq!(interpreter.eval("read[]").unwrap().to_string(), "\"XYZ!\"");
    interpreter.eval("s:\"new\"").unwrap();
    assert_eq!(string.snapshot().as_slice(), b"XYZ!");
    check("s:\"ab\";s,:s;s", "\"abab\"");
    check("s:\"\";s,:\"a\";s,:\"bc\";s", "\"abc\"");
}

#[test]
fn invalid_string_updates_preserve_all_aliases() {
    let mut interpreter = Interpreter::new();
    interpreter.eval("s:\"abc\";alias:s").unwrap();
    for source in [
        "s[0 9]:\"XY\"",
        "s[0 1]:(\"X\";1)",
        "s[0]:1",
        "s[0 1]:\"XYZ\"",
        "s,:1",
        "s[-1]:\"X\"",
        "s[0 0n]:\"XY\"",
        "@[s;0 0n;:;\"XY\"]",
    ] {
        assert!(interpreter.eval(source).is_err(), "{source}");
        assert_eq!(interpreter.eval("alias").unwrap().to_string(), "\"abc\"");
    }
    interpreter.eval("d:(,`s)!,s;d[`s][1]:\"X\"").unwrap();
    assert_eq!(interpreter.eval("alias").unwrap().to_string(), "\"aXc\"");
}

#[test]
fn string_reads_reject_float_indices_including_nan() {
    for source in ["\"abc\"[1.0]", "\"abc\"[nan]"] {
        assert!(Interpreter::new().eval(source).is_err(), "{source}");
    }
}

#[test]
fn string_primitives_keep_character_vectors_and_empty_strings() {
    for (source, expected) in [
        ("+(\"ab\";\"cd\")", "(\"ac\";\"bd\")"),
        ("0 2_\"abcd\"", "(\"ab\";\"cd\")"),
        ("(@\"abc\";@\"\";@\"a\")", "10 10 -10"),
        ("(\"abc\")[1]", "\"b\""),
        ("\"abc\"@2 0", "\"ca\""),
        ("\"abc\"@()", "\"\""),
        ("3#\"\"", "\"   \""),
        ("0#\"abc\"", "\"\""),
        ("1_\"abc\"", "\"bc\""),
        ("\"ab\",\"cd\"", "\"abcd\""),
        ("|\"abc\"", "\"cba\""),
        ("_\"ABC\"", "\"abc\""),
        ("?\"abac\"", "\"abc\""),
        ("\"abc\"?\"cx\"", "2 3"),
        ("\"abc\"=\"axc\"", "1 0 1"),
        ("^\"a b\"", "0 0 0"),
        ("{x=\"a\"}'\"abc\"", "1 0 0"),
        ("s:\"abc\";r:|s;r[0]:\"X\";s", "\"abc\""),
        ("s:\"abc\";r:@[s;0;:;\"X\"];s", "\"abc\""),
        (".[\"abc\";(),0;:;\"X\"]", "\"Xbc\""),
    ] {
        check(source, expected);
    }
}

#[test]
fn byte_storage_and_display_round_trip_without_utf8_loss() {
    let string = MutableString::new([b'a', 0, 0xff, b'"', b'\\']);
    let printed = Value::String(string.clone()).to_string();
    let Value::String(parsed) = Interpreter::new().eval(&printed).unwrap() else {
        panic!("not a string")
    };
    assert_eq!(parsed.snapshot(), string.snapshot());
    for source in ["\"\"", "\"a\\nb\"", "\"\\xff\\x00\"", ",\"a\""] {
        let mut interpreter = Interpreter::new();
        let original = interpreter.eval(source).unwrap();
        let printed = original.to_string();
        let restored = interpreter.eval(&printed).unwrap();
        let Value::String(a) = original else {
            panic!("{source}")
        };
        let Value::String(b) = restored else {
            panic!("{printed}")
        };
        assert_eq!(a.snapshot(), b.snapshot());
    }
}
