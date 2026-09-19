use super::*;

/// Bind before spawning so parallel tests use distinct OS-assigned ports.
fn daemon() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    thread::spawn(move || {
        let mut interpreter = Interpreter::new();
        serve(&mut interpreter, listener).unwrap();
    });
    address
}

fn connect(address: &str) -> TcpStream {
    let stream = TcpStream::connect(address).unwrap();
    stream.set_nodelay(true).unwrap();
    stream
}

fn request(stream: &mut TcpStream, source: &str) -> (u8, String) {
    write_frame(stream, MODE_TEXT, source.as_bytes()).unwrap();
    let frame = read_frame(stream).unwrap().unwrap();
    (frame.kind, String::from_utf8(frame.payload).unwrap())
}

#[test]
fn frames_round_trip_including_empty_payloads() {
    let mut buffer = Vec::new();
    write_frame(&mut buffer, MODE_TEXT, b"+/!100").unwrap();
    write_frame(&mut buffer, TAG_ERROR, b"").unwrap();
    let mut reader = buffer.as_slice();

    let first = read_frame(&mut reader).unwrap().unwrap();
    assert_eq!(first.kind, MODE_TEXT);
    assert_eq!(first.payload, b"+/!100");

    let second = read_frame(&mut reader).unwrap().unwrap();
    assert_eq!(second.kind, TAG_ERROR);
    assert!(second.payload.is_empty());

    assert!(read_frame(&mut reader).unwrap().is_none());
}

