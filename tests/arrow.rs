use std::sync::Arc;

use arrow_array::{Array as _, ArrayRef, Float64Array, Int32Array, Int64Array};
use arrow_schema::DataType;
use pliq::{Array, Interpreter, Number, Value};

fn array(source: &str) -> Array {
    let Value::Array(array) = Interpreter::new().eval(source).unwrap() else {
        panic!("expected array: {source}");
    };
    array
}

#[test]
fn language_vectors_use_compact_typed_arrow_buffers() {
    for (source, expected) in [
        ("1 2 3", DataType::Int64),
        ("(1;2.0)", DataType::Float64),
        ("1 2 3=2", DataType::Boolean),
        ("`a`b", DataType::Utf8),
    ] {
        assert_eq!(array(source).to_arrow().unwrap().data_type(), &expected);
    }
    let ints = array("!1000").to_arrow().unwrap();
    assert_eq!(ints.to_data().buffers()[0].len(), 8000);
    assert_eq!(ints.null_count(), 0);
    let bools = array("(!1000)=2").to_arrow().unwrap();
    assert_eq!(bools.to_data().buffers()[0].len(), 125);
}

#[test]
fn construction_promotes_numbers_and_rejects_incompatible_types() {
    let promoted = Array::numbers(vec![Number::Bool(true), Number::Int(2), Number::Float(3.5)]);
    assert_eq!(promoted.to_arrow().unwrap().data_type(), &DataType::Float64);
    assert_eq!(promoted.get(0).unwrap().to_string(), "1");
    for source in [
        "(1;`a)",
        "1,`a",
        "(1;1 2)",
        "(1 2;1.0 2.0)",
        "(();1 2;`a`b)",
        "{ $[x;`a;1] }'0 1",
        "({x};1)",
    ] {
        assert!(
            Interpreter::new()
                .eval(source)
                .unwrap_err()
                .contains("homogeneous"),
            "{source}"
        );
    }
    assert!(Value::array([Value::Number(Number::Int(1)), Value::Symbol("a".into())]).is_err());
    assert_eq!(array("(1 2;,3)").len(), 2);
}

#[test]
fn arrow_snapshots_and_slices_share_buffers_and_survive_mutation() {
    let source = Arc::new(Int64Array::from(vec![Some(99), Some(1), None, Some(3)]));
    let input: ArrayRef = Arc::new(source.slice(1, 3));
    let imported = Array::from_arrow(input.clone()).unwrap();
    let exported = imported.to_arrow().unwrap();
    assert_eq!(
        input.to_data().buffers()[0].as_ptr(),
        exported.to_data().buffers()[0].as_ptr()
    );
    assert_eq!(imported.get(0).unwrap().to_string(), "1");
    assert_eq!(imported.get(1).unwrap().to_string(), "0n");
    let alias = imported.clone();
    imported.set(0, Value::Number(Number::Int(8))).unwrap();
    assert_eq!(alias.get(0).unwrap().to_string(), "8");
    assert_eq!(
        exported
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .value(0),
        1
    );
    assert!(exported.is_null(1));
}

#[test]
fn nulls_are_arrow_validity_bits_and_unsupported_imports_are_errors() {
    let ints = array("1 0n 3").to_arrow().unwrap();
    assert_eq!(ints.null_count(), 1);
    assert!(ints.is_null(1));
    let floats = array("1.0 0n 0w").to_arrow().unwrap();
    assert!(floats.is_null(1));
    assert!(!floats.is_null(2));
    assert!(Array::from_arrow(Arc::new(Int32Array::from(vec![1]))).is_err());
    let imported = Array::from_arrow(Arc::new(Float64Array::from(vec![
        Some(-0.0),
        None,
        Some(f64::INFINITY),
    ])))
    .unwrap();
    let output = imported.to_arrow().unwrap();
    let output = output.as_any().downcast_ref::<Float64Array>().unwrap();
    assert_eq!(output.value(0).to_bits(), (-0.0f64).to_bits());
    assert!(output.is_null(1));
    assert_eq!(output.value(2), f64::INFINITY);
}

#[test]
fn typed_empty_vectors_keep_types_through_slices_reverse_and_noop_append() {
    let mut interpreter = Interpreter::new();
    for (source, expected) in [
        ("@()", "7"),
        ("a:0#1.0 2.0;@a", "9"),
        ("@|a", "9"),
        ("a,:();@a", "9"),
        ("a[0]", "0n"),
        ("@(`a`b)[()]", "11"),
    ] {
        assert_eq!(
            interpreter.eval(source).unwrap().to_string(),
            expected,
            "{source}"
        );
    }
    assert!(interpreter.eval("a,:1").is_err());
    assert_eq!(interpreter.eval("#a").unwrap().to_string(), "0");
    interpreter.eval("a,:1.0").unwrap();
    assert_eq!(interpreter.eval("@a").unwrap().to_string(), "9");
}

#[test]
fn type_errors_preserve_aliases_and_roll_back_multiple_destinations() {
    let mut interpreter = Interpreter::new();
    interpreter.eval("a:1 2;b:a").unwrap();
    for source in ["a[0]:1.5", "a,:`a", "a[0 1]:1.0 2.0"] {
        assert!(interpreter.eval(source).is_err());
        assert_eq!(interpreter.eval("b").unwrap().to_string(), "1 2");
    }
    interpreter
        .eval("d:(,`a)!,1 2;e:(,`a)!,`x`y;t:(d;e)")
        .unwrap();
    assert!(interpreter.eval("t[0 1][`a][0]:7").is_err());
    assert_eq!(interpreter.eval("d[`a]").unwrap().to_string(), "1 2");
    assert_eq!(interpreter.eval("e[`a]").unwrap().to_string(), "`x`y");
    assert!(interpreter.eval("d[0]:`x").is_err());
    assert_eq!(interpreter.eval("!d").unwrap().to_string(), ",(`a)");
}

#[test]
fn arrow_vector_kernels_match_scalar_arithmetic_at_boundaries() {
    for literals in [
        vec!["0n", "-0W", "-1", "0", "1", "0W"],
        vec!["0n", "-0w", "-0.0", "0.0", "1.0", "0w"],
    ] {
        for op in ['+', '-', '*', '%'] {
            for left in &literals {
                let source = format!(
                    "({}){op}({})",
                    vec![*left; literals.len()].join(" "),
                    literals.join(" ")
                );
                let result = array(&source);
                for (i, right) in literals.iter().enumerate() {
                    let scalar = Interpreter::new()
                        .eval(&format!("({left}){op}({right})"))
                        .unwrap();
                    assert_eq!(
                        result.get(i).unwrap().to_string(),
                        scalar.to_string(),
                        "{source}, element {i}"
                    );
                }
                let arrow = result.to_arrow().unwrap();
                for (i, value) in result.iter().enumerate() {
                    assert_eq!(
                        arrow.logical_nulls().is_some_and(|n| n.is_null(i)),
                        matches!(value, Value::Null),
                        "{source}, element {i}"
                    );
                }
            }
        }
    }
}

#[test]
fn heterogeneous_call_arguments_and_internal_selector_paths_are_not_arrays() {
    let mut interpreter = Interpreter::new();
    assert_eq!(interpreter.eval("{[x;y]x}[1;`a]").unwrap().to_string(), "1");
    assert_eq!(
        interpreter
            .eval("a:1 2;d:(,`a)!,a;d[`a][0]+:2;a")
            .unwrap()
            .to_string(),
        "3 2"
    );
    assert_eq!(
        interpreter.eval("d[`a][0 1]+:1 2;a").unwrap().to_string(),
        "4 4"
    );
}
