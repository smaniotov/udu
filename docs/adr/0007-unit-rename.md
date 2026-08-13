# ADR-0007: systemd unit renamed to klack.service with legacy migration

Status: accepted · 2026-08-09

## Context
The managed unit was `wayvibes-tui.service`, a name tied to the external dependency it
managed. With the first-party backend (ADR-0001) the "wayvibes" identity is a
leftover; the user chose a clean break (ask_user).

## Decision
The unit is renamed to `klack.service` (`ExecStart={bin} --service --config {path}`,
`Restart=on-failure` crash-only, `RestartSec=2s`). `ensure_running` migrates an
installed legacy `wayvibes-tui.service` when present (disable + remove unit file,
then enable/start the new unit). Unmanaged external process termination (`pkill
wayvibes`) is removed — there is no external process anymore.

## Consequences
- Installed setups migrate once; the legacy unit is not left running alongside.
- README and CLI surface drop all `--wayvibes` references; the config schema drops
  `wayvibes_path`.
- The singleton guarantee moves to socket bind ownership (ADR-0002), audited against
  the old flock.