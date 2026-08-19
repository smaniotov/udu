# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/).

This project is currently `0.y.z`. Per SemVer 2.0.0 clause 4, the public
interface (CLI flags, control-socket protocol, config file format) is not yet
stable, and anything may change at any time without a major version bump.

## [0.1.1] - 2026-08-19

### Changed

- Searching for a soundpack now requires pressing `/` to enter search mode. While a
  search is active, keys that used to be shortcuts (`s`, `p`, `r`, `q`, `x`, `1/2/3`,
  `?`, `U`, `+/-`) are inserted into the query instead of firing their actions; `Esc`
  clears the search and restores the shortcuts. The header hints advertise `/ search`.

## [0.1.0] - 2026-08-13

Initial public release.

### Added

- First-party TUI and backend for playing mechanical-keyboard sounds on Linux,
  built as a from-scratch, first-party re-implementation of the observable
  feature surface of the macOS utility Klack.
- In-process capture, mapping, and audio backend: keystrokes are read directly
  from the Linux input subsystem (evdev), mapped through a Mechvibes-compatible
  keycode table, and played with a low-latency audio engine (cpal + symphonia).
- A persistent per-user `udu.service` systemd unit that keeps the backend
  running; the TUI controls it live over a Unix control socket, so switching
  soundpacks, devices, or volume never restarts the backend.
- Support for loading third-party Mechvibes-format soundpacks (packs are not
  bundled with this project); pack and file validation with graceful filtering
  of packs that reference missing audio files.
- Automatic reconnect with backoff when the selected keyboard device is
  unplugged.
- Press and release keystroke sounds with randomized pitch and velocity per
  key, peak-normalized sample loudness across packs, and a soft-knee limiter on
  the output bus to prevent clipping.
- Master volume control (0-100) with Soft/Balanced/Loud presets, live in the
  TUI (`+`/`=`, `-`, `1`/`2`/`3`).
- Ratatui-based TUI: soundpack and keyboard-device lists, an Audio settings
  form, live connection status, pack preview (`p`), mute/unmute (`x`), usage
  stats with Markdown export (`s`), a 2D Tone Pad for pan/distance (`[`/`]`,
  `;`/`'`), device/pack rediscovery (`r`), and a help modal (`?`).
- Onboarding guidance in the TUI for granting non-root read access to
  `/dev/input` (via the `input` group) when the session doesn't already provide
  it.
- Automatic migration of config and soundpacks from the project's earlier
  `wayvibes-tui` name on first launch, and retirement of the legacy
  `wayvibes-tui.service` / `klack.service` units.

### Security

- The `udu.service` systemd unit is hardened: `NoNewPrivileges`,
  `SystemCallFilter=@system-service`, `RestrictAddressFamilies=AF_UNIX`,
  `RestrictSUIDSGID`, `LockPersonality`, `MemoryDenyWriteExecute`,
  `ProtectKernelTunables`, `ProtectKernelModules`, `ProtectControlGroups`,
  `PrivateTmp`, and `UMask=0077`.
- Muting (`x` in the TUI, or `set_volume`/mute over the control socket) makes
  the backend release the keyboard device: it stops reading from the evdev
  source entirely and polls for the unmute instead of holding the device open.
  A muted `udu` is not capturing keystrokes.
- No soundpacks, telemetry, or network calls are bundled or made by this
  project; the only local IPC surface is the Unix control socket used by the
  TUI to talk to its own backend.
