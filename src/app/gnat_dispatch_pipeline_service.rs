//! Composes Cortex Gnat admission negotiation with real shard dispatch and
//! truthful forensic recording.
//!
//! [`GnatDispatchValidator::negotiate`] only ever decides whether FA Local
//! *admits* a Cortex-initiated Gnat run; on its own it has no onward path
//! to actually running a shard. [`GnatDispatchPipelineService::run`] is
//! that onward path: build a full runnable request for every declared
//! shard (see [`GnatShardDispatchRequest::from_declared_shard`]), negotiate,
//! and only when the negotiated posture actually admits FA-Local-owned
//! dispatch, deliver every shard through a [`GnatShardDeliveryAdapter`] and
//! collect its real outcome. Every outcome -- denied, serial-fallback,
//! ready, and each shard's own result -- is recorded as a
//! [`GnatDispatchForensicEvent`](crate::integrations::cortex::GnatDispatchForensicEvent),
//! FA Local's own execution-request forensic contract having no equivalent
//! of a Cortex-initiated run to record it as.
//!
//! Recording is currently in-memory only: [`GnatDispatchRunResult::forensic_events`]
//! is returned for the caller to inspect or persist, the same way
//! `execute`'s forensic records existed before this repo's JSONL/SQLite
//! export sinks did. Wiring a matching export sink for this event family
//! is a disclosed, separate concern, not something this pipeline does.

use std::collections::HashMap;

use serde_json::Value;

use crate::domain::guards::DenialGuard;
use crate::domain::shared::{ForensicEventId, TimestampUtc};
use crate::errors::{FaLocalError, FaLocalResult};
use crate::integrations::cortex::{
    GnatDispatchAdmission, GnatDispatchAdmissionState, GnatDispatchEnvelope,
    GnatDispatchForensicEvent, GnatDispatchValidator, GnatFaLocalCapabilityState,
    GnatForensicEventType, GnatForensicRedactionLevel, GnatNegotiationOutcome, GnatReceiptState,
    GnatShardDeliveryAdapter, GnatShardDispatchRequest, GnatShardDispatchResult,
    GnatShardEnrichment, GnatShardOutcome, ValidatedGnatDispatchForensicEvent,
};

/// The full result of one Gnat dispatch run.
#[derive(Debug)]
pub enum GnatDispatchRunOutcome {
    /// Negotiation itself denied the run; no shard was ever dispatched.
    Denied(DenialGuard),
    /// FA Local's Gnat dispatch is unavailable, but the plan's own declared
    /// `serial_fallback_allowed` permits Cortex to run the shards itself,
    /// in-process, through its own serial runner -- that is Cortex's job,
    /// not this pipeline's (`DECISIONS/0019`), so no shard is dispatched
    /// here either.
    SerialFallbackPermitted(GnatDispatchAdmission),
    /// The run was admitted for FA-Local-owned dispatch, and every declared
    /// shard was delivered; each result is exactly what the adapter
    /// reported, in declared order, whether or not it completed.
    Dispatched {
        admission: GnatDispatchAdmission,
        shard_results: Vec<(String, GnatShardDispatchResult)>,
    },
}

/// [`GnatDispatchRunOutcome`] plus the truthful forensic trail recorded
/// along the way: exactly one negotiation event, and (only for a
/// [`GnatDispatchRunOutcome::Dispatched`] run) one event per declared
/// shard, in declared order.
#[derive(Debug)]
pub struct GnatDispatchRunResult {
    pub outcome: GnatDispatchRunOutcome,
    pub forensic_events: Vec<ValidatedGnatDispatchForensicEvent>,
}

#[derive(Debug, Default)]
pub struct GnatDispatchPipelineService;

impl GnatDispatchPipelineService {
    pub fn run(
        &self,
        envelope: &GnatDispatchEnvelope,
        fa_local_capabilities: &GnatFaLocalCapabilityState,
        shard_enrichments: &HashMap<String, GnatShardEnrichment>,
        adapter: &dyn GnatShardDeliveryAdapter,
        now: TimestampUtc,
    ) -> FaLocalResult<GnatDispatchRunResult> {
        let shard_requests = build_shard_requests(envelope, shard_enrichments)?;
        let mut forensic_events = Vec::new();

        let admission = match GnatDispatchValidator::negotiate(envelope, fa_local_capabilities) {
            Ok(admission) => admission,
            Err(denial) => {
                forensic_events.push(build_negotiation_event(
                    envelope,
                    GnatNegotiationOutcome::Denied,
                    denial.summary.clone(),
                    now,
                )?);
                return Ok(GnatDispatchRunResult {
                    outcome: GnatDispatchRunOutcome::Denied(denial),
                    forensic_events,
                });
            }
        };

        match admission.state {
            GnatDispatchAdmissionState::SerialFallbackPermitted => {
                forensic_events.push(build_negotiation_event(
                    envelope,
                    GnatNegotiationOutcome::SerialFallbackPermitted,
                    admission.operator_visible_summary.clone(),
                    now,
                )?);
                Ok(GnatDispatchRunResult {
                    outcome: GnatDispatchRunOutcome::SerialFallbackPermitted(admission),
                    forensic_events,
                })
            }
            GnatDispatchAdmissionState::ReadyForFaLocalDispatch => {
                forensic_events.push(build_negotiation_event(
                    envelope,
                    GnatNegotiationOutcome::ReadyForFaLocalDispatch,
                    admission.operator_visible_summary.clone(),
                    now,
                )?);

                let mut shard_results = Vec::with_capacity(shard_requests.len());
                for request in &shard_requests {
                    let result = adapter.deliver_shard(request);
                    forensic_events.push(build_shard_event(envelope, request, &result, now)?);
                    shard_results.push((request.shard_id.clone(), result));
                }

                Ok(GnatDispatchRunResult {
                    outcome: GnatDispatchRunOutcome::Dispatched {
                        admission,
                        shard_results,
                    },
                    forensic_events,
                })
            }
        }
    }
}

