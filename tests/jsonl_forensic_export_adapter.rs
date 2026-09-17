mod support;

use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde_json::Value;
use uuid::Uuid;

use fa_local::adapters::exports::jsonl_forensic_export::{
    JsonlForensicExportAdapter, JsonlForensicExportAdapterConfig,
};
use fa_local::adapters::exports::{ForensicEventExportAdapter, ForensicExportResult};
use fa_local::app::forensic_service::{
    ForensicRecordContext, ForensicRecordInput, ForensicRecordKind, ForensicService,
};
use fa_local::domain::forensics::{ForensicEventType, RedactionLevel};
use fa_local::domain::routing::{RouteDecision, RouteDecisionLoader};
use fa_local::domain::status::{ExecutionStatus, ValidatedExecutionStatus};

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

fn context() -> ForensicRecordContext {
    ForensicRecordContext::new(ts(2030, 1, 1, 0, 45, 0))
}

fn temp_export_path() -> PathBuf {
    std::env::temp_dir().join(format!("fa-local-forensic-export-{}.jsonl", Uuid::new_v4()))
}

fn route_decision(file_name: &str) -> RouteDecision {
    RouteDecisionLoader::load_contract_value(&support::load_fixture_json("valid", file_name))
        .unwrap()
}

fn validated_execution_status(file_name: &str) -> ValidatedExecutionStatus {
    let status =
        ExecutionStatus::load_contract_value(&support::load_fixture_json("valid", file_name))
            .unwrap();
    ValidatedExecutionStatus::new(status).unwrap()
}

fn read_lines(path: &PathBuf) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn exports_each_event_as_one_appended_jsonl_line() {
    let export_path = temp_export_path();
    let adapter =
        JsonlForensicExportAdapter::new(JsonlForensicExportAdapterConfig::new(export_path.clone()));

    let denial_outcome = ForensicService
        .record_and_export_event(
            ForensicRecordInput::new(
                ForensicRecordKind::DenialIssued {
                    route_decision: route_decision("route-decision-denied-basic.json"),
                },
                RedactionLevel::SensitiveFieldsRedacted,
                context(),
            )
            .unwrap(),
            &adapter,
        )
        .unwrap();

    let status_outcome = ForensicService
        .record_and_export_event(
            ForensicRecordInput::new(
                ForensicRecordKind::ExecutionStatusObserved {
                    route_decision: route_decision("route-decision-policy-preapproved-basic.json"),
                    execution_status: validated_execution_status(
                        "execution-status-completed-with-constraints-basic.json",
                    ),
                },
                RedactionLevel::LinkageOnly,
                context(),
            )
            .unwrap(),
            &adapter,
        )
        .unwrap();

    let lines = read_lines(&export_path);
    assert_eq!(lines.len(), 2);

    let first: Value = serde_json::from_str(&lines[0]).unwrap();
    let second: Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(first["event_type"], "denial_issued");
    assert_eq!(second["event_type"], "execution_status_observed");
    assert_eq!(
        first["forensic_event_id"],
        denial_outcome.event.event.forensic_event_id.to_string()
    );
    assert_eq!(
        second["forensic_event_id"],
        status_outcome.event.event.forensic_event_id.to_string()
    );

    assert_eq!(
        denial_outcome.export_receipt.export_reference,
        denial_outcome.event.event.forensic_event_id.to_string()
    );
    assert_eq!(
        denial_outcome.export_receipt.adapter_id,
        "jsonl-forensic-export"
    );
    assert_eq!(
        status_outcome.event.event.event_type,
        ForensicEventType::ExecutionStatusObserved
    );

    fs::remove_file(&export_path).ok();
}

#[test]
fn direct_adapter_call_reports_unavailable_dependency_for_missing_directory() {
    let missing_dir_path = std::env::temp_dir()
        .join(format!(
            "fa-local-forensic-export-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.jsonl");
    let adapter =
        JsonlForensicExportAdapter::new(JsonlForensicExportAdapterConfig::new(missing_dir_path));

    let event = ForensicService
        .record_event(
            ForensicRecordInput::new(
                ForensicRecordKind::DenialIssued {
                    route_decision: route_decision("route-decision-denied-basic.json"),
                },
                RedactionLevel::SensitiveFieldsRedacted,
                context(),
            )
            .unwrap(),
        )
        .unwrap();

    let result = adapter.export_event(&event);
    assert_eq!(
        result,
        ForensicExportResult::DependencyUnavailable {
            summary: "jsonl forensic export directory is unavailable".to_owned(),
        }
    );
}

#[test]
fn record_and_export_via_service_fails_closed_when_export_sink_is_unavailable() {
    let missing_dir_path = std::env::temp_dir()
        .join(format!(
            "fa-local-forensic-export-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.jsonl");
    let adapter =
        JsonlForensicExportAdapter::new(JsonlForensicExportAdapterConfig::new(missing_dir_path));

    let error = ForensicService
        .record_and_export_event(
            ForensicRecordInput::new(
                ForensicRecordKind::DenialIssued {
                    route_decision: route_decision("route-decision-denied-basic.json"),
                },
                RedactionLevel::SensitiveFieldsRedacted,
                context(),
            )
            .unwrap(),
            &adapter,
        )
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "contract invalid: forensic export dependency unavailable: jsonl forensic export directory is unavailable"
    );
}
