# §1 — Overview & Charter

> **System identity — bds family (Boswell Digital Solutions business system, local-systems tier).** This service is part of the Forge ecosystem backend in `ecosystem/local-systems`. It is **not** the Forge public-app support counterpart `apps/public-app-local-support/fa-local`.

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
- capability-scoped adapter registry resolving one runtime-selected adapter per admitted route at dispatch time; fails closed on duplicate capability registration and reports a missing adapter as a truthful degraded status, not a fabricated success
- bounded forensic recorder/export workflow over already-known execution truth
- append-only local JSONL forensic export sink, one compact record per line, reporting an unavailable sink as a fail-closed error rather than dropping the event
- per-step, per-capability adapter dispatch and coordination (`ExecutionService::deliver_plan_per_step_via_registry`): a plan whose steps span different capabilities is dispatched one step at a time, resolving each step's own adapter from the registry, and the per-step outcomes are aggregated into one truthful final status — `Completed` only when every step completed, `PartialSuccess`/`degraded_partial` for any real mix of completed and not-completed steps, and `Failed`/`Canceled`/`Degraded` chosen from whichever steps were actually attempted (not ones skipped after an earlier step stopped the run) when zero steps completed; `cancellation_policy` governs whether a failure stops later steps from being attempted at all
- declared-fallback coordination across per-step-dispatched adapters: when a step's own attempt is `Failed`, `Unavailable`, or `Canceled` and the plan declares a fallback for it (`validated_plan.plan.fallback_references`), the coordinator dispatches the declared fallback step out of its normal order to *its own* registry-resolved adapter, which may differ from the primary step's; plan validation already guarantees the fallback step is declared later in the plan and targets a different step, so this never dispatches out of causal order. A fallback step already consumed by one failed step's fallback attempt is never dispatched a second time for another step declaring the same fallback (the second step's own original outcome stands). A plan that completes entirely only because one or more steps needed their declared fallback reports `CompletedWithConstraints`/`degraded_fallback_limited` — never `DegradedFallbackEquivalent`, since the coordinator has no way to know two different capabilities are truly equivalent, only that a plan author declared one a fallback for the other — rather than a plain `Completed`
- `fa-local-run execute --per-step-dispatch`: an operator-chosen flag selecting per-step dispatch over the pipeline's default single whole-route call; the configured adapter is registered under the route's own top-level capability, so a step declaring a different capability truthfully reports as unavailable rather than silently succeeding, unless an adapter for that capability is registered separately (see `--adapter` below)
- `fa-local-run execute --adapter <CAPABILITY_UUID>:<local-file-write|nmap-preflight>:<PARAMS>`: a repeatable flag registering an additional adapter under an explicitly declared capability (`ExecutionPipelineService::run`'s `additional_adapters: Vec<CapabilityScopedAdapterSelection>`), alongside the route's own implicit one; a heterogeneous multi-capability plan run with `--per-step-dispatch` no longer needs every adapter registered by hand through the library API
- queryable local forensic store backed by bundled SQLite (`SqliteForensicStore`), indexed by `correlation_id` and `event_type`, round-tripping the same already-minimal `ForensicEvent` encoding used by the JSONL sink; exposed as an alternative `--forensic-sqlite` export sink on `fa-local-run execute` and queried back out via `fa-local-run forensics-query`
- `fa-local-run route` CLI command wiring intake, requester-trust evaluation, policy loading, and capability admission into one resolved route decision from raw untrusted JSON files, exit-coded on whether the resolved posture admits execution
- `fa-local-run execute` CLI command wiring `route` through plan validation, `AdapterRegistry`-backed dispatch, and forensic recording/export in one bounded run: denied and review-required routes stop with a truthful forensic record and no plan is ever touched; an unbounded plan reports a plan denial instead of running; an admitted route with no adapter registered for its capability degrades truthfully rather than fabricating success
- pure execution-status validation and construction helpers
- pure review-package validation and construction helpers
- pure forensic-event validation and construction helpers
- pure friction-payload validation and construction helpers
- deny smoke tests for the current fail-closed baseline rules
- `integrations::cortex::CortexSubprocessGnatShardAdapter`: dispatches one admitted Cortex Gnat shard by spawning Cortex's `cortex_runtime.gnats.shard_cli` (a new bounded CLI entry point added in the COR repo alongside this) as a subprocess and parsing the `GnatWorkerReceipt.v1` it prints, live-verified against the real COR checkout. This is the "later FA-Local dispatch adapter" `DECISIONS/0019` (COR repo) names as the missing half of `GnatDispatchValidator::negotiate`'s admission-only path. Bounded to the two worker types `DECISIONS/0018` (COR) authorizes for this proving slice (`markdown_syntax`, `plain_text_syntax`) — refused before ever spawning a process, not left for Cortex's own CLI to reject. Takes a caller-supplied, already-complete `GnatShardDispatchRequest`, not a `GnatDispatchShard` (the negotiation-time envelope's shard summary, which lacks several needed fields) — bridging admission negotiation into a full runnable shard descriptor remains undelivered. Deadline enforcement on the subprocess call is a disclosed gap, not an implemented one.
- `GnatDispatchPipelineService` (`src/app/gnat_dispatch_pipeline_service.rs`) and `fa-local-run gnat-dispatch`: the first real admission-to-dispatch code path in this repo. Composes `GnatDispatchValidator::negotiate` with `CortexSubprocessGnatShardAdapter` — negotiate, and only when the negotiated posture is `ReadyForFaLocalDispatch`, deliver every declared shard and collect its real outcome; a denied run or a `SerialFallbackPermitted` run (Cortex's own job per `DECISIONS/0019`, not this pipeline's) dispatches nothing. Requires the caller to supply the full runnable descriptor for every declared shard directly and only checks those descriptors are consistent with what the envelope declared (same `run_id`/`shard_id`/`worker_type`/`source_ref`) — refuses a mismatch, never derives one from the other. Live-verified end to end against the real COR checkout: real negotiation, real admission, two real shards dispatched to two real Cortex workers, both receipts `complete`. No forensic recording yet — `ForensicRecordKind` is built entirely around FA Local's own `RouteDecision`/`ExecutionStatus` domain, which a Cortex-initiated Gnat run has no equivalent of; giving these runs the same truthful forensic trail every other admitted path gets needs its own forensic-event contract extension, not a bolt-on to this pipeline.

What is still intentionally not delivered:

- broad cross-service adapter integrations beyond the Cortex Gnat proving slice above (which itself still has no forensic recording and no negotiation-to-dispatch bridge from `GnatDispatchShard`); NeuronForge-Local and DF-Local integrations are unstarted
- daemon or API surfaces
- persistence layer beyond forensic evidence (e.g. durable policy/capability/execution state across restarts)

This is the current bounded baseline, not a claim that later execution-facing phases are already delivered.

## Foundational references

This section is grounded in:

- `README.md`
- `SYSTEM.md`
- `BOUNDARIES.md`
- `ROADMAP.md`
