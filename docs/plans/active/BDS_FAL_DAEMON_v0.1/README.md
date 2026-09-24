# BDS-FAL-DAEMON-v0.1 — FA Local Daemon and Capability-Registry Lookup

**Status:** Implemented; closeout pending. Scope ratified 2026-09-23 (OD-1 through OD-4).
`02_IMPLEMENTATION_SCOPING_PACKET.md` is fully authorized (2026-09-23). Its three open items (two
new Cargo dependencies, default port, `CLAUDE.md` edit safety) are resolved, and a 2026-09-24
amendment sets the default port to 8012. #27 implements the packet: `fa-local-run serve`, one
read-only route, default-off. #29 moves the default port to 8012, the port that the Forge
`PORT_REGISTRY.md` registers. No closeout record exists yet.
**Owner repository:** `Boswell-Digital-Solutions/fa-local-operator`
**Scope:** Repository (`fa-local-operator` only — OD-1 ruled out the `forge-df-local-foundation`
dependency the original draft proposed; `Boswell-Digital-Solutions/Forge_Command` is the motivating
downstream consumer, not an implementer under this plan)
**Governing doctrine:** `forge-local-systems-runtime/BOUNDARIES.md`, `forge-local-systems-runtime/DECISIONS/0005-falocal-boundary.md`
**Registration:** OD-4 ruled yes. The Forge `docs/canonical/plan_registry_v1.json` registers it
(forge#209), and forge#212 records the merged implementation.

## Why this exists

`Boswell-Digital-Solutions/Forge_Command`'s Living Topology Assurance plan
(`BDS-FC-LIVING-TOPOLOGY-ASSURANCE-002`) ruled DR-039/DR-040: no live, queryable capability
registry exists in either FA Local repo, so its CP4 shadow-proposal machinery can only ever
produce `ineligible: missing capability`, and CP5 (real execution) cannot begin at all. FA
Local's own `ROADMAP.md`, `CLAUDE.md`, and `doc/system/00_overview/01-overview-charter.md`
independently and consistently list "a daemon or API surface" as not yet delivered and state it
"would need an explicit, separately-authorized architectural decision." This plan is that
decision: `01_...md` is a code-verified current-state and doctrine check with a proposed design, and
`02_...md` authorizes the implementation.

## Package contents

| File | Purpose |
|---|---|
| `README.md` | Identity and navigation |
| `01_CURRENT_STATE_DOCTRINE_AND_PROPOSAL.md` | Code-verified current state, doctrine-compatibility check, and the ratified architecture (OD-1 through OD-4 resolved) |
| `02_IMPLEMENTATION_SCOPING_PACKET.md` | Fully authorized 2026-09-23 (port amended to 8012 on 2026-09-24). Exact route shape, exact file allowlist, exact test allowlist, and the accepted authorization text |

## Explicit non-goals

- No execution-over-HTTP route. `route`/`execute` stay CLI-only.
- No write-capable route of any kind.
- No change to `admit_execution_request`'s pure, I/O-free semantics.
- No durable state inside FA Local's own process or a FA-Local-owned database (forbidden by `forge-local-systems-runtime/BOUNDARIES.md`'s "FA Local does not own: hidden persistence authority").
- No implementation in `forge-df-local-foundation`, `Forge_Command`, or anywhere else.
