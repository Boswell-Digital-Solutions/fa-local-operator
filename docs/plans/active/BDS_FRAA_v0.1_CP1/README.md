# BDS-FRAA-v0.1 CP1/WP01 — FA Local Sealed-Corpus Reuse Reconnaissance

**Status:** implementation candidate on a bounded branch; not merged, promoted, or released.  
**Owner:** `Boswell-Digital-Solutions/fa-local-operator`  
**Baseline:** `690434006b2fc726d31eea0d57a1fd4c7b8d1363`  
**Authority:** `AUTHORIZATION_RECORD.txt`

## Purpose

Prove that FA Local can admit and execute one explicit read-only capability over a frozen candidate-visible Tarcie CP0 corpus, emit a deterministic reuse/topology result, and permit an independent test evaluator to compare that frozen result with a physically separate hidden oracle.

## Delivered candidate surfaces

- explicit `local_file_read` capability type with `side_effect_class=none`;
- root-confined, hash-verifying, non-symlink sealed-corpus reader;
- repo-local `FraaReconnaissanceInput.v0` and `FraaReconnaissanceResult.v0` contracts;
- deterministic donor classification and topology selection;
- result self-hash;
- six-step bounded execution-plan verification through existing FA Local machinery;
- blind oracle comparison in test code only.

## Authority boundary

This slice has no live repository, network, model, DataForge, memory, Contract Core, Smithy, promotion, deployment, merge, or release authority.
