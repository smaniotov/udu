---
type: decision
status: closed
blocked_by: [06]
---

# Feature→module mapping and phase skeleton (curated set)

## Evidence
| Fact | Verified at |
|---|---|
| Our modules: capture (press filter, reconnect), mapping (Mechvibes iohook→evdev, strict), audio (cache, voice pool, master gain), control socket + client, TUI. | docs/plans/wayvibes-tui-backend/ (live) |
| Curated set = F1, F2, F3, F4, F5, F6, F8, F9, F10, F12, F14, F17, F18, F20. | [[06-curated-features]] |

## Choice
(closed with the curated set; details below are the execution skeleton)

| # | Phase (features) | Modules touched | Acceptance oracle@ |
|---|---|---|---|
| 1 | Rename to udu (rollout) | Cargo, service, control, main, README, tests | [[04-rollout]] decision; binary/service/socket `udu`; live setup keeps working |
| 2 | Press+release + pitch (F3/F4/F5) | capture (release surface), mapping (`*-up` optional — flips the current fatal rule), audio (per-voice pitch jitter) | press→down sample, release→up sample, pitch differs per press; pack with `*-up` validates; existing packs unaffected |
| 3 | Modifier mute + Return ding (F6/F8) | mapping (modifier filter + ding entry), control (toggle) | toggles; Enter ding optional; mods silent when muted |
| 4 | Volume 0–100 + presets + output routing (F9/F10) | config (scale migration 0–10→0–100 + migration path), control protocol, audio gain; sink select | Soft30/Balanced60/Loud90 match Klack; old configs migrate; sink change applies |
| 5 | TUI surface + preview + feedback + onboarding (F17/F18/F20) | control (play_once), ui (preview-on-select, settings, stats entry, perms wizard) | preview on select; toggle/switch cues; first-run perms guide |
| 6 | Stats (F14) | backend stats (counters + persistence + JSON), ui stats view | counters persist; export to Markdown |
| 7 | Tone Pad DSP (F12) | audio (pan + distance DSP, 2D control), control (tone pad set), ui (pad widget) | pad x/y changes pan/distance audibly |

## Consequence
Each phase gets contracts + files + acceptance oracle written at zoom time, never
executed from this skeleton.