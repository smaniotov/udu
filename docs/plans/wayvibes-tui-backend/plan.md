# Replace the external wayvibes executable with a first-party Rust backend

## Destination
This repository ships a first-party `klack` backend: a Rust process that captures
Linux keyboard events from the local input subsystem and plays Mechvibes-compatible
soundpack sounds in-process, replacing the external `wayvibes` binary. The systemd
user service (`klack.service`, singleton, `Restart=on-failure` as crash recovery only)
owns the backend; the TUI configures it in real time through a Unix control socket —
no process restart on configuration change. The TUI/manager, device discovery,
soundpack validation, volume range, and existing tests are preserved; every CLI
option, config field, and code path that referenced the external binary is removed.

## Notes
- Stack: Rust 2024, ratatui 0.30.0 (crossterm via ratatui), evdev 0.13.2, serde 1;
  audio engine settled by [[03-audio-engine]] (cpal 0.18 + symphonia 0.6 direct, rodio
  0.22 alternative — final crate pick is a user decision, pending). Gates:
  `cargo fmt -- --check` · `cargo test` · `cargo clippy --all-targets --all-features
  -- -D warnings` · `cargo build --release`.
- Reference behavior source: `/home/smaniotov/Documents/external/wayvibes` @ `0c94f0c`
  (C++17, miniaudio 0.11.21 vendored, raw `input_event` reads). Engineering brief with
  `file:line` citations produced during charting (evidence in notes 01-10).
- This plan supersedes and reopens `docs/plans/wayvibes-tui/plan.md` decisions 01
  (backend ownership) and 06 (audio latency). It keeps 02 (systemd control boundary —
  now as crash recovery only), 03 (Rust stack), 04 (evdev input boundary), 05 (device
  permissions), 07 (soundpack model), 08 (service lifecycle).
- User-required constraint: the soundpack model must stay compatible with the
  Mechvibes-format `config.json` (`defines` object). Pack keys are **iohook
  keycodes**, not raw evdev codes. The **local** wayvibes `0c94f0c` does **no**
  remap (nav/arrows/F13-15/6xxxx are silent today); upstream main added
  `remapMechToLinuxKey` (config.cpp:15-86). Our mapper = the upstream table, corrected
  per [[10-remap-fidelity]].
- Local soundpack inventory (verified during charting): 20 bundled packs, all with
  `config.json`; referenced audio extensions `.wav` (2084) and `.mp3` (171); defines
  keys decimal `1`..`61011` (iohook codes incl. arrows/nav/6xxxx that wayvibes remaps
  to evdev); 30 `null` values; all packs are v1 `multi`-shape.
- Skills in play: `research` (crate selection, versioned docs), `rust`, `increments`
  (surgical phases), `tdd`/`test-triage` (test discipline), `code-style` (form),
  `planning` (this map).

## Decisions taken
- [[00-ownership-scope]] — "100% autoral" means base parity + explicit improvements (live reload, device recovery, decode cache, measured budget); mapping contract stays Mechvibes-exact.
- [[01-backend-boundary]] — in-process backend behind the existing `--service` mode of the same binary; no separate daemon binary.
- [[02-control-contract]] — real-time control via a per-user Unix domain socket; the TUI applies changes on Enter with status replies; no restart on config change.
- [[03-audio-engine]] — audio stack confirmed by user: cpal 0.18.1 + symphonia 0.6.0 direct (voice pool + one master gain, best latency control); rodio 0.22.2 and miniaudio-FFI rejected.
- [[04-event-capture]] — evdev 0.13.2 implements the full capture contract (nonblocking + poll tick, EV_KEY value==1 filter, no grab, ENODEV via raw_os_error); batch reads up to 32 events/syscall.
- [[05-key-mapping-fidelity]] — strict pack validation (u16, no traversal, files exist) + faithful runtime mapper; byte-compat leniency rejected.
- [[06-device-resilience]] — reconnect-with-backoff in-process on ENODEV (re-resolve device name, restart the loop); no restart, no dead-loop; `Restart=on-failure` stays crash-only.
- [[08-cli-service-cleanup]] — unit renamed to `klack.service` with legacy-unit migration; `--wayvibes`/`wayvibes_path`/pkill/`process.rs` removed; socket `klack.sock`.
- [[09-soundpack-schema]] — defines-only parity is correct for the bundled v1 packs; validator rules: fatal = non-string value / `*-up` / non-numeric key; warn = version≠1 / key_define_type ∉ {multi,single}; ignore = proven-inert metadata; `default` is not a fallback feature.
- [[10-remap-fidelity]] — corrected iohook→evdev remap: wayvibes table with End/PgDn swap fixed (empirical check first) and RightShift 54→62 mapped; divergence documented.
- [[07-performance-budget]] — targets set (dispatch ≤1 ms p95, event→audio ≤25 ms, zero drops at 30 keys/s, CPU <1 core); measurement is execution-phase work.

