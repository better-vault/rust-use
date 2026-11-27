# The rust-use compact format, v1

This is what an agent reads and writes. Every byte here is paid for in tokens on every call, so
the format is line-oriented, has no JSON punctuation, and never repeats what the agent already
knows. The bridge also offers a JSON form of the same data for MCP clients that want structured
content. The text form is canonical.

## Hello

Send `hello` to read the server's crate version, the `Ui` session id, and its current revision.
The reply is one line:

```
rust-use 0.2.0 session 0123456789abcdef rev 7
```

The session id is a nonzero 64-bit value encoded as 16 lowercase hex digits. It identifies
one `Ui` instance across connections. `hello` leaves the connection's revision cursor untouched.

## Snapshot

Send `snapshot [--verbose] [--from @id] [--limit n]`.

```
rev 7
@1 mute-button "Mute" =false volume=40 !toggle !set_volume(volume:int)
@2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
@3 select "Country" =Sweden open ?query
```

- The first line is `rev <n>`, the `Ui` revision the snapshot reflects.
- Each component then gets one line, ordered by id:
  `@<id> <kind> ["<label>"] [=<value>] [<key>=<value> | <flag>]* [(!|!!)<action>[(<params>)]]* [?query]`.
  - `<kind>` is the primitive's `NAME`.
  - The label is quoted and is omitted when the component has none.
  - `=<value>` is the primary value. It is omitted when there is none, or when it is `null`
    (an empty `Option`).
  - Attributes appear in the order the component wrote them. A flag (`Value::Bool(true)`
    written with `Describe::flag`) is a bare key. An attribute whose value is `false` or
    `null` is omitted entirely.
  - Only actions that are enabled right now are listed, prefixed with `!`. Actions requiring a person use
    `!!`, as in `!!delete(id)`. Verbose docs append ` (needs a person: <text>)`.
    Structured action descriptions include the prompt in `confirm`.
  - Parameters are `name` when the type is `str`, `name:type` otherwise, and a trailing `?`
    marks an optional one: `!rename_to(name,note?)`, `!set_volume(volume:int)`.
  - `?query` marks a component that answers `query`.
- Hidden components are omitted from discovery and cannot be addressed. Secret field values
  are rendered as the string `"[redacted]"`.
- Values are written as:
  - **strings**: bare when they match `[A-Za-z0-9_./:+-]+` and are not `true`, `false`, `null`
    or numeric-looking; otherwise double-quoted with `\"`, `\\` and `\n` escapes.
  - **numbers and booleans**: as JSON.
  - **lists**: `[a,b,c]`, with items formatted by the same rules.
  - **objects**: compact JSON. Avoid them; a component should flatten into attributes.
- Action docs are omitted by default. `snapshot --verbose` (or
  `SnapshotOptions { docs: true, ..SnapshotOptions::default() }`) appends indented `# <doc>`
  lines under each component. These lines do not count toward the component limit.
- Each page includes up to 100 components by default. Use `--limit n` to set a different
  maximum. Use `--from @id` to start at that id or the next mounted id.
- If more components remain, the final line is `more <remaining count> from=@<next id>`.
  Pass that id as `--from` on the next snapshot. The final page has no `more` line.
  A limit of zero returns just the revision and any continuation.
- Structured snapshots include `more` with fields `remaining` and `from` (a numeric id)
  when there is a continuation.

`snapshot --from @2 --limit 1` returns:

```
rev 7
@2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
more 1 from=@3
```

`snapshot --from @3 --limit 1` returns:

```
rev 7
@3 select "Country" =Sweden open ?query
```

## Diff

The same lines as a snapshot, for only the components that changed since a revision, plus one
removal line `-@id` when a visible component is hidden or unmounted. Showing a component
emits its current snapshot line. The change set is uncapped, including when returned by
`act`.

```
rev 9
@2 board "Chess board" ="rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" turn=w !move(uci) !undo
-@4
```

The bridge remembers the last revision each agent session saw. `diff` with no argument means
"since my last snapshot, diff or act". `diff <n>` starts from revision `n` and advances the
session cursor to the returned revision. The CLI saves `<session hex>:<revision>` in
`<app>.rev` across invocations. Before sending `diff`, it checks the session and revision with
`hello` on the same connection. A saved revision from a different or unknown session, or one
ahead of the current revision, triggers `diff 0` and a CLI notice before the reply:

```
# app restarted; showing everything
```

## Act

A script is one or more steps separated by `;` or newlines. Outside values, `#` begins a comment that runs to the end of the line:

```
<ref> <action> [<arg>]*
```

- `<ref>` is `@<id>`, a label (quoted if it contains spaces: `"Chess board" move e2e4`), or a
  component kind when exactly one component has it (`board move e2e4`).
- Arguments use the value syntax above. A bare token that parses as an integer, float, `true`,
  `false` or `null` becomes that JSON value; anything else is a string.

The reply is `ok` followed by the diff produced by the whole script, so the agent never needs a
follow-up snapshot:

```
ok
rev 9
@2 board "Chess board" ="rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" turn=w !move(uci) !undo
```

When a step returns a non-null value, the line `=> <step index> <value>` comes before the diff.

On failure, steps before the failing one stay applied by default. The reply names the step
and gives enough for the agent to fix it without asking again:

```
err 2 @2 move e2e5: illegal move (legal: a2a3 a2a4 b1a3 b1c3 …)
rev 8
@2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
```

The error line comes first, then the diff of whatever the earlier steps changed.

### Atomic scripts

Start the script with `atomic` on its own line or followed by a semicolon. For example,
`atomic; board move e2e4; board move e2e5`. In Rust, use `Script::from(steps).atomic()`.
In the CLI, use `rust-use act --atomic "…"`. In MCP, pass `atomic: true` with the script.

Every target must resolve to a primitive that supports checkpoints before any steps run.
Use `#[primitive(checkpoint)]` on a `Clone` primitive, or implement the `Primitive::checkpoint`
and `Primitive::restore` hooks manually. A target without a checkpoint causes a validation error
with no revision change from the batch:

```
err 2 @3 set 50: @3 slider can't be rolled back (add #[primitive(checkpoint)])
```

If a step fails during dispatch, the reply adds `rolled back` after the error line. The diff
shows the restored components at their original state, or only the `rev` line when there
are no visible changes to report. Results from earlier steps are discarded. Subscribers see
ordinary action events as the script runs, then one restoration event per restored primitive
with a new revision.

```
err 2 board move e2e5: illegal move e2e5 (legal: a7a5 a7a6 …)
rolled back
rev 9
@2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" turn=w !move(uci) !reset
```

## Query

To search a component's options, start with its reference.

```
query "Country" sw            → text filter "sw", from 0
query "Country" sw from=50
```

The reply is one value per line, then `more from=<n>` when there are further pages, or `total <n>`
on the last page.

To find components, send `query --kind <kind> [text] [--from @id] [--limit n]` or
`query --text <text> [--from @id] [--limit n]`. The kind filter uses the exact primitive name.
Text matches a case-insensitive substring of the label, kind, or primary value. Text containing
whitespace can be double-quoted.

The reply uses snapshot format, including the revision line and the same default limit of 100.
The `more` line counts remaining matches and gives the next matching id. Repeat the same filter
with that id as `--from` to continue. Both forms of `query` leave the diff cursor unchanged.

`query --kind select SWE` returns:

```
rev 7
@3 select "Country" =Sweden open ?query
```

`query --text "CHESS BOARD"` returns:

```
rev 7
@2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
```
