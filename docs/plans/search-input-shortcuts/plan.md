# Fix: search is entered explicitly with `/` and then suppresses shortcuts

## Destination

Typing a soundpack search currently fires single-letter shortcuts that collide with
search characters (`s` opens Settings, `p` previews, `r` refreshes, `q` quits, …) because
`handle_launcher_key` matches shortcuts before the `type_search` catch-all. Per the user's
decision, search must be entered **explicitly and consciously with `/`**: until `/` is
pressed the launcher behaves normally (shortcuts active); once search mode is entered,
all printable keys — including the shortcut letters — are inserted into the query and no
shortcut fires. This removes the first-character ambiguity entirely. Scoped to `src/ui.rs`
key handling + a `searching` flag in `src/app.rs`.

## Notes

- [[01-search-suppresses-shortcuts]] — decision: explicit `/` opens search mode; while
  active, shortcuts are suppressed and printable keys type into the query.

## Fog

- (none) — resolved: `/` is the sole search entry, so a shortcut letter typed as the
  first search character cannot collide; it is simply typed into the query.

## Out of scope

- Redesign of the shortcut scheme beyond the search-mode gate (key remapping, chords).
- Any change to the Settings screen, modals, or the backend.

## Phases (execution shape — one phase = one transform, one commit, one review)

| # | Phase | Files | Contracts / behavior to cover | Acceptance (oracle + gate) |
|---|---|---|---|---|
| 1 | Explicit `/` search mode suppresses shortcuts | `src/ui.rs`, `src/app.rs` (+ tests) | `/` in the launcher (not searching) enters search mode; while searching, `Up/Down/Enter/Backspace/Esc` + printable keys work, `q/s/?/x/+/-/1/2/3/p/r/U` type into the query and do NOT fire; `Esc` clears the query and exits search mode (shortcuts restored); `Ctrl+Q/C` always quit; with an inactive search every shortcut behaves as before; the header offers `[/] search` at the minimum terminal width | Existing tests pass minus the two explicitly updated for the `/`-entry flow; new tests: `/` then `s` inserts `s` without opening Settings; `[/]` present in header hints at width 78; reviewer confirms `s` (no `/`) still opens Settings; gates green |
