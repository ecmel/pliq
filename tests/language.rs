use pliq::Interpreter;

fn evaluates(source: &str, expected: &str) {
    let result = Interpreter::new()
        .eval(source)
        .unwrap_or_else(|e| panic!("{source}: {e}"));
    assert_eq!(result.to_string(), expected, "source: {source}");
}

#[test]
fn right_to_left_and_parentheses() {
    evaluates("2*3+4", "14");
    evaluates("(2*3)+4", "10");
    evaluates("10-3-2", "9");
    evaluates("-2+3", "1");
    evaluates("(-2)+3", "1");
    evaluates(".5+1e-2", "0.51");
}

#[test]
fn arrays_broadcast_recursively() {
    evaluates("1 2 3+10", "11 12 13");
    evaluates("2*1 2 3", "2 4 6");
    evaluates("1 2 3+4 5 6", "5 7 9");
    evaluates("(1 2;3 4)+10", "(11 12;13 14)");
    evaluates("(1 2;3 4)+(10 20;30 40)", "(11 22;33 44)");
    evaluates("1+()", "()");
    evaluates(",(1 2)", ",(1 2)");
}

#[test]
fn reductions_and_scans() {
    evaluates("+/!10", "45");
    evaluates("*/1+!5", "120");
    evaluates("-/1 2 3", "-4");
    evaluates("+\\1 2 3 4", "1 3 6 10");
    evaluates("+/[10;1 2 3]", "16");
    evaluates("+\\[10;1 2 3]", "11 13 16");
    evaluates("+/()", "()");
    evaluates("*/()", "()");
    evaluates("-/()", "()");
    evaluates(",/()", "()");
    evaluates("+\\()", "()");
    evaluates("-/[42;()]", "42");
    evaluates("+/7", "7");
    evaluates("+/ (1 2;3 4)", "4 6");
}

#[test]
fn functions_each_and_higher_order_calls() {
    evaluates("sq:{x*x};sq[5]", "25");
    evaluates("sq:{x*x};sq'[1 2 3]", "1 4 9");
    evaluates("{x+y}'[1 2;10 20]", "11 22");
    evaluates("{x+y}'[1 2;10]", "11 12");
    evaluates("{x+y}'[10;1 2]", "11 12");
    evaluates("{x*y}'[3;4]", "12");
    evaluates("{x*x}'[()]", "()");
    evaluates("{[a;b]a*a+b*b}[3;4]", "57"); // a*(a+(b*b)), right to left
    evaluates("{[a;b](a*a)+b*b}[3;4]", "25");
    evaluates("f:{x+y};f/[1 2 3]", "6");
    evaluates("apply:{[f;a]f[a]};apply[{x*x};6]", "36");
    evaluates("sum:+/;sum[1 2 3]", "6");
    evaluates("{42}[]", "42");
    evaluates("{x+y+z}[1;2;3]", "6");
}

#[test]
fn whitespace_calls_accept_functions_vectors_and_closures() {
    evaluates("square:{x*x};square 5", "25");
    evaluates("{x*x} 5", "25");
    evaluates("square:{x*x};(square) 5", "25");
    evaluates("square:{x*x};square(5)", "25");
    evaluates("square:{x*x};square 1 2 3", "1 4 9");
    evaluates("square:{x*x};square' 1 2 3", "1 4 9");
    evaluates("sum:+/;sum 1 2 3", "6");
    evaluates("(+/) 1 2 3", "6");
    evaluates("{x+y}/ 1 2 3", "6");
    evaluates("apply:{[f;a]f a};apply[{x*x};6]", "36");
    evaluates("make:{[a]{[b]a+b}};add:make 10;add 5", "15");
    evaluates("make:{[a]{[b]a+b}};(make 10) 5", "15");
    evaluates("make:{[a]{[b]a+b}};make[10] 5", "15");
    evaluates("fs:({x+1};{x*x});fs[1] 4", "16");
    evaluates("{$[1;{x+1};{x*x}]}[] 3", "4");
    evaluates("$[1;{x+1};{x*x}] 3", "4");
    evaluates("size:{#x};size ()", "0");
    evaluates("size:{#x};size (1 2;3 4)", "2");
    evaluates("a:10 20 30;a 1", "20");
    evaluates("a:10 20 30;a 2 0", "30 10");
}

