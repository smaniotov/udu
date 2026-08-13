---
type: decision
status: closed
blocked_by: []
---

# Tolerate unreadable input nodes during device discovery

## Evidence

| Fact | Verified at |
|---|---|
| `discover_keyboards` propagates the first `Device::open`/read error via `collect::<Result<Vec<_>,_>>()?` — one `EACCES` node fails the whole scan. | local `src/device.rs` |
| The UX criteria state devices are listed read-only and `/dev/input` access is a documented prerequisite, never root. | `docs/ux-criteria.md`; plan note `05-device-permissions` |
| Desktops typically expose several event nodes (mouse, touchpad, …) beyond the keyboard. | systems with `/dev/input/event*` (inferred) |

## Choice

Skip devices that fail to open/classify (annotating the error in the status line), keep
listing readable ones. `is_active`/per-device errors no longer abort the whole scan.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Keep fail-fast | One unreadable node hides every discoverable keyboard — violates the UX criteria |
| Require root | Explicitly out of scope per `05-device-permissions` |

## Consequence

Discovery becomes fault-tolerant: the app shows whatever is readable and reports why the rest
is missing. Error annotation stays in `DeviceError` variants.