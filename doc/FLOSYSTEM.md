# fa-local-operator — Compiled System Reference

**Designation:** FLO
**Document role:** Canonical compiled technical reference for the ForgeAgents-Local operator (FA-Local)
**Source:** `doc/system/`
**Build command:** `bash doc/system/BUILD.sh`
**Document version:** 2.0 (2026-06-19) — BDS canonical-compliance migration (7-group class-aware structure, truth classes, designation-bound fail-closed assembly, authored governance trio)
**Protocol:** BDS Documentation Protocol v2.0; BDS Repo Documentation System Canonical Compliance Standard

> **Generated artifact warning:** `doc/FLOSYSTEM.md` is assembled output. Edit the
> source modules under `doc/system/` and rebuild. Hand edits to the compiled
> artifact are overwritten by the next build.

Assembly contract:

- Command: `bash doc/system/BUILD.sh`
- Validation: `bash doc/system/validate_snapshots.sh` runs during assembly
- Primary output: `doc/FLOSYSTEM.md`

This `doc/system/` tree is the canonical source of truth for fa-local-operator. It
uses explicit **truth classes**: *canonical facts* define the routing role, swarm
boundaries, contract-surface conformance, and bounded-worker discipline;
*snapshot facts* are dated, audit-derived counts (modules, tests, slices).
fa-local-operator is the FA-Local routing Gnat in the local Cortex swarm. See §5
for the scope/authority boundary and §6 for ownership and designation doctrine.

| Part | File | Contents |
| --- | --- | --- |
| §1 | `00_overview/01-overview-charter.md` | Identity, charter, FA-Local routing role |
| §2 | `10_service-contract/02-contract-surface.md` | Contract surface |
| §3 | `20_runtime/03-execution-bridge-writeback.md` | Execution bridge & writeback |
| §4 | `30_dependencies/04-dependencies.md` | Swarm peers + contract dependencies |
| §5 | `40_governance/05-scope.md` | Service authority boundary, bounded-worker discipline, truth classes |
| §6 | `40_governance/06-governance.md` | Ownership, designation doctrine, authority hierarchy |
| §7 | `40_governance/07-change-control.md` | Change classes, evidence, verification commands |
| §8 | `40_governance/08-boundaries-and-doctrine.md` | Detailed boundaries & doctrine |
| §9 | `50_operations/09-validation-and-delivery.md` | Validation & delivery |
| §10 | `99_appendices/10-appendices.md` | Glossary, cross-references, revision history |

## Quick Assembly

```bash
bash doc/system/BUILD.sh
```

---

# §1 — Overview & Charter

## Purpose

FA Local is the bounded local execution-control service for Forge applications.

Its current MVP purpose is narrow:

- accept trusted execution requests only
- enforce policy before side effects
- admit execution only through registered capabilities
- require bounded execution plans for multi-step work
- preserve truthful denial, degraded, partial, and completion state
- hand back to human review through a structured bounded review package when direct execution is not admissible
- keep local forensics minimal and auditable

## Constitutional role

FA Local is a service/library implementation repository for the governed FA Local boundary.

It must not become:

- a standalone product UI
- a semantic authority
- a workflow memory surface
- a hidden planner
- a generic agent runtime
- an unbounded plugin executor

## Success posture

FA Local is only successful if it remains:

- bounded by contract
- fail-closed by default
- policy-first before execution
- capability-scoped rather than request-trusting
- truthful about degraded and denied posture
- explicit about human approval and handoff
- unable to drift into hidden orchestration or semantic control

## Current bounded baseline

The currently delivered implementation baseline is no longer scaffold-only.
It currently includes:

