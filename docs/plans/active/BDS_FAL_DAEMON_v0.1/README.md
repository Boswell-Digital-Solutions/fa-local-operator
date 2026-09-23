# BDS-FAL-DAEMON-v0.1 — FA Local Daemon and Capability-Registry Lookup

**Status:** Scope ratified 2026-09-23 (OD-1 through OD-4, all resolved). Still documentation-only —
no code in any repo is authorized by this plan set yet. Next step: `02_IMPLEMENTATION_SCOPING_PACKET.md`.
**Owner repository:** `Boswell-Digital-Solutions/fa-local-operator`
**Scope:** Repository (`fa-local-operator` only — OD-1 ruled out the `forge-df-local-foundation`
dependency the original draft proposed; `Boswell-Digital-Solutions/Forge_Command` is the motivating
downstream consumer, not an implementer under this plan)
**Governing doctrine:** `forge-local-systems-runtime/BOUNDARIES.md`, `forge-local-systems-runtime/DECISIONS/0005-falocal-boundary.md`
**Registration:** OD-4 ruled yes — to be registered in `docs/canonical/plan_registry_v1.json` as a
follow-up action.

## Why this exists

`Boswell-Digital-Solutions/Forge_Command`'s Living Topology Assurance plan
(`BDS-FC-LIVING-TOPOLOGY-ASSURANCE-002`) ruled DR-039/DR-040: no live, queryable capability
registry exists in either FA Local repo, so its CP4 shadow-proposal machinery can only ever
produce `ineligible: missing capability`, and CP5 (real execution) cannot begin at all. FA
Local's own `ROADMAP.md`, `CLAUDE.md`, and `doc/system/00_overview/01-overview-charter.md`
independently and consistently list "a daemon or API surface" as not yet delivered and state it
"would need an explicit, separately-authorized architectural decision." This plan is that
decision packet — a code-verified current-state/doctrine check plus a proposed design, not an
implementation authorization.

## Package contents

| File | Purpose |
|---|---|
| `README.md` | Identity and navigation |
| `01_CURRENT_STATE_DOCTRINE_AND_PROPOSAL.md` | Code-verified current state, doctrine-compatibility check, and the ratified architecture (OD-1 through OD-4 resolved) |
| `02_IMPLEMENTATION_SCOPING_PACKET.md` | Not yet written. Exact route shape, exact file allowlist, exact test allowlist, and the authorization text that would actually permit writing code |

## Explicit non-goals

- No execution-over-HTTP route. `route`/`execute` stay CLI-only.
- No write-capable route of any kind.
- No change to `admit_execution_request`'s pure, I/O-free semantics.
- No durable state inside FA Local's own process or a FA-Local-owned database (forbidden by `forge-local-systems-runtime/BOUNDARIES.md`'s "FA Local does not own: hidden persistence authority").
- No implementation in `forge-df-local-foundation`, `Forge_Command`, or anywhere else.
