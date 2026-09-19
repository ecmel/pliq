use super::*;

fn int(n: i64) -> Value {
    Value::Number(Number::Int(n))
}

fn float(n: f64) -> Value {
    Value::Number(Number::Float(n))
}

fn dictionary(keys: &[Value]) -> Dictionary {
    Dictionary::from_arrays(
        Array::new(keys.iter().cloned()).unwrap(),
        Array::new((0..keys.len()).map(|i| int(i as i64))).unwrap(),
    )
    .unwrap()
}

fn assert_matches_scan(dictionary: &Dictionary, queries: &[Value]) {
    let keys = dictionary.keys();
    let values = dictionary.values();
    for query in queries {
        let expected = keys.iter().position(|key| key.same(query));
        let actual = dictionary.get_value(query);
        match expected {
            Some(i) => assert!(actual.unwrap().same(&values.get(i).unwrap()), "{query:?}"),
            None => assert!(actual.is_none(), "{query:?}"),
        }
    }
}

#[test]
fn dictionary_index_matches_scan_for_float_boundaries_and_nested_keys() {
    let mut numbers = vec![
        0.0,
        -0.0,
        f64::NAN,
        f64::from_bits(0xfff8_0000_0000_0001),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
    ];
    // Sample every normal exponent and both sides of binade boundaries.
    for exponent in 1..2047u64 {
        let n = f64::from_bits(exponent << 52);
        numbers.extend([n, -n, n.next_down(), n.next_up()]);
    }
    let keys: Vec<_> = numbers.iter().copied().map(float).collect();
    for chunk in keys.chunks(64) {
        let mut queries = chunk.to_vec();
        for key in chunk {
            if let Value::Number(Number::Float(n)) = key {
                queries.extend([
                    float(n * (1.0 + 2f64.powi(-44))),
                    float(n * (1.0 - 2f64.powi(-44))),
                    float(n * (1.0 + 2f64.powi(-41))),
                ]);
            }
        }
        assert_matches_scan(&dictionary(chunk), &queries);
    }
    let queries = keys.clone();

    let keys: Vec<_> = keys
        .iter()
        .take(40)
        .map(|key| Value::array([float(2.0), key.clone(), float(3.0)]).unwrap())
        .collect();
    let queries: Vec<_> = queries
        .iter()
        .take(200)
        .map(|key| Value::array([float(2.0), key.clone(), float(3.0 + 1e-14)]).unwrap())
        .collect();
    assert_matches_scan(&dictionary(&keys), &queries);
}

#[test]
fn dictionary_index_preserves_first_match_with_nontransitive_float_tolerance() {
    let epsilon = 2f64.powi(-43);
    let keys = [float(1.0 + 1.5 * epsilon), float(1.0), float(1.0)];
    let dictionary = dictionary(&keys);
    // The two stored values do not match, but the query matches both.
    // Key order, rather than float sort order, must decide the result.
    let query = float(1.0 + 0.75 * epsilon);
    assert!(dictionary.get_value(&query).unwrap().same(&int(0)));
    dictionary.insert_value(query, int(99)).unwrap();
    assert_eq!(dictionary.len(), 3);
    assert!(dictionary.values().get(0).unwrap().same(&int(99)));
    assert!(dictionary.values().get(1).unwrap().same(&int(1)));
    assert_matches_scan(&dictionary, &keys);
}

#[test]
fn dictionary_index_handles_detached_keys_and_mutable_function_captures() {
    let mut interpreter = crate::Interpreter::new();
    let function = interpreter.eval("a:,1;f:{a};f").unwrap();
    let other_function = interpreter.eval("b:,1;g:{b};g").unwrap();
    let functions = dictionary(&[function, other_function.clone()]);
    interpreter.eval("a[0]:2").unwrap();
    assert!(functions.get_value(&other_function).unwrap().same(&int(1)));
    let updated_function = interpreter.eval("{a}").unwrap();
    assert!(
        functions
            .get_value(&updated_function)
            .unwrap()
            .same(&int(0))
    );

    let string = Value::text("key");
    let strings = dictionary(std::slice::from_ref(&string));
    if let Value::String(s) = string {
        s.set(0, b'X').unwrap();
    }
    assert!(
        strings
            .get_value(&Value::text("key"))
            .unwrap()
            .same(&int(0))
    );

    let array = Value::array([int(1), int(2)]).unwrap();
    let arrays = dictionary(std::slice::from_ref(&array));
    if let Value::Array(a) = array {
        a.set(0, int(99)).unwrap();
    }
    assert!(
        arrays
            .get_value(&Value::array([int(1), int(2)]).unwrap())
            .unwrap()
            .same(&int(0))
    );
    assert_matches_scan(&arrays, &arrays.keys().iter().collect::<Vec<_>>());
}

#[test]
fn dictionary_index_and_key_depth_restore_after_failed_multi_update() {
    let mut interpreter = crate::Interpreter::new();
    interpreter.eval("d:()!();e:()!();t:(d;e)").unwrap();
    // The first dictionary accepts an integer; the second rejects a cycle.
    let Value::Array(t) = interpreter.eval("t").unwrap() else {
        panic!()
    };
    let Value::Dictionary(d) = t.get(0).unwrap() else {
        panic!()
    };
    let backup = Backup::new(&Value::Dictionary(d.clone()));
    d.insert_value(int(1), int(7)).unwrap();
    backup.restore();
    assert_eq!(d.len(), 0);
    assert!(d.get_value(&int(1)).is_none());

    let dictionary =
        Dictionary::from_arrays(Array::numbers(vec![]), Array::numbers(vec![])).unwrap();
    let backup = Backup::new(&Value::Dictionary(dictionary.clone()));
    dictionary
        .insert_value(Value::array([int(2), int(3)]).unwrap(), int(9))
        .unwrap();
    assert!(dictionary.key_is_atom(&Value::array([int(7), int(8)]).unwrap()));
    backup.restore();
    assert!(!dictionary.key_is_atom(&Value::array([int(7), int(8)]).unwrap()));
    assert_matches_scan(&dictionary, &[int(1), int(2)]);
}
