//! Composes NeuronForge Local task dispatch with truthful forensic
//! recording, the same shape `GnatDispatchPipelineService` gives Cortex Gnat
//! dispatch -- but simpler, since one NeuronForge Local dispatch run has no
//! separate negotiation phase and produces exactly one forensic event, not
//! one negotiation event plus one per shard.
//!
//! Recording is exported through an optional
//! [`NeuronForgeForensicEventExportAdapter`] the same way
//! [`GnatDispatchPipelineService`](crate::app::gnat_dispatch_pipeline_service::GnatDispatchPipelineService)
//! exports its own forensic records: the event is always returned in
//! [`NeuronForgeDispatchRunResult::forensic_record`], carrying an
//! `export_reference` when a sink was supplied. A caller that passes `None`
//! gets in-memory-only recording.

use serde_json::Value;

use crate::app::forensic_service::map_export_result;
use crate::domain::shared::{ForensicEventId, TimestampUtc};
use crate::errors::{FaLocalError, FaLocalResult};
use crate::integrations::neuronforge_local::{
    NeuronForgeDispatchOutcome, NeuronForgeForensicEventExportAdapter,
    NeuronForgeForensicRedactionLevel, NeuronForgeReceiptValidationStatus,
    NeuronForgeTaskDeliveryAdapter, NeuronForgeTaskDispatchForensicEvent,
    NeuronForgeTaskDispatchRequest, NeuronForgeTaskDispatchResult,
    ValidatedNeuronForgeTaskDispatchForensicEvent,
};

/// One recorded [`NeuronForgeTaskDispatchForensicEvent`], plus where it
/// landed: `None` when no export adapter was supplied (in-memory only),
/// `Some(export_reference)` when it was exported.
#[derive(Debug)]
pub struct NeuronForgeForensicRecordOutcome {
    pub event: ValidatedNeuronForgeTaskDispatchForensicEvent,
    pub export_reference: Option<String>,
}

/// The dispatch outcome plus the one forensic event recorded for it.
#[derive(Debug)]
pub struct NeuronForgeDispatchRunResult {
    pub outcome: NeuronForgeTaskDispatchResult,
    pub forensic_record: NeuronForgeForensicRecordOutcome,
}

#[derive(Debug, Default)]
pub struct NeuronForgeDispatchPipelineService;

impl NeuronForgeDispatchPipelineService {
    pub fn run(
        &self,
        request: &NeuronForgeTaskDispatchRequest,
        adapter: &dyn NeuronForgeTaskDeliveryAdapter,
        forensic_export_adapter: Option<&dyn NeuronForgeForensicEventExportAdapter>,
        now: TimestampUtc,
    ) -> FaLocalResult<NeuronForgeDispatchRunResult> {
        let outcome = adapter.dispatch_task(request);
        let event = build_forensic_event(request, &outcome, now)?;
        let forensic_record = record_forensic_event(event, forensic_export_adapter)?;

        Ok(NeuronForgeDispatchRunResult {
            outcome,
            forensic_record,
        })
    }
}

fn record_forensic_event(
    event: ValidatedNeuronForgeTaskDispatchForensicEvent,
    export_adapter: Option<&dyn NeuronForgeForensicEventExportAdapter>,
) -> FaLocalResult<NeuronForgeForensicRecordOutcome> {
    match export_adapter {
        Some(adapter) => {
            let receipt = map_export_result(adapter.adapter_id(), adapter.export_event(&event))?;
            Ok(NeuronForgeForensicRecordOutcome {
                event,
                export_reference: Some(receipt.export_reference),
            })
        }
        None => Ok(NeuronForgeForensicRecordOutcome {
            event,
            export_reference: None,
        }),
    }
}

fn build_forensic_event(
    request: &NeuronForgeTaskDispatchRequest,
    outcome: &NeuronForgeTaskDispatchResult,
    now: TimestampUtc,
) -> FaLocalResult<ValidatedNeuronForgeTaskDispatchForensicEvent> {
    let (dispatch_outcome, receipt_validation_status, summary) = match outcome {
        NeuronForgeTaskDispatchResult::Completed { .. } => (
            NeuronForgeDispatchOutcome::Completed,
            Some(NeuronForgeReceiptValidationStatus::Valid),
            format!(
                "NeuronForge Local dispatch {} completed for task {}.",
                request.dispatch_id, request.task_id
            ),
        ),
        NeuronForgeTaskDispatchResult::NotCompleted { receipt } => {
            let status = receipt_validation_status_from_receipt(receipt)?;
            (
                NeuronForgeDispatchOutcome::NotCompleted,
                Some(status),
                format!(
                    "NeuronForge Local dispatch {} did not complete ({status:?}).",
                    request.dispatch_id
                ),
            )
        }
        NeuronForgeTaskDispatchResult::DispatchUnavailable { summary } => (
            NeuronForgeDispatchOutcome::DispatchUnavailable,
            None,
            summary.clone(),
        ),
    };

    NeuronForgeTaskDispatchForensicEvent::new(
        ForensicEventId::new(),
        request.dispatch_id.clone(),
        request.request_id.clone(),
        request.task_id.clone(),
        dispatch_outcome,
        receipt_validation_status,
        now,
        bounded_summary(summary),
        NeuronForgeForensicRedactionLevel::LinkageOnly,
        true,
    )?
    .validated()
}

fn receipt_validation_status_from_receipt(
    receipt: &Value,
) -> FaLocalResult<NeuronForgeReceiptValidationStatus> {
    let status = receipt
        .get("schema_validation_status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FaLocalError::ContractInvalid(
                "NeuronForge Local receipt missing schema_validation_status field".to_owned(),
            )
        })?;
    match status {
        "degraded" => Ok(NeuronForgeReceiptValidationStatus::Degraded),
        "failed" => Ok(NeuronForgeReceiptValidationStatus::Failed),
        other => Err(FaLocalError::ContractInvalid(format!(
            "NeuronForge Local receipt reported an unrecognized schema_validation_status for a not-completed outcome: {other:?}"
        ))),
    }
}

/// Keeps a possibly-unbounded upstream string within
/// `NeuronForgeTaskDispatchForensicEvent`'s 160-character summary bound,
/// truncating on a character boundary so it never panics on non-ASCII text.
fn bounded_summary(text: String) -> String {
    if text.is_empty() {
        return "(no summary provided)".to_owned();
    }
    if text.chars().count() <= 160 {
        text
    } else {
        let mut truncated: String = text.chars().take(159).collect();
        truncated.push('…');
        truncated
    }
}
