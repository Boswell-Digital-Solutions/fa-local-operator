mod support;

use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use fa_local::adapters::exports::jsonl_forensic_export::{
    JsonlForensicExportAdapter, JsonlForensicExportAdapterConfig,
};
use fa_local::app::execution_pipeline_service::{
    AdapterSelection, CapabilityScopedAdapterSelection, DispatchMode, ExecutionPipelineInputs,
    ExecutionPipelineService,
};
use fa_local::domain::execution::{ExecutionPlan, ExecutionPlanValidator};
use fa_local::domain::forensics::ForensicEventType;
use fa_local::domain::posture::RouteResolutionContext;
use fa_local::{ApprovalPosture, DegradedSubtype, ExecutionState, RouteDecisionId};

fn decision_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 5, 0).unwrap()
}

fn decision_context(value: &str) -> RouteResolutionContext {
    RouteResolutionContext::new(
        RouteDecisionId::from_uuid(Uuid::parse_str(value).unwrap()),
        decision_time(),
    )
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("fa-local-pipeline-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn temp_export_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-pipeline-{label}-{}.jsonl",
        Uuid::new_v4()
    ))
}

#[test]
fn admitted_route_with_registered_adapter_completes_and_exports_every_status() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");
    let plan = support::load_fixture_json("valid", "execution-plan-basic.json");

    let delivery_root = temp_dir("delivery-root");
    let export_path = temp_export_path("forensics");
    let export_adapter =
        JsonlForensicExportAdapter::new(JsonlForensicExportAdapterConfig::new(export_path.clone()));

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: Some(&plan),
            },
            Some(AdapterSelection::LocalFileWrite {
                delivery_root: delivery_root.clone(),
            }),
            Vec::new(),
            DispatchMode::WholeRoute,
            Some(&export_adapter),
            decision_context("77777777-7777-4777-8777-777777777774"),
        )
        .unwrap();

    assert_eq!(
        outcome.route_decision.resolved_approval_posture,
        ApprovalPosture::PolicyPreapproved
    );
    assert!(outcome.plan_denial.is_none());

    let trace = outcome.execution_trace.unwrap();
    assert_eq!(trace.final_status().status.state, ExecutionState::Completed);

    assert_eq!(outcome.forensic_records.len(), trace.statuses.len());
    for record in &outcome.forensic_records {
        assert_eq!(
            record.event.event.event_type,
            ForensicEventType::ExecutionStatusObserved
        );
        assert!(record.export_reference.is_some());
    }

    let exported_lines = fs::read_to_string(&export_path).unwrap();
    assert_eq!(
        exported_lines.lines().count(),
        outcome.forensic_records.len()
    );

    fs::remove_file(&export_path).ok();
    fs::remove_dir_all(&delivery_root).ok();
}

#[test]
fn admitted_route_with_no_adapter_registered_degrades_truthfully_instead_of_fabricating_success() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");
    let plan = support::load_fixture_json("valid", "execution-plan-basic.json");

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: Some(&plan),
            },
            None,
            Vec::new(),
            DispatchMode::WholeRoute,
            None,
            decision_context("77777777-7777-4777-8777-777777777775"),
        )
        .unwrap();

    let trace = outcome.execution_trace.unwrap();
    assert_eq!(trace.final_status().status.state, ExecutionState::Degraded);
    assert_eq!(outcome.forensic_records.len(), trace.statuses.len());
    for record in &outcome.forensic_records {
        assert!(record.export_reference.is_none());
    }
}

#[test]
fn denied_route_records_a_denial_issued_forensic_event_and_never_reaches_a_plan() {
    let mut request = support::load_fixture_json("valid", "execution-request-basic.json");
    request["requested_side_effect_class"] =
        serde_json::Value::String("external_network_denied_by_default".to_owned());
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: None,
            },
            None,
            Vec::new(),
            DispatchMode::WholeRoute,
            None,
            decision_context("77777777-7777-4777-8777-777777777776"),
        )
        .unwrap();

    assert_eq!(
        outcome.route_decision.resolved_approval_posture,
        ApprovalPosture::Denied
    );
    assert!(outcome.execution_trace.is_none());
    assert!(outcome.plan_denial.is_none());
    assert_eq!(outcome.forensic_records.len(), 1);
    assert_eq!(
        outcome.forensic_records[0].event.event.event_type,
        ForensicEventType::DenialIssued
    );
}

