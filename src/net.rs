//! TCP transport.
//!
//! Connection threads exchange bytes only; values never cross a thread, so the
//! interpreter keeps its `Rc` representation and evaluates requests serially on
//! the thread that called [`listen`].

use std::io::{self, Read, Write};

/// Request encoding. Source text in, printed output back.
/// Reserved for later encodings: 1 for JSON, 2 for Arrow IPC.
pub(crate) const MODE_TEXT: u8 = 0;

/// Response kinds.
pub(crate) const TAG_OK: u8 = 0;
pub(crate) const TAG_ERROR: u8 = 1;

/// Caps the allocation a single frame header can request.
const MAX_FRAME: usize = 16 << 20;

const ATTACH_HELP: &str = r#"  \h  Show this help; keep pending input
  \c  Discard pending input
  \q  Detach; the daemon keeps running
"#;

/// A length-prefixed message: `[u32 little-endian length][u8 kind][payload]`,
/// where the length counts the kind byte and the payload.
#[derive(Debug)]
pub(crate) struct Frame {
    pub(crate) kind: u8,
    pub(crate) payload: Vec<u8>,
}

pub(crate) fn write_frame(writer: &mut impl Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    let length = payload.len() + 1;
    if length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame exceeds the maximum size",
        ));
    }
    writer.write_all(&(length as u32).to_le_bytes())?;
    writer.write_all(&[kind])?;
    writer.write_all(payload)?;
    writer.flush()
}

/// Returns `None` at a clean end of stream.
pub(crate) fn read_frame(reader: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut header = [0u8; 4];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let length = u32::from_le_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame length",
        ));
    }
    let mut kind = [0u8; 1];
    reader.read_exact(&mut kind)?;
    let mut payload = vec![0u8; length - 1];
    reader.read_exact(&mut payload)?;
    Ok(Some(Frame {
        kind: kind[0],
        payload,
    }))
}

use std::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Sender},
    thread,
};

use crate::Output;
use pliq::Interpreter;

/// One evaluation request and the channel its answer goes back on.
struct Request {
    mode: u8,
    payload: Vec<u8>,
    reply: Sender<(u8, Vec<u8>)>,
}

fn tcp_address(address: &str) -> String {
    if !address.is_empty() && address.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("127.0.0.1:{address}")
    } else {
        address.to_owned()
    }
}

/// Serve `address` until the process is stopped. Evaluation stays on this thread.
pub(crate) fn listen(
    interpreter: &mut Interpreter,
    address: &str,
    banner: &mut impl Write,
) -> io::Result<()> {
    let listener = TcpListener::bind(tcp_address(address))?;
    writeln!(banner, "pliq listening on {}", listener.local_addr()?)?;
    banner.flush()?;
    serve(interpreter, listener)
}

fn serve(interpreter: &mut Interpreter, listener: TcpListener) -> io::Result<()> {
    let (sender, requests) = mpsc::channel::<Request>();
    // The accept thread holds a sender for the process lifetime, so the loop
    // below never observes a closed channel and keeps serving.
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let sender = sender.clone();
            let peer = stream
                .peer_addr()
                .map(|address| address.to_string())
                .unwrap_or_else(|_| "unknown peer".into());
            let _ = writeln!(io::stderr().lock(), "pliq client connected: {peer}");
            thread::spawn(move || {
                connection(stream, &sender);
                let _ = writeln!(io::stderr().lock(), "pliq client disconnected: {peer}");
            });
        }
    });
    for request in requests {
        let answer = evaluate(interpreter, &request);
        let _ = request.reply.send(answer);
    }
    Ok(())
}

fn evaluate(interpreter: &mut Interpreter, request: &Request) -> (u8, Vec<u8>) {
    if request.mode != MODE_TEXT {
        return (
            TAG_ERROR,
            format!("unsupported request mode: {}", request.mode).into_bytes(),
        );
    }
    let Ok(source) = std::str::from_utf8(&request.payload) else {
        return (TAG_ERROR, b"request is not valid UTF-8".to_vec());
    };
    let (tag, payload) = match interpreter.eval(source) {
        Ok(value) => (TAG_OK, Output(value).to_string().into_bytes()),
        Err(error) => (TAG_ERROR, error.into_bytes()),
    };
    // A frame that cannot be written would otherwise drop the connection with
    // no explanation, so report the size instead.
    if payload.len() + 1 > MAX_FRAME {
        return (
            TAG_ERROR,
            format!(
                "result of {} bytes exceeds the {MAX_FRAME} byte response limit",
                payload.len()
            )
            .into_bytes(),
        );
    }
    (tag, payload)
}

/// Read frames, hand each to the interpreter thread, and write its answer back
/// to this client alone.
fn connection(mut stream: TcpStream, sender: &Sender<Request>) {
    if stream.set_nodelay(true).is_err() {
        return;
    }
    let (reply, answers) = mpsc::channel();
    while let Ok(Some(frame)) = read_frame(&mut stream) {
        let request = Request {
            mode: frame.kind,
            payload: frame.payload,
            reply: reply.clone(),
        };
        if sender.send(request).is_err() {
            break;
        }
        let Ok((tag, payload)) = answers.recv() else {
            break;
        };
        if write_frame(&mut stream, tag, &payload).is_err() {
            break;
        }
    }
}

/// A REPL against a daemon. Multiline input and the `\` commands stay here, so
/// each frame carries one complete expression.
pub(crate) fn attach(
    address: &str,
    mut input: impl crate::input::Input,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> io::Result<()> {
    let address = tcp_address(address);
    let mut stream = TcpStream::connect(&address)?;
    stream.set_nodelay(true)?;
    writeln!(
        output,
        "pliq attached to {address}. \\h for help, \\q to detach."
    )?;
    let mut source = String::new();
    loop {
        let prompt = if source.is_empty() { "  " } else { ".." };
        let Some(line) = input.line(prompt, output)? else {
            break;
        };
        match line.trim() {
            "\\q" => {
                source.clear();
                break;
            }
            "\\h" => {
                write!(output, "{ATTACH_HELP}")?;
                continue;
            }
            "\\c" => {
                source.clear();
                continue;
            }
            "" if source.is_empty() => continue,
            _ => {}
        }
        source.push_str(&line);
        source.push('\n');
        if crate::incomplete(&source) {
            continue;
        }
        input.remember(&source);
        write_frame(&mut stream, MODE_TEXT, source.as_bytes())?;
        source.clear();
        match read_frame(&mut stream)? {
            Some(frame) if frame.kind == TAG_OK => {
                writeln!(output, "{}", String::from_utf8_lossy(&frame.payload))?;
            }
            Some(frame) => {
                writeln!(errors, "error: {}", String::from_utf8_lossy(&frame.payload))?;
            }
            None => {
                writeln!(errors, "error: the daemon closed the connection")?;
                break;
            }
        }
    }
    if !source.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete expression at end of input",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/net.rs"]
mod tests;
