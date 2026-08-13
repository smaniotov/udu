---
type: decision
status: closed
blocked_by: [01, 02]
---

# What soundpack model should the TUI expose?

## Evidence
| Fact | Verified at |
|---|---|
| The external project consumes a Mechvibes-compatible `config.json` with a `defines` object mapping numeric key codes to file names. | `/home/smaniotov/Documents/external/wayvibes/src/config.cpp:8-38`; `/home/smaniotov/Documents/external/wayvibes/README.md:117-185` |
| The existing repository contains many packs with different naming conventions and per-pack license files. | `/home/smaniotov/Documents/external/wayvibes/soundpacks` inventory |
| The current loader does not define a TUI-facing validation or metadata contract. | `/home/smaniotov/Documents/external/wayvibes/src/config.cpp:8-38` |

## Choice
Keep user-provided soundpacks external and reference them by path. The TUI will discover directories configured by the user, use `~/.local/share/wayvibes-tui/soundpacks` on Linux when no root is configured, and scan the private repository's reviewed `sounds/` directory when no explicit root is provided. Every pack requires a Mechvibes-compatible `config.json`; referenced files and mappings are validated before restarting `wayvibes`. The manager persists the selected pack path and volume. Only individually reviewed, complete packs copied from the local wayvibes clone may be bundled, preserving each pack's license and attribution.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Copy every pack into a manager-owned library | It would copy assets whose licenses or completeness are not verified. Only individually reviewed packs may be bundled. |
| Convert or rewrite pack formats automatically | The first release should preserve upstream compatibility and avoid modifying user assets. |

## Consequence
The TUI must support configured search roots plus an explicit path workflow, provide a portable default root for first-run discovery, display directory names as pack labels, report malformed or missing files before process restart, preserve licenses for bundled packs, and keep unreviewed pack assets outside the repository.
