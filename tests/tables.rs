use pliq::{Array, Interpreter, Number, Table, Value};
use std::rc::Rc;

fn evaluates(source: &str, expected: &str) {
    let result = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(result.to_string(), expected, "{source}");
}

#[test]
fn construction_preserves_ragged_columns_and_evaluation_order() {
    evaluates("t:([] name:`alice`bob; age:30 25 40);#t", "3");
    evaluates("t:([] name:`alice`bob; age:30 25 40);#t[`name]", "2");
    evaluates("t:([] name:`alice`bob; age:30 25 40);t[`name;2]", "0n");
    evaluates("t:([] a:();b:0#0.0);@t[`b]", "9");
    evaluates("t:([] a:();b:0#0.0);#t", "0");
    evaluates("a:,0;t:([] left:(a[0]:2)*a;right:(a[0]:1)*a);a", ",(2)");
    evaluates("a:7;t:([] a:,1);a", "7");
    evaluates("!([] name:`alice`bob;age:30 25)", "`name`age");
    evaluates("@([] age:30 25)", "98");
    evaluates("([] a:1 2; b:,3)", "([]a:1 2;b:,(3))");
    for source in [
        "([])",
        "([] a:1)",
        "([] a:,1;a:,2)",
        "([] a:)",
        "([a:1] b:,2)",
    ] {
        assert!(Interpreter::new().eval(source).is_err(), "{source}");
    }
}

#[test]
fn columns_and_table_aliases_share_mutation_and_closure_access() {
    evaluates(
        "t:([] name:`alice`bob;age:30 25);u:t;a:t[`age];a[0]:31;u[`age;0]",
        "31",
    );
    evaluates("t:([] a:1 2;b:,3);a:t[`a];a,:4;(#t;#t[`b])", "3 1");
    evaluates("t:([] a:1 2);a:t[`a];t[`a;0]:8;a", "8 2");
    evaluates("t:([] a:1 2);f:{t[`a;0]};t[`a;0]:8;f[]", "8");
    evaluates("a:1 2;f:{([] c:a)};t:f[];a[0]:9;t[`c;0]", "9");
    evaluates("f:{([] a:x;b:y)};t:f[1 2;3 4];t[`b]", "3 4");
    evaluates("t:([] a:1 2);t[`a],:3;t[`a]", "1 2 3");
}

#[test]
fn selections_preserve_types_and_missing_rows_are_null() {
    evaluates(
        "t:([] name:`alice`bob;age:30 25 40);t[2 0]",
        "([]name:(0n;`alice);age:40 30)",
    );
    evaluates("t:([] a:1 2);t[1]", "([]a:,(2))");
    evaluates("t:([] a:1 2);t[-1 8 0n]", "([]a:0n 0n 0n)");
    evaluates("t:([] a:1 2;b:3.0 4.0);@t[()][`b]", "9");
    evaluates("t:([] a:1 2;b:3 4);s:t[`b`a];s[`a;0]:9;t[`a]", "9 2");
    evaluates("t:([] a:1 2;b:3 4);s:t[1 0];s[`a;0]:9;t[`a]", "1 2");
    evaluates("t:([] a:1 2;b:3 4);t[;1]", "([]a:,(2);b:,(4))");
    evaluates("t:([] a:1 2;b:3 4);t[`b`a;1]", "([]b:,(4);a:,(2))");
    evaluates("t:([] a:1 2 3);t[&t[`a]>1]", "([]a:2 3)");
    for source in ["([] a:,1)[1.0]", "([] a:,1)[`missing]", "([] a:,1)[`a`a]"] {
        assert!(Interpreter::new().eval(source).is_err(), "{source}");
    }
}

#[test]
fn append_aligns_logical_rows_and_updates_extracted_columns() {
    evaluates(
        "t:([] name:`alice`bob;age:30 25);a:t[`age];a,:40;t,:([] age:28 35;name:`dan`eve);t",
        "([]name:(`alice;`bob;0n;`dan;`eve);age:30 25 40 28 35)",
    );
    evaluates("t:([] a:1 2;b:,3);a:t[`a];t,:([] a:,4;b:5 6);a", "1 2 4 0n");
    evaluates("t:([] a:1 2;b:,3);t,:t;t", "([]a:1 2 1 2;b:3 0n 3 0n)");
    evaluates("t:([] a:1 2;b:,3);t,:([] a:();b:());#t[`b]", "1");
    evaluates("t:([] a:1 2;b:,3);s:t,([] a:,4;b:,5);(#t;#s)", "2 3");
    evaluates("t:([] a:1 2);s:t,([] a:,3);s[`a;0]:9;t[`a]", "1 2");
}

