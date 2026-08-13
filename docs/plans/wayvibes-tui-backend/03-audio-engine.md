---
type: research
status: closed
blocked_by: []
---

# Rust audio engine for the first-party backend

## Evidence
| Fact | Verified at |
|---|---|
| Runtime requirement: 20 bundled packs reference `.wav` (2084 entries) and `.mp3` (171 entries) — the engine must decode both. | Local scan of `sounds/*/config.json` (charting session) |
| Reference engine: miniaudio 0.11.21 vendored; `ma_engine_init(NULL)` defaults — float32, native channels/sample rate, default period size, no exclusive mode. | `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:12`, miniaudio.h `ma_engine_init` internals |
| Playback contract to replicate: per press, fire-and-forget play of a fully-decoded-in-memory sound, overlapping instances allowed, recycled when finished, missing file → stderr per press and continue. | Engineering brief §4; `/home/smaniotov/Documents/external/wayvibes/src/audio.cpp:16-20` |
| Volume is applied as engine-global gain (values 1.0–10.0 boost up to 10×), never per-sound, never from config. | Engineering brief §4/§5; `audio.cpp:22`, `main.cpp:84` |
| `miniaudio` (ExPixel bindings) is **archived at 0.10.0** (2020, last push 2024-03-16); not 0.11 parity. | crates.io/crates/miniaudio; github.com/ExPixel/miniaudio-rs |
| maudio 0.1.8 (2026-07, ~1k downloads, 1 maintainer) and miniaudio_aurex 0.11.26 (niche, 1 dependent) are the only maintained bindings — both too young/niche to build on. | crates.io/crates/maudio, crates.io/crates/miniaudio_aurex |
| cpal 0.18.1 (2026-06-07) very active; 0.18 added native PipeWire and PulseAudio backends. ALSA default buffer = 512 frames post-fix; `BufferSize::Fixed` is a request, may round up. | cpal CHANGELOG; cpal issue #1029; cpal/src/host/alsa/mod.rs |
| rodio 0.22.2 (2026-02-22) active; `mixer::mixer` + `Mixer::add` = fire-and-forget unbounded voices; **no master-volume API** (needs a gain wrapper Source); **pins cpal ^0.17** (no cpal 0.18 backends through rodio); mixer silently detaches when empty (keep Zero source). | docs.rs/rodio, mixer.rs, CHANGELOG/UPGRADE |
| rodio mp3 via symphonia default (PR #453); `SamplesBuffer::new(channels, rate, Vec<f32>)` serves cached decoded samples with cheap Arc-backed cloning. | rodio PR #453; docs.rs SamplesBuffer |
| symphonia 0.6.0 (2026-05-15) breaking vs 0.5.x; rodio still on 0.5.5; direct use must pin one major. | crates.io/crates/symphonia; symphony releases |
| pipewire-rs 0.10.0 requires a live PipeWire daemon (fails on pure PulseAudio/ALSA); no decode (needs symphonia anyway); buffer API mixes safe/unsafe. | crates.io/crates/pipewire; freedesktop Stream docs |
| Decoded-buffer caching is ours in every candidate: decode each file once to `Vec<f32>` (Arc-shared via DashMap/OnceCell), spawn buffer-backed voices per press. Precedent: keymon (rodio + JSON sound-buffer caching). | Research brief §5; github.com/Agastya18/keymon |
| Vendored `miniaudio.h` 0.11.21 + ~50-line FFI shim is the only 1:1 C-engine parity path (miniaudio stays active: 0.11.25, 0.12 split upcoming). | miniaudio releases; research brief |

## Conclusion
**Primary: cpal 0.18.1 + symphonia 0.6.0 directly** (~150-250 lines of our own mixer:
decode-to-cache + voice pool + one master multiply = exact "engine-global volume
applied once" semantics, best latency control via `BufferSize::Fixed(128..512)`).
Both crates actively maintained; wav+mp3 via `symphonia-bundle-wav`/`symphonia-bundle-mp3`.
Owning the mixer is exactly where the no-hard-cap / recycling / single-gain
requirements live.

**Alternative: rodio 0.22.2** — least code, best docs; `mixer::mixer` + `Mixer::add` +
cached `SamplesBuffer` covers every requirement; trade-offs: no master-volume API
(gain wrapper), no direct period control, one extra internal buffer hop, cpal ^0.17 pin.

**Rejected:** miniaudio Rust bindings (dead/niche) and pipewire-rs (extra transport
dep + still our mixer). Vendoring miniaudio.h + FFI remains the parity escape hatch.

Latency expectation to measure (feeds 07): event→audio ~10-25 ms total with 256-512
frame periods; wayvibes feels instant at default engine config.

**User confirmed (ask_user): cpal + symphonia** is the chosen stack for the backend.