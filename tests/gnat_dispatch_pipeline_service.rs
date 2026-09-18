mod support;

use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

use fa_local::app::gnat_dispatch_pipeline_service::{
    GnatDispatchPipelineService, GnatDispatchRunOutcome,
};
use fa_local::integrations::cortex::{
    GnatDispatchAdmissionState, GnatDispatchEnvelope, GnatFaLocalCapabilityState,
    GnatShardDeliveryAdapter, GnatShardDispatchRequest, GnatShardDispatchResult,
    GnatSourceFingerprint, GnatWorkerType,
};

fn load_basic_envelope() -> GnatDispatchEnvelope {
    let value = support::load_fixture_json("valid", "gnat-dispatch-envelope-basic.json");
    GnatDispatchEnvelope::load_contract_value(&value).unwrap()
}

fn shard_request(
    shard_id: &str,
    worker_type: GnatWorkerType,
    source_ref: &str,
) -> GnatShardDispatchRequest {
    GnatShardDispatchRequest {
        run_id: "gnat-run-fixture-001".to_owned(),
        shard_id: shard_id.to_owned(),
        ordinal: 0,
        worker_type,
        source_ref: source_ref.to_owned(),
        source_path_token: "token".to_owned(),
        media_type: "text/markdown".to_owned(),
        source_fingerprint: GnatSourceFingerprint {
            algorithm: "sha256".to_owned(),
            digest: "a".repeat(64),
            byte_count: 42,
            modified_at: "2030-01-01T00:00:00Z".to_owned(),
        },
        deadline_ms: 30000,
        max_bytes: 20 * 1024 * 1024,
        local_path: "/tmp/unused".into(),
    }
}

fn basic_shard_requests() -> Vec<GnatShardDispatchRequest> {
    vec![
        shard_request(
            "gnat-run-fixture-001-shard-0000",
            GnatWorkerType::MarkdownSyntax,
            "chapter-01",
        ),
        shard_request(
            "gnat-run-fixture-001-shard-0001",
            GnatWorkerType::PlainTextSyntax,
            "note-plain",
        ),
    ]
}

/// Stub adapter reporting `Completed` for every shard and counting how many
/// times it was actually called, so tests can assert dispatch never
/// happened for denied or serial-fallback runs.
#[derive(Default)]
struct StubGnatShardAdapter {
    calls: AtomicUsize,
}

impl GnatShardDeliveryAdapter for StubGnatShardAdapter {
    fn adapter_id(&self) -> &'static str {
        "stub-gnat-shard-adapter"
    }

    fn deliver_shard(&self, request: &GnatShardDispatchRequest) -> GnatShardDispatchResult {
        self.calls.fetch_add(1, Ordering::SeqCst);
        GnatShardDispatchResult::Completed {
            receipt: json!({"state": "complete", "shard_id": request.shard_id}),
        }
    }
}

#[test]
fn a_ready_run_dispatches_every_declared_shard() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_requests = basic_shard_requests();
    let adapter = StubGnatShardAdapter::default();

    let outcome = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap();

    match outcome {
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
fn an_unavailable_run_with_serial_fallback_never_dispatches() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::unavailable();
    let shard_requests = basic_shard_requests();
    let adapter = StubGnatShardAdapter::default();

    let outcome = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap();

    assert!(matches!(
        outcome,
        GnatDispatchRunOutcome::SerialFallbackPermitted(_)
    ));
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_denied_run_never_dispatches() {
    let mut envelope = load_basic_envelope();
    envelope.plan.serial_fallback_allowed = false;
    let capabilities = GnatFaLocalCapabilityState::unavailable();
    let shard_requests = basic_shard_requests();
    let adapter = StubGnatShardAdapter::default();

    let outcome = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap();

    assert!(matches!(outcome, GnatDispatchRunOutcome::Denied(_)));
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mismatched_shard_count_is_rejected_before_negotiation() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let shard_requests = vec![basic_shard_requests().remove(0)];
    let adapter = StubGnatShardAdapter::default();

    let error = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap_err();

    assert!(error.to_string().contains("declared shard count"));
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_shard_descriptor_missing_for_a_declared_shard_is_rejected() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let mut shard_requests = basic_shard_requests();
    shard_requests[1].shard_id = "some-other-shard-id".to_owned();
    let adapter = StubGnatShardAdapter::default();

    let error = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap_err();

    assert!(error.to_string().contains("no shard descriptor supplied"));
}

#[test]
fn a_shard_descriptor_with_the_wrong_worker_type_is_rejected() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let mut shard_requests = basic_shard_requests();
    shard_requests[0].worker_type = GnatWorkerType::PlainTextSyntax;
    let adapter = StubGnatShardAdapter::default();

    let error = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap_err();

    assert!(error.to_string().contains("worker_type does not match"));
}

#[test]
fn a_shard_descriptor_with_the_wrong_source_ref_is_rejected() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let mut shard_requests = basic_shard_requests();
    shard_requests[0].source_ref = "wrong-source".to_owned();
    let adapter = StubGnatShardAdapter::default();

    let error = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap_err();

    assert!(error.to_string().contains("source_ref does not match"));
}

#[test]
fn a_shard_descriptor_with_the_wrong_run_id_is_rejected() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::ready_default();
    let mut shard_requests = basic_shard_requests();
    shard_requests[0].run_id = "some-other-run".to_owned();
    let adapter = StubGnatShardAdapter::default();

    let error = GnatDispatchPipelineService
        .run(&envelope, &capabilities, &shard_requests, &adapter)
        .unwrap_err();

    assert!(error.to_string().contains("run_id does not match"));
}
