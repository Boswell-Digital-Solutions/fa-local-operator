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