#[test]
fn review_required_route_records_a_route_decision_resolved_forensic_event() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let mut policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    policy["capability_rules"][0]["required_approval_posture"] =
        serde_json::Value::String("review_required".to_owned());
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: None,
            },
            None,
            Vec::new(),
            DispatchMode::WholeRoute,
            None,
            decision_context("77777777-7777-4777-8777-777777777777"),
        )
        .unwrap();

    assert_eq!(
        outcome.route_decision.resolved_approval_posture,
        ApprovalPosture::ReviewRequired
    );
    assert!(outcome.execution_trace.is_none());
    assert_eq!(outcome.forensic_records.len(), 1);
    assert_eq!(
        outcome.forensic_records[0].event.event.event_type,
        ForensicEventType::RouteDecisionResolved
    );
}

#[test]
fn admitted_route_without_a_plan_is_a_hard_error() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");

    let error = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: None,
            },
            None,
            Vec::new(),
            DispatchMode::WholeRoute,
            None,
            decision_context("77777777-7777-4777-8777-777777777778"),
        )
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "contract invalid: admitted route decision requires an execution plan"
    );
}

#[test]
fn admitted_route_with_an_unbounded_plan_reports_a_plan_denial_instead_of_running() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");
    let mut plan = support::load_fixture_json("valid", "execution-plan-basic.json");
    plan["declared_max_step_count"] = serde_json::json!(1);

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: Some(&plan),
            },
            None,
            Vec::new(),
            DispatchMode::WholeRoute,
            None,
            decision_context("77777777-7777-4777-8777-777777777779"),
        )
        .unwrap();

    assert!(outcome.execution_trace.is_none());
    assert!(outcome.plan_denial.is_some());
    assert!(outcome.forensic_records.is_empty());
}

#[test]
fn per_step_dispatch_mode_dispatches_each_declared_step_and_completes() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");
    let capability_registry = support::load_fixture_json("valid", "capability-registry-basic.json");
    let plan = support::load_fixture_json("valid", "execution-plan-basic.json");

    let delivery_root = temp_dir("per-step-delivery-root");

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: Some(&plan),
            },
            Some(AdapterSelection::LocalFileWrite {
                delivery_root: delivery_root.clone(),
            }),
            Vec::new(),
            DispatchMode::PerStep,
            None,
            decision_context("77777777-7777-4777-8777-77777777777a"),
        )
        .unwrap();

    let trace = outcome.execution_trace.unwrap();
    assert_eq!(trace.final_status().status.state, ExecutionState::Completed);

    let in_progress_steps: Vec<_> = trace
        .statuses
        .iter()
        .filter(|status| status.status.state == ExecutionState::InProgress)
        .map(|status| status.status.current_step.clone().unwrap())
        .collect();
    assert_eq!(
        in_progress_steps,
        vec![
            "step_export_prepare".to_owned(),
            "step_export_commit".to_owned()
        ]
    );

    fs::remove_dir_all(&delivery_root).ok();
}

fn plan_json_with_computed_hash(
    steps_and_capabilities: &[(&str, &str)],
    referenced_capabilities: &[&str],
) -> serde_json::Value {
    let steps: Vec<_> = steps_and_capabilities
        .iter()
        .map(|(step_id, capability_id)| {
            serde_json::json!({
                "step_id": step_id,
                "capability_id": capability_id,
                "declared_side_effect_class": "local_file_write",
                "timeout_budget_ms": 400
            })
        })
        .collect();

    let mut plan_json = serde_json::json!({
        "execution_plan_id": Uuid::new_v4().to_string(),
        "correlation_id": "66666666-6666-4666-8666-666666666666",
        "originating_request_id": "55555555-5555-4555-8555-555555555555",
        "steps": steps,
        "referenced_capabilities": referenced_capabilities,
        "declared_max_step_count": 4,
        "declared_side_effect_classes": ["local_file_write"],
        "fallback_references": [],
        "cancellation_policy": "cancel_remaining_steps",
        "completion_policy": "all_steps_must_succeed",
        "max_duration_budget_ms": 2000,
        "stable_plan_hash": "a".repeat(64),
        "planned_at_utc": "2030-01-01T00:10:00Z"
    });

    let typed_plan = ExecutionPlan::load_contract_value(&plan_json).unwrap();
    plan_json["stable_plan_hash"] = serde_json::json!(
        ExecutionPlanValidator::compute_stable_plan_hash(&typed_plan)
    );
    plan_json
}

