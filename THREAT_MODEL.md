# Threat Model

`udu` reads every keystroke from `/dev/input`. That fact cannot be softened, so this
document does the opposite: it states exactly what the program touches, what leaves each
boundary, which attackers it does and does not defend against, and how you can check the
claims without trusting the author.

Reporting and disclosure live in [`SECURITY.md`](SECURITY.md).

## Assets

Ordered by what an attacker would actually want.

1. **The keystroke stream.** Every key you press on the selected keyboard, including
   passwords, sudo prompts, and anything typed into any window. This is the asset. Every
   other item on this list is a rounding error next to it.
2. **Keyboard activity metadata.** Not what you typed, but when you were typing and how
   much. Enough to infer presence, work hours, and idle periods.
3. **Configuration** (`~/.config/udu/config.json`) — soundpack roots, selected device
   name, volume. Low value alone; the device name discloses your keyboard model.
4. **The control socket** (`$XDG_RUNTIME_DIR/udu.sock`) — an interface that can change
   what the daemon plays and reads.
5. **The audio output path.** Abusable for nuisance rather than disclosure.

## Trust boundaries

```
  [1] kernel input subsystem
        |  evdev InputEvent structs (type, code, value)
        v
  [2] evdev file descriptor -- one device, opened by name
        |  KeyEvent { code: u16, kind: Press | Release }
        v
  [3] daemon process (udu --service)
        |            \
        |             `--> [4] mapping lookup -> PathBuf -> audio -> DROPPED
        |
        |  control protocol (JSON lines)
        v
  [5] $XDG_RUNTIME_DIR/udu.sock
        |
        v
  [6] TUI client                    [7] disk: config.json, stats.json
