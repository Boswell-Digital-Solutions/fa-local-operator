mod support;

use chrono::{TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

use fa_local::adapters::execution_delivery::registry::AdapterRegistry;
use fa_local::adapters::execution_delivery::{
    AdapterDeliveryRequest, AdapterDeliveryResult, ExternalRouteDeliveryAdapter,
};
use fa_local::app::execution_service::{CoordinationContext, ExecutionService};
use fa_local::app::routing_service::{RoutingInput, RoutingService};
use fa_local::domain::capabilities::{CapabilityRegistry, CapabilityRegistryLoader};
use fa_local::domain::execution::{ExecutionPlan, ExecutionPlanValidator, ValidatedExecutionPlan};
use fa_local::domain::routing::{RouteDecision, RouteDecisionLoader};
use fa_local::{ApprovalPosture, CapabilityId, DegradedSubtype, ExecutionState};

fn ts(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .unwrap()
}

fn context() -> CoordinationContext {
    CoordinationContext::new(
        ts(2030, 1, 1, 0, 55, 0),
        ts(2030, 1, 1, 0, 55, 5),
        ts(2030, 1, 1, 0, 55, 30),
    )
}

fn second_capability_id() -> CapabilityId {
    CapabilityId::from_uuid(Uuid::parse_str("88888888-8888-4888-8888-888888888888").unwrap())
}

fn two_capability_registry() -> CapabilityRegistry {
    let mut value = support::load_fixture_json("valid", "capability-registry-basic.json");
    let mut second = value["capabilities"][0].clone();
    second["capability_id"] = json!(second_capability_id().to_string());
    value["capabilities"].as_array_mut().unwrap().push(second);
    CapabilityRegistryLoader::load_contract_value(&value).unwrap()
}

fn multi_step_plan(
    cancellation_policy: &str,
    registry: &CapabilityRegistry,
) -> ValidatedExecutionPlan {
    let value = json!({
        "execution_plan_id": Uuid::new_v4().to_string(),
        "correlation_id": "66666666-6666-4666-8666-666666666666",
        "originating_request_id": "55555555-5555-4555-8555-555555555555",
        "steps": [
            {
                "step_id": "step_a",
                "capability_id": "44444444-4444-4444-8444-444444444444",
                "declared_side_effect_class": "local_file_write",
                "timeout_budget_ms": 400
            },
            {
                "step_id": "step_b",
                "capability_id": second_capability_id().to_string(),
                "declared_side_effect_class": "local_file_write",
                "timeout_budget_ms": 400
            }
        ],
        "referenced_capabilities": [
            "44444444-4444-4444-8444-444444444444",
            second_capability_id().to_string()
        ],
        "declared_max_step_count": 4,
        "declared_side_effect_classes": ["local_file_write"],
        "fallback_references": [],
        "cancellation_policy": cancellation_policy,
        "completion_policy": "all_steps_must_succeed",
        "max_duration_budget_ms": 2000,
        "stable_plan_hash": "a".repeat(64),
        "planned_at_utc": "2030-01-01T00:10:00Z"
    });

    let mut plan = ExecutionPlan::load_contract_value(&value).unwrap();
    plan.stable_plan_hash = ExecutionPlanValidator::compute_stable_plan_hash(&plan);
    ExecutionPlanValidator::validate(&plan, registry).unwrap()
}

fn route_decision() -> RouteDecision {
    RouteDecisionLoader::load_contract_value(&support::load_fixture_json(
        "valid",
        "route-decision-policy-preapproved-basic.json",
    ))
    .unwrap()
}

fn selected_route(
    validated_plan: ValidatedExecutionPlan,
) -> fa_local::app::routing_service::SelectedExecutionRoute {
    RoutingService
        .select_route(RoutingInput::new(route_decision(), Some(validated_plan)).unwrap())
        .unwrap()
}

#[derive(Debug)]
struct StubAdapter {
    id: &'static str,
    result: AdapterDeliveryResult,
}

impl StubAdapter {
    fn new(id: &'static str, result: AdapterDeliveryResult) -> Self {
        Self { id, result }
    }
}

impl ExternalRouteDeliveryAdapter for StubAdapter {
    fn adapter_id(&self) -> &'static str {
        self.id
    }

    fn deliver_route(&self, _request: &AdapterDeliveryRequest) -> AdapterDeliveryResult {
        self.result.clone()
    }
}

#[test]
fn each_step_dispatches_to_its_own_capability_scoped_adapter_and_completes() {
    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            CapabilityId::from_uuid(
                Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
            ),
            Box::new(StubAdapter::new(
                "adapter-a",
                AdapterDeliveryResult::DeliveredAllSteps,
            )),
        )
        .unwrap();
    registry
        .register(
            second_capability_id(),
            Box::new(StubAdapter::new(
                "adapter-b",
                AdapterDeliveryResult::DeliveredAllSteps,
            )),
        )
        .unwrap();

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    let states: Vec<_> = trace.statuses.iter().map(|s| s.status.state).collect();
    assert_eq!(
        states,
        vec![
            ExecutionState::AdmittedNotStarted,
            ExecutionState::InProgress,
            ExecutionState::InProgress,
            ExecutionState::Completed,
        ]
    );
    assert_eq!(
        trace.statuses[1].status.current_step.as_deref(),
        Some("step_a")
    );
    assert_eq!(
        trace.statuses[2].status.current_step.as_deref(),
        Some("step_b")
    );
}

