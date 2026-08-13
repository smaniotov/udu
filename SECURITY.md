# Security Policy

`udu` reads every keystroke from `/dev/input` through evdev and runs as a persistent
systemd user service. That is the same shape as a keylogger. You should not take our
word that it is not one — this document, and [`THREAT_MODEL.md`](THREAT_MODEL.md),
exist to give you claims you can test instead of assurances you have to trust.

## Supported versions

`udu` is a pre-1.0 project maintained by one person. Only the latest release receives
security fixes. There are no backports and no long-term support branches.

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Anything older | No — upgrade to the latest release |

Current version: `0.1.0`. Until a `1.0` release exists, the public API, the config
format, and the control protocol may change between releases.

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

The primary channel is GitHub private vulnerability reporting:

1. Go to <https://github.com/smaniotov/udu>
2. Open the **Security** tab
3. Click **Report a vulnerability**

This creates a private advisory visible only to you and the maintainer.

If GitHub private reporting is unavailable to you, email **smaniotov@gmail.com** with
`udu security` in the subject. Email is a fallback, not the preferred path: it is not
encrypted and has no tracking.

A useful report includes the version or commit, your distribution and kernel, how `udu`
was installed and launched (systemd user unit, or foreground), reproduction steps, and
what an attacker gains. A proof of concept helps more than a description of one.

## What to expect

- **Initial response: within 14 days.** This is a solo-maintainer project, so this is a
  deliberately conservative commitment rather than a courtesy figure that gets missed.
  If you have not heard back in 14 days, escalate by opening a public issue that says a
  security report is awaiting response — with no technical detail in it.
- After the initial response, expect an assessment of whether the report is in scope and
  a rough remediation timeline. Both are communicated in the advisory thread.
- Reports that turn out to be out of scope get an explanation of why, not silence.
- There is no bug bounty. Credit in the advisory and the release notes is offered by
  default and can be declined.

## Scope

A report is in scope if it lets an attacker do something they could not already do,
given the trust boundaries in [`THREAT_MODEL.md`](THREAT_MODEL.md). Concretely:

**In scope**

- Any path by which a keycode, or data from which typed content could be reconstructed,
  leaves the process — written to disk, emitted to a log or stderr, or sent over the
  control socket.
- Any network activity at all. `udu` has no reason to open a socket to anything but the
  local control endpoint.
- A control-socket client with a **different uid** succeeding at any request, or causing
  the daemon to crash, hang, or consume unbounded memory.
- Reading or playing a file outside the configured `soundpack_roots` via the control
  socket or a crafted soundpack `config.json`.
- A malicious soundpack achieving code execution, memory-unsafe behaviour, or a crash
  loop through the audio decode path.
- Privilege escalation, or any escape from the hardening in the generated systemd unit.
- Files or directories created with permissions broader than the process umask implies.

**Out of scope**

- Anything that requires the attacker to already run code as the same user with read
  access to `/dev/input`. At that point they can open the evdev device themselves and
  read your keystrokes directly, without involving `udu`. `udu` is not a defence against
  a same-user attacker who already has input access, and does not claim to be.
- Anything that requires root. Root reads `/dev/input` and your memory regardless.
- The blast radius of adding your user to the `input` group. That is a documented and
  deliberately surfaced consequence of a system configuration choice, not a flaw in
  `udu` — see "Input group" in [`THREAT_MODEL.md`](THREAT_MODEL.md). A report showing
  that `udu` makes this *worse* than the group membership alone is in scope.
- Denial of service against yourself (killing your own daemon, filling your own disk).
- Vulnerabilities in dependencies that `udu` does not reach. Report the reachable path,
  not just the advisory ID.
- Missing hardening that has no demonstrated impact, absent an attack it would stop.

## Coordinated disclosure

1. You report privately through one of the channels above.
2. The report is triaged and confirmed or rejected, with reasoning either way.
3. If confirmed, a fix is developed in a private fork or a private advisory branch.
4. A GitHub Security Advisory is published with a CVE requested where warranted, at the
   same time as the release carrying the fix.
5. You are credited unless you ask not to be.

The default embargo is **90 days** from the initial report. If a fix is ready earlier,
disclosure happens earlier. If the issue is being actively exploited, the fix ships
first and the advisory follows immediately. You are welcome to disclose after 90 days
whether or not a fix exists — please tell the maintainer when you intend to.

