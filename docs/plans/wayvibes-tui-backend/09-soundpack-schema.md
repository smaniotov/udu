---
type: research
status: closed
blocked_by: []
---

# Soundpack schema edge fields in Mechvibes-format packs

## Evidence
| Fact | Verified at |
|---|---|
| All 20 bundled packs carry top-level fields beyond `defines`: `_zip`, `default`, `description`, `group`, `id`, `includes_numpad`, `key_define_type`, `m_author`, `name`, `sound`, `tags`. wayvibes reads **only** `defines`. | Local scan; `/home/smaniotov/Documents/external/wayvibes/src/config.cpp:24` |
| **Defines keys are iohook (libuiohook) keycodes, not evdev codes and not DOM codes.** Mechvibes remaps identity for linux and looks up `keycode-${event.keycode}` from iohook; wayvibes must translate to evdev via `remapMechToLinuxKey` (arrows 57416/57424/57419/57421 + 61xxx win variants, Home/End/PgUp/PgDn/Ins/Del, KP specials, RCtrl/RAlt/Meta/Menu, F13-F15 91/92/93→183/184/185, PrtSc, Pause; base block passes through). | mechvibes `config-v1.js`/`keycodes.js`/`remapper.js`; wayvibes `config.cpp`; research brief §2, §5 |
| `key_define_type` selects value shape: `"multi"` = filename string per key; `"single"` = `[startMs, lenMs]` clip on top-level `sound` file; wild packs also use `"multiple"` (mechvibes rejects, wayvibes accepts). Full file-array semantics exist only in draft v3. | mechvibes `config-v1.js`/`config-v2.js`; wiki "Config Versions"; local scan (Creams, boxjade = `"multiple"`) |
| `defines["default"]` is **not a fallback feature** in any implementation: inert as `keycode-default` in mechvibes, hard crash (`std::stoi("default")`) in wayvibes. Real v2 fallback = top-level `sound`/`soundup`. | research brief §3, §4; mechvibes `HandleEvent` |
| v2 packs (`version:2`) lose `sound`/`soundup` fallback under wayvibes AND crash on `*-up` keys (`std::stoi("14-up")` throws, escapes the catch). v3 packs fully silent under wayvibes (array values throw `type_error`, map aborts). | research brief §3-§4; wayvibes `config.cpp` |
| `sound`, `includes_numpad`, `name`, `id`, `group`, `tags`, `description`, `_zip`, `m_author` are inert at playback time in both implementations for v1 packs. | research brief §3 (per-field table) |
| Suspected wayvibes bugs (feed decision 10): End(3663)/PgDn(3665) mapped opposite to the iohook/win-scancode tables; iohook RightShift=54 (evdev 62) not remapped. Packs that distinguish End/PgDn (Creams, banana split, nk-cream) are bundled here. | research brief §5; local scan |
| All 20 bundled packs are v1, `defines`-based, with iohook codes incl. remapped >767 entries (57416, 60999-61011, 91/92/93) and 30 `null` values. | Local scan; research brief §2 |

## Conclusion
Defines-only parity is **correct for the bundled v1 packs**: every key with a defines
entry plays, unmapped keys are silent in both implementations, and the metadata fields
are inert. The mapping contract therefore needs an **iohook→evdev remap layer** in the
runtime mapper (new decision note 10), and the strict validator adopts:

- **Fatal** (silent-failure modes under wayvibes): defines value that is non-string
  non-null (single-mode clip arrays → "sprite packs unsupported"; numbers/bools);
  `*-up` keys and any non-numeric defines key (wayvibes hard-crashes — we must never
  crash, clear message instead, since `*-up` signals an unhonorable v2 pack).
- **Warn** (behavioral difference, not load failure): `version` present and ≠ 1
  (v2 loses fallback, v3 unsupported); `key_define_type` ∉ {multi, single}.
- **Skip-with-warning**: inert metadata keys like `"default"` in defines.
- **Ignore** (proven inert): `includes_numpad`, `name`, `id`, `group`, `tags`,
  `description`, `_zip`, `m_author`, and (when `version` absent) `sound`.

Current `soundpack.rs` validation already rejects non-u16 keys and non-string values —
consistent with the fatal rules above; the validator gains the `*-up`/`default`
distinction and the v2/v3 warnings in execution phases.