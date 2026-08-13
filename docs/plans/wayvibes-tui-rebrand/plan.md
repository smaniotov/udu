# Rebrand the app and reach feature parity with the official macOS Klack

## Destination
The application (currently branded `klack`, crate `wayvibes-tui`) receives a new
original product name and is re-scoped to reproduce — on our Linux/Rust/evdev/cpal
stack, through the first-party backend and the TUI — the full observable feature set
of the official macOS Klack app, mapped feature by feature in this plan.

## Notes
- Stack stays: Rust 2024, ratatui 0.30, evdev 0.13.2, cpal 0.18.1 + symphonia 0.6.0,
  serde; systemd `*.service` + control socket; gates unchanged: `cargo fmt -- --check`
  · `cargo test` · `cargo clippy --all-targets --all-features -- -D warnings` ·
  `cargo build --release`.
- Reference architecture: the first-party backend plan (docs/plans/wayvibes-tui-backend/)
  is complete and live (capture → mapping → audio → control socket; TUI apply-on-Enter).
  Parity features plug into those modules; nothing below reopens them.
- Rename surface measured (2026-08-09): binary `klack` (`Cargo.toml [[bin]]`), lib
  crate `wayvibes_tui` + package `wayvibes-tui`, systemd `SERVICE_NAME=klack.service`
  (+ legacy `wayvibes-tui.service` already migrated), socket `klack.sock`
  (backend/control.rs `SOCKET_NAME`), config dir `wayvibes-tui/` (config.json) and data
  dir `wayvibes-tui/soundpacks`, README (11 refs), CLI help strings, tests.
- Reference identity constraints: the official macOS app is called "Klack"
  (klack.app) — a trademark/collision concern the rename must avoid. "wayvibes" is
  also taken by the former external project.
- The official Klack is a macOS app; some of its features depend on macOS
  accessibility APIs. Parity here means "same observable user value" on Linux, not
  byte-identical internals.
- Skills in play: `planning` (this map), `research`/`master-learning` (Klack feature
  inventory + versioned docs), `code-style`/`simplicity`/`increments` + `tdd` per
  implementation phase.

## Decisions taken
- [[01-name]] — the app is renamed **udu** (binary/service/socket `udu`; user decision; meaning: Igbo pot-drum percussion; no system collision).
- [[05-parity-scope]] — parity covers **all F1–F22**, prioritized: portable core first, macOS-bound analogues after; proprietary Klack assets out of scope.
- [[06-curated-features]] — the operative scope is the **14 user-selected features** (F1–F6, F8–F10, F12, F14, F17, F18, F20); F7/F11/F13/F15/F16/F19/F21/F22 excluded by the user.
- [[04-rollout]] — **full rename including data dirs**: binary/service/socket/lib `udu`, config `~/.config/udu/`, packs `~/.local/share/udu/`; first launch migrates old config+packs (kept, not deleted) and retires `klack.service`.

## Fog
(empty — map complete)

## Out of scope
- macOS port or a graphical UI.
- Using the official Klack's proprietary sound assets (licensing; parity covers the
  feature surface and pack model, not their content).
- Trademark registration and App Store distribution.
- Reopening the completed first-party backend plan.