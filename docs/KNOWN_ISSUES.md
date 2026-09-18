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