#[test]
fn whitespace_calls_follow_right_to_left_evaluation() {
    evaluates("square:{x*x};square 2+3", "25");
    evaluates("square:{x*x};(square 2)+3", "7");
    evaluates("square:{x*x};square[2]+3", "7");
    evaluates("square:{x*x};square square 2", "16");
    evaluates("inc:{x+1};square:{x*x};square inc 2", "9");
    evaluates("inc:{x+1};square:{x*x};inc square 2", "5");
    evaluates("square:{x*x};1+square 2+3", "26");
    evaluates("square:{x*x};square (-2)", "4");
    evaluates("sum:+/;sum (!5)", "10");
    evaluates("square:{x*x};square (+/1 2 3)", "36");
    evaluates("square:{x*x};square round[1.255;2]", "1.5625");
    evaluates("fact:{$[x<2;1;x*fact (x-1)]};fact 10", "3628800");
    evaluates("a:10;a-2", "8"); // infix operators retain their meaning
    evaluates("1 2 3", "1 2 3"); // numeric strands stay vectors
    evaluates("f:{x+1};f 2;7", "7");
    evaluates("f:{x+1}\nf 2\n7", "7");
    evaluates("f:{x+1};f // comment\n2", "2"); // newline ends the statement
    evaluates("f:{x+1};(f\n2)", "3"); // newlines inside () are whitespace
    evaluates("a:1;read:{a+x};read (a:2);a", "2");
    evaluates("f:{x+1};f (3*#(f:{x*x}))", "9"); // argument evaluated before function lookup
}

#[test]
fn whitespace_calls_keep_explicit_arity_and_errors() {
    evaluates("add:{x+y};add[2;3]", "5");
    evaluates("answer:{[]42};answer[]", "42");
    for (source, expected) in [
        ("answer:{[]42};answer 1", "expects 0 arguments, got 1"),
        ("f:{x};f missing", "undefined name: missing"),
        ("f:{x};f (", "expected expression"),
    ] {
        let error = Interpreter::new().eval(source).expect_err(source);
        assert!(error.contains(expected), "{source}: {error}");
    }
    let source = format!("f:{{x}};{}1", "f ".repeat(130));
    assert!(
        Interpreter::new()
            .eval(&source)
            .unwrap_err()
            .contains("nesting limit")
    );
}

#[test]
fn lexical_scope_and_persistent_environment() {
    let mut interpreter = Interpreter::new();
    interpreter.eval("a:10;f:{a+x};a:99").unwrap();
    assert_eq!(interpreter.eval("f[2]").unwrap().to_string(), "12");
    assert_eq!(interpreter.eval("a").unwrap().to_string(), "99");
    evaluates("a:1;f:{a:2;x+a};f[3];a", "1");
    evaluates("make:{[a]{[b]a+b}};add:make[10];add[5]", "15");
}

#[test]
fn closure_capture_preserves_assignment_order_and_conditional_scope() {
    evaluates("a:10;f:{[]a:a+1;a};a:99;f[]", "11");
    evaluates("a:10;f:{[](a:1;a)};a:99;f[]", "1 10");
    evaluates("a:10;f:{[]+[a:1;a]};a:99;f[]", "11");
    evaluates("a:10;f:{[x]$[x;a:1;0];a};a:99;(f[0];f[1])", "10 1");
    evaluates("a:10;f:{[x]$[x;0;a:1];a};a:99;(f[0];f[1])", "1 10");
    evaluates("a:10;f:{[x]$[x;a;0]};a:99;f[1]", "10");
    evaluates("a:10;f:{[x]$[x;0;a]};a:99;f[0]", "10");
    evaluates("a:1 2 3;f:{[]+/a};a:99;f[]", "6");
    evaluates("f:{[]$[0;missing;7]};f[]", "7");
    let mut interpreter = Interpreter::new();
    interpreter.eval("f:{[]missing};missing:7").unwrap();
    assert_eq!(
        interpreter.eval("f[]").unwrap_err(),
        "undefined name: missing"
    );
}

