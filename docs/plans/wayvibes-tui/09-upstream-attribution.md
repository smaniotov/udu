---
type: task
status: closed
blocked_by: [01]
---

# What upstream material can the new project carry?

## Evidence
| Fact | Verified at |
|---|---|
| The local wayvibes clone has no top-level license file in the inspected inventory. | `/home/smaniotov/Documents/external/wayvibes` file inventory |
| Individual soundpack directories include their own `LICENSE.txt` files, so pack redistribution cannot be treated as one global license decision. | `/home/smaniotov/Documents/external/wayvibes/soundpacks` inventory |
| The project will invoke the upstream executable rather than copy its source, so source-code licensing is not part of the new repository's implementation boundary. | `01-backend-ownership.md` |
| The upstream repository is hosted at `https://github.com/sahaj-b/wayvibes` and has no top-level license file in the inspected `HEAD`. | `git -C /home/smaniotov/Documents/external/wayvibes remote -v`; `git ls-tree HEAD` |

## Conclusion
The new repository documents `wayvibes` as an external runtime dependency and links to its upstream repository. It does not copy upstream source. This is a private, personal, non-commercial project with no intention to sell, publish, or distribute the application. Three complete packs with explicit GPL-3 `LICENSE.txt` files were copied from the local clone into `sounds/`, preserving their licenses: Banana Split Lubed, Banana Split Stock, and MX Speed Silver. User-selected external packs remain the user's responsibility, including their individual licenses.

## Consequence
The README must state the runtime dependency, upstream URL, no-root/input-permission prerequisite, private-use scope, and the pack-by-pack asset review. Any future bundled pack requires the same completeness and explicit-license check.
