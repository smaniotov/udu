---
type: research
status: closed
blocked_by: [03, 04]
---

# Performance budget and measurement

## Evidence
| Fact | Verified at |
| **Measured 2026-08-09 (tests/perf.rs, uinput virtual device)**: dispatch p95=37 us (mean 33 us, 100 presses), zero drops at 30/s sustained (60/60) and at a 100/s burst (120/120), idle tick 1.06 ms (poll-driven), mapping lookup p95=151 ns (200 distinct codes cycled 200k times), decode mean 17 us incl. warmed calls, hot mean 1.6 us (11x via the decode cache; true first-decode of the fixture is ~150 us). | `cargo test --test perf -- --ignored` run on the target machine |
|---|---|
| wayvibes latency floor is ≈1 ms idle tick (single read, `usleep(1000)` on empty) + miniaudio default-buffer latency + in-memory decode startup for a file's first-ever press. | `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:51-53`; brief §2, §4 |
| evdev 0.13.2 reads batch up to 32 events per syscall and its iterator is syscall-free — a burst drains in one read, no 1 ms tick between events. | [[04-event-capture]] (`lib.rs:220`, `sync_stream.rs:570-602`) |
| cpal allows `BufferSize::Fixed(128..512)` (a request — ALSA may round); expected event→audio ≈ 10-25 ms total with 256-512 frame periods; default was 512 frames post-fix. | [[03-audio-engine]]; cpal issue #1029 |
| Decode caching is ours: `Vec<f32>` per file, Arc-shared, spawn buffer-backed voices per press; 2084 wav + 171 mp3 candidates from bundled packs; mp3 decode µs-ms per file at load. | [[03-audio-engine]] §5; local scan |
| Voice pool: pre-allocated `Vec<Voice>` + lock-free handoff (crossbeam/rtrb) from the evdev thread to the audio callback; zero allocation in the audio thread. | [[03-audio-engine]] §5; design |
| The capture thread must never block on decode or audio — decode off-thread (lazy loader), dispatch via ring buffer. | [[03-audio-engine]] §5 |

## Conclusion
Targets (benchmarked in the execution phase with a synthetic `uinput`/user-space event
feed; numbers recorded in the phase acceptance, not here):
1. **Dispatch latency**: capture thread event→voice-spawn handoff ≤ 1 ms p95 at idle;
   end-to-end event→audio ≤ 25 ms perceived-instant (cpal 256-frame period default).
2. **Drop rate**: zero missed presses at sustained 30 keys/s bursts (and ≤ 1 dropped
   at a pathological 100/s rollover for 1 s).
3. **CPU**: backend stays below one full core; idle tick is poll-sleep (0% busy loop).
4. **Decode cache**: per-file `Arc<Vec<f32>>`, loaded lazily off-thread on first press,
   reused across presses (wayvibes re-decodes per press — this is the measurable win);
   cache eviction keeps resident bytes bounded (LRU if a pack's total exceeds a cap
   defined in the phase).
5. **Benchmark harness**: deterministic synthetic feed (uinput device or loopback
   `input_event` pipe) measuring dispatch latency and drop rate; plus one real-device
   smoke measurement with the target keyboard.