---
type: decision
status: closed
blocked_by: [01]
---

# Adopt the verified Ratatui 0.30 idioms

## Evidence

| Fact | Verified at |
|---|---|
| `ratatui::run(closure)` wraps init + restore + **panic hook**; added in 0.30.0; `init()` installs the panic hook since 0.28.1 (signals still uncovered). | ratatui 0.30 release notes; official `panic` example; Context7 docs |
| `ListState` must live in app state ("offset computed during the previous draw call — natural scrolling"); fresh per-draw state loses it (selection stays visible; long lists re-anchor to the edge). | docs.rs 0.30 `ListState`; scratch experiments E1–E5 (this repo) |
| `Layout::{vertical,horizontal}([...]).areas()` is the 0.30 idiom; `Constraint::Fill(n)` splits proportionally (verified 3:2 → 60/40). | docs.rs 0.30; scratch experiment C |
| `ratatui::crossterm` re-export keeps the terminal/event versions locked; direct crossterm dep drifts silently. | ratatui 0.30 workspace manifest |
| In raw mode Ctrl-C arrives as a key event; unhandled = user cannot quit, no feedback. | ratatui `simple` template `on_key_event`; scratch knowledge |
| `Widget for &App` rendering whole layouts is the official example/template structure. | ratatui `todo-list` example; `event-driven` template `ui.rs` |

## Choice

Phase 4 switches the TUI to: `ratatui::run(|terminal| …)`; `Layout::{vertical,horizontal}
(...).areas()` with `Constraint::Fill`; two `ListState` fields owned by `App` (reset on item
changes) driving `render_stateful_widget`; imports via `ratatui::crossterm` (drop the direct
`crossterm` dep); Ctrl-C (`Char('c')` + Ctrl) quits. `Widget for &App` render entry point.
Status/event code stays functional.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Keep `Layout::default().direction(...).split(...)` | Works, older style; the 0.30 idiom is `areas()` destructuring |
| Keep per-draw `ListState` | Loses natural scrolling; duplicates selection state the struct already owns |
| Adopt the `component` template architecture | Over-architecture for this app; `event-driven` shape fits |

## Consequence

The TUI code matches the official examples/templates the learning pass established; terminal
lifecycle and panic handling are delegated; the render loop no longer owns service polling
(phase 5).