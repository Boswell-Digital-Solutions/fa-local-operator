mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn third_capability_id() -> CapabilityId {
    CapabilityId::from_uuid(Uuid::parse_str("99999999-9999-4999-8999-999999999999").unwrap())
}

fn two_capability_registry() -> CapabilityRegistry {
    let mut value = support::load_fixture_json("valid", "capability-registry-basic.json");
    let mut second = value["capabilities"][0].clone();
    second["capability_id"] = json!(second_capability_id().to_string());
    value["capabilities"].as_array_mut().unwrap().push(second);
    CapabilityRegistryLoader::load_contract_value(&value).unwrap()
}

fn three_capability_registry() -> CapabilityRegistry {
    let mut value = support::load_fixture_json("valid", "capability-registry-basic.json");
    let mut second = value["capabilities"][0].clone();
    second["capability_id"] = json!(second_capability_id().to_string());
    let mut third = value["capabilities"][0].clone();
    third["capability_id"] = json!(third_capability_id().to_string());
    value["capabilities"].as_array_mut().unwrap().push(second);
    value["capabilities"].as_array_mut().unwrap().push(third);
    CapabilityRegistryLoader::load_contract_value(&value).unwrap()
}

/// Like [`multi_step_plan`], but for scenarios needing declared fallback
/// references: an arbitrary `steps` list (each a `(step_id, capability_id)`
/// pair) and `fallback_references` (each a `(step_id, fallback_step_id)`
/// pair), under the fallback-aware `allow_declared_fallback_completion`
/// completion policy `ExecutionPlanValidator` requires whenever
/// `fallback_references` is non-empty.
fn plan_with_fallback(
    cancellation_policy: &str,
    steps: &[(&str, CapabilityId)],
    fallback_references: &[(&str, &str)],
    registry: &CapabilityRegistry,
) -> ValidatedExecutionPlan {
    let steps_json: Vec<_> = steps
        .iter()
        .map(|(step_id, capability_id)| {
            json!({
                "step_id": step_id,
                "capability_id": capability_id.to_string(),
                "declared_side_effect_class": "local_file_write",
                "timeout_budget_ms": 400
            })
        })
        .collect();

    let referenced_capabilities: Vec<String> = {
        let mut seen = std::collections::BTreeSet::new();
        for (_, capability_id) in steps {
            seen.insert(capability_id.to_string());
        }
        seen.into_iter().collect()
    };

    let fallback_json: Vec<_> = fallback_references
        .iter()
        .map(|(step_id, fallback_step_id)| {
            json!({
                "step_id": step_id,
                "fallback_step_id": fallback_step_id
            })
        })
        .collect();

    let value = json!({
        "execution_plan_id": Uuid::new_v4().to_string(),
        "correlation_id": "66666666-6666-4666-8666-666666666666",
        "originating_request_id": "55555555-5555-4555-8555-555555555555",
        "steps": steps_json,
        "referenced_capabilities": referenced_capabilities,
        "declared_max_step_count": 6,
        "declared_side_effect_classes": ["local_file_write"],
        "fallback_references": fallback_json,
        "cancellation_policy": cancellation_policy,
        "completion_policy": "allow_declared_fallback_completion",
        "max_duration_budget_ms": 4000,
        "stable_plan_hash": "a".repeat(64),
        "planned_at_utc": "2030-01-01T00:10:00Z"
    });

    let mut plan = ExecutionPlan::load_contract_value(&value).unwrap();
    plan.stable_plan_hash = ExecutionPlanValidator::compute_stable_plan_hash(&plan);
    ExecutionPlanValidator::validate(&plan, registry).unwrap()
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

/// Like [`StubAdapter`], but counts its own `deliver_route` calls through a
/// shared handle the test keeps, so a test can assert a step was dispatched
/// exactly once even when it is also a declared fallback target.
#[derive(Debug)]
struct CountingStubAdapter {
    id: &'static str,
    result: AdapterDeliveryResult,
    calls: Arc<AtomicUsize>,
}

impl CountingStubAdapter {
    fn new(id: &'static str, result: AdapterDeliveryResult, calls: Arc<AtomicUsize>) -> Self {
        Self { id, result, calls }
    }
}

impl ExternalRouteDeliveryAdapter for CountingStubAdapter {
    fn adapter_id(&self) -> &'static str {
        self.id
    }

    fn deliver_route(&self, _request: &AdapterDeliveryRequest) -> AdapterDeliveryResult {
        self.calls.fetch_add(1, Ordering::SeqCst);
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

#[test]
fn a_declared_fallback_rescues_a_failed_step_via_a_different_adapter() {
    let registry_typed = two_capability_registry();
    let first_capability_id =
        CapabilityId::from_uuid(Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap());
    let plan = plan_with_fallback(
        "cancel_remaining_steps",
        &[
            ("step_a", first_capability_id),
            ("step_b", second_capability_id()),
        ],
        &[("step_a", "step_b")],
        &registry_typed,
    );
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            first_capability_id,
            Box::new(StubAdapter::new(
                "adapter-a",
                AdapterDeliveryResult::FailedAtDeclaredStep {
                    step_id: "step_a".to_owned(),
                    failure_summary: "adapter a refused the step".to_owned(),
                },
            )),
        )
        .unwrap();
    let step_b_calls = Arc::new(AtomicUsize::new(0));
    registry
        .register(
            second_capability_id(),
            Box::new(CountingStubAdapter::new(
                "adapter-b",
                AdapterDeliveryResult::DeliveredAllSteps,
                step_b_calls.clone(),
            )),
        )
        .unwrap();

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    // step_b is dispatched exactly once, as step_a's fallback -- its own
    // later normal turn in declared order reuses that same result rather
    // than delivering it a second time.
    assert_eq!(step_b_calls.load(Ordering::SeqCst), 1);

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
    assert_eq!(final_status.state, ExecutionState::CompletedWithConstraints);
    assert_eq!(
        final_status.degraded_subtype,
        Some(DegradedSubtype::DegradedFallbackLimited)
    );
    assert!(
        final_status
            .completion_summary
            .as_deref()
            .unwrap()
            .contains("step_a -> step_b")
    );
}

#[test]
fn a_declared_fallback_that_also_fails_leaves_the_original_failure_standing() {
    let registry_typed = two_capability_registry();
    let first_capability_id =
        CapabilityId::from_uuid(Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap());
    let plan = plan_with_fallback(
        "cancel_remaining_steps",
        &[
            ("step_a", first_capability_id),
            ("step_b", second_capability_id()),
        ],
        &[("step_a", "step_b")],
        &registry_typed,
    );
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            first_capability_id,
            Box::new(StubAdapter::new(
                "adapter-a",
                AdapterDeliveryResult::FailedAtDeclaredStep {
                    step_id: "step_a".to_owned(),
                    failure_summary: "adapter a refused the step".to_owned(),
                },
            )),
        )
        .unwrap();
    let step_b_calls = Arc::new(AtomicUsize::new(0));
    registry
        .register(
            second_capability_id(),
            Box::new(CountingStubAdapter::new(
                "adapter-b",
                AdapterDeliveryResult::FailedAtDeclaredStep {
                    step_id: "step_b".to_owned(),
                    failure_summary: "adapter b also refused the step".to_owned(),
                },
                step_b_calls.clone(),
            )),
        )
        .unwrap();

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    // The fallback was attempted exactly once, not skipped and not retried.
    assert_eq!(step_b_calls.load(Ordering::SeqCst), 1);

    let final_status = &trace.final_status().status;
    assert_eq!(final_status.state, ExecutionState::Failed);
    assert_eq!(
        final_status.failure_summary.as_deref(),
        Some("adapter a refused the step")
    );
}

