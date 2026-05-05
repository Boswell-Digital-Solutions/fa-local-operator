# FA Local - System Documentation

**Document version:** 0.15 (2026-03-19) - Contract, posture, plan, execution-status, review-package, forensic-event, friction-payload, internal coordinator, internal routing, bounded adapter-delivery, first concrete adapter, review-emitter, and forensic recorder/export slices aligned to current repo state
**Protocol:** Forge Documentation Protocol v1

| Key | Value |
|-----|-------|
| **Project** | FA Local |
| **Prefix** | `fa` |
| **Output** | `doc/faSYSTEM.md` |

This `doc/system/` tree is the assembled system reference for FA Local as a bounded local execution-control service.
It reflects the current repository state after the standalone crate scaffold, the schema-backed Phase 0.5 contract slice, the opening of Phase 1 requester/policy/capability deny logic, the pure route-decision and approval-posture slice, the bounded execution-plan and stable-hash slice, the truthful execution-status contract slice, the bounded review-package contract slice, the full bounded review-package emitter workflow slice for both review postures, the minimal forensic-event contract slice, the bounded forensic recorder/export workflow slice, the bounded friction-payload contract slice, the internal bounded execution-coordinator slice, the internal deterministic execution-routing slice, the explicit adapter-backed delivery boundary, the first concrete local-file-write adapter slice, and the current fail-closed test coverage.

Assembly contract:

- Command: `bash doc/system/BUILD.sh`
- Output: `doc/faSYSTEM.md`

| Part | File | Contents |
|------|------|----------|
| SS1 | [01-overview-charter.md](01-overview-charter.md) | Mission, role, success posture, and current bounded baseline |
| SS2 | [02-boundaries-and-doctrine.md](02-boundaries-and-doctrine.md) | Authority boundaries, policy-before-execution doctrine, and anti-drift posture |
| SS3 | [03-contract-surface.md](03-contract-surface.md) | Implemented contract inventory, typed validation surfaces, and current execution-control vocabulary |
| SS4 | [04-validation-and-delivery.md](04-validation-and-delivery.md) | Build/test wiring, delivered contract slice, and current delivery posture |
| SS5 | [05-execution-bridge-writeback.md](05-execution-bridge-writeback.md) | Execution bridge writeback path design: FA Local → DataForge Local → DataForge Cloud → Forge_Command |

## Quick Assembly

```bash
bash doc/system/BUILD.sh
```

*Last updated: 2026-03-19*
