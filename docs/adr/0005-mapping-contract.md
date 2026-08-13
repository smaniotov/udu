# ADR-0005: Mechvibes mapping contract with corrected iohook→evdev remap

Status: accepted · 2026-08-09

## Context
Mechvibes-soundpack `defines` keys are iohook (libuiohook) keycodes, not evdev codes.
The **local** wayvibes clone (`0c94f0c`) performs no remap — arrows, nav cluster,
F13-F15 and 6xxxx codes never match, so those keys are silent today. Upstream main
added `remapMechToLinuxKey` (config.cpp:15-86) but contains a suspected End/PgDn
swap (3663→PAGEDOWN, 3665→END — reversed vs the iohook/thock/win-scancode tables) and
omits RightShift (iohook 54, evdev 62). The user requires the keys to be mapped
perfectly.

## Decision
The runtime mapper keeps klack's strict pack validation (u16 keys, relative paths
without `..`, files exist) and implements the **corrected** upstream remap table:
pass-through base block + the full table with the End/PgDn pair corrected
(3663→KEY_END, 3665→KEY_PAGEDOWN — iohook enum, confirmed by the empirical probe and
visible in 17/20 bundled packs). RightShift is **not** remapped (retracted research
claim): iohook 54 == evdev `KEY_RIGHTSHIFT` 54, base block covers it. `null` entries
create no mapping; codes >767 not present in the table never match.

## Consequences
- Behavior diverges from the local wayvibes (nav/arrows/F13-15/6xxxx now sound —
  currently silent) and from upstream (End/PgDn pair) — documented in README.
- Each remap entry and the End/PgDn correction gets a dedicated test.
- `*-up` defines keys were initially fatal (v2-style, wayvibes hard-crashes on
  them); **amended 2026-08-09 (udu F4)**: well-formed `<digits>-up` entries are now
  accepted as optional key-up sounds (releases play them), the rest of the map
  unchanged; malformed dash keys and non-numeric keys remain fatal with a clear
  message.