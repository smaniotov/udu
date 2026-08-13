---
type: decision
status: closed
blocked_by: [02]
---

# Parity scope: full F1–F22, prioritized phases

## Evidence
| Fact | Verified at |
|---|---|
| User chose "Tudo (F1–F22), em fases priorizadas" in ask_user: core first (keystroke→sound, up/down, pitch, switches, volume+presets, TUI), analogues after (HRTF/TonePad, PipeWire sleep triggers, libnotify, CLI/IPC, SQLite stats). | ask_user session record |
| Portable subset vs macOS-bound analogues classified per feature with sources. | [[02-reference-feature-survey]] |

## Choice
The plan deep-covers every feature F1–F22 on our stack. Execution is prioritized:
núcleo portável first (F1–F12 + F14 + F16–F19), macOS-bound analogues after
(F11/F12 spatial+TonePad, F13 sleep triggers, F15 notifications, F22 licensing
self-decided). Each feature group becomes a phase with contract + acceptance
oracle. Proprietary Klack assets stay out of scope; parity covers surface + pack
MODEL (per-switch banks, up/down samples, pitching).

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Portable core only | User asked for all features |
| Per-feature curation turns | User picked the phased plan over iterative per-feature negotiation |

## Consequence
Feeds [[03-parity-mapping]]'s execution shape and the phase drafts in the plan. The
following flagged sub-decisions surface during phase work (execution-time
decisions, not closing here):
- Volume scale: Klack is 0–100 + presets Soft30/Balanced60/Loud90; our backend is
  0–10 (wayvibes parity, ADR-0004). Adopting Klack semantics changes the socket
  protocol + config (F9/F17 phase).
- Up/down sounds (F4) reuse the Mechvibes v2 `*-up` key convention, which the
  backend currently rejects as fatal (UpKey rule from the mapping plan). The
  up/down phase flips that rule into an optional feature and must revise
  `src/soundpack.rs` + tests + the README note.
- Mouse clicks (F7) require opening a second capture device (mouse) with its own
  mapping — device contract change (F7 phase).
- Global toggle hotkey (F16) conflicts with the no-grab capture policy; the TUI
  phase decides between a session-level shortcut (desktop env) and a grab scoped
  to a rare hotkey combo.