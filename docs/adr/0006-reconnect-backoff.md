# ADR-0006: Reconnect-with-backoff device recovery (no restart, no dead-loop)

Status: accepted · 2026-08-09

## Context
wayvibes opens one fd for process lifetime and, on device removal, silently sleeps in
a 1 ms loop forever (never exits, never reconnects). The control socket (ADR-0002)
means a process restart would drop the socket and state for the RestartSec window.
The user chose in-process recovery.

## Decision
On ENODEV (`raw_os_error()`), the capture loop enters a reconnect loop: re-resolve the
persisted device name and reopen with exponential backoff (500 ms → ~10 s cap),
keeping the socket, audio engine, and loaded mapping alive. Playback resumes as soon
as the device returns. `Restart=on-failure` remains crash-only. No grab (keyboard
stays usable by the system); exact first-match name resolution preserved.

## Consequences
- Unplugging/replugging the keyboard no longer kills sound or the control channel.
- Divergence from wayvibes (dead-loop vs silent wait-and-resume) is documented.
- Graceful-exit-to-systemd and hybrid backoff-then-exit variants were rejected
  (socket drop and restart gap).