---
type: decision
status: closed
blocked_by: [01]
---

# CLI/service surface cleanup

## Evidence
| Fact | Verified at |
|---|---|
| The manager config carries `wayvibes_path`, the CLI exposes `--wayvibes`, the service pkill-s `wayvibes` processes, and `process.rs` builds the wayvibes command line. All become dead once the backend is in-process. | `src/config.rs:14-33`, `src/cli.rs:20-24`, `src/service.rs:214-229`, `src/process.rs` |
| The installed unit name is `wayvibes-tui.service`; existing users have it installed. `SERVICE_NAME` is asserted by a test. | `src/service.rs:7`, `src/service.rs:272` |
| The README documents klack as "a local terminal interface for managing wayvibes" — the description itself changes with the new backend. | `README.md` |
| The unit is the singleton boundary: `Restart=on-failure`, `XDG_RUNTIME_DIR` flock, unmanaged-process termination. | `src/service.rs:145-193,196-204` |

## Choice
User chose to rename the unit to `klack.service`. The removal of `--wayvibes`/
`wayvibes_path`/`pkill`/`process.rs` and the `ExecStart` switch to in-process
`--service` mode are deterministic consequences of decisions 01 and 02.
`ensure_running` migrates: when the legacy `wayvibes-tui.service` exists, disable and
remove it, then install `klack.service`. Socket name follows the product: `klack.sock`
in `$XDG_RUNTIME_DIR`. README is rewritten around klack as its own backend, no
external dependency.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Keep `wayvibes-tui.service` | User chose the clean break (ask_user); leftover "wayvibes" identity contradicts the 100% autoral goal |

## Rejected alternatives
(only when closing)

## Consequence
Sets the CLI contract (dropped options), config schema migration (no `wayvibes_path`),
README rewrite, legacy-unit migration path, and which old tests die vs change
(unit-name assertions, pkill/wayvibes-command tests).