- standalone Rust crate and repo framing
- top-level governance and boundary docs
- domain/app/adapter/integration module seams
- typed runtime vocabulary for environment, requester, posture, denial, and degraded state
- typed UUID-backed identity primitives
- fail-closed denial guards and helpers
- schema-backed contracts for requester trust, policy artifact, capability registry, execution request, execution plan, execution status, route decision, and denial guard
- schema-backed contract for review package
- schema-backed contract for forensic event
- schema-backed contract for friction payload
- valid and invalid fixtures for those contract surfaces
- pure schema loading and validation helpers
- pure requester-trust evaluation and capability-admission deny logic
- pure approval-posture resolution and typed route-decision output
- pure bounded execution-plan validation and stable plan hashing
- internal deterministic execution routing from validated route and plan artifacts
- internal bounded execution coordination from validated route and plan artifacts
- explicit adapter boundary for already routed admitted work
- bounded adapter-backed external route delivery mapped back into truthful execution-status surfaces
- one concrete capability-scoped local-file-write adapter implementation
- one concrete Nmap preflight adapter that checks declared local runtime availability without running scans or accepting free-form arguments
- bounded review-package emission workflow for coherent review-required and explicit-approval paths
- bounded forensic recorder/export workflow over already-known execution truth
- pure execution-status validation and construction helpers
- pure review-package validation and construction helpers
- pure forensic-event validation and construction helpers
- pure friction-payload validation and construction helpers
- deny smoke tests for the current fail-closed baseline rules

What is still intentionally not delivered:

- multi-adapter dispatch or runtime selection surface
- broad cross-service adapter integrations
- external adapter-backed execution coordination beyond the current bounded delivery seam
- CLI, daemon, or API surfaces
- forensic persistence layer or concrete export sink
- persistence layer

This is the current bounded baseline, not a claim that later execution-facing phases are already delivered.

## Foundational references

This section is grounded in:

- `README.md`
- `SYSTEM.md`
- `BOUNDARIES.md`
- `ROADMAP.md`

---

# §2 — Contract Surface

## Implemented and planned contract set

The intended FA Local contract surface covers:

- requester trust
- policy artifact
- capability registry
- execution request
- route decision
- execution plan
- execution status
- denial guard
- forensic event
- review package
- friction payload

The currently implemented schema-backed subset is:

- requester trust
- policy artifact
- capability registry
- execution request
- execution plan
- execution status
- review package
- forensic event
- friction payload
- route decision
- denial guard

All planned baseline contract surfaces now exist in schema-backed form.

## Current typed surface

The current machine-checked typed surface includes:

- runtime vocabulary enums
- UUID-backed identity types
- UTC timestamp utility
- structured denial guard payloads
- fail-closed helper functions
- requester trust envelope and trust-evaluation context
- policy artifact and capability-rule types
- capability registry and capability-record types
- execution request type
- execution-plan, execution-plan-step, and fallback-reference types
- pure execution-plan validator and validated-plan wrapper
- execution-status and validated-execution-status types
- pure execution-status invariant validation helpers
- review-package, review-execution-status-context, and approval-option types
- pure review-package invariant validation helpers
- forensic-event, forensic-event-type, and redaction-level types
- pure forensic-event invariant validation helpers
- friction-payload, friction-kind, and operator-action types
- pure friction-payload invariant validation helpers
- route-decision, policy-reference, and capability-decision-summary types
- pure approval-posture resolver inputs and context
- schema-name dispatch plus contract load/deserialize helpers
- `IntakeService` — schema-validated execution request intake entry point (`validate_request()` and `validate_request_bytes()`)

This gives FA Local a stable baseline for deny-by-default behavior with the first contract layer, the first machine-checked decision layer, the first bounded plan-validation layer, the first truthful status layer, the first structured review-handoff layer, the first minimal forensic-truth layer, the first bounded operator-friction layer, and the first typed intake boundary already in place.

## Approval and execution posture

The current vocabulary distinguishes:

- approval posture: `denied`, `review_required`, `explicit_operator_approval`, `policy_preapproved`, `execute_allowed`
- execution state: `denied`, `review_required`, `waiting_explicit_approval`, `admitted_not_started`, `in_progress`, `degraded`, `partial_success`, `completed_with_constraints`, `completed`, `failed`, `canceled`
- degraded subtype: `degraded_pre_start`, `degraded_in_flight`, `degraded_fallback_equivalent`, `degraded_fallback_limited`, `degraded_partial`, `unavailable_dependency_block`

That split keeps approval authority distinct from execution truth rather than collapsing them into one label set.

## Denial surface

The current denial guard preserves:

- denial reason class
- denial scope
- denial basis
- remediable flag
- review-available flag
- operator-visible summary
- UTC timestamp

This is intentionally narrow, but it already supports fail-closed truth without reducing all denials to generic errors.

## Current pure validation and admission logic

The current pure logic layer can already:

