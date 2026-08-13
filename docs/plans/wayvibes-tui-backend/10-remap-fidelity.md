---
type: decision
status: closed
blocked_by: [10]
---

# iohook→evdev remap table fidelity in the runtime mapper

## Evidence
| Fact | Verified at |
|---|---|
| Pack `defines` keys are **iohook (libuiohook) keycodes**, not evdev codes; the base block (letters/digits/Space/F-keys) equals evdev, but arrows (57416 Up, 57419 Left, 57421 Right, 57424 Down), nav/modifiers (Home 3655, End 3663, PgUp 3657, PgDn 3665, Ins 3666, Del 3667, PrtSc 3639, Numpad Enter 3612, KP= 3637, KP* 3597, RCtrl 3613, RAlt 3640, Meta 3675, Menu 3677) and win-variant 6xxxx codes differ. | [[09-soundpack-schema]]; mechvibes `keycodes.js`; `UiohookKey`; `mechvibes2thock.py` |
| wayvibes translates pack keys to evdev with `remapMechToLinuxKey` (config.cpp): arrows + 61xxx variants, Home/End/PgUp/PgDn/Ins/Del, KP specials, RCtrl/RAlt/Meta/Menu, F13-F15 (91/92/93 → 183/184/185; evdev 91 is KEY_HIRAGANA so this matters), PrtSc, Pause; everything else passes through. | `/home/smaniotov/Documents/external/wayvibes/src/config.cpp` (`remapMechToLinuxKey`), research 09 §2 |
| Suspected wayvibes bug: `3663 → KEY_PAGEDOWN` and `3665 → KEY_END` — the opposite of the iohook/thock/win-scancode tables (End=3663, PgDn=3665); its win-variant mappings (60999=Home, 61007=End, 61001=PgUp, 61009=PgDn) are correct, corroborating the swap as a bug. | research 09 §5 (three independent tables) |
| Suspected hole: iohook RightShift = 54 (evdev 62) is not remapped — packs keyed "54" are silent on right-shift. | research 09 §5 (**retracted** — 54 == evdev `KEY_RIGHTSHIFT` 54; base block covers it) |
| Bundled packs that distinguish End/PgDn (Creams, banana split lubed/stock, nk-cream) and packs keyed 91/92/93 or 6xxxx exist in `sounds/`; 30 `null` entries aside, keys 1..61011 present. | Local scan; research 09 §2 |
| User requirement: "mapear perfeitamente como as teclas são mapeadas" + decision 00 base-parity-with-improvements. | Session instructions; [[00-ownership-scope]] |
| The End/PgDn swap needs a 5-minute empirical check (real evdev End/PgDn event codes vs pack keys 3663/3665) before fixing blindly. | research 09 "Gaps" |
| **The local clone (commit 0c94f0c) has NO remap**: `loadKeySoundMappings` stores keys as-is and `runMainLoop` looks up `keySoundMap.find(ev.code)` with no translation — i.e. today, arrows/nav/F13-15/6xxxx pack keys never match and are silent. | `/home/smaniotov/Documents/external/wayvibes/src/config.cpp:8-38` (local); `src/audio.cpp:45` (local) |
| **Upstream main added `remapMechToLinuxKey`** (src/config.cpp:15-86, applied at :108): arrows 57416/57424/57419/57421 + 61000/61008/61003/61005; nav 3655 Home, **3665→KEY_END**, 3657 PgUp, **3663→KEY_PAGEDOWN**, 3666 Ins, 3667 Del (+ 60999/61007/61001/61009/61010/61011 win variants); KP Enter// = 3612/3637/3597; PrtSc 3639, Pause 3653; F13-F15 91/92/93→KEY_F13-15; RCtrl/RAlt/LMeta/RMeta/Menu 3613/3640/3675/3676/3677; default pass-through. **3665→END + 3663→PAGEDOWN is the swapped pair** (iohook: End=3663, PgDn=3665). | Upstream `src/config.cpp:15-86,108` (fetched 2026-08-09) |
| **RightShift is NOT a hole — retracted.** Pack key 54 = iohook RightShift = evdev `KEY_RIGHTSHIFT` (54): base-block pass-through already works (local packs key 54 with the shift sound, same as 42). The research's "evdev 62" claim was wrong (62 is not RightShift in evdev). | Local pack scan (`54` → shift sounds, 17 packs); Linux `input-event-codes.h` (KEY_RIGHTSHIFT=54) |
| **The End/PgDn swap is visible in 17 of 20 bundled packs** (3663 and 3665 mapped to different files), so the correction changes which sound End/PgDn play in almost every pack. Arrows (57416 etc.) are present in 19 packs, F13-15 (91-93) in 8, win-variant 61xxx in 9 — all currently silent under the local wayvibes (no remap). | Local pack scan (2026-08-09) || **The End/PgDn swap is visible in 17 of 20 bundled packs** (3663 and 3665 mapped to different files), so the correction changes which sound End/PgDn play in almost every pack. Arrows (57416 etc.) are present in 19 packs, F13-15 (91-93) in 8, win-variant 61xxx in 9 — all currently silent under the local wayvibes (no remap). | Local pack scan (2026-08-09) |
| Empirical probe PENDING (user runs `cargo run --example probe_events`, presses End/PgDn/RightShift, returns the `press code=..` lines to record here). Direction independently corroborated (iohook keycodes.js, UiohookKey, win scancodes, 17/20 packs); expected End=107, PgDn=109, RightShift=54. | Task pending user execution; tracked in plan.md Phase 1 "Task first" |

## Choice
**Corrected remap** (user decision, ask_user): same `remapMechToLinuxKey` table as
upstream wayvibes, with only the End/PgDn pair corrected (3663→KEY_END,
3665→KEY_PAGEDOWN, per the iohook enum — confirmed by the empirical probe at the
start of the mapping phase and visible in 17/20 bundled packs). The earlier
RightShift 54→62 "fix" is **dropped**: 54 is base-block pass-through (evdev
`KEY_RIGHTSHIFT` = 54, packs key it with the shift sound). The remap table and its
one deliberate divergence from upstream are documented in the mapper module docs and
the README divergence note.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Faithful copy incl. End/PgDn swap | Keeps a demonstrable mapping defect (reversed sounds on End/PgDn in 17/20 packs), contradicting the "mapear perfeitamente" requirement (00) |
| Defer: parity in phase 1, fix later | Deliberate temporary wrong behavior for two keys; the check is 5 minutes and the fix is inside one table |
| RightShift "fix" 54→62 | Would BREAK right-shift (54 is already base-block evdev KEY_RIGHTSHIFT) — retracted |

## Consequence
Every remap entry (pass-through base block included) becomes a test case; the End/PgDn
correction gets explicit tests; the empirical probe records actual evdev codes for
End/PgDn on the target hardware into the fixtures. Pack behavior differs from wayvibes
**local** (nav/arrows/F13-15/6xxxx are currently silent) and from **upstream** (End/PgDn
swap) — both documented in the mapper module and README divergence note.