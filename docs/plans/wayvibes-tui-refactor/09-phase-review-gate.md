---
type: decision
status: closed
blocked_by: []
---

# Isolated agent reviews each phase before approval

## Evidence

| Fact | Verified at |
|---|---|
| The user requires each phase to be approved by an isolated agent, in a loop, before the next phase. | explicit user instruction (this session) |
| Fresh-context read-only review agents catch what the writer cannot; the parent stays the decision-maker. | pi-subagents constraints; master-learning execution model |

## Choice

After each phase lands with gates green, launch a `reviewer` subagent (fresh context) with the
phase diff + this plan + the required acceptance; it returns approve or reject with findings.
On reject, the parent fixes within the phase's contract and re-reviews. Loop until approval.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Self-review | The writer is the worst auditor of its own work |
| Review only at the end | Phase-locality of defects lost; fixes cross phase boundaries |

## Consequence

Each phase is independently defensible; defects stay contained to the phase that introduced
them. Review artifacts are recorded in each phase commit message's context.