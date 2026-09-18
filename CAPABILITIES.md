# Capability Admission

Capability admission is separate from request validity.

`CapabilityRegistryLoader` (`src/domain/capabilities/mod.rs`) loads the schema-backed registry and implements `admit_execution_request`, which checks registration, `enabled_state`, `revocation_state`, owner service, requester-class allow-list, side-effect-class match, and policy coupling — failing closed on any mismatch. `ExecutionPlanValidator` (`src/domain/execution/mod.rs`) separately admits each declared execution-plan step's own capability against the same registry, so a plan's steps may reference different capabilities. `AdapterRegistry` (`src/adapters/execution_delivery/registry.rs`) then resolves which concrete adapter serves an admitted capability at dispatch time, one adapter per capability, failing closed on duplicate registration or a missing adapter.
