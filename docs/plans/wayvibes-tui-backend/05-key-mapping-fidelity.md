---
type: decision
status: closed
blocked_by: []
---

# Key mapping fidelity: strict validation + faithful runtime vs byte-compat leniency

## Evidence
| Fact | Verified at |
|---|---|
| klack already validates packs strictly: `defines` keys must parse as `u16`, values must be relative paths without `..`, referenced files must exist and be openable. | `src/soundpack.rs:136-166` |
| wayvibes is lenient: `std::stoi` prefix parsing (`"30abc"` → 30), values concatenated verbatim as `soundpack + "/" + value` with no sanitization (a `../` value can escape the pack dir), non-numeric key → `std::terminate` crash; map loaded once. | `/home/smaniotov/Documents/external/wayvibes/src/config.cpp:25-35`, `audio.cpp:47`; brief §3, §6 |
| Mechvibes-format packs carry codes > 767 (observed up to 61011) that can never fire on Linux — accepted and never matched. | Local scan; brief §3 |
| Runtime semantics to preserve: only `EV_KEY && value == 1` plays; `null` entries create no mapping; unmapped key = silence; missing file at runtime = stderr per press, continue. | brief §2, §3 |
| User constraint: soundpack model must stay compatible with wayvibes' Mechvibes format. | Session instruction |

## Choice
Keep klack's selection-time validation (u16 decimal keys, relative paths without
`..`, referenced files exist and openable) and implement a faithful runtime mapper:
decimal parse of keys, `null` entries create no mapping, codes > 767 accepted but
never matched, values resolved relative to the pack directory, missing file at
runtime → stderr per press and continue. Distinct from wayvibes only in rejecting
packs wayvibes would crash on.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Byte-compat leniency (stoi prefix semantics, verbatim path concat incl. `../` escape, hard-crash on bad key) | User endorsed "mapeamento Mechvibes perfeito" via 00; unsafe paths and ctashes are defects, not contract |

## Consequence
Sets the mapping module contract and its tests; `soundpack.rs` validation is kept as-is. Runtime mapping matches wayvibes exactly for every pack klack accepts. Feeds the performance budget (07): decode caching applies to mapped files only.