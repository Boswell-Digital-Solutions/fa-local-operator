# Known Issues

Findings tracked here per the Forge workspace-wide protocol (`/home/charlie/Forge/CLAUDE.md`):
a bug, a root cause, a gap, or an open tracking state goes here, checked first before a finding
is reported as new, and written in the same session it is found.

---

## KI-FLO-20260918-001 — `operational_facts().execution_enabled` is hardcoded `false`, now stale

**Date found:** 2026-09-18
**Status:** closed (fixed same session)

**What is wrong:** `domain::service_status::operational_facts()`
(`src/domain/service_status/mod.rs`) unconditionally returns
`execution_enabled: false`. This value feeds both the plain `status` CLI subcommand and the
canonical `canonical-status` projection consumed by Forge_Command (FC-LTA-P007), which currently
reports `state: "degraded"` / `degraded_subtype: "degraded_pre_start"` on the premise that "the
execution bridge has not started."

**Root cause:** `execution_enabled: false` was accurate when this function was written, because no
`execute` subcommand existed yet — the fact was structural (derived from the absence of code), not
a runtime probe. `fa-local-run execute` was added in the same working period that ended up
shipping `ExecutionPipelineService`, `AdapterRegistry`-backed dispatch, and JSONL/SQLite forensic
export (PRs #5–#11 on `fa-local-operator`), but `operational_facts()` itself was never revisited
to match. It is now a stale hardcoded value, not a true structural fact.

**Fix:** `execution_enabled` is now `true` — the same kind of structural, code-presence fact it
always was, not a runtime probe: `fa-local-run execute` exists and dispatches admitted plans
through the `AdapterRegistry`, so there is no longer an absence of code to justify `false`.
`build_canonical_service_status_envelope()` was updated in the same change (it would otherwise
have hit its own `InternalInvariant` fail-loud guard the moment `execution_enabled` flipped): with
`execution_enabled: true` and `writeback_wired: false`, it now reports `state: "degraded"` /
`degraded_subtype: "unavailable_dependency_block"` — core validation and dispatch work, but
forensic status events cannot be staged to DataForge Local because its Phase X4 endpoint does not
exist yet. The guard clause itself was kept (inverted to match the new valid combination) so a
future change that also flips `writeback_wired` to `true` still fails loudly instead of silently
carrying forward this message. Also updated the plain `status` subcommand's `note` string, which
made the same "execution bridge ... pending Phase X4" claim.

**Scope:** closed. Changed `src/domain/service_status/mod.rs` (`operational_facts()`,
`build_canonical_service_status_envelope()`, its doc comments, and its three unit tests) and the
`status` subcommand's note string in `src/bin/fa_local_run.rs`. `writeback_wired: false` is
unchanged and remains accurate (DataForge Local's Phase X4 staging endpoint still does not exist).

---

## KI-FLO-20260918-002 — Root pointer docs and `doc/system/` had drifted well behind the code

**Date found:** 2026-09-18
**Status:** closed (fixed same session)

**What was wrong:** `CLAUDE.md`, `ROADMAP.md`, `POLICY.md`, `CAPABILITIES.md`, and `REVIEWS.md` all
still described fa-local-operator as an early scaffold with policy loading, capability admission,
and review-package handoff "not yet implemented" — false since well before this session started.
`doc/system/10_service-contract/02-contract-surface.md` and
`doc/system/50_operations/09-validation-and-delivery.md` explicitly listed "no persistence layer,
no concrete forensic export sink, and no multi-adapter dispatch or runtime selection surface in
the current baseline" — also false after PRs #5–#11 shipped exactly those things.

**Root cause:** `doc/system/00_overview/01-overview-charter.md` was kept current as part of each
PR's own change-control requirement (per this repo's C0 rule), but the other `doc/system/`
chapters and the root pointer `.md` files were not — they were written once early on and never
revisited as the "current scaffold-vs-planned boundary" they claim to be.

**Fix:** Rewrote `CLAUDE.md`'s status paragraph and architecture notes, `ROADMAP.md`,
`POLICY.md`, `CAPABILITIES.md`, `REVIEWS.md`, and the stale sections of
`02-contract-surface.md`/`09-validation-and-delivery.md` to match current reality, then rebuilt
`doc/FLOSYSTEM.md` via `bash doc/system/BUILD.sh`.

**Scope:** closed for the files touched. `docs/fa-local_architecture_spec.md` and
`docs/fa-local_extended_roadmap.md` were reviewed and left alone — both are explicitly
self-scoped as deferring to `doc/system`/`doc/FLOSYSTEM.md` on conflict rather than asserting
current implementation state themselves, so they were not making false claims. `docs/README.md`
stub directories (`architecture/`, `contracts/`, `controls/`, `doctrine/`, `risks/`) are empty
placeholders, not stale claims, and were left alone.

---

## KI-FLO-20260918-003 — `mentions_fallback()` false-positives on any step id or adapter text that happens to contain "fallback"

**Date found:** 2026-09-18
**Status:** closed (fixed same session)

**What is wrong:** `ExecutionStatus::validate()` (`src/domain/status/mod.rs:111-120`), and the
matching guards in `ValidatedForensicEvent` (`src/domain/forensics/mod.rs:126`) and
`ValidatedFrictionPayload` (`src/domain/friction/mod.rs:130`), reject any status/event/payload
whose `completion_summary`, `failure_summary`, or `truthful_user_visible_summary` contains the
case-insensitive substring `"fallback"` unless `degraded_subtype` is explicitly
`DegradedFallbackEquivalent`/`DegradedFallbackLimited`. Found live-testing `fa-local-run execute
--per-step-dispatch` (PRs #14/#15) with a plan step named `step_export_fallback`: the in-progress
status's message (`"executing externally delivered step step_export_fallback"`,
`build_in_progress_statuses_from_route` in `src/app/execution_service.rs`) embeds the step id
verbatim, has no `degraded_subtype` at all (in-progress statuses never set one), and the run
hard-errored with `contract invalid: execution status cannot mention fallback without an explicit
fallback degraded_subtype` instead of returning a normal execution trace.

**Root cause:** `mentions_fallback()` (`src/domain/status/mod.rs:626-630`) is a blunt
case-insensitive substring scan over free-text fields that legitimately embed untrusted,
operator/plan-author-controlled text: a step id (`is_valid_step_id`, same file, only checks
length 1-48 and charset — nothing stops the substring "fallback" appearing in an otherwise
ordinary step name) via `format!("executing externally delivered step {step_id}")` and
`format!("executing declared step {step_id}")`, and an adapter's own `failure_summary`
(`AdapterDeliveryResult::FailedAtDeclaredStep`, validated only for length 1-160 chars, not
content). The guard's real intent — catching a caller that *claims* a fallback outcome in text
without tagging the matching `degraded_subtype` — cannot distinguish that from a step or adapter
message that merely contains the word for unrelated reasons. This is independent of the new
per-step fallback coordination shipped in PR #15: it predates that change and would trip on the
*pre-existing* per-step in-progress path just as easily for any step named e.g.
`build_fallback_ui`, fallback coordination or not.

**Fix:** Removed `mentions_fallback()` and its guard entirely from all three modules
(`src/domain/status/mod.rs`, `src/domain/forensics/mod.rs`, `src/domain/friction/mod.rs`), rather
than narrowing its text scan. Investigation showed the guard was fully redundant with an
already-existing *structural* invariant in the same `validate()` functions: `ExecutionState::
CompletedWithConstraints` already hard-requires `degraded_subtype` to be
`DegradedFallbackEquivalent`/`DegradedFallbackLimited` (`src/domain/status/mod.rs:370-378`,
mirrored in the forensic and friction `validate_degraded_subtype` helpers) — the *only* state
where a genuine "completed via fallback" claim is ever legitimate. For every other state, the
text-scan added no real protection (nothing else ever legitimately claims a fallback completion)
while being actively harmful: for `ExecutionState::PartialSuccess`, `degraded_subtype` is
structurally required to be exactly `DegradedPartial` — so any `failure_summary` that happened to
mention "fallback" (plausible from an adapter's own text) would have created an unsatisfiable
conflict between that requirement and the removed guard's, hard-failing every such run with no
possible valid status to construct. `is_explicit_fallback_subtype()` was kept where the structural
checks still use it (forensics, friction); removed as dead code where it was only reachable from
the deleted guard (status). Repurposed the three tests that exercised the old guard
(`tests/execution_status_invariants.rs`, `tests/forensic_event_invariants.rs`;
`tests/friction_payload_invariants.rs`'s equivalent test already exercised the *structural* check,
not the guard, so it needed no change) into regression guards asserting the opposite: mentioning
"fallback" in text alone must not fail validation. Re-ran the exact originally-failing scenario
(same plan, same `step_export_fallback` step name, same hash) against the fixed binary and
confirmed it now completes normally (`exit=0`) instead of hard-erroring.

**Scope:** closed. Changed `src/domain/status/mod.rs`, `src/domain/forensics/mod.rs`,
`src/domain/friction/mod.rs`, `tests/execution_status_invariants.rs`, and
`tests/forensic_event_invariants.rs`.

---

## KI-FLO-20260918-004 — shelling out to an external `kill` binary silently no-ops for process-group signals in this dev sandbox

**Date found:** 2026-09-18
**Status:** closed (fixed same session, before ever shipping the broken version)

**What is wrong:** `CortexSubprocessGnatShardAdapter`'s deadline enforcement
(`src/integrations/cortex/shard_dispatch.rs`) needs to kill a whole process group when a Cortex
Gnat shard subprocess outruns its `deadline_ms` -- `Child::kill()` alone only signals the one
process directly spawned, and the Python interpreter it runs can itself fork further processes
(a shell wrapping the interpreter, for instance) that survive the direct kill and keep the
stdout/stderr pipes open indefinitely, defeating the deadline entirely. The first implementation
put the child in its own process group at spawn time (`std::os::unix::process::CommandExt::process_group(0)`,
stable stdlib) and signaled it by spawning `Command::new("kill").arg("-9").arg(format!("-{pid}"))`
-- shelling out to the external `kill` binary rather than a syscall. That spawned `kill` process
reported exit code 0 (success) on every invocation, but the target process group was still alive
afterward every time, reproduced both in a `cargo test` unit test (a fake shell script sleeping
10s past a 200ms deadline; the test blocked for the full 10s) and in a manual, minimal repro
outside any FA Local code (`/usr/bin/kill -9 -$PGID` against a plain backgrounded `sleep`, in this
same sandboxed dev environment: exit code 0, process still alive). Bash's own `kill` *builtin*
(not the external binary) delivered the identical signal correctly in the same shell session,
confirming the gap is specific to a spawned `kill` *process* attempting a group-signal in this
sandbox, not group-signaling in general.

**Root cause:** Not fully understood at the syscall level (no visibility into why this specific
sandbox differentiates a builtin's `kill()` call from an external process's `kill()` call for a
negative-pid target), but conclusively reproduced and narrow: `Child::kill()` (std's own direct
single-process kill, used internally by the same code path) worked correctly every time, ruling
out kill-signal delivery being broken in general in this environment.

**Fix:** Stopped shelling out to an external `kill` binary. `kill_process_group` now calls
`libc::kill(-(pid as libc::pid_t), libc::SIGKILL)` directly (one `unsafe` block, plain integer
arguments, no pointers) from within the same process that already successfully calls
`Child::kill()` -- added the `libc` crate as a new dependency for this. The same unit test that
reproduced the bug (10s to return) now passes in 0.2s; also live-verified against the real COR
checkout with a shard given a 1ms deadline: the real subprocess is killed promptly
(`dispatch_unavailable`, forensic event recorded), while a sibling shard with a normal deadline
in the same run still completes.

**Scope:** closed. Changed `Cargo.toml` (added `libc`) and
`src/integrations/cortex/shard_dispatch.rs`. Worth remembering for any other local-plane tooling
in this ecosystem that shells out to `kill` (or any external signal-delivery binary) for
process-group management on this class of dev machine -- prefer a direct `libc` syscall.

---

## KI-FLO-20260918-005 — forensic-event `summary` validation checked byte length against a char-count-truncated string

**Date found:** 2026-09-18
**Status:** closed (fixed same session)

**What is wrong:** `GnatDispatchForensicEvent::validate()` (`src/integrations/cortex/forensics.rs`)
and `NeuronForgeTaskDispatchForensicEvent::validate()` (`src/integrations/neuronforge_local/forensics.rs`)
both rejected `summary` strings that their own pipeline's `bounded_summary()` helper
(`src/app/gnat_dispatch_pipeline_service.rs`, `src/app/neuronforge_dispatch_pipeline_service.rs`)
had already truncated to fit. Found live-testing `fa-local-run neuronforge-dispatch` against a
stopped NeuronForge Local service: the real `ureq` connection-refused message (well over 160
characters) was truncated by `bounded_summary()` as designed, but the resulting event still
failed to validate, turning a truthful `dispatch_unavailable` outcome into a hard `FaLocalResult`
error instead -- the whole run failed closed on recording its own degraded-but-real outcome.

**Root cause:** `bounded_summary()` truncates by Unicode *character* count --
`text.chars().take(159).collect()` plus one trailing `'…'` character, exactly 160 codepoints --
matching the JSON Schema `maxLength: 160` constraint on `summary` in both
`schemas/gnat-dispatch-forensic-event.schema.json` and
`schemas/neuronforge-task-dispatch-forensic-event.schema.json` (JSON Schema's `maxLength` is
Unicode-codepoint-based per spec, confirmed independently against Python's `jsonschema` library).
Both `validate()` methods instead checked `self.summary.len() > 160` -- Rust's `String::len()` is
*byte* length, not char count. `'…'` (U+2026) alone encodes to 3 UTF-8 bytes, so any ASCII text
truncated to exactly 159 chars plus the ellipsis is 160 codepoints but 162 bytes: the schema
(codepoint-based) accepts it, the Rust check (byte-based) rejected it. This bug existed in the
already-merged Gnat forensic-event code (PR #21) from the start -- it happened to never trigger
there because every `dispatch_unavailable` summary Gnat's own code produces stayed under the
byte/char threshold in practice, so it went undetected until a genuinely long real-world message
(a `ureq` transport error, not a synthetic test string) exercised it for the first time.

**Fix:** Both `validate()` methods now check `self.summary.chars().count() > 160` instead of
`self.summary.len() > 160`, matching the JSON Schema's own codepoint-based semantics and
`bounded_summary()`'s truncation unit. Added a regression test to each pipeline-service test file
(`tests/gnat_dispatch_pipeline_service.rs`, `tests/neuronforge_dispatch_pipeline_service.rs`)
reproducing the exact failure with a 300-character all-ASCII `DispatchUnavailable` summary and
asserting the recorded event's `summary` is exactly 160 codepoints and validates. Re-ran the
originally-failing live scenario (`fa-local-run neuronforge-dispatch` against a stopped NeuronForge
Local service, `--forensic-sqlite`) and confirmed it now truncates, records, and exports correctly
(`exit=1`, truthful `dispatch_unavailable`, not a hard error).

**Scope:** closed. Changed `src/integrations/cortex/forensics.rs`,
`src/integrations/neuronforge_local/forensics.rs`, `tests/gnat_dispatch_pipeline_service.rs`, and
`tests/neuronforge_dispatch_pipeline_service.rs`. Worth checking for the same byte-vs-codepoint
mismatch in any future forensic-event-style contract that pairs a JSON Schema `maxLength` with a
truncate-then-validate pipeline step -- the schema's own unit is codepoints, not bytes.

---

## KI-FLO-20260918-006 — ROADMAP item 3 (persistence layer) has no live gap to solve while item 2 (daemon) is unauthorized

**Date found:** 2026-09-18
**Status:** closed (scoping finding, not a code defect; recorded so the roadmap item isn't
re-opened as if it were independently actionable)

**What is wrong:** `ROADMAP.md`'s item 3, "a persistence layer beyond forensic evidence (e.g.
durable policy/capability/execution state across restarts)," reads as an independently
actionable open item. It is not: there is currently no running FA Local process whose state
could be lost across a restart to justify one.

**Root cause:** FA Local is a CLI binary, not a daemon. Every invocation
(`fa-local-run route`/`execute`/`gnat-dispatch`/`neuronforge-dispatch`) takes its policy,
capability-registry, and requester-trust inputs as file arguments and exits; there is no
in-memory server state between invocations. Item 3 implicitly presumes item 2 (a daemon or
networked API surface) already exists, but item 2 is itself unauthorized -- checked both
`forge-local-runtime` (the accepted doctrine repo) and an extensive Google Drive research-corpus
review, and found zero precedent or analysis proposing FA Local gain an inbound daemon/HTTP
surface.

Separately, `forge-local-runtime`'s accepted boundary doctrine (`BOUNDARIES.md`,
`ARCHITECTURE.md`, `DECISIONS/0005-falocal-boundary.md`, Status: Accepted) already settles *who*
would own durable state if this ever becomes real: FA Local's own "does not own" list explicitly
names "hidden persistence authority," and DF Local Foundation (`dataforge-Local` in this
ecosystem) is the accepted owner of "local database lifecycle, migrations, backup/restore/export
doctrine... bounded recovery and integrity support." Checked DataForge Local's actual API
surface (`standalone/dataforge-Local`, synced to `origin/master`) for a reusable pattern: its
only FA-Local-facing route, `POST /api/v1/execution-bridge/status-events` (Phase X4, the same
route `DfLocalAdapter::post_execution_status_event` already POSTs to), is write-only staging with
no read-back surface, and its own docstring says it was never meant to be one ("performs storage
mechanics only... does not re-derive or second-guess FA Local's own execution semantics").

**Fix:** N/A -- not a defect. `ROADMAP.md` item 3 rewritten to state explicitly that it is
blocked on item 2 and not independently actionable, and to record the ownership answer
(DF Local Foundation, not FA Local) so a future session doesn't have to re-derive it.

**Scope:** closed for this scoping question. If item 2 (the daemon) is ever separately
authorized, item 3 should be re-opened as its own concrete proposal at that point, informed by
whatever state a running FA Local process actually turns out to need -- not drafted speculatively
ahead of that decision.

---

## KI-FLO-20260918-007 — no next NeuronForge-Local task admission is justified (GATE-00 finding, `BDS-FAL-NFL-ADMISSION-v0.1`)

**Date found:** 2026-09-18
**Status:** closed (scoping finding, not a code defect)

**What was investigated:** whether a second NeuronForge-Local task should be admitted for
FA-Local dispatch, beyond the one `ADR-002` (`neuronforge-local-operator` repo) already admits
(`analyze.style.scene.v1`). Investigated under a Drive-hosted governed plan,
`BDS-FAL-NFL-ADMISSION-v0.1` (`/Forge/Plans/BDS_FAL_NFL_ADMISSION_v0.1_PLAN_SET/`, Charlie
Boswell authorized CP0/WP00 only: read-only source-truth, task inventory, and candidate
selection -- no implementation, no task admission).

**Source-lock:** all five primary repo heads (`fa-local-operator`, `neuronforge-local-operator`,
`forge-local-runtime`, `dataforge-Local`, `Forge_Command`) reproduced with zero drift against the
plan's recorded pins. `fa-local-operator` PR #25 / `KI-FLO-20260918-006` confirmed present at
`master`.

**NeuronForge-Local task inventory:** beyond the already-admitted `analyze.style.scene.v1`, found
two other candidates with real, stable contracts:

- `analyze.continuity.adjacent_scene.v1` -- a more formally documented contract than
  `analyze.style.scene.v1` itself (`doc/system/10_service-contract/20-analyze-continuity-adjacent-scene-v1.md`
  in `neuronforge-local-operator`: schema-validated request/response, fail-closed envelope, hard-fail
  triggers, PACT context-lineage support), but no existing HTTP route -- only bash+Python CLI
  scripts (`scripts/run-continuity-adjacent-scene.sh` and helpers), and a heavier `HIGH_QUALITY_LOCAL`
  route class than `analyze.style.scene.v1`'s `WORKHORSE_LOCAL`.
- `drift-analysis` -- simple, fully deterministic (no model call at all), already has an HTTP
  route (`POST /api/v1/authorforge/drift-analysis`).

Also found and rejected as candidates: `run_beat_candidate_bakeoff.sh` (no formal `task_id` or
doc/system contract, ad-hoc comparison script) and `cor_gnat_semantic_handoff` (literal-typed to
Cortex as originator, not admissible for FA-Local per its own schema -- the same finding recorded
when scoping `ADR-002` originally).

**The decisive finding:** neither surviving candidate has a demonstrated FA-Local-side consumer.
`fa-local-operator` has zero references to either task anywhere. Both are already consumed
directly by AuthorForge without FA-Local involvement (`apps/api/src/routes/drift.ts`,
`apps/api/src/adapters/ai-contract.ts`) -- which is not itself disqualifying (the already-admitted
`analyze.style.scene.v1` is *also* consumed directly by AuthorForge, e.g.
`StyleAnalysisPanel.svelte`/`neuroforge.ts`, alongside its FA-Local path), but there is no evidence
of an actual FA-Local-mediated need for either: no work order, plan, or code anywhere asks FA-Local
to dispatch them, and AuthorForge's own vendored copy of `fa-local-operator`
(`apps/Author-Forge/third_party/fa_local/`) is stale -- last touched 2026-08-06, predating even the
*first* NeuronForge-Local dispatch slice built earlier in this same session, confirming the
desktop app does not yet call `fa-local-run neuronforge-dispatch` for anything, including the
already-admitted task.

**Fix:** N/A -- not a defect. Both candidates pass every technical admission criterion in
`BDS-FAL-NFL-ADMISSION-v0.1`'s rejection-condition list, but fail its preference signal
("extends a real current workflow rather than adding speculative capability"). GATE-00 closed
with disposition **no next task admission justified**: `drift-analysis` held (cheapest to admit
later if a real need appears -- deterministic, already HTTP-exposed); `analyze.continuity.adjacent_scene.v1`
held (would also need a new NeuronForge-Local-side HTTP route built first, unlike `drift-analysis`
or the already-admitted task); `run_beat_candidate_bakeoff.sh` rejected (no stable contract). No
CP1 was activated.

**Scope:** closed for this GATE-00 decision. Per the plan's own terms, "no admission justified" is
a valid, successful CP0 outcome, not a stalled one. Re-open only if a concrete FA-Local-side
consumer or workflow need for `drift-analysis` or `analyze.continuity.adjacent_scene.v1` is
identified in the future -- do not re-run this evidence cycle speculatively.

---

## KI-FLO-20260924-001 — `fa-local-run serve` defaulted to port 8011, which the registry gives to context-runtime

**Date found:** 2026-09-24
**Status:** closed (fixed same session)

**What is wrong:** `DEFAULT_SERVE_PORT` (`src/adapters/serve/http_server.rs`) was 8011. The
`serve` help text and `CLAUDE.md` said the same. The forge `PORT_REGISTRY.md` gives 8011 to
context-runtime (forge#210). context-runtime binds 8011 by default, and Forge_Command, forgeHQ,
and AuthorForge address it there. With `FA_LOCAL_SERVE_ENABLED` set, the daemon and
context-runtime could not both start on one machine.

**Root cause:** The registry listed 8011 as reserved, but context-runtime and ForgeMath already
bound it. Two changes then claimed the row on the same day. forge#208 claimed it for this daemon;
forge#210 registered it for context-runtime and moved ForgeMath to 8006. forge#210 merged first,
so forge#208 conflicted and did not merge. This daemon (#27) merged after that, still defaulting
to 8011. No check compares port defaults with the registry (forge `KI-FORGE-20260923-009`).

**Fix:** `DEFAULT_SERVE_PORT` is 8012, the next free Agent Layer port. None of the 41
repositories checked out in this session uses 8012. The help text, `CLAUDE.md`, and the scoping packet (open item 2, with an
amendment note) say 8012. forge#208 now claims 8012 in `PORT_REGISTRY.md`. `--port` still
overrides the default.

**Scope:** Closed. The daemon is default-off, so only an install with `FA_LOCAL_SERVE_ENABLED`
set ever bound 8011. A script that passes `--port 8011` still collides and must change.

---

## KI-FLO-20260924-002 — `doc/system`, `CLAUDE.md`, and `ROADMAP.md` still say FA Local has no daemon or API surface

**Date found:** 2026-09-24
**Status:** closed (fixed 2026-09-24)

**What is wrong:** `fa-local-run serve` (#27, `BDS-FAL-DAEMON-v0.1`) adds one read-only HTTP
route. Three documents still say that no such surface exists:

- `doc/system/00_overview/01-overview-charter.md` lists "daemon or API surfaces" under "still
  intentionally not delivered". `CLAUDE.md` names this chapter as the canonical current-versus-
  not-delivered reference, and it wins on conflict. `doc/FLOSYSTEM.md` is assembled from it.
- The `CLAUDE.md` status paragraph lists "a daemon/API surface" as not delivered. The Notes
  section of the same file describes `serve`.
- `ROADMAP.md` item 2 says FA Local "stays a CLI binary with no HTTP surface by doctrine". Item 3
  says persistence is blocked on item 2.

**Root cause:** The file allowlist in `02_IMPLEMENTATION_SCOPING_PACKET.md` covered the code, the
tests, and one `CLAUDE.md` line. It did not list `doc/system/` or `ROADMAP.md`, so #27 left them
unchanged.

**Fix:** The update changed these files:

- `doc/system/00_overview/01-overview-charter.md` lists `serve` as delivered, keeps only broader
  routes as not delivered, and drops the "no daemon" premise from the persistence item.
- `doc/system/10_service-contract/02-contract-surface.md` replaces its "no HTTP surface" and "no
  daemon/API surface" statements with the one `serve` route and its limits.
- `doc/system/50_operations/09-validation-and-delivery.md` limits its "not delivered" statements
  to surfaces beyond `serve` and adds `serve`, `gnat-dispatch`, and `neuronforge-dispatch` to its
  subcommand list.
- `CLAUDE.md` lists the daemon and declared-fallback coordination as delivered and adds the same
  three subcommands to both subcommand lists.
- `ROADMAP.md` item 2 records the narrow daemon that `BDS-FAL-DAEMON-v0.1` authorized, and item 3
  no longer depends on item 2.
- `bash doc/system/BUILD.sh` rebuilt `doc/FLOSYSTEM.md` and reported `BUILD_OK`.

**Scope:** Closed. The charter, `CLAUDE.md`, `ROADMAP.md`, and `doc/FLOSYSTEM.md` describe `serve`
as delivered, with its limits. The plan set under `docs/plans/` is a record and stays unchanged.
The legacy `doc/faSYSTEM.md` is non-canonical (§6) and stays unchanged. `KI-FLO-20260924-003`
tracks other stale text that this sweep found.

---

## KI-FLO-20260924-003 — `doc/system` §3 and §9, `CLAUDE.md`, and one `serve` doc comment disagree with the code

**Date found:** 2026-09-24
**Status:** open

**What is wrong:** The `KI-FLO-20260924-002` sweep found stale text that `serve` does not cause:

- `doc/system/20_runtime/03-execution-bridge-writeback.md` describes Phase X3. It says that
  `post_execution_status_event` returns `FaLocalError::WritebackNotWired` and that FA Local needs
  an HTTP client. The code POSTs through `ureq`, and `writeback_wired` is `true`. The envelope
  table gives `promotion_class` as `promotable`; the code sends `local_only`.
- `doc/system/50_operations/09-validation-and-delivery.md` says that Gnat dispatch has no forensic
  export sink and that NeuronForge-Local and DF-Local integrations are unstarted. It lists the
  DataForge Local staging-endpoint wiring as not delivered and calls the writeback a stub. The §1
  charter lists all of this work as delivered. The §9 command list omits `serve`, `gnat-dispatch`,
  and `neuronforge-dispatch`. Its test list omits the `serve` and NeuronForge-Local suites and
  most Gnat suites.
- The `CLAUDE.md` Architecture section says that NeuronForge Local has "no forensic recording
  yet". `NeuronForgeDispatchPipelineService` records one forensic event per run and can export it.
- The `ServeService::reload` doc comment (`src/app/serve_service.rs`) names a `--watch` path.
  `serve` has no `--watch` flag; only `SIGHUP` reloads. The scoping packet's 503 case also names a
  failed re-load, but the code and packet test case 11 keep the previous good registry.

**Root cause:** Same as `KI-FLO-20260918-002`. Each PR updates the §1 charter, but not always the
other chapters or `CLAUDE.md`. The proposal and the scoping packet both mention a `--watch`
refresh. The binary implements only `SIGHUP`, and the doc comment kept the `--watch` name.

**Fix:** None yet. Update §3, §9, and the `CLAUDE.md` Architecture section to match §1 and the
code. Remove `--watch` from the doc comment. Then run `bash doc/system/BUILD.sh`. The plan set
under `docs/plans/` is a record and stays unchanged.

**Scope:** Open. Close this entry when §3, §9, `CLAUDE.md`, and the doc comment match the code.
