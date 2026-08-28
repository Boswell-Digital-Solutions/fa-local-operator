# CP1 Verification Plan

## Required commands

```bash
cargo fmt --check
cargo test --test reuse_reconnaissance
cargo test
bash ci_gate.sh
```

`bash ci_gate.sh` additionally requires a compatible local `forge_contract_core` checkout. No workflow mutation is part of this slice.

## Zero-tolerance assertions

- hidden oracle members exposed through the candidate manifest: `0`
- path traversal accepted: `0`
- symlink escape accepted: `0`
- digest mismatch accepted: `0`
- live repository calls: `0`
- network calls: `0`
- model calls: `0`
- source or repository mutations performed by the runtime: `0`
- false donor admissions in the frozen oracle comparison: `0`
- authority-boundary violations: `0`
- deterministic replay drift: `0`

## Current evidence posture

The branch supplies executable tests and contract fixtures. Test success must be read from repository CI or an independently reproduced clean checkout; the plan documents do not pre-claim a pass.