## What udu promises, and what it does not

Every promise below is written so you can check it. The commands are in
[`THREAT_MODEL.md`](THREAT_MODEL.md#verifying-these-claims-yourself).

### Promises

- **No keystroke is logged.** The only capture-related output the crate produces is
  `eprintln!("keyboard capture error: {error}")` in `src/backend/mod.rs`. The
  `CaptureError` variants in `src/backend/capture.rs` format a device path and an
  errno. No variant carries a keycode.
- **No keystroke is persisted.** `src/backend/stats.rs` stores exactly four fields:
  `keystrokes: u64`, `dings: u64`, `since: String`, and
  `per_switch: BTreeMap<String, u64>` — keyed by **soundpack name**, not by keycode.
  There are no per-key counters, no timestamps, and no ordering. Typed text cannot be
  reconstructed from that file.
- **No key identity crosses the control socket.** The `Request` type carries
  `cmd`, `path`, `value`, and `name`. There is no key field. Responses carry the
  aggregate counters above and never individual events.
- **No network client is in the dependency tree.** The full dependency list is in
  `Cargo.toml`, and none of `anyhow`, `clap`, `cpal`, `dirs`, `evdev`, `fastrand`,
  `libc`, `ratatui`, `serde`, `serde_json`, `symphonia`, `thiserror` is a network or
  logging client. The generated systemd unit sets `RestrictAddressFamilies=AF_UNIX`,
  so the kernel refuses `AF_INET`/`AF_INET6` socket creation for the service.
- **No logging framework is in the dependency tree.** There is no `log`, `tracing`,
  `slog`, or equivalent. Output is `eprintln!` only, and you can enumerate every call.
- **Muting closes the device.** Toggling sound off does not merely silence audio; the
  capture loop drops the `Capture` value, which closes the evdev file descriptor. While
  muted, `lsof /dev/input/eventN` shows nothing for the process. This is externally
  observable, which is the point of stating it.
- **Soundpack paths are confined.** The control socket canonicalizes any requested path
  and rejects it unless it resolves inside a configured `soundpack_root`
  (`resolve_within_roots` in `src/backend/control.rs`). Soundpack manifests additionally
  reject absolute paths and any `..` component.
- **The control socket rejects other users.** The daemon reads `SO_PEERCRED` and drops
  any connection whose uid differs from its own euid, caps a request at 64 KB, and sets
  a 5-second read and write timeout.

### Not promises

- **`udu` does not protect you from a same-user attacker.** Every mitigation here is
  scoped to *other* uids and to `udu`'s own behaviour. Another process running as you,
  with read access to `/dev/input`, does not need `udu` and is not stopped by it.
- **`udu` does not reduce the risk of `input` group membership.** Adding your user to
  the `input` group grants that user permanent read access to **all** input devices,
  which means any process running as you can read every keystroke system-wide, including
  sudo prompts and password fields. A udev rule scoped to one device with
  `TAG+="uaccess"` is the narrower alternative. This is a system configuration decision
  you make, and `udu` cannot undo it.
- **`udu` does not hide that you were typing.** `~/.local/share/udu/stats.json` is
  rewritten when typing resumes after a gap, so its mtime is a coarse record of when you
  were at the keyboard. This is metadata, not content — see "Residual risks" in
  [`THREAT_MODEL.md`](THREAT_MODEL.md).
- **`udu` does not set explicit file permissions.** It relies on the process umask.
  Under the generated systemd unit (`UMask=0077`) new files land at `0600`; started from
  a shell with a typical `umask 022` they land at `0644`, and rewriting an existing file
  does not change a mode it already has. Check the mode of your own files.
- **The soundpack decode path is not sandboxed beyond the systemd unit.** A soundpack is
  untrusted input handed to an audio decoder. Install soundpacks from sources you trust.
- **No formal audit, and no guarantee of timely fixes.** One maintainer, best effort,
  no SLA beyond the 14-day initial response above.
- **No supply-chain guarantee beyond the lockfile.** `Cargo.lock` is committed. A
  `cargo deny` policy banning network and logging crates is intended to enforce the
  dependency claims above in CI; until that lands, `cargo tree` is the check you can run
  today.
