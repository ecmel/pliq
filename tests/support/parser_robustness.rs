use super::parse;

#[test]
fn malformed_syntax_reports_errors() {
    for source in [
        "{[1]1}", "{[x;y}x}", "{x", "1)", "1;]", "1e", "1e+", "1e-", "[1]", "a:", "(1;)",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn mutated_programs_do_not_panic_in_the_parser() {
    // Parse only: mutations may turn a small valid input into an enormous range.
    let seeds = [
        "t:([] name:`a`b;age:1 2);t,:([] name:,`c;age:,3);t[`age;0]:4",
        "// comment\na:1 2;+ /a// trailing comment",
        "mm:{[a;b]{[row]+/row*b}' a};mm[(1 2;3 4);(5 6;7 8)]",
        "f:{$[x<2;1;x*f[x-1]]};f 10",
        "round[1.255;2]",
        "f:+[;2];f[3];^1 0n 2",
        "a:1 2;a[0 0]+:10 20;@[a;0;:;3]",
        "10 20+/:1 2;3{x+1}/0",
        "{ :x;99}[4];\"(]\"",
        "a:!10;a@&0=mod[a;2]",
        "codes:`USD`EUR;isUSD:{x=`USD};isUSD codes",
        "prices:`USD`EUR!1 1.08;.[prices];prices `EUR",
        "a:1 2;a[0 1]:3 4;d:(,`items)!,a;d[`items][-1]:9",
        "a:1 2;f:{a,:x};f[3];d:(,`items)!,a;d[`items],:4 5",
    ];
    let replacements = "01e.`+-*/\\'()[]{}:;$\n α";
    for seed in seeds {
        parse(seed).unwrap();
        for i in 0..seed.len() {
            for c in replacements.chars() {
                let mut mutated = seed.to_owned();
                mutated.replace_range(i..i + 1, &c.to_string());
                let _ = parse(&mutated);
            }
            let _ = parse(&seed[..i]);
        }
    }
}
