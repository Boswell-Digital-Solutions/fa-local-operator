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

These checks remain bounded to validation, deny-path admission, pure decision output, bounded plan fingerprinting, truthful status shaping, deterministic internal routing, bounded internal coordination, explicit adapter-backed delivery over already selected admitted routes, a capability-scoped `AdapterRegistry` resolving one adapter per capability (both for a whole route in one call and per declared plan step), one concrete capability-scoped local-file-write adapter, one concrete Nmap preflight adapter that only reports declared local runtime availability, one bounded review-package emitter workflow for contract-compatible review-required and explicit-approval paths, and one bounded forensic recorder/export workflow over already-known execution truth, exported to an append-only local JSONL sink or a queryable local SQLite store.
They still do not perform semantic interpretation, planner behavior, or unbounded external invocation.

## Current implementation boundary

All currently planned baseline contracts now exist in schema-backed form.

Phase X4 added:
- `IntakeService` in `src/app/intake_service.rs` — the schema-validated entry point for external execution requests. It wraps `ExecutionRequest::load_contract_value()` and provides both `validate_request(&Value)` and `validate_request_bytes(&[u8])` convenience methods.
- `DecisionService` in `src/app/decision_service.rs` — composes intake, requester-trust evaluation, policy loading, and capability admission into one resolved `RouteDecision` from raw JSON inputs.
- `AdapterRegistry` in `src/adapters/execution_delivery/registry.rs` — a capability-to-adapter dispatch table; `ExecutionService::deliver_selected_route_via_registry` resolves one adapter for a whole route, and `ExecutionService::deliver_plan_per_step_via_registry` resolves one adapter per declared plan step, aggregating per-step outcomes (including `PartialSuccess`) truthfully.
- `JsonlForensicExportAdapter` (`src/adapters/exports/jsonl_forensic_export.rs`) and `SqliteForensicStore` (`src/adapters/exports/sqlite_forensic_store.rs`) — an append-only local JSONL sink and a queryable local SQLite store (bundled `rusqlite`, indexed by `correlation_id` and `event_type`), both implementing `ForensicEventExportAdapter`.
- `ExecutionPipelineService` in `src/app/execution_pipeline_service.rs` — composes all of the above into one bounded run: resolve a route decision, and only when it admits execution, validate the plan, dispatch through the adapter registry, and record forensic evidence for every truthful outcome (denied, review-required, plan-invalid, and admitted paths alike). `run()` takes both an optional implicit `AdapterSelection` (registered under the route's own top-level capability) and `additional_adapters: Vec<CapabilityScopedAdapterSelection>`, each registered under its own explicitly declared capability — how a heterogeneous multi-capability plan gets more than one adapter without hand-building the registry through the library API.
- `fa-local-run` CLI binary (`src/bin/fa_local_run.rs`) — `validate`, `route`, `execute` (plan validation, adapter dispatch via `--local-file-write-root`/`--nmap-binary`, `--per-step-dispatch`, repeatable `--adapter <CAPABILITY_UUID>:<local-file-write|nmap-preflight>:<PARAMS>` for additional capability-scoped adapters, forensic export via `--forensic-export`/`--forensic-sqlite`), `forensics-query`, `status`, and `canonical-status` (FC-LTA-P007's external service-status projection, see below).

The Nmap preflight adapter is bounded to a declared `local_process_spawn` capability and execution plan, does not run scans, does not accept free-form arguments, and does not create a networked daemon surface. Missing `nmap` runtime truth can be represented as a degraded execution status and recorded through the existing minimized forensic-event path. The review-package emitter remains intentionally bounded to the two current review postures only and does not introduce generic workflow behavior beyond `review_required` and `explicit_operator_approval`. Still not delivered: a daemon/API surface and persistence beyond forensic evidence.

Declared-fallback coordination across per-step-dispatched adapters is delivered: when a step's own attempt is `Failed`, `Unavailable`, or `Canceled` and the plan declares a fallback for it, `deliver_plan_per_step_via_registry` dispatches the declared fallback step immediately, out of its normal declared order, resolving the fallback step's *own* adapter from the registry (which may differ from the primary step's capability's adapter) rather than requiring the same adapter to know about the substitution. This is a coordinator-level retry across adapters, distinct from the whole-route path's `CompletedWithDeclaredFallback` (where a single adapter reports internally that it used a fallback it was told about); a per-step adapter reporting that result is still an unsupported condition, since a per-step request never carries fallback references. A fallback step already dispatched as one step's fallback target is never dispatched again for a different step declaring the same fallback — its real result is reused. A plan that completes entirely only because at least one step needed its declared fallback reports `CompletedWithConstraints`/`degraded_fallback_limited`, never `DegradedFallbackEquivalent` (the coordinator cannot verify two different capabilities are truly equivalent, only that a plan author declared one a fallback for the other).

## Cortex Gnat shard dispatch

`integrations::cortex::CortexSubprocessGnatShardAdapter` (`src/integrations/cortex/shard_dispatch.rs`) is the first piece of broad cross-service adapter integration: it dispatches one admitted Cortex Gnat shard by spawning Cortex's `cortex_runtime.gnats.shard_cli` (a new bounded CLI entry point added in the COR repo alongside this) as a subprocess, one shard per call, and parses back the `GnatWorkerReceipt.v1` it prints on stdout — live-verified against the real COR checkout (real `python3`, real worker, real fixture file), not just against a stub. This is the "later FA-Local dispatch adapter" `DECISIONS/0019` (COR repo) names as the missing counterpart to `GnatDispatchValidator::negotiate`'s admission-only decision, which on its own has no onward path to actually running a shard.

Bounded to the two worker types `DECISIONS/0018` (COR) authorizes for this proving slice (`markdown_syntax`, `plain_text_syntax`, via `AUTHORIZED_WORKER_TYPES`) — every other worker type the registry otherwise supports is refused before ever spawning a process, not left for Cortex's own CLI to reject. `GnatShardDispatchRequest` carries the full `GnatShard.v1` contract fields plus the real `local_path` that contract deliberately excludes (COR's `source_path_token` is a one-way derived diagnostic, not a reversible path reference).

`GnatShardDispatchRequest::from_declared_shard` bridges admission negotiation into a full runnable shard descriptor: `GnatDispatchShard` (the envelope's own negotiation-time shard summary) never carries `source_path_token`, `media_type`, the fingerprint beyond its bare digest, `max_bytes`, or `local_path`. `GnatShardEnrichment` supplies exactly those, keyed by `shard_id`; `from_declared_shard` merges an enrichment with its matching declared shard, taking every envelope-declared field (`shard_id`, `ordinal`, `worker_type`, `source_ref`, the fingerprint digest, `deadline_ms`) from the envelope alone. There is exactly one source of truth for each field this way — no separate caller-supplied copy that could disagree with what was actually negotiated.

`GnatDispatchPipelineService` (`src/app/gnat_dispatch_pipeline_service.rs`) closes the loop `negotiate` alone never did: build a full request for every declared shard via the bridge above, negotiate, and only when the negotiated posture is `ReadyForFaLocalDispatch`, deliver every shard through a `GnatShardDeliveryAdapter` and collect its real outcome. A denied run dispatches nothing; a `SerialFallbackPermitted` run also dispatches nothing here, since running the shards itself in that case is Cortex's own job (`DECISIONS/0019`), not this pipeline's. A declared shard with no enrichment supplied for it is refused before negotiation ever runs. Exposed as `fa-local-run gnat-dispatch --envelope <FILE> --shard-enrichment <FILE> --cortex-repo-root <DIR> [--cortex-python <BINARY>] [--forensic-export <FILE>|--forensic-sqlite <FILE>]`, exit 0 only when the run was dispatched and every shard's receipt reports `state: "complete"`. Live-verified end to end against the real COR checkout, both paths: a two-shard envelope (`markdown_syntax` + `plain_text_syntax`) with matching fingerprint digests negotiated, admitted, both shards dispatched to two real Cortex workers, both receipts `complete` (exit 0); the same envelope with deliberately stale digests reports both shards `stale` through the same real subprocess calls (exit 1).

Every run is also recorded as forensic truth via `integrations::cortex::GnatDispatchForensicEvent` (`schemas/gnat-dispatch-forensic-event.schema.json`) — a separate contract from `forensic-event.schema.json`, since that schema's `route_decision_id`/`execution_plan_id`/`ApprovalPosture`/`ExecutionState` fields have no equivalent for a Cortex-initiated run. `run()` takes an optional `forensic_export_adapter: Option<&dyn GnatForensicEventExportAdapter>` and returns `GnatDispatchRunResult { outcome, forensic_events }`: exactly one `gnat_dispatch_negotiated` event per run (`negotiation_outcome` one of `denied`/`serial_fallback_permitted`/`ready_for_fa_local_dispatch`), and, only for a dispatched run, one `gnat_shard_dispatched` event per declared shard, each carrying `worker_type`, `shard_outcome` (`completed`/`not_completed`/`dispatch_unavailable`), and `receipt_state` (the real receipt's own `state` field — structurally absent for `dispatch_unavailable`, structurally required otherwise). Each `GnatForensicRecordOutcome` in `forensic_events` carries its own `export_reference`: `None` when no adapter was supplied (the original in-memory-only behavior), `Some(reference)` when it was exported — a failed export fails the whole run closed (`FaLocalError`), the same doctrine `ForensicService::record_and_export_event_via` already enforces for FA Local's own forensic-event family, reusing its `map_export_result` validation rather than duplicating it. `fa-local-run gnat-dispatch --forensic-export <FILE>|--forensic-sqlite <FILE>` (mutually exclusive, both optional) wires `JsonlGnatForensicExportAdapter`/`SqliteGnatForensicStore` (`src/integrations/cortex/forensic_export.rs`) into this parameter; with neither flag, recording stays in-memory only.

Live-verified end to end: a real dispatch run against the COR checkout produced exactly 3 forensic events (1 negotiation + 2 shard), each independently valid against `gnat-dispatch-forensic-event.schema.json`; with `--forensic-sqlite`, the same 3 events land as 3 real rows in the SQLite store (verified by querying the file directly), and with `--forensic-export`, as 3 real JSONL lines.

The dispatch subprocess call is deadline-bounded: `CortexSubprocessGnatShardAdapter` puts the spawned Python interpreter in its own process group (`process_group(0)`) at spawn time, and `wait_with_deadline` polls for exit against the shard's own `deadline_ms`, killing the whole group (`libc::kill(-pid, SIGKILL)`, called directly rather than by shelling out to an external `kill` binary) if it's exceeded. `Child::kill()` alone only reaches the one process spawned directly; the interpreter can itself fork further processes (a shell wrapping it, say) that would otherwise survive and keep the stdout/stderr reader threads blocked indefinitely, defeating the deadline entirely — confirmed by direct reproduction (`KI-FLO-20260918-004`): the shelled-out `kill` binary reported success but delivered nothing in this crate's own sandboxed dev environment, while `Child::kill()`, called from the same process, worked correctly throughout. Live-verified against the real COR checkout: a shard given a 1ms deadline is killed promptly (`dispatch_unavailable`) while a sibling shard with a normal deadline in the same run still completes.

The execution bridge writeback path (`DfLocalAdapter::post_execution_status_event`, `src/integrations/df_local/mod.rs`) is wired: it serializes a `ValidatedExecutionStatus` into an `execution_status_event.v1` artifact envelope, computes the canonical proving-slice idempotency key (`forge_contract_core.identity.compute_idempotency_key`'s algorithm, cross-language-verified against the real Python implementation, not just shape-matched), and POSTs it to DataForge Local's `POST /api/v1/execution-bridge/status-events` (`dataforge-Local#35`, `DATAFORGE_LOCAL_URL` env var, default `http://127.0.0.1:8005`) via `ureq`. `promotion_class: "local_only"` keeps the artifact out of any cloud-egress path, matching `execution_trace_state`'s classification in `forge_contract_core`'s `repo_role_matrix.json`'s `local_only_truth_classes`. No signing-key infrastructure exists for this artifact family yet, so `signature` is a real SHA-256 digest of the canonical payload body rather than a cryptographic signature — schema-honest (an opaque string, not verified by `validate_artifact`), not a placeholder.

## NeuronForge Local task dispatch

`integrations::neuronforge_local::HttpNeuronForgeLocalAdapter` (`src/integrations/neuronforge_local/mod.rs`) is a first NeuronForge-Local proving slice, authorized by `neuronforge-local-operator`'s `ADR-002` (`docs/adr/ADR-002-fa-local-task-routing.md` in that repo, Accepted 2026-09-18): FA Local owns routing for exactly one bounded task, `analyze.style.scene.v1`. Unlike the Cortex Gnat adapter, transport is HTTP, not a spawned subprocess — ADR-002's own transport section explains why: FA Local dispatching over HTTP as a client does not conflict with its own "no HTTP surface" doctrine, which constrains what it serves, not what it calls. The same `ureq`-based client pattern `DfLocalAdapter` already uses against DataForge Local applies here, POSTing to NeuronForge Local's own `POST /api/v1/fa-local/task-dispatch` route (`NEURONFORGE_LOCAL_URL` env var, default `http://127.0.0.1:8000`).

`ADMITTED_TASK_ID` bounds `dispatch_task` to the one task ADR-002 names — any other `task_id` is refused before ever making a network call, mirroring how Cortex's `AUTHORIZED_WORKER_TYPES` refuses an unsupported worker type before spawning a process. NeuronForge Local's response already carries `semantic_result_posture: "non_canonical_candidate"` and `registry_guardrails` proving it cannot mutate registry state or promote a baseline (ADR-002's output-posture guarantee); this adapter passes the receipt through unmodified rather than re-deriving that posture. The response's `schema_validation_status` maps to a truthful `NeuronForgeTaskDispatchResult`: `Completed` for `"valid"`, `NotCompleted` for `"degraded"`/`"failed"` (a real receipt came back but the result wasn't usable — not a dispatch failure), and `DispatchUnavailable` for anything that never produced a real receipt at all (task not admitted, unreachable, non-2xx, unparseable body, or a missing/unrecognized `schema_validation_status`).

Exposed as `fa-local-run neuronforge-dispatch --scene <FILE> [--neuronforge-url <URL>] [--model <ID>]` (default model `qwen2.5:14b`, NeuronForge Local's own documented baseline), exit 0 only on `Completed`. Live-verified end to end against a real running `neuronforge-local-operator` service and a real local Ollama model: a real scene POSTed through the real CLI produced a genuine structured style-analysis candidate (`schema_validation_status: "valid"`, every `registry_guardrails` flag `false`, exit 0); with the service stopped, the same CLI call correctly reported `dispatch_unavailable` with a truthful connection-refused summary (exit 1).

Still not delivered: forensic recording for NeuronForge-Local dispatch runs — the same disclosed gap Cortex Gnat dispatch had before its own forensic-event contract and export sinks landed.

## External service-status projection (FC-LTA-P007)

FA Local exposes this projection through `domain::service_status::build_canonical_service_status_envelope()`, reached from the `fa-local-run canonical-status` CLI subcommand. Forge_Command reads it the same way it already reads Cortex's projection: as a CLI subprocess. It runs the compiled `fa-local-run` binary with the `canonical-status` argument and parses the one line of JSON on stdout. FA Local adds no HTTP surface for this — it stays a CLI binary only, consistent with `CLAUDE.md`'s "no HTTP surface" doctrine.

FA Local has no existing whole-service status computation to project from — its only status concept, `domain::status::ExecutionStatus`, is per-request. `domain::service_status::operational_facts()` is the single real source of truth this projection and the plain `status` subcommand both read: `execution_enabled: true` and `writeback_wired: true` (`DfLocalAdapter::post_execution_status_event` now serializes and POSTs a real artifact to DataForge Local's Phase X4 endpoint, `dataforge-Local#35`). Both facts remain structural in intent — read from what code exists (a working `execute` subcommand and a working writeback POST path), not a runtime probe: `writeback_wired: true` means the code path exists, not that DataForge Local is reachable at this exact moment.

The plain `status` subcommand's old `posture: "policy_first_admission"` field was never derived from a real check and has been removed; it never appears in the canonical projection either.

With `execution_enabled: true` and `writeback_wired: true`, the canonical projection reports `state: "ready"` with no `degraded_subtype` (optional in the canonical schema, omitted rather than set to `null` when not applicable) — both real structural facts are now positive. `build_canonical_service_status_envelope()` fails loudly (`FaLocalError::InternalInvariant`) on any other combination of the two facts, so a future change that flips either fact back to `false` must update this projection in the same change, not silently keep reporting `ready`.

The external schema is vendored at `schemas/forge_local_runtime/`, kept separate from FA Local's own 12-member `SchemaName` registry (`src/domain/shared/schema.rs`) because it is a different, external, `additionalProperties: false` contract owned by `forge-local-systems-runtime`, not by FA Local. The vendored `service-status.schema.json`'s `denied_state` field `$ref`s `./denial-state.schema.json`; because the `jsonschema` crate resolves every `$ref` eagerly at validator-build time (unlike Python's `jsonschema`, which resolves lazily per validated branch) and this crate has no `resolve-http` feature enabled, the denial-state resource is registered in-memory at its declared `$id` URI rather than fetched over the network.

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
- `src/adapters/exports/jsonl_forensic_export.rs`
- `src/adapters/exports/sqlite_forensic_store.rs`
- `src/adapters/execution_delivery/mod.rs`
- `src/adapters/execution_delivery/registry.rs`
- `src/adapters/execution_delivery/local_file_write.rs`
- `src/adapters/execution_delivery/nmap_preflight.rs`
- `src/app/decision_service.rs`
- `src/app/execution_pipeline_service.rs`
- `src/app/execution_service.rs`
- `src/app/forensic_service.rs`
- `src/app/intake_service.rs`
- `src/app/review_service.rs`
- `src/app/routing_service.rs`
- `src/bin/fa_local_run.rs`
- `src/domain/service_status/mod.rs`
- `schemas/forge_local_runtime/service-status.schema.json`
- `docs/fa_local_codex_build_plan_v_1.md`
