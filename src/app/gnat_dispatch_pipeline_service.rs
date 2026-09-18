//! Composes Cortex Gnat admission negotiation with real shard dispatch.
//!
//! [`GnatDispatchValidator::negotiate`] only ever decides whether FA Local
//! *admits* a Cortex-initiated Gnat run; on its own it has no onward path
//! to actually running a shard. [`GnatDispatchPipelineService::run`] is
//! that onward path: negotiate, and only when the negotiated posture
//! actually admits FA-Local-owned dispatch, deliver every declared shard
//! through a [`GnatShardDeliveryAdapter`] and collect its real outcome.
//!
//! Two things this does *not* yet do, both disclosed rather than silently
//! assumed away:
//!
//! - **No forensic recording.** [`ForensicRecordKind`](crate::app::forensic_service::ForensicRecordKind)
//!   is built entirely around FA Local's own `RouteDecision`/`ExecutionStatus`
//!   domain, which a Cortex-initiated Gnat run has no equivalent of. Giving
//!   Gnat dispatch runs the same truthful, append-only forensic trail every
//!   other admitted path gets needs its own forensic-event contract
//!   extension, not a bolt-on to this pipeline.
//! - **No negotiation-to-dispatch bridge.** `GnatDispatchShard` (the
//!   envelope's own embedded shard summary) lacks `source_path_token`,
//!   `media_type`, `max_bytes`, and `local_path` -- everything
//!   [`GnatShardDispatchRequest`] needs beyond what negotiation alone ever
//!   sees. This pipeline requires the caller to supply the full descriptor
//!   for every declared shard directly, and only checks that those
//!   descriptors are consistent with what the envelope actually declared
//!   (same shard ids, worker types, and source refs) -- it does not derive
//!   one from the other.

use crate::domain::guards::DenialGuard;
use crate::errors::{FaLocalError, FaLocalResult};
use crate::integrations::cortex::{
    GnatDispatchAdmission, GnatDispatchAdmissionState, GnatDispatchEnvelope, GnatDispatchValidator,
    GnatFaLocalCapabilityState, GnatShardDeliveryAdapter, GnatShardDispatchRequest,
    GnatShardDispatchResult,
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
        shard_requests: &[GnatShardDispatchRequest],
        adapter: &dyn GnatShardDeliveryAdapter,
    ) -> FaLocalResult<GnatDispatchRunOutcome> {
        validate_shard_requests_match_envelope(envelope, shard_requests)?;

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

/// Checks that `shard_requests` is exactly the set of shards `envelope`
/// declares, agreeing on `run_id`, `shard_id`, `worker_type`, and
/// `source_ref` -- the fields negotiation itself already reasoned about.
/// This is a consistency check, not a derivation: it never fills in a
/// missing descriptor, only refuses a mismatched one.
fn validate_shard_requests_match_envelope(
    envelope: &GnatDispatchEnvelope,
    shard_requests: &[GnatShardDispatchRequest],
) -> FaLocalResult<()> {
    if shard_requests.len() != envelope.plan.shards.len() {
        return Err(FaLocalError::ContractInvalid(format!(
            "Gnat dispatch shard descriptors ({}) do not match the envelope's declared shard count ({})",
            shard_requests.len(),
            envelope.plan.shards.len()
        )));
    }

    for declared in &envelope.plan.shards {
        let Some(request) = shard_requests
            .iter()
            .find(|request| request.shard_id == declared.shard_id)
        else {
            return Err(FaLocalError::ContractInvalid(format!(
                "no shard descriptor supplied for declared shard {}",
                declared.shard_id
            )));
        };

        if request.run_id != envelope.plan.run_id {
            return Err(FaLocalError::ContractInvalid(format!(
                "shard descriptor {} run_id does not match the envelope's plan run_id",
                declared.shard_id
            )));
        }
        if request.worker_type != declared.worker_type {
            return Err(FaLocalError::ContractInvalid(format!(
                "shard descriptor {} worker_type does not match the envelope's declared worker_type",
                declared.shard_id
            )));
        }
        if request.source_ref != declared.source_ref {
            return Err(FaLocalError::ContractInvalid(format!(
                "shard descriptor {} source_ref does not match the envelope's declared source_ref",
                declared.shard_id
            )));
        }
    }

    Ok(())
}
