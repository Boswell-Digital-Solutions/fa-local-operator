# 01 — Current State, Doctrine Check, and Proposed Architecture

**Plan ID (proposed, not yet ratified):** `BDS-FAL-DAEMON-v0.1`
**Status:** Documentation-only. Proposes a design and lists open decisions. Does not authorize any code.
**Decision scope:** Read-only investigation and design, same discipline as `Forge_Command`'s
`11_CP3_PERSISTENCE_AND_REPLAY_AUTHORIZATION.md` (the closest precedent in this ecosystem for
"a service gains a new persistence dependency and a new cross-repo write surface").
**Motivating consumer:** `Boswell-Digital-Solutions/Forge_Command`,
`BDS-FC-LIVING-TOPOLOGY-ASSURANCE-002`, DR-039/DR-040. That plan does not authorize or scope any
part of this document — it only establishes why the gap matters. This document's own ratification
happens under `fa-local-operator`'s and `forge-local-systems-runtime`'s authority, not
Forge_Command's.

## Background

Forge_Command's CP4 (`13_CP4_RESUMPTION_PROPOSAL.md`, `14_CP4_S003_IMPLEMENTATION_PACKET.md`)
resumed shadow-proposal generation for exactly one strategy (S003) while leaving `capabilityId`
resolution hardcoded to a sentinel meaning "no capability available" (DR-043), because DR-040
found no live, queryable capability registry anywhere to check against. That finding is
re-verified here, against the current `fa-local-operator` checkout, before proposing anything.

## Current state (code-verified)

