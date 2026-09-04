# udu Engineering Guide

This file defines the working contract for the `udu` codebase. It applies to the whole repository unless a more specific `AGENTS.md` is added below it.

## What udu is

`udu` is a Linux-only Rust application that plays mechanical-keyboard sounds. It reads key events from one configured evdev keyboard, maps them to Mechvibes-format soundpack files, and plays them through a low-latency audio engine. A Ratatui TUI controls a per-user systemd service through a JSON-lines Unix socket.

The most important product and security properties are:

- key events stay inside the process; never log, persist, export, or transmit individual keys;
- there is no network client or network protocol;
- mute disables capture by closing the keyboard device, not merely by silencing audio;
- configuration changes apply to the running backend without restarting it;
- a removed keyboard is recovered by reconnecting with backoff;
- third-party soundpacks are treated as untrusted input and cannot escape their configured roots;
- the application must run as the user, never as root.

When a proposed change threatens one of these properties, stop and resolve the design before implementing it.

## Current contracts and names

The current runtime names are `udu`, `udu.service`, and `$XDG_RUNTIME_DIR/udu.sock`. Older ADRs and plans contain historical names such as `wayvibes` and `klack`; those documents preserve earlier decisions and must not be copied into new code or documentation.

The current public behavior is documented in `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, and `THREAT_MODEL.md`. Architecture decisions live in `docs/adr/`. Read the relevant ADR before changing a protocol, boundary, dependency, service behavior, mapping rule, or performance budget.

## Repository map

- `src/main.rs` — startup, migrations, service mode, and the Ratatui event loop.
- `src/app.rs` — TUI application state, user actions, backend synchronization, and screen behavior.
- `src/ui.rs` — rendering and input dispatch. Keep presentation concerns here; do not move backend policy into widgets.
- `src/control.rs` — TUI-side client for the backend control socket.
- `src/backend/` — the in-process service backend:
  - `mod.rs` owns engine orchestration, shared state, event dispatch, reconnect, mute, and status;
  - `capture.rs` owns evdev discovery/opening, polling, filtering, and device-loss detection;
  - `mapping.rs` owns Mechvibes/iohook-to-evdev mapping;
  - `audio.rs` owns decoding, cache, voices, mixing, normalization, limiting, and output routing;
  - `control.rs` owns the Unix socket server, request validation, command application, and path confinement;
  - `stats.rs` owns aggregate statistics and their owner-only persistence.
- `src/config.rs` — persisted configuration, defaults, clamping, and legacy migrations.
- `src/device.rs` — keyboard discovery and device metadata.
- `src/soundpack.rs` — soundpack parsing, validation, discovery, and path-safety checks.
- `src/service.rs` — systemd user-unit installation, migration, lifecycle, locking, and rendering.
- `tests/` — integration and environment-dependent performance tests.
- `examples/` — focused probes and smoke programs, not application architecture.
- `scripts/` — development-only tooling such as TUI screenshot capture.

Keep dependencies flowing toward the domain: TUI/client code may call the backend contract, but mapping and audio code must not depend on UI code. Service management belongs in `service.rs`, not in render functions.

## Before changing code

1. Read the target module and its neighboring tests.
2. Search for callers and the existing implementation of the behavior before adding a helper, trait, command, config field, or dependency.
3. Read the relevant ADR and update it when the accepted decision changes.
4. Preserve the existing boundary. A new dependency, public API, socket command, persisted field, service-unit setting, soundpack rule, or cross-module ownership change needs explicit design discussion before implementation.
5. Prefer the smallest change that solves the demonstrated problem. Do not add speculative abstractions, compatibility layers, or future-facing configuration.

For a non-trivial external contribution, open an issue before the PR. Small fixes with an obvious scope can go directly to a PR, as described in `CONTRIBUTING.md`.

## Rust and code shape

Follow idiomatic Rust and the surrounding code:

- Use `Result` for expected failures and `Option` for expected absence. Use `thiserror` for domain errors and keep `anyhow` at the application edge.
- Propagate errors with `?` and add actionable context. Do not use `unwrap()` in production paths; use a justified `expect` only for a proven invariant. `expect` is normal in test setup when the message identifies the setup failure.
- Keep public APIs minimal. Use private helpers for local transformations and introduce a trait only when there are multiple real implementations or a test boundary genuinely needs one.
- Prefer borrowed inputs (`&str`, `&Path`, slices) when ownership is not required. Do not clone merely to silence the borrow checker; reduce the borrow scope or return a new value first.
- Prefer enums and typed errors over sentinel values and unstructured strings. Keep matches exhaustive for domain enums.
- Use iterators for transformations and filtering. A mutable accumulator is acceptable when it is the clearest or measured efficient form; do not turn simple collection building into nested imperative loops.
- Keep functions focused on one transformation. Separate pure mapping/calculation from filesystem, process, socket, device, and audio effects.
- Name by domain intent (`resolve_keyboard`, `set_output_device`, `parse_pack`), not by implementation detail. Keep one term for one concept across the crate.
- Keep comments rare. Code, names, tests, and ADRs should explain what; comments are for non-obvious why, safety rationale, or a temporary constraint with an issue/reference.
- Do not add `unsafe` without a narrow boundary, a documented safety argument, and tests around the observable behavior. Existing libc boundaries are concentrated in service locking and evdev polling.

### Concurrency and real-time paths

- The audio callback and keyboard capture path are latency-sensitive. Do not add blocking filesystem, process, socket, or unbounded allocation work to them without measuring the effect.
- Keep decode and other potentially slow work off the audio callback. Reuse the existing decode cache and voice-pool design.
- Keep mutex scopes short. Do not hold a configuration or mapping lock while doing disk I/O, socket I/O, or audio playback.
- Preserve the current reconnect behavior: device loss becomes a reconnect loop with backoff, not a process restart or a busy loop.
- Preserve the current event filtering contract: only the intended keyboard events become sounds; repeats and unrelated input events must not leak into playback.

### Ratatui UI

- Keep the render loop deterministic and non-blocking. Do not run `systemctl`, spawn processes, perform unbounded I/O, or perform device discovery from a draw closure.
- Keep application state in `App`; keep rendering in `ui.rs`; route user actions through named app methods.
- Preserve `ratatui::run` terminal lifecycle and the existing `ratatui::crossterm` dependency path. Do not add a separate `crossterm` dependency.
- Keep list selection and scrolling state in application state rather than recreating it on every frame.
- Test user-visible state and rendered behavior with `TestBackend` where appropriate, rather than asserting implementation details.
- Preserve keyboard accessibility and modal behavior: a modal must consume its own input, and Ctrl-C must remain a visible, reliable exit path.

## Backend and security invariants

Treat all external input as untrusted: evdev events, soundpack JSON, soundpack paths, audio bytes, socket requests, device names, and persisted files.

- Validate soundpack paths before loading or playing. Reject absolute references and `..` traversal, and keep all resolution within configured roots.
- Keep the Unix socket request limit and read/write timeouts. Preserve peer-UID validation; socket directory permissions alone are not the complete boundary.
- Add a socket command only by changing both the client and server deliberately, validating required fields, returning a meaningful response, and testing the round trip. Do not silently change existing JSON field meanings.
- Configuration changes must persist through the existing config path and update the running backend through the control contract. Do not restart the service just to apply volume, pack, device, or other live settings.
- Statistics may contain aggregate pack counters only. Never add a key code, key name, timestamp per key, typed text, or raw request to stats or logs.
- Do not add telemetry, HTTP, analytics, remote update behavior, or a dependency that introduces network access without an explicit security and architecture decision.
- Keep service hardening intact. Changes to the generated unit must explain why each permission or sandboxing change is necessary and must be tested against the rendered unit.
- Do not broaden permissions as a convenience. Never recommend or implement running as root; prefer a narrow per-device udev rule over broad `input` group access.
- Keep audio sample playback confined to validated soundpack roots. Preview and control-socket paths must use the same confinement rules as normal activation.

## Soundpack and mapping rules

The supported input format is the Mechvibes `config.json` contract already implemented in `soundpack.rs` and `backend/mapping.rs`:

- mapping keys are iohook codes and are translated to evdev codes by the maintained table;
- malformed keys, unsafe paths, missing referenced files, and invalid pack structure fail with a useful error;
- `null` entries and optional `*-up` definitions retain their existing semantics;
- the End/PageDown correction and every remap entry are compatibility behavior, not incidental implementation details.

When changing a mapping entry or parser rule, update focused tests and `docs/adr/0005-mapping-contract.md` if the contract changes. Do not copy a mapping table from another project without checking the local contract and attribution requirements.

## Testing and validation

Tests should prove observable behavior and protect the boundary being changed. Prefer extending the existing focused suite over adding several overlapping regression tests.

- Unit tests live beside the implementation and cover pure mapping, parsing, validation, state transitions, service rendering, and backend command behavior.
- `tests/cli.rs` covers the executable's CLI behavior.
- `tests/perf.rs` contains explicit `#[ignore]` tests that need `/dev/uinput` and, for audio cases, a usable output device. Run them only when the environment supports them.
- For TUI changes, use `TestBackend` and, when visual layout matters, the existing `scripts/render-tui.sh` workflow. Store screenshots under `~/Pictures/screenshots/udu/<YYYY-MM-DD>/`, not in the repository root or home directory.
- For backend changes, test failure, reconnect, path confinement, and protocol behavior without requiring a physical keyboard whenever a focused test seam exists.

