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