#[test]
fn nested_closures_keep_required_outer_values_and_local_snapshots() {
    evaluates("a:10;make:{[]{[]{[]a}}};a:99;make[][][]", "10");
    evaluates("a:10;make:{[]g:{[]a};a:20;g};a:99;make[][]", "10");
    evaluates("a:10;make:{[]a:20;{[]a}};a:99;make[][]", "20");
    evaluates("a:10;make:{[]g:{[]a:20;a};a};a:99;make[]", "10");
    evaluates("a:10;make:{[]g:{[a]a};a};a:99;make[]", "10");
    evaluates(
        "a:10;make:{[x]$[x;a:20;0];{[]a}};a:99;(make[0][];make[1][])",
        "10 20",
    );
    evaluates("f:{$[x<1;{[]f};f[x-1]]};g:f[1];g[][0][]~f", "1");
}

#[test]
fn closures_release_unneeded_outer_values() {
    use std::rc::Rc;

    for (definition, call, expected) in [
        ("f:{x+1}", "f[2]", "3"),
        ("f:{[a]a}", "f[2]", "2"),
        ("f:{[]a:1;a}", "f[]", "1"),
        ("f:{[]{[a]a}}", "f[][2]", "2"),
        ("f:{[]a:1;{[]a}}", "f[][]", "1"),
        ("f:{[x]$[x;a:1;a:2];a}", "f[0]", "2"),
        ("f:{[](a;a:1)}", "f[]", "1 1"),
        ("f:{[]+[a;a:1]}", "f[]", "2"),
        ("f:{[]a[(a:10 20)[0]-10]}", "f[]", "10"),
        ("f:{[]$[a:0;1;2];a}", "f[]", "0"),
    ] {
        let mut interpreter = Interpreter::new();
        interpreter.eval("a:`marker").unwrap();
        let pliq::Value::Symbol(marker) = interpreter.eval("a").unwrap() else {
            panic!("expected symbol");
        };
        let weak = Rc::downgrade(&marker);
        drop(marker);
        interpreter.eval(definition).unwrap();
        interpreter.eval("a:0").unwrap();
        assert!(weak.upgrade().is_none(), "{definition} retained outer a");
        assert_eq!(interpreter.eval(call).unwrap().to_string(), expected);
    }
}

#[test]
fn closures_release_captured_values_when_the_last_closure_is_dropped() {
    use std::rc::Rc;

    let mut interpreter = Interpreter::new();
    let pliq::Value::Symbol(marker) = interpreter.eval("a:`marker").unwrap() else {
        panic!("expected symbol");
    };
    let weak = Rc::downgrade(&marker);
    drop(marker);
    interpreter.eval("f:{[]{[]a}};a:0;g:f[];f:0").unwrap();
    assert!(weak.upgrade().is_some());
    assert_eq!(interpreter.eval("g[]").unwrap().to_string(), "`marker");
    interpreter.eval("g:0").unwrap();
    assert!(weak.upgrade().is_none());
}

#[test]
fn recursive_redefinition_releases_the_previous_function() {
    use std::rc::Rc;

    let mut interpreter = Interpreter::new();
    let pliq::Value::Function(previous) = interpreter.eval("f:{x}").unwrap() else {
        panic!("expected function");
    };
    let weak = Rc::downgrade(&previous);
    drop(previous);
    interpreter.eval("f:{$[x<2;1;x*f[x-1]]}").unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(interpreter.eval("f[5]").unwrap().to_string(), "120");
    interpreter.eval("alias:f;f:0").unwrap();
    assert_eq!(interpreter.eval("alias[5]").unwrap().to_string(), "120");
}

#[test]
fn lazy_conditionals_and_recursion() {
    evaluates("$[1;42;1+`bad]", "42");
    evaluates("$[0;missing;7]", "7");
    evaluates("$[1;2;3]+4", "6");
    evaluates("+/[1 2 3]*2", "12");
    evaluates("(-1)+2", "1");
    evaluates("fact:{$[x<2;1;x*fact[x-1]]};fact[10]", "3628800");
    evaluates("fib:{$[x<2;x;fib[x-1]+fib[x-2]]};fib[10]", "55");
}

