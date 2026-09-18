mod support;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::{TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

use fa_local::app::gnat_dispatch_pipeline_service::{
    GnatDispatchPipelineService, GnatDispatchRunOutcome,
};
use fa_local::integrations::cortex::{
    GnatDispatchAdmissionState, GnatDispatchEnvelope, GnatFaLocalCapabilityState,
    GnatForensicEventType, GnatNegotiationOutcome, GnatReceiptState, GnatShardDeliveryAdapter,
    GnatShardDispatchRequest, GnatShardDispatchResult, GnatShardEnrichment, GnatShardOutcome,
    JsonlGnatForensicExportAdapter, JsonlGnatForensicExportAdapterConfig,
};

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2030, 1, 1, 0, 10, 0).unwrap()
}

fn load_basic_envelope() -> GnatDispatchEnvelope {
    let value = support::load_fixture_json("valid", "gnat-dispatch-envelope-basic.json");
    GnatDispatchEnvelope::load_contract_value(&value).unwrap()
}

fn enrichment(local_path: &str) -> GnatShardEnrichment {
    GnatShardEnrichment {
        source_path_token: "token".to_owned(),
        media_type: "text/markdown".to_owned(),
        fingerprint_algorithm: "sha256".to_owned(),
        fingerprint_byte_count: 42,
        fingerprint_modified_at: "2030-01-01T00:00:00Z".to_owned(),
        max_bytes: 20 * 1024 * 1024,
        local_path: local_path.into(),
    }
}

/// The basic fixture declares exactly these two shards.
fn basic_enrichments() -> HashMap<String, GnatShardEnrichment> {
    HashMap::from([
        (
            "gnat-run-fixture-001-shard-0000".to_owned(),
            enrichment("/tmp/unused-chapter-01"),
        ),
        (
            "gnat-run-fixture-001-shard-0001".to_owned(),
            enrichment("/tmp/unused-note-plain"),
        ),
    ])
}

/// Stub adapter reporting `Completed` for every shard, counting calls, and
/// recording every request it actually received so tests can assert on
/// exactly what the bridge built.
#[derive(Default)]
struct StubGnatShardAdapter {
    calls: AtomicUsize,
    received: Mutex<Vec<GnatShardDispatchRequest>>,
}

impl GnatShardDeliveryAdapter for StubGnatShardAdapter {
    fn adapter_id(&self) -> &'static str {
        "stub-gnat-shard-adapter"
    }

    fn deliver_shard(&self, request: &GnatShardDispatchRequest) -> GnatShardDispatchResult {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.received.lock().unwrap().push(request.clone());
        GnatShardDispatchResult::Completed {
            receipt: json!({"state": "complete", "shard_id": request.shard_id}),
        }
    }
}

#[test]
fn a_ready_run_dispatches_every_declared_shard() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_enrichments = basic_enrichments();
    let adapter = StubGnatShardAdapter::default();

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    match result.outcome {
        GnatDispatchRunOutcome::Dispatched {
            admission,
            shard_results,
        } => {
            assert_eq!(
                admission.state,
                GnatDispatchAdmissionState::ReadyForFaLocalDispatch
            );
            assert_eq!(shard_results.len(), 2);
            for (_, result) in &shard_results {
                assert!(matches!(result, GnatShardDispatchResult::Completed { .. }));
            }
        }
        other => panic!("expected Dispatched, got {other:?}"),
    }
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_dispatched_run_records_one_negotiation_event_and_one_event_per_shard() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_enrichments = basic_enrichments();
    let adapter = StubGnatShardAdapter::default();

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    assert_eq!(result.forensic_events.len(), 3);

    let negotiation = &result.forensic_events[0].event.event;
    assert_eq!(
        negotiation.event_type,
        GnatForensicEventType::GnatDispatchNegotiated
    );
    assert_eq!(
        negotiation.negotiation_outcome,
        GnatNegotiationOutcome::ReadyForFaLocalDispatch
    );
    assert!(negotiation.shard_id.is_none());

    for shard_event in &result.forensic_events[1..] {
        let event = &shard_event.event.event;
        assert_eq!(event.event_type, GnatForensicEventType::GnatShardDispatched);
        assert_eq!(event.shard_outcome, Some(GnatShardOutcome::Completed));
        assert_eq!(event.receipt_state, Some(GnatReceiptState::Complete));
        assert!(event.shard_id.is_some());
    }
}

