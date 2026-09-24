# ADR-0002: Real-time control channel over a per-user Unix domain socket

Status: accepted · 2026-08-09 · Amended 2026-09-24 for live tone controls · Supersedes the restart-on-change control model of `docs/plans/wayvibes-tui/02-control-contract.md`

## Context
Changing pack/volume/device required stopping and starting the external binary,
interrupting playback. The user wants the TUI to update the backend in real time on
every Enter ("atualizar as informações do nosso backend em tempo real (ou o mais
rápido possível) sempre que o usuário apertar enter").

## Decision
The service process owns a Unix stream socket at `$XDG_RUNTIME_DIR/udu.sock`. The
TUI sends JSON-lines commands for soundpack, volume, device, spatial tone controls,
and spectral tone controls (`set_tone_bass_gain` and `set_tone_treble_gain`). The
backend applies each command immediately and replies with the current state,
including both EQ gains. Configuration changes never restart the process. The
persisted `config.json` is written through on each change and remains the source of
truth for cold starts. Socket bind ownership is the singleton guard (ServiceLock
flock audited against it in implementation).

## Consequences
- The TUI's "pending until exit" semantics are replaced by apply-on-Enter with live
  status; moving the tone pad sends both gain changes through the same path.
- Spatial tone controls remain separate from the spectral EQ controls.
- Signals-only or file-reload mechanisms were rejected: no request/response, no
  status readback, racy.
- A backend crash still restarts via systemd and rebinds the socket.