- validate requester-trust envelopes against schema and typed rules
- deny unknown requesters
- deny malformed requester envelopes
- deny environment mismatch
- deny invalid or expired nonce/token posture
- deny missing required policy
- deny invalid policy artifacts
- deny unregistered capabilities
- deny disabled or revoked capabilities
- deny policy/capability mismatch
- resolve deterministic approval posture from requester trust, policy, capability admission, review class, and side-effect posture
- produce typed route decisions for `denied`, `review_required`, `explicit_operator_approval`, `policy_preapproved`, and `execute_allowed`
- validate bounded execution plans against declared step counts, declared fallbacks, admitted capabilities, and timeout ceilings
- compute stable execution-plan hashes from canonical plan content
- validate truthful execution-status payloads without collapsing posture into state
- require explicit degraded subtype handling for degraded and constrained status outputs
- validate bounded review-package payloads for explicit operator approval only
- preserve distinction between approval posture and execution state inside review handoff artifacts
- reject fabricated execution success in review-package status context
- require explicit degraded or fallback posture when review-package narration mentions those conditions
- validate minimal forensic-event payloads and record/export them through a bounded workflow without introducing persistence
- preserve distinction between approval posture and execution state inside forensic records
- reject planner, workflow, or semantic narration in forensic-event summaries
- require explicit degraded or fallback subtype handling when forensic-event summaries mention fallback
- validate bounded friction-payload artifacts without collapsing denial, review, approval, and status concerns
- preserve explicit operator-action semantics without inventing workflow authorship
- require explicit linkage or omission rules for review-package, plan-hash, and denial surfaces inside friction payloads

These checks remain bounded to validation, deny-path admission, pure decision output, bounded plan fingerprinting, truthful status shaping, deterministic internal routing, bounded internal coordination, explicit adapter-backed delivery over already selected admitted routes, one concrete capability-scoped local-file-write adapter, one concrete Nmap preflight adapter that only reports declared local runtime availability, one bounded review-package emitter workflow for contract-compatible review-required and explicit-approval paths, and one bounded forensic recorder/export workflow over already-known execution truth.
They still do not perform semantic interpretation, planner behavior, or unbounded external invocation.

## Current implementation boundary

All currently planned baseline contracts now exist in schema-backed form.

Phase X4 added:
- `IntakeService` in `src/app/intake_service.rs` — the schema-validated entry point for external execution requests. It wraps `ExecutionRequest::load_contract_value()` and provides both `validate_request(&Value)` and `validate_request_bytes(&[u8])` convenience methods.
- `fa-local-run` CLI binary (`src/bin/fa_local_run.rs`) — a synchronous binary providing `validate` and `status` subcommands for local operator use.

There is no persistence layer, no concrete forensic export sink, and no multi-adapter dispatch or runtime selection surface in the current baseline. The Nmap preflight adapter is bounded to a declared `local_process_spawn` capability and execution plan, does not run scans, does not accept free-form arguments, and does not create a networked daemon surface. Missing `nmap` runtime truth can be represented as a degraded execution status and recorded through the existing minimized forensic-event path. The review-package emitter remains intentionally bounded to the two current review postures only and does not introduce generic workflow behavior beyond `review_required` and `explicit_operator_approval`.

The execution bridge writeback path (`DfLocalAdapter::post_execution_status_event`) is present as a typed stub — the DataForge Local staging endpoint is pending Phase X4 completion on the DataForge side.

## Supporting references

This section is grounded in:

- `src/domain/shared/schema.rs`
- `src/domain/shared/vocabulary.rs`
- `src/domain/shared/ids.rs`
- `src/domain/guards/mod.rs`
- `src/domain/requester_trust/mod.rs`
- `src/domain/policy/mod.rs`
- `src/domain/capabilities/mod.rs`
- `src/domain/execution/mod.rs`
- `src/domain/forensics/mod.rs`
- `src/domain/friction/mod.rs`
- `src/domain/posture/mod.rs`
- `src/domain/routing/mod.rs`
- `src/adapters/exports/mod.rs`
- `src/adapters/execution_delivery/mod.rs`
- `src/adapters/execution_delivery/local_file_write.rs`
- `src/adapters/execution_delivery/nmap_preflight.rs`
- `src/app/execution_service.rs`
- `src/app/forensic_service.rs`
- `src/app/intake_service.rs`
- `src/app/review_service.rs`
- `src/app/routing_service.rs`
- `src/bin/fa_local_run.rs`
- `docs/fa_local_codex_build_plan_v_1.md`

