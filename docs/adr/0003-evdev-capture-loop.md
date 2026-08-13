# ADR-0003: evdev 0.13.2 capture loop (no grab, poll-driven, ENODEV-aware)

Status: accepted · 2026-08-09

## Context
wayvibes reads `/dev/input/event*` with raw syscalls: `O_RDONLY|O_NONBLOCK`, single
24-byte reads, `usleep(1000)` idle, `EV_KEY && value==1` filter, no grab, silent
dead-loop on ENODEV. The first-party backend needs the same observable capture
contract with a better idle/tick budget and removable-device handling.

## Decision
Use the already-pinned `evdev 0.13.2` crate: `Device::open` + `set_nonblocking(true)`,
1 ms idle tick via `libc::poll(2)` on `as_raw_fd()`, one `fetch_events()` per
poll-readiness (batches up to 32 events/syscall), filter
`event_type()==EventType::KEY && value()==1` (release/autorepeat dropped), never call
`grab()`, ENODEV detected as `raw_os_error()==Some(ENODEV)` (also EIO).

## Consequences
- The fd is not O_NONBLOCK by default — the backend sets it explicitly.
- The sync `Device` holds events until `SYN_REPORT` (fine for keyboards).
- `WouldBlock` is the idle signal — no busy loop; strictly better tick budget than the
  C++ 1 ms sleep.