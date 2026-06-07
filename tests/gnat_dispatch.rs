mod support;

use fa_local::domain::guards::DenialGuard;
use fa_local::integrations::cortex::{
    GnatDispatchAdmissionState, GnatDispatchEnvelope, GnatDispatchValidator,
    GnatFaLocalCapabilityState, GnatWorkerType,
};
use fa_local::{DenialReasonClass, DenialScope};

fn load_basic_envelope() -> GnatDispatchEnvelope {
    let value = support::load_fixture_json("valid", "gnat-dispatch-envelope-basic.json");
    GnatDispatchEnvelope::load_contract_value(&value).unwrap()
}

fn assert_denied(error: &DenialGuard, reason_class: DenialReasonClass, scope: DenialScope) {
    assert_eq!(error.reason_class, reason_class);
    assert_eq!(error.scope, scope);
}

#[test]
fn ready_fa_local_admits_gnat_dispatch_and_clamps_concurrency() {
    let mut envelope = load_basic_envelope();
    envelope.plan.execution_limits.requested_concurrency = 8;
    envelope.plan.execution_limits.max_concurrency = 8;
    let mut capabilities = GnatFaLocalCapabilityState::ready_default();
    capabilities.max_concurrency = 3;

    let admission = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap();

    assert_eq!(
        admission.state,
        GnatDispatchAdmissionState::ReadyForFaLocalDispatch
    );
    assert_eq!(admission.effective_concurrency, 3);
    assert_eq!(
        admission.admitted_worker_types,
        vec![
            GnatWorkerType::MarkdownSyntax,
            GnatWorkerType::PlainTextSyntax
        ]
    );
    assert!(
        admission
            .operator_visible_summary
            .contains("bounded dispatch")
    );
}

#[test]
fn unavailable_fa_local_reports_serial_fallback_when_contract_permits_it() {
    let envelope = load_basic_envelope();
    let capabilities = GnatFaLocalCapabilityState::unavailable();

    let admission = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap();

    assert_eq!(
        admission.state,
        GnatDispatchAdmissionState::SerialFallbackPermitted
    );
    assert_eq!(admission.effective_concurrency, 1);
    assert!(
        admission
            .operator_visible_summary
            .contains("serial fallback")
    );
}

#[test]
fn unavailable_fa_local_denies_when_serial_fallback_is_not_permitted() {
    let mut envelope = load_basic_envelope();
    envelope.plan.serial_fallback_allowed = false;
    let capabilities = GnatFaLocalCapabilityState::unavailable();

    let error = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap_err();

    assert_denied(
        &error,
        DenialReasonClass::DependencyUnavailable,
        DenialScope::Service,
    );
}

#[test]
fn worker_type_not_admitted_by_fa_local_denies_dispatch() {
    let envelope = load_basic_envelope();
    let mut capabilities = GnatFaLocalCapabilityState::ready_default();
    capabilities.admitted_worker_types = vec![GnatWorkerType::MarkdownSyntax];

    let error = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap_err();

    assert_denied(
        &error,
        DenialReasonClass::CapabilityNotAdmitted,
        DenialScope::Capability,
    );
}

#[test]
fn shard_count_mismatch_is_contract_invalid() {
    let mut envelope = load_basic_envelope();
    envelope.plan.shard_count = 1;
    let capabilities = GnatFaLocalCapabilityState::ready_default();

    let error = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap_err();

    assert_denied(
        &error,
        DenialReasonClass::ContractInvalid,
        DenialScope::Operation,
    );
    assert!(error.summary.contains("shard_count"));
}

#[test]
fn cortex_cannot_claim_execution_routing_in_gnat_dispatch() {
    let mut envelope = load_basic_envelope();
    envelope.route_policy.fa_local_owns_execution_routing = false;
    let capabilities = GnatFaLocalCapabilityState::ready_default();

    let error = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap_err();

    assert_denied(
        &error,
        DenialReasonClass::ContractInvalid,
        DenialScope::Operation,
    );
}

