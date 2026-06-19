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
- deterministic contract fixtures and deny smoke coverage
- latest `jsonschema` validator release aligned in the crate dependency set

## Not yet delivered

The following planned surfaces are explicitly not delivered yet:

- any second adapter or multi-adapter runtime surface
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
- first typed intake boundary present (`IntakeService`)
- first CLI binary surface present (`fa-local-run`)
- first typed writeback stub present (`DfLocalAdapter::post_execution_status_event` — not yet wired)
- contract gate runner present (`ci_gate.sh`)
- no full external FA Local runtime surface admitted yet

That wording matters because the crate now has meaningful contract, deny-path, posture-resolution, bounded plan-validation, truthful status, bounded review-handoff behavior, a bounded review-package emitter workflow for both current review postures, minimal forensic-event truth behavior, a bounded forensic recorder/export workflow, bounded operator-friction behavior, deterministic internal routing behavior, bounded internal coordination behavior, a narrow adapter-backed delivery seam, one concrete capability-scoped adapter, a typed intake entry point, a CLI binary, and a typed writeback stub — but it still does not ship persistence, a concrete forensic export sink, a second adapter, generic workflow orchestration, or a networked API/daemon runtime surface.
