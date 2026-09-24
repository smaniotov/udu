# ADR-0004: cpal 0.18 + symphonia 0.6 audio engine with live tone shaping

Status: accepted · 2026-08-09 · Amended 2026-09-24 for the two-dimensional tone pad

## Context
wayvibes uses miniaudio 0.11.21: engine-global volume (one gain on the endpoint),
fire-and-forget overlapping inlined sounds recycled on finish, in-memory full decode
per file. The Rust binding landscape makes miniaudio parity impractical: the classic
`miniaudio` crate is archived at 0.10; maudio/miniaudio_aurex are too young/niche;
pipewire-rs adds a transport dependency and still needs our mixer.

## Decision
Use `cpal 0.18.1` + `symphonia 0.6.0` (`symphonia-bundle-wav`, `symphonia-bundle-mp3`)
directly. The backend owns the mixer: decode-to-cache (`Arc<Vec<f32>>` per file, lazy
off the audio thread), a voice pool with recycling and no hard cap, and a single
master multiply implementing the engine-global volume (0.0–10.0, clamped).
**Amended 2026-08-09 (udu F4)**: the volume surface follows the Klack model — range
0–100 with presets Soft 30 / Balanced 60 / Loud 90; the engine gain is `volume / 10`,
so legacy values keep their loudness after the one-time scale migration
(`migrate_volume_scale`). `BufferSize::Fixed(256)` is the latency knob. rodio 0.22.2
was the alternative (less code, no master-volume API, cpal ^0.17 pin) — user chose
cpal+symphonia.

**Amended 2026-09-24**: add `biquad 0.6.0` for a two-dimensional spectral tone pad.
The pad controls fixed-frequency low- and high-shelf gains from -12 to +12 dB;
the center is neutral. The existing pan and distance controls remain independent.
The mixer applies the shelves after voice mixing and before the output ceiling. Each
output channel owns persistent Direct Form 1 filter state. Gain changes retune the
existing filters from atomically published values, and an output-device change
rebuilds the state for the new channel count and sample rate. The callback adds no
I/O or allocation for this stage.

## Consequences
- Decoded-buffer caching is ours: udu decodes each file once and reuses it for
  overlapping playback. This is the main measurable performance win.
- `biquad` is a focused DSP dependency; no general-purpose audio engine is added.
- A neutral tone pad bypasses the filter work. Non-neutral settings add up to two
  shelf filters per output channel before the existing ceiling limiter.
- symphonia is MPL-2.0 (fine for this private, personal, non-commercial project).
- The benchmark phase must validate achieved latency and callback CPU cost (cpal
  `BufferSize::Fixed` is a request — ALSA may round).
- The stream can be rebuilt on any enumerated output device, keeping the voice pool,
  filter state reset for the new format, and decode cache alive across switches.