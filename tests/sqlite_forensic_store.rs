mod support;

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use fa_local::CorrelationId;
use fa_local::adapters::exports::ForensicEventExportAdapter;
use fa_local::adapters::exports::sqlite_forensic_store::SqliteForensicStore;
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
    ForensicRecordContext::new(ts(2030, 1, 1, 0, 50, 0))
}

fn temp_db_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-forensic-store-{}.sqlite3",
        Uuid::new_v4()
    ))
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

#[test]
fn writes_and_queries_events_by_correlation_id_in_timestamp_order() {
    let db_path = temp_db_path();
    let store = SqliteForensicStore::open(&db_path).unwrap();

    let denial_route = route_decision("route-decision-denied-basic.json");
    let correlation_id = denial_route.correlation_id;

    let denial_outcome = ForensicService
        .record_and_export_event(
            ForensicRecordInput::new(
                ForensicRecordKind::DenialIssued {
                    route_decision: denial_route,
                },
                RedactionLevel::SensitiveFieldsRedacted,
                context(),
            )
            .unwrap(),
            &store,
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
            &store,
        )
        .unwrap();

    let events = store.query_by_correlation_id(correlation_id).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].forensic_event_id,
        denial_outcome.event.event.forensic_event_id
    );
    assert_eq!(
        events[1].forensic_event_id,
        status_outcome.event.event.forensic_event_id
    );
    assert_eq!(events[0].event_type, ForensicEventType::DenialIssued);
    assert_eq!(
        events[1].event_type,
        ForensicEventType::ExecutionStatusObserved
    );
    assert_eq!(events[0].summary, denial_outcome.event.event.summary);

    for event in &events {
        assert_eq!(event.correlation_id, correlation_id);
    }
}

#[test]
fn queries_events_by_event_type() {
    let db_path = temp_db_path();
    let store = SqliteForensicStore::open(&db_path).unwrap();

    ForensicService
        .record_and_export_event(
            ForensicRecordInput::new(
                ForensicRecordKind::DenialIssued {
                    route_decision: route_decision("route-decision-denied-basic.json"),
                },
                RedactionLevel::SensitiveFieldsRedacted,
                context(),
            )
            .unwrap(),
            &store,
        )
        .unwrap();

    ForensicService
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
            &store,
        )
        .unwrap();

    let denials = store
        .query_by_event_type(ForensicEventType::DenialIssued)
        .unwrap();
    assert_eq!(denials.len(), 1);
    assert_eq!(denials[0].event_type, ForensicEventType::DenialIssued);

    let statuses = store
        .query_by_event_type(ForensicEventType::ExecutionStatusObserved)
        .unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(
        statuses[0].event_type,
        ForensicEventType::ExecutionStatusObserved
    );
}

#[test]
fn unrelated_correlation_id_returns_no_events() {
    let db_path = temp_db_path();
    let store = SqliteForensicStore::open(&db_path).unwrap();

    ForensicService
        .record_and_export_event(
            ForensicRecordInput::new(
                ForensicRecordKind::DenialIssued {
                    route_decision: route_decision("route-decision-denied-basic.json"),
                },
                RedactionLevel::SensitiveFieldsRedacted,
                context(),
            )
            .unwrap(),
            &store,
        )
        .unwrap();

    let events = store.query_by_correlation_id(CorrelationId::new()).unwrap();
    assert!(events.is_empty());
}

#[test]
fn opening_the_store_under_a_missing_parent_directory_fails_closed() {
    let missing_dir_path = std::env::temp_dir()
        .join(format!(
            "fa-local-forensic-store-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.sqlite3");

    let error = SqliteForensicStore::open(&missing_dir_path).unwrap_err();
    assert!(!error.to_string().is_empty());
}

#[test]
fn export_event_reports_a_bounded_export_reference_equal_to_the_event_id() {
    let db_path = temp_db_path();
    let store = SqliteForensicStore::open(&db_path).unwrap();

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

    let result = store.export_event(&event);
    assert_eq!(
        result,
        fa_local::adapters::exports::ForensicExportResult::Exported {
            export_reference: event.event.forensic_event_id.to_string(),
        }
    );
}
