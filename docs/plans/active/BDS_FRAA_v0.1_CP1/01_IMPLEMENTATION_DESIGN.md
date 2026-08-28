# CP1 Implementation Design

## Native FA Local path

```text
FraaReconnaissanceInput.v0
  -> requester / policy / capability admission
  -> exact six-step ExecutionPlan validation + stable hash
  -> policy-preapproved RouteDecision
  -> root-confined SealedCorpusReadAdapter
  -> DeterministicReuseReconnaissanceEngine
  -> frozen FraaReconnaissanceResult.v0 + self-hash
  -> existing ExecutionService truthful completion trace
  -> independent test-harness oracle comparison
```

## Why `local_file_read` is explicit

Read-only evidence access is not represented as a write, process-spawn, or generic governed action. The capability registry therefore gains `local_file_read`, while the side-effect class remains `none`. This preserves reviewability and prevents a later executor from silently broadening the operation.

## Candidate-only contracts

The FRAA schemas remain repo-local under `schemas/`. They are not added to `forge_contract_core`, its artifact-family registry, or its producer/consumer role matrix.

## Deterministic selection

Donors are classified from visible evidence using fixed priority and hold/reject rules. Topology selection filters hard blockers and target constraints, then chooses the lowest complexity rank with a stable identifier tie-break. Every output collection is sorted before self-hashing.
