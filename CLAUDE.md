# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

fa-local-operator (FLO) is the governed local execution operator for Forge applications — a business/internal ForgeAgents worker handling capability admission, bounded-plan validation, dispatch, adapter execution, review handoff, and status evidence. It is the business-side system at `ecosystem/local-systems/fa-local-operator`, implemented separately from `forge-local-runtime` (which remains the governance-and-contracts authority for the shared local runtime layer).

**Not `forge-fa-local`.** Same brand, two families: this repo is the business-side backend/ecosystem execution boundary; `apps/public-app-local-support/forge-fa-local` is the unrelated public-app support variant. Path decides which one owns the behavior.

Current status: early scaffold. The crate builds, exposes typed baseline vocabulary, and defaults toward fail-closed admission. Contract schemas, artifact loaders, and execution coordination are intentionally not implemented yet — see `ROADMAP.md`.

## Common Commands

- Build/test: `cargo build`, `cargo test`
- Contract gate: `bash ci_gate.sh` — execution bridge v1 contract participation. There is no GitHub workflow; `ci_gate.sh` is the gate, and `cargo test` covers the Rust suites.
- Context bundle listing: `./scripts/context-bundle.sh --list`

## Architecture

The crate is structured inside-out:

- `domain/` — core vocabulary and pure decision primitives
- `app/` — orchestration services that compose domain logic without absorbing policy authority (planned)
- `adapters/` — storage, schema, clock, hashing, and export boundaries (planned)
- `integrations/` — keeps Cortex, NeuronForge Local, and DF Local behind explicit contracts (planned)

FLO is the validating dispatcher of the local plane: **it validates plans and dispatches; it does not extract or prepare** (that is COR's job), and it is not a general-purpose executor.

- **Deny by default.** Capability admission is a gate, not a lookup — `denial-guard.schema.json` defines the refusal shape, and a refusal is a first-class outcome, not an error path.
- Plans are **bounded**. A plan that cannot be validated is not run.
- Forensic events and status evidence are append-only.
- Canonical reference: `doc/FLOSYSTEM.md`, assembled from `doc/system/` via `bash doc/system/BUILD.sh`.
- Contracts: [`schemas/`](schemas/) — `capability-registry`, `execution-request`, `execution-plan`, `execution-status`, `denial-guard`, `friction-payload`, `forensic-event`, with worked cases in `schemas/examples/`.
- See `BOUNDARIES.md`, `CAPABILITIES.md`, `POLICY.md`, `FORENSICS.md`, `REVIEWS.md`, and `ROADMAP.md` for the current scaffold-vs-planned boundary on each subsystem.

## Notes

- Do not invent undocumented APIs, tables, routes, or environment variables.
- `ci_gate.sh` resolves forge-contract-core by relative path (`../../contracts/forge_contract_core`, falling back to `forge-contract-core`) and preferentially uses that repo's `.venv/bin/python`. A moved or renamed sibling checkout makes the gate fail loudly at the path check — and a missing `.venv` silently downgrades it to bare `python3`. Confirm which interpreter it printed.
- FLO is a Rust CLI with no HTTP surface, so it cannot be supervised as a local HTTP service. Anything that needs to reach it over a port needs a different host process.
- `forge-local-runtime` remains the doctrine and shared-vocabulary authority; this repo implements FA Local within those bounds.
