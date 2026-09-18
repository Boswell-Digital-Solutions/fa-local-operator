# Policy Posture

Policy is a first-class execution gate.

`PolicyArtifactLoader` (`src/domain/policy/mod.rs`) loads schema-backed policy artifacts and exposes capability-rule lookup. `CapabilityRegistryLoader::admit_execution_request` (`src/domain/capabilities/mod.rs`) couples policy, capability, and requester-trust admission, failing closed on any mismatch or missing requirement. `ApprovalPostureResolver` (`src/domain/posture/mod.rs`) resolves the final approval posture from policy, capability, requester-trust, review-class, and side-effect factors together. `DecisionService::resolve_route_decision` (`src/app/decision_service.rs`) wires all of this into one callable entry point from raw request/policy/registry/trust JSON, reachable directly or via `fa-local-run route`/`execute`.
