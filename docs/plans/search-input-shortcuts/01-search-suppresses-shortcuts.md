---
type: decision
status: closed
blocked_by: []
---

# Search is entered explicitly with `/` and then suppresses shortcuts

## Evidence

| Fact | Verified at |
|---|---|
| `handle_launcher_key` matches shortcut keys (`q`, `s`, `?`, `x`, `+/-`, `1/2/3`, `p`, `r`, `U`) **before** the `type_search` catch-all, so shortcut letters can't be typed. | local `src/ui.rs:788` |
| The launcher has no search-mode flag today; an active search is only signalled by a non-empty `search_query`, which cannot represent a just-opened empty search. | local `src/app.rs:246,331` |
| User decision (plan approval note): search must be entered by pressing `/` "de forma 100% consciente" — explicit entry, not implicit type-to-search. | plan approval feedback |
| `docs/ux-criteria.md` codifies the pattern "while the modal is open, background shortcuts do not activate packs, change volume, or quit" — an active focused context suppresses background shortcuts. | `docs/ux-criteria.md`, interaction clarity |

## Choice

Add a `searching: bool` flag to `App` (default `false`). `handle_launcher_key` routes to
`handle_search_key` only while `searching` is true. `/` on an inactive launcher calls
`App::start_search()` (clear query, set `searching`, select first pack). While in search
mode, printable keys (including `/` and space) go to `type_search` and no single-letter
shortcut fires; `Up/Down/Enter` navigate/activate; `Backspace` edits the query; `Esc` calls
`clear_search()` which clears the query and leaves search mode. On an inactive launcher every
shortcut keeps its current behavior. `type_search`/`backspace_search` keep `searching` true;
`clear_search` sets it false, so direct calls stay consistent with the UI flow. `Ctrl+C/`Ctrl+Q`
quit unconditionally.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Suppress shortcuts only while the query is non-empty (no `/`) | Leaves the first-character ambiguity the user rejected: `s` as the first key still opens Settings |
| Move the `type_search` catch-all before all shortcuts | Makes an implicit first key search and leaves no conscious search entry; conflicts with the user's explicit `/` requirement |

## Consequence

Search requires one conscious keystroke (`/`); after that, every shortcut letter types into
the query and no sound/volume/settings side-effect fires mid-search. `Esc` restores the
normal launcher with shortcuts active. Existing behavior is preserved when no search is
in progress.