#[test]
fn the_bridge_merges_declared_envelope_fields_with_enrichment_fields() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_enrichments = basic_enrichments();
    let adapter = StubGnatShardAdapter::default();

    GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    let received = adapter.received.lock().unwrap();
    let request = received
        .iter()
        .find(|request| request.shard_id == "gnat-run-fixture-001-shard-0000")
        .expect("adapter received the first declared shard");

    // Fields the envelope itself declares come from the envelope alone.
    assert_eq!(request.run_id, "gnat-run-fixture-001");
    assert_eq!(request.ordinal, 0);
    assert_eq!(
        request.worker_type,
        fa_local::integrations::cortex::GnatWorkerType::MarkdownSyntax
    );
    assert_eq!(request.source_ref, "chapter-01");
    assert_eq!(request.deadline_ms, 30000);
    assert_eq!(
        request.source_fingerprint.digest,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );

    // Fields only the enrichment supplies.
    assert_eq!(request.source_path_token, "token");
    assert_eq!(request.media_type, "text/markdown");
    assert_eq!(request.max_bytes, 20 * 1024 * 1024);
    assert_eq!(
        request.local_path.to_str().unwrap(),
        "/tmp/unused-chapter-01"
    );
    assert_eq!(request.source_fingerprint.algorithm, "sha256");
    assert_eq!(request.source_fingerprint.byte_count, 42);
}

#[test]
fn an_unavailable_run_with_serial_fallback_never_dispatches() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::unavailable();
    let shard_enrichments = basic_enrichments();
    let adapter = StubGnatShardAdapter::default();

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        GnatDispatchRunOutcome::SerialFallbackPermitted(_)
    ));
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);

    // Exactly one negotiation event, no shard events -- Cortex's own job in
    // this case, not this pipeline's.
    assert_eq!(result.forensic_events.len(), 1);
    assert_eq!(
        result.forensic_events[0].event.event.negotiation_outcome,
        GnatNegotiationOutcome::SerialFallbackPermitted
    );
}

#[test]
fn a_denied_run_never_dispatches() {
    let mut envelope = load_basic_envelope();
    envelope.plan.serial_fallback_allowed = false;
    let capabilities = GnatFaLocalCapabilityState::unavailable();
    let shard_enrichments = basic_enrichments();
    let adapter = StubGnatShardAdapter::default();

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    assert!(matches!(result.outcome, GnatDispatchRunOutcome::Denied(_)));
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);

    assert_eq!(result.forensic_events.len(), 1);
    assert_eq!(
        result.forensic_events[0].event.event.negotiation_outcome,
        GnatNegotiationOutcome::Denied
    );
}

#[test]
fn a_declared_shard_with_no_enrichment_supplied_is_rejected_before_negotiation() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let mut shard_enrichments = basic_enrichments();
    shard_enrichments.remove("gnat-run-fixture-001-shard-0001");
    let adapter = StubGnatShardAdapter::default();

    let error = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap_err();

    assert!(error.to_string().contains("no shard enrichment supplied"));
    assert!(
        error
            .to_string()
            .contains("gnat-run-fixture-001-shard-0001")
    );
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn an_enrichment_for_an_undeclared_shard_is_simply_ignored() {
    // The bridge only ever looks up enrichments by the envelope's own
    // declared shard ids -- an extra, unrelated entry in the map is neither
    // an error nor dispatched.
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let mut shard_enrichments = basic_enrichments();
    shard_enrichments.insert(
        "some-undeclared-shard-id".to_owned(),
        enrichment("/tmp/unused-extra"),
    );
    let adapter = StubGnatShardAdapter::default();

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        GnatDispatchRunOutcome::Dispatched { .. }
    ));
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 2);
}

