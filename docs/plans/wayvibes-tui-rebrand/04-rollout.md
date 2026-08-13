---
type: decision
status: open
blocked_by: [01]
---

# Rebrand mechanics/rollout

## Evidence
| Fact | Verified at |
|---|---|
| Rename surface measured (2026-08-09): `[[bin]] name = "klack"`, lib crate `wayvibes_tui`, package `wayvibes-tui`; `SERVICE_NAME = "klack.service"` (legacy `wayvibes-tui.service` already migrated in Phase 6); socket `SOCKET_NAME = "klack.sock"`; config dir `wayvibes-tui/` + data dir `wayvibes-tui/soundpacks`; README (11 refs), CLI help strings, `tests/cli.rs`, `tests/perf.rs` device names. | grep identity (charting grounding) |
| Users have an installed `klack.service` (active, ExecStart = ~/.local/bin/klack) and a `wayvibes-tui` config/data dir with the real packs + a `klack.sock` in `$XDG_RUNTIME_DIR`. Migration must keep the live setup working. | Live system state (2026-08-09) |

## Choice
**Full rename including data dirs** (user decision, ask_user "Renomear até os diretórios de dados"):
- Binary `udu`, systemd `udu.service`, socket `udu.sock`, package/lib `udu`
  (lib from src/lib.rs; bin target `udu`).
- Config → `$XDG_CONFIG_HOME/udu/config.json`; data → `$XDG_DATA_HOME/udu/soundpacks`.
- README, CLI help strings, tests, perf device names renamed.
- First launch migrates the old config + packs (copy) from `~/.config/wayvibes-tui/`
  and `~/.local/share/wayvibes-tui/`; the old dirs are **kept, not deleted** (no data
  loss on migration failure). The installed legacy `klack.service` is stopped,
  disabled and removed (same pattern as the earlier wayvibes-tui.service→klack.service
  migration), then `udu.service` is installed/enabled/started.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Keep data paths as wayvibes-tui/ | User chose the clean break with migration |

## Consequence
Sets rename phase scope and its oracle: after first launch, `udu.service` active,
`socket udu.sock`, config/packs at the new paths, old dirs intact, old unit gone,
no `klack`/`wayvibes-tui` product strings left (crate-name note: package `udu`, lib
`udu`; the historical docs/plans keep their names as history).