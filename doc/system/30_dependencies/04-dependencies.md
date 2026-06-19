# §4 — Dependencies

**Truth class:** snapshot (audit-derived)

fa-local-operator is a bounded worker in the local Cortex "Gnats" swarm; its
dependencies are the swarm peers and the contracts it conforms to. Re-measure
against the build manifest when this changes.

## Swarm Peers

| Peer | Role | Relationship |
|------|------|--------------|
| Cortex (`COR`) | Planning / extraction | Upstream — produces the plan fa-local-operator routes |
| NeuronForge-Local | Semantics / candidate generation | Peer — supplies semantics/candidates |
| DataForge-Local (`DLO`) | Local durable persistence | Downstream — persists operational truth |
| ForgeCommand (`FCO`) | Operator / control plane | Orchestrates; consumes results |

## Contract Dependencies

fa-local-operator honors its **contract surface** (§2) exactly and bridges results
back via the **execution bridge** (§3); it consumes contracts, it does not redefine
them.

## Runtime Dependencies

Versions and crate/package pins are catalogued in the build manifest; this chapter
records the *relationships* (canonical), while exact versions are snapshot facts
re-measured at build time.
