# BDS-FAL-DAEMON-v0.1 — FA Local Daemon and Capability-Registry Lookup

**Status:** Implemented and closed out, 2026-09-24. `03_CLOSEOUT.md` re-verifies all 12 test-allowlist
cases against `fa-local-operator@f53e34f` (current `master`), accounts for the real port collision
(`8011`→`8012`, see below) `#29` fixed, and files one unrelated pre-existing flaky test found during
verification (`KI-FLO-20260924-004`, non-blocking). `#27` implemented the packet
(`fa-local-run serve`, one read-only route, default-off); `#29` moved the default port to `8012`
after `8011` turned out to already be live-bound by `context-runtime`/ForgeMath, undocumented in
`PORT_REGISTRY.md` at claim time; `#30` swept stale `doc/system`/`CLAUDE.md` text. Registered in the
canonical registry as `implemented_unverified` (not `verified_complete` — that status requires a
Drive-archived closeout package per this registry's own established convention, which this closeout
does not attempt; the evidence lives in-repo in `03_CLOSEOUT.md` instead).
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
| `03_CLOSEOUT.md` | Independent re-verification against merged `master`, the port-collision writeup, and accepted limitations |

## Explicit non-goals

- No execution-over-HTTP route. `route`/`execute` stay CLI-only.
- No write-capable route of any kind.
- No change to `admit_execution_request`'s pure, I/O-free semantics.
- No durable state inside FA Local's own process or a FA-Local-owned database (forbidden by `forge-local-systems-runtime/BOUNDARIES.md`'s "FA Local does not own: hidden persistence authority").
- No implementation in `forge-df-local-foundation`, `Forge_Command`, or anywhere else.
