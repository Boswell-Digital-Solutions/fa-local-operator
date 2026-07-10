use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use fa_local::adapters::execution_delivery::nmap_preflight::{
    NmapPreflightAdapterConfig, NmapPreflightDeliveryAdapter, NmapScanProfile,
};
use fa_local::adapters::execution_delivery::{
    AdapterDeliveryRequest, AdapterDeliveryResult, ExternalRouteDeliveryAdapter,
};
use fa_local::app::execution_service::{CoordinationContext, ExecutionService};
use fa_local::app::routing_service::{RoutePathKind, SelectedExecutionRoute};
use fa_local::{
    ApprovalPosture, CapabilityId, CorrelationId, DegradedSubtype, ExecutionPlanId, ExecutionState,
    RequestId, RouteDecisionId,
};

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
        ts(2030, 1, 1, 0, 30, 0),
        ts(2030, 1, 1, 0, 30, 5),
        ts(2030, 1, 1, 0, 30, 30),
    )
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fa-local-nmap-{label}-{}", Uuid::new_v4()))
}

fn capability_id() -> CapabilityId {
    CapabilityId::from_uuid(Uuid::parse_str("77777777-7777-4777-8777-777777777777").unwrap())
}

fn other_capability_id() -> CapabilityId {
    CapabilityId::from_uuid(Uuid::parse_str("88888888-8888-4888-8888-888888888888").unwrap())
}

fn nmap_adapter(nmap_binary: PathBuf) -> NmapPreflightDeliveryAdapter {
    NmapPreflightDeliveryAdapter::new(NmapPreflightAdapterConfig::new(
        capability_id(),
        nmap_binary,
        NmapScanProfile::LoopbackTcpConnectV1,
    ))
}

fn request() -> AdapterDeliveryRequest {
    AdapterDeliveryRequest {
        route_decision_id: RouteDecisionId::from_uuid(
            Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
        ),
        correlation_id: CorrelationId::from_uuid(
            Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap(),
        ),
        request_id: RequestId::from_uuid(
            Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        ),
        resolved_approval_posture: ApprovalPosture::PolicyPreapproved,
        requested_capability_id: capability_id(),
        execution_plan_id: ExecutionPlanId::from_uuid(
            Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
        ),
        stable_plan_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_owned(),
        declared_step_ids: vec!["step_nmap_preflight".to_owned()],
        declared_capability_ids: vec![capability_id()],
        declared_fallback_references: Vec::new(),
    }
}

fn selected_route() -> SelectedExecutionRoute {
    let request = request();
    SelectedExecutionRoute {
        route_path_kind: RoutePathKind::ExternalAdapterBoundedExecution,
        route_decision_id: request.route_decision_id,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
        resolved_approval_posture: request.resolved_approval_posture,
        requested_capability_id: request.requested_capability_id,
        execution_plan_id: Some(request.execution_plan_id),
        stable_plan_hash: Some(request.stable_plan_hash),
        declared_step_ids: request.declared_step_ids,
        declared_capability_ids: request.declared_capability_ids,
        declared_fallback_references: request.declared_fallback_references,
        executable: true,
        explicit_approval_required: false,
        operator_visible_summary: "nmap preflight route selected".to_owned(),
    }
}

#[test]
fn missing_nmap_runtime_maps_to_dependency_unavailable() {
    let adapter = nmap_adapter(temp_path("missing-runtime"));

    let result = adapter.deliver_route(&request());

    assert_eq!(
        result,
        AdapterDeliveryResult::DependencyUnavailable {
            summary: "nmap runtime is unavailable for declared scan profile".to_owned()
        }
    );
}

#[test]
fn available_nmap_runtime_marks_preflight_delivered_without_scan() {
    let nmap_binary = temp_path("available-runtime");
    fs::write(&nmap_binary, "not invoked by preflight adapter").unwrap();
    let adapter = nmap_adapter(nmap_binary.clone());

    assert_eq!(adapter.nmap_binary(), nmap_binary.as_path());
    assert_eq!(adapter.scan_profile().as_str(), "loopback_tcp_connect_v1");
    assert_eq!(
        adapter.deliver_route(&request()),
        AdapterDeliveryResult::DeliveredAllSteps
    );

    let _ = fs::remove_file(nmap_binary);
}

#[test]
fn preflight_contract_has_profile_but_no_free_form_argument_surface() {
    let adapter = nmap_adapter(temp_path("profile-only"));

    assert_eq!(adapter.adapter_id(), "nmap-preflight-delivery");
    assert_eq!(adapter.scan_profile().as_str(), "loopback_tcp_connect_v1");
    assert_eq!(
        NmapScanProfile::AuthorizedPrivateSubnetTcpConnectV1.as_str(),
        "authorized_private_subnet_tcp_connect_v1"
    );
}

#[test]
fn capability_mismatch_is_unsupported() {
    let nmap_binary = temp_path("capability-mismatch");
    fs::write(&nmap_binary, "not invoked by preflight adapter").unwrap();
    let adapter = nmap_adapter(nmap_binary.clone());
    let mut request = request();
    request.requested_capability_id = other_capability_id();

    assert_eq!(
        adapter.deliver_route(&request),
        AdapterDeliveryResult::Unsupported {
            summary: "nmap preflight adapter capability mismatch".to_owned()
        }
    );

    let _ = fs::remove_file(nmap_binary);
}

#[test]
fn non_admitted_posture_is_unsupported() {
    let nmap_binary = temp_path("non-admitted");
    fs::write(&nmap_binary, "not invoked by preflight adapter").unwrap();
    let adapter = nmap_adapter(nmap_binary.clone());
    let mut request = request();
    request.resolved_approval_posture = ApprovalPosture::ReviewRequired;

    assert_eq!(
        adapter.deliver_route(&request),
        AdapterDeliveryResult::Unsupported {
            summary: "nmap preflight adapter requires admitted posture".to_owned()
        }
    );

    let _ = fs::remove_file(nmap_binary);
}

#[test]
fn execution_service_maps_missing_nmap_to_degraded_status() {
    let adapter = nmap_adapter(temp_path("service-missing-runtime"));

    let trace = ExecutionService
        .deliver_selected_route(&selected_route(), &adapter, context())
        .unwrap();

    assert_eq!(trace.statuses.len(), 2);
    assert_eq!(
        trace.statuses.first().unwrap().status.state,
        ExecutionState::AdmittedNotStarted
    );
    assert_eq!(trace.final_status().status.state, ExecutionState::Degraded);
    assert_eq!(
        trace.final_status().status.degraded_subtype,
        Some(DegradedSubtype::UnavailableDependencyBlock)
    );
    assert_eq!(
        trace.final_status().status.truthful_user_visible_summary,
        "nmap runtime is unavailable for declared scan profile"
    );
}
