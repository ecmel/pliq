use pliq::{Interpreter, Number, Value};

fn check(source: &str, expected: &str) {
    let value = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(value.to_string(), expected, "{source}");
}

#[test]
fn integer_and_float_literals_use_native_types() {
    let mut interpreter = Interpreter::new();
    assert!(matches!(
        interpreter.eval("1").unwrap(),
        Value::Number(Number::Int(1))
    ));
    assert!(matches!(
        interpreter.eval("1.0").unwrap(),
        Value::Number(Number::Float(1.0))
    ));
    assert!(matches!(
        interpreter.eval("1e2").unwrap(),
        Value::Number(Number::Float(100.0))
    ));
    check("(@1;@1.0;@1=1)", "-7 -9 -1");
    check("(@1 2;@1 2.0)", "7 9");
    check("1=1.0", "1");
    check("1~1.0", "0");
    check("9007199254740993+1", "9007199254740994");
    check("0.1+0.2", "0.30000000000000004");
    check("(0.1+0.2)=0.3", "1");
    check("1e-13", "0.0000000000001");
    check("1%3", "0.3333333333333333");
    check("pow[2;0.5]", "1.4142135623730951");
}

#[test]
fn native_nulls_infinities_and_wrapping_are_defined() {
    check("1%0", "0w");
    check("-1%0", "-0w");
    check("0%0", "0n");
    check("^0n 0n 1", "1 1 0");
    check("0n+1", "0n");
    check("0n+1", "0n");
    check("9223372036854775806+2", "-9223372036854775808");
    check("9223372036854775806*2", "-4");
    check("(@(1 2)[9];@(1.0 2.0)[9])", "0 0");
    check("10^0n", "10");
    check("round[1%3;5]", "0.33333");
    check("_1.9 -1.9", "1 -2");
    for source in [
        "9223372036854775808",
        "(1 2)[1.0]",
        "(1 2)[nan]",
        "!1.5",
        "round[1;309]",
    ] {
        assert!(Interpreter::new().eval(source).is_err(), "{source}");
    }
}

#[test]
fn numeric_snapshots_survive_shared_mutation_and_keep_integer_precision() {
    let mut interpreter = Interpreter::new();
    let Value::Array(a) = interpreter.eval("a:1 2 3").unwrap() else {
        panic!()
    };
    let snapshot = a.as_numbers().unwrap();
    interpreter.eval("b:a;a[0]:9007199254740993;a,:4").unwrap();
    assert_eq!(snapshot[0], Number::Int(1));
    assert_eq!(
        interpreter.eval("b[0]").unwrap().to_string(),
        "9007199254740993"
    );
    assert_eq!(a.as_numbers().unwrap()[3], Number::Int(4));
}

#[test]
fn numeric_folds_and_simple_lambdas_match_wide_integer_reference() {
    let boundaries = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    let literal = |n: i64| match n {
        i64::MAX => "0W".into(),
        n if n == i64::MIN + 1 => "-0W".into(),
        n => n.to_string(),
    };
    for op in ['+', '-', '*'] {
        for a in boundaries {
            for b in boundaries {
                let mut acc = a;
                let mut scan = vec![literal(a)];
                for x in [b, 1] {
                    acc = match op {
                        '+' => (i128::from(acc) + i128::from(x)) as i64,
                        '-' => (i128::from(acc) - i128::from(x)) as i64,
                        '*' => (i128::from(acc) * i128::from(x)) as i64,
                        _ => unreachable!(),
                    };
                    scan.push(literal(acc));
                }
                let input = format!("({};{};1)", literal(a), literal(b));
                check(&format!("{op}/{input}"), &literal(acc));
                check(&format!("{op}\\{input}"), &scan.join(" "));
                check(
                    &format!("{op}/[{};({};1)]", literal(a), literal(b)),
                    &literal(acc),
                );
                check(&format!("f:{{[x;y]x{op}y}};f/[{input}]"), &literal(acc));
            }
        }
    }
}

