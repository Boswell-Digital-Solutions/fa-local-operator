# §4 — Dependencies

**Truth class:** snapshot (audit-derived)

fa-local-operator is a bounded execution worker in the federated local plane; its
dependencies are the local-plane peers and the contracts it conforms to. Re-measure
against the build manifest when this changes.

## Local-plane peers

| Peer | Role | Relationship |
|------|------|--------------|
| Yellowjacket | Workcell admission & lane routing | Upstream — resolves/pins the approved skill/workcell and routes the lane |
| Cortex (`COR`) | Preparation / extraction | Upstream — file intelligence + retrieval-preparation packages fa-local-operator consumes |
| NeuronForge-Local | Model intelligence | Peer — supplies inference/embeddings/LoRAs |
| DataForge-Local (`DLO`) | Local durable persistence | Downstream — persists operational truth |
| ForgeCommand (`FCO`) | Operator / control plane | Governs; consumes results |

## Contract Dependencies

fa-local-operator honors its **contract surface** (§2) exactly and bridges results
back via the **execution bridge** (§3); it consumes contracts, it does not redefine
them.

## Runtime Dependencies

Versions and crate/package pins are catalogued in the build manifest; this chapter
records the *relationships* (canonical), while exact versions are snapshot facts
re-measured at build time.