## Fog
(empty — map complete; all decision and research items closed)

## Out of scope
- Replacing or editing the external wayvibes clone itself.
- Graphical UI, mouse/touchpad sounds, compositor-specific global input APIs.
- Wayvibes-parity interactive `--device` prompt in the backend; device selection keeps
  living in the TUI manager.
- Changing the Mechvibes pack format or migrating bundled `sounds/` assets.
- Packaging, distribution, or portable release concerns.
- Editing `sounds/` assets (kept out of git by convention).
## Phases (execution shape — one phase = one transform, one commit, one review)

| # | Phase | Files | Contracts / behavior to cover | Acceptance (oracle + gate) |
|---|---|---|---|---|
| 1 | Mapping module | `src/backend/mod.rs` (new), `src/backend/mapping.rs` (new), `src/soundpack.rs` (expose parsed defines), `src/lib.rs` (add `backend`), optional `examples/probe_events.rs` | `pub fn load(pack_path: &Path) -> Result<Mapping, MappingError>` (validates via `soundpack` rules: fatal = non-string value / `*-up` / non-numeric key; warn = version≠1 / key_define_type ∉ {multi,single}); free `iohook_to_evdev(u16) -> u16` with the full upstream table (config.cpp:15-86) **corrected on the End/PgDn pair only**: 3663→KEY_END, 3665→KEY_PAGEDOWN (after the empirical probe); RightShift 54 is base-block pass-through (no remap, no fix); `Mapping::lookup(evdev_code: u16) -> Option<&Path>`; null-skip; >767 unmapped never match. **Task first**: empirical probe — run `examples/probe_events.rs` on the target keyboard, record End/PgDn codes into test fixtures and note 10 | Oracle: a fixture pack — key 57416→KEY_UP sound; 30→`wasd.wav`; 3663→KEY_END sound, 3665→KEY_PAGEDOWN sound (corrected); 54→KEY_RIGHTSHIFT sounds (pass-through); unmapped code silent; `*-up` pack rejected with clear message. Gate: `cargo fmt -- --check` · `cargo test` · `cargo clippy --all-targets --all-features -- -D warnings` · `cargo build --release` |
| 2 | Capture module | `src/backend/capture.rs` (new), `src/device.rs` (reuse resolution) | `Capture::open(device_name: &str) -> Result<Self, CaptureError>` (exact-name resolution, first match — device.rs semantics); `next_keypress(&mut self) -> Result<Option<KeyPress>, CaptureError>` with `KeyPress { code: u16 }`; nonblocking fd + `libc::poll` 1 ms idle tick; filter `event_type()==KEY && value()==1` (release/autorepeat dropped); batch reads (32/syscall); `Err(CaptureError::DeviceGone)` via `raw_os_error()==ENODEV` (also EIO); `reconnect(&mut self)` re-resolves and reopens; no grab | Oracle: injected-event tests — press plays, release/autorepeat dropped, WouldBlock = idle (no busy loop), ENODEV surfaces DeviceGone; probe runs on a real device manually. Gate: same as P1 |
| 3 | Audio engine | `Cargo.toml` (+ `cpal = "0.18.1"`, `symphonia = "0.6.0"`, `symphonia-bundle-wav`, `symphonia-bundle-mp3`), `src/backend/audio.rs` (new), `tests/fixtures/` (wav + mp3 samples) | `Audio::new(default_volume: f32) -> Result<Self, AudioError>` (cpal default output device, f32, `BufferSize::Fixed(256)` fallback default, native rate/channels); `play(&self, path: &Path)` fire-and-forget from a cached decoded `Arc<Vec<f32>>`, lazy decode off the audio thread, voice pool with recycling and no hard cap, zero allocation in the audio callback; `set_master_volume(f32)` — single gain multiply, clamped 0.0–10.0; missing file → `eprintln!` per press, continue | Oracle: fixture decode wav+mp3 → f32 buffers; cache hit does not re-decode (counter); master-gain math across range; overlapped voices both audible; invalid file does not panic. Gate: same |
| 4 | Backend service + control socket | `src/backend/control.rs` (new), `src/backend/mod.rs` (orchestration: threads, startup order, reconnect loop with backoff 500 ms…10 s), `src/service.rs` (`run_service` → `backend::run`; drop process spawn), `src/process.rs` (deleted), `src/config.rs` (drop `wayvibes_path`) | Unix stream socket `$XDG_RUNTIME_DIR/klack.sock`; JSON-lines protocol: req `{"cmd":"set_soundpack","path":…}` / `{"cmd":"set_volume","value":…}` / `{"cmd":"set_device","name":…}` / `{"cmd":"status"}` → resp `{"ok":true,…}` or `{"ok":false,"error":…}`; commands apply immediately and write through to `config.json`; socket bind = singleton (replaces ServiceLock flock — audit, keep flock if simpler); startup order: config → audio → mapping → capture → socket → loop; DeviceGone → reconnect loop keeps socket/mapping/audio alive | Oracle: protocol unit tests (parse, reply, unknown cmd, error reply); write-through persists after command; second process fails to bind (singleton); status returns live state. Gate: same |
| 5 | TUI real-time client | `src/control.rs` (client; or `src/backend/client.rs`), `src/app.rs`, `src/ui.rs`, `src/main.rs` | `ControlClient::connect()`, `set_soundpack(path)`, `set_volume(v)`, `set_device(name)`, `status() -> BackendStatus`; App: replace `WayvibesService`/`has_pending_changes` with immediate apply-on-Enter + live status line + volume gauge from replies; connection failure → actionable error (service not running); help-modal text updated; closing TUI leaves backend running | Oracle: client tests against an in-test socket server; app tests adapted (expected full state after Enter, not pending); manual ux-criteria walk (Tab/Enter/+/-/r/? flow with immediate audible change). Gate: same |
| 6 | Service unit + CLI/config cleanup + migration | `src/service.rs` (`SERVICE_NAME` → `klack.service`; legacy `wayvibes-tui.service` migration: disable + remove unit file; drop pkill/`stop_unmanaged_wayvibes`), `src/cli.rs` (drop `--wayvibes`), `src/config.rs` (schema without `wayvibes_path`), `src/main.rs`, `tests/cli.rs`, `README.md` | Render unit `ExecStart={bin} --service --config {path}`; `Restart=on-failure` crash-only; migration helper only touches the legacy unit when present; CLI surface without `--wayvibes`; README rewritten (klack owns its backend; divergence notes: corrected remap vs local/upstream; no external dependency) | Oracle: unit/render + quote + migration-order tests (no real systemctl in unit tests); cli parse tests updated; manual systemd smoke — installed `klack.service`, one backend process, `is-active` after TUI close, legacy unit removed. Gate: same |
| 7 | Performance benchmark | `benches/` or `tests/perf.rs` (new), `examples/` synthetic feed (uinput or loopback input_event pipe) | Harness measures: dispatch latency (event→voice spawn, p95), drop rate at 30 keys/s sustained and at a 100/s 1 s rollover, idle CPU (<1 full core), decode-cache hit path vs cold (wayvibes re-decodes per press — ours must beat it); real-device smoke measurement with the target keyboard | Oracle: targets from [[07-performance-budget]]: dispatch ≤1 ms p95; event→audio ≤25 ms; zero drops at 30/s; ≤1 dropped in the 100/s burst; idle ≈ 0% busy loop. Numbers recorded in the phase commit. Gate: same |

## Rules that govern execution

- Gates run per phase (P1 gate list above; identical for all). A phase is done only when its oracle AND gates pass.
- One GPG-signed commit per phase, English message; `sounds/` assets and `target/` excluded (gitignore).
- Phase review: an isolated reviewer agent (fresh context) approves each phase against this plan before the next starts. Reject → fix → re-review.
- Before implementing a phase, **open every decision note it touches** (zoom — no execution from one-line summaries).
- First phase task: the empirical End/PgDn probe (run `examples/probe_events.rs`, record codes); until then the corrected End/PgDn entries stay flagged in `iohook_to_evdev` tests. RightShift needs no probe (base block).
- Architecture skill applies: new `backend` module layout is the boundary chosen in [[01-backend-boundary]]; do not cross into TUI/UI or systemd responsibilities from backend modules (and vice versa).
- Anything not named here is decided with the file open, following `/skill:code-style`, `/skill:increments`, and the `tdd`/`test-triage` discipline (test behavior, not implementation).
