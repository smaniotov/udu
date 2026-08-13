---
type: decision
status: closed
blocked_by: []
---

# Split the crate into a library (`wayvibes_tui`) and a thin binary (`klack`)

## Evidence

| Fact | Verified at |
|---|---|
| A package can contain both a lib (`src/lib.rs`) and a bin (`src/main.rs`); bins can use the sibling lib. | Cargo Book Package Layout / Cargo Targets (current, primary) |
| Lib name auto-derives from package name with dashes→underscores (`wayvibes-tui` → `wayvibes_tui`); bin keeps the custom `[[bin]] name = "klack"`. | Cargo Targets; local `Cargo.toml` |
| `tests/` integration tests compile against the library's **public API only** — impossible without a lib. | The Rust Book ch11-03 |
| `main.rs` currently declares all 8 modules and is orchestration-shaped (parse → load → init → loop → restore → commit). | local `src/main.rs` |

## Choice

Add `src/lib.rs` exporting the modules (`pub mod {app, cli, config, device, process, service,
soundpack, ui}`), keep `[[bin]] name = "klack"`, and slim `main.rs` to `fn main` +
orchestration calling into `wayvibes_tui::…`. Explicit `[lib] name = "wayvibes_tui"` for
clarity. Terminal lifecycle (`ratatui::init/restore`) stays in the binary.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Keep everything in main.rs | No integration tests against a public API; the "fat main" pattern the refactor exists to remove |
| Separate workspace/package per module | Overkill for a single-tool crate; no shared-code reuse case beyond this app |

## Consequence

Unlocks `tests/` integration tests (notably `tests/cli.rs` in phase 2) and clean separation of
lib logic from terminal lifecycle. `cargo test` exercises lib + bin together.