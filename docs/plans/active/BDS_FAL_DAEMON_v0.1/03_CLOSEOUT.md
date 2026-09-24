# 03 — Closeout

## Outcome

`BDS-FAL-DAEMON-v0.1` is verified complete.

| Evidence | Value |
|---|---|
| Implementation PR | `Boswell-Digital-Solutions/fa-local-operator#27` — merge `ba8e3524bbec97c1c9f5bbd662a7944431304c52` |
| Port correction PR | `#29` — merge `3e7b39f96ac214e8a40056dcc13ef4cbfbc3a368` |
| Documentation-sweep PR | `#30` — merge `f53e34f0950ffc72d1fef47d055baa37e3152016` |
| Current `fa-local-operator` head verified against | `f53e34f0950ffc72d1fef47d055baa37e3152016` (`master`) |
| Companion registry/port PRs | `Boswell-Digital-Solutions/Forge_Command#360`, `Boswell-Digital-Solutions/forge#208` (superseded, see below), `#209`, `#212` |
| Runtime posture | Default off (`FA_LOCAL_SERVE_ENABLED` unset by default) |

## What shipped, against the scoping packet's own file allowlist

Every file `02_IMPLEMENTATION_SCOPING_PACKET.md` authorized was written, and nothing else:
`src/adapters/serve/{mod,http_server,token_verify}.rs`, `src/app/serve_service.rs`, three test
files, `Cargo.toml`/`Cargo.lock`, the `serve` arm in `src/bin/fa_local_run.rs`, module
declarations, and `CLAUDE.md`.

One route, `GET /api/v1/capabilities/{capability_id}`, `tiny_http`-backed, Ed25519 JWS auth
(`jsonwebtoken`/`ed25519-dalek`, `scope == "capability:read"` required), fail-closed on an
unconfigured key map, `SIGHUP`-triggered reload that keeps the previous good registry on a failed
reload. Nothing in `admit_execution_request` or the `route`/`execute` file-argument path changed.

## The port correction (#29) — a real collision the plan's own port claim missed

`01_...md`/`02_...md` and the companion `forge#208` PR claimed port `8011` for this daemon,
following `PORT_REGISTRY.md`'s own "claim before coding" rule — the registry listed 8011 as
reserved. It was not actually free: `context-runtime` and `ForgeMath` were already binding it in
real running code, undocumented in the registry. `forge#210` (a different, unrelated PR) registered
8011 for `context-runtime` and moved ForgeMath to 8006, and merged first — so `forge#208`'s claim
conflicted and this repo's own default port would have collided with a live service on any machine
running both with `FA_LOCAL_SERVE_ENABLED` set.

