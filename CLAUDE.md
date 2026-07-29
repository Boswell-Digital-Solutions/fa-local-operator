# fa-local-operator (FLO) — Claude Code Context

Business/internal local ForgeAgents worker: capability admission, bounded-plan validation,
dispatch, adapter execution, review handoff, status evidence.

> **Not `forge-fa-local`.** Same brand, two families. This is the business-side system at
> `ecosystem/local-systems/fa-local-operator`; `apps/public-app-local-support/forge-fa-local` is
> the public-app support variant. Path decides which one owns the behaviour.

Canonical reference: `doc/FLOSYSTEM.md`, assembled from `doc/system/` via `bash doc/system/BUILD.sh`.
Contracts: [`schemas/`](schemas/) — `capability-registry`, `execution-request`, `execution-plan`,
`execution-status`, `denial-guard`, `friction-payload`, `forensic-event`, with worked cases in
`schemas/examples/`.

---

## Boundaries

FLO is the validating dispatcher of the local plane: **it validates plans and dispatches; it does
not extract or prepare** (that is COR), and it is not a general-purpose executor.

- **Deny by default.** Capability admission is a gate, not a lookup — `denial-guard.schema.json`
  defines the refusal shape, and a refusal is a first-class outcome, not an error path.
- Plans are **bounded**. A plan that cannot be validated is not run.
- Forensic events and status evidence are append-only.
- Do not invent undocumented APIs, tables, routes, or environment variables.

---

## Verification

```bash
bash ci_gate.sh      # execution bridge v1 contract participation
```

There is no GitHub workflow — `ci_gate.sh` is the gate. `cargo test` covers the Rust suites.

---

## Non-obvious

- **`ci_gate.sh` resolves forge-contract-core by relative path** (`../../contracts/forge_contract_core`,
  falling back to `forge-contract-core`) and preferentially uses that repo's `.venv/bin/python`.
  A moved or renamed sibling checkout makes the gate fail loudly at the path check — and a missing
  `.venv` silently downgrades it to bare `python3`. Confirm which interpreter it printed.
- **FLO is a Rust CLI with no HTTP surface**, so it cannot be supervised as a local HTTP service.
  Anything that needs to reach it over a port needs a different host process.

```bash
./scripts/context-bundle.sh --list
```
