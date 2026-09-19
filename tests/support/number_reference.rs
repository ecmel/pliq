//! Native integer results checked with wider independent intermediates.
use super::*;

#[test]
fn integer_arithmetic_matches_wide_reference_and_wraps_in_both_profiles() {
    let boundaries = [
        i64::MIN,
        i64::MIN + 1,
        -1_000_000_000,
        -1,
        0,
        1,
        1_000_000_000,
        i64::MAX - 1,
        i64::MAX,
    ];
    for a in boundaries {
        for b in boundaries {
            let x = Number::Int(a);
            let y = Number::Int(b);
            assert_eq!(
                x.add(&y).unwrap().as_i64(),
                Some((i128::from(a) + i128::from(b)) as i64)
            );
            assert_eq!(
                x.subtract(&y).unwrap().as_i64(),
                Some((i128::from(a) - i128::from(b)) as i64)
            );
            assert_eq!(
                x.multiply(&y).unwrap().as_i64(),
                Some((i128::from(a) * i128::from(b)) as i64)
            );
        }
    }
}

#[test]
fn native_float_ieee_edges_and_integer_promotion() {
    assert_eq!(
        Number::Int(3).divide(&Number::Int(2)).unwrap().as_f64(),
        1.5
    );
    assert_eq!(
        Number::Int(1).divide(&Number::Int(0)).unwrap().as_f64(),
        f64::INFINITY
    );
    assert!(
        Number::Int(0)
            .divide(&Number::Int(0))
            .unwrap()
            .as_f64()
            .is_nan()
    );
    assert_eq!(
        Number::Float(-0.0).negated().unwrap().as_f64().to_bits(),
        0f64.to_bits()
    );
    assert_eq!(
        Number::Float(0.1)
            .add(&Number::Float(0.2))
            .unwrap()
            .as_f64()
            .to_bits(),
        0x3fd3333333333334
    );
    assert_eq!(
        Number::Int(9_007_199_254_740_993)
            .add(&Number::Int(1))
            .unwrap(),
        Number::Int(9_007_199_254_740_994)
    );
    assert_eq!(
        Number::Float(f64::MAX)
            .multiply(&Number::Int(2))
            .unwrap()
            .as_f64(),
        f64::INFINITY
    );
    assert_eq!(
        Number::Int(i64::MIN).add(&Number::Int(1)).unwrap(),
        Number::Int(i64::MIN + 1)
    );
}

#[test]
fn floating_literals_preserve_native_precision_and_subnormals() {
    for text in ["1e-13", "5e-324", "1.2345678901234567", "1e300", "-0.0"] {
        let expected: f64 = text.parse().unwrap();
        assert_eq!(
            text.parse::<Number>().unwrap().as_f64().to_bits(),
            expected.to_bits()
        );
    }
    assert!("9223372036854775808".parse::<Number>().is_err());
    assert_eq!("1e9999".parse::<Number>().unwrap().as_f64(), f64::INFINITY);
    assert_eq!(Number::Float(2.5).round(0).unwrap(), Number::Float(2.0));
    assert_eq!(Number::Float(3.5).round(0).unwrap(), Number::Float(4.0));
}
