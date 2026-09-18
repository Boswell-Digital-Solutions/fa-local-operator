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
- The DataForge Local `execution_status_event` staging endpoint (`dataforge-Local#35`) and this
  repo's writeback wiring (`DfLocalAdapter::post_execution_status_event`,
  `src/integrations/df_local/mod.rs` — DataForge Local's Phase X4). `domain::service_status`'s
  `writeback_wired` fact now reads `true`, and the FC-LTA-P007 canonical status projection reports
  `state: "ready"` accordingly.
- JSONL/SQLite export sinks for `GnatDispatchForensicEvent`
  (`src/integrations/cortex/forensic_export.rs`, `JsonlGnatForensicExportAdapter`/
  `SqliteGnatForensicStore`), mirroring `execute`'s own sinks for a separate contract.
  `GnatDispatchPipelineService::run` takes an optional export adapter; a failed export fails the
  whole run closed, reusing the same validation `execute`'s own forensic records already use
  rather than duplicating it. Exposed as `fa-local-run gnat-dispatch --forensic-export <FILE>|
  --forensic-sqlite <FILE>`. Live-verified end to end against the real COR checkout: both sinks
  independently produced 3 real records from the same dispatch run.
- A first NeuronForge-Local proving slice: `integrations::neuronforge_local::HttpNeuronForgeLocalAdapter`
  dispatches the one task neuronforge-local-operator's `ADR-002` admits
  (`analyze.style.scene.v1`) to its `POST /api/v1/fa-local/task-dispatch` route over HTTP
  (`ureq`, mirroring `DfLocalAdapter`'s client pattern, not a spawned subprocess). Any other
  `task_id` is refused before ever making a network call. Exposed as `fa-local-run
  neuronforge-dispatch --scene <FILE> [--neuronforge-url <URL>] [--model <ID>]`. Live-verified
  end to end against a real running `neuronforge-local-operator` service and a real local Ollama
  model (`qwen2.5:14b`): a real scene produced a genuine structured style-analysis candidate,
  `schema_validation_status: "valid"`, every `registry_guardrails` flag `false`.
- Forensic recording and JSONL/SQLite export sinks for NeuronForge-Local dispatch runs
  (`integrations::neuronforge_local::NeuronForgeTaskDispatchForensicEvent`,
  `schemas/neuronforge-task-dispatch-forensic-event.schema.json`,
  `NeuronForgeDispatchPipelineService`), closing the gap Cortex Gnat dispatch also had before
  its own forensic-event contract and export sinks landed. One dispatch run records exactly one
  event (no separate negotiation phase, unlike Gnat). Exposed as `fa-local-run
  neuronforge-dispatch --forensic-export <FILE>|--forensic-sqlite <FILE>`. Live-testing this
  found and fixed a real bug (`KI-FLO-20260918-005`): `summary` validation checked byte length
  against a string the pipeline's own truncation helper had already bounded by Unicode character
  count, so a real, long `ureq` connection-refused message failed closed instead of recording
  truthfully — the same latent bug existed in the already-merged Gnat forensic-event validation
  too and is now fixed in both. Live-verified end to end, both outcomes: a real completed
  dispatch and a real `dispatch_unavailable` (service stopped) both recorded and exported
  correctly via `--forensic-sqlite`.

Not yet delivered, in no particular priority order:

1. Broad cross-service adapter integrations (adapters that reach real peer services — Cortex,
   NeuronForge-Local — instead of local-only delivery). The Cortex Gnat proving slice
   is delivered end to end and live-verified against the real COR checkout: (COR repo)
   `cortex_runtime/gnats/shard_cli.py`, a bounded, spawnable single-shard CLI entry point;
   (this repo) `integrations::cortex::CortexSubprocessGnatShardAdapter`, which spawns it and
   parses back a `GnatWorkerReceipt.v1`; `GnatShardDispatchRequest::from_declared_shard` and
   `GnatShardEnrichment`, the negotiation-to-dispatch bridge (an envelope-declared
   `GnatDispatchShard` merged with a caller-supplied `GnatShardEnrichment` supplying exactly the
   fields the envelope never carries — `source_path_token`, `media_type`, the fingerprint beyond
   its bare digest, `max_bytes`, and the real `local_path`, which by design no schema-validated
   contract ever carries); and `GnatDispatchPipelineService` (`fa-local-run gnat-dispatch`),
   which composes `GnatDispatchValidator::negotiate` with the bridge and the adapter so an
   admitted run's declared shards are actually dispatched, not just admitted — the first real
   admission-to-dispatch code path this repo has, and records a
   `GnatDispatchForensicEvent` (`schemas/gnat-dispatch-forensic-event.schema.json`, a separate
   bounded contract from `forensic-event.schema.json` since that schema's fields have no
   equivalent for a Cortex-initiated run) for every negotiation outcome and every dispatched
   shard's own outcome. All bounded to the two worker types `DECISIONS/0018` (COR) authorizes
   for this proving slice (`markdown_syntax`, `plain_text_syntax`). The dispatch subprocess call
   is deadline-bounded: `kill_process_group` signals the whole process group (`libc::kill` on a
   negative pid, not a shelled-out `kill` binary — see `KI-FLO-20260918-004`) if a shard's own
   `deadline_ms` is exceeded, so an interpreter that spawns further processes can never hold the
   call open past its declared bound. Live-verified end to end, all paths: matching fingerprint
   digests dispatch and complete against two real Cortex workers, recording 3 real,
   independently schema-valid forensic events (exit 0); deliberately stale digests report
   `stale` through the same real subprocess calls (exit 1); an unrealistically tight deadline on
   one shard against the real COR checkout is killed promptly while a sibling shard in the same
   run still completes. A first NeuronForge-Local proving slice (task dispatch to
   `analyze.style.scene.v1`, forensic recording and export sinks, above) and DF-Local's
   execution-bridge writeback (above) are both delivered; no further NeuronForge-Local task is
   admitted beyond the one `ADR-002` names.
2. A daemon or networked API surface (FA Local stays a CLI binary with no HTTP surface by
   doctrine; this would need an explicit, separately-authorized architectural decision).
3. A persistence layer beyond forensic evidence (e.g. durable policy/capability/execution state
   across restarts) — **blocked on item 2, not independently actionable**. `forge-local-runtime`'s
   accepted boundary doctrine (`BOUNDARIES.md`, `ARCHITECTURE.md`, `DECISIONS/0005-falocal-boundary.md`)
   already settles *who* would own it: FA Local's own "does not own" list explicitly names
   "hidden persistence authority," and DF Local Foundation is the accepted owner of "local
   database lifecycle, migrations, backup/restore/export doctrine... bounded recovery and
   integrity support." What doesn't yet exist is a live gap to solve: FA Local is a CLI, not a
   daemon, and every invocation takes its policy, capability-registry, and requester-trust inputs
   as file arguments and exits — there is no in-memory process state to lose across restarts.
   DataForge Local's only FA-Local-facing route today (`POST /api/v1/execution-bridge/status-events`,
   Phase X4) is write-only staging, with no read-back surface, and was never meant to be one (its
   own docstring: "performs storage mechanics only... does not re-derive or second-guess FA
   Local's own execution semantics"). A durable-state proposal only becomes concrete once item 2
   is itself authorized and defines what state a running FA Local process would actually need to
   survive a restart.