---

# §3 — Execution Bridge & Writeback

## Purpose

This section documents the planned writeback path for FA Local execution status
artifacts into the shared proving-slice pipeline.

FA Local produces truthful execution status events. Those events must travel to
DataForge Local's staging queue, be promoted to DataForge Cloud, and become
visible in Forge_Command's execution review surface — following exactly the same
pipeline as `source_drift_finding` artifacts in proving slice 01.

## Writeback path

```
FA Local
  ↓ post_execution_status_event()
DataForge Local staging queue  (execution_status_event v1 artifact)
  ↓ local promotion
DataForge Cloud intake         (shared truth ingestion)
  ↓ read model
Forge_Command execution review surface
```

## Artifact family

Writeback artifacts are in the `execution_status_event` v1 family, defined in
`forge-contract-core/contracts/families/execution_status_event/`. They travel
in the shared envelope format.

Key envelope fields for writeback artifacts:

| Field | Value |
|-------|-------|
| `artifact_family` | `execution_status_event` |
| `artifact_version` | `1` |
| `produced_by_system` | `fa-local-operator` |
| `produced_by_component` | `execution_service.status_emitter` |
| `source_scope` | `local` |
| `promotion_class` | `promotable` |

## Implementation boundary

The `DfLocalAdapter.post_execution_status_event()` method in
`src/integrations/df_local/mod.rs` establishes the typed contract boundary.

**Current state (Phase X3):** Method signature and `ExecutionStatusWritebackRequest`/
`ExecutionStatusWritebackResult` types are defined. The method returns
`FaLocalError::WritebackNotWired` until Phase X4 completes the real endpoint.

**Required for Phase X4:**

1. DataForge Local needs an `execution_status_event` staging endpoint
   (mirrors the existing `source_drift_finding` staging path)
2. FA Local needs an HTTP client dependency (e.g., `reqwest` or `ureq`)
3. `post_execution_status_event()` must:
   - Serialize `ValidatedExecutionStatus` into `execution_status_event` v1 payload
   - Build the full shared envelope with idempotency key
   - POST to DataForge Local's configured local API address
   - Return `ExecutionStatusWritebackResult { acknowledged: true }` on success

## Authority constraints

FA Local may NEVER:
- Write directly to DataForge Cloud
- Write source_drift_finding, promotion_envelope, or promotion_receipt artifacts
- Write lifecycle transitions (ForgeCommand owns those)
- Invent or modify approval decisions

FA Local may ONLY:
- POST `execution_request` and `execution_status_event` artifacts to DataForge Local
- Consume `approval_artifact` artifacts from Forge_Command (via DataForge Local)

## Approval artifact consumption path

```
Forge_Command
  ↓ approval_artifact (v1)
DataForge Cloud        (shared truth storage)
  ↓ DataForge Local polls or subscribes
DataForge Local
  ↓ approval delivery to FA Local
FA Local execution_service  (resumes waiting_explicit_approval path)
```

This path is not yet implemented. It will be designed in Phase X4 alongside
the writeback endpoint work.

---

# §4 — Dependencies

**Truth class:** snapshot (audit-derived)

fa-local-operator is a bounded worker in the local Cortex "Gnats" swarm; its
dependencies are the swarm peers and the contracts it conforms to. Re-measure
against the build manifest when this changes.

## Swarm Peers

| Peer | Role | Relationship |
|------|------|--------------|
| Cortex (`COR`) | Planning / extraction | Upstream — produces the plan fa-local-operator routes |
| NeuronForge-Local | Semantics / candidate generation | Peer — supplies semantics/candidates |
| DataForge-Local (`DLO`) | Local durable persistence | Downstream — persists operational truth |
| ForgeCommand (`FCO`) | Operator / control plane | Orchestrates; consumes results |

## Contract Dependencies

fa-local-operator honors its **contract surface** (§2) exactly and bridges results
back via the **execution bridge** (§3); it consumes contracts, it does not redefine
them.

## Runtime Dependencies

Versions and crate/package pins are catalogued in the build manifest; this chapter
records the *relationships* (canonical), while exact versions are snapshot facts
re-measured at build time.

---

# §5 — Scope

**Truth class:** canonical doctrine

