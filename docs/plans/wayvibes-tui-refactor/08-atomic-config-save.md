---
type: decision
status: closed
blocked_by: []
---

# Save config atomically (temp file + rename)

## Evidence

| Fact | Verified at |
|---|---|
| `save_config` writes directly with `fs::write` — a crash mid-write truncates the config; a corrupted config stops the managed backend from starting. | local `src/config.rs` |
| Writing a temp file in the same directory then `fs::rename` is atomic on the same filesystem (POSIX). | rename(2) semantics (verified knowledge; applied in many tools) |

## Choice

Serialize to a sibling temp file (e.g., `<path>.tmp`), then `fs::rename` over the target on
success. Clean up the temp file on error.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Keep in-place write | Small window of corruption that bricks the service start path |
| `tempfile` crate | One more dependency for a two-line pattern already present |

## Consequence

The config file is either the old or the new complete content, never partial. No behavior
change in the happy path.