```

### What crosses each boundary — and what does not

| Boundary | Crosses | Does **not** cross |
| --- | --- | --- |
| **[1] → [2]** kernel to fd | `InputEvent` for the **one** keyboard opened by name | Events from any other input device. `udu` opens a single device resolved from the configured name (`src/backend/capture.rs`) |
| **[2] → [3]** fd to daemon | `KeyEvent { code, kind }`, filtered to `EventType::KEY` with value `1` (press) or `0` (release) | Key repeats (value `2`) and every non-key event type are discarded in `push_events` |
| **[3] → [4]** keycode lifetime | The keycode is looked up in a `BTreeMap<u16, PathBuf>` (`src/backend/mapping.rs`), yields a sample path, and is dropped | The keycode reaches no file, no socket, and no output stream. Its entire lifetime spans three files: `capture.rs`, `mapping.rs`, `mod.rs` |
| **[3] → stderr** | `eprintln!("keyboard capture error: {error}")`. The `CaptureError` variants format a **device path** and an **errno** | No variant carries a keycode. There is no logging framework in the dependency tree |
| **[3] → [5] → [6]** daemon to TUI | `Request { cmd, path, value, name }`; `Response { ok, error, status, stats, exported }`. `stats` and `exported` carry the four aggregate counters | **No key field exists in the protocol.** No individual key event, and no ordering, is representable |
| **[3] → [7]** daemon to disk | `config.json` (settings); `stats.json` with `keystrokes: u64`, `dings: u64`, `since: String`, `per_switch: BTreeMap<String, u64>` keyed by **soundpack name** | No per-key counters, no per-key timestamps, no ordering. Typed text cannot be reconstructed from `stats.json` |
| **[3] → network** | Nothing | No network client crate is present. The generated systemd unit sets `RestrictAddressFamilies=AF_UNIX`, so the kernel refuses `AF_INET`/`AF_INET6` socket creation |

## Adversaries considered

**In scope.**

- **A process running as the same user, without `/dev/input` access.** This is the
  meaningful same-user case. It may try to reach the control socket, read
  `~/.local/share/udu/`, or observe file metadata. `udu` should not hand it anything it
  could not get on its own.
- **A malicious or malformed soundpack.** Untrusted JSON and untrusted audio bytes are
  parsed by `serde_json` and `symphonia`. A soundpack should not be able to read or play
  files outside its own directory, escape the configured roots, or achieve code
  execution. Manifests are rejected if an audio reference is absolute or contains a `..`
  component (`check_audio_reference` in `src/soundpack.rs`).
- **A hostile control-socket client.** Anything connecting to
  `$XDG_RUNTIME_DIR/udu.sock` — wrong uid, oversized payload, malformed JSON, a
  connection opened and left to stall.
- **Someone with read access to the user's home directory.** A backup service, a synced
  directory, a shared or badly permissioned home, a recovered disk. They should learn
  aggregate counts at most, never content.

**Explicitly out of scope.**

- **An attacker who already has root.** Root reads `/dev/input`, `/proc/<pid>/mem`, and
  your keyring directly. No user-space daemon changes that.
- **An attacker who already runs code as you *with* the ability to read `/dev/input`.**
  This is the boundary that matters most, so it is stated bluntly: at that point they
  open the evdev device themselves and read every keystroke system-wide. They do not
  need `udu`, are not helped by `udu`, and are not stopped by `udu`. Every mitigation in
  this document is scoped to attackers who lack that access.
- **Physical attackers**, hardware keyloggers, and compromised firmware.
- **The kernel and the audio stack.** `udu` trusts evdev and PipeWire/ALSA via `cpal`.

## Residual risks

Named without euphemism. These are known, not hidden.

### 1. `stats.json` mtime is a coarse keyboard-activity timeline

`~/.local/share/udu/stats.json` is rewritten when a keystroke or ding arrives at least
five seconds after the last write (`SAVE_INTERVAL` in `src/backend/stats.rs`). Its mtime
therefore records roughly when you were at the keyboard, at about five-second
resolution. An attacker who can poll the file also reads the cumulative `keystrokes`
counter, which turns repeated polling into a typing-*rate* timeline.

This is metadata, not content. Five-second aggregation is far too coarse for the
inter-keystroke timing analysis that recovers typed text, and the file holds no
ordering. But "when you were typing, and roughly how fast" does leak, and calling it
anything else would be dishonest. If that matters to your threat model, do not run the
stats-writing daemon, or place the data directory somewhere unreadable and unbacked-up.

### 2. File permissions follow the umask, and are never set explicitly

`udu` calls `fs::write` and does not `chmod` anything. Consequences:

- Under the generated systemd unit, `UMask=0077` yields `0600` for newly created files.
- Started from a shell with a typical `umask 022`, new files land at `0644` —
  world-readable.
- **Rewriting an existing file does not change its mode.** A `stats.json` first created
  at `0644` stays `0644` after you switch to the systemd unit.

Check yours: `stat -c '%a %n' ~/.local/share/udu/stats.json ~/.config/udu/config.json`.
Fix with `chmod 600` if it is not already.

### 3. `input` group membership is a permanent, system-wide grant

The project mentions `sudo usermod -aG input $USER` as a way to get device access.
Understand what it does: it grants that user read access to **all** input devices,
permanently. Any process running as that user can then read every keystroke
system-wide — including sudo prompts, password managers, and full-disk-encryption
passphrases typed at a graphical prompt. It is not scoped to `udu`, it is not scoped to
one keyboard, and it survives uninstalling `udu`.

The narrower alternative is a udev rule matching one device with `TAG+="uaccess"`, which
grants access to the user at the active seat for that device only.

This is a system configuration decision, and its blast radius is the largest security
consequence of running `udu`. `udu` does not create it, cannot reduce it, and does not
claim to.

### 4. The control socket relies on directory permissions as its first line

`$XDG_RUNTIME_DIR` is mode `0700`, which is what primarily keeps other users out. `udu`
does not set an explicit mode on the socket file itself, so its mode also follows the
umask. The `SO_PEERCRED` check is the backstop: the daemon reads the peer's uid and
drops any connection whose uid differs from its own euid
(`peer_uid_matches` in `src/backend/control.rs`).

A same-uid process can still connect, and can also race the stale-socket cleanup in
`bind()` to take over the control endpoint. Both are accepted: they fall inside the
same-user boundary declared out of scope above.

### 5. The audio decode path is not sandboxed beyond the systemd unit

A soundpack hands untrusted bytes to `symphonia`. Path confinement is enforced, but a
decoder bug is a decoder bug. `MemoryDenyWriteExecute`, `SystemCallFilter=@system-service`,
and `NoNewPrivileges` raise the cost of exploiting one; they do not eliminate it. Install
soundpacks from sources you trust.

### 6. The systemd unit does not confine the filesystem

Current hardening in the generated unit (`render_service_unit` in `src/service.rs`):
`NoNewPrivileges`, `SystemCallFilter=@system-service`, `SystemCallArchitectures=native`,
`RestrictAddressFamilies=AF_UNIX`, `RestrictSUIDSGID`, `LockPersonality`,
`MemoryDenyWriteExecute`, `ProtectKernelTunables`, `ProtectKernelModules`,
`ProtectControlGroups`, `PrivateTmp`, `UMask=0077`.

Deliberately absent: `ProtectHome` and `ProtectSystem` (the daemon must write
`~/.config/udu` and `~/.local/share/udu`, and read soundpack roots), and any
`DeviceAllow` policy (it must open `/dev/input`). Not yet applied, and reasonable to
add: `ProtectProc`, `ProtectHostname`, `ProtectClock`, `RestrictNamespaces`,
`RestrictRealtime`, `PrivateUsers`. The practical effect is that the daemon can read
your home directory as you can. Run `systemd-analyze security udu.service` and judge for
yourself.

## Verifying these claims yourself

Do not trust the table above. Run these.

**The device is genuinely closed while muted.** Find your device number, then compare
the two states:

```sh
lsof /dev/input/eventN        # unmuted: the udu process appears
# toggle sound off in the TUI, wait ~1s
lsof /dev/input/eventN        # muted: no udu process
```

Muting sets an atomic flag; the capture loop then drops the `Capture` value, closing the
file descriptor, and polls every 100 ms (`MUTE_POLL_MS` in `src/backend/mod.rs`). Allow
a moment for the loop to notice.

**No network activity.**

```sh
ss -xp | grep udu             # unix sockets only
ss -tunp | grep udu           # expect no output at all
strace -f -e trace=socket,connect,sendto -p "$(pgrep -x udu)"
```

**No network or logging crate in the tree.**

```sh
cargo tree | grep -iE 'reqwest|hyper|curl|ureq|tokio|log|tracing|slog|sentry'
```

**Nothing keystroke-shaped in the binary.**

```sh
strings "$(command -v udu)" | grep -iE 'https?://|api\.|token|telemetry'
```

This is weak evidence on its own — a determined exfiltrator would not leave a URL in
`.rodata` — but it is cheap, and combined with `cargo tree` and `strace` it is hard to
reconcile with a hidden client.

**Read the stats code and the stats file.** `src/backend/stats.rs` is 151 lines,
including tests. Read it in full, then look at what it actually wrote:

```sh
cat ~/.local/share/udu/stats.json
stat -c '%a %n' ~/.local/share/udu/stats.json
```

**Enumerate every output call in the crate.**

```sh
grep -rn 'eprintln!\|println!\|dbg!' src/
```

Every hit is inspectable. Confirm for yourself that none of them formats a keycode.

**Audit the systemd unit.**

```sh
systemd-analyze security udu.service
systemctl --user cat udu.service
```

If any of these checks contradicts this document, that is a security report — see
[`SECURITY.md`](SECURITY.md). It is the most valuable kind we can receive.
