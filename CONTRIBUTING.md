# Contributing to udu

Thanks for taking an interest in `udu`. Read this before you open an issue or a PR
— it will save you time either way.

## This is a solo-maintainer project

`udu` is maintained by one person (Vinícius Smanioto), in spare time. That shapes
how contributions work here:

- **Open an issue before any non-trivial PR.** "Non-trivial" means anything that
  adds a feature, changes behavior, touches the control protocol, the mapping
  contract, or the systemd unit, or moves more than a handful of lines. Describe
  the problem and your proposed approach first, and wait for a response before
  writing code. An unsolicited PR that skips this step risks being closed even if
  the code is good, simply because nobody asked for the change and no design
  conversation happened.
- **Small fixes can go straight to a PR.** Typos, obvious bugs with an evident
  one-line fix, broken links, CI fixes — open the PR directly.
- **Review may be slow.** One person reads every PR around a day job. There is no
  service-level agreement on response time, and none will be promised. Silence
  does not mean rejection; it means the maintainer hasn't gotten to it yet.

### What will and will not be accepted

- **New features** need a use case discussed in an issue first, and ideally fit
  the curated Klack-parity roadmap described in the README. Features outside that
  scope are unlikely to be merged, however well implemented.
- **Bug fixes** with a clear repro are welcome, PR directly.
- **Refactors with no behavior change** need a strong, stated reason (readability
  win, unblocks a real follow-up, fixes a measured performance problem). "I would
  have written it differently" is not a reason.
- **New dependencies are added reluctantly.** Fewer crates means less audit
  surface and fewer breakages to chase. If your change seems to need one, say why
  the standard library or an existing dependency isn't enough — in the issue,
  before the PR.
- **Soundpacks are never accepted into this repository.** `udu` loads third-party
  Mechvibes-format packs at runtime; it does not ship or vendor any.

## Development setup

```bash
cargo build
cargo test
cargo run -- <args>          # run the TUI against a debug build
```

A release build is what actually gets installed and run day to day:

```bash
cargo build --release
~/.local/bin/udu
```

See `README.md` for the runtime requirements (evdev read access, an audio output
device) and what the TUI controls do.

## Before you open a PR

Run the same checks CI runs:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

A PR that fails any of these will not be merged until it's green.

## Code style

Names and structure should carry the meaning of the code. This codebase uses
inline comments sparingly — please don't add comments to explain what a change
does; make the code (naming, small functions, structure) say it instead.

## Architectural context

Non-trivial areas of this codebase are backed by an ADR in `docs/adr/` — e.g. why
the backend is first-party and in-process, why control goes over a Unix socket,
how the evdev capture loop and its reconnect behavior work, and the Mechvibes
mapping contract. Read the relevant ADR before changing that area; if your change
contradicts a decision recorded there, say so explicitly in the issue.

## Commit conventions

- Imperative mood in the subject line ("Fix reconnect backoff", not "Fixed" or
  "Fixes").
- Explain **why** in the commit body when the reason isn't obvious from the diff
  alone — the diff already shows *what* changed.
- English only, for commits, code, and PR descriptions.

## Sign off your commits (DCO)

This project uses the [Developer Certificate of Origin](https://developercertificate.org/)
instead of a Contributor License Agreement. Sign every commit with:

```bash
git commit -s
```

This adds a `Signed-off-by:` trailer asserting you wrote the contribution, or
otherwise have the right to submit it, under the project's license. A DCO costs
nothing to use, requires no legal entity on either side, and doesn't ask
contributors to assign away any rights — it just records the same assurance a CLA
would, more simply.

## License

By contributing, you agree your contribution is licensed under the project's MIT
license (see `LICENSE`).