#[test]
fn failed_appends_and_updates_preserve_all_destinations() {
    let mut i = Interpreter::new();
    i.eval("t:([] a:1 2;b:,3);a:t[`a];u:t").unwrap();
    for bad in [
        "t,:([] a:,4;b:,5.0)",
        "t,:([] a:,4;c:,5)",
        "t,:([] a:,4)",
        "t,:4",
        "t[`b;1]:4",
        "t[`a]:1.0 2.0",
        "t[`a]:3",
        "t[`a;0 8]:9 10",
        "t[`a`b;0]:(9;1.0)",
    ] {
        assert!(i.eval(bad).is_err(), "{bad}");
        assert_eq!(
            i.eval("t").unwrap().to_string(),
            "([]a:1 2;b:,(3))",
            "{bad}"
        );
        assert_eq!(i.eval("a").unwrap().to_string(), "1 2", "{bad}");
    }
    i.eval("t[`c]:4 5 6").unwrap();
    assert_eq!(i.eval("#u").unwrap().to_string(), "3");
    i.eval("t[`a]:7 8 9").unwrap();
    assert_eq!(i.eval("a").unwrap().to_string(), "1 2");
}

#[test]
fn table_cycles_through_columns_and_closures_are_rejected() {
    let mut i = Interpreter::new();
    i.eval("t:([] a:1 2)").unwrap();
    assert!(i.eval("t[`loop]:,t").is_err());
    assert!(i.eval("f:{t};t[`loop]:,f").is_err());
    assert_eq!(i.eval("!t").unwrap().to_string(), ",(`a)");
    i.eval("a:,{42};t[`f]:a;g:{t}").unwrap();
    assert!(i.eval("a,:g").is_err());
    assert_eq!(i.eval("#a").unwrap().to_string(), "1");
}

#[test]
fn table_equality_and_dictionary_keys_snapshot_columns() {
    evaluates("([] a:1 2)~([] a:1 2)", "1");
    evaluates("([] a:1 2;b:,3)~([] a:1 2;b:3 0n)", "0");
    evaluates("t:([] a:1 2);d:(,t)!,9;t[`a;0]:8;d[([] a:1 2)]", "9");
    evaluates("t:([] a:1 2);t[`a;0]+:4;t[`a]", "5 2");
}

#[test]
fn arrow_export_pads_snapshots_without_changing_shared_arrays() {
    use arrow_array::{Array as _, Int64Array};
    let a = Array::numbers(vec![Number::Int(1), Number::Int(2)]);
    let b = Array::numbers(vec![Number::Int(3)]);
    let t = Table::new(vec![(Rc::from("a"), a.clone()), (Rc::from("b"), b.clone())]).unwrap();
    let batch = t.to_arrow().unwrap();
    assert_eq!(batch.num_rows(), 2);
    assert_eq!(b.len(), 1);
    assert!(batch.column(1).is_null(1));
    a.set(0, Value::Number(Number::Int(9))).unwrap();
    assert_eq!(
        batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .value(0),
        1
    );
    let imported = Table::from_arrow(&batch).unwrap();
    assert_eq!(imported.len(), 2);
    assert_eq!(imported.column("b").unwrap().len(), 2);
    imported
        .column("a")
        .unwrap()
        .append(Value::Number(Number::Int(4)))
        .unwrap();
    assert_eq!(batch.num_rows(), 2);
}

#[test]
fn append_checks_shared_destinations_and_cross_column_cycles_atomically() {
    evaluates("a:1 2;t:([] x:a;y:a);t,:([] x:,3;y:,3);a", "1 2 3");
    let mut i = Interpreter::new();
    i.eval("a:1 2;t:([] x:a;y:a)").unwrap();
    assert!(i.eval("t,:([] x:,3;y:,4)").is_err());
    assert_eq!(i.eval("a").unwrap().to_string(), "1 2");
    i.eval("a:,{0};b:,{0};t:([] a:a;b:b);fa:{a};fb:{b}")
        .unwrap();
    assert!(i.eval("t,:([] a:,fb;b:,fa)").unwrap_err().contains("cycle"));
    assert_eq!(i.eval("(#a;#b)").unwrap().to_string(), "1 1");
}

