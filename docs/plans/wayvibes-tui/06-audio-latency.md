---
type: decision
status: closed
blocked_by: [01]
---

# Which audio behavior should the managed process use?

## Evidence
| Fact | Verified at |
|---|---|
| The current backend uses a global miniaudio engine and starts a sound on each mapped key press. | `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:13-24,26-57` |
| Miniaudio documents Linux playback backends and high-level sound playback; PipeWire documents native stream creation and buffer negotiation. | Miniaudio manual and PipeWire stream documentation from the research brief |
| No latency or CPU benchmark has been run against the target machine/audio session. | Local validation status |

## Choice
Accept the external `wayvibes` audio behavior and miniaudio backend unchanged in the first release. The manager will only configure the existing volume option and will not add a separate audio boundary.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Replace miniaudio with native PipeWire playback | The project manages the existing backend and has no demonstrated audio defect requiring replacement. |
| Require a latency benchmark before the TUI | It would block the process-manager milestone without a current user-visible problem. |

## Consequence
The first release inherits the external process's audio behavior and limitations. Audio replacement or benchmarking can be proposed later if a concrete latency, routing, or burst-playback issue is demonstrated.
