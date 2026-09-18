use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

use fa_local::app::neuronforge_dispatch_pipeline_service::{
    NeuronForgeDispatchPipelineService, NeuronForgeDispatchRunResult,
};
use fa_local::integrations::neuronforge_local::{
    JsonlNeuronForgeForensicExportAdapter, JsonlNeuronForgeForensicExportAdapterConfig,
    ModelResourceDisclosure, NeuronForgeDispatchOutcome, NeuronForgeReceiptValidationStatus,
    NeuronForgeTaskDeliveryAdapter, NeuronForgeTaskDispatchRequest, NeuronForgeTaskDispatchResult,
};

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 10, 0).unwrap()
}

fn request() -> NeuronForgeTaskDispatchRequest {
    NeuronForgeTaskDispatchRequest {
        dispatch_id: "dispatch-001".to_owned(),
        request_id: "request-001".to_owned(),
        task_id: "analyze.style.scene.v1".to_owned(),
        scene_text: "A quiet scene.".to_owned(),
        model_resource_disclosure: ModelResourceDisclosure {
            route_class: "WORKHORSE_LOCAL".to_owned(),
            model_id: "qwen2.5:14b".to_owned(),
            resource_budget_class: "workhorse_local".to_owned(),
            execution_mode: "local_model".to_owned(),
        },
        operator_visible_message: "test dispatch".to_owned(),
    }
}

struct StubAdapter(NeuronForgeTaskDispatchResult);

impl NeuronForgeTaskDeliveryAdapter for StubAdapter {
    fn adapter_id(&self) -> &'static str {
        "stub-neuronforge-adapter"
    }

    fn dispatch_task(
        &self,
        _request: &NeuronForgeTaskDispatchRequest,
    ) -> NeuronForgeTaskDispatchResult {
        self.0.clone()
    }
}

fn temp_jsonl_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-neuronforge-pipeline-export-{}.jsonl",
        Uuid::new_v4()
    ))
}

#[test]
fn a_completed_dispatch_records_one_completed_event() {
    let adapter = StubAdapter(NeuronForgeTaskDispatchResult::Completed {
        receipt: json!({ "schema_validation_status": "valid" }),
    });

    let NeuronForgeDispatchRunResult {
        outcome,
        forensic_record,
    } = NeuronForgeDispatchPipelineService
        .run(&request(), &adapter, None, ts())
        .unwrap();

    assert!(matches!(
        outcome,
        NeuronForgeTaskDispatchResult::Completed { .. }
    ));
    assert_eq!(
        forensic_record.event.event.outcome,
        NeuronForgeDispatchOutcome::Completed
    );
    assert_eq!(
        forensic_record.event.event.receipt_validation_status,
        Some(NeuronForgeReceiptValidationStatus::Valid)
    );
    assert_eq!(forensic_record.export_reference, None);
}

#[test]
fn a_not_completed_dispatch_derives_receipt_validation_status_from_the_receipt() {
    let adapter = StubAdapter(NeuronForgeTaskDispatchResult::NotCompleted {
        receipt: json!({ "schema_validation_status": "degraded" }),
    });

    let result = NeuronForgeDispatchPipelineService
        .run(&request(), &adapter, None, ts())
        .unwrap();

    assert_eq!(
        result.forensic_record.event.event.outcome,
        NeuronForgeDispatchOutcome::NotCompleted
    );
    assert_eq!(
        result.forensic_record.event.event.receipt_validation_status,
        Some(NeuronForgeReceiptValidationStatus::Degraded)
    );
}

#[test]
fn a_dispatch_unavailable_outcome_records_an_event_with_no_receipt_status() {
    let adapter = StubAdapter(NeuronForgeTaskDispatchResult::DispatchUnavailable {
        summary: "could not reach NeuronForge Local".to_owned(),
    });

    let result = NeuronForgeDispatchPipelineService
        .run(&request(), &adapter, None, ts())
        .unwrap();

    assert_eq!(
        result.forensic_record.event.event.outcome,
        NeuronForgeDispatchOutcome::DispatchUnavailable
    );
    assert_eq!(
        result.forensic_record.event.event.receipt_validation_status,
        None
    );
    assert_eq!(
        result.forensic_record.event.event.summary,
        "could not reach NeuronForge Local"
    );
}

#[test]
fn a_supplied_export_adapter_exports_the_recorded_event_and_populates_export_reference() {
    let adapter = StubAdapter(NeuronForgeTaskDispatchResult::Completed {
        receipt: json!({ "schema_validation_status": "valid" }),
    });
    let export_path = temp_jsonl_path();
    let export_adapter = JsonlNeuronForgeForensicExportAdapter::new(
        JsonlNeuronForgeForensicExportAdapterConfig::new(export_path.clone()),
    );

    let result = NeuronForgeDispatchPipelineService
        .run(&request(), &adapter, Some(&export_adapter), ts())
        .unwrap();

    assert_eq!(
        result.forensic_record.export_reference.as_deref(),
        Some(
            result
                .forensic_record
                .event
                .event
                .forensic_event_id
                .to_string()
                .as_str()
        )
    );

    let exported_lines = fs::read_to_string(&export_path).unwrap().lines().count();
    assert_eq!(exported_lines, 1);

    fs::remove_file(&export_path).ok();
}

#[test]
fn a_run_fails_closed_when_the_export_sink_is_unavailable() {
    let adapter = StubAdapter(NeuronForgeTaskDispatchResult::Completed {
        receipt: json!({ "schema_validation_status": "valid" }),
    });
    let missing_dir_path = std::env::temp_dir()
        .join(format!(
            "fa-local-neuronforge-pipeline-export-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.jsonl");
    let export_adapter = JsonlNeuronForgeForensicExportAdapter::new(
        JsonlNeuronForgeForensicExportAdapterConfig::new(missing_dir_path),
    );

    let error = NeuronForgeDispatchPipelineService
        .run(&request(), &adapter, Some(&export_adapter), ts())
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("forensic export dependency unavailable")
    );
}

#[test]
fn a_not_completed_receipt_with_an_unrecognized_status_is_a_hard_error() {
    let adapter = StubAdapter(NeuronForgeTaskDispatchResult::NotCompleted {
        receipt: json!({ "schema_validation_status": "something_unexpected" }),
    });

    let error = NeuronForgeDispatchPipelineService
        .run(&request(), &adapter, None, ts())
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unrecognized schema_validation_status")
    );
}

#[test]
fn a_long_dispatch_unavailable_summary_is_truncated_and_still_validates() {
    // Longer than 160 bytes once bounded_summary's trailing ellipsis (3
    // UTF-8 bytes) is appended, but exactly 160 Unicode codepoints --
    // reproduces a real live-testing failure (KI-FLO-20260918-005): a
    // real ureq connection-refused message is well over 160 chars, and
    // validate() used to check byte length against a string bounded_summary
    // had already truncated by char count, rejecting its own output.
    let adapter = StubAdapter(NeuronForgeTaskDispatchResult::DispatchUnavailable {
        summary: "x".repeat(300),
    });

    let result = NeuronForgeDispatchPipelineService
        .run(&request(), &adapter, None, ts())
        .unwrap();

    let summary = &result.forensic_record.event.event.summary;
    assert_eq!(summary.chars().count(), 160);
    assert!(summary.ends_with('…'));
}
