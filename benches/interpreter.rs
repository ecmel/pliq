//! Interpreter benchmarks: `cargo bench`, or `cargo bench -- NAME` to filter.
//!
//! Each benchmark evaluates setup code once, then times repeated evaluations of
//! one program in the same interpreter and checks the program's result. Without
//! `--bench` (for example under `cargo test --benches`), every program runs once
//! as a smoke test.
use std::time::{Duration, Instant};

use pliq::Interpreter;

struct Benchmark {
    name: &'static str,
    setup: &'static str,
    program: &'static str,
    expected: &'static str,
}

const BENCHMARKS: &[Benchmark] = &[
    // Function calls: frames, captures, and primitive dispatch.
    Benchmark {
        name: "each_arithmetic_lambda",
        setup: "",
        program: "+/{x*x}'[!200000]",
        expected: "2666646666700000",
    },
    Benchmark {
        name: "each_lambda",
        setup: "",
        program: "+/{x*x+1}'[!200000]",
        expected: "2666666666600000",
    },
    Benchmark {
        name: "fold_lambda",
        setup: "",
        program: "{x+y*y}/[!200000]",
        expected: "2666646666700000",
    },
    Benchmark {
        name: "recursion",
        setup: "fib:{$[x<2;x;fib[x-1]+fib[x-2]]}",
        program: "fib 20",
        expected: "6765",
    },
    Benchmark {
        name: "closure_captures",
        setup: "a:1;b:2;c:3;d:4;e:5;f:{a+b+c+d+e+x}",
        program: "+/f'[!200000]",
        expected: "20002900000",
    },
    Benchmark {
        name: "closure_creation",
        setup: "mk:{[a]{[b]a+b}}",
        program: "+/{mk[x][1]}'[!100000]",
        expected: "5000050000",
    },
    Benchmark {
        name: "local_assignment",
        setup: "f:{[n]s:0;i:n;s+:i*i;s}",
        program: "+/f'[!100000]",
        expected: "333328333350000",
    },
    Benchmark {
        name: "nested_calls",
        setup: "g:{x+1};h:{g g x}",
        program: "+/h'[!200000]",
        expected: "20000300000",
    },
    Benchmark {
        name: "conditional",
        setup: "",
        program: "+/{$[x>5;x;0-x]}'[!200000]",
        expected: "19999899970",
    },
    Benchmark {
        name: "literal_vector",
        setup: "",
        program: "+/{+/x+1 2 3}'[!100000]",
        expected: "15000450000",
    },
    Benchmark {
        name: "dynamic_value",
        setup: "",
        program: "+/{[x] value \"x+1\"}'[!20000]",
        expected: "200010000",
    },
    // Whole-array primitives: interpreter overhead is amortized.
    Benchmark {
        name: "vector_arithmetic",
        setup: "a:!1000000",
        program: "+/a*a+1",
        expected: "333333333333000000",
    },
    Benchmark {
        name: "vector_scalar",
        setup: "a:!1000000",
        program: "+/(a*2)+a*3",
        expected: "2499997500000",
    },
    Benchmark {
        name: "vector_compare",
        setup: "a:!1000000",
        program: "+/a<500000",
        expected: "500000",
    },
    Benchmark {
        name: "vector_mixed_types",
        setup: "a:!1000000",
        program: "+/a*2.5",
        expected: "1249998750000",
    },
    Benchmark {
        name: "vector_scan",
        setup: "a:!1000000",
        program: "#+\\a",
        expected: "1000000",
    },
    Benchmark {
        name: "grade",
        setup: "a:100000?1000000",
        program: "#<a",
        expected: "100000",
    },
    Benchmark {
        name: "dictionary_lookup",
        setup: "k:{`$string x}'[!10000];d:k!!10000",
        program: "+/d k",
        expected: "49995000",
    },
];

const TARGET: Duration = Duration::from_millis(500);
const MIN_SAMPLES: usize = 5;
const MAX_SAMPLES: usize = 100;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let bench = args.iter().any(|arg| arg == "--bench");
    let filters = args
        .iter()
        .filter(|arg| !arg.starts_with("--"))
        .collect::<Vec<_>>();
    let selected = BENCHMARKS
        .iter()
        .filter(|b| filters.is_empty() || filters.iter().any(|f| b.name.contains(f.as_str())))
        .collect::<Vec<_>>();
    if bench {
        println!(
            "{:<24} {:>12} {:>12} {:>8}",
            "benchmark", "median", "min", "samples"
        );
    }
    for benchmark in selected {
        let mut interpreter = Interpreter::new();
        interpreter
            .eval(benchmark.setup)
            .unwrap_or_else(|e| panic!("{} setup: {e}", benchmark.name));
        let mut run = || {
            let start = Instant::now();
            let result = interpreter
                .eval(benchmark.program)
                .unwrap_or_else(|e| panic!("{}: {e}", benchmark.name));
            let elapsed = start.elapsed();
            assert_eq!(result.to_string(), benchmark.expected, "{}", benchmark.name);
            elapsed
        };
        let first = run();
        if !bench {
            println!("{} ok", benchmark.name);
            continue;
        }
        let count = (TARGET.as_secs_f64() / first.as_secs_f64().max(1e-9)) as usize;
        let mut samples = (0..count.clamp(MIN_SAMPLES, MAX_SAMPLES))
            .map(|_| run())
            .collect::<Vec<_>>();
        samples.sort();
        println!(
            "{:<24} {:>12} {:>12} {:>8}",
            benchmark.name,
            format_duration(samples[samples.len() / 2]),
            format_duration(samples[0]),
            samples.len()
        );
    }
}

fn format_duration(duration: Duration) -> String {
    let micros = duration.as_secs_f64() * 1e6;
    if micros >= 1000.0 {
        format!("{:.2} ms", micros / 1000.0)
    } else {
        format!("{micros:.1} µs")
    }
}
