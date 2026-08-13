---
type: decision
status: closed
blocked_by: []
---

# Which keyboard input boundary should the managed backend use?

## Evidence
| Fact | Verified at |
|---|---|
| Wayvibes identifies keyboard-capable `/dev/input/event*` devices with libevdev and reads Linux `input_event` records directly. | `/home/smaniotov/Documents/external/wayvibes/src/device.cpp:18-55`; `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:26-57` |
| Cherrybuckle has a separate libinput scanner that dispatches keyboard events through a seat and polls the libinput file descriptor. | `/home/smaniotov/Documents/personal/cherrybuckle/scan-libinput.c:13-118` |
| Direct evdev is compositor-independent but requires device permissions and careful device selection; permission behavior varies by distribution/session. | External research brief citing Linux input and libevdev documentation |

## Choice
Keep the direct evdev/libevdev input boundary used by `wayvibes`, and add a read-only device-discovery view in the manager. The manager may enumerate keyboard devices and persist the selected exact device name, but it will not open a device for event capture or add a parallel libinput backend. `wayvibes` remains the only process that consumes keyboard events.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Move the backend to libinput | The new project does not own the input loop, and changing the external backend would exceed the process-manager scope. |
| Support both event-capture backends | It would add configuration and testing surface without a current requirement from the selected runtime architecture. |
| Omit device discovery and require only `--device-name` | It would leave the planned device-inspection workflow unavailable in the TUI. |

## Consequence
The manager needs a read-only evdev enumeration boundary and must report permission failures without capturing events. The first release still inherits `wayvibes` device-selection and hotplug limitations; the new discovery list is advisory and is refreshed on demand.