#[test]
fn a_fallback_step_already_consumed_by_one_failed_step_is_not_dispatched_again_for_another() {
    let registry_typed = three_capability_registry();
    let first_capability_id =
        CapabilityId::from_uuid(Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap());
    // Declared order matters: the fallback target (step_b) must be later
    // than every step that declares it as a fallback (plan validation
    // enforces this), so step_b comes last even though it is not the
    // second step chronologically triggered.
    let plan = plan_with_fallback(
        "cancel_remaining_steps",
        &[
            ("step_a", first_capability_id),
            ("step_c", third_capability_id()),
            ("step_b", second_capability_id()),
        ],
        &[("step_a", "step_b"), ("step_c", "step_b")],
        &registry_typed,
    );
    let route = selected_route(plan.clone());

    let mut registry = AdapterRegistry::new();
    registry
        .register(
            first_capability_id,
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
            third_capability_id(),
            Box::new(StubAdapter::new(
                "adapter-c",
                AdapterDeliveryResult::FailedAtDeclaredStep {
                    step_id: "step_c".to_owned(),
                    failure_summary: "adapter c refused the step".to_owned(),
                },
            )),
        )
        .unwrap();
    let step_b_calls = Arc::new(AtomicUsize::new(0));
    registry
        .register(
            second_capability_id(),
            Box::new(CountingStubAdapter::new(
                "adapter-b",
                AdapterDeliveryResult::DeliveredAllSteps,
                step_b_calls.clone(),
            )),
        )
        .unwrap();

    let trace = ExecutionService
        .deliver_plan_per_step_via_registry(&route, &plan, &registry, context())
        .unwrap();

    // step_a consumes step_b as its fallback; step_c declares the same
    // fallback but step_b is already spent, so step_c's own failure stands
    // and step_b is dispatched only once overall.
    assert_eq!(step_b_calls.load(Ordering::SeqCst), 1);

    let final_status = &trace.final_status().status;
    assert_eq!(final_status.state, ExecutionState::PartialSuccess);
    assert_eq!(
        final_status.degraded_subtype,
        Some(DegradedSubtype::DegradedPartial)
    );
    assert_eq!(
        final_status.completion_summary.as_deref(),
        Some("2 of 3 declared plan steps completed")
    );
    assert!(
        final_status
            .failure_summary
            .as_deref()
            .unwrap()
            .contains("step_c")
    );
}
