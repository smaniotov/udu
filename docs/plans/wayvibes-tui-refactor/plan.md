# Refactor wayvibes-tui into a clean library-crate architecture

## Destination

wayvibes-tui is restructured as a library crate (`wayvibes_tui`) with a thin `klack` binary.
Each systemic manual spot is replaced by the ready library the master-learning pass selected:
CLI parsing via `clap` derive, domain errors via `thiserror` with `anyhow` only at the
application boundary. The TUI follows the confirmed Ratatui 0.30 idioms (`ratatui::run`,
`Layout::{vertical,horizontal}(...).areas()`, `Constraint::Fill`, `ListState` owned by `App`,
`ratatui::crossterm`, Ctrl-C handling). Service-status polling is decoupled from the render
tick; device discovery tolerates unreadable `/dev/input` nodes; the volume clamp lives in one
place; config saves atomically. All existing behavior, user-facing text, and tests are
preserved. Each phase is reviewed by an isolated agent before moving on.

## Notes

- Stack pinned: Rust edition 2024, `rust-version` floor 1.86 (toolchain 1.97.1); ratatui 0.30.0
  ↔ crossterm 0.29.0; evdev 0.13.2; serde 1.
- Skills in play: `ratatui` (0.30 idioms, verified), `rust` (idioms), `increments` (surgical
  steps), `tdd`/`test-triage` (test discipline), `code-style` (form), `planning` (this map).
- Gates per phase: `cargo fmt -- --check` · `cargo test` · `cargo clippy --all-targets
  --all-features -- -D warnings` · `cargo build --release`.
- Phase review: an isolated reviewer agent (fresh context) approves each phase against this
  plan before the next phase starts. Reject → fix → re-review loop until approval.
- Commits: one GPG-signed commit per phase, English message, `sounds/` assets excluded.

## Decisions taken

- [[01-crate-split]] — split into lib crate `wayvibes_tui` + thin binary `klack`.
- [[02-clap-cli]] — replace the hand-rolled CLI parser with clap 4 derive.
- [[03-error-model]] — thiserror 2 for the 5 domain error enums; anyhow + `.context()` at the
  app boundary.
- [[04-ratatui-idioms]] — adopt the verified Ratatui 0.30 idioms.
- [[05-service-polling]] — decouple service-status checks from the render tick.
- [[06-device-tolerance]] — skip unreadable input nodes instead of failing the whole scan.
- [[07-clamp-unity]] — one `clamp_volume` in the lib owns the volume invariant.
- [[08-atomic-config-save]] — config written via temp file + rename.
- [[09-phase-review-gate]] — isolated agent reviews each phase before approval.

## Fog

- `research` — exact clap line to pin at install (`cargo add clap --features derive` confirms
  4.6.x; verify MSRV vs 1.86). — blocked by: nothing: takeable at phase 2
- `decision` — ownership of key handling after the split: `App::handle_event` in the lib with
  the `ui` module rendering only. — blocked by: 01
- `research` — confirm `ratatui::run` panic-hook behavior vs external SIGTERM before adopting
  it (already documented: init installs the hook since 0.28.1; signals remain uncovered). —
  blocked by: nothing: takeable at phase 4

## Out of scope

- tracing/logging (deferred by the research until service-mode debugging hurts).
- TestBackend whole-screen snapshot tests; unit buffer tests already exist.
- Any change to the systemd unit contract, `pkill` policy, `ServiceLock`, or the wayvibes
  child contract (Chesterton's fence: behavior is deliberate, documented in `08-service-lifecycle`).
- Editing the external `wayvibes` backend or bundled sounds.
- cargo-generate adoption: templates are a style reference for existing apps, not a migration
  path.

## Phases (execution shape — one phase = one transform, one commit, one review)

| # | Phase | Files | Contracts / behavior to cover | Acceptance (oracle + gate) |
|---|---|---|---|---|
| 1 | Crate split: lib + thin bin | `src/lib.rs` (new), `src/main.rs`, `Cargo.toml` | `pub mod {app, cli, config, device, process, service, soundpack, ui}` + public API from lib; `main.rs` only `fn main` + orchestration via `wayvibes_tui::…`; bin stays `klack` | Gates green; all existing unit tests pass (import path updates only); binary still named `klack`; no logic moved to main |
| 2 | CLI via clap | `src/cli.rs`, `src/main.rs` (apply path), `Cargo.toml`, tests | Same surface: `--config`, `--wayvibes`, `--root` (repeatable), `--soundpack`, `--device-name`, `--service`, `-h/--help`; `#[command(name = "klack")]`; help shows `klack` | Existing parse tests re-target `try_parse_from`; `--help` output verified; missing-value and unknown-option errors still reported; gates green |
| 3 | Error model: thiserror + anyhow | `src/cli.rs` (CliError), `config.rs`, `service.rs`, `soundpack.rs`, `device.rs`, `app.rs`, `ui.rs`, `main.rs`, `Cargo.toml` | 5 enums → `#[derive(Error, Debug)]` with `#[error]` messages copied verbatim; `#[from]` replaces manual `From`; `Box<dyn Error>` at app boundary → `anyhow::Result` + `.context()` | User-facing error text unchanged (tests assert samples); enums keep `Send + Sync`; no `anyhow` inside domain modules; gates green |
| 4 | Ratatui 0.30 idioms | `src/main.rs` (`ratatui::run`), `src/ui.rs`, `src/app.rs` | `Layout::{vertical,horizontal}(...).areas()` + `Constraint::Fill`; `ListState` owned in `App` (reset on item change); `ratatui::crossterm` imports (drop direct crossterm dep); `KeyCode::Char('c')`+Ctrl quit | Existing `TestBackend` tests pass unchanged; new test proves navigation via owned `ListState`; help modal/focus/gauge/footer behavior preserved; gates green |
| 5 | Service polling decoupled | `src/app.rs`, `src/main.rs`, `src/service.rs` | Status feature kept; `is_active` checked ≤ 1×/5 s (deadline), plus after apply/refresh; never inside draw | Unit test on the throttle logic (clock/counter injected); reviewer confirms no subprocess spawn in the tight loop; gates green |
| 6 | Device tolerance + clamp unity + atomic save | `src/device.rs`, `src/config.rs` (+`clamp_volume`, atomic save), `service.rs`, `process.rs`, `app.rs` | Discovery skips unreadable nodes and still lists readable ones; single `clamp_volume` used by config/service/app/process; config persisted via tmp + rename (no `PartialEq` breakage: keep semantics) | Tests: unreadable-node filter, clamp identity across call sites, atomic save leaves no partial file; reviewer checks no duplicated clamp remains; gates green |

## Rules that govern execution

- Contracts yes, function bodies no. Each phase's acceptance is the oracle; green is not the
  deliverable — the observable behavior is.
- No ceremonial steps. TDD discipline via `test-triage`; every behavior change starts from a
  failing test where a phase touches behavior.
- Stay faithful to this plan; scope creep in a phase is a plan amendment, not an edit.