# Add a two-dimensional tone-shaping pad

## Brief
- Status: implemented; audio-device performance validation pending
- Outcome: users shape the keyboard sound's frequency balance from the Audio tab with a compact, keyboard-operable two-dimensional pad.
- Why now: the current `Tone pan` and `Tone distance` controls change spatial placement, not spectral tone. The user wanted frequency-based tone shaping, using a pad-style interface.

## Evidence
- `src/backend/audio.rs` requests 256-frame output buffers, mixes voices in the CPAL callback, applies the tone shelves, then applies the output ceiling.
- `src/backend/audio.rs` exposes live pan, distance, bass gain, and treble gain through the audio control interface.
- `src/config.rs` persists the four tone values; `src/app.rs` exposes the EQ gains through the pad and keeps pan and distance as separate adjustable rows in Audio settings.
- Klack's official listing advertises “Tone pad tuning” but does not document frequency bands or its DSP. A MacBorn review describes a perceived range from thick/dull to sharp/bright; an independent Klack 2.0 review describes adjusting source position and distance. These accounts are not a specification of Klack's internal signal processing.
- The W3C Audio EQ Cookbook documents standard biquad filters for shelving and peaking EQ. `biquad` 0.6.0 was selected because its current docs support low/high shelves and Direct Form 1 for live coefficient changes; its March 2026 release uses edition 2024 and MIT OR Apache-2.0.

## Decisions
| ID | Decision | Rationale and consequence |
|---|---|---|
| D01 | Use a two-dimensional pad to change spectral tone, not the existing pan/distance behavior. | The user explicitly chose frequency-based timbre shaping. Klack is a visual reference, not a confirmed DSP specification. |
| D02 | Use fixed low- and high-shelf frequency ranges, with the pad adjusting their gains. | This keeps the pad quick to use without requiring numeric frequency entry. Axes: warm ↔ bright adjusts the high shelf; light ↔ full adjusts the low shelf. The center is neutral. |
| D03 | Keep the existing pan and distance controls as separate settings. | Spatial placement remains available and is not replaced by EQ. |
| D04 | Extend persisted configuration and live control/status to carry the EQ values. | The user approved this boundary change on 2026-09-24 so the setting can persist and affect the running backend. |
| D05 | Use `biquad` 0.6.0 for the low- and high-shelf filters. | The user approved the focused dependency on 2026-09-24. Its Direct Form 1 implementation supports live coefficient updates with reduced retuning artifacts, avoiding a locally maintained filter implementation. |

## Scope
### In
- A keyboard-operated pad in Audio settings, with visible axis meanings and a neutral center.
- Live application and persistence of the two EQ gains.
- DSP that does not add blocking work or allocation to the audio callback, with fixed cutoff frequencies and bounded gains; measure CPU and latency impact where the audio environment permits.

### Out
- Reproducing Klack's undisclosed DSP or claiming frequency behavior from its Tone Pad.
- Replacing the existing pan and distance behavior with EQ.
- Adding numeric frequency selection or a general multi-band equalizer.

## Delivered
- Added serde-defaulted bass and treble gain settings, backend status fields, and live `apply_config` synchronization.
- Applied low- and high-shelf filters to the mixed output before the ceiling limiter. Filter state is per output channel; coefficient changes account for output sample-rate changes without callback I/O or allocation.
- Added an interactive Audio pad with warm ↔ bright and light ↔ full axes, a neutral center, and separate pan and distance controls.
- Fixed shelf frequencies at 180 Hz and 4 kHz and clamped gains to -12…+12 dB. The neutral position bypasses the filters.
- Updated ADR-0002 and ADR-0004. System-level CPU and latency measurements remain dependent on an available audio environment.
