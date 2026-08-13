---
type: research
status: closed
blocked_by: []
---

# Official Klack (macOS) — full feature inventory

## Evidence
| Fact | Verified at |
|---|---|
| Klack is a menu-bar macOS utility by **Henrik Ruscon** (bundle `com.henrikruscon.Klack`), $4.99 one-time (no subscription/IAP), official site **tryklack.com** (`klack.app` does not resolve; the charting brief's "Wayney/Wayvans" attribution was wrong). | App Store listing; Daring Fireball (linked 2026-05-14); tryklack.com DNS check |
| **7 switch sets**: CherryMX Japanese Black, Everglide Crystal Purple + Oreo, Flurples Cardboard, Gateron Milky Yellow, Keychron Super Red, NovelKeys Cream (+ "None"). Each = **100+ individually recorded/mastered per-key files**, separate **down/up** samples, **randomized pitching**. **No user-importable packs, no .kpack, no drag-drop** (bundled proprietary format, ~12.4 MB app). | tryklack.com homepage (raw HTML + FAQs); App Store; Raycast klack extension (constants.ts); Forbes; 9to5Mac |
| Feature inventory F1–F22 (2026-08-09, v2.1.4): keystroke→sound hook; switch sets; up/down samples; randomized pitch; modifier-key mute; mouse click sounds; return-key "ding"; volume 0–100 + presets Soft 30 / Balanced 60 / Loud 90 (capped by system volume); output routing (speakers/headphones/both); spatial audio (v2.0, AirPods) + **Tone Pad** 2D placement pad (distance/position, not EQ); sleep triggers (ext keyboard, music playing, mic in use, headphones, in meeting) with mute-or-lower-volume; usage stats (keystrokes/dings/clicks/per-switch favorites, Markdown export, AppleScript `current stats`); notifications; global hotkey toggle (default ⌃⌘K in 2023; changed in 2.1.4, value unpublished); menu-bar surface (toggle, volume slider, switches submenu with **hover = realtime audio preview**, settings, quit); UI feedback sounds on toggle/switch change; AppleScript automation surface (toggle/on/off/sleep switch/volume/stats state JSON); accessibility onboarding; web demo/preview; direct license channel (Stripe, 5 devices, 14-day refund). | Raised by the AFK researcher with per-feature source URLs (tryklack.com/faqs, App Store, 9to5Mac, Pocket-lint, Forbes, MacSales/Rocket Yard, yeyulingfeng CN review, Raycast extension source, Daring Fireball, Reddit r/macapps) — full table in the research handoff (see session artifacts) |
| **Explicitly NOT features of Klack** (corrections to charting assumptions): no "Typer/Thinker/Master" modes, no on-screen key-press visualization, no user-importable packs — those belong to competitors (Klakk, Keeby) or were unverifiable; "Klack Loves" unverified. | Research classification + dropped sources (tryklakk, Keeby, ForeverZer0/klack) |
| Portable subset: F1/F4/F5/F6/F7/F8/F9 (presets 30/60/90)/F14/F16/F17/F18/F19 all map cleanly to evdev + our engine + TUI/CLI. macOS-bound analogues: spatial audio+Tone Pad (own HRTF/spatial DSP), sleep-trigger detection (PipeWire subset), notifications (libnotify), AppleScript (CLI/IPC twin). Pack format proprietary → port needs our own pack spec (Mechvibes-compatible as user format; ship 3–5 built-in switch sets to match). | Research portability column + top-15 ranking |

## Conclusion
The parity target is now concrete (F1–F22 with analogues ranked). The brief's assumed
modes/visualizer/pack-import were wrong and are dropped; korrect scope = the real
Klack surface. Proprietary pack assets are out of scope (licensing); parity covers
the feature surface and pack MODEL (per-switch banks, up/down samples, pitching).

## Gaps (open evidence, non-blocking)
- Exact asset formats/per-key maps unverifiable without decompiling the .app (not done).
- Held-key/autorepeat behavior unverified (Klack docs silent) — our design can decide (wayvibes parity: press-only).
- Sleep-trigger set rests largely on one CN secondary review.
- Full version history 1.8–2.0 not reconstructed.