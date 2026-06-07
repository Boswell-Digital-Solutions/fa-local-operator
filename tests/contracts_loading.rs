mod support;

use fa_local::DenialGuard;
use fa_local::deserialize_contract_value;
use fa_local::domain::capabilities::CapabilityRegistryLoader;
use fa_local::domain::execution::{ExecutionPlan, ExecutionRequest};
use fa_local::domain::forensics::ForensicEvent;
use fa_local::domain::friction::FrictionPayload;
use fa_local::domain::policy::PolicyArtifactLoader;
use fa_local::domain::requester_trust::RequesterTrustEngine;
use fa_local::domain::review::ReviewPackage;
use fa_local::domain::routing::RouteDecisionLoader;
use fa_local::domain::status::ExecutionStatus;
use fa_local::integrations::cortex::GnatDispatchEnvelope;
use fa_local::{EnvironmentMode, RequesterClass, SchemaName};

#[test]
fn valid_requester_trust_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "requester-trust-basic.json");
    let envelope = RequesterTrustEngine::load_contract_value(&value).unwrap();

    assert_eq!(envelope.requester_class, RequesterClass::TrustedAppSurface);
    assert_eq!(envelope.environment_mode, EnvironmentMode::Prod);
    assert_eq!(envelope.app_context.app_id, "forge-author");
}

#[test]
fn valid_policy_artifact_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let policy = PolicyArtifactLoader::load_contract_value(&value).unwrap();

    assert_eq!(policy.scope.service_id, "fa-local");
    assert_eq!(policy.capability_rules.len(), 1);
    assert_eq!(policy.side_effect_rules.len(), 1);
}

#[test]
fn valid_capability_registry_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "capability-registry-basic.json");
    let registry = CapabilityRegistryLoader::load_contract_value(&value).unwrap();

    assert_eq!(registry.capabilities.len(), 1);
    assert_eq!(registry.capabilities[0].owner_service, "fa-local");
}

#[test]
fn valid_execution_request_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "execution-request-basic.json");
    let request = ExecutionRequest::load_contract_value(&value).unwrap();

    assert_eq!(request.environment_mode, EnvironmentMode::Prod);
    assert_eq!(request.intent_summary, "write approved local export");
}

#[test]
fn valid_execution_plan_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "execution-plan-basic.json");
    let plan = ExecutionPlan::load_contract_value(&value).unwrap();

    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.declared_max_step_count, 4);
    assert_eq!(plan.max_duration_budget_ms, 2000);
}

#[test]
fn valid_denial_guard_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "denial-guard-basic.json");
    let guard: DenialGuard = deserialize_contract_value(SchemaName::DenialGuard, &value).unwrap();

    assert!(!guard.remediable);
    assert_eq!(guard.summary, "unknown requester denied");
}

#[test]
fn valid_route_decision_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "route-decision-policy-preapproved-basic.json");
    let decision = RouteDecisionLoader::load_contract_value(&value).unwrap();

    assert!(decision.execution_allowed);
    assert_eq!(decision.denial_guards.len(), 0);
    assert_eq!(
        decision.operator_visible_summary,
        "request is policy preapproved for capability 44444444-4444-4444-8444-444444444444"
    );
}

#[test]
fn valid_execution_status_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "execution-status-in-progress-basic.json");
    let status = ExecutionStatus::load_contract_value(&value).unwrap();

    assert_eq!(status.state, fa_local::ExecutionState::InProgress);
    assert_eq!(
        status.current_posture,
        fa_local::ApprovalPosture::PolicyPreapproved
    );
    assert_eq!(status.current_step.as_deref(), Some("step_export_prepare"));
}

#[test]
fn valid_review_package_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "review-package-basic.json");
    let package = ReviewPackage::load_contract_value(&value).unwrap();

    assert_eq!(
        package.current_posture,
        fa_local::ApprovalPosture::ExplicitOperatorApproval
    );
    assert_eq!(
        package
            .execution_status_context
            .as_ref()
            .map(|context| context.state),
        Some(fa_local::ExecutionState::WaitingExplicitApproval)
    );
    assert!(
        package
            .approval_options_allowed_by_policy
            .contains(&fa_local::domain::review::ApprovalOption::ApproveExecute)
    );
}

#[test]
fn valid_review_required_review_package_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "review-package-review-required-basic.json");
    let package = ReviewPackage::load_contract_value(&value).unwrap();

    assert_eq!(
        package.current_posture,
        fa_local::ApprovalPosture::ReviewRequired
    );
    assert!(package.execution_plan_id.is_none());
    assert!(package.stable_plan_hash.is_none());
    assert_eq!(
        package
            .execution_status_context
            .as_ref()
            .map(|context| context.state),
        Some(fa_local::ExecutionState::ReviewRequired)
    );
}

#[test]
fn valid_forensic_event_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "forensic-event-execution-status-basic.json");
    let event = ForensicEvent::load_contract_value(&value).unwrap();

    assert_eq!(
        event.event_type,
        fa_local::domain::forensics::ForensicEventType::ExecutionStatusObserved
    );
    assert_eq!(
        event.current_posture,
        fa_local::ApprovalPosture::PolicyPreapproved
    );
    assert_eq!(
        event.execution_state,
        fa_local::ExecutionState::CompletedWithConstraints
    );
    assert_eq!(
        event.degraded_subtype,
        Some(fa_local::DegradedSubtype::DegradedFallbackLimited)
    );
}

#[test]
fn valid_friction_payload_fixture_loads_into_typed_model() {
    let value =
        support::load_fixture_json("valid", "friction-payload-explicit-approval-basic.json");
    let payload = FrictionPayload::load_contract_value(&value).unwrap();

    assert_eq!(
        payload.friction_kind,
        fa_local::domain::friction::FrictionKind::ExplicitApprovalRequired
    );
    assert_eq!(
        payload.operator_action,
        fa_local::domain::friction::OperatorAction::ApproveOrDecline
    );
    assert_eq!(
        payload.current_posture,
        fa_local::ApprovalPosture::ExplicitOperatorApproval
    );
    assert_eq!(
        payload.execution_state,
        fa_local::ExecutionState::WaitingExplicitApproval
    );
}

#[test]
fn valid_gnat_dispatch_envelope_fixture_loads_into_typed_model() {
    let value = support::load_fixture_json("valid", "gnat-dispatch-envelope-basic.json");
    let envelope = GnatDispatchEnvelope::load_contract_value(&value).unwrap();

    assert_eq!(envelope.requester_service, "cortex");
    assert_eq!(envelope.plan.shard_count, 2);
    assert_eq!(envelope.required_capabilities.worker_types.len(), 2);
}
