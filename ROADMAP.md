# FA Local Roadmap

The original near-term execution order (core scaffold, schema-backed contracts, requester-trust
evaluation, capability-registry admission, policy loading and coupling, posture resolution,
bounded plan validation and hashing, execution coordination, review-package emission, forensic
recording and export) is complete. `doc/system/00_overview/01-overview-charter.md` is the
canonical current-baseline reference; this file only tracks what is still open.

Delivered since the list below was last trimmed:

- CLI configuration of more than one adapter per `execute` run: the repeatable `--adapter
  <CAPABILITY_UUID>:<local-file-write|nmap-preflight>:<PARAMS>` flag
  (`src/bin/fa_local_run.rs`) registers additional capability-scoped adapters alongside the
  route's own implicit one, via `ExecutionPipelineService::run`'s new `additional_adapters`
  parameter (`Vec<CapabilityScopedAdapterSelection>`). A heterogeneous multi-capability plan run
  with `--per-step-dispatch` no longer needs every adapter registered by hand through the library
  API.
- Declared-fallback coordination across steps dispatched to different adapters in the per-step
  delivery path (`ExecutionService::deliver_plan_per_step_via_registry`): when a step's own
  attempt is `Failed`, `Unavailable`, or `Canceled` and the plan declares a fallback for it, the
  coordinator dispatches the declared fallback step out of order to *its own* registry-resolved
  adapter (which may differ from the primary step's). A fallback step already consumed by one
  failed step is never dispatched a second time for another step declaring the same fallback. A
  plan that completes entirely, but only because one or more steps needed their fallback, reports
  `CompletedWithConstraints`/`degraded_fallback_limited` (never `DegradedFallbackEquivalent`, since
  the coordinator cannot verify two different capabilities are truly equivalent) rather than a
  plain `Completed`.

Not yet delivered, in no particular priority order:

1. Broad cross-service adapter integrations (adapters that reach real peer services — Cortex,
   NeuronForge-Local, DF Local — instead of local-only delivery).
2. A daemon or networked API surface (FA Local stays a CLI binary with no HTTP surface by
   doctrine; this would need an explicit, separately-authorized architectural decision).
3. A persistence layer beyond forensic evidence (e.g. durable policy/capability/execution state
   across restarts).
4. The DataForge Local `execution_status_event` staging endpoint and writeback wiring
   (`src/integrations/df_local/mod.rs`; DataForge Local's Phase X4, not this repo's).