#[test]
fn structural_primitives() {
    evaluates("!5", "0 1 2 3 4");
    evaluates("#1 2 3", "3");
    evaluates("#7", "1");
    evaluates("*10 20 30", "10");
    evaluates("|1 2 3", "3 2 1");
    evaluates("5#1 2 3", "1 2 3 1 2");
    evaluates("(-5)#1 2 3", "2 3 1 2 3");
    evaluates("0#()", "()");
    evaluates("2_1 2 3 4", "3 4");
    evaluates("(-2)_1 2 3 4", "1 2");
    evaluates("1 2,3 4", "1 2 3 4");
    evaluates("&0 2 1", "1 1 2");
    evaluates("?3 1 3 2 1", "3 1 2");
    evaluates("asc 3 1 2", "1 2 3");
    evaluates("<30 10 20", "1 2 0");
    evaluates(">30 10 20", "0 2 1");
    evaluates("10 20 30?20 99", "1 3");
}

#[test]
fn indexing_and_boolean_arithmetic() {
    evaluates("a:10 20 30;a[1]", "20");
    evaluates("10 20 30@2 0", "30 10");
    evaluates("(10 20 30)[-1]", "0n");
    evaluates("(1 2;3 4)[1][0]", "3");
    evaluates("1 2 3=2", "0 1 0");
    evaluates("1 2 3>1", "0 1 1");
    evaluates("~0 1 2", "1 0 0");
    evaluates("(1 2;3 4)~(1 2;3 4)", "1");
    evaluates("1 2~1 2 3", "0");
    evaluates("mod[0 1 2 3 4;2]", "0 1 0 1 0");
    evaluates("a:!10;a@&0=mod[a;2]", "0 2 4 6 8");
    evaluates("pow[2;3]", "8");
    evaluates("1 4&2 3", "1 3");
    evaluates("1 4|2 3", "2 4");
    evaluates("_1.9 2.1", "1 2");
    evaluates("%2 4", "0.5 0.25");
}

#[test]
fn comments_and_multiline_programs() {
    evaluates("// hello\na:1 2 3\n+/a // sum\n", "6");
    evaluates("f:{\na:x+1\na*a\n}\nf[3]", "16");
    evaluates("(\n1 2\n)+3", "4 5");
    evaluates("{x+y}[\n1;\n2\n]", "3");
    evaluates("", "()");
}

#[test]
fn only_double_slashes_start_comments_outside_strings() {
    evaluates("// { ignored\n1// no separating space", "1");
    evaluates("// comment only", "()");
    evaluates("1// ignored\r\n2", "2");
    evaluates("\"//\"", "\"//\"");
    evaluates("\"a\\\"//b\"// ignored", "\"a\\\"//b\"");
    evaluates("+ /1 2 3", "6");
    evaluates("+ /1 2 3// total", "6");
    evaluates("10 20+ /:1 2", "(11 21;12 22)");
    for source in ["/ comment", "1 / comment", "1 / { ignored"] {
        assert!(Interpreter::new().eval(source).is_err(), "{source}");
    }
}

#[test]
fn errors_are_reported_without_panics() {
    for (source, message) in [
        ("1 2+3 4 5", "length mismatch"),
        ("1+`bad", "requires numbers"),
        ("9223372036854775808", "overflow"),
        ("unknown", "undefined name"),
        ("(1 2)[0.5]", "index out of bounds"),
        ("!2.5", "integer"),
        ("!(-2)", "nonnegative"),
        ("!1e20", "integer"),
        ("&,(-1)", "nonnegative"),
        ("1'[2]", "function"),
        ("{x+y}'[1 2;1 2 3]", "length mismatch"),
        ("{[a;a]a}", "duplicate parameter"),
        ("{}", "body cannot be empty"),
        ("$[4;2]", "$ requires"),
        ("$[1 2;3;4]", "expected a number"),
        ("(1+2", "expected"),
        ("1..2", "invalid floating-point number"),
        ("\u{2373}5", "ASCII"),
        ("f:{f[x]};f[1]", "depth limit"),
    ] {
        let error = Interpreter::new().eval(source).expect_err(source);
        assert!(
            error.contains(message),
            "{source}: expected {message:?}, got {error:?}"
        );
    }
}

#[test]
fn arrays_and_evaluation_are_not_subject_to_arbitrary_quotas() {
    // Each used to hit the one-million-item or two-million-step quota.
    evaluates("#!1000001", "1000001");
    evaluates("+/!1000001", "500000500000");
    evaluates("#1000001#7", "1000001");
    evaluates("#(500001#1),500001#2", "1000002");
    evaluates("#&500001 500001", "1000002");
    evaluates("1000001_1 2 3", "()");
    evaluates("(-1000001)_1 2 3", "()");
}

