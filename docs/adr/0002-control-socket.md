# ADR-0002: Real-time control channel over a per-user Unix domain socket

Status: accepted · 2026-08-09 · Supersedes the restart-on-change control model of `docs/plans/wayvibes-tui/02-control-contract.md`

## Context
Changing pack/volume/device required stopping and starting the external binary,
interrupting playback. The user wants the TUI to update the backend in real time on
every Enter ("atualizar as informações do nosso backend em tempo real (ou o mais
rápido possível) sempre que o usuário apertar enter").

## Decision
The service process owns a Unix stream socket at `$XDG_RUNTIME_DIR/klack.sock`. The
TUI sends JSON-lines commands (`set_soundpack`, `set_volume`, `set_device`, `status`)
and applies them immediately; the backend replies with state. Configuration changes
never restart the process. The persisted `config.json` is written through on each
change and remains the source of truth for cold starts. Socket bind ownership is the
singleton guard (ServiceLock flock audited against it in implementation).

## Consequences
- The TUI's "pending until exit" semantics are replaced by apply-on-Enter with live
  status.
- Signals-only or file-reload mechanisms were rejected: no request/response, no
  status readback, racy.
- A backend crash still restarts via systemd and rebinds the socket.