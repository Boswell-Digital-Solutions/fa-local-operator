//! Composes Cortex Gnat admission negotiation with real shard dispatch.
//!
//! [`GnatDispatchValidator::negotiate`] only ever decides whether FA Local
//! *admits* a Cortex-initiated Gnat run; on its own it has no onward path
//! to actually running a shard. [`GnatDispatchPipelineService::run`] is
//! that onward path: build a full runnable request for every declared
//! shard (see [`GnatShardDispatchRequest::from_declared_shard`]), negotiate,
//! and only when the negotiated posture actually admits FA-Local-owned
//! dispatch, deliver every shard through a [`GnatShardDeliveryAdapter`] and
//! collect its real outcome.
//!
//! One thing this does *not* yet do, disclosed rather than silently assumed
//! away: **no forensic recording.**
//! [`ForensicRecordKind`](crate::app::forensic_service::ForensicRecordKind)
//! is built entirely around FA Local's own `RouteDecision`/`ExecutionStatus`
//! domain, which a Cortex-initiated Gnat run has no equivalent of. Giving
//! Gnat dispatch runs the same truthful, append-only forensic trail every
//! other admitted path gets needs its own forensic-event contract
//! extension, not a bolt-on to this pipeline.

use std::collections::HashMap;

use crate::domain::guards::DenialGuard;
use crate::errors::{FaLocalError, FaLocalResult};
use crate::integrations::cortex::{
    GnatDispatchAdmission, GnatDispatchAdmissionState, GnatDispatchEnvelope, GnatDispatchValidator,
    GnatFaLocalCapabilityState, GnatShardDeliveryAdapter, GnatShardDispatchRequest,
    GnatShardDispatchResult, GnatShardEnrichment,
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

#[derive(Debug, Default)]
pub struct GnatDispatchPipelineService;

impl GnatDispatchPipelineService {
    pub fn run(
        &self,
        envelope: &GnatDispatchEnvelope,
        fa_local_capabilities: &GnatFaLocalCapabilityState,
        shard_enrichments: &HashMap<String, GnatShardEnrichment>,
        adapter: &dyn GnatShardDeliveryAdapter,
    ) -> FaLocalResult<GnatDispatchRunOutcome> {
        let shard_requests = build_shard_requests(envelope, shard_enrichments)?;

        let admission = match GnatDispatchValidator::negotiate(envelope, fa_local_capabilities) {
            Ok(admission) => admission,
            Err(denial) => return Ok(GnatDispatchRunOutcome::Denied(denial)),
        };

        match admission.state {
            GnatDispatchAdmissionState::SerialFallbackPermitted => {
                Ok(GnatDispatchRunOutcome::SerialFallbackPermitted(admission))
            }
            GnatDispatchAdmissionState::ReadyForFaLocalDispatch => {
                let shard_results = shard_requests
                    .iter()
                    .map(|request| (request.shard_id.clone(), adapter.deliver_shard(request)))
                    .collect();
                Ok(GnatDispatchRunOutcome::Dispatched {
                    admission,
                    shard_results,
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