This `doc/system/` tree is the modular source of the **fa-local-operator compiled
system reference**, assembled into the designation-bound artifact
`doc/FLOSYSTEM.md` (designation `FLO`) via `bash doc/system/BUILD.sh`. This chapter
defines fa-local-operator's authority and where it ends. fa-local-operator is an
internal Forge ecosystem service — single-operator, local, fail-closed — not a
public product and not externally release-certified. Detailed boundaries and
doctrine are in §8.

## fa-local-operator Service Authority

fa-local-operator is the **ForgeAgents-Local operator** — a bounded, deterministic
worker in the local Cortex "Gnats" swarm. Its role is **routing**: within the
local self-healing swarm, Cortex plans/extracts, fa-local-operator routes,
NeuronForge-Local supplies semantics/candidates, and DataForge-Local persists.
fa-local-operator's authority is routing-and-bridge-oriented: it routes work to
the right local worker and bridges execution results back through a contract
surface — it does not plan, decide, or persist canonical truth.

## What fa-local-operator Owns

- **Routing** within the local Gnats swarm (the FA-Local lane).
- **Contract-surface conformance** (§2) — honoring its contract surface exactly.
- **Execution-bridge writeback** (§3) — bridging execution results back across the
  contract boundary.
- **Validation & delivery** (§9) of its own bounded behavior.

## What fa-local-operator Does Not Own

- **Planning / extraction.** Cortex (`COR`) owns swarm planning.
- **Semantics / candidate generation.** NeuronForge-Local owns those.
- **Durable persistence.** DataForge-Local persists the swarm's operational truth.
- **Decision / canonical truth or orchestration.** ForgeCommand is the
  operator/control plane.

## Bounded-Worker Discipline

fa-local-operator is a bounded, deterministic Gnat: it fails closed on ambiguity,
stays within its routing/bridge lane, and never expands into planning, semantics,
persistence, or decision authority.

## Release / Readiness Language Restrictions

This documentation describes an internal service under governed, slice-by-slice
development. It must be described as a verification-current internal service, not
as externally release-certified, and must not claim public-release/SaaS readiness
or present coverage percentages as guarantees unless a later governed slice proves
the specific claim.

## Documentation truth classes

- **Canonical facts** define fa-local-operator's routing role, swarm boundaries,
  contract-surface conformance, and bounded-worker discipline. They change only
  through deliberate change control (§7).
- **Snapshot facts** are audit-derived counts (modules, tests, slices) labelled
  with a measurement date and corrected by re-measurement, not change control.

Ownership, designation doctrine, and the authority hierarchy that govern this tree
are defined in §6; detailed boundaries and doctrine in §8.

---

# §6 — Governance

**Truth class:** canonical doctrine

Ownership, review, and change-authority boundaries for this documentation system.
§5 defines fa-local-operator's *service* authority; this chapter defines the
*documentation* authority that governs how this `doc/system/` tree is owned,
designated, and changed.

## Ownership

| Artifact | Owner |
|----------|-------|
| `doc/system/` source modules | fa-local-operator repository (this repo) |
| `doc/FLOSYSTEM.md` compiled artifact | fa-local-operator repository — generated, never hand-edited |
| `FLO` designation | ForgeCommand designation registry (governed registry state, not local repo opinion) |
| Ecosystem composite compiled system reference | ForgeCommand |

The operating context is a single-operator governed environment: compliance state
must be explicit, visible, and reconstructable; remediation is bounded and
reviewable; approval remains human-authoritative.

## Designation doctrine

- The designation is exactly three letters (`FLO`), unique across the governed
  repo registry, and stable once assigned.
- The compiled artifact filename is bound to the designation:
  `doc/{DESIGNATION}SYSTEM.md` → `doc/FLOSYSTEM.md`. `BUILD.sh` fails closed if the requested
  output path does not end in `FLOSYSTEM.md`.
- Designation changes occur only through explicit change control in the
  ForgeCommand registry, never by local edit.
- Legacy outputs (`SYSTEM.md` at repo root, two-letter prefixed artifacts, the
  bootstrap `doc/SYSTEM.md`) are non-canonical; if detected they are migration
  signals, not truth surfaces.

## Authority hierarchy

When documentation sources conflict, resolve in this order:

