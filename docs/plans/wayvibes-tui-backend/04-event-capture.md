---
type: research
status: closed
blocked_by: []
---

# Event capture mechanics with the evdev crate

## Evidence
| Fact | Verified at |
|---|---|
| The repo already depends on `evdev 0.13.2` and uses it read-only for discovery (`Device::open`, `supported_keys`, `device.name()`). | `Cargo.toml`, `src/device.rs` |
| Reference capture loop: `open(O_RDONLY | O_NONBLOCK)`, no `EVIOCGRAB`, single 24-byte `input_event` reads, filter `EV_KEY && value == 1` only, 1 ms sleep on empty read, infinite loop, never reconnects, partial reads dropped. | `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:24-57` (engineering brief §2) |
| `Device::open(path)` is the only open API — **no `EventDevice` type exists** in 0.12/0.13 (that name is from python-evdev). `Device::from_fd`/`RawDevice::from_fd` exist for full open-flag control. | evdev 0.13.2 vendored source `src/sync_stream.rs:43-49` |
| `fetch_events()` **blocks by default** (single `read` syscall inside the call; the returned iterator is pure in-memory work, zero syscalls per `next()`); reads up to `EVENT_BATCH_SIZE = 32` events per syscall. | `sync_stream.rs:346-349`, `raw_stream.rs:431-453`, `lib.rs:220` |
| The fd is **NOT `O_NONBLOCK`** by default (`Device::_open` uses `OpenOptions` RDWR→RDONLY, no custom flags). `Device::set_nonblocking(bool)` exists (fcntl F_GETFL/F_SETFL); `RawDevice` has no such method. | `raw_stream.rs:99-108`, `sync_stream.rs:366-371` |
| `impl AsRawFd for Device`; crate docs sanction poll/epoll on the fd; shipped `examples/evtest_nonblocking.rs` is the template (set_nonblocking + epoll + WouldBlock handling). | `sync_stream.rs:433-436`, `lib.rs:146-147`, `examples/evtest_nonblocking.rs:22-43` |
| `InputEvent` accessors: `event_type() -> EventType`, `code() -> u16`, `value() -> i32`; filter = `event_type() == EventType::KEY && value() == 1`. `EventType::KEY = 0x01`. | `lib.rs:369-410`, `constants.rs:36-61` |
| `KeyCode(pub u16)` newtype with `const fn new(u16)` and `.code() -> u16` — a defines map keyed by u16 ("30") works directly: `KeyCode::new(30)`. | `scancodes.rs:10-22` |
| Device removal: `fetch_events()` returns `io::Error`; unplug = kind `Other` with `raw_os_error() == Some(libc::ENODEV)` (reliable match; `EIO` rarer alternate). | `raw_stream.rs:437-438`; libc 0.2.189 already a direct dep |
| Grab: `Device::grab()/ungrab()/is_grabbed()` are explicit; **no implicit grab anywhere** in open/from_fd; `Drop`-ungrab is a guarded no-op when never grabbed (0.13.1 changelog). | `sync_stream.rs:378-391`, `raw_stream.rs:645-667`, CHANGELOG.md:20 |
| Capability re-validation: `Device::supported_keys() -> Option<&AttributeSetRef<KeyCode>>` with `contains(KeyCode)`. | `sync_stream.rs:173-177`, `attribute_set.rs:44-47` |
| 0.12→0.13 read-path API unchanged in shape; relevant changes: `evdev::Error` removed (plain `io::Error`), `InputEventKind`→`EventSummary`, `Key`→`KeyCode`. | CHANGELOG.md:24-53 |

## Conclusion
The full capture contract is implementable with evdev 0.13.2 (already pinned):
`Device::open` + `set_nonblocking(true)`, idle tick via `libc::poll` (timeout 1 ms) on
`as_raw_fd()`, one `fetch_events()` per poll-readiness, filter
`event_type() == EventType::KEY && value() == 1` (drop release/autorepeat), never call
`grab()`, ENODEV detected via `raw_os_error() == Some(libc::ENODEV)` (also match EIO).
`WouldBlock` = idle, never busy-poll — strictly better tick budget than the C++ 1 ms
sleep (reads batch up to 32 events).

Implementation flags: (1) set nonblocking explicitly — the crate opens blocking;
(2) the sync `Device` holds events until `SYN_REPORT` (fine for keyboards); if the
literal C++ "whatever is readable now" semantics are wanted, `RawDevice` +
`custom_flags(O_NONBLOCK)` at open is the exact match; (3) batch reads mean a burst
drains in one syscall — feeds 06 and 07.