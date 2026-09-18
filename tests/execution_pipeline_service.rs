mod support;

use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use fa_local::adapters::exports::jsonl_forensic_export::{
    JsonlForensicExportAdapter, JsonlForensicExportAdapterConfig,
};
use fa_local::app::execution_pipeline_service::{
    AdapterSelection, ExecutionPipelineInputs, ExecutionPipelineService,
};
use fa_local::domain::forensics::ForensicEventType;
use fa_local::domain::posture::RouteResolutionContext;
use fa_local::{ApprovalPosture, ExecutionState, RouteDecisionId};

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
            None,
            decision_context("77777777-7777-4777-8777-777777777779"),
        )
        .unwrap();

    assert!(outcome.execution_trace.is_none());
    assert!(outcome.plan_denial.is_some());
    assert!(outcome.forensic_records.is_empty());
}
