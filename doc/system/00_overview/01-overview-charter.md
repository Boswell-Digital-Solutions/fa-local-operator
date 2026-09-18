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
- `integrations::cortex::CortexSubprocessGnatShardAdapter`: dispatches one admitted Cortex Gnat shard by spawning Cortex's `cortex_runtime.gnats.shard_cli` (a new bounded CLI entry point added in the COR repo alongside this) as a subprocess and parsing the `GnatWorkerReceipt.v1` it prints, live-verified against the real COR checkout. This is the "later FA-Local dispatch adapter" `DECISIONS/0019` (COR repo) names as the missing half of `GnatDispatchValidator::negotiate`'s admission-only path. Bounded to the two worker types `DECISIONS/0018` (COR) authorizes for this proving slice (`markdown_syntax`, `plain_text_syntax`) — refused before ever spawning a process, not left for Cortex's own CLI to reject. Deadline enforcement on the subprocess call is a disclosed gap, not an implemented one.
- `GnatShardDispatchRequest::from_declared_shard` and `GnatShardEnrichment` (`src/integrations/cortex/shard_dispatch.rs`): the negotiation-to-dispatch bridge. `GnatDispatchShard` (the envelope's own negotiation-time shard summary) never carries `source_path_token`, `media_type`, the fingerprint beyond its bare digest, `max_bytes`, or the real `local_path` (which, by design, no schema-validated contract ever carries). `GnatShardEnrichment` supplies exactly those, keyed by `shard_id`; `from_declared_shard` merges an enrichment with its matching declared shard to build a full `GnatShardDispatchRequest` — every envelope-declared field (`shard_id`, `ordinal`, `worker_type`, `source_ref`, the fingerprint digest, `deadline_ms`) comes from the envelope alone, so there is exactly one source of truth for each, never two that could disagree.
- `GnatDispatchPipelineService` (`src/app/gnat_dispatch_pipeline_service.rs`) and `fa-local-run gnat-dispatch`: the first real admission-to-dispatch code path in this repo. Composes `GnatDispatchValidator::negotiate` with the bridge above and `CortexSubprocessGnatShardAdapter` — negotiate, and only when the negotiated posture is `ReadyForFaLocalDispatch`, deliver every declared shard and collect its real outcome; a denied run or a `SerialFallbackPermitted` run (Cortex's own job per `DECISIONS/0019`, not this pipeline's) dispatches nothing. The caller supplies a `GnatShardEnrichment` per declared shard (`--shard-enrichment <FILE>`, a JSON object keyed by `shard_id`) — a declared shard with none supplied is refused before negotiation ever runs, never silently skipped. Live-verified end to end against the real COR checkout, both paths: matching digests dispatch two real shards to two real Cortex workers with both receipts `complete` (exit 0); deliberately stale digests report both as `stale` through the same real subprocess calls (exit 1).
- `integrations::cortex::GnatDispatchForensicEvent` (`src/integrations/cortex/forensics.rs`, `schemas/gnat-dispatch-forensic-event.schema.json`): a separate, bounded forensic contract for Cortex Gnat dispatch runs, alongside `forensic-event.schema.json` rather than forced through it — that schema is built entirely around FA Local's own `route_decision_id`/`execution_plan_id`/`ApprovalPosture`/`ExecutionState`, none of which a Cortex-initiated Gnat run has, matching how every other Gnat contract already gets its own schema instead of reusing an execution-request one. `GnatDispatchPipelineService::run` records exactly one `gnat_dispatch_negotiated` event per run and, only for an admitted run, one `gnat_shard_dispatched` event per declared shard (`GnatDispatchRunResult::forensic_events`); a `dispatch_unavailable` shard structurally cannot carry a `receipt_state`, a `completed`/`not_completed` one structurally must. Live-verified end to end: a real dispatch run against the COR checkout produced 3 real events (1 negotiation + 2 shard), each independently schema-valid.
- Export sinks for `GnatDispatchForensicEvent` (`src/integrations/cortex/forensic_export.rs`, `JsonlGnatForensicExportAdapter`/`SqliteGnatForensicStore`), mirroring `execute`'s own JSONL/SQLite sinks for a separate contract. `GnatDispatchPipelineService::run` takes an optional `forensic_export_adapter`; each `GnatForensicRecordOutcome` carries its own `export_reference` (`None` without a sink, `Some(reference)` with one), and a failed export fails the whole run closed — reusing `ForensicService::map_export_result`'s validation rather than duplicating it. Exposed as `fa-local-run gnat-dispatch --forensic-export <FILE>|--forensic-sqlite <FILE>` (mutually exclusive, both optional). Live-verified end to end against the real COR checkout: both sinks independently produced 3 real records (1 negotiation + 2 shard) from the same dispatch run, confirmed by reading the JSONL file and querying the SQLite store directly.
- Deadline enforcement on the Gnat dispatch subprocess call: `CortexSubprocessGnatShardAdapter` puts the spawned interpreter in its own process group (`process_group(0)`) and, if a shard's own `deadline_ms` is exceeded, kills the whole group via a direct `libc::kill(-pid, SIGKILL)` syscall — not by shelling out to an external `kill` binary, which silently no-ops for process-group signals in this crate's own sandboxed dev environment (`KI-FLO-20260918-004`; `Child::kill()`, used internally by the same code path, worked correctly throughout, isolating the gap to that one shelled-out pattern specifically). Live-verified against the real COR checkout: an unrealistically tight deadline on one shard is killed promptly (`dispatch_unavailable`, forensic event recorded) while a sibling shard in the same run still completes normally.
- DataForge Local execution-bridge writeback (Phase X4, `DfLocalAdapter::post_execution_status_event`, `src/integrations/df_local/mod.rs`): serializes a `ValidatedExecutionStatus` into an `execution_status_event.v1` artifact envelope and POSTs it to DataForge Local's `POST /api/v1/execution-bridge/status-events` (`dataforge-Local#35`, `DATAFORGE_LOCAL_URL` env var, default `http://127.0.0.1:8005`, via `ureq`). Idempotency key computed by the canonical proving-slice algorithm, cross-language-verified against the real Python implementation. `promotion_class: "local_only"` keeps the artifact out of any cloud-egress path, matching `execution_trace_state`'s classification in `forge_contract_core`'s `repo_role_matrix.json`. No signing-key infrastructure exists for this artifact family, so `signature` is a real SHA-256 digest of the payload body, not a cryptographic signature. `domain::service_status::operational_facts()`'s `writeback_wired` fact now reads `true`, and the FC-LTA-P007 canonical status projection reports `state: "ready"` accordingly (previously `degraded`/`unavailable_dependency_block`).
- `integrations::neuronforge_local::HttpNeuronForgeLocalAdapter` (`src/integrations/neuronforge_local/mod.rs`): a first NeuronForge-Local proving slice, dispatching the one task `neuronforge-local-operator`'s `ADR-002` (`docs/adr/ADR-002-fa-local-task-routing.md` in that repo) admits — `analyze.style.scene.v1` — to its `POST /api/v1/fa-local/task-dispatch` route over HTTP (`ureq`, `NEURONFORGE_LOCAL_URL` env var, default `http://127.0.0.1:8000`), mirroring `DfLocalAdapter`'s client pattern rather than a spawned subprocess, since ADR-002's transport section explains FA Local dispatching over HTTP as a client doesn't conflict with its own "no HTTP surface" doctrine (which constrains what it serves, not what it calls). Any other `task_id` is refused before ever making a network call, the same way Cortex's `AUTHORIZED_WORKER_TYPES` check happens before spawning a process. The response's `schema_validation_status` maps to a truthful `NeuronForgeTaskDispatchResult`: `Completed` for `"valid"`, `NotCompleted` for `"degraded"`/`"failed"` (a real receipt came back but wasn't usable), `DispatchUnavailable` for anything that never produced a real receipt (unreachable, non-2xx, unparseable, or a missing/unrecognized status). Exposed as `fa-local-run neuronforge-dispatch --scene <FILE> [--neuronforge-url <URL>] [--model <ID>] [--forensic-export <FILE>|--forensic-sqlite <FILE>]`. Live-verified end to end against a real running `neuronforge-local-operator` service and a real local Ollama model (`qwen2.5:14b`): a real scene produced a genuine structured style-analysis candidate (`schema_validation_status: "valid"`, every `registry_guardrails` flag `false`); a stopped service correctly reported `dispatch_unavailable` (exit 1).
- `integrations::neuronforge_local::NeuronForgeTaskDispatchForensicEvent` (`src/integrations/neuronforge_local/forensics.rs`, `schemas/neuronforge-task-dispatch-forensic-event.schema.json`) and `NeuronForgeDispatchPipelineService` (`src/app/neuronforge_dispatch_pipeline_service.rs`): forensic recording for NeuronForge-Local dispatch runs, closing the gap Cortex Gnat dispatch also had before its own forensic-event contract and export sinks landed. Unlike Gnat (one negotiation event plus one per shard), one NeuronForge-Local dispatch run has no separate negotiation phase and records exactly one event; `outcome` (`completed`/`not_completed`/`dispatch_unavailable`) and `receipt_validation_status` (`valid`/`degraded`/`failed`, structurally required for `completed`/`not_completed`, forbidden for `dispatch_unavailable`) mirror the same truthful-outcome shape `GnatDispatchForensicEvent` uses for `shard_outcome`/`receipt_state`. `JsonlNeuronForgeForensicExportAdapter`/`SqliteNeuronForgeForensicStore` (`src/integrations/neuronforge_local/forensic_export.rs`) mirror `execute`'s and Gnat dispatch's own export sinks; a failed export fails the run closed, reusing `ForensicService::map_export_result`. Exposed via the same `--forensic-export`/`--forensic-sqlite` flags on `neuronforge-dispatch`. Live-testing this found and fixed a real bug (`KI-FLO-20260918-005`, `docs/KNOWN_ISSUES.md`): `summary` validation checked byte length against a string the pipeline's own truncation helper had already bounded by Unicode character count (matching the JSON Schema's own codepoint-based `maxLength`), so a real, long `ureq` connection-refused message failed closed instead of recording truthfully — the same latent bug existed in the already-shipped Gnat forensic-event validation and is now fixed in both. Live-verified end to end, both outcomes: a real completed dispatch and a real `dispatch_unavailable` (service stopped) both recorded and exported correctly via `--forensic-sqlite`, confirmed by querying the SQLite store directly.

What is still intentionally not delivered:

- broad cross-service adapter integrations beyond the Cortex Gnat proving slice and the first NeuronForge-Local proving slice above (which admits only the one task `ADR-002` names). DF-Local's execution-bridge writeback (above) is delivered; other DF-Local integration surfaces remain unstarted.
- daemon or API surfaces
- persistence layer beyond forensic evidence (e.g. durable policy/capability/execution state across restarts)

This is the current bounded baseline, not a claim that later execution-facing phases are already delivered.

## Foundational references

This section is grounded in:

- `README.md`
- `SYSTEM.md`
- `BOUNDARIES.md`
- `ROADMAP.md`
