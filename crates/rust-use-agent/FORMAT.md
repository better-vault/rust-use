# The rust-use compact format, v1

This is what an agent reads and writes. Every byte here is paid for in tokens on every call, so
the format is line-oriented, has no JSON punctuation, and never repeats what the agent already
knows. The bridge also offers a JSON form of the same data for MCP clients that want structured
content. The text form is canonical.

## Snapshot

```
rev 7
@1 mute-button "Mute" =false volume=40 !toggle !set_volume(volume:int)
@2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
@3 select "Country" =Sweden open ?query
```

- The first line is `rev <n>`, the `Ui` revision the snapshot reflects.
- Each component then gets one line, ordered by id:
  `@<id> <kind> ["<label>"] [=<value>] [<key>=<value> | <flag>]* [!<action>[(<params>)]]* [?query]`.
  - `<kind>` is the primitive's `NAME`.
  - The label is quoted and is omitted when the component has none.
  - `=<value>` is the primary value. It is omitted when there is none.
  - Attributes appear in the order the component wrote them. A flag (`Value::Bool(true)`
    written with `Describe::flag`) is a bare key. An attribute whose value is `false` or
    `null` is omitted entirely.
  - Only actions that are enabled right now are listed, prefixed with `!`.
  - Parameters are `name` when the type is `str`, `name:type` otherwise, and a trailing `?`
    marks an optional one: `!rename_to(name,note?)`, `!set_volume(volume:int)`.
  - `?query` marks a component that answers `query`.
- Values are written as:
  - **strings**: bare when they match `[A-Za-z0-9_./:+-]+` and are not `true`, `false`, `null`
    or numeric-looking; otherwise double-quoted with `\"`, `\\` and `\n` escapes.
  - **numbers and booleans**: as JSON.
  - **lists**: `[a,b,c]`, with items formatted by the same rules.
  - **objects**: compact JSON. Avoid them; a component should flatten into attributes.
- Action docs are not repeated in a snapshot. `snapshot --verbose` (or
  `SnapshotOptions { docs: true }`) appends an indented `# <doc>` line under each component, one
  per action, for the first time an agent meets a screen.

## Diff

The same lines as a snapshot, for only the components that changed since a revision, plus one
removal line per unmounted component:

```
rev 9
@2 board "Chess board" ="rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" turn=w !move(uci) !undo
-@4
```

The bridge remembers the last revision each agent session saw. `diff` with no argument means
"since my last snapshot, diff or act". `diff <n>` starts from revision `n` and advances the
session cursor to the returned revision. The CLI saves this revision in `<app>.rev` across invocations.

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

On failure, steps before the failing one stay applied by default (`atomic` is reserved for a later
version). The reply names the step and gives enough for the agent to fix it without asking again:

```
err 2 @2 move e2e5: illegal move (legal: a2a3 a2a4 b1a3 b1c3 …)
rev 8
@2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
```

The error line comes first, then the diff of whatever the earlier steps changed.

## Query

```
query "Country" sw            → text filter "sw", from 0
query "Country" sw from=50
```

The reply is one value per line, then `more from=<n>` when there are further pages, or `total <n>`
on the last page.
