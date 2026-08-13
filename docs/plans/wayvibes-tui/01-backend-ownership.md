---
type: decision
status: closed
blocked_by: []
---

# What owns the sound backend?

## Evidence
| Fact | Verified at |
|---|---|
| The external implementation is C++17 and loads mappings before entering a long-running input loop. | `/home/smaniotov/Documents/external/wayvibes/src/main.cpp:90-119`; `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:26-57` |
| The upstream CLI documents no live reload or control socket. | `/home/smaniotov/Documents/external/wayvibes/README.md:70-115`; inspected upstream source |
| A long-lived daemon with a separate local controller is a viable pattern, but the control API would be new project behavior. | Research brief: Linux input/libevdev, miniaudio, PipeWire, and systemd primary documentation |
| The external repository has no top-level license file in the inspected clone; soundpack directories contain individual license files. | `/home/smaniotov/Documents/external/wayvibes` file inventory |

## Choice
Keep the existing `wayvibes` backend as the sound process and make the new project a TUI/process manager around it. Configuration changes will be applied by stopping and starting `wayvibes` with updated command-line arguments or configuration files.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Incorporate the backend into the new project | It would create an unnecessary maintenance and attribution boundary when restarting the existing process meets the initial workflow. |
| Write a completely new backend | It would discard the existing Wayland-compatible input/audio implementation without a demonstrated need. |

## Consequence
The new project must manage the child process lifecycle, preserve compatibility with the upstream CLI and soundpack format, surface startup/runtime failures, and avoid claiming ownership of the backend. Live changes may briefly interrupt playback while the process restarts.
