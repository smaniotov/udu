---
type: decision
status: closed
blocked_by: [04]
---

# Device resilience: wayvibes parity vs first-party improvements

## Evidence
| Fact | Verified at |
|---|---|
| wayvibes opens one fd for process lifetime, never re-enumerates, and on device removal enters a silent infinite 1 ms sleep loop (never exits, never reconnects). No grab: the keyboard stays usable by the system. | `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:27,51-56`; brief §1, §2 |
| `--device-name` matching is exact, case-sensitive, first match wins; duplicate names make first-match ambiguous. Manager persists the exact name. | `/home/smaniotov/Documents/external/wayvibes/src/device.cpp:124-132`; `src/app.rs:173-183` |
| The systemd unit already provides `Restart=on-failure` — a graceful exit on ENODEV would restart the unit instead of dead-looping. | `src/service.rs:235-244` |
| "100% autoral" (session goal) argues for owning the failure behavior; exact parity argues for keeping observed wayvibes behavior. | Session instruction |

## Choice
On ENODEV (detected via `raw_os_error() == Some(libc::ENODEV)`, also match EIO), the
capture loop enters a reconnect loop: re-resolve the persisted device name and reopen
with backoff (e.g. 500 ms, 1 s, 2 s … capped ~10 s), keeping the socket, audio engine
and loaded mapping alive; playback resumes as soon as the device returns. The loop
logs each attempt and keeps waiting indefinitely — no restart, no socket drop,
no dead-loop. (User decision, ask_user, option "Reconnect com backoff in-process".)

Kept from wayvibes parity: no grab, exact first-match name resolution.
(User decision via ask_user: "Reconnect com backoff in-process".)

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Graceful exit(1) → systemd restart | Drops `klack.sock` and state for ~2 s+ (RestartSec); contradicts real-time control (02) |
| Hybrid: short backoff then exit | Reintroduces the restart gap for a case (long absence) that logging already covers in-process |

The resolution also feeds the unit contract: `Restart=on-failure` stays as crash
recovery only; a self-recovering backend means the unit restarts rarely.

## Consequence
Sets the capture loop's disconnect handling and its tests (ENODEV detection, backoff
schedule, device-name re-resolution). Observed divergence from wayvibes: unplug no
longer dead-loops — the backend silently waits and resumes — and the socket stays
addressable throughout. `Restart=on-failure` remains crash-only.