#[test]
fn per_step_dispatch_mode_reports_partial_success_for_a_step_with_no_adapter() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");

    let second_capability_id = "88888888-8888-4888-8888-888888888888";
    let mut capability_registry =
        support::load_fixture_json("valid", "capability-registry-basic.json");
    let mut second_capability = capability_registry["capabilities"][0].clone();
    second_capability["capability_id"] = serde_json::json!(second_capability_id);
    capability_registry["capabilities"]
        .as_array_mut()
        .unwrap()
        .push(second_capability);

    let plan = plan_json_with_computed_hash(
        &[
            ("step_a", "44444444-4444-4444-8444-444444444444"),
            ("step_b", second_capability_id),
        ],
        &["44444444-4444-4444-8444-444444444444", second_capability_id],
    );

    let delivery_root = temp_dir("per-step-partial-delivery-root");

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: Some(&plan),
            },
            // Registers an adapter only for the route's own capability
            // (44444444...); step_b's capability (88888888...) has none.
            Some(AdapterSelection::LocalFileWrite {
                delivery_root: delivery_root.clone(),
            }),
            Vec::new(),
            DispatchMode::PerStep,
            None,
            decision_context("77777777-7777-4777-8777-77777777777b"),
        )
        .unwrap();

    let trace = outcome.execution_trace.unwrap();
    let final_status = &trace.final_status().status;
    assert_eq!(final_status.state, ExecutionState::PartialSuccess);
    assert_eq!(
        final_status.degraded_subtype,
        Some(DegradedSubtype::DegradedPartial)
    );
    assert!(
        final_status
            .failure_summary
            .as_deref()
            .unwrap()
            .contains("step_b")
    );

    fs::remove_dir_all(&delivery_root).ok();
}

#[test]
fn per_step_dispatch_mode_completes_a_heterogeneous_plan_via_additional_adapters() {
    let request = support::load_fixture_json("valid", "execution-request-basic.json");
    let requester_trust = support::load_fixture_json("valid", "requester-trust-basic.json");
    let policy = support::load_fixture_json("valid", "policy-artifact-basic.json");

    let second_capability_id = "88888888-8888-4888-8888-888888888888";
    let mut capability_registry =
        support::load_fixture_json("valid", "capability-registry-basic.json");
    let mut second_capability = capability_registry["capabilities"][0].clone();
    second_capability["capability_id"] = serde_json::json!(second_capability_id);
    capability_registry["capabilities"]
        .as_array_mut()
        .unwrap()
        .push(second_capability);

    let plan = plan_json_with_computed_hash(
        &[
            ("step_a", "44444444-4444-4444-8444-444444444444"),
            ("step_b", second_capability_id),
        ],
        &["44444444-4444-4444-8444-444444444444", second_capability_id],
    );

    let delivery_root = temp_dir("per-step-multi-adapter-delivery-root");
    let second_delivery_root = temp_dir("per-step-multi-adapter-second-delivery-root");

    let outcome = ExecutionPipelineService
        .run(
            ExecutionPipelineInputs {
                request: &request,
                requester_trust: &requester_trust,
                policy: &policy,
                capability_registry: &capability_registry,
                execution_plan: Some(&plan),
            },
            // Registers the route's own capability (44444444...) as usual.
            Some(AdapterSelection::LocalFileWrite {
                delivery_root: delivery_root.clone(),
            }),
            // Registers a second, distinct adapter for step_b's own
            // capability (88888888...), which the route's own adapter
            // selection above cannot reach.
            vec![CapabilityScopedAdapterSelection {
                capability_id: fa_local::CapabilityId::from_uuid(
                    Uuid::parse_str(second_capability_id).unwrap(),
                ),
                selection: AdapterSelection::LocalFileWrite {
                    delivery_root: second_delivery_root.clone(),
                },
            }],
            DispatchMode::PerStep,
            None,
            decision_context("77777777-7777-4777-8777-77777777777c"),
        )
        .unwrap();

    let trace = outcome.execution_trace.unwrap();
    assert_eq!(trace.final_status().status.state, ExecutionState::Completed);

    fs::remove_dir_all(&delivery_root).ok();
    fs::remove_dir_all(&second_delivery_root).ok();
}
