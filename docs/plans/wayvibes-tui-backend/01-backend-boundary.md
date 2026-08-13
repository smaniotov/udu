---
type: decision
status: closed
blocked_by: []
---

# In-process backend shape: same binary vs separate backend

## Evidence
| Fact | Verified at |
|---|---|
| `klack` already has a `--service` mode that the systemd unit runs (`ExecStart=klack --service --config ...`); today it spawns the external `wayvibes` child and waits. | `src/main.rs:31-32`, `src/service.rs:114-143` |
| The systemd unit identity provides singleton + `Restart=on-failure` supervision; the manager never owns the backend child directly. | `src/service.rs:196-204`, `docs/plans/wayvibes-tui/08-service-lifecycle.md` |
| Wayvibes' own binary is a single long-lived process: open fd, load map once, infinite loop, never exits on its own. | `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:41-54`, `main.cpp:116` |
| The capture loop and the audio callback both block in their own threads; a first-party loop needs its own thread + audio engine, not the TUI event loop. | Engineering brief §2, §4 (miniaudio worker threads) |
| User picked "Mesmo binário, modo --service (recomendado)" in ask_user (options: same-binary service vs separate klackd). | ask_user session record |

## Choice
The `--service` mode stays the systemd unit entrypoint but runs the capture + audio
loop in-process instead of spawning the external binary. `wayvibes_tui` gains backend
modules (capture, mapping, audio, control channel) reusing `ServiceLock`, config
loading, and the current flow. No new binary, no new install surface.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Separate `klackd` binary | Adds a crate, install point, and coordination surface the user did not want; the TUI-orchestration already distinguishes modes via `--service` |

## Consequence
Sets the crate layout (backend modules inside `wayvibes_tui`, invoked from `--service`
mode), the unit `ExecStart` (unchanged shape), and unblocks 02 (control contract) and
08 (CLI cleanup).