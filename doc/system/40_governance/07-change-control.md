# §7 — Change Control

**Truth class:** canonical doctrine

This chapter defines how changes to fa-local-operator are classified, evidenced,
verified, and rolled back. Every change class names the evidence and verification
commands that must accompany it. fa-local-operator is an internal Forge ecosystem
service; nothing here authorizes public-release or production-certification claims.

## Change Classes

| Class | Scope | Example |
|-------|-------|---------|
| C0 | Documentation only | Editing `doc/system/` chapters, rebuilding `doc/FLOSYSTEM.md` |
| C1 | Execution logic | The FA-Local capability-admission / dispatch / execution lane (§5/§8) |
| C2 | Contract surface | Honoring/changing the contract surface (§2) |
| C3 | Execution bridge | Writeback bridge behavior (§3) |
| C4 | Dependencies | Local-plane-peer or contract pin changes (§4) |
| C5 | Validation & delivery | Proof/validation gates (§9) |
| C6 | Configuration / security | Env contract, fail-closed posture |

## Required Evidence Per Change Class

- **C0** — rebuilt artifact (`bash doc/system/BUILD.sh` → `BUILD_OK`), edited
  source chapter (never a hand-edit to `doc/FLOSYSTEM.md`).
- **C1** — tests proving admission/dispatch/execution stays bounded/deterministic
  and in-lane (does not absorb workflow planning, workcell routing, model
  intelligence, or persistence).
- **C2** — proof the contract surface is honored exactly (consumed, not redefined).
- **C3** — tests for the changed writeback bridge path.
- **C4** — the local-plane-peer/contract change reflected in §4.
- **C5** — the validation/delivery evidence in §9.
- **C6** — env/setting change with secrets never hard-coded; fail-closed preserved.

## Required Verification Commands

```bash
# run the repo's test suite (cargo test / pytest, per the build manifest)
bash doc/system/BUILD.sh                # doc changes (C0) -> BUILD_OK designation=FLO
```

## Bounded-Worker / Boundary Rules

fa-local-operator stays a bounded, deterministic execution worker (§5/§8): a change
must not expand it into workflow planning or executor selection (Cortex
prepares/extracts), workcell admission/routing (Yellowjacket), model intelligence
(NeuronForge-Local), persistence (DataForge-Local), or decision authority
(ForgeCommand). It fails closed on
ambiguity rather than guessing.

## Documentation Change Rules (C0)

`doc/system/` source modules are the only editing surface. The compiled
`doc/FLOSYSTEM.md` is regenerated, never hand-edited (§6). Snapshot facts are
re-measured and re-dated, not asserted as guarantees.

## Release / Readiness Claim Rules

No change may introduce public-release, public-SaaS, or production-certification
language, or present a coverage percentage as a guarantee, unless a governed
release slice proves that specific claim.
