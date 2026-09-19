# CLI and REPL

[User guide](index.md) · [Troubleshooting](troubleshooting.md)

## Invocation

| Command | Behavior |
| --- | --- |
| `pliq` | Start the REPL when stdin is a terminal; otherwise read stdin to EOF |
| `pliq -e 'expression'` | Evaluate one source argument |
| `pliq path.pliq` | Read and evaluate a UTF-8 source file |
| `pliq -` | Read source from stdin to EOF, including from a terminal |
| `pliq --listen [HOST:]PORT [path.pliq]` | Evaluate an optional file, then serve TCP |
| `pliq --attach [HOST:]PORT` | Attach a REPL to a running daemon |
| `pliq --help` or `pliq -h` | Print usage |

The file suffix is a convention; the CLI accepts a single path that does not
start with `-`. It does not pass additional arguments to a program. Each CLI
invocation creates a fresh interpreter, except `--attach`, which uses the
interpreter of the daemon it connects to.

```sh
pliq -e '+/!100'
pliq examples/stats.pliq
pliq - < examples/stats.pliq
```

Quote expressions so the shell does not expand `$`, backticks, `*`, or other
language punctuation. The commands above use POSIX shell quoting; adapt quoting
when invoking pliq from another shell.

## Output and exit status

Expression, file, and stdin modes print the last statement's value followed by
a newline. Tables display column headers and aligned rows, with `0n` for missing
cells. Dictionaries display aligned key/value rows. They print `error: ...` to stderr and return a failure exit status on
an error. Help and successful evaluation return success. Intermediate
statements are evaluated but not individually printed.