#[test]
fn ready_fa_local_admits_pdf_text_worker_when_requested() {
    let mut envelope = load_basic_envelope();
    envelope.required_capabilities.worker_types.push(GnatWorkerType::PdfTextSyntax);
    envelope.plan.shards.push(fa_local::integrations::cortex::GnatDispatchShard {
        shard_id: "gnat-run-fixture-001-shard-0002".to_owned(),
        ordinal: 2,
        worker_type: GnatWorkerType::PdfTextSyntax,
        source_ref: "pdf-note".to_owned(),
        source_fingerprint_digest: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
        deadline_ms: 30000,
    });
    envelope.plan.shard_count = 3;
    let capabilities = GnatFaLocalCapabilityState::ready_default();

    let admission = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap();

    assert!(admission.admitted_worker_types.contains(&GnatWorkerType::PdfTextSyntax));
}

#[test]
fn ready_fa_local_admits_docx_text_worker_when_requested() {
    let mut envelope = load_basic_envelope();
    envelope.required_capabilities.worker_types.push(GnatWorkerType::DocxTextSyntax);
    envelope.plan.shards.push(fa_local::integrations::cortex::GnatDispatchShard {
        shard_id: "gnat-run-fixture-001-shard-0002".to_owned(),
        ordinal: 2,
        worker_type: GnatWorkerType::DocxTextSyntax,
        source_ref: "docx-note".to_owned(),
        source_fingerprint_digest: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned(),
        deadline_ms: 30000,
    });
    envelope.plan.shard_count = 3;
    let capabilities = GnatFaLocalCapabilityState::ready_default();

    let admission = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap();

    assert!(admission.admitted_worker_types.contains(&GnatWorkerType::DocxTextSyntax));
}

#[test]
fn ready_fa_local_admits_rtf_text_worker_when_requested() {
    let mut envelope = load_basic_envelope();
    envelope.required_capabilities.worker_types.push(GnatWorkerType::RtfTextSyntax);
    envelope.plan.shards.push(fa_local::integrations::cortex::GnatDispatchShard {
        shard_id: "gnat-run-fixture-001-shard-0002".to_owned(),
        ordinal: 2,
        worker_type: GnatWorkerType::RtfTextSyntax,
        source_ref: "rtf-note".to_owned(),
        source_fingerprint_digest: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_owned(),
        deadline_ms: 30000,
    });
    envelope.plan.shard_count = 3;
    let capabilities = GnatFaLocalCapabilityState::ready_default();

    let admission = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap();

    assert!(admission.admitted_worker_types.contains(&GnatWorkerType::RtfTextSyntax));
}

#[test]
fn ready_fa_local_admits_odt_text_worker_when_requested() {
    let mut envelope = load_basic_envelope();
    envelope.required_capabilities.worker_types.push(GnatWorkerType::OdtTextSyntax);
    envelope.plan.shards.push(fa_local::integrations::cortex::GnatDispatchShard {
        shard_id: "gnat-run-fixture-001-shard-0002".to_owned(),
        ordinal: 2,
        worker_type: GnatWorkerType::OdtTextSyntax,
        source_ref: "odt-note".to_owned(),
        source_fingerprint_digest: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned(),
        deadline_ms: 30000,
    });
    envelope.plan.shard_count = 3;
    let capabilities = GnatFaLocalCapabilityState::ready_default();

    let admission = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap();

    assert!(admission.admitted_worker_types.contains(&GnatWorkerType::OdtTextSyntax));
}

#[test]
fn unsupported_contract_version_denies_dispatch() {
    let mut envelope = load_basic_envelope();
    envelope
        .required_capabilities
        .supported_contract_versions
        .push("GnatFutureContract.v9".to_owned());
    let capabilities = GnatFaLocalCapabilityState::ready_default();

    let error = GnatDispatchValidator::negotiate(&envelope, &capabilities).unwrap_err();

    assert_denied(
        &error,
        DenialReasonClass::ContractInvalid,
        DenialScope::Operation,
    );
}
