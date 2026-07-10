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
