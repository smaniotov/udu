---
type: decision
status: closed
blocked_by: [01]
---

# Error model: thiserror 2 for domain enums, anyhow at the app boundary

## Evidence

| Fact | Verified at |
|---|---|
| thiserror derive produces a hand-written `std::error::Error` implementation; converting is explicitly *not* a breaking change; messages can be copied verbatim. | thiserror README v2.0.19/2.0.20, 61k dependents |
| Canonical split: libraries define typed error enums; the application boundary converts to `anyhow::Error` with `.context()` for actionable top-level messages. | BurntSushi "Error Handling in Rust"; Google Comprehensive Rust; anyhow docs |
| Rust API Guidelines C-GOOD-ERR: `Error` + `Send + Sync`, `Display` lowercase without trailing punctuation — the current manual impls already comply. | API Guidelines |
| 5 enums with manual `Display`+`Error`+`From` ≈ 140–150 lines of boilerplate; `Box<dyn Error>` crawls into `app.rs`/`ui.rs`/`main.rs`. | local source read (5 modules) |

## Choice

Convert `CliError`(then removed in phase 2 — thiserror applies to the remaining
`ConfigError`, `ServiceError`, `SoundpackError`, `DeviceError`), `#[derive(Error, Debug)]`
with `#[error("…")]` messages **copied verbatim**; `#[source]`/`#[from]` replace the manual
`From` impl (e.g., `ServiceError: From<ConfigError>`). Replace `Box<dyn Error>` at the app
boundary (`main.rs`, `app.rs`, `ui.rs` handlers) with `anyhow::Result` + `.context()`.
Domain modules keep typed enums only.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Keep manual `Display`+`Error` impls | ~150 lines of boilerplate; no behavior gain |
| anyhow everywhere | Erases exhaustive matches and `#[from]` chaining in domain logic |
| `Box<dyn Error>` everywhere | Throwaway-code style; loses typed handling and `source()` chains |

## Consequence

User-facing error text unchanged (tests assert samples). Errors stay `Send + Sync`. The app
boundary gains actionable messages. Deps: `thiserror = "2"`, `anyhow = "1"`.