`#29` moved this daemon's default to `8012` — the port `forge#208` should have claimed had the
collision been visible at the time — and is reflected in `PORT_REGISTRY.md`'s current Agent Layer
table. `forge#212` (merged the same day) added `scripts/check-port-registry.py`, a real automated
check against running-code port defaults across the ecosystem's sibling checkouts, closing the
class of gap that let this happen in the first place (`KI-FORGE-20260924-001`/`-002`,
`KI-FORGE-20260923-009`, all in the `forge` root repo's own `docs/KNOWN_ISSUES.md`).

**Net assessment:** the plan's own governance discipline (claim before coding) worked exactly as
intended — it surfaced a real, pre-existing, undocumented collision instead of silently shipping
into it, and every artifact this closeout evidences reflects the corrected port, not the originally
claimed one.

## Test allowlist verification (against `f53e34f`, not just at merge time)

Re-ran independently, not assumed from the implementation PR's own report:

| # | Case (`02_...md`'s Test allowlist) | Verified |
|---|---|---|
| 1 | Well-formed, present `capability_id` → 200 with the record | `a_well_formed_capability_id_present_in_the_registry_returns_200_with_the_record` — pass |
| 2 | Well-formed, absent `capability_id` → 404 | `a_well_formed_capability_id_absent_from_the_registry_returns_404` — pass |
| 3 | Malformed `{capability_id}` → 400 before lookup/auth | `a_malformed_capability_id_returns_400_before_any_registry_lookup_or_auth_check` — pass |
| 4 | Missing `Authorization` header → 401 | `missing_authorization_header_is_denied` — pass |
| 5 | Unknown `kid` → 401 | `a_kid_not_present_in_the_configured_public_keys_is_denied` — pass |
| 6 | Wrong `scope` → 401 | `a_token_with_anything_other_than_exactly_capability_read_scope_is_denied` — pass |
| 7 | Expired token → 401 | `a_token_past_its_exp_is_denied` — pass |
| 8 | Empty/unset key map fails closed | `an_empty_public_key_map_denies_every_request_even_a_well_formed_token` — pass |
| 9 | `FA_LOCAL_SERVE_ENABLED` unset → no listener at all | `serve_enabled_env_var_unset_means_the_daemon_never_starts_a_listener` — pass |
| 10 | `SIGHUP` reload adds a capability | `reload_after_editing_the_registry_file_makes_a_new_capability_immediately_lookupable` — pass |
| 11 | `SIGHUP` reload over a corrupted file keeps the previous good registry | `a_reload_over_a_corrupted_file_leaves_the_previously_good_registry_serving` — pass |
| 12 | Existing `route`/`execute`/`admit_execution_request` tests unchanged | Full `cargo test` run, see below |

An additional case beyond the 12 (`a_kid_not_present`'s sibling,
`a_token_signed_by_a_different_key_than_the_kid_publishes_is_denied`, and a startup-failure case,
`loading_an_invalid_registry_file_fails_loudly_at_startup`) were also written and pass — real
coverage beyond the floor the packet required, not a shortfall.

## Verification commands run against `f53e34f`

- `cargo build`: clean, no warnings.
- `cargo test --test serve_http_server --test serve_service --test serve_token_verify`: 16/16 pass.
- `cargo test` (full suite): every test passes **except** one pre-existing, unrelated flake —
  `cortex_gnat_shard_dispatch::dispatch_maps_a_complete_receipt_to_completed`, which fails
  intermittently under `cargo test`'s default parallel runner and passes reliably with
  `--test-threads=1`. Filed as `KI-FLO-20260924-004`; not caused by and does not block this closeout
  (no shared state with `serve`, confirmed by reading both modules — Cortex Gnat dispatch and this
  daemon share no code path).
- `cargo fmt --all -- --check`: clean on every file this plan's PRs touched.
- `bash ci_gate.sh`: `FA Local CI gate: PASSED`, including the `forge_contract_core` sibling gate.

## Accepted limitations

- No durable, centrally-persisted registry (OD-1's own ruling — never in scope for this slice).
- No key-rotation mechanism for `FA_LOCAL_SERVE_PUBLIC_KEYS` beyond a process restart.
- Key-format support is narrower than `dataforge-Local`'s Python verifier (PEM SPKI or raw
  base64url-no-pad only) — documented in `token_verify.rs`, a deliberate scope decision, not a gap.
- `KI-FLO-20260924-003` (stale `doc/system` §3/§9 and one doc-comment `--watch` reference) remains
  open, tracked separately, explicitly non-blocking per that entry's own scope note.
- Forge_Command's own consuming side (replacing DR-043's hardcoded `capabilityId` sentinel in
  `strategy.ts` with a real call to this route) is **not** part of this plan — it is
  `Forge_Command`'s own future implementation PR, under `BDS-FC-LIVING-TOPOLOGY-ASSURANCE-002`'s own
  authority. This closeout does not claim CP4's `capabilityId` gap is closed end to end, only that
  FA Local's own half of it now exists, is tested, and is live (default-off) on `master`.

## Current gate

- [x] Implementation matches `02_...md`'s frozen file allowlist exactly (verified by diff review at
      merge time and re-confirmed against current `master`).
- [x] All 12 packet test-allowlist cases independently re-verified against current `master`, not
      assumed from the merge-time report.
- [x] The port correction is accounted for, not silently absorbed — `PORT_REGISTRY.md`'s current
      state (8012, not the originally claimed 8011) is what this closeout evidences.
- [x] One unrelated flaky test found during verification is filed (`KI-FLO-20260924-004`), not
      ignored, and explicitly assessed as non-blocking with reasoning, not by default.
- [x] `ci_gate.sh` (this repo's actual designated gate) passes on current `master`.