1. `doc/FLOSYSTEM.md` — the compiled system reference (implemented reality)
2. `CLAUDE.md` — AI implementation instructions and working rules
3. Module/feature specs and plans under `docs/`
4. README and ad-hoc notes

The compiled system reference wins because it describes implemented reality; all
other surfaces describe intent, instruction, or history.

## Truth-class enforcement

Every statement in this tree is a **canonical fact** (service role, contracts, and
invariants — changed only through change control, §7) or a **snapshot fact**
(audit-derived counts: routes, tables, tests, coverage — labelled with a
measurement date and corrected by re-measurement, not change control). Snapshot
facts must never be promoted to guarantees; release/readiness language is
constrained per §5.

## Editing rule

Source modules under `doc/system/` are the only editing surface. The compiled
artifact is regenerated by `bash doc/system/BUILD.sh` and validated by
`doc/system/validate_snapshots.sh` during assembly. A hand edit to `doc/FLOSYSTEM.md` is a
governance violation and is overwritten by the next build.

## Enforcement

ForgeCommand is the enforcement surface for documentation compliance. Where
automated enforcement is not yet wired up for this repo, enforcement is manual but
explicit: the change-control workflow in §7.

---

# §7 — Change Control

**Truth class:** canonical doctrine

This chapter defines how changes to fa-local-operator are classified, evidenced,
verified, and rolled back. Every change class names the evidence and verification
commands that must accompany it. fa-local-operator is an internal Forge ecosystem
service; nothing here authorizes public-release or production-certification claims.

## Change Classes

| Class | Scope | Example |
|-------|-------|---------|
| C0 | Documentation only | Editing `doc/system/` chapters, rebuilding `doc/FLOSYSTEM.md` |
| C1 | Routing logic | The FA-Local routing lane within the swarm (§5/§8) |
| C2 | Contract surface | Honoring/changing the contract surface (§2) |
| C3 | Execution bridge | Writeback bridge behavior (§3) |
| C4 | Dependencies | Swarm-peer or contract pin changes (§4) |
| C5 | Validation & delivery | Proof/validation gates (§9) |
| C6 | Configuration / security | Env contract, fail-closed posture |

## Required Evidence Per Change Class

- **C0** — rebuilt artifact (`bash doc/system/BUILD.sh` → `BUILD_OK`), edited
  source chapter (never a hand-edit to `doc/FLOSYSTEM.md`).
- **C1** — tests proving routing stays bounded/deterministic and in-lane (does not
  absorb planning/semantics/persistence).
- **C2** — proof the contract surface is honored exactly (consumed, not redefined).
- **C3** — tests for the changed writeback bridge path.
- **C4** — the swarm-peer/contract change reflected in §4.
- **C5** — the validation/delivery evidence in §9.
- **C6** — env/setting change with secrets never hard-coded; fail-closed preserved.

## Required Verification Commands

```bash
# run the repo's test suite (cargo test / pytest, per the build manifest)
bash doc/system/BUILD.sh                # doc changes (C0) -> BUILD_OK designation=FLO
```

## Bounded-Worker / Boundary Rules

fa-local-operator stays a bounded, deterministic routing Gnat (§5/§8): a change must
not expand it into planning (Cortex), semantics (NeuronForge-Local), persistence
(DataForge-Local), or decision authority (ForgeCommand). It fails closed on
ambiguity rather than guessing.

## Documentation Change Rules (C0)

`doc/system/` source modules are the only editing surface. The compiled
`doc/FLOSYSTEM.md` is regenerated, never hand-edited (§6). Snapshot facts are
re-measured and re-dated, not asserted as guarantees.

## Release / Readiness Claim Rules

No change may introduce public-release, public-SaaS, or production-certification
language, or present a coverage percentage as a guarantee, unless a governed
release slice proves that specific claim.

---

# §8 — Boundaries & Doctrine

## Authority line

FA Local owns:

- requester-trust-gated execution intake
- requester trust posture evaluation
- policy-before-execution enforcement
- capability admission checks
- approval posture resolution
- bounded execution-plan validation
- controlled execution coordination for admitted routes
- review-package handoff support
- minimized forensic event generation for execution truth

FA Local does not own:

- application business semantics
- syntax authority
- model or inference authority
- durable workflow memory
- hidden workflow policy authority
- open-ended planning
- ungoverned tool access
- canonical business truth

