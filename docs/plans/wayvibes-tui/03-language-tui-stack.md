---
type: decision
status: closed
blocked_by: [01, 02]
---

# What language and TUI stack should the project use?

## Evidence
| Fact | Verified at |
|---|---|
| The local Rust project uses edition 2024 but has no dependencies or reusable TUI code. | `/home/smaniotov/Documents/personal/klack-manual/Cargo.toml:1-6`; `/home/smaniotov/Documents/personal/klack-manual/src/main.rs:1-3` |
| No local `klack-tui` project or other TUI implementation was found. | Local workspace search |
| Context7 resolves Ratatui documentation for v0.29.0/v0.30.0 and Crossterm documentation. | Context7 capability-first resolution performed during charting |
| The external backend is C++17 with a vendored C audio header and libevdev/nlohmann-json dependencies. | `/home/smaniotov/Documents/external/wayvibes/CMakeLists.txt`; `/home/smaniotov/Documents/external/wayvibes/src` |

## Choice
Use Rust 2024 with Ratatui and Crossterm for the TUI and process manager. The application will launch and manage the external C++ `wayvibes` executable rather than binding to its internals.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Rust for the sound backend and TUI | The project will keep the external `wayvibes` backend; migrating it would add scope without improving the process-manager design. |
| C++ for the TUI and process manager | The selected Rust TUI ecosystem provides the intended interface stack while keeping the manager independent from the backend implementation language. |

## Consequence
The repository will use Cargo and Rust tests for the manager/TUI. Ratatui and Crossterm versions and APIs must be checked through versioned documentation before implementation. The external `wayvibes` binary remains a runtime prerequisite or configurable executable path.
