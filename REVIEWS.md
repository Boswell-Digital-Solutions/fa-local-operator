# Review Handover

FA Local may hand work back to a human when direct execution is not admissible.

`ReviewService` (`src/app/review_service.rs`) and the schema-backed `review-package` contract (`src/domain/review/mod.rs`) implement bounded explicit-approval and review-required handoff: a review package can never fabricate execution success, must preserve the distinction between approval posture and execution state, and must carry an explicit decline option. `ForensicRecordKind::ReviewPackagePrepared` links a prepared review package back into the forensic record for that route decision. This remains intentionally bounded to the `review_required` and `explicit_operator_approval` postures only — it does not introduce generic workflow behavior.
