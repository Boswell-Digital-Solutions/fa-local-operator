# 02 — Implementation Scoping Packet

**Plan ID:** `BDS-FAL-DAEMON-v0.1`
**Status:** **Fully authorized, 2026-09-23.** Freezes the concrete design
`01_CURRENT_STATE_DOCTRINE_AND_PROPOSAL.md` ratified (OD-1 through OD-4), and its own three open
items (dependencies, port, `CLAUDE.md` edit safety) are now all resolved. This authorizes writing
exactly the files in "File allowlist" below, in `fa-local-operator` only.
**Decision scope:** Design/scoping only, same discipline as `Forge_Command`'s
`14_CP4_S003_IMPLEMENTATION_PACKET.md`. This document's own acceptance is what authorizes writing
exactly the files listed in "File allowlist" below — nothing broader.
**Primary rule:** Unchanged from `01_...md` — read-only, default-off, no execution-over-HTTP route,
no write-capable route, no change to `admit_execution_request`'s pure semantics.

## New dependencies (explicit decision, not covered by OD-1–OD-4)

| Crate | Purpose | Why this one |
|---|---|---|
| `tiny_http` | Minimal blocking HTTP server | This crate has no async runtime anywhere (`Cargo.toml` has no `tokio`/`async-std`; every existing HTTP call is synchronous `ureq`). `tiny_http` is a single-purpose, blocking, dependency-light server that matches the crate's existing synchronous style rather than pulling in an async framework (`axum`/`actix-web`) and its transitive runtime for one read-only route. |
| `jsonwebtoken` (`= "9"`, matching `Forge_Command/src-tauri/Cargo.toml:133`) + `ed25519-dalek` (`= "2"`, matching `Forge_Command/src-tauri/Cargo.toml:130`) | Verify the Ed25519 JWS tokens Forge_Command mints | Reuses the exact crate versions Forge_Command's own token authority already uses (`src-tauri/src/token_authority.rs`), so the two sides are verified against a proven-compatible implementation pair rather than two different JWT libraries that happen to both claim RFC 7519 support. |

**Open item — operator sign-off needed on this table specifically** before `02_...md` is
otherwise treated as fully authorized. `01_...md`'s ratification covered architecture shape, not a
supply-chain addition.

## What this freezes

### Route contract

One route, one method, no others:

```
GET /api/v1/capabilities/{capability_id}
Authorization: Bearer <JWS>
```

`{capability_id}` is the same UUID shape `schemas/capability-registry.schema.json`'s
`uuid_string` `$def` already requires (`^[0-9a-fA-F]{8}-...`) — no new ID format.

**Success (200):** the matching `CapabilityRecord` (`src/domain/capabilities/mod.rs:52-69`),
JSON-serialized exactly as `CapabilityRegistryLoader`/`serde` already render it elsewhere in this
crate (it already derives `Serialize`) — no new response type, no field renaming, no envelope
wrapper invented for this packet.

**Not found (404):** `capability_id` is well-formed but absent from the loaded registry. Body:
`{"error": "capability_not_found"}`.

**Malformed ID (400):** `{capability_id}` does not match `uuid_string`. Body:
`{"error": "malformed_capability_id"}`.

**Unauthorized (401):** missing `Authorization` header, malformed JWS, bad signature, wrong `kid`,
expired, or wrong `scope` (must be exactly `capability:read`). Body:
`{"error": "unauthorized"}` — never a more specific reason (mirrors
`dataforge-Local`'s own `run_token.py` `TokenVerdict.deny`, which returns a reason to the caller's
logs but a flat denial to the wire).

**Unavailable (503):** the daemon is up but the registry failed to load or re-load (parse error,
schema-validation failure, file missing on a `--watch` refresh). Never silently serves a stale or
partially-loaded registry. Body: `{"error": "registry_unavailable"}`.

No other route exists. No `GET /api/v1/capabilities` (list) — a targeted lookup is all CP4 needs;
a list route is a scope increase this packet does not propose.

### Startup and configuration

```
fa-local-run serve \
  --registry-file <path to a capability-registry.schema.json-valid file> \
  --port 8011 \
  --public-keys-env FA_LOCAL_SERVE_PUBLIC_KEYS
```

