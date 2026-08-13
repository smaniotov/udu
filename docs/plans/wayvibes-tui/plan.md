# Klack, a Linux Wayland keyboard-sound controller with a local TUI

## Destination
A Linux application provides a local TUI that manages the external `wayvibes` process, allowing the user to inspect devices, browse compatible soundpacks, switch the active pack, adjust volume, and observe operational errors. Configuration changes stop and restart `wayvibes` with the selected settings.

The first delivery targets Wayland sessions and preserves compatibility with the existing Mechvibes-style `config.json` soundpack format. The TUI provides read-only keyboard-device discovery, while `wayvibes` remains responsible for event capture. It does not promise compositor-specific global input APIs, mouse sounds, a graphical UI, a custom audio engine, or a replacement backend.

## Notes
- Runtime dependency: `/home/smaniotov/Documents/external/wayvibes`, currently C++17; it discovers keyboard devices through libevdev, reads `/dev/input/event*`, loads a soundpack mapping once, and plays files through vendored miniaudio. The new project manages this process instead of incorporating its backend.
- The installed executable is `~/.local/bin/klack`; the persistent owner is the systemd user unit `wayvibes-tui.service`. The TUI writes its runner/unit configuration and does not own the backend child.
- Existing local references: `/home/smaniotov/Documents/personal/cherrybuckle` has a libinput-based Linux/Wayland-capable scanner; `klack-manual` is a dependency-free Rust 2024 stub; no local TUI implementation was found.
- Wayland clients do not receive a general global keyboard hook; direct evdev access remains compositor-independent but requires a least-privilege device-access policy.
- The systemd user service owns the external `wayvibes` child process. The TUI keeps configuration changes in memory and applies them once after the TUI exits; closing the TUI leaves the backend running. On activation, unmanaged `wayvibes` processes are stopped before the unit starts. Smoke tests use a fake backend/unit where possible.
- The manager also performs read-only evdev keyboard discovery for the device-selection view; it never consumes keyboard events.
- The TUI keeps a persistent shortcut footer, marks the focused panel with `[ACTIVE]`, and opens a centered `?` help modal using Ratatui `Clear` overlay semantics.
- Soundpacks use configured directories plus the portable user data root `~/.local/share/wayvibes-tui/soundpacks` on Linux. When no explicit root is provided, the private repository's reviewed `sounds/` directory is also scanned. The manager validates compatible `config.json` files and referenced audio files before restart, and persists the selected path and volume. Bundled assets preserve their original licenses.
- The first release accepts the external `wayvibes` miniaudio playback behavior unchanged. Volume is clamped to `0.0`–`10.0` and rendered with a Ratatui progress gauge.
- The systemd user service is the singleton/persistence boundary: closing the TUI leaves the backend active, and unexpected backend exits are restarted.
- This repository is private and personal; it is not intended for sale, publication, or distribution. Bundled sound assets remain accompanied by their original licenses.
- Implementation stack: Rust 2024 with Ratatui and Crossterm. Implementation gates: `cargo fmt -- --check`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo build --release`.

## Decisions taken
- [[01-backend-ownership]] — Keep `wayvibes` as the runtime backend and manage it as an external child process.
- [[02-control-contract]] — Control the persistent backend through the systemd user service and restart the same singleton unit after configuration changes.
- [[03-language-tui-stack]] — Use Rust 2024 with Ratatui and Crossterm for the TUI/process manager.
- [[04-input-access]] — Retain direct evdev/libevdev capture in external `wayvibes` and add read-only keyboard-device discovery in the manager.
- [[05-device-permissions]] — Document `/dev/input` access as a prerequisite, recommend the `input` group where needed, and never require root.
- [[06-audio-latency]] — Accept the external `wayvibes` miniaudio behavior unchanged in the first release.
- [[07-soundpack-model]] — Reference configured Mechvibes-compatible soundpack paths, scan the private repository's reviewed `sounds/` directory by default, validate packs before restart, and persist the selected path/volume in manager configuration.
- [[08-service-lifecycle]] — Use one persistent systemd user service as the backend owner; closing the TUI leaves it running.
- [[09-upstream-attribution]] — Treat `wayvibes` as an external runtime dependency, link upstream, and bundle the reviewed local soundpacks with their original licenses.

## Fog

## Out of scope
- Implementing the TUI or process manager during the decision session.
- Rewriting or modifying `/home/smaniotov/Documents/external/wayvibes`.
- Solving compositor-specific permission setup for every Wayland distribution in the first milestone.
- Adding new soundpacks without pack-by-pack completeness and license review.
- Mouse events, a graphical desktop UI, cloud synchronization, and soundpack marketplace features.
