# fa-local-operator — Compiled System Reference

**Designation:** FLO
**Document role:** Canonical compiled technical reference for the ForgeAgents-Local operator (FA-Local)
**Source:** `doc/system/`
**Build command:** `bash doc/system/BUILD.sh`
**Document version:** 2.0 (2026-06-19) — BDS canonical-compliance migration (7-group class-aware structure, truth classes, designation-bound fail-closed assembly, authored governance trio)
**Protocol:** BDS Documentation Protocol v2.0; BDS Repo Documentation System Canonical Compliance Standard

> **Generated artifact warning:** `doc/FLOSYSTEM.md` is assembled output. Edit the
> source modules under `doc/system/` and rebuild. Hand edits to the compiled
> artifact are overwritten by the next build.

Assembly contract:

- Command: `bash doc/system/BUILD.sh`
- Validation: `bash doc/system/validate_snapshots.sh` runs during assembly
- Primary output: `doc/FLOSYSTEM.md`

This `doc/system/` tree is the canonical source of truth for fa-local-operator. It
uses explicit **truth classes**: *canonical facts* define the routing role, swarm
boundaries, contract-surface conformance, and bounded-worker discipline;
*snapshot facts* are dated, audit-derived counts (modules, tests, slices).
fa-local-operator is the FA-Local routing Gnat in the local Cortex swarm. See §5
for the scope/authority boundary and §6 for ownership and designation doctrine.

| Part | File | Contents |
| --- | --- | --- |
| §1 | `00_overview/01-overview-charter.md` | Identity, charter, FA-Local routing role |
| §2 | `10_service-contract/02-contract-surface.md` | Contract surface |
| §3 | `20_runtime/03-execution-bridge-writeback.md` | Execution bridge & writeback |
| §4 | `30_dependencies/04-dependencies.md` | Swarm peers + contract dependencies |
| §5 | `40_governance/05-scope.md` | Service authority boundary, bounded-worker discipline, truth classes |
| §6 | `40_governance/06-governance.md` | Ownership, designation doctrine, authority hierarchy |
| §7 | `40_governance/07-change-control.md` | Change classes, evidence, verification commands |
| §8 | `40_governance/08-boundaries-and-doctrine.md` | Detailed boundaries & doctrine |
| §9 | `50_operations/09-validation-and-delivery.md` | Validation & delivery |
| §10 | `99_appendices/10-appendices.md` | Glossary, cross-references, revision history |

## Quick Assembly

```bash
bash doc/system/BUILD.sh
```
