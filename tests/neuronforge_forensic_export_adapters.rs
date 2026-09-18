use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde_json::Value;
use uuid::Uuid;

use fa_local::adapters::exports::ForensicExportResult;
use fa_local::domain::shared::ForensicEventId;
use fa_local::integrations::neuronforge_local::{
    JsonlNeuronForgeForensicExportAdapter, JsonlNeuronForgeForensicExportAdapterConfig,
    NeuronForgeDispatchOutcome, NeuronForgeForensicEventExportAdapter,
    NeuronForgeForensicRedactionLevel, NeuronForgeReceiptValidationStatus,
    NeuronForgeTaskDispatchForensicEvent, SqliteNeuronForgeForensicStore,
};

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 10, 0).unwrap()
}

fn completed_event()
-> fa_local::integrations::neuronforge_local::ValidatedNeuronForgeTaskDispatchForensicEvent {
    NeuronForgeTaskDispatchForensicEvent::new(
        ForensicEventId::new(),
        "dispatch-fixture-001".to_owned(),
        "request-fixture-001".to_owned(),
        "analyze.style.scene.v1".to_owned(),
        NeuronForgeDispatchOutcome::Completed,
        Some(NeuronForgeReceiptValidationStatus::Valid),
        ts(),
        "NeuronForge Local dispatch completed".to_owned(),
        NeuronForgeForensicRedactionLevel::LinkageOnly,
        true,
    )
    .unwrap()
    .validated()
    .unwrap()
}

fn temp_jsonl_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-neuronforge-forensic-export-{}.jsonl",
        Uuid::new_v4()
    ))
}

fn temp_sqlite_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-neuronforge-forensic-store-{}.sqlite3",
        Uuid::new_v4()
    ))
}

#[test]
fn jsonl_adapter_appends_one_line_per_event() {
    let export_path = temp_jsonl_path();
    let adapter = JsonlNeuronForgeForensicExportAdapter::new(
        JsonlNeuronForgeForensicExportAdapterConfig::new(export_path.clone()),
    );

    let event = completed_event();
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
    assert_eq!(parsed["outcome"], "completed");
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
            "fa-local-neuronforge-forensic-export-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.jsonl");
    let adapter = JsonlNeuronForgeForensicExportAdapter::new(
        JsonlNeuronForgeForensicExportAdapterConfig::new(missing_dir_path),
    );

    let result = adapter.export_event(&completed_event());
    assert_eq!(
        result,
        ForensicExportResult::DependencyUnavailable {
            summary: "jsonl neuronforge forensic export directory is unavailable".to_owned(),
        }
    );
}

#[test]
fn sqlite_store_writes_and_round_trips_an_event() {
    let store_path = temp_sqlite_path();
    let store = SqliteNeuronForgeForensicStore::open(&store_path).unwrap();

    let event = completed_event();
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
            "fa-local-neuronforge-forensic-store-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.sqlite3");

    let error = SqliteNeuronForgeForensicStore::open(&missing_dir_path).unwrap_err();
    assert!(!error.to_string().is_empty());
}