#[test]
fn one_missing_adapter_among_two_reports_partial_success_not_fabricated_completion() {
    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            CapabilityId::from_uuid(
                Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
            ),
            Box::new(StubAdapter::new(
                "adapter-a",
                AdapterDeliveryResult::DeliveredAllSteps,
            )),
        )
        .unwrap();
    // No adapter registered for the second capability.

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    let final_status = &trace.final_status().status;
    assert_eq!(final_status.state, ExecutionState::PartialSuccess);
    assert_eq!(
        final_status.degraded_subtype,
        Some(DegradedSubtype::DegradedPartial)
    );
    assert_eq!(
        final_status.completion_summary.as_deref(),
        Some("1 of 2 declared plan steps completed")
    );
    assert!(
        final_status
            .failure_summary
            .as_deref()
            .unwrap()
            .contains("step_b")
    );
}

#[test]
fn no_adapters_registered_for_either_step_degrades_truthfully() {
    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let route = selected_route(plan.clone());
    let registry = AdapterRegistry::new();

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    let final_status = &trace.final_status().status;
    assert_eq!(final_status.state, ExecutionState::Degraded);
    assert_eq!(
        final_status.degraded_subtype,
        Some(DegradedSubtype::UnavailableDependencyBlock)
    );
}

#[test]
fn cancel_remaining_steps_policy_skips_later_steps_after_a_failure_and_reports_failed() {
    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            CapabilityId::from_uuid(
                Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
            ),
            Box::new(StubAdapter::new(
                "adapter-a",
                AdapterDeliveryResult::FailedAtDeclaredStep {
                    step_id: "step_a".to_owned(),
                    failure_summary: "adapter a refused the step".to_owned(),
                },
            )),
        )
        .unwrap();
    registry
        .register(
            second_capability_id(),
            Box::new(StubAdapter::new(
                "adapter-b",
                AdapterDeliveryResult::DeliveredAllSteps,
            )),
        )
        .unwrap();

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    // step_b must never reach an InProgress status: it was canceled before dispatch.
    let in_progress_steps: Vec<_> = trace
        .statuses
        .iter()
        .filter(|s| s.status.state == ExecutionState::InProgress)
        .map(|s| s.status.current_step.clone().unwrap())
        .collect();
    assert_eq!(in_progress_steps, vec!["step_a".to_owned()]);

    let final_status = &trace.final_status().status;
    assert_eq!(final_status.state, ExecutionState::Failed);
    assert_eq!(
        final_status.failure_summary.as_deref(),
        Some("adapter a refused the step")
    );
}

#[test]
fn finish_in_flight_only_policy_still_dispatches_later_steps_after_a_failure() {
    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("finish_in_flight_only", &registry_typed);
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            CapabilityId::from_uuid(
                Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
            ),
            Box::new(StubAdapter::new(
                "adapter-a",
                AdapterDeliveryResult::FailedAtDeclaredStep {
                    step_id: "step_a".to_owned(),
                    failure_summary: "adapter a refused the step".to_owned(),
                },
            )),
        )
        .unwrap();
    registry
        .register(
            second_capability_id(),
            Box::new(StubAdapter::new(
                "adapter-b",
                AdapterDeliveryResult::DeliveredAllSteps,
            )),
        )
        .unwrap();

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    let in_progress_steps: Vec<_> = trace
        .statuses
        .iter()
        .filter(|s| s.status.state == ExecutionState::InProgress)
        .map(|s| s.status.current_step.clone().unwrap())
        .collect();
    assert_eq!(
        in_progress_steps,
        vec!["step_a".to_owned(), "step_b".to_owned()]
    );

    let final_status = &trace.final_status().status;
    assert_eq!(final_status.state, ExecutionState::PartialSuccess);
}

#[test]
fn declared_fallback_completion_is_unsupported_in_per_step_delivery() {
    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            CapabilityId::from_uuid(
                Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
            ),
            Box::new(StubAdapter::new(
                "adapter-a",
                AdapterDeliveryResult::CompletedWithDeclaredFallback {
                    step_id: "step_a".to_owned(),
                    fallback_step_id: "step_b".to_owned(),
                    degraded_subtype: DegradedSubtype::DegradedFallbackLimited,
                },
            )),
        )
        .unwrap();

    let error = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "contract invalid: declared fallback completion is not supported in per-step delivery"
    );
}

#[test]
fn a_plan_that_does_not_match_the_route_is_rejected() {
    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let other_plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let route = selected_route(plan);

    let registry = AdapterRegistry::new();

    let error = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &other_plan, &registry, context())
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "contract invalid: per-step delivery plan does not match the selected route's plan"
    );
}

#[test]
fn denied_and_review_required_routes_never_reach_per_step_delivery() {
    let denied_route_decision = RouteDecisionLoader::load_contract_value(
        &support::load_fixture_json("valid", "route-decision-denied-basic.json"),
    )
    .unwrap();
    let route = RoutingService
        .select_route(RoutingInput::new(denied_route_decision, None).unwrap())
        .unwrap();

    let registry_typed = two_capability_registry();
    let plan = multi_step_plan("cancel_remaining_steps", &registry_typed);
    let registry = AdapterRegistry::new();

    let error = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "contract invalid: non-executable route must not reach adapter delivery"
    );

    // Sanity: the resolved route posture really was non-executable.
    assert!(!route.executable);
    assert_eq!(route.resolved_approval_posture, ApprovalPosture::Denied);
}
