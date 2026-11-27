# Roadmap

Where rust-use is going after 0.1.0. Versions are targets, not promises: rust-use is in beta, and
the order changes with what people build. To push something up the list or propose something new,
[open a feature request](https://github.com/better-vault/rust-use/issues/new?template=feature_request.yml)
or start a [discussion](https://github.com/better-vault/rust-use/discussions).

Every new behavior has to clear one bar: an agent operates it with **one direct action**
(`reorder 3 0`, `expand "src"`, `set 40`), never a sequence of keys or pointer moves, and its
snapshot line stays short.

## Shipped in 0.1.0

`Press` · `Toggle` · `Field` · `Selection` · `List` · `RovingFocus` (lists and 2-D grids) ·
`Typeahead` · `Popup` · `DragDrop` · the agent bridge (`snapshot`, `diff`, `act`, `query`,
`allow`) · IPC and the `rust-use` CLI · the MCP shim · the GPUI adapter

## 0.2 — collections and Tauri

| Behavior | What an agent can do | Typical components |
| --- | --- | --- |
| `Reorder` | `move <from> <to>` in one step, validated | sortable lists, kanban columns, playlists |
| `Tree` | `expand`, `collapse`, `select` by path; `query` into closed branches | file trees, outlines, nested menus |
| `Range` | `set <value>` within bounds and steps; two thumbs for a span | sliders, volume, price filters |
| `Combobox` | `query` the options, then `pick`; free text where allowed | search pickers, country fields, tag inputs |
| `Tabs` | `select` a tab; focus and panel visibility follow | settings panes, editors |

- **`rust-use-tauri`.** An adapter for [Tauri](https://v2.tauri.app) v2. A plugin manages the `Ui`
  as app state, exposes `snapshot`, `act` and `query` as Tauri commands, and emits a change event
  so the webview re-renders. Agents reach a Tauri app over the same IPC and MCP paths as a GPUI app.
- **Atomic `act`.** All steps apply or none do, as an opt-in per script.

## 0.3 — text, overlays and big data

| Behavior | What an agent can do | Typical components |
| --- | --- | --- |
| `TextInput` | `set` text, `insert` at a position, `select` a range, see validation errors | inputs, search boxes, editors |
| `Dialog` | `confirm`, `cancel`; focus is trapped and restored for people | modals, alerts, sheets |
| `Menu` | `open` and `choose` an item by label in one step, including submenus | context menus, menubars, command palettes |
| `Viewport` | page through a long list with `query`, and `scroll_to` an item, without rendering it all | virtualized lists, logs, tables |
| `Resize` | `set` a pane or column size | split views, table columns |
| `Toast` | read pending notifications, `dismiss` them | status messages |

- **`observe`.** Agents subscribe to changes and receive diffs as they happen, instead of polling
  `diff`.

## Later

| Behavior | What an agent can do |
| --- | --- |
| `Canvas` | `pan`, `zoom`, `select` and `move` items by id on a 2-D surface: diagrams, maps, boards |
| `Calendar` | `pick` a date or range, step months, respect disabled days |
| `Form` | read every field's validity at once, `submit`, `reset` |
| `History` | app-wide `undo` and `redo` that agents and people share |
| `Shortcuts` | list the app's commands and `run` one by name |
| `Clipboard` | `copy` and `paste` through the app, with the same `allow` checks |

- **More adapters.** Candidates are [Dioxus](https://dioxuslabs.com), [egui](https://www.egui.rs)
  and [Iced](https://iced.rs). [Expo Exact](https://github.com/expo/exact) is worth revisiting once
  it has a stable integration point for Rust state; today it is pre-1.0, ships no packaged release
  and changes daily.
- **Hardware.** A `rust-use-hw` crate behind a `hw` feature, so the same vocabulary drives physical
  devices: buttons, dials, sensors and displays as primitives an agent can see and operate, over the
  same bridge.
- **1.0.** Freeze the compact format and the core API once the behaviors above have settled.
