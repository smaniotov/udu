---
type: decision
status: closed
blocked_by: [01]
---

# Replace the hand-rolled CLI parser with clap 4 derive

## Evidence

| Fact | Verified at |
|---|---|
| clap 4 derive is the canonical CLI pattern; `#[derive(Parser)]` + `#[command(name=…, version, about)]`; repeatable `--root` via `Vec<PathBuf>`; parse errors and `-h` come free. | clap derive tutorial/Parser docs, clap 4.6.x (primary) |
| clap defaults the command name to the **package name** (`wayvibes-tui`); must set `#[command(name = "klack")]` explicitly or help/`Usage:` changes. | clap `_derive` attribute docs (verified default) |
| lexopt self-limits: not for apps that care about help text/unicode error handling; our CLI is user-facing (`--device-name` needs unicode). | lexopt README "Why not" |
| The parser is ~120 lines incl. tests; hand-rolling is only defensible for frozen surface + sacred dep count — neither holds (serde already present). | local `src/cli.rs`; rust-cli book guidance |

## Choice

`cargo add clap --features derive` (4.6.x); derive `CliOptions` with `#[command(name = "klack",
version, about)]`; keep every current option (`--config`, `--wayvibes`, `--root*`,
`--soundpack`, `--device-name`, `--service`, `-h/--help`); retarget the parse tests at
`try_parse_from`; delete the hand-written parser and `CliError`.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| lexopt | No generated help, no unicode guarantees, marginal line savings |
| Keep hand-rolled parser | The systemic manual-CLI problem this refactor removes; grown help/errors are the default with clap |

## Consequence

`-h/--help`/`--version` become generated; missing-value and unknown-option errors keep
user-facing behavior (tests assert them). `tests/cli.rs` (assert_cmd) becomes possible in a
later phase. One new dependency, Apache-2.0, active.