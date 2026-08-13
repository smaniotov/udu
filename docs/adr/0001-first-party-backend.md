# ADR-0001: First-party in-process backend replacing the external wayvibes binary

Status: accepted · 2026-08-09 · Supersedes `docs/plans/wayvibes-tui/plan.md` decisions 01 and 06

## Context
klack originally managed the external C++ `wayvibes` process via a systemd unit,
restarting it on every configuration change ("extremamente custoso e lento" per user).
The user now wants the application 100% first-party (autoral): own the kernel input
tracking and the sound backend, with no external binary dependency.

## Decision
The `--service` mode of the same `klack` binary runs the capture + audio loop
in-process. `wayvibes_tui` gains a `backend` module (capture, mapping, audio, control)
invoked from service mode. No separate daemon binary. The systemd unit keeps the
lifecycle but no longer spawns an external child.

## Consequences
- The systemd unit's `Restart=on-failure` becomes crash recovery only; configuration
  changes never restart the process (see ADR-0002).
- `wayvibes_path` config, `--wayvibes` CLI, `pkill wayvibes`, and `src/process.rs`
  are removed.
- Behavior parity with the local wayvibes binary is deliberately broken where that
  binary was defective (see ADR-0005).