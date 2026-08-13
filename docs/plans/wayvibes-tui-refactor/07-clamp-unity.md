---
type: decision
status: closed
blocked_by: []
---

# One `clamp_volume` owns the volume invariant

## Evidence

| Fact | Verified at |
|---|---|
| `clamp(MIN_VOLUME, MAX_VOLUME)` duplicated in 4 modules: `config.rs::prepare_config`, `service.rs::normalized_config`, `app.rs::adjust_volume`, `process.rs::build_wayvibes_command`. | local source read |
| The invariant (0.0–10.0) is a domain rule; one definition site prevents silent drift when the range changes. | code-style: one concept = one name |

## Choice

Add `pub fn clamp_volume(value: f32) -> f32` (using the existing `MIN_VOLUME`/`MAX_VOLUME` in
`config.rs`) and call it from all four sites. A newtype is deferred (no external boundary
needs it yet).

## Rejected alternatives

| Alternative | Why not |
|---|---|
| `Volume` newtype | Stronger typing but touches serde representation and every call site; no second consumer today |
| Leave 4 clones | The invariant can drift; this refactor exists to remove systemic duplication |

## Consequence

Changing the volume range touches one function. Behavior identical (same clamp in every
site).