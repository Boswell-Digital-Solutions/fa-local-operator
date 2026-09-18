# FA Local Roadmap

The original near-term execution order (core scaffold, schema-backed contracts, requester-trust
evaluation, capability-registry admission, policy loading and coupling, posture resolution,
bounded plan validation and hashing, execution coordination, review-package emission, forensic
recording and export) is complete. `doc/system/00_overview/01-overview-charter.md` is the
canonical current-baseline reference; this file only tracks what is still open.

Not yet delivered, in no particular priority order:

1. Broad cross-service adapter integrations (adapters that reach real peer services — Cortex,
   NeuronForge-Local, DF Local — instead of local-only delivery).
2. Declared-fallback coordination across steps dispatched to different adapters in the per-step
   delivery path (`ExecutionService::deliver_plan_per_step_via_registry`).
3. CLI configuration of more than one adapter per `execute` run (heterogeneous multi-capability
   plans currently need every relevant adapter registered by hand through the library API).
4. A daemon or networked API surface (FA Local stays a CLI binary with no HTTP surface by
   doctrine; this would need an explicit, separately-authorized architectural decision).
5. A persistence layer beyond forensic evidence (e.g. durable policy/capability/execution state
   across restarts).
6. The DataForge Local `execution_status_event` staging endpoint and writeback wiring
   (`src/integrations/df_local/mod.rs`; DataForge Local's Phase X4, not this repo's).
