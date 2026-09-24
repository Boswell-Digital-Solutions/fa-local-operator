# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

fa-local-operator (FLO) is the governed local execution operator for Forge applications — a business/internal ForgeAgents worker handling capability admission, bounded-plan validation, dispatch, adapter execution, review handoff, and status evidence. It is the business-side system at `ecosystem/local-systems/fa-local-operator`, implemented separately from `forge-local-runtime` (which remains the governance-and-contracts authority for the shared local runtime layer).

**Not `forge-fa-local`.** Same brand, two families: this repo is the business-side backend/ecosystem execution boundary; `apps/public-app-local-support/forge-fa-local` is the unrelated public-app support variant. Path decides which one owns the behavior.

Current status: bounded baseline delivered, not a scaffold. Schema-backed contracts, requester-trust/policy/capability admission, approval-posture resolution, bounded execution-plan validation, a capability-scoped `AdapterRegistry` for multi-adapter and per-step dispatch, JSONL and SQLite forensic export, a DataForge Local execution-bridge writeback path (Phase X4 — `DfLocalAdapter::post_execution_status_event`), and a CLI (`route`, `execute`, `forensics-query`, `validate`, `status`, `canonical-status`) all exist and are tested. Still genuinely not delivered: broad cross-service adapter integrations, declared-fallback coordination across steps dispatched to different adapters, a daemon/API surface, and persistence beyond forensic evidence — see `doc/system/00_overview/01-overview-charter.md`, the canonical current-baseline reference (`ROADMAP.md` and the other root pointer files predate this and are being superseded by it).

## Common Commands

- Build/test: `cargo build`, `cargo test`
- Contract gate: `bash ci_gate.sh` — execution bridge v1 contract participation. There is no GitHub workflow; `ci_gate.sh` is the gate, and `cargo test` covers the Rust suites.
- Context bundle listing: `./scripts/context-bundle.sh --list`
- CLI: `./target/debug/fa-local-run --help` lists all subcommands (`validate`, `route`, `execute`, `forensics-query`, `status`, `canonical-status`); `execute --help`-equivalent detail is in the same `--help` output, including adapter-selection and dispatch-mode flags.

## Architecture

The crate is structured inside-out:

- `domain/` — core vocabulary and pure decision primitives
- `app/` — orchestration services that compose domain logic without absorbing policy authority (`decision_service`, `execution_pipeline_service`, `execution_service`, `routing_service`, `forensic_service`, `review_service`, `intake_service`)
- `adapters/` — storage, schema, clock, hashing, and export boundaries (`execution_delivery/` adapters + `AdapterRegistry`; `exports/` JSONL and SQLite forensic sinks)
- `integrations/` — keeps Cortex, NeuronForge Local, and DF Local behind explicit contracts. DF Local's execution-bridge writeback (Phase X4) is wired: `DfLocalAdapter::post_execution_status_event` POSTs a real `execution_status_event.v1` artifact to DataForge Local's `/api/v1/execution-bridge/status-events` (`dataforge-Local#35`). Cortex's Gnat dispatch proving slice is fully delivered (negotiate → dispatch → forensics → deadline enforcement → export sinks). NeuronForge Local has a first proving slice (`HttpNeuronForgeLocalAdapter`, one ADR-002-admitted task, no forensic recording yet).

FLO is the validating dispatcher of the local plane: **it validates plans and dispatches; it does not extract or prepare** (that is COR's job), and it is not a general-purpose executor.

- **Deny by default.** Capability admission is a gate, not a lookup — `denial-guard.schema.json` defines the refusal shape, and a refusal is a first-class outcome, not an error path.
- Plans are **bounded**. A plan that cannot be validated is not run.
- Forensic events and status evidence are append-only.
- Canonical reference: `doc/FLOSYSTEM.md`, assembled from `doc/system/` via `bash doc/system/BUILD.sh`.
- Contracts: [`schemas/`](schemas/) — `requester-trust`, `policy-artifact`, `capability-registry`, `execution-request`, `execution-plan`, `execution-status`, `route-decision`, `denial-guard`, `review-package`, `forensic-event`, `friction-payload`, with worked cases in `schemas/examples/`.
- See `BOUNDARIES.md`, `CAPABILITIES.md`, `POLICY.md`, `FORENSICS.md`, `REVIEWS.md`, and `ROADMAP.md` for a per-subsystem summary; `doc/system/00_overview/01-overview-charter.md` is the canonical current-vs-not-yet-delivered reference and wins on conflict.

## Notes

- Do not invent undocumented APIs, tables, routes, or environment variables.
- `ci_gate.sh` resolves forge-contract-core by relative path (`../../contracts/forge_contract_core`, falling back to `forge-contract-core`) and preferentially uses that repo's `.venv/bin/python`. A moved or renamed sibling checkout makes the gate fail loudly at the path check — and a missing `.venv` silently downgrades it to bare `python3`. Confirm which interpreter it printed.
- FLO's `serve` subcommand (`BDS-FAL-DAEMON-v0.1`) is its only HTTP-serving surface: one read-only route, `GET /api/v1/capabilities/{capability_id}`, default-off unless `FA_LOCAL_SERVE_ENABLED` is set, port 8011 by default. It is not a general HTTP service -- no write-capable route, no execution-over-HTTP route, and `route`/`execute` stay CLI-only. Everything else FA Local does over HTTP remains client-only (`integrations/df_local`, `integrations/neuronforge_local`).
- `forge-local-runtime` remains the doctrine and shared-vocabulary authority; this repo implements FA Local within those bounds.
