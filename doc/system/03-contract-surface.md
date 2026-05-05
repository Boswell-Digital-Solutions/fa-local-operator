# 3. Contract Surface

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

These checks remain bounded to validation, deny-path admission, pure decision output, bounded plan fingerprinting, truthful status shaping, deterministic internal routing, bounded internal coordination, explicit adapter-backed delivery over already selected admitted routes, one concrete capability-scoped local-file-write adapter, one bounded review-package emitter workflow for contract-compatible review-required and explicit-approval paths, and one bounded forensic recorder/export workflow over already-known execution truth.
They still do not perform semantic interpretation, planner behavior, or unbounded external invocation.

## Current implementation boundary

All currently planned baseline contracts now exist in schema-backed form.

Phase X4 added:
- `IntakeService` in `src/app/intake_service.rs` — the schema-validated entry point for external execution requests. It wraps `ExecutionRequest::load_contract_value()` and provides both `validate_request(&Value)` and `validate_request_bytes(&[u8])` convenience methods.
- `fa-local-run` CLI binary (`src/bin/fa_local_run.rs`) — a synchronous binary providing `validate` and `status` subcommands for local operator use.

There is no persistence layer, no concrete forensic export sink, and no second adapter or multi-adapter runtime surface in the current baseline. The review-package emitter remains intentionally bounded to the two current review postures only and does not introduce generic workflow behavior beyond `review_required` and `explicit_operator_approval`.

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
- `src/app/execution_service.rs`
- `src/app/forensic_service.rs`
- `src/app/intake_service.rs`
- `src/app/review_service.rs`
- `src/app/routing_service.rs`
- `src/bin/fa_local_run.rs`
- `docs/fa_local_codex_build_plan_v_1.md`
