---
type: decision
status: closed
blocked_by: [01]
---

# Control contract: real-time control channel, no restart on change

## Evidence
| Fact | Verified at |
|---|---|
| User motivation, verbatim intent: "o fluxo de ficar subindo e matando o processo do wayvibes é extremamente custoso e lento... não temos nenhum tipo de atualização em tempo real do processo... algo completamente nosso, que consiga ser atualizado e dando a possibilidade do TUI atualizar as informações do nosso backend em tempo real (ou o mais rápido possível) sempre que o usuário apertar enter em alguma opção." | Session instruction (user message) |
| Today the TUI buffers changes and applies them once after exit; `apply` saves config, reinstalls the unit, stops managed/unmanaged processes, and restarts the service. | `src/service.rs:34-42`, `src/app.rs:240-252` |
| The old plan rejected a private Unix socket only because the daemon was an external process without a control contract — the reason disappears with a first-party backend. | `docs/plans/wayvibes-tui/02-control-contract.md` (Rejected alternatives) |
| The backend is now in-process behind `--service` (see 01): same binary owns capture + audio + socket. | [[01-backend-boundary]] |
| systemd keeps providing singleton supervision; `Restart=on-failure` remains for crashes, not for config changes. | `src/service.rs:196-204` |
| The manager already requires `XDG_RUNTIME_DIR` (ServiceLock), the natural per-user home for a control socket. | `src/service.rs:145-160` |

## Choice
The service process owns a **Unix domain socket** in `$XDG_RUNTIME_DIR` (e.g.
`klack.sock`). The TUI sends commands — select soundpack, set volume, select device,
query status — and the backend applies them **immediately on Enter**, replying with
state. Configuration changes no longer restart the process and no longer wait for TUI
exit. The persisted `config.json` remains the source of truth for cold starts and
restarts (written through on each change). Socket bind ownership supersedes the flock
as the singleton guard (audit in implementation; keep flock semantics if simpler).

## Rejected alternatives
| Alternative | Why not |
|---|---|
| SIGHUP + config-file reload | No request/response, no status readback, racy (write then signal); cannot answer "status em tempo real" |
| Signals only | Cannot carry payloads (pack path, volume) or reply state |
| Keep stop/start on change (status quo) | Exactly the "extremamente custoso e lento" flow the user wants eliminated |

## Consequence
The TUI semantics change from "pending until exit" to **apply-on-Enter with live
status**: `has_pending_changes`/`commit_pending_changes` flow in `app.rs` and
`WayvibesService` are replaced by a control-channel client; status lines can reflect
backend state live; `service.rs` `apply`/pkill paths die with the external-binary
cleanup (08). The unit keeps `Restart=on-failure` only as crash recovery. New tests:
socket protocol (parse/respond), immediate-apply behavior, singleton via bind.