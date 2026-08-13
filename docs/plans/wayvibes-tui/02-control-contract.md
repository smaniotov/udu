---
type: decision
status: closed
blocked_by: [01]
---

# How does the TUI control the active sound service?

## Evidence
| Fact | Verified at |
|---|---|
| The current external process has no in-process command path and its event loop does not return during normal operation. | `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:26-57` |
| A separate TUI avoids competing for the keyboard event descriptor and can leave audio playback active while the UI exits. | Local architecture analysis; external research brief |
| Candidate mechanisms include a private Unix socket, file reload, or signals; a socket provides command responses and error reporting, while file/signal approaches need debounce and transactional validation. | External research brief citing systemd socket documentation |

## Choice
Use the systemd user service as the control boundary. The TUI persists the selected configuration and invokes `systemctl --user` to start, restart, stop, and query the singleton backend service. The service owns the external `wayvibes` process; the TUI never owns or directly waits on that child.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Private Unix socket to a first-party daemon | It would require implementing custom supervision and singleton recovery that systemd already provides on the supported environment. |
| TUI-owned child process | It would stop `wayvibes` when the TUI closes and would allow lifecycle ownership to be coupled to terminal state. |
| Configuration file plus `SIGHUP` | The upstream process does not document reload support, and it would not provide singleton supervision. |

## Consequence
The project must define a systemd unit, service installation/update behavior, configuration persistence before restart, systemctl error reporting, and backend status semantics. The TUI keeps changes in memory during a session and applies them once after the terminal is restored on exit; closing the TUI leaves the unit running.
