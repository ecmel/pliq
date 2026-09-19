use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn expression_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["-e", "+/!10"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "45\n");
}

#[test]
fn mutation_example_runs_from_a_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/mutation.pliq"
        ))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"(99 7 9 4;1 2)\n");
}

#[test]
fn stdin_mode() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"a:1 2 3\n+/a\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "6\n");
}

#[test]
fn errors_have_nonzero_exit_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["-e", "1+`bad"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("requires numbers")
    );
}

#[test]
fn native_floats_and_explicit_rounding_are_available() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["-e", "round[1%3;5]"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "0.33333\n");
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["--precision", "5", "-e", "1%3"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--precision is not supported")
    );
}

#[test]
fn help_and_invalid_arguments() {
    for flag in ["--help", "-h"] {
        let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
            .arg(flag)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains("Usage:"));
        assert!(output.stderr.is_empty());
    }
    for args in [vec!["-e"], vec!["--unknown"], vec!["-e", "1", "extra"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr).unwrap().contains("usage:"));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn file_mode_and_file_errors() {
    // A directory unique to this test process; remove only files we created.
    let directory = std::env::temp_dir().join(format!("pliq-cli-test-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let file = directory.join("program with spaces.pliq");
    std::fs::write(&file, "square:{x*x}\nsquare 5\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .arg(&file)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"25\n");
    for path in [directory.join("missing.pliq"), file.clone()] {
        std::fs::write(&file, [0xff]).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
            .arg(path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn explicit_stdin_and_invalid_input() {
    for (input, expected) in [
        (b"2+3".as_slice(), Some(b"5\n".as_slice())),
        (b"", Some(b"()\n")),
        (b"\xff", None),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pliq"))
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.success(), expected.is_some());
        if let Some(expected) = expected {
            assert_eq!(output.stdout, expected);
        } else {
            assert!(!output.stderr.is_empty());
        }
    }
}

#[test]
fn tables_print_headers_aligned_rows_and_missing_cells() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["-e", "([] name:`alice`bob;age:30 25 40)"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(
        output.stdout,
        b"name  age\n----- ---\nalice  30\nbob    25\n0n     40\n"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .args(["-e", "([] name:0#`alice;age:())"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"name age\n---- ---\n");
}

#[test]
fn dictionaries_print_aligned_key_value_rows() {
    for (source, expected) in [
        ("`alice`bob!30 25", "alice | 30\nbob   | 25\n"),
        ("`a`a!1 2", "a | 1\na | 2\n"),
        ("(,`items)!,1 2", "items | 1 2\n"),
        ("()!()", "(()!())\n"),
        ("(1 2;3 4)!5 6", "1 2 | 5\n3 4 | 6\n"),
        ("`a`long!0n 2", "a    | 0n\nlong | 2\n"),
        ("(,`$\"a\\nb\")!,`$\"x\\ty\"", "a\\nb | x\\ty\n"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
            .args(["-e", source])
            .output()
            .unwrap();
        assert!(output.status.success(), "{source}: {:?}", output.stderr);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn listen_runs_a_script_then_serves_attached_repls() {
    use std::{
        fs,
        io::{BufRead, BufReader},
    };

    let directory = std::env::temp_dir().join(format!("pliq-cli-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let script = directory.join("startup.pliq");
    fs::write(&script, "seed:41\n").unwrap();

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .arg("--listen")
        .arg("0")
        .arg(&script)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut banner = String::new();
    BufReader::new(daemon.stdout.take().unwrap())
        .read_line(&mut banner)
        .unwrap();
    let address = banner.trim().strip_prefix("pliq listening on ").unwrap();
    let port = address.strip_prefix("127.0.0.1:").unwrap();

    let mut client = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .arg("--attach")
        .arg(port)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    client
        .stdin
        .take()
        .unwrap()
        .write_all(b"seed+1\n\\q\n")
        .unwrap();
    let attached = client.wait_with_output().unwrap();

    daemon.kill().unwrap();
    daemon.wait().unwrap();
    let _ = fs::remove_dir_all(&directory);

    assert!(attached.status.success(), "{:?}", attached.stderr);
    let text = String::from_utf8(attached.stdout).unwrap();
    assert!(text.contains("42"), "{text}");
}

#[test]
fn attaching_to_an_invalid_address_fails_clearly() {
    let output = Command::new(env!("CARGO_BIN_EXE_pliq"))
        .arg("--attach")
        .arg("127.0.0.1:invalid")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("error:"),
        "{:?}",
        output.stderr
    );
}