#[test]
fn frame_lengths_are_validated() {
    let mut zero = &0u32.to_le_bytes()[..];
    assert_eq!(
        read_frame(&mut zero).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );

    let mut oversized = Vec::from(u32::MAX.to_le_bytes());
    oversized.push(MODE_TEXT);
    assert_eq!(
        read_frame(&mut oversized.as_slice()).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn truncated_payloads_are_an_error_not_a_clean_end() {
    let mut truncated = Vec::from(8u32.to_le_bytes());
    truncated.extend_from_slice(b"\0ab");
    assert_eq!(
        read_frame(&mut truncated.as_slice()).unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn bindings_persist_across_requests_on_one_connection() {
    let address = daemon();
    let mut client = connect(&address);

    assert_eq!(
        request(&mut client, "a:10 20 30"),
        (TAG_OK, "10 20 30".into())
    );
    assert_eq!(request(&mut client, "+/a"), (TAG_OK, "60".into()));
}

#[test]
fn errors_come_back_tagged_and_the_connection_survives() {
    let address = daemon();
    let mut client = connect(&address);

    let (tag, message) = request(&mut client, "1+`bad");
    assert_eq!(tag, TAG_ERROR);
    assert!(message.contains("requires numbers"), "{message}");

    assert_eq!(request(&mut client, "1+1"), (TAG_OK, "2".into()));
}

#[test]
fn concurrent_clients_share_one_environment() {
    let address = daemon();
    let mut first = connect(&address);
    let mut second = connect(&address);

    assert_eq!(request(&mut first, "state:`hits`misses!0 0").0, TAG_OK);
    // The mutation arrives on one connection and is read back on the other.
    assert_eq!(request(&mut second, "state[`hits]:7"), (TAG_OK, "7".into()));
    assert_eq!(request(&mut first, "state[`hits]"), (TAG_OK, "7".into()));
}

#[test]
fn oversized_results_are_reported_and_the_session_survives() {
    let address = daemon();
    let mut client = connect(&address);

    let (tag, message) = request(&mut client, "20000000#\"x\"");
    assert_eq!(tag, TAG_ERROR);
    assert!(message.contains("exceeds"), "{message}");

    // The connection is still usable.
    assert_eq!(request(&mut client, "1+1"), (TAG_OK, "2".into()));
}

#[test]
fn unknown_request_modes_are_rejected() {
    let address = daemon();
    let mut client = connect(&address);

    write_frame(&mut client, 9, b"1+1").unwrap();
    let frame = read_frame(&mut client).unwrap().unwrap();
    assert_eq!(frame.kind, TAG_ERROR);
    assert!(
        String::from_utf8_lossy(&frame.payload).contains("unsupported request mode"),
        "{:?}",
        frame.payload
    );
}

#[test]
fn listening_on_an_occupied_address_fails() {
    let address = daemon();
    let error = listen(&mut Interpreter::new(), &address, &mut Vec::new()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
}

#[test]
fn attached_sessions_handle_multiline_input_and_detach() {
    let address = daemon();
    connect(&address);

    let mut output = Vec::new();
    let mut errors = Vec::new();
    attach(
        &address,
        &b"f:{\nx+1 // } ignored\n}\nf 4\n1+`bad\n\\q\n"[..],
        &mut output,
        &mut errors,
    )
    .unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("pliq attached to"), "{output}");
    assert!(output.contains(".."), "{output}");
    assert!(output.contains("5\n"), "{output}");
    let errors = String::from_utf8(errors).unwrap();
    assert!(errors.contains("requires numbers"), "{errors}");
}

#[test]
fn an_attached_session_sees_daemon_state() {
    let address = daemon();
    let mut seeded = connect(&address);
    request(&mut seeded, "shared:41");

    let mut output = Vec::new();
    attach(
        &address,
        &b"shared+1\n\\q\n"[..],
        &mut output,
        &mut Vec::new(),
    )
    .unwrap();
    assert!(String::from_utf8(output).unwrap().contains("42"));
}

#[test]
fn attached_sessions_limit_results_to_their_console_size() {
    let address = daemon();
    let mut output = Vec::new();
    attach(
        &address,
        &b"\\c 6 20\n!1000\n([] a:!10)\n\\c 0 0\n!30\n\\q\n"[..],
        &mut output,
        &mut Vec::new(),
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("  0 1 2 3 4 5 6 7 8 ..\n"), "{output}");
    assert!(output.contains("a\n-\n0\n1\n..\n10 rows\n"), "{output}");
    assert!(output.contains("27 28 29\n"), "{output}");
}

#[test]
fn console_requests_carry_their_size_before_the_source() {
    let address = daemon();
    let mut client = connect(&address);
    let mut payload = Vec::new();
    payload.extend(6u32.to_le_bytes());
    payload.extend(20u32.to_le_bytes());
    payload.extend(b"!1000");
    write_frame(&mut client, MODE_CONSOLE, &payload).unwrap();
    let frame = read_frame(&mut client).unwrap().unwrap();
    assert_eq!(frame.kind, TAG_OK);
    assert_eq!(frame.payload, b"0 1 2 3 4 5 6 7 8 ..");

    write_frame(&mut client, MODE_CONSOLE, &[6, 0, 0]).unwrap();
    let frame = read_frame(&mut client).unwrap().unwrap();
    assert_eq!(frame.kind, TAG_ERROR);
    assert!(String::from_utf8_lossy(&frame.payload).contains("missing its size"));

    // Text requests still return the whole result.
    assert_eq!(request(&mut client, "#$!1000").0, TAG_OK);
    assert_eq!(request(&mut client, "!30").1.len(), 79);
}

#[test]
fn attached_sessions_fall_back_to_text_requests_for_older_daemons() {
    // Answer as version 0.1.0 did: only text requests are evaluated.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let modes = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut modes = Vec::new();
        while let Some(frame) = read_frame(&mut stream).unwrap() {
            modes.push(frame.kind);
            if frame.kind == MODE_TEXT {
                let mut answer = b"old ".to_vec();
                answer.extend(frame.payload.trim_ascii_end());
                write_frame(&mut stream, TAG_OK, &answer).unwrap();
            } else {
                let refusal = format!("unsupported request mode: {}", frame.kind);
                write_frame(&mut stream, TAG_ERROR, refusal.as_bytes()).unwrap();
            }
        }
        modes
    });
    let mut output = Vec::new();
    let mut errors = Vec::new();
    attach(&address, &b"1+1\n2+2\n\\q\n"[..], &mut output, &mut errors).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("old 1+1\n"), "{output}");
    assert!(output.contains("old 2+2\n"), "{output}");
    assert!(errors.is_empty(), "{}", String::from_utf8_lossy(&errors));
    assert_eq!(modes.join().unwrap(), [MODE_CONSOLE, MODE_TEXT, MODE_TEXT]);
}
