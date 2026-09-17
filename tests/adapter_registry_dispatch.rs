mod support;

use chrono::{TimeZone, Utc};

use fa_local::adapters::execution_delivery::registry::AdapterRegistry;
use fa_local::adapters::execution_delivery::{
    AdapterDeliveryRequest, AdapterDeliveryResult, ExternalRouteDeliveryAdapter,
};
use fa_local::app::execution_service::{CoordinationContext, ExecutionService};
use fa_local::app::routing_service::{RoutingInput, RoutingService, SelectedExecutionRoute};
use fa_local::domain::capabilities::CapabilityRegistryLoader;
use fa_local::domain::execution::{ExecutionPlan, ExecutionPlanValidator, ValidatedExecutionPlan};
use fa_local::domain::routing::{RouteDecision, RouteDecisionLoader};
use fa_local::{DegradedSubtype, ExecutionState};

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
        ts(2030, 1, 1, 0, 40, 0),
        ts(2030, 1, 1, 0, 40, 5),
        ts(2030, 1, 1, 0, 40, 30),
    )
}

fn basic_validated_plan() -> ValidatedExecutionPlan {
    let registry = CapabilityRegistryLoader::load_contract_value(&support::load_fixture_json(
        "valid",
        "capability-registry-basic.json",
    ))
    .unwrap();
    let plan = ExecutionPlan::load_contract_value(&support::load_fixture_json(
        "valid",
        "execution-plan-basic.json",
    ))
    .unwrap();

    ExecutionPlanValidator::validate(&plan, &registry).unwrap()
}

fn basic_route_decision(file_name: &str) -> RouteDecision {
    RouteDecisionLoader::load_contract_value(&support::load_fixture_json("valid", file_name))
        .unwrap()
}

fn basic_selected_route() -> SelectedExecutionRoute {
    RoutingService
        .select_route(
            RoutingInput::new(
                basic_route_decision("route-decision-policy-preapproved-basic.json"),
                Some(basic_validated_plan()),
            )
            .unwrap(),
        )
        .unwrap()
}

fn nmap_validated_plan() -> ValidatedExecutionPlan {
    let registry = CapabilityRegistryLoader::load_contract_value(&support::load_fixture_json(
        "valid",
        "capability-registry-nmap-preflight.json",
    ))
    .unwrap();
    let plan = ExecutionPlan::load_contract_value(&support::load_fixture_json(
        "valid",
        "execution-plan-nmap-preflight.json",
    ))
    .unwrap();

    ExecutionPlanValidator::validate(&plan, &registry).unwrap()
}

fn nmap_selected_route() -> SelectedExecutionRoute {
    RoutingService
        .select_route(
            RoutingInput::new(
                RouteDecisionLoader::load_contract_value(&support::load_fixture_json(
                    "valid",
                    "route-decision-nmap-preflight-policy-preapproved.json",
                ))
                .unwrap(),
                Some(nmap_validated_plan()),
            )
            .unwrap(),
        )
        .unwrap()
}

#[derive(Debug)]
struct StubAdapter {
    adapter_id: &'static str,
    result: AdapterDeliveryResult,
}

impl StubAdapter {
    fn new(adapter_id: &'static str, result: AdapterDeliveryResult) -> Self {
        Self { adapter_id, result }
    }
}

impl ExternalRouteDeliveryAdapter for StubAdapter {
    fn adapter_id(&self) -> &'static str {
        self.adapter_id
    }

    fn deliver_route(&self, _request: &AdapterDeliveryRequest) -> AdapterDeliveryResult {
        self.result.clone()
    }
}

