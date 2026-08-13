---
type: decision
status: closed
blocked_by: [02, 03, 05]
---

# How should the managed `wayvibes` process be supervised and installed?

## Evidence
| Fact | Verified at |
|---|---|
| The current external CLI implements background mode by forking, creating a session, and redirecting standard streams. | `/home/smaniotov/Documents/external/wayvibes/src/main.cpp:60-88` |
| The upstream README documents a NixOS home-manager service and manual restart after device changes, but no general user-service contract. | `/home/smaniotov/Documents/external/wayvibes/README.md:21-38,100-115` |
| The target environment provides a systemd user manager (`systemd 259`), and current systemd documentation defines user units, `Restart=on-failure`, `daemon-reload`, and `systemctl --user` lifecycle commands. | `systemctl --user --version`; systemd.service/systemctl documentation researched during implementation |

## Choice
Use a systemd user service as the persistent owner of the external `wayvibes` process. The TUI installs or updates one unit, calls `systemctl --user start/restart/stop/is-active`, and closes without stopping the service. systemd's unit identity provides the singleton guarantee; the service uses `Restart=on-failure` for unexpected backend exits.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| TUI-owned foreground child | It terminates when the TUI exits and cannot satisfy persistent audio playback. |
| Rely on `wayvibes --background` | The manager would lose reliable status, singleton ownership, and restart supervision. |

## Consequence
The first release targets Linux sessions with a working systemd user manager. The service unit and generated backend command become persistent user-session artifacts; changing soundpack, volume, or device writes configuration and restarts the same unit. Closing the TUI leaves `wayvibes` running.
