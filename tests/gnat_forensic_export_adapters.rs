use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde_json::Value;
use uuid::Uuid;

use fa_local::adapters::exports::ForensicExportResult;
use fa_local::domain::shared::{CorrelationId, ForensicEventId};
use fa_local::integrations::cortex::{
    GnatDispatchForensicEvent, GnatForensicEventExportAdapter, GnatForensicEventType,
    GnatForensicRedactionLevel, GnatNegotiationOutcome, JsonlGnatForensicExportAdapter,
    JsonlGnatForensicExportAdapterConfig, SqliteGnatForensicStore,
};

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 10, 0).unwrap()
}

fn negotiation_event() -> fa_local::integrations::cortex::ValidatedGnatDispatchForensicEvent {
    GnatDispatchForensicEvent::new(
        ForensicEventId::new(),
        CorrelationId::new(),
        "gnat-run-fixture-001".to_owned(),
        GnatForensicEventType::GnatDispatchNegotiated,
        GnatNegotiationOutcome::ReadyForFaLocalDispatch,
        None,
        None,
        None,
        None,
        ts(),
        "negotiation admitted for FA-Local dispatch".to_owned(),
        GnatForensicRedactionLevel::LinkageOnly,
        true,
    )
    .unwrap()
    .validated()
    .unwrap()
}

fn temp_jsonl_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-gnat-forensic-export-{}.jsonl",
        Uuid::new_v4()
    ))
}

fn temp_sqlite_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-gnat-forensic-store-{}.sqlite3",
        Uuid::new_v4()
    ))
}

#[test]
fn jsonl_adapter_appends_one_line_per_event() {
    let export_path = temp_jsonl_path();
    let adapter = JsonlGnatForensicExportAdapter::new(JsonlGnatForensicExportAdapterConfig::new(
        export_path.clone(),
    ));

    let event = negotiation_event();
    let result = adapter.export_event(&event);

    assert_eq!(
        result,
        ForensicExportResult::Exported {
            export_reference: event.event.forensic_event_id.to_string(),
        }
    );

    let lines: Vec<String> = fs::read_to_string(&export_path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(lines.len(), 1);

    let parsed: Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(parsed["event_type"], "gnat_dispatch_negotiated");
    assert_eq!(
        parsed["forensic_event_id"],
        event.event.forensic_event_id.to_string()
    );

    fs::remove_file(&export_path).ok();
}

#[test]
fn jsonl_adapter_reports_unavailable_dependency_for_missing_directory() {
    let missing_dir_path = std::env::temp_dir()
        .join(format!(
            "fa-local-gnat-forensic-export-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.jsonl");
    let adapter = JsonlGnatForensicExportAdapter::new(JsonlGnatForensicExportAdapterConfig::new(
        missing_dir_path,
    ));

    let result = adapter.export_event(&negotiation_event());
    assert_eq!(
        result,
        ForensicExportResult::DependencyUnavailable {
            summary: "jsonl gnat forensic export directory is unavailable".to_owned(),
        }
    );
}

#[test]
fn sqlite_store_writes_and_round_trips_an_event() {
    let store_path = temp_sqlite_path();
    let store = SqliteGnatForensicStore::open(&store_path).unwrap();

    let event = negotiation_event();
    let result = store.export_event(&event);

    assert_eq!(
        result,
        ForensicExportResult::Exported {
            export_reference: event.event.forensic_event_id.to_string(),
        }
    );

    fs::remove_file(&store_path).ok();
}

#[test]
fn sqlite_store_fails_closed_under_a_missing_parent_directory() {
    let missing_dir_path = std::env::temp_dir()
        .join(format!(
            "fa-local-gnat-forensic-store-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.sqlite3");

    let error = SqliteGnatForensicStore::open(&missing_dir_path).unwrap_err();
    assert!(!error.to_string().is_empty());
}
