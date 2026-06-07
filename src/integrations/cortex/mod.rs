use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::guards::{DenialGuard, deny};
use crate::domain::shared::{
    CorrelationId, DenialBasis, DenialReasonClass, DenialScope, SchemaName,
    deserialize_contract_value,
};
use crate::errors::FaLocalResult;

#[derive(Debug, Default)]
pub struct CortexAdapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatWorkerType {
    MarkdownSyntax,
    PlainTextSyntax,
    PdfTextSyntax,
    DocxTextSyntax,
    RtfTextSyntax,
    OdtTextSyntax,
    EpubTextSyntax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatCancellationPolicy {
    CancelRemainingShards,
    FinishInFlightOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatRetryPolicy {
    None,
    InfrastructureOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GnatFaLocalState {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GnatDispatchEnvelope {
    pub contract_version: String,
    pub correlation_id: CorrelationId,
    pub requester_service: String,
    pub plan: GnatDispatchPlan,
    pub required_capabilities: GnatRequiredCapabilities,
    pub route_policy: GnatRoutePolicy,
}

impl GnatDispatchEnvelope {
    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        deserialize_contract_value(SchemaName::GnatDispatchEnvelope, value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GnatDispatchPlan {
    pub contract_version: String,
    pub run_id: String,
    pub plan_hash: String,
    pub operation: String,
    pub expected_receipt_schema: String,
    pub shard_count: u16,
    pub shards: Vec<GnatDispatchShard>,
    pub execution_limits: GnatExecutionLimits,
    pub serial_fallback_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GnatDispatchShard {
    pub shard_id: String,
    pub ordinal: u16,
    pub worker_type: GnatWorkerType,
    pub source_ref: String,
    pub source_fingerprint_digest: String,
    pub deadline_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GnatExecutionLimits {
    pub requested_concurrency: u8,
    pub max_concurrency: u8,
    pub deadline_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GnatRequiredCapabilities {
    pub supported_contract_versions: Vec<String>,
    pub worker_types: Vec<GnatWorkerType>,
    pub cancellation_policy: GnatCancellationPolicy,
    pub deadline_enforced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GnatRoutePolicy {
    pub fa_local_owns_execution_routing: bool,
    pub cortex_validates_receipts: bool,
    pub retry_policy: GnatRetryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnatFaLocalCapabilityState {
    pub fa_local_state: GnatFaLocalState,
    pub supported_contract_versions: Vec<String>,
    pub admitted_worker_types: Vec<GnatWorkerType>,
    pub max_concurrency: u8,
    pub cancellation_supported: bool,
}

impl GnatFaLocalCapabilityState {
    pub fn ready_default() -> Self {
        Self {
            fa_local_state: GnatFaLocalState::Ready,
            supported_contract_versions: vec![
                "GnatDispatchEnvelope.v1".to_owned(),
                "GnatRunPlan.v1".to_owned(),
                "GnatWorkerReceipt.v1".to_owned(),
            ],
            admitted_worker_types: vec![
                GnatWorkerType::MarkdownSyntax,
                GnatWorkerType::PlainTextSyntax,
                GnatWorkerType::PdfTextSyntax,
                GnatWorkerType::DocxTextSyntax,
                GnatWorkerType::RtfTextSyntax,
                GnatWorkerType::OdtTextSyntax,
                GnatWorkerType::EpubTextSyntax,
            ],
            max_concurrency: 4,
            cancellation_supported: true,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            fa_local_state: GnatFaLocalState::Unavailable,
            supported_contract_versions: Vec::new(),
            admitted_worker_types: Vec::new(),
            max_concurrency: 0,
            cancellation_supported: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GnatDispatchAdmissionState {
    ReadyForFaLocalDispatch,
    SerialFallbackPermitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnatDispatchAdmission {
    pub state: GnatDispatchAdmissionState,
    pub correlation_id: CorrelationId,
    pub run_id: String,
    pub effective_concurrency: u8,
    pub admitted_worker_types: Vec<GnatWorkerType>,
    pub cancellation_policy: GnatCancellationPolicy,
    pub operator_visible_summary: String,
}

#[derive(Debug, Default)]
pub struct GnatDispatchValidator;

impl GnatDispatchValidator {
    pub fn negotiate(
        envelope: &GnatDispatchEnvelope,
        fa_local_capabilities: &GnatFaLocalCapabilityState,
    ) -> Result<GnatDispatchAdmission, DenialGuard> {
        validate_plan_surface(envelope)?;

        match fa_local_capabilities.fa_local_state {
            GnatFaLocalState::Ready => {
                validate_supported_contracts(envelope, fa_local_capabilities)?;
                validate_worker_admission(envelope, fa_local_capabilities)?;
                validate_cancellation(envelope, fa_local_capabilities)?;

                let effective_concurrency = envelope
                    .plan
                    .execution_limits
                    .requested_concurrency
                    .min(envelope.plan.execution_limits.max_concurrency)
                    .min(fa_local_capabilities.max_concurrency)
                    .max(1);
                Ok(GnatDispatchAdmission {
                    state: GnatDispatchAdmissionState::ReadyForFaLocalDispatch,
                    correlation_id: envelope.correlation_id,
                    run_id: envelope.plan.run_id.clone(),
                    effective_concurrency,
                    admitted_worker_types: sorted_worker_types(
                        &envelope.required_capabilities.worker_types,
                    ),
                    cancellation_policy: envelope.required_capabilities.cancellation_policy,
                    operator_visible_summary: format!(
                        "FA Local admitted Cortex Gnat run {} for bounded dispatch.",
                        envelope.plan.run_id
                    ),
                })
            }
            GnatFaLocalState::Degraded | GnatFaLocalState::Unavailable => {
                if envelope.plan.serial_fallback_allowed {
                    Ok(GnatDispatchAdmission {
                        state: GnatDispatchAdmissionState::SerialFallbackPermitted,
                        correlation_id: envelope.correlation_id,
                        run_id: envelope.plan.run_id.clone(),
                        effective_concurrency: 1,
                        admitted_worker_types: Vec::new(),
                        cancellation_policy: envelope.required_capabilities.cancellation_policy,
                        operator_visible_summary: "FA Local Gnat dispatch is unavailable; Cortex serial fallback is permitted by contract.".to_owned(),
                    })
                } else {
                    Err(deny(
                        DenialReasonClass::DependencyUnavailable,
                        DenialScope::Service,
                        DenialBasis::RuntimeSafety,
                        "FA Local Gnat dispatch is unavailable and serial fallback is not permitted",
                    ))
                }
            }
        }
    }
}

fn validate_plan_surface(envelope: &GnatDispatchEnvelope) -> Result<(), DenialGuard> {
    if envelope.contract_version != "GnatDispatchEnvelope.v1" {
        return Err(contract_invalid(
            "unsupported Gnat dispatch envelope version",
        ));
    }
    if envelope.requester_service != "cortex" {
        return Err(contract_invalid(
            "Gnat dispatch envelope requester must be cortex",
        ));
    }
    if envelope.plan.contract_version != "GnatRunPlan.v1" {
        return Err(contract_invalid("unsupported Gnat run plan version"));
    }
    if envelope.plan.operation != "syntax_extract" {
        return Err(contract_invalid("Gnat dispatch admits syntax_extract only"));
    }
    if envelope.plan.expected_receipt_schema != "gnat-worker-receipt.schema.json" {
        return Err(contract_invalid(
            "Gnat dispatch expected receipt schema mismatch",
        ));
    }
    if envelope.plan.shard_count as usize != envelope.plan.shards.len() {
        return Err(contract_invalid(
            "Gnat dispatch shard_count does not match shards",
        ));
    }
    if !envelope.route_policy.fa_local_owns_execution_routing {
        return Err(contract_invalid("FA Local must own Gnat execution routing"));
    }
    if !envelope.route_policy.cortex_validates_receipts {
        return Err(contract_invalid("Cortex must validate Gnat receipts"));
    }
    if envelope.route_policy.retry_policy != GnatRetryPolicy::InfrastructureOnly {
        return Err(contract_invalid(
            "Gnat dispatch admits infrastructure-only retry policy",
        ));
    }
    if !envelope.required_capabilities.deadline_enforced {
        return Err(contract_invalid(
            "Gnat dispatch requires enforced deadlines",
        ));
    }

    let declared_workers = envelope
        .required_capabilities
        .worker_types
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for shard in &envelope.plan.shards {
        if !declared_workers.contains(&shard.worker_type) {
            return Err(contract_invalid(
                "Gnat dispatch shard uses undeclared worker type",
            ));
        }
    }

    Ok(())
}

fn validate_supported_contracts(
    envelope: &GnatDispatchEnvelope,
    fa_local_capabilities: &GnatFaLocalCapabilityState,
) -> Result<(), DenialGuard> {
    let supported = fa_local_capabilities
        .supported_contract_versions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    for required in &envelope.required_capabilities.supported_contract_versions {
        if !supported.contains(required.as_str()) {
            return Err(contract_invalid(
                "FA Local does not support a required Gnat contract version",
            ));
        }
    }
    Ok(())
}

fn validate_worker_admission(
    envelope: &GnatDispatchEnvelope,
    fa_local_capabilities: &GnatFaLocalCapabilityState,
) -> Result<(), DenialGuard> {
    let admitted = fa_local_capabilities
        .admitted_worker_types
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    for worker_type in &envelope.required_capabilities.worker_types {
        if !admitted.contains(worker_type) {
            return Err(deny(
                DenialReasonClass::CapabilityNotAdmitted,
                DenialScope::Capability,
                DenialBasis::Policy,
                "Gnat worker type is not admitted by FA Local",
            ));
        }
    }
    Ok(())
}

fn validate_cancellation(
    _envelope: &GnatDispatchEnvelope,
    fa_local_capabilities: &GnatFaLocalCapabilityState,
) -> Result<(), DenialGuard> {
    if !fa_local_capabilities.cancellation_supported {
        return Err(deny(
            DenialReasonClass::DependencyUnavailable,
            DenialScope::Service,
            DenialBasis::RuntimeSafety,
            "FA Local Gnat cancellation support is unavailable",
        ));
    }
    Ok(())
}

fn sorted_worker_types(worker_types: &[GnatWorkerType]) -> Vec<GnatWorkerType> {
    worker_types
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn contract_invalid(summary: &'static str) -> DenialGuard {
    deny(
        DenialReasonClass::ContractInvalid,
        DenialScope::Operation,
        DenialBasis::Contract,
        summary,
    )
}