- `FA_LOCAL_SERVE_ENABLED` (env var): the route does not exist — the daemon refuses to start,
  matching `NEUROFORGE_AAR_INTAKE_ENABLED`'s dark-launch shape (`forge` PR #196) — unless set.
- `FA_LOCAL_SERVE_PUBLIC_KEYS` (env var): a JSON object mapping `kid -> public key`, mirroring
  `dataforge-Local`'s own `XA_RUN_TOKEN_PUBLIC_KEYS` mechanism exactly
  (`execution_authority/services/run_token.py:62,121-139`) — same shape, same "empty key set is not
  an open door: with no key to verify against, every request is denied" rule
  (`run_token.py:110-113`), reimplemented in Rust rather than invented fresh.
- Registry file is loaded and schema-validated (`CapabilityRegistryLoader::load_contract_value`,
  already exists, `src/domain/capabilities/mod.rs:88-90`) once at startup. A `SIGHUP` triggers a
  re-load; a failed re-load leaves the previous good registry in place and logs the failure rather
  than serving a partial one (fail-closed only on requests that need registry data the daemon
  cannot currently answer for, not by dropping already-good state).

### Auth verification (Rust side)

New `adapters::serve::token_verify` module: given the `Authorization` header value and the loaded
public-key map, decode and verify the JWS (`jsonwebtoken::decode` with `Algorithm::EdDSA`), check
`scope == "capability:read"`, check `exp`, check `kid` is in the configured key map. A pure
function taking the header string and the key map, returning `Result<(), AuthDenyReason>` — no I/O
beyond what the caller already did to obtain the key map at startup.

## File allowlist

New:

- `src/adapters/serve/mod.rs`, `src/adapters/serve/http_server.rs` — the `tiny_http`-backed
  listener: parses the one route, calls into `app::serve_service`, writes the response. No business
  logic here beyond routing and (de)serialization — matches this crate's existing `adapters/`
  convention of "storage, schema, clock, hashing, and export boundaries" (`CLAUDE.md`).
- `src/adapters/serve/token_verify.rs` — the auth verification function above.
- `src/app/serve_service.rs` — orchestration: holds the loaded `CapabilityRegistry` (behind a
  `RwLock` or equivalent for the `SIGHUP` re-load path), exposes a `lookup(capability_id) ->
  ServeLookupResult` function the HTTP adapter calls. Mirrors this crate's existing `app/` layer
  convention (`decision_service.rs`, `execution_pipeline_service.rs`, etc.) of "orchestration
  services that compose domain logic without absorbing policy authority."
- `tests/serve_service.rs`, `tests/serve_http_server.rs`, `tests/serve_token_verify.rs` — see "Test
  allowlist" below.
- `schemas/examples/` — no new schema; `serve`'s response is an existing `CapabilityRecord`, so no
  new fixture family is needed beyond what `capability-registry.schema.json`'s existing examples
  already cover.

Modified:

- `Cargo.toml` — the two new dependencies above.
- `src/bin/fa_local_run.rs` — one new `Some("serve") => { ... }` arm in the existing
  `match args.get(1).map(String::as_str)` dispatch (`fa_local_run.rs:85`), following the exact
  hand-rolled flag-parsing style every other subcommand there already uses (`flag_path` closure
  pattern, `fa_local_run.rs:140-143`) — no `clap` or other arg-parsing crate introduced.
- `src/lib.rs` (or `src/adapters/mod.rs` / `src/app/mod.rs`) — module declarations for the new
  files, matching the existing one-line-per-module pattern.
- `CLAUDE.md` — the line stating "FLO is a Rust CLI with no HTTP surface" needs to become accurate
  once this ships: a daemon *mode* exists, default-off, one read-only route. **Confirmed hand-written,
  not generated** — no `repo.manifest.yaml` exists in this repo and `CLAUDE.md` carries no
  generated-file marker (unlike `Forge_Command`'s manifest-driven `CLAUDE.md`/`AGENTS.md`/
  `CODEX.md`/`GEMINI.md`, which this repo does not use — it has a separate, lowercase `agents.md`
  instead). Direct edit is safe.

