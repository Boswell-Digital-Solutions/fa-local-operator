# §3 — Execution Bridge & Writeback

## Purpose

This section documents the planned writeback path for FA Local execution status
artifacts into the shared proving-slice pipeline.

FA Local produces truthful execution status events. Those events must travel to
DataForge Local's staging queue, be promoted to DataForge Cloud, and become
visible in Forge_Command's execution review surface — following exactly the same
pipeline as `source_drift_finding` artifacts in proving slice 01.

## Writeback path

```
FA Local
  ↓ post_execution_status_event()
DataForge Local staging queue  (execution_status_event v1 artifact)
  ↓ local promotion
DataForge Cloud intake         (shared truth ingestion)
  ↓ read model
Forge_Command execution review surface
```

## Artifact family

Writeback artifacts are in the `execution_status_event` v1 family, defined in
`forge-contract-core/contracts/families/execution_status_event/`. They travel
in the shared envelope format.

Key envelope fields for writeback artifacts:

| Field | Value |
|-------|-------|
| `artifact_family` | `execution_status_event` |
| `artifact_version` | `1` |
| `produced_by_system` | `fa-local-operator` |
| `produced_by_component` | `execution_service.status_emitter` |
| `source_scope` | `local` |
| `promotion_class` | `promotable` |

## Implementation boundary

The `DfLocalAdapter.post_execution_status_event()` method in
`src/integrations/df_local/mod.rs` establishes the typed contract boundary.

**Current state (Phase X3):** Method signature and `ExecutionStatusWritebackRequest`/
`ExecutionStatusWritebackResult` types are defined. The method returns
`FaLocalError::WritebackNotWired` until Phase X4 completes the real endpoint.

**Required for Phase X4:**

1. DataForge Local needs an `execution_status_event` staging endpoint
   (mirrors the existing `source_drift_finding` staging path)
2. FA Local needs an HTTP client dependency (e.g., `reqwest` or `ureq`)
3. `post_execution_status_event()` must:
   - Serialize `ValidatedExecutionStatus` into `execution_status_event` v1 payload
   - Build the full shared envelope with idempotency key
   - POST to DataForge Local's configured local API address
   - Return `ExecutionStatusWritebackResult { acknowledged: true }` on success

## Authority constraints

FA Local may NEVER:
- Write directly to DataForge Cloud
- Write source_drift_finding, promotion_envelope, or promotion_receipt artifacts
- Write lifecycle transitions (ForgeCommand owns those)
- Invent or modify approval decisions

FA Local may ONLY:
- POST `execution_request` and `execution_status_event` artifacts to DataForge Local
- Consume `approval_artifact` artifacts from Forge_Command (via DataForge Local)

## Approval artifact consumption path

```
Forge_Command
  ↓ approval_artifact (v1)
DataForge Cloud        (shared truth storage)
  ↓ DataForge Local polls or subscribes
DataForge Local
  ↓ approval delivery to FA Local
FA Local execution_service  (resumes waiting_explicit_approval path)
```

This path is not yet implemented. It will be designed in Phase X4 alongside
the writeback endpoint work.
