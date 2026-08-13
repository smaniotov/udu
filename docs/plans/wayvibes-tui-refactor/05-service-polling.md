---
type: decision
status: closed
blocked_by: [01, 04]
---

# Decouple service-status polling from the render tick

## Evidence

| Fact | Verified at |
|---|---|
| `run_tui` calls `app.poll_process()` every loop iteration (~10 Hz), each spawning `systemctl --user is-active` (fork+exec per frame). | local `src/main.rs` + `src/service.rs` |
| The status line "wayvibes service is not active" is the observable feature; the per-frame mechanism is the problem, not the status. | Chesterton's fence over `src/app.rs::poll_process` |
| The official event-loop guidance emits tick events on a schedule so polling happens outside the render-drive loop. | ratatui templates `event.rs` (`TICK_FPS`); event-loop docs |

## Choice

Check `is_active` at TUI start, then at most once per 5 s (a deadline/`Instant` guard) and on
demand after `apply`/`refresh`. Keep the status-line feature and its text. No subprocess is
spawned inside `draw`.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Move to a background event thread (template `EventHandler`) | One more moving part (channel, thread lifecycle) than a 5 s deadline guard; the app has no other async needs |
| Drop the status feature | Behavior removal without a user decision (Chesterton's fence) |

## Consequence

The render loop stops spawning processes; status freshness becomes ≤ 5 s stale, which is
invisible for this status. Throttle logic is unit-testable (injected clock/counter).