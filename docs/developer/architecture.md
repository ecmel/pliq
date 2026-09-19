# Architecture

[Developer guide](index.md) · [Source map](development.md#source-map) · [Rust API](rust-api.md)

## Execution flow

```text
CLI / REPL / socket client / Rust caller
    -> Interpreter::eval(source)
    -> lexer and parser -> expression tree
    -> name resolution -> nodes with function frame slots
    -> evaluator + persistent environment + call-frame stack
    -> operator / numeric / collection operations
    -> Result<Value, String>
    -> display or host-side inspection
```

The parser lexes bytes into tokens with positions, then builds an `Expr` tree.
The tree represents literals, calls, lambdas, assignment, append, modified
assignment, returns, projections, and lazy conditionals. Newline handling
depends on the containing delimiter. Both syntax nesting and evaluator
application have depth guards.

Each `Interpreter::eval` parses and resolves the complete source before
executing it and creates an evaluator for that call. Resolution turns the
expression tree into evaluation nodes: top-level names stay names in the
persistent environment, while each function literal becomes a shared prototype
whose names are numbered frame slots. Primitives become shared function values,
and applications of a primitive to all of its operands skip the function value.
Numeric strands and constant lists keep a prebuilt buffer; each evaluation
still creates a new mutable array that shares it until an update.

The interpreter retains its environment between calls. Statements execute in
order; expression/application evaluation preserves the language's right-to-left
rules. The final statement supplies the result. The evaluator walks the resolved
nodes; no bytecode compiler or JIT is involved.

## Socket transport

`--listen` serves a TCP listener. One thread accepts connections and one
further thread per connection reads and writes frames. Those threads exchange
byte frames and reply channels over `std::sync::mpsc`; values never cross a
thread, so the interpreter keeps its `Rc` representation unchanged.

The thread that started listening owns the interpreter and drains the request
channel, so requests are evaluated one at a time in arrival order against the
single persistent environment. Each answer is written back only on the
connection that sent it. Attached sessions therefore share bindings and observe
each other's shared-container mutations, and one long-running request delays
every other session.

Requests carry a mode byte and responses a tag byte, so further encodings can
be added without changing the framing. Attached REPLs use a mode that carries
their console size, so the daemon renders only the part of a result that each
session will show. See
[CLI and REPL](../user/cli.md#connecting-from-other-programs) for the frame
layout.

## Environments and functions

Top-level code uses the persistent environment, an `Rc<HashMap<String, Value>>`.
Binding updates use `Rc::make_mut`, separating maps when needed without copying
the mutable containers inside their values.

Function bodies use call frames instead. Resolution numbers every name a body
binds or reads: parameters first, then captured and local names. A call pushes
one slot per name onto the evaluator's value stack and truncates the stack on
return, so reading or assigning a local is an index rather than a hash lookup,
and a call copies no environment.

Free-name analysis records the outer names a closure reads before assigning
them. It accounts for assignment order, branches, and nested lambdas, and runs
once per function literal during resolution. Creating a closure copies the
current values of those names into its captures; names unbound at that point
stay unbound. Captures keep their lexical values when an outer name is rebound.
Assigning an anonymous function to a name makes calls bind that name to the
function itself, supporting recursion without storing a reference cycle in the
captures.

Text evaluated by a direct `value` or `.` call can read, rebind, and create
names in the calling function. For that evaluation, the frame's bound slots
form a temporary environment. Afterwards, names with slots are written back, and
other new names stay with the frame for later evaluated text. Like the function
body, evaluated text sees parameters, locals, and captures, not uncaptured
globals. Frames without evaluated text pay nothing for this.

Function values distinguish binary verbs, explicit unary primitives, named
native functions, user functions, projections, compositions, and iterator-derived
functions. Application fills projection holes or dispatches to the appropriate
implementation. A return carries a value through evaluator control flow and is
not handled as an ordinary trapped language error.

## Shared storage

| Value      | Storage                                                       |
| ---------- | ------------------------------------------------------------- |
| Number     | Tagged `bool`, `i64`, or `f64`                                |
| Symbol     | Immutable `Rc<str>`                                           |
| Array      | Shared `Rc<RefCell<Storage>>`; Arrow vectors or typed runtime values |
| Dictionary | Shared ordered key/value arrays behind `Rc<RefCell<...>>`     |
| Table | Shared ordered named arrays; row count is the longest column |
| String     | Shared byte storage behind `Rc<RefCell<Rc<Vec<u8>>>>`         |

Cloning a container shares mutable identity. Numeric and symbol arrays hold
reference-counted Arrow buffers. Updates construct replacement buffers, so
snapshot readers keep old storage. Nested homogeneous arrays, strings, functions,
and dictionaries retain typed runtime storage to preserve shared child identity. This internal storage
strategy does not change the language's alias-visible mutation semantics.

`Array::iter()` snapshots the element sequence; nested containers remain
shared. `Array::to_arrow()` shares numeric or symbol buffers; `Array::as_numbers()`
materializes a numeric snapshot. `MutableString::snapshot()` shares byte storage. Iterators can therefore survive appends and
replacements from callbacks.

Dictionary keys are detached recursively on insertion to stabilize lookup
identity. Keys retain order and can repeat. Value views have independent outer
arrays while nested containers can remain shared. Dictionary values may have
independent types: homogeneous values retain typed storage, while mixed values
use a runtime value list. Ordinary arrays and table columns remain homogeneous.

### Dictionary indexing

Dictionaries retain ordered key/value arrays and maintain a separate lookup
index. Integer, boolean, character, symbol, and string keys use hash indexing,
with expected constant lookup time in the number of dictionary entries. Hashing
strings and compound keys still takes time proportional to their contents.

Float matching uses a relative tolerance and is not transitive, so raw float
bits cannot serve as hash keys. A sorted index finds candidates within a
conservative tolerance range, then language matching checks them. Compound keys
use a structural hash and their first float, if present, to narrow candidates.
Lookup returns the earliest matching entry, preserving duplicate-key behavior.
Float range searches take logarithmic time plus candidate comparisons.

Function captures can mutate, so fingerprints exclude function contents. Keys
that share a fingerprint, including function keys and some compound keys, still
require candidate comparisons and can have linear worst-case lookup time.
Insertion maintains the index and cached maximum key depth; missing-key lookup
does not scan all keys to decide whether its argument is a key or a list of keys.
Failed updates restore the index and depth together with the ordered arrays.

## Memory management

pliq manages memory automatically through Rust's reference counting (`Rc`).
Shared arrays, dictionaries, strings, functions, and closure captures stay
alive while references to them exist. When the last strong reference to an
allocation is dropped, Rust releases it and drops the values it owns. This can
release further allocations whose reference counts also reach zero.

A closure owns references to its captured values. A nested function can therefore
keep those values alive after its enclosing function returns. Rebinding an outer
name does not release a value still held by a closure or another alias. The
interpreter's current bindings, values returned to a Rust caller, and storage
snapshots can also keep allocations alive.

Reference counting alone cannot reclaim a reference cycle: objects in the cycle
keep each other's counts above zero even when nothing else can reach them. For
example, storing a closure in an array that the closure captures would create a
cycle. pliq rejects updates that would create such cycles, including indirect
cycles through dictionaries and function captures. Recursive functions avoid
cycles by binding their own name during each call, as described above.

With cycles excluded, reference counting is sufficient; pliq has no separate
tracing garbage collector that scans for unreachable values. Supporting cyclic
structures in the future would require additional cycle management, such as a
cycle collector or tracing garbage collector.

## Update validation

Indexed writes validate the destination, replacement shape/types, and cycle
constraints before committing changes. Multi-destination updates preserve
destinations when validation fails. Append snapshots its source, including
self-append, and rejects inserted values that would create cycles.

Cycle checking traverses arrays, dictionaries, strings, and function captures.
Functional amend detaches data containers before applying changes, whereas
indexed assignment commits to existing shared containers. This distinction is
covered by dedicated [mutation tests](testing.md).

The guarantee is local to updates. Argument evaluation and callbacks can have
earlier side effects; the whole interpreter call is not transactional.

## Numeric semantics

`number.rs` owns integer wrapping, IEEE arithmetic and numeric promotion,
and tolerant language comparisons. Keep signed-boundary arithmetic independent
of debug/release overflow settings. See [testing](testing.md) for arithmetic
reference checks and regression coverage.

See [Arrow backend](arrow.md) for type rules, kernel coverage, and import limits.