Not touched: `admit_execution_request` or any other function in `src/domain/capabilities/mod.rs`;
`route`/`execute`'s existing file-argument path (unchanged); any `forge-df-local-foundation` file;
any `Forge_Command` file (that repo's own future PR wires its consuming side, under its own plan).

## Test allowlist and negative cases

1. A well-formed `capability_id` present in the loaded registry returns 200 with the exact
   `CapabilityRecord` JSON.
2. A well-formed `capability_id` absent from the registry returns 404
   `capability_not_found`.
3. A malformed `{capability_id}` (not `uuid_string`-shaped) returns 400
   `malformed_capability_id` before any registry lookup is attempted.
4. Missing `Authorization` header returns 401 `unauthorized`.
5. A JWS with a `kid` not present in `FA_LOCAL_SERVE_PUBLIC_KEYS` returns 401 — proves an unknown
   key is rejected, not silently accepted.
6. A JWS with the wrong `scope` (anything other than exactly `capability:read`) returns 401 — proves
   scope is checked, not just signature validity.
7. A JWS past its `exp` returns 401.
8. An empty/unset `FA_LOCAL_SERVE_PUBLIC_KEYS` denies every request — proves the
   "no key configured means fail closed" rule, mirroring `run_token.py`'s own equivalent test.
9. `FA_LOCAL_SERVE_ENABLED` unset: the daemon does not start the listener at all — proves the
   dark-launch gate, not just a 404 on the route.
10. A `SIGHUP` after editing the registry file to add a new capability makes that capability
    immediately lookupable — proves the re-load path works.
11. A `SIGHUP` after corrupting the registry file leaves the previously-good registry serving
    correctly (logs the failure, does not 503 requests the old registry could still answer) — proves
    fail-closed applies to the reload, not to already-good state.
12. `admit_execution_request` and every existing `route`/`execute` test continue passing unmodified
    — proves this packet touched no shared domain logic.

## Open items — all resolved 2026-09-23

1. **The two new dependencies (table above) — RESOLVED: authorized.** Operator ruling, 2026-09-23:
   "Authorized." `tiny_http` and `jsonwebtoken`/`ed25519-dalek` (matching
   `Forge_Command/src-tauri/Cargo.toml`'s exact versions) may be added to `Cargo.toml`.
2. **Default port — RESOLVED: `8011`.** Claimed in the canonical `PORT_REGISTRY.md` (Agent Layer,
   `forge` root repo, 2026-09-23) ahead of any code, per that file's own Rule 1 ("claim before
   coding"). `--port` defaults to `8011`; overridable at the operator's discretion.
3. **`CLAUDE.md` generation — RESOLVED: confirmed hand-written, direct edit is safe.** No
   `repo.manifest.yaml` exists in this repo and `CLAUDE.md` carries no generated-file marker.
   Operator confirmed.

**This packet is now fully authorized.** The exact authorization text below is accepted as written.

## Proposed exact human authorization (for when the operator is ready to rule)

> Authorized: `BDS-FAL-DAEMON-v0.1`'s implementation scoping packet (`02_...md`) is accepted,
> including the `tiny_http` and `jsonwebtoken`/`ed25519-dalek` dependency additions. Default port:
> `8011` (claimed in `PORT_REGISTRY.md`). This authorizes writing exactly the files in "File
> allowlist" above, and no others, in `Boswell-Digital-Solutions/fa-local-operator` only. It does
> not authorize any change in `Forge_Command`, `forge-df-local-foundation`, or any other repository.

Any materially different scope requires a delta review and renewed authorization, consistent with
every other authorization packet in this ecosystem.

## Gate for this document

- [x] Every design claim cites the existing code pattern it mirrors (subcommand dispatch style,
      `adapters`/`app` layer split, `dataforge-Local`'s own public-key-env mechanism) rather than
      inventing a new convention for this one slice.
- [x] New dependencies are called out as their own explicit, unbundled decision rather than assumed
      covered by `01_...md`'s scope ratification.
- [x] File allowlist is exhaustive; nothing outside it is implied to change.
- [x] Test allowlist includes the fail-closed cases (no key configured, malformed scope, expired
      token, corrupted reload) at the same density this ecosystem's other authorization packets use.
- [x] Port resolved (`8011`, claimed in `PORT_REGISTRY.md`).
- [x] `CLAUDE.md` generation status resolved (hand-written, direct edit confirmed safe).
- [x] Operator authorized the two new dependencies (`tiny_http`, `jsonwebtoken`/`ed25519-dalek`),
      2026-09-23.
- [x] This document's proposed authorization text is accepted as written, 2026-09-23.
