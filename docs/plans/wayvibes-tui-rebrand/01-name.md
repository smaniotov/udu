---
type: decision
status: closed
blocked_by: []
---

# New product/binary name: udu

## Evidence
| Fact | Verified at |
|---|---|
| User chose **udu** in ask_user (options: ngoma / udu / shekere / ngoni, all verified African percussion/instrument words; user had redirected away from Western onomatopoeia toward African culture + sounds). | ask_user session record |
| **udu** (ùdù): Igbo word for "vessel/pot"; the Udu of the Igbo of Nigeria is a plosive aerophone/idiophone hand percussion pot-drum — a real African percussion instrument with a coherent sound meaning. | Wikipedia "Udu" (2026-08-09) |
| Name-safe mechanically: no `udu` binary on PATH, no `udu.sock`/`udu.service` installed, casing/pattern valid for Rust bin + systemd unit + socket names. | Local checks (2026-08-09) |
| crates.io availability could not be confirmed (API rate-limited 403); final registry check stays a rename-phase task (we do not publish today). | crates.io API (2026-08-09) |
| Distinct from "klack" (official macOS app, collided with) and "wayvibes" (former external project). | [[02-reference-feature-survey]], grounding |

## Choice
The product/binary/service/socket is renamed to **udu** (udu.service, udu.sock,
binary `udu`) — rollout mechanics in [[04-rollout]].

## Rejected alternatives
| Alternative | Why not |
|---|---|
| ngoma / shekere / ngoni | User chose udu; ngoma (5 letras, música+danca+tambor) and shekere (7) were strong contenders, ngoni is a string instrument (less "key tap") |
| thock / kata / tappa / cadence (Western candidates) | User explicitly redirected to African culture + sound meaning |

## Consequence
Sets the rename target for [[04-rollout]] and the README/help/tests surface; the 02
inventory and parity plan are name-independent and unaffected.