## Doctrine line

The governing doctrines are:

- policy before execution
- requester trust before admission
- fail closed over convenience
- bounded execution rather than runtime invention
- truthful degraded-state reporting
- explicit approval and review handoff
- privacy-preserving, minimized forensics
- explicit adapters rather than absorbed cross-service semantics

## Cross-service boundaries

### DF Local Foundation

Provides bounded substrate support for readiness, persistence, and local records.
It does not become execution authority.

### NeuronForge Local

May be invoked only through admitted inference contracts where policy allows.
It does not transfer final execution authority away from FA Local.

### Cortex

May provide approved preparation or readiness contracts only.
It does not make FA Local a syntax authority, and FA Local does not delegate execution authority back into Cortex.

## Anti-drift warning

Any proposal that turns FA Local into a planner, semantic authority, broad agent substrate, generic tool governor, or stealth orchestrator should be rejected unless the architecture is explicitly reworked.

No automatic expansion is implied by the current scaffold.
Further implementation must stay inside the constitutional boundaries already established in the repo and the shared runtime doctrine.

---

# §9 — Validation & Delivery

## Validation surface

FA Local currently includes:

- Rust build metadata in `Cargo.toml`
- JSON schemas in `schemas/`
- valid fixtures in `tests/contracts/fixtures/valid/`
- invalid fixtures in `tests/contracts/fixtures/invalid/`
- schema loading and validation tests in `tests/contracts_schema.rs`
- typed contract loading tests in `tests/contracts_loading.rs`
- deny smoke tests in `tests/denial_smoke.rs`
- deterministic enum serialization tests in `tests/enums_roundtrip.rs`
- fail-closed guard tests in `tests/guard_helpers.rs`
- repo-local assembly for system documentation through `doc/system/BUILD.sh`

The current machine-checked layer covers:

- schema validation for the eleven implemented contract surfaces
- valid and invalid fixture coverage for each implemented schema
- typed contract deserialization after schema validation
- requester-trust fail-closed rules
- policy artifact fail-closed rules
- capability admission fail-closed rules
- route-decision schema invariants for posture/bool consistency
- golden approval-posture resolution for all five posture outcomes
- deny-to-posture mapping and invalid-input fail-closed posture behavior
- bounded execution-plan validation rules
- undeclared fallback rejection
- disabled, revoked, and unregistered capability rejection for execution-plan references
- deterministic stable execution-plan hash behavior
- execution-status schema invariants for truthful state shaping
- typed execution-status invariant validation and construction helpers
- execution-status tests proving posture remains distinct from state
- explicit degraded subtype enforcement for degraded and constrained status outputs
- review-package schema invariants for bounded structured review handoff
- typed review-package invariant validation and construction helpers
- review-package tests proving posture remains distinct from execution state
- review-package tests rejecting fabricated execution-success context
- forensic-event schema invariants for minimal bounded forensic truth
- typed forensic-event invariant validation and construction helpers
- forensic-event tests proving posture remains distinct from execution state
- forensic-event tests rejecting planner or workflow narration
- forensic recorder/export workflow tests for truthful linkage and fail-closed emission/export behavior
- friction-payload schema invariants for bounded operator-visible friction truth
- typed friction-payload invariant validation and construction helpers
- friction-payload tests proving denial, review, approval, and constrained status remain distinct
- friction-payload tests rejecting planner or workflow narration
- stable snake-case serialization for baseline enums
- unknown-enum rejection behavior
- typed guard creation
- fail-closed helper behavior
- UTC timestamp stamping on denials

## Delivered slice

The currently delivered implementation slice is Phase 0.5 plus the opening of Phase 1 only.

It adds:

