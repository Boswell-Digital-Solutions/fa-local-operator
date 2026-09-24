# 01 — Current State, Doctrine Check, and Proposed Architecture

**Plan ID:** `BDS-FAL-DAEMON-v0.1`
**Status:** **Scope ratified 2026-09-23 (OD-1 through OD-4).** This document's design and open
decisions are accepted as amended below. This still does not authorize writing product code — the
next step is a separate implementation-scoping packet (`02_...md`), same two-step discipline as
`Forge_Command`'s own `13_...md` → `14_...md`.
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
| DF Local Foundation **owns**: "local database lifecycle, migrations, backup/restore/export doctrine, app registration conventions, readiness and health state, bounded recovery and integrity support" | Would be the doctrine-consistent home for durable capability-registry storage if durability were needed. **OD-1 resolved this is not needed for this slice** — moot for now, not ruled out for a future slice. |
| `DECISIONS/0005-falocal-boundary.md` (ADR 0005): FA Local "must not become app semantic authority, durable semantic memory, hidden planner, or open-ended autonomous agent substrate" | A daemon that serves the same admission-shaped read FA Local's CLI already performs does not, by itself, cross into any of these. It would cross the line only if it grew into a write-capable or execution-triggering surface — explicitly excluded from this proposal's scope (see README's non-goals). |
| "Forbidden drift patterns": "FA Local drifting into open-ended autonomy or stealth orchestration monolith behavior" | Not implicated — this proposal adds a transport, not a decision-making capability FA Local doesn't already have. |

**Finding: a narrowly-scoped, read-only capability-registry lookup daemon is compatible with
`forge-local-systems-runtime`'s accepted boundary doctrine**, provided (a) it stays read-only, (b)
it does not become FA Local's own durable store, and (c) it is not extended into an
execution-triggering surface without a separate, later authorization.

## Ratified architecture (OD-1 through OD-4 resolved, 2026-09-23)

Scoped narrowly to what Forge_Command's CP4 specifically needs, mirroring the narrow, dark-launch
discipline `Forge_Command`'s own CP5 AAR authorization already used for a comparable
first-cross-boundary slice (`BDS-NF-OVERNIGHT-SHAPING-001` CP5, `forge` PR #196):

1. **One new capability, not a general server.** A `fa-local-run serve` subcommand (OD-2 —
   resolved: subcommand on the existing binary, not a separate daemon binary; see rationale below),
   exposing exactly one route: a read-only capability-registry lookup by `capability_id`. No `route`
   or `execute` semantics move to this surface.
2. **Default-off.** A flag (e.g. `FA_LOCAL_SERVE_ENABLED`) gates whether the surface exists at all,
   matching this ecosystem's existing dark-launch convention (CP5's
   `NEUROFORGE_AAR_INTAKE_ENABLED`).
3. **In-memory, file-backed — no `forge-df-local-foundation` dependency (OD-1 — resolved).**
   DR-040's actual requirement is something "live and queryable" to check `capabilityId` eligibility
   against — not durable, multi-writer, centrally-persisted storage. The daemon loads and
   schema-validates a locally-configured `capability-registry.schema.json` file once at startup,
   holds it in memory, and re-reads it on `SIGHUP` or an equivalent `--watch` mechanism (exact
   mechanism is the implementation packet's call). This satisfies DR-040's blocking fact — a
   long-running process to query, instead of per-CLI-invocation file arguments — without opening
   the harder cross-repo `forge-df-local-foundation` schema/migration question at all. **The prior
   draft of this document proposed Foundation persistence as the default shape; that proposal is
   superseded by this ruling**, consistent with this ecosystem's own practice of correcting rather
   than silently carrying forward an unnecessary design (cf. `Forge_Command`'s own DR-030,
   superseded same-day by DR-032, for exactly this kind of self-caught over-design). Durable,
   centrally-persisted storage remains available as a later, separately-authorized slice if a real
   operational need for it — multiple independent writers, an audit trail, cross-restart durability
   beyond "re-read the file" — actually materializes; none has been demonstrated today.
4. **Auth: reuse Forge_Command's existing token authority (OD-3 — resolved).** Forge_Command
   already mints real Ed25519 JWS tokens (`POST /fc/token`, `src-tauri/src/token_authority.rs`,
   cited in Forge_Command's own `11_CP3_...md` Finding 4), and `dataforge-Local` already verifies
   them for a different scope. FA Local's daemon verifies the same live mechanism under a new scope
   value (e.g. `capability:read`) rather than building new auth machinery or extending
   `DECISIONS/0002-requester-trust-model.md`'s local, file-argument-supplied trust model to a
   network boundary it was never scoped for.
5. **Initial caller scope: Forge_Command's CP4 `capabilityId` resolution only.** No other consumer
   is in scope for a first slice. If built, this replaces DR-043's hardcoded sentinel in
   `strategy.ts` — that replacement is Forge_Command's own future implementation PR, authorized
   under Forge_Command's own plan, not this one.
6. **Not proposed:** any execution-over-HTTP route; any write-capable route; any change to
   `admit_execution_request`'s pure semantics; any change to how `route`/`execute` obtain a
   registry today (they keep the existing file-argument path unless a later document proposes
   otherwise); any `forge-df-local-foundation` work of any kind.

## Open decisions for the operator — all resolved 2026-09-23

| # | Decision | Resolution |
|---|---|---|
| OD-1 | Does the capability registry need to become durable/centrally-stored, or does a long-running process reading a locally-configured file satisfy CP4/CP5's actual need? | **RESOLVED — in-memory, file-backed only. No `forge-df-local-foundation` work.** DR-040 only requires something live and queryable; durability was undemonstrated need, not a real requirement. See "Ratified architecture" point 3. |
| OD-2 | New `serve` subcommand on the existing `fa-local-run` binary, or a separate daemon binary? | **RESOLVED — `serve` subcommand on `fa-local-run`.** Reuses the existing crate directly; consistent with `route`/`execute`/`status`/`canonical-status` already being subcommands on one binary. |
| OD-3 | Auth mechanism: reuse Forge_Command's Ed25519 token authority, or extend FA Local's own requester-trust model? | **RESOLVED — reuse Forge_Command's existing token authority**, new `capability:read` scope. See "Ratified architecture" point 4. |
| OD-4 | Register this plan in `docs/canonical/plan_registry_v1.json` as `BDS-FAL-DAEMON-v0.1`? | **RESOLVED — yes, register it.** This plan's registration is a follow-up action to this document, not a separate implementation. |

No open decisions remain in this document.

## Not authorized by this document

- Any code in `fa-local-operator`, `Forge_Command`, or any other repo.
- Any execution-over-HTTP route (`route`/`execute` stay CLI-only).
- Any write-capable route.
- Any change to `admit_execution_request`'s pure, I/O-free semantics.
- Any `forge-df-local-foundation` work of any kind (OD-1 ruled this out of scope entirely).
- Any auth-mechanism implementation.

The next authorized step is a concrete implementation-scoping packet (`02_...md`: exact route
shape, exact file allowlist, exact test allowlist) — mirroring `Forge_Command`'s own
`13_CP4_RESUMPTION_PROPOSAL.md` → `14_CP4_S003_IMPLEMENTATION_PACKET.md` two-step discipline. That
packet's own acceptance is what authorizes writing product code, not this document.

## Traceability

| Decision | Resolution recorded in |
|---|---|
| OD-1–OD-4 | This document, "Ratified architecture" and "Open decisions" sections, 2026-09-23 |

## Gate for this document

- [x] Every current-state claim cites a file path (and line numbers where read directly).
- [x] The doctrine check verifies against `forge-local-systems-runtime/BOUNDARIES.md` and
      `DECISIONS/0005-falocal-boundary.md` directly, not assumed compatible from FA Local's own
      framing alone.
- [x] The proposal is scoped narrowly (read-only, default-off, one consumer, no unnecessary
      persistence dependency) rather than a general daemon, matching this ecosystem's existing
      narrow-first-slice discipline.
- [x] The Foundation-persistence default in the original draft is explicitly superseded, not
      silently dropped, once OD-1 ruled it unnecessary.
- [x] Operator rules on OD-1 through OD-4 (2026-09-23, all resolved as above).
- [ ] Registered in `docs/canonical/plan_registry_v1.json` (OD-4) — follow-up action.
- [ ] `02_IMPLEMENTATION_SCOPING_PACKET.md` drafted and accepted before any code is written.
