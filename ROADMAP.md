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
   NeuronForge-Local, DF Local — instead of local-only delivery). The Cortex Gnat proving slice
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
   run still completes. Still open, disclosed rather than silently assumed away: no export sink
   (JSONL/SQLite) for Gnat dispatch forensic events yet — recording is in-memory only, returned
   to the caller. NeuronForge-Local and DF-Local integrations remain unstarted (DF Local is its
   own separate item below).
2. A daemon or networked API surface (FA Local stays a CLI binary with no HTTP surface by
   doctrine; this would need an explicit, separately-authorized architectural decision).
3. A persistence layer beyond forensic evidence (e.g. durable policy/capability/execution state
   across restarts).
4. The DataForge Local `execution_status_event` staging endpoint and writeback wiring
   (`src/integrations/df_local/mod.rs`; DataForge Local's Phase X4, not this repo's).
