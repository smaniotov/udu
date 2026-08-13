# ADR-0004: cpal 0.18 + symphonia 0.6 audio engine (own mixer, one master gain)

Status: accepted · 2026-08-09

## Context
wayvibes uses miniaudio 0.11.21: engine-global volume (one gain on the endpoint),
fire-and-forget overlapping inlined sounds recycled on finish, in-memory full decode
per file. The Rust binding landscape makes miniaudio parity impractical: the classic
`miniaudio` crate is archived at 0.10; maudio/miniaudio_aurex are too young/niche;
pipewire-rs adds a transport dependency and still needs our mixer.

## Decision
Use `cpal 0.18.1` + `symphonia 0.6.0` (`symphonia-bundle-wav`, `symphonia-bundle-mp3`)
directly. The backend owns ~150-250 lines of mixer: decode-to-cache
(`Arc<Vec<f32>>` per file, lazy off the audio thread), a voice pool with recycling and
no hard cap, and a single master multiply implementing the engine-global volume
(0.0–10.0, clamped). **Amended 2026-08-09 (udu F4)**: the volume surface follows the
Klack model — range 0–100 with presets Soft 30 / Balanced 60 / Loud 90; the engine
gain is `volume / 10`, so legacy values keep their loudness after the one-time
scale migration (`migrate_volume_scale`). `BufferSize::Fixed(256)` is the latency knob. rodio 0.22.2 was the
alternative (less code, no master-volume API, cpal ^0.17 pin) — user chose
cpal+symphonia.

## Consequences
- Decoded-buffer caching is ours: wayvibes re-decodes per press; klack decodes once
  per file. This is the main measurable performance win (ADR/plan 07).
- symphonia is MPL-2.0 (fine for this private, personal, non-commercial project).
- The benchmark phase must validate achieved latency (cpal `BufferSize::Fixed` is a
  request — ALSA may round).
- F10 output routing: the stream can be rebuilt on any enumerated output device
  (`host.output_devices()` + `DeviceTrait` display name), keeping the voice pool and
  decode cache alive across switches.