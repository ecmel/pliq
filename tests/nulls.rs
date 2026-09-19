use std::sync::Arc;

use arrow_array::{Array as _, BooleanArray, Float64Array, Int64Array, NullArray, StringArray};
use arrow_schema::DataType;
use pliq::{Array, Interpreter, Number, Value};

fn check(source: &str, expected: &str) {
    let actual = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(actual.to_string(), expected, "{source}");
}

#[test]
fn one_null_is_distinct_from_numbers_and_empty_values() {
    assert!(matches!(
        Interpreter::new().eval("0n").unwrap(),
        Value::Null
    ));
    for source in ["0N", "nan"] {
        assert!(Interpreter::new().eval(source).is_err(), "{source}");
    }
    for (source, expected) in [
        ("@0n", "0"),
        ("^0n", "1"),
        ("^(-9223372036854775808)", "0"),
        ("^\" \"", "0"),
        ("^`", "0"),
        ("0n~(-9223372036854775808)", "0"),
        ("0n~`", "0"),
        ("0n~\"\"", "0"),
        ("-9223372036854775808+1", "-0W"),
        ("0W+1", "-9223372036854775808"),
        ("0n+1.0", "0n"),
        ("round[0n;2]", "0n"),
        ("\"f\"$0n", "0n"),
        ("0n=0n", "1"),
        ("<3 0n 1", "1 2 0"),
    ] {
        check(source, expected);
    }
}

#[test]
fn undefined_arithmetic_produces_explicit_null_in_every_execution_path() {
    for source in [
        "0%0",
        "0w-0w",
        "0*0w",
        "pow[-1;0.5]",
        "mod[7;0]",
        "\"F\"$\"NaN\"",
        "+/(0w;-0w;1)",
        "{x-y}[0w;0w]",
    ] {
        assert!(
            matches!(Interpreter::new().eval(source).unwrap(), Value::Null),
            "{source}"
        );
    }
    check("(0.0;1.0)%(0.0;0.0)", "0n 0w");
    check("+\\(0w;-0w;1)", "0w 0n 0n");
    check("^((0.0;1.0)%(0.0;0.0))", "1 0");
    check("(1.0;2.0)%0n", "0n 0n");
    check("1%0", "0w");
}

#[test]
fn missing_reads_return_the_same_null_for_all_collection_types() {
    for source in [
        "(1 2)[9]",
        "(1.0 2.0)[0n]",
        "(`a`b)[-1]",
        "\"abc\"[9]",
        "*\"\"",
        "((1 2);(3 4))[9]",
        "(`a`b!1 2)[`missing]",
        "(`a`b!`x`y)[`missing]",
    ] {
        assert!(
            matches!(Interpreter::new().eval(source).unwrap(), Value::Null),
            "{source}"
        );
    }
    check("\"abc\"[0 0n 2]", "(\"a\";0n;\"c\")");
    check("d:(0n;1)!(10;20);d[0n]", "10");
    check("(0n;`)[0]", "0n");
}

#[test]
fn nullable_arrays_keep_their_type_through_updates_and_selection() {
    for (source, expected) in [
        ("@1 0n 3", "7"),
        ("@1.0 0n 3.0", "9"),
        ("@(1=1;0n)", "1"),
        ("@(0n;`a)", "11"),
        ("@0n 0n", "0"),
        ("@\"f\"$0n 0n", "9"),
        ("@\"b\"$0n 0n", "1"),
        ("a:,1.0;b:a;a[0]:0n;(@a;@|a;@a[,9])", "9 9 9"),
        ("a:,1.0;b:a;a[0]:0n;a[0]:2.0;b", ",(2)"),
        ("a:,`a;a[0]:0n;a,:`b;a", "(0n;`b)"),
        ("d:`a`b!1.0 2.0;@d[`x`y]", "9"),
        ("@(1.0;2.0)%0n", "9"),
        ("@(1.0;2.0)[0n 0n]", "9"),
    ] {
        check(source, expected);
    }
    let mut interpreter = Interpreter::new();
    interpreter.eval("a:,1.0;b:a;a[0]:0n").unwrap();
    assert!(interpreter.eval("a[0]:1").is_err());
    check("a:1 2;b:a;a[0]:0n;b", "0n 2");
    assert_eq!(interpreter.eval("b").unwrap().to_string(), ",(0n)");
}

#[test]
fn arrow_boundaries_preserve_validity_and_normalize_undefined_floats() {
    let inputs: Vec<arrow_array::ArrayRef> = vec![
        Arc::new(BooleanArray::from(vec![Some(false), None])),
        Arc::new(StringArray::from(vec![Some(""), None])),
        Arc::new(Int64Array::from(vec![Some(i64::MIN), None])),
        Arc::new(Float64Array::from(vec![Some(1.0), None])),
    ];
    for input in inputs {
        let array = Array::from_arrow(input.clone()).unwrap();
        assert!(matches!(array.get(1), Some(Value::Null)));
        assert!(!matches!(array.get(0), Some(Value::Null)));
        assert!(array.as_numbers().is_none());
        let output = array.to_arrow().unwrap();
        assert_eq!(input.data_type(), output.data_type());
        assert_eq!(output.null_count(), 1);
        array.set(0, Value::Null).unwrap();
        assert_eq!(array.to_arrow().unwrap().data_type(), input.data_type());
        assert_eq!(array.to_arrow().unwrap().null_count(), 2);
        assert_eq!(output.null_count(), 1);
    }
    let array = Array::from_arrow(Arc::new(Float64Array::from(vec![
        Some(f64::NAN),
        None,
        Some(f64::INFINITY),
    ])))
    .unwrap();
    assert_eq!(array.to_arrow().unwrap().null_count(), 2);
    assert!(matches!(array.get(0), Some(Value::Null)));
    assert!(matches!(
        Value::from_number(Number::Float(f64::NAN)),
        Value::Null
    ));
    let empty = Array::from_arrow(Arc::new(NullArray::new(0))).unwrap();
    assert_eq!(empty.to_arrow().unwrap().data_type(), &DataType::Null);
    let array = Array::from_arrow(Arc::new(NullArray::new(2))).unwrap();
    assert_eq!(array.to_arrow().unwrap().data_type(), &DataType::Null);
    assert!(array.iter().all(|v| matches!(v, Value::Null)));
}