- standalone `fa-local` repository framing
- top-level repo docs and ADR stubs
- bounded source-tree layout for domain, app, adapters, and integrations
- shared runtime vocabulary aligned to the FA Local doctrine
- `IntakeService` typed schema-validated entry point (`validate_request`, `validate_request_bytes`)
- `fa-local-run` CLI binary (`validate` and `status` subcommands)
- `DfLocalAdapter::post_execution_status_event()` typed writeback stub (returns `WritebackNotWired` until DataForge Local endpoint is live)
- `ci_gate.sh` contract gate runner (forge-contract-core gates + `cargo test`)
- typed denial/error primitives
- schema-backed contracts for requester trust, policy artifact, capability registry, execution request, execution plan, execution status, route decision, and denial guard
- pure schema loading and validation helpers
- pure requester-trust evaluation
- pure policy-required loading
- pure capability-admission deny logic
- pure approval-posture resolution
- typed route-decision output with deterministic posture flags
- pure execution-plan validation with declared fallback checks
- stable execution-plan hash generation from canonical plan content
- pure execution-status validation with truthful-state invariants
- schema-backed review-package contract and pure validation helpers
- schema-backed forensic-event contract and pure validation helpers
- bounded forensic recorder/export workflow over already-known route, review, and execution truth
- schema-backed friction-payload contract and pure validation helpers
- internal deterministic routing service over validated route and plan inputs
- internal bounded execution coordinator over validated route and plan inputs
- bounded review-package emitter workflow over coherent review-required and explicit-approval inputs
- explicit adapter boundary for external route delivery from already selected admitted routes
- bounded adapter-result mapping back into existing execution-status truth surfaces
- one concrete capability-scoped local-file-write adapter behind the delivery boundary
- one concrete Nmap preflight adapter that checks declared local runtime availability and maps missing `nmap` to `unavailable_dependency_block`
- Nmap preflight fixtures proving `local_process_spawn` capability/plan validation and minimized forensic recording for degraded missing-runtime truth
- deterministic contract fixtures and deny smoke coverage
- latest `jsonschema` validator release aligned in the crate dependency set

## Not yet delivered

The following planned surfaces are explicitly not delivered yet:

- multi-adapter dispatch or runtime selection surface
- broad cross-service adapter integrations
- daemon or networked API surface
- forensic persistence layer
- concrete forensic export sink
- persistence layer
- DataForge Local staging endpoint wiring for execution_status_event writeback (Phase X4 DataForge side)

## Current delivery posture

The repo currently supports:

- `cargo fmt`
- `cargo test`
- `bash doc/system/BUILD.sh`
- `bash ci_gate.sh` (forge-contract-core gates + cargo test)
- `./target/debug/fa-local-run validate <path>` (or stdin)
- `./target/debug/fa-local-run status`

The current delivered state should be described as:

- governance scaffold present
- typed baseline present
- first contract layer present
- first deny-path admission layer present
- first machine-checked route-decision layer present
- first bounded execution-plan layer present
- first truthful execution-status layer present
- first structured review-package handoff layer present
- first minimal forensic-event truth layer present
- first bounded forensic recorder/export workflow present
- first bounded friction-payload layer present
- first deterministic internal execution-routing layer present
- first internal bounded execution-coordinator layer present
- first bounded review-package emitter workflow present
- first bounded adapter-backed external route-delivery layer present
- first concrete capability-scoped adapter present
- second concrete adapter present only for Nmap runtime preflight, with no scan execution or free-form argument surface
- first typed intake boundary present (`IntakeService`)
- first CLI binary surface present (`fa-local-run`)
- first typed writeback stub present (`DfLocalAdapter::post_execution_status_event` — not yet wired)
- contract gate runner present (`ci_gate.sh`)
- no full external FA Local runtime surface admitted yet

That wording matters because the crate now has meaningful contract, deny-path, posture-resolution, bounded plan-validation, truthful status, bounded review-handoff behavior, a bounded review-package emitter workflow for both current review postures, minimal forensic-event truth behavior, a bounded forensic recorder/export workflow, bounded operator-friction behavior, deterministic internal routing behavior, bounded internal coordination behavior, a narrow adapter-backed delivery seam, one concrete capability-scoped local-file-write adapter, one concrete Nmap preflight adapter, a typed intake entry point, a CLI binary, and a typed writeback stub — but it still does not ship persistence, a concrete forensic export sink, multi-adapter dispatch, generic workflow orchestration, live scan execution, or a networked API/daemon runtime surface.

---

# §10 — Appendices

**Document version:** 1.0 (carry-forward)

Appendices, glossary, and cross-references.

## Unmapped legacy chapters

The following legacy chapters were carried forward but could not be
deterministically mapped to a class-aware slot. Review and place them by
hand:

- `FA Local - System Documentation`
- `2. Boundaries and Doctrine`
- `3. Contract Surface`
- `4. Validation and Delivery`
- `5. Execution Bridge Writeback`