Output follows [pliq's display conventions](values.md#display). There is no
JSON-output switch or separate precision mode. Use `round[value;places]` for
explicit numeric rounding; `--precision` is rejected with guidance to use it.

## Interactive commands

The REPL prints a version banner, then a two-space prompt. Each completed input
prints its last result. Bindings persist between inputs.

| Command | Effect |
| --- | --- |
| `\h` | Show command help; retain pending input and bindings |
| `\c` | Show the console size for results |
| `\c rows columns` | Set the size: 5 to 1000 rows and at least 20 columns; `0` for the most |
| `\c auto` | Return to the default size |
| `\d` | Discard pending multiline input; retain existing bindings |
| `\q` | Discard pending input and exit |

These commands must occupy the input line. They are REPL commands, not language
expressions usable inside a source file. `\d` does not reset the interpreter;
start a new process for a fresh environment. Lines such as `:c`, `:d`, `:h`, and
`:q` remain language expressions, including early returns inside multiline
functions.

## Console size

A REPL result never shows more than 1000 rows. In a terminal it also fits the
terminal's width, measured again for each result so a resized window takes
effect; scrolling back covers the height. A line cut at the width ends in `..`. A table or
dictionary with more rows than fit shows its first rows, then `..` and its row
or entry count:

```text
  \c 6 30
  !1000
0 1 2 3 4 5 6 7 8 9 10 11 12..
  ([] price:!100)
price
-----
    0
    1
..
100 rows
```

`\c rows columns` sets a fixed size of 5 to 1000 rows and at least 20 columns.
`0` rows means 1000 and `0` columns removes the width limit, so `\c 0 0` shows
up to 1000 rows at any width. `\c` alone prints the current size, marked
`(auto)` for the default, and `\c auto` restores it. Only the part that fits is
rendered, so a large result prints quickly. Error messages are never cut.

To see later rows, index them, as in `t[1000+!1000]`. Output from `-e`, files,
and piped standard input is never limited. An attached REPL reading piped input
has no width limit until `\c` sets one.

## Multiline input

An unclosed `(`, `[`, `{`, or quoted string changes the prompt to `..` and reads
another line. Delimiters in strings and comments are ignored. Mismatched closing
delimiters are sent to the parser for an error instead of causing indefinite
continuation. Use `\d` to cancel a partly entered expression.

EOF exits normally when there is no pending source. EOF during an incomplete
expression produces an error. An ordinary evaluation error is printed to stderr
and the REPL continues. Earlier side effects can remain; see
[error recovery](troubleshooting.md#error-recovery).

In a terminal, use Up/Down to recall previous expressions, Ctrl-R to search
history, and the arrow keys to edit input. Completed multiline expressions are
stored as a single entry, including expressions that produce evaluation errors.
Blank input, cancelled expressions, and REPL commands are not stored.
Ctrl-C cancels pending input and keeps bindings; Ctrl-D on an empty line sends EOF.

Local and attached REPLs share history in `.pliq_history` in your home directory
(`~/.pliq_history` on Unix). New entries are saved after each completed expression
and loaded on startup. History contains the source text you enter. If history
cannot be loaded or saved, a warning is printed and the session continues;
without a home directory, history lasts only for the current session.
Piped input uses ordinary line input without history.

## Daemon and attached sessions

`pliq --listen` evaluates an optional startup file, then serves TCP until the process is stopped. `pliq --attach` opens a REPL against a
running daemon instead of against a local interpreter.

```sh
pliq --listen 127.0.0.1:5139 startup.pliq   # one terminal
pliq --attach 127.0.0.1:5139                # any number of others
```

The listener logs client connections and disconnections to standard error,
including each client's IP address and port.

Every attached session shares one interpreter and one set of bindings. A
function defined in one session is callable from all the others. Requests are
evaluated one at a time in arrival order, so a long-running expression delays
every other session until it finishes.

Because bindings are shared, keep daemon state in a dictionary or table.
Mutation through a shared container is visible everywhere, while rebinding a
name is not:

```text
state[`hits]:state[`hits]+1   // visible to every session
n:n+1                         // rebinding; local to this expression
```

`\h`, `\c`, `\d`, and `\q` behave as they do in the local REPL, except that
`\q` detaches and leaves the daemon running. Each session sends its own console
size with each request, and the daemon renders results to fit it. Stop the daemon itself with a signal,
such as Ctrl-C in its own terminal.

### TCP addresses and access

A bare port defaults to IPv4 loopback: `pliq --listen 5139` listens on
`127.0.0.1:5139`, and `pliq --attach 5139` connects there.

To specify a host, use `HOST:PORT`, for example `127.0.0.1:5139`, `localhost:5139`, or
`[::1]:5139` for IPv6 loopback. Port `0` lets the operating system choose an
available port; the startup banner prints the actual bound address and port.
An address already in use is refused.

Use a loopback address for local sessions. TCP has no owner-only file
permissions: other local users can connect too. The daemon has no authentication
or encryption, and clients can evaluate against its full environment. Binding
to `0.0.0.0` or `[::]` exposes it on network interfaces; use only trusted networks
with appropriate access controls.

### Connecting from other programs

Any language that can open a TCP connection can drive a daemon. Requests and
responses are length-prefixed frames:

| Field | Bytes | Meaning |
| --- | --- | --- |
| length | 4 | Little-endian `u32`, counting the kind byte and the payload |
| kind | 1 | Request mode, or response tag |
| payload | length - 1 | UTF-8 source, or printed output |

Request mode `0` sends source text and returns its printed result. Mode `3`
does the same within a console size: its payload starts with the rows and then
the columns as little-endian `u32` values, with `0` for no limit, followed by
the source text. Modes `1` and `2` are reserved for later encodings. Response
tag `0` carries a result and `1` carries an error message.

An attached REPL sends mode `3`. When a daemon from version 0.1.0 answers
`unsupported request mode: 3`, the REPL repeats that request in mode `0` and
keeps using mode `0` for the rest of the session.

There is no handshake and no authentication step: connect and send. Requests on
one connection are answered in order, one at a time, and each request receives
exactly one response on the connection that sent it. A connection can carry a
single request or stay open for many.

```python
import socket, struct

def send(sock, source, mode=0):
    payload = source.encode()
    sock.sendall(struct.pack("<I", len(payload) + 1) + bytes([mode]) + payload)

def receive(sock):
    def exactly(count):
        buffer = b""
        while len(buffer) < count:
            chunk = sock.recv(count - len(buffer))
            if not chunk:
                raise ConnectionError("the daemon closed the connection")
            buffer += chunk
        return buffer

    length = struct.unpack("<I", exactly(4))[0]
    tag = exactly(1)[0]
    return tag, exactly(length - 1).decode()

sock = socket.create_connection(("127.0.0.1", 5139))
sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
send(sock, "+/!100")
tag, output = receive(sock)          # (0, '4950')
```

Always read exactly the advertised number of bytes. A single socket read can
return part of a frame.

#### Limits and failure modes

Frames are limited to 16 MiB in each direction.

| Condition | Result |
| --- | --- |
| Source that fails to parse or evaluate | Tag `1` with the error message |
| Payload that is not valid UTF-8 | Tag `1` |
| Unrecognized request mode | Tag `1` |
| Mode `3` payload shorter than its 8-byte size | Tag `1` |
| Result larger than the frame limit | Tag `1` reporting the size |
| Request frame with an invalid length | The daemon closes the connection |
| Daemon stopped | The connection closes and reads return no data |

An error response leaves the connection usable, and bindings made before the
error persist; see [error recovery](troubleshooting.md#error-recovery).
Responses use pliq's display conventions, which are meant for reading rather
than parsing. `--listen` and `--attach` use TCP on Linux, macOS, and Windows.
