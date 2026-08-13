---
type: decision
status: closed
blocked_by: [02]
---

# Curated feature set (user selection)

## Evidence
| Fact | Verified at |
|---|---|
| User selected **F1, F2, F3, F4, F5, F6, F8, F9, F10, F12, F14, F17, F18, F20** (14 of 22) in a multi-select over the F1–F22 inventory. | ask_user session record |
| Not selected (excluded by the user): F7 mouse clicks, F11 spatial audio, F13 sleep triggers, F15 notifications, F16 global hotkey, F19 automation CLI/IPC, F21 web demo, F22 licensing. No per-feature comment was given; exclusions recorded as-is without inventing reasons. | ask_user session record |
| F12 Tone Pad kept while F11 spatial was dropped — coherent: pan/distance DSP can be built without full HRTF. | Inference from selection |
| Volume scale migration (F9), `*-up` rule flip (F3/F4), mouse-device contract (F7 not selected → no mouse phase), hotkey grab policy (F16 not selected → no hotkey phase). | [[05-parity-scope]] flagged sub-decisions |

## Choice
The operative parity scope for udu = the 14 selected features. This supersedes the
earlier "all F1–F22" scope ([[05-parity-scope]]). Dropped features are out of scope
for this plan unless the user re-adds one later.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Full F1–F22 (previous scope) | User curated it down to 14 during feature review |
| Portable-only subset (F1–F12+F14+F16–F19) | User's selection is neither: keeps F10/F12 (analogues) and drops F7/F11/F13/F15/F16/F19 |

## Consequence
The phase skeleton ([[03-parity-mapping]]) is rewritten for the 14 features only.
Phases: rename (uffle rollout [[04-rollout]]), press/release+pitch (F3/F4/F5),
modifier mute + return ding (F6/F8), volume 0–100+presets + output routing
(F9/F10), TUI surface + preview + feedback sounds + onboarding (F17/F18/F20),
stats (F14), Tone Pad DSP (F12). Each phase gets zoom-written contracts before
implementation.