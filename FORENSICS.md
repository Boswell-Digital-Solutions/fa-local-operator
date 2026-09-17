# Forensic Posture

FA Local forensics are intended to be minimal, locally auditable, and content-sparing.

FA Local exports forensic events through an append-only local JSONL sink (`src/adapters/exports/jsonl_forensic_export.rs`), one compact record per line. SQLite-backed queryable storage remains planned follow-up work.
