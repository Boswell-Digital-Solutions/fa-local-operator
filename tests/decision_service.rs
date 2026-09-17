mod support;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use fa_local::app::decision_service::DecisionService;
use fa_local::domain::posture::RouteResolutionContext;
use fa_local::domain::routing::RouteDecisionLoader;
use fa_local::{ApprovalPosture, RouteDecisionId};

fn decision_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 5, 0).unwrap()
}

fn decision_context(value: &str) -> RouteResolutionContext {
    RouteResolutionContext::new(
        RouteDecisionId::from_uuid(Uuid::parse_str(value).unwrap()),
        decision_time(),
    )
}

#[test]
fn resolves_the_same_golden_route_decision_as_the_domain_layer_from_raw_json_inputs() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");

    let route_decision = DecisionService
        .resolve_route_decision(
            &request,
            &requester_trust,
            &policy,
            &capability_registry,
            decision_context("77777777-7777-4777-8777-777777777774"),
        )
        .unwrap();

    let expected = RouteDecisionLoader::load_contract_value(&support::load_fixture_json(
        "valid",
        "route-decision-policy-preapproved-basic.json",
    ))
    .unwrap();

    assert_eq!(route_decision, expected);
    assert_eq!(
        route_decision.resolved_approval_posture,
        ApprovalPosture::PolicyPreapproved
    );
    assert!(route_decision.execution_allowed);
}

#[test]
fn malformed_requester_trust_envelope_folds_into_a_denied_route_decision_not_a_hard_error() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let mut requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    requester_trust["trust_basis"] = serde_json::Value::Null;
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");

    let route_decision = DecisionService
        .resolve_route_decision(
            &request,
            &requester_trust,
            &policy,
            &capability_registry,
            decision_context("77777777-7777-4777-8777-777777777776"),
        )
        .unwrap();

    assert_eq!(
        route_decision.resolved_approval_posture,
        ApprovalPosture::Denied
    );
    assert!(!route_decision.execution_allowed);
    assert!(!route_decision.denial_guards.is_empty());
}

#[test]
fn malformed_execution_request_is_a_hard_error_not_a_denial() {
    let request = serde_json::json!({ "not": "a valid execution request" });
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");

    let error = DecisionService
        .resolve_route_decision(
            &request,
            &requester_trust,
            &policy,
            &capability_registry,
            decision_context("77777777-7777-4777-8777-777777777777"),
        )
        .unwrap_err();

    assert!(!error.to_string().is_empty());
}

#[test]
fn malformed_capability_registry_is_a_hard_error_not_a_denial() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = serde_json::json!({ "not": "a valid capability registry" });

    let error = DecisionService
        .resolve_route_decision(
            &request,
            &requester_trust,
            &policy,
            &capability_registry,
            decision_context("77777777-7777-4777-8777-777777777778"),
        )
        .unwrap_err();

    assert!(!error.to_string().is_empty());
}
