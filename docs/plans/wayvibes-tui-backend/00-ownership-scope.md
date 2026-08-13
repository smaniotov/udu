---
type: decision
status: closed
blocked_by: []
---

# Scope of "100% autoral": behavioral parity with improvements

## Evidence
| Fact | Verified at |
|---|---|
| User was asked (ask_user, single question, 3 options) how far the rewrite's scope goes: parity drop-in vs autoral-with-improvements vs architecture-only. | ask_user session record |
| User selected "Autoral com melhorias (recomendado)": base behavior identical to wayvibes (perfect Mechvibes mapping, EV_KEY value==1, volume 0-10) while fixing known failures: recovery/restart when the device vanishes, live reload of pack/volume without restart gap, decode caching, measured latency budget. | ask_user session record, option description verbatim |
| The rejected option ("Paridade total") would have preserved the silent dead-loop, restart-on-config-change, and per-press re-decode. | ask_user option list, session record |

## Choice
The backend replicates wayvibes' base observable behavior (event filter, mapping
contract, volume clamp, Mechvibes compatibility) but is allowed — and expected — to
improve: device-loss recovery, live reload without playback gap, decoded-audio
caching, and a measured performance budget. Improvement decisions must be explicit
per item (notes 02, 06, 07), never silent divergence.

## Rejected alternatives
| Alternative | Why not |
|---|---|
| Parity drop-in (bug-for-bug, including dead-loop, restart-on-change, per-press decode) | User chose the improvements option; keeps known defects without a need demonstrated |
| Architecture-only (no parity commitment, no improvement commitment) | Too loose; user wants the mapping contract and Mechvibes compatibility held exact |

## Consequence
Binds notes 02 (control contract → live reload direction), 06 (device resilience →
recovery direction), 07 (performance → measured budget direction). Mapping fidelity
(05) is resolved to strict validation + faithful runtime, upholding "mapeamento
Mechvibes perfeito".