#[test]
fn numeric_folds_preserve_promotion_empty_inputs_and_float_order() {
    for op in ['+', '-', '*', '%'] {
        for input in [
            "()",
            ",2",
            "1 2 3",
            "(1=1;1=0;1=1)",
            "1 2.5 3",
            "0n 1.0 2",
            "(0w;-0w;1)",
            "(0.0;-0.0;0.0)",
            "(1e16;1;-1e16)",
        ] {
            for adverb in ['/', '\\'] {
                for seed in ["", "0.5;"] {
                    let fast = format!("{op}{adverb}[{seed}{input}]");
                    // Assignment forces the ordinary function frame and generic fold.
                    let reference = format!("{{[x;y]r:x{op}y;r}}{adverb}[{seed}{input}]");
                    let mut i = Interpreter::new();
                    let actual = i.eval(&fast).unwrap();
                    let expected = i.eval(&reference).unwrap();
                    fn same_numbers(actual: Value, expected: Value) {
                        match (actual, expected) {
                            (Value::Null, Value::Null) => {}
                            (Value::Number(Number::Float(a)), Value::Number(Number::Float(b))) => {
                                assert!(a.is_nan() && b.is_nan() || a.to_bits() == b.to_bits());
                            }
                            (Value::Number(a), Value::Number(b)) => {
                                assert_eq!(a.type_code(), b.type_code());
                                assert_eq!(a.as_i64(), b.as_i64());
                            }
                            (Value::Array(a), Value::Array(b)) => {
                                assert_eq!(a.len(), b.len());
                                for (a, b) in a.iter().zip(b.iter()) {
                                    same_numbers(a, b);
                                }
                            }
                            (a, b) => panic!("unexpected result types: {a:?}, {b:?}"),
                        }
                    }
                    same_numbers(actual, expected);
                }
            }
        }
    }
}

#[test]
fn vector_operations_follow_element_rules_for_types_and_nulls() {
    // (source, type code, display): each result matches applying the
    // primitive to every pair of elements, with scalars on either side.
    for (source, code, display) in [
        ("1 2 3*2", 7, "2 4 6"),
        ("2*1 2 3", 7, "2 4 6"),
        ("9223372036854775807 1+1", 7, "-9223372036854775808 2"),
        ("(1 2 3)%2", 9, "0.5 1 1.5"),
        ("1 0n 3+2", 7, "3 0n 5"),
        ("2-1 0n 3", 7, "1 0n -1"),
        ("(1 0n 3)<2", 1, "1 1 0"),
        ("(1 0n 3)=0n", 1, "0 1 0"),
        ("(1 0n 3)>0n", 1, "1 0 1"),
        ("0n<1 0n 3", 1, "1 0 1"),
        ("(1 0n 3)|2", 7, "2 2 3"),
        ("(1 0n 3)&2", 7, "1 0n 2"),
        ("0n|1 0n 3", 7, "1 0n 3"),
        ("0^1 0n 3", 7, "1 0 3"),
        ("(1 0n 3)^0n", 7, "1 0n 3"),
        ("mod[7 -7 7 0n;0 2 -2 3]", 7, "0n 1 1 0n"),
        ("mod[7 -7;0.0]", 9, "0n 0n"),
        ("(1.0 2.0)=1.0000000000001", 1, "1 0"),
        ("(1.0 2.0)<1.0000000000001", 1, "0 0"),
        ("(-0.0 0.0)&0.0 -0.0", 9, "-0 0"),
        ("(-0.0 0.0)|0.0 -0.0", 9, "0 -0"),
        ("(1 2 3)%0", 9, "0w 0w 0w"),
        ("(0w -0w 1.5)+(-0w 0w 0n)", 9, "0n 0n 0n"),
        ("pow[2 3;0.5]", 9, "1.4142135623730951 1.7320508075688772"),
        ("((!4)<2)+(1=1)", 7, "2 2 1 1"),
        ("((!4)<2)&(1=1)", 1, "1 1 0 0"),
        ("((!4)<2)|3", 7, "3 3 3 3"),
        ("(1 2 3)*2.5", 9, "2.5 5 7.5"),
        ("(0#0)+0.5", 9, "()"),
        ("(0#0)<1", 1, "()"),
        ("-(1 0n 3)", 7, "-1 0n -3"),
        ("-(!3)<2", 7, "-1 -1 0"),
        ("~1 0n 0", 1, "0 0 1"),
        ("~0.0 -0.0 1.5", 1, "1 1 0"),
        ("_1.5 -1.5 -0w 0w", 7, "1 -2 -0W 0W"),
        ("%2 0n 0", 9, "0.5 0n 0w"),
        ("-\"f\"$0n 0n", 0, "0n 0n"),
        ("-(0#0.5)", 7, "()"),
    ] {
        check(&format!("r:{source};@r"), &code.to_string());
        check(&format!("r:{source};r"), display);
    }
}
