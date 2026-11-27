# Roadmap

Where rust-use is going after 0.1. Versions are targets, not promises: rust-use is in beta, and
the order changes with what people build. To push something up the list or propose something new,
[open a feature request](https://github.com/better-vault/rust-use/issues/new?template=feature_request.yml)
or start a [discussion](https://github.com/better-vault/rust-use/discussions).

Every new behavior has to clear one bar: an agent operates it with **one direct action**
(`reorder 3 0`, `expand "src"`, `set 40`), never a sequence of keys or pointer moves, and its
snapshot line stays short.

## Shipped in 0.1

`Press` · `Toggle` · `Field` · `Selection` · `List` · `RovingFocus` (lists and 2-D grids) ·
`Typeahead` · `Popup` · `DragDrop` · the agent bridge (`snapshot`, `diff`, `act`, `query`,
`allow`) · IPC and the `rust-use` CLI · the MCP shim · the GPUI adapter

## 0.2: collections, safety and Tauri

| Behavior | What an agent can do | Typical components |
| --- | --- | --- |
| `Reorder` | `move <from> <to>` in one step, validated; also between two lists (`Transfer`) | sortable lists, kanban columns, playlists |
| `Tree` | `expand`, `collapse`, `select` by path; `query` into closed branches; reorder within the tree | file trees, outlines, nested menus |
| `Range` | `set <value>` within bounds and steps; two thumbs for a span | sliders, volume, price filters |
| `Combobox` | `query` the options, then `pick`; free text where allowed | search pickers, country fields, tag inputs |
| `Tabs` | `select` a tab; focus and panel visibility follow | settings panes, editors |

**Agent safety.** What an agent must not see, and what it must not do alone:

- `#[agent(secret)]` shows a field as `[redacted]` in every snapshot and diff. Password-like
  fields are secret by default.
- `#[agent(hidden)]` and `Handle::hide_from_agents()` leave a component out of `snapshot` and
  `query` entirely.
- `#[action(confirm = "Delete the project")]` marks an action that needs a person. Without an
  `allow` hook that asks someone, the step fails with "needs a person".
- **Async `allow`.** The hook can wait for a person's answer, with a timeout, and the agent's `act`
  waits with it.
- **Busy and error state.** Snapshot lines carry `pending` while an async action runs and `error`
  when the last one failed, so an agent doesn't have to guess.

**`rust-use-tauri`.** An adapter for [Tauri](https://v2.tauri.app) v2. A plugin manages the `Ui` as
app state, exposes `snapshot`, `act` and `query` as Tauri commands, and emits a change event so the
webview re-renders. Agents reach a Tauri app over the same IPC and MCP paths as a GPUI app.

**Atomic `act`.** All steps apply or none do, opt-in per script.

## 0.3: text, overlays, history and big data

| Behavior | What an agent can do | Typical components |
| --- | --- | --- |
| `TextInput` | `set` text, `insert` at a position, `select` a range, see validation errors; input history | inputs, search boxes, editors |
| `InlineEdit` | `edit` a label in place and commit or cancel | renaming list items, table cells |
| `Dialog` | `confirm`, `cancel`; focus is trapped and restored for people | modals, alerts, sheets |
| `Menu` | `open` and `choose` an item by label in one step, including submenus | context menus, menubars, command palettes |
| `Viewport` | page through a long list with `query`, and `scroll_to` an item, without rendering it all; stick to the bottom for logs | virtualized lists, logs, chat transcripts |
| `Resize` | `set` a pane or column size | split views, table columns |
| `Toast` | read pending notifications, `dismiss` them | status messages |

**History and undo.** A `History` records every change with its source. An agent's changes can be
undone on their own, without touching the person's, and a set of changes can be staged, shown and
then committed or reverted.

**Smarter refs.** Reach a component inside a closed tab, accordion or page, which the bridge opens
on the way. Address one by place (`"Settings > Volume"`). Match labels whole-word, ignoring case
and accents.

**`observe`.** Agents subscribe to changes and receive diffs as they happen, instead of polling
`diff`. Each change says where it came from: keyboard, pointer, program or agent.

**Testing kit.** `rust_use::testing` mounts a primitive headless, drives it like a person or an
agent, and asserts on snapshots. Pattern checks verify the expected keyboard behavior of lists,
menus, tabs, trees and dialogs.

## Later

| Behavior | What an agent can do |
| --- | --- |
| `Canvas` | `pan`, `zoom`, `select` and `move` items by id on a 2-D surface; `connect` ports between nodes: diagrams, maps, boards |
| `CellRange` | `select` a block of cells and read or paste it as text: spreadsheets, data grids |
| `Lasso` | `select` every item inside a region |
| `Calendar` | `pick` a date or range, step months, respect disabled days; place spans on a timeline |
| `Area` | `set` a 2-D value: colour pickers, XY pads |
| `Hold` and `SwipeActions` | run what a long press or a swipe would, as one action |
| `Form` | read every field's validity at once, `submit`, `reset` |
| `Shortcuts` | list the app's commands and `run` one by name |
| `Clipboard` | `copy` and `paste` through the app, with the same `allow` checks |
| `Preferences` | read and `set` colour scheme and similar settings, persisted |

- **A component kit.** `rust-use-components`: ready-made GPUI components built from these behaviors
  (Select, Combobox, Dialog, Menu, Tabs, Slider, Toast, Date Picker, Tree, …), unstyled and
  themable, so a GPUI app is agent-ready from its first screen.
- **A model inside the app.** Run Claude in the app itself with the three tools, through the
  Claude API's tool runner, for in-app assistants that act on the UI directly.
- **Accessibility.** Screen-reader announcements and right-to-left layouts in the behaviors that
  need them.
- **Git gpui.** CI against Zed's git gpui as well as crates.io, and a documented way to build with
  either. Many GPUI apps use the git version.
- **More adapters.** Candidates are [Dioxus](https://dioxuslabs.com), [egui](https://www.egui.rs)
  and [Iced](https://iced.rs). [Expo Exact](https://github.com/expo/exact) is worth revisiting once
  it has a stable integration point for Rust state; today it is pre-1.0, ships no packaged release
  and changes daily.
- **Hardware.** A `rust-use-hw` crate behind a `hw` feature, so the same vocabulary drives physical
  devices: buttons, dials, sensors and displays as primitives an agent can see and operate, over the
  same bridge.
- **1.0.** Freeze the compact format and the core API once the behaviors above have settled.

Several of these ideas come from [ui-sdk](https://ui-sdk.dev), the JavaScript library rust-use
shares its approach with.