#[test]
fn registry_dispatches_each_route_to_its_own_capability_scoped_adapter() {
    let basic_route = basic_selected_route();
    let nmap_route = nmap_selected_route();
    assert_ne!(
        basic_route.requested_capability_id,
        nmap_route.requested_capability_id
    );

    let basic_adapter = StubAdapter::new("basic-adapter", AdapterDeliveryResult::DeliveredAllSteps);
    let nmap_adapter = StubAdapter::new("nmap-adapter", AdapterDeliveryResult::DeliveredAllSteps);

    let mut registry = AdapterRegistry::new();
    registry
        .register(basic_route.requested_capability_id, Box::new(basic_adapter))
        .unwrap();
    registry
        .register(nmap_route.requested_capability_id, Box::new(nmap_adapter))
        .unwrap();

    let basic_trace = ExecutionService
        .deliver_selected_route_via_registry(&basic_route, &registry, context())
        .unwrap();
    assert_eq!(
        basic_trace.final_status().status.state,
        ExecutionState::Completed
    );

    let nmap_trace = ExecutionService
        .deliver_selected_route_via_registry(&nmap_route, &registry, context())
        .unwrap();
    assert_eq!(
        nmap_trace.final_status().status.state,
        ExecutionState::Completed
    );

    let basic_calls = registry
        .resolve(basic_route.requested_capability_id)
        .map(|adapter| adapter.adapter_id())
        .unwrap();
    let nmap_calls = registry
        .resolve(nmap_route.requested_capability_id)
        .map(|adapter| adapter.adapter_id())
        .unwrap();
    assert_eq!(basic_calls, "basic-adapter");
    assert_eq!(nmap_calls, "nmap-adapter");
}

#[test]
fn registry_only_invokes_the_capability_matched_adapter() {
    let basic_route = basic_selected_route();
    let nmap_route = nmap_selected_route();

    let basic_adapter = StubAdapter::new("basic-adapter", AdapterDeliveryResult::DeliveredAllSteps);
    let nmap_adapter = StubAdapter::new(
        "nmap-adapter",
        AdapterDeliveryResult::FailedAtDeclaredStep {
            step_id: "should-not-be-reached".to_owned(),
            failure_summary: "nmap adapter must not be invoked for the basic route".to_owned(),
        },
    );

    let mut registry = AdapterRegistry::new();
    registry
        .register(basic_route.requested_capability_id, Box::new(basic_adapter))
        .unwrap();
    registry
        .register(nmap_route.requested_capability_id, Box::new(nmap_adapter))
        .unwrap();

    let trace = ExecutionService
        .deliver_selected_route_via_registry(&basic_route, &registry, context())
        .unwrap();

    assert_eq!(trace.final_status().status.state, ExecutionState::Completed);
}

#[test]
fn unregistered_capability_maps_to_truthful_degraded_status_not_an_error() {
    let route = basic_selected_route();
    let registry = AdapterRegistry::new();

    let trace = ExecutionService
        .deliver_selected_route_via_registry(&route, &registry, context())
        .unwrap();

    assert_eq!(trace.statuses.len(), 2);
    assert_eq!(
        trace.statuses[0].status.state,
        ExecutionState::AdmittedNotStarted
    );
    assert_eq!(trace.final_status().status.state, ExecutionState::Degraded);
    assert_eq!(
        trace.final_status().status.degraded_subtype,
        Some(DegradedSubtype::UnavailableDependencyBlock)
    );
    assert!(
        trace
            .final_status()
            .status
            .truthful_user_visible_summary
            .contains(&route.requested_capability_id.to_string())
    );
}

#[test]
fn duplicate_capability_registration_is_rejected_before_any_dispatch() {
    let route = basic_selected_route();
    let first = StubAdapter::new("first", AdapterDeliveryResult::DeliveredAllSteps);
    let second = StubAdapter::new("second", AdapterDeliveryResult::DeliveredAllSteps);

    let mut registry = AdapterRegistry::new();
    registry
        .register(route.requested_capability_id, Box::new(first))
        .unwrap();

    let error = registry
        .register(route.requested_capability_id, Box::new(second))
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        format!(
            "contract invalid: adapter already registered for capability {}",
            route.requested_capability_id
        )
    );
}

#[test]
fn denied_and_review_routes_never_reach_registry_resolution() {
    let denied_route = RoutingService
        .select_route(
            RoutingInput::new(
                basic_route_decision("route-decision-denied-basic.json"),
                None,
            )
            .unwrap(),
        )
        .unwrap();

    let registry = AdapterRegistry::new();
    let error = ExecutionService
        .deliver_selected_route_via_registry(&denied_route, &registry, context())
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "contract invalid: non-executable route must not reach adapter delivery"
    );
}
