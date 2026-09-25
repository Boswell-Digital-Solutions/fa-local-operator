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

fa-local-operator is the **business/internal ForgeAgents-Local worker** — a
bounded, deterministic execution worker in the federated local plane. Its role is
**execution**: requester and capability admission, bounded-plan validation,
deterministic dispatch, adapter execution, review handoff, and status evidence.
Within the local plane, Cortex (`COR`) prepares and extracts (file intelligence,
retrieval preparation, handoff packages — it does not sequence workflows or select
executors), NeuronForge-Local supplies model intelligence, DataForge-Local
persists, and Yellowjacket governs workcell admission and lane routing above the
worker. fa-local-operator admits and dispatches capabilities and bridges execution
results back through a contract surface — it does not sequence workflows, decide,
or persist canonical truth.

## What fa-local-operator Owns

- **Capability admission & deterministic dispatch** within the local FA-Local lane
  — requester/capability admission, bounded-plan validation, and adapter execution.
- **Contract-surface conformance** (§2) — honoring its contract surface exactly.
- **Execution-bridge writeback & review handoff** (§3) — bridging execution results
  and status evidence back across the contract boundary.
- **Validation & delivery** (§9) of its own bounded behavior.

## What fa-local-operator Does Not Own

- **Preparation / extraction.** Cortex (`COR`) owns file intelligence and retrieval
  preparation — not workflow planning or executor selection.
- **Workcell admission & lane routing.** Yellowjacket resolves and pins the approved
  skill/workcell and routes the lane; fa-local-operator executes within it.
- **Model intelligence.** NeuronForge-Local supplies inference/embeddings/LoRAs.
- **Durable persistence.** DataForge-Local persists the local plane's operational truth.
- **Decision / canonical truth or orchestration.** ForgeCommand is the
  operator/control plane.

## Bounded-Worker Discipline

fa-local-operator is a bounded, deterministic worker: it fails closed on ambiguity,
stays within its capability-admission / dispatch / execution-bridge lane, and never
expands into workflow planning, workcell routing, model intelligence, persistence,
or decision authority.

## Release / Readiness Language Restrictions

This documentation describes an internal service under governed, slice-by-slice
development. It must be described as a verification-current internal service, not
as externally release-certified, and must not claim public-release/SaaS readiness
or present coverage percentages as guarantees unless a later governed slice proves
the specific claim.

## Documentation truth classes

- **Canonical facts** define fa-local-operator's execution role (capability
  admission, dispatch, execution bridge), local-plane boundaries,
  contract-surface conformance, and bounded-worker discipline. They change only
  through deliberate change control (§7).
- **Snapshot facts** are audit-derived counts (modules, tests, slices) labelled
  with a measurement date and corrected by re-measurement, not change control.

Ownership, designation doctrine, and the authority hierarchy that govern this tree
are defined in §6; detailed boundaries and doctrine in §8.
