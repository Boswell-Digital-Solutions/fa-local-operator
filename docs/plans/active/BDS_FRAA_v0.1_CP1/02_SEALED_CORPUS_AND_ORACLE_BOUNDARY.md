# Sealed Corpus and Hidden Oracle Boundary

## Candidate-visible corpus

- Root: `tests/fixtures/fraa/tarcie_cp0/candidate-visible/`
- Manifest profile: `FraaSealedCorpusManifest.v0`
- Aggregate profile: `path-sha256-size-v1`
- Bound aggregate SHA-256: `8f71ab007f845dd7c7c9734aacbceb802408ac20dee47a82e7ce59610d797d38`

The adapter rejects absolute paths, parent traversal, Windows-style separators, colon-bearing paths, symlinks, non-files, root escapes, duplicate members, oversized members, digest mismatches, and any member path containing `oracle` or `hidden_oracle`.

## Hidden oracle

- Path: `tests/fixtures/fraa/tarcie_cp0/oracle/expected-result.json`
- The production service and deterministic engine receive neither this path nor the corpus root.
- Only the independent integration test reads the oracle, after the candidate result has been frozen and self-hashed.

This is adequate for the CP1 sealed-fixture proof. A later model or external-agent qualification must add process-level isolation; filename or path filtering alone is not represented as production-grade answer-key isolation.