#[test]
fn matrix_multiplication() {
    let mm = "mm:{[a;b]{[row]+/row*b}' a};";
    for (source, expected) in [
        ("mm[(1 2;3 4);(5 6;7 8)]", "(19 22;43 50)"),
        ("mm[(1 2 3;4 5 6);(7 8;9 10;11 12)]", "(58 64;139 154)"),
        ("mm[(1 2;3 4);(1 0;0 1)]", "(1 2;3 4)"),
        (
            "mm[(0.1 0.2;0.3 0.4);(0.5 0.6;0.7 0.8)]",
            "(0.19 0.22000000000000003;0.42999999999999994 0.5)",
        ),
        ("mm[,(1 2);(,3;,4)]", ",(,(11))"),
        ("mm[();(1 2;3 4)]", "()"),
    ] {
        evaluates(&format!("{mm}{source}"), expected);
    }
    let error = Interpreter::new()
        .eval(&format!("{mm}mm[(1 2;3 4);(1 2;3 4;5 6)]"))
        .unwrap_err();
    assert!(error.contains("length mismatch"), "{error}");
}

#[test]
fn syntax_and_function_stack_guards_remain_recoverable() {
    let mut interpreter = Interpreter::new();
    for source in [
        format!("{}1{}", "(".repeat(200), ")".repeat(200)),
        format!("f:{{x}};f{}", "[1]".repeat(200)),
        "f:{f x};f 1".to_owned(),
    ] {
        assert!(interpreter.eval(&source).unwrap_err().contains("limit"));
        assert_eq!(interpreter.eval("1+2").unwrap().to_string(), "3");
    }
}

#[test]
fn function_arity_and_operand_domains_are_checked() {
    for (source, message) in [
        ("1[0]", "indexing requires an array"),
        ("+[]", "expects one or two arguments"),
        ("+/[]", "reduce and scan expect"),
        ("+'[]", "each expects"),
        ("round[{x};2]", "requires numbers"),
        ("+{x}", "flip requires"),
        ("4$2", "$ requires"),
    ] {
        let error = Interpreter::new().eval(source).unwrap_err();
        assert!(error.contains(message), "{source}: {error}");
    }
    evaluates("f:{x};f~f", "1");
    evaluates("{x}~{x}", "1");
    evaluates("1~()", "0");
    evaluates("?((1 2;3 4);(1 2;3 4);(2 3;4 5))", "((1 2;3 4);(2 3;4 5))");
    evaluates("<2 1 2 1", "1 3 0 2");
    evaluates("10 20?99", "2");
    evaluates("+\\[3;()]", "()");
}

#[test]
fn simple_numeric_lambdas_keep_projection_shadowing_and_fallback_behavior() {
    evaluates("f:{[x;y]x-y};g:f[10];g[3]", "7");
    evaluates("f:{[f]f*f};f[3]", "9");
    evaluates("f:{2*x};f[4]", "8");
    evaluates("f:{x-2};f[4]", "2");
    evaluates("f:{x*x};f[1 2 3]", "1 4 9");
    evaluates("a:3;f:{x*a};a:4;f[2]", "6");
    evaluates("f:{x+x};f[0n]", "0n");
    evaluates("f:{x%y};f[1;0]", "0w");
    evaluates("f:{x*x};f'1 2.5 3", "1 6.25 9");
    evaluates("a:0;f:{x+y};f[a:a+1;a:a+1];a", "2");
    assert!(Interpreter::new().eval("f:{x*x};f[`a]").is_err());
}

#[test]
fn optimized_numeric_calls_preserve_depth_limits() {
    for (body, last_allowed) in [("x*x", 126), ("+/1 2", 125), ("+/()", 126), ("+/,1", 126)] {
        let mut setup = format!("f0:{{[x]{body}}};");
        for depth in 1..=last_allowed + 1 {
            setup.push_str(&format!("f{depth}:{{f{}[x]}};", depth - 1));
        }
        let mut interpreter = Interpreter::new();
        interpreter.eval(&setup).unwrap();
        assert!(
            interpreter.eval(&format!("f{last_allowed}[2]")).is_ok(),
            "{body}"
        );
        let error = interpreter
            .eval(&format!("f{}[2]", last_allowed + 1))
            .unwrap_err();
        assert!(error.contains("depth limit"), "{body}: {error}");
    }
}
