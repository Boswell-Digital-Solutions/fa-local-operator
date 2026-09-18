# Known Issues

Findings tracked here per the Forge workspace-wide protocol (`/home/charlie/Forge/CLAUDE.md`):
a bug, a root cause, a gap, or an open tracking state goes here, checked first before a finding
is reported as new, and written in the same session it is found.

---

## KI-FLO-20260918-001 — `operational_facts().execution_enabled` is hardcoded `false`, now stale

**Date found:** 2026-09-18
**Status:** open

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

**Fix, if any:** Not fixed in this pass — this was found while refreshing `doc/system/` and the
root docs for session handoff, not while doing execution-pipeline work, and changing what
`execution_enabled` means (and what `canonical-status` should report once it can be `true`) is a
real design decision — e.g. whether it should reflect "the `execute` subcommand exists" versus
"at least one adapter is registered and reachable" versus something else — that deserves its own
reviewed change, consistent with how every other capability this session shipped as its own PR.
`build_canonical_service_status_envelope()` already fails loudly (`FaLocalError::InternalInvariant`)
if `operational_facts()` ever reports a combination it has no honest envelope for, so flipping
`execution_enabled` to `true` without also updating the envelope logic will crash that function
rather than silently emitting a wrong-but-plausible status — the next session picking this up
should treat that as the forcing function, not an obstacle.

**Scope:** open. Affects `src/domain/service_status/mod.rs`, the `status` and `canonical-status`
CLI subcommands, and Forge_Command's FC-LTA-P007 read of FA Local's projected state. Does not
affect `writeback_wired: false`, which remains accurate (DataForge Local's Phase X4 staging
endpoint still does not exist).

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