/// Builds a full [`GnatShardDispatchRequest`] for every shard `envelope`
/// declares, merging each with its matching entry in `shard_enrichments`
/// (keyed by `shard_id`). Refuses a declared shard with no enrichment
/// supplied for it; never fills one in.
fn build_shard_requests(
    envelope: &GnatDispatchEnvelope,
    shard_enrichments: &HashMap<String, GnatShardEnrichment>,
) -> FaLocalResult<Vec<GnatShardDispatchRequest>> {
    envelope
        .plan
        .shards
        .iter()
        .map(|declared| {
            let enrichment = shard_enrichments.get(&declared.shard_id).ok_or_else(|| {
                FaLocalError::ContractInvalid(format!(
                    "no shard enrichment supplied for declared shard {}",
                    declared.shard_id
                ))
            })?;
            Ok(GnatShardDispatchRequest::from_declared_shard(
                &envelope.plan.run_id,
                declared,
                enrichment,
            ))
        })
        .collect()
}

fn build_negotiation_event(
    envelope: &GnatDispatchEnvelope,
    negotiation_outcome: GnatNegotiationOutcome,
    summary: String,
    now: TimestampUtc,
) -> FaLocalResult<ValidatedGnatDispatchForensicEvent> {
    GnatDispatchForensicEvent::new(
        ForensicEventId::new(),
        envelope.correlation_id,
        envelope.plan.run_id.clone(),
        GnatForensicEventType::GnatDispatchNegotiated,
        negotiation_outcome,
        None,
        None,
        None,
        None,
        now,
        bounded_summary(summary),
        GnatForensicRedactionLevel::LinkageOnly,
        true,
    )?
    .validated()
}

fn build_shard_event(
    envelope: &GnatDispatchEnvelope,
    request: &GnatShardDispatchRequest,
    result: &GnatShardDispatchResult,
    now: TimestampUtc,
) -> FaLocalResult<ValidatedGnatDispatchForensicEvent> {
    let (shard_outcome, receipt_state) = match result {
        GnatShardDispatchResult::Completed { receipt } => (
            GnatShardOutcome::Completed,
            Some(receipt_state_from_receipt(receipt)?),
        ),
        GnatShardDispatchResult::NotCompleted { receipt } => (
            GnatShardOutcome::NotCompleted,
            Some(receipt_state_from_receipt(receipt)?),
        ),
        GnatShardDispatchResult::DispatchUnavailable { .. } => {
            (GnatShardOutcome::DispatchUnavailable, None)
        }
    };

    GnatDispatchForensicEvent::new(
        ForensicEventId::new(),
        envelope.correlation_id,
        envelope.plan.run_id.clone(),
        GnatForensicEventType::GnatShardDispatched,
        GnatNegotiationOutcome::ReadyForFaLocalDispatch,
        Some(request.shard_id.clone()),
        Some(request.worker_type),
        Some(shard_outcome),
        receipt_state,
        now,
        bounded_summary(shard_event_summary(request, result)),
        GnatForensicRedactionLevel::LinkageOnly,
        true,
    )?
    .validated()
}

fn shard_event_summary(
    request: &GnatShardDispatchRequest,
    result: &GnatShardDispatchResult,
) -> String {
    match result {
        GnatShardDispatchResult::Completed { .. } => {
            format!("Cortex Gnat shard {} completed.", request.shard_id)
        }
        GnatShardDispatchResult::NotCompleted { receipt } => {
            let state = receipt
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            format!(
                "Cortex Gnat shard {} did not complete ({state}).",
                request.shard_id
            )
        }
        GnatShardDispatchResult::DispatchUnavailable { summary } => summary.clone(),
    }
}

fn receipt_state_from_receipt(receipt: &Value) -> FaLocalResult<GnatReceiptState> {
    let state = receipt
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FaLocalError::ContractInvalid("Cortex Gnat receipt missing state field".to_owned())
        })?;
    match state {
        "complete" => Ok(GnatReceiptState::Complete),
        "denied" => Ok(GnatReceiptState::Denied),
        "stale" => Ok(GnatReceiptState::Stale),
        "failed" => Ok(GnatReceiptState::Failed),
        other => Err(FaLocalError::ContractInvalid(format!(
            "Cortex Gnat receipt reported an unrecognized state {other:?}"
        ))),
    }
}

/// Keeps a possibly-unbounded upstream string (an adapter's own
/// `DispatchUnavailable` summary, which can wrap subprocess stderr text)
/// within `GnatDispatchForensicEvent`'s 160-character summary bound,
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