#[test]
fn multi_selector_updates_and_amends_keep_existing_array_behavior() {
    evaluates("a:(1 2;3 4);a[1;0]:8;a", "(1 2;8 4)");
    evaluates("t:([] a:1 2;b:3 4);t[;0]:9;t", "([]a:9 2;b:9 4)");
    evaluates("t:([] a:1 2;b:3 4);t[`a][1]:9;t[`a]", "1 9");
    evaluates("t:([] a:1 2);s:.[t;,`a;+;4];t[`a]", "1 2");
    evaluates("t:([] a:1 2);s:.[t;,`a;+;4];s[`a]", "5 6");
}

#[test]
fn column_labels_and_display_round_trip() {
    for source in [
        "([] a:1 2;b:3 0n)",
        "([] `price.usd:1.0 2.0)",
        "([] \"first name\":`alice`bob;\"\":,1)",
    ] {
        let mut i = Interpreter::new();
        let value = i.eval(source).unwrap();
        let displayed = value.to_string();
        let reparsed = i.eval(&displayed).unwrap();
        assert_eq!(reparsed.to_string(), displayed);
    }
}

#[test]
fn failed_multi_column_replacement_rolls_back_new_columns() {
    let mut i = Interpreter::new();
    i.eval("t:([] a:`x`y);u:t").unwrap();
    assert!(i.eval("t[`new`a]:(1 2;3 4)").is_err());
    assert_eq!(i.eval("!u").unwrap().to_string(), ",(`a)");
    assert_eq!(i.eval("u[`a]").unwrap().to_string(), "`x`y");
}

#[test]
fn arrow_tables_preserve_supported_types_and_reject_unsupported_columns() {
    use arrow_array::{
        Array as _, ArrayRef, BooleanArray, Float64Array, Int64Array, NullArray, RecordBatch,
        StringArray, UInt64Array,
    };
    use std::sync::Arc;
    let batch = RecordBatch::try_from_iter([
        (
            "bool",
            Arc::new(BooleanArray::from(vec![Some(true), None])) as ArrayRef,
        ),
        (
            "int",
            Arc::new(Int64Array::from(vec![Some(i64::MIN), None])) as ArrayRef,
        ),
        (
            "float",
            Arc::new(Float64Array::from(vec![
                Some(f64::NAN),
                Some(f64::INFINITY),
            ])) as ArrayRef,
        ),
        (
            "symbol",
            Arc::new(StringArray::from(vec![Some(""), None])) as ArrayRef,
        ),
        ("null", Arc::new(NullArray::new(2)) as ArrayRef),
    ])
    .unwrap();
    let table = Table::from_arrow(&batch).unwrap();
    let exported = table.to_arrow().unwrap();
    for (original, result) in batch.columns().iter().zip(exported.columns()) {
        assert_eq!(original.data_type(), result.data_type());
    }
    assert!(exported.column(2).is_null(0));
    assert!(!exported.column(1).is_null(0));
    assert!(!exported.column(3).is_null(0));
    let empty = Table::from_arrow(&batch.slice(0, 0))
        .unwrap()
        .to_arrow()
        .unwrap();
    assert_eq!(empty.num_rows(), 0);
    assert_eq!(empty.schema(), exported.schema());
    let unsupported = RecordBatch::try_from_iter([(
        "unsigned",
        Arc::new(UInt64Array::from(vec![1])) as ArrayRef,
    )])
    .unwrap();
    assert!(Table::from_arrow(&unsupported).is_err());
    let mut i = Interpreter::new();
    let Value::Table(runtime) = i.eval("([] nested:,(1 2))").unwrap() else {
        panic!("table")
    };
    assert!(runtime.to_arrow().is_err());
}

#[test]
fn screen_display_escapes_line_breaks_and_keeps_columns_unmodified() {
    let mut i = Interpreter::new();
    let Value::Table(t) = i.eval("([] \"a\\nb\":,`$\"x\\ty\";n:1 20)").unwrap() else {
        panic!("expected table");
    };
    assert_eq!(t.to_string(), "a\\nb n\n---- --\nx\\ty  1\n0n   20");
    assert_eq!(t.column("a\nb").unwrap().len(), 1);
    assert_eq!(t.len(), 2);
}

#[test]
fn dot_column_access_supports_reads_updates_and_shared_columns() {
    evaluates("t:([] price:10 20);t.price", "10 20");
    evaluates("t:([] price:10 20);t.price[1]", "20");
    evaluates("t:([] price:10 20);a:t.price;t.price[0]:30;a", "30 20");
    evaluates("t:([] price:10 20);t.price+:1;t.price", "11 21");
    evaluates("t:([] price:10 20);t.price,:30;t.price", "10 20 30");
    assert!(Interpreter::new().eval("([] price:10 20).missing").is_err());
}