/// Adapter reporting a truthful non-completion for every shard, so tests
/// can assert the forensic event correctly reflects a `NotCompleted`
/// outcome (as opposed to the always-`Completed` `StubGnatShardAdapter`).
struct NotCompletedGnatShardAdapter;

impl GnatShardDeliveryAdapter for NotCompletedGnatShardAdapter {
    fn adapter_id(&self) -> &'static str {
        "not-completed-gnat-shard-adapter"
    }

    fn deliver_shard(&self, request: &GnatShardDispatchRequest) -> GnatShardDispatchResult {
        GnatShardDispatchResult::NotCompleted {
            receipt: json!({"state": "stale", "shard_id": request.shard_id}),
        }
    }
}

#[test]
fn a_not_completed_shard_records_its_real_receipt_state() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_enrichments = basic_enrichments();
    let adapter = NotCompletedGnatShardAdapter;

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    for shard_event in &result.forensic_events[1..] {
        let event = &shard_event.event.event;
        assert_eq!(event.shard_outcome, Some(GnatShardOutcome::NotCompleted));
        assert_eq!(event.receipt_state, Some(GnatReceiptState::Stale));
    }
}

/// Adapter reporting the shard's capability as entirely unreachable.
struct UnavailableGnatShardAdapter;

impl GnatShardDeliveryAdapter for UnavailableGnatShardAdapter {
    fn adapter_id(&self) -> &'static str {
        "unavailable-gnat-shard-adapter"
    }

    fn deliver_shard(&self, _request: &GnatShardDispatchRequest) -> GnatShardDispatchResult {
        GnatShardDispatchResult::DispatchUnavailable {
            summary: "no delivery adapter registered for this shard".to_owned(),
        }
    }
}

#[test]
fn a_dispatch_unavailable_shard_records_no_receipt_state() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_enrichments = basic_enrichments();
    let adapter = UnavailableGnatShardAdapter;

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            None,
            ts(),
        )
        .unwrap();

    for shard_event in &result.forensic_events[1..] {
        let event = &shard_event.event.event;
        assert_eq!(
            event.shard_outcome,
            Some(GnatShardOutcome::DispatchUnavailable)
        );
        assert_eq!(event.receipt_state, None);
    }
}

fn temp_jsonl_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fa-local-gnat-dispatch-pipeline-export-{}.jsonl",
        Uuid::new_v4()
    ))
}

#[test]
fn a_supplied_export_adapter_exports_every_recorded_event_and_populates_export_reference() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_enrichments = basic_enrichments();
    let adapter = StubGnatShardAdapter::default();
    let export_path = temp_jsonl_path();
    let export_adapter = JsonlGnatForensicExportAdapter::new(
        JsonlGnatForensicExportAdapterConfig::new(export_path.clone()),
    );

    let result = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            Some(&export_adapter),
            ts(),
        )
        .unwrap();

    assert_eq!(result.forensic_events.len(), 3);
    for record in &result.forensic_events {
        assert_eq!(
            record.export_reference.as_deref(),
            Some(record.event.event.forensic_event_id.to_string().as_str())
        );
    }

    let exported_lines = fs::read_to_string(&export_path).unwrap().lines().count();
    assert_eq!(exported_lines, 3);

    fs::remove_file(&export_path).ok();
}

#[test]
fn a_run_fails_closed_when_the_export_sink_is_unavailable() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_enrichments = basic_enrichments();
    let adapter = StubGnatShardAdapter::default();
    let missing_dir_path = std::env::temp_dir()
        .join(format!(
            "fa-local-gnat-dispatch-pipeline-export-missing-{}",
            Uuid::new_v4()
        ))
        .join("events.jsonl");
    let export_adapter = JsonlGnatForensicExportAdapter::new(
        JsonlGnatForensicExportAdapterConfig::new(missing_dir_path),
    );

    let error = GnatDispatchPipelineService
        .run(
            &envelope,
            &capabilities,
            &shard_enrichments,
            &adapter,
            Some(&export_adapter),
            ts(),
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("forensic export dependency unavailable")
    );
    // Fails closed on the very first event (negotiation) before any shard is dispatched.
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
}