Before considering a change complete, run the applicable checks:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --all-targets --locked
```

CI also runs the declared MSRV build with Rust 1.88 and `cargo deny check` for advisories, licenses, bans, and sources. Run `cargo deny check` when changing dependencies, licenses, or supply-chain-sensitive code.

## Anti-patterns to reject

- Restarting or replacing the backend process for a setting that has a control command.
- Putting backend policy, socket calls, `systemctl`, or filesystem work inside a TUI draw function.
- Adding a generic `utils`, `helpers`, or catch-all module instead of naming the domain subject.
- Introducing a trait, abstraction, dependency, config flag, or compatibility layer for a single speculative use.
- Using `unwrap`, broad `catch`-style recovery, or silent fallbacks to hide an expected runtime failure.
- Cloning to defeat ownership errors without first restructuring the borrow or ownership boundary.
- Holding a mutex during disk/socket/process/audio I/O, or blocking the audio callback or capture loop.
- Bypassing path validation for preview, test-only-looking commands, or persisted selections.
- Logging raw input events, key codes, socket payloads, file contents, or per-key activity.
- Adding network access, telemetry, remote downloads, or broad filesystem/device permissions without an explicit decision.
- Treating `cargo test` as proof that hardware-dependent performance behavior is correct; run the ignored harness when the change affects latency, uinput capture, or audio playback.
- Updating only one side of a client/server protocol or changing persisted JSON semantics without tests and an ADR/issue when needed.
- Copying historical `wayvibes`/`klack` names into current code, service files, socket names, or new documentation.
- Adding comments that restate the code or large refactors justified only by personal preference.

## Contribution conventions

Use English for code, comments, tests, commits, documentation, and PR descriptions. Commit subjects use imperative mood. Sign commits off with the DCO trailer (`git commit -s`). Keep changes focused, explain why when the diff does not make it obvious, and include the checks you actually ran in the PR.
