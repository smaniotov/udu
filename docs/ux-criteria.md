# TUI beauty and usability criteria

This checklist defines the manual acceptance criteria for the private `wayvibes-tui` interface. It is intentionally observable: a reviewer can run the application in a terminal and verify each item without relying on implementation details.

## Visual hierarchy

- [ ] The header identifies the application and the Wayland keyboard-sound context.
- [ ] The focused panel is unmistakable without relying on color alone: it has `●` and `[ACTIVE]` in its title.
- [ ] The selected row has a `▸` indicator and strong text styling.
- [ ] The volume area is a visible horizontal progress gauge with a centered numeric label.
- [ ] Status feedback is separated from the shortcut footer.
- [ ] The footer stays pinned to the bottom and does not move when list content changes.
- [ ] The help modal is centered, bordered, readable, and visually above the underlying screen.
- [ ] The help modal does not leak characters or styles from the screen behind it.

## Interaction clarity

- [ ] `Tab` changes focus between Soundpacks and Keyboard devices.
- [ ] The active panel title changes immediately after `Tab`.
- [ ] `Up` and `Down` affect only the focused list.
- [ ] `Enter` activates the selected item in the focused list.
- [ ] `+`/`=` and `-` change the volume in steps of `0.5`.
- [ ] Volume never displays below `0.0` or above `10.0`.
- [ ] `?` opens the help modal from the main screen.
- [ ] While the modal is open, background shortcuts do not activate packs, change volume, or quit.
- [ ] `?` and `Esc` close the modal.
- [ ] `Esc` exits when no modal is open; `q` always exits.
- [ ] `r` refreshes both soundpacks and keyboard devices.

## Empty and error states

- [ ] An empty soundpack list explains the default directory to use.
- [ ] An empty device list explains that `/dev/input` permissions may be missing.
- [ ] A malformed pack reports the path and validation problem before restarting `wayvibes`.
- [ ] A missing or unreadable `wayvibes` executable reports an actionable startup error.
- [ ] Closing the TUI leaves `wayvibes-tui.service` active.
- [ ] `systemctl --user is-active wayvibes-tui.service` reports `active` after the TUI closes.
- [ ] Restarting the TUI/service leaves exactly one backend process running.
- [ ] The service unit contains `Restart=on-failure` and does not kill the backend during automatic restart.

## Manual test command

Use a private temporary configuration and a reviewed pack:

```bash
~/.local/bin/klack \
  --config /tmp/wayvibes-tui-manual/config.json \
  --wayvibes /path/to/wayvibes \
  --soundpack "$HOME/.local/share/wayvibes-tui/soundpacks/banana split lubed"
```

Verify the interaction checklist in a terminal at least 80 columns wide and 24 rows high. Repeat once at a narrower terminal size to confirm that the footer and modal remain readable. Do not run the manual test as root.

## Automated evidence

- `cargo fmt -- --check`
- `cargo test --release`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo build --release`
- `TestBackend` assertions cover visible volume text and help interaction state.
- A systemd smoke test verifies TUI exit persistence, singleton count, restart behavior, and `Restart=on-failure`.