1. **FA Local is a CLI binary with no HTTP-serving surface.** Stated directly in
   `CLAUDE.md:42` ("FLO is a Rust CLI with no HTTP surface, so it cannot be supervised as a local
   HTTP service. Anything that needs to reach it over a port needs a different host process.")
   and confirmed by `doc/system/00_overview/01-overview-charter.md:99`, which lists "daemon or API
   surfaces" under "What is still intentionally not delivered."

2. **The capability registry is a per-invocation, caller-supplied JSON value — not a persisted or
   queryable store.** `schemas/capability-registry.schema.json` defines the contract shape
   (`registry_version`, `capabilities[]`, each with `capability_id`, `owner_service`,
   `capability_type`, `side_effect_class`, `approval_posture`, `allowed_requester_classes`, timeout
   and retry budgets, `enabled_state`, `review_class`). `CapabilityRegistryLoader::load_contract_value`
   (`src/domain/capabilities/mod.rs:88-90`) deserializes it fresh from a `serde_json::Value` the
   caller passes in — `decision_service.rs:37` and `execution_pipeline_service.rs:167` both call it
   this way, once per request, with no cache, file-watch, or backing store behind it.

3. **`admit_execution_request` is a pure, I/O-free function.** (`src/domain/capabilities/mod.rs`,
   the function starting at line ~98.) Given a registry, policy, requester, and request, it checks
   capability existence, `enabled_state`, `revocation_state`, `owner_service`, allowed requester
   classes, and side-effect-class match — entirely in memory, entirely deterministic, no network or
   disk access. There is nothing in this function a daemon would need to change; a daemon only
   needs to change how the `registry` argument is obtained.

4. **FA Local already calls out over HTTP as a client — twice, proven live.**
   `DfLocalAdapter::post_execution_status_event` (`src/integrations/df_local/mod.rs`) POSTs to
   `dataforge-Local`'s `/api/v1/execution-bridge/status-events`. `HttpNeuronForgeLocalAdapter`
   (`src/integrations/neuronforge_local/mod.rs`) POSTs to `neuronforge-local-operator`'s
   `/api/v1/fa-local/task-dispatch`. `doc/system/00_overview/01-overview-charter.md:93` cites
   `neuronforge-local-operator`'s own `ADR-002` transport section as establishing that FA Local
   dispatching *out* over HTTP does not conflict with its "no HTTP surface" doctrine, because that
   doctrine "constrains what it serves, not what it calls." Serving is the genuinely new,
   genuinely ungoverned surface this document is about.

5. **No plan for this exists yet anywhere.** `docs/canonical/plan_registry_v1.json` (the forge-root
   cross-repo registry) has zero entries with `owner_repository` containing "fa-local" — confirmed
   by direct query, not assumed. `BDS-FAL-NFL-ADMISSION-v0.1` (cited in `ROADMAP.md` and the
   overview-charter as prior related work) is about NeuronForge-Local *task* admission, a different
   question, and is also not present in that registry — it appears to exist only as a Drive plan
   set referenced from `ROADMAP.md`, not as a registered `fa-local-operator`-repo plan. This
   document does not attempt to reconcile that gap; it only notes that this plan, if ratified, would
   be a first for the repo.

## Doctrine check

`forge-local-systems-runtime/BOUNDARIES.md` is the accepted, binding ownership doctrine for the
local runtime layer (per that repo's own `CLAUDE.md`: "Ownership boundaries here are binding on the
services they govern"). Checked directly, not assumed compatible:

| Doctrine clause | Bearing on this proposal |
|---|---|
| FA Local **owns**: "capability admission checks" | A read-only capability-registry lookup surface is inside this owned scope — it changes *how* the registry is obtained, not *what FA Local decides* with it. |
| FA Local **does not own**: "hidden persistence authority" | Durable capability-registry storage must not live inside FA Local's own process or database. This is the binding constraint on the persistence design below. |
| DF Local Foundation **owns**: "local database lifecycle, migrations, backup/restore/export doctrine, app registration conventions, readiness and health state, bounded recovery and integrity support" | The natural, doctrine-consistent home for durable capability-registry storage, if durability is wanted at all (see Open Decision OD-1). |
| `DECISIONS/0005-falocal-boundary.md` (ADR 0005): FA Local "must not become app semantic authority, durable semantic memory, hidden planner, or open-ended autonomous agent substrate" | A daemon that serves the same admission-shaped read FA Local's CLI already performs does not, by itself, cross into any of these. It would cross the line only if it grew into a write-capable or execution-triggering surface — explicitly excluded from this proposal's scope (see README's non-goals). |
| "Forbidden drift patterns": "FA Local drifting into open-ended autonomy or stealth orchestration monolith behavior" | Not implicated — this proposal adds a transport, not a decision-making capability FA Local doesn't already have. |

**Finding: a narrowly-scoped, read-only capability-registry lookup daemon is compatible with
`forge-local-systems-runtime`'s accepted boundary doctrine**, provided (a) it stays read-only, (b)
it does not become FA Local's own durable store, and (c) it is not extended into an
execution-triggering surface without a separate, later authorization.

## Proposed architecture (design only — not authorized)

Scoped narrowly to what Forge_Command's CP4 specifically needs, mirroring the narrow, dark-launch
discipline `Forge_Command`'s own CP5 AAR authorization already used for a comparable
first-cross-boundary slice (`BDS-NF-OVERNIGHT-SHAPING-001` CP5, `forge` PR #196):

1. **One new capability, not a general server.** A `fa-local-run serve` subcommand (or a small
   separate binary sharing the existing crate's domain/app layers — Open Decision OD-2), exposing
   exactly one route: a read-only capability-registry lookup by `capability_id`. No `route` or
   `execute` semantics move to this surface.
2. **Default-off.** A flag (e.g. `FA_LOCAL_SERVE_ENABLED`) gates whether the surface exists at all,
   matching this ecosystem's existing dark-launch convention (CP5's
   `NEUROFORGE_AAR_INTAKE_ENABLED`).
3. **Persistence lives outside FA Local.** If the registry needs to be durable and centrally
   queryable (rather than, say, still file-based but now read by a long-running process instead of
   per-CLI-invocation — see OD-1), it is proposed to be a new app-domain schema in
   `forge-df-local-foundation`, mirroring the "Foundation owns migration/lifecycle mechanics; the
   owning application owns domain schema and content" split Forge_Command's own CP3 work already
   established with the separate `dataforge-Local` repo (DR-029, a different repo but the same
   ecosystem-wide storage-mechanics-only pattern). **This is not yet verified against
   `forge-df-local-foundation`'s actual current schema/migration surface** — that verification is a
   precondition of ratifying this document, not a fact this document asserts (OD-1).
4. **Auth is an open decision with two live, real precedents in this ecosystem, not a default
   choice:**
   - **(a) Reuse Forge_Command's existing token authority.** Forge_Command already mints real
     Ed25519 JWS tokens (`POST /fc/token`, `src-tauri/src/token_authority.rs`, cited in
     Forge_Command's own `11_CP3_...md` Finding 4), and `dataforge-Local` already verifies them for
     a different scope. A new scope value (e.g. `capability:read`) would let FA Local's daemon
     verify the same live mechanism rather than build a new one.
   - **(b) Extend FA Local's own requester-trust model.** `DECISIONS/0002-requester-trust-model.md`
     and `schemas/requester-trust.schema.json` already define trust envelopes for local,
     file-argument-supplied callers; this would extend that model to a network-borne requester
     envelope.
   This document recommends (a) — it reuses a mechanism already proven twice in this ecosystem
   (`dataforge-Local`, and Forge_Command is this document's own motivating consumer) rather than
   stretching a model designed for trusted local file-argument callers across a new network
   boundary it was never scoped for — but does not rule it; see OD-3.
5. **Initial caller scope: Forge_Command's CP4 `capabilityId` resolution only.** No other consumer
   is in scope for a first slice. If ratified and built, this replaces DR-043's hardcoded sentinel
   in `strategy.ts` — that replacement is Forge_Command's own future implementation PR, authorized
   under Forge_Command's own plan, not this one.
6. **Not proposed:** any execution-over-HTTP route; any write-capable route; any change to
   `admit_execution_request`'s pure semantics; any change to how `route`/`execute` obtain a
   registry today (they keep the existing file-argument path unless a later document proposes
   otherwise).

## Open decisions for the operator

| # | Decision | Notes |
|---|---|---|
| OD-1 | Does the capability registry need to become durable/centrally-stored at all, or would a long-running daemon process that still reads a locally-configured file (just not re-read per CLI invocation) satisfy CP4/CP5's actual need? If durable, is `forge-df-local-foundation` confirmed (via a code-verified Finding, not assumed) as able to host a new app-domain schema the way this document proposes? | Unresolved. A Foundation-side current-state investigation, mirroring Forge_Command's own CP3 Findings 1–5, is a precondition of ratifying the persistence half of this design. |
| OD-2 | New `serve` subcommand on the existing `fa-local-run` binary, or a separate daemon binary sharing the crate's `domain`/`app` layers? | Unresolved. Affects packaging, supervision, and whether `fa-local-run`'s existing CLI-only framing (`CLAUDE.md`) needs updating. |
| OD-3 | Auth mechanism: reuse Forge_Command's Ed25519 token authority (recommended above), or extend FA Local's own requester-trust model? | Unresolved. |
| OD-4 | Should this plan be registered in `docs/canonical/plan_registry_v1.json` under `BDS-FAL-DAEMON-v0.1`, given `fa-local-operator` currently has none registered there at all despite the forge workspace's own plan-registry protocol? | Unresolved — noted, not decided, by drafting this folder. |

## Not authorized by this document

- Any code in `fa-local-operator`, `forge-df-local-foundation`, `Forge_Command`, or any other repo.
- Any execution-over-HTTP route (`route`/`execute` stay CLI-only).
- Any write-capable route.
- Any change to `admit_execution_request`'s pure, I/O-free semantics.
- Any Foundation-side schema or migration work — needs its own code-verified Finding first (OD-1).
- Any auth-mechanism implementation — needs OD-3 ruled first.
- Registration in the canonical plan registry — needs OD-4 ruled first.

## Proposed exact human authorization (for when the operator is ready to rule)

> Authorized: `BDS-FAL-DAEMON-v0.1`'s current-state and doctrine-compatibility findings above are
> accepted. [OD-1 through OD-4 rulings inserted here.] This authorizes a Foundation-side
> current-state investigation (Finding-style, `forge-df-local-foundation`) and, separately, a
> concrete implementation-scoping packet (exact routes, exact file allowlist, exact test allowlist)
> for whichever OD-1/OD-2/OD-3 shape is ruled — mirroring `Forge_Command`'s own
> `13_CP4_RESUMPTION_PROPOSAL.md` → `14_CP4_S003_IMPLEMENTATION_PACKET.md` two-step discipline. It
> does not itself authorize writing product code in any repo.

Any materially different scope requires a delta review and renewed authorization, consistent with
every other authorization packet in this ecosystem.

## Gate for this document

- [x] Every current-state claim cites a file path (and line numbers where read directly).
- [x] The doctrine check verifies against `forge-local-systems-runtime/BOUNDARIES.md` and
      `DECISIONS/0005-falocal-boundary.md` directly, not assumed compatible from FA Local's own
      framing alone.
- [x] The proposal is scoped narrowly (read-only, default-off, one consumer) rather than a general
      daemon, matching this ecosystem's existing narrow-first-slice discipline.
- [x] Every place this document is not certain (persistence necessity, Foundation's actual schema
      fit, auth mechanism, registry registration) is listed as an open decision, not assumed.
- [ ] Operator rules on OD-1 through OD-4.
- [ ] This document's proposed authorization text is accepted, amended, or rejected.
