# Forensic Posture

FA Local forensics are intended to be minimal, locally auditable, and content-sparing.

FA Local exports forensic events through an append-only local JSONL sink (`src/adapters/exports/jsonl_forensic_export.rs`), one compact record per line, or a queryable local SQLite store (`src/adapters/exports/sqlite_forensic_store.rs`), indexed by `correlation_id` and `event_type` and queried back out via `fa-local-run forensics-query`.
