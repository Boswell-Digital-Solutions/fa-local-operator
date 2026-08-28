use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::SERVICE_ID;
use crate::domain::execution::ExecutionRequest;
use crate::domain::guards::{DenialGuard, deny};
use crate::domain::policy::PolicyArtifact;
use crate::domain::requester_trust::RequesterTrustEnvelope;
use crate::domain::shared::{
    ApprovalPosture, CapabilityId, DenialBasis, DenialReasonClass, DenialScope, RequesterClass,
    RevocationState, SchemaName, SideEffectClass, TimestampUtc, deserialize_contract_value,
};
use crate::errors::FaLocalResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityType {
    LocalFileRead,
    LocalFileWrite,
    LocalDbMutation,
    LocalProcessSpawn,
    GovernedOther,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnabledState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewClass {
    None,
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityProvenanceKind {
    RegistryFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityProvenance {
    pub source_kind: CapabilityProvenanceKind,
    pub issued_at: TimestampUtc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRecord {
    pub capability_id: CapabilityId,
    pub owner_service: String,
    pub capability_type: CapabilityType,
    pub side_effect_class: SideEffectClass,
    pub approval_posture: ApprovalPosture,
    pub allowed_requester_classes: Vec<RequesterClass>,
    pub timeout_budget_ms: u64,
    pub retry_budget: u32,
    pub max_duration_budget_ms: u64,
    pub max_cpu_budget: Option<u32>,
    pub max_mem_budget_mb: Option<u32>,
    pub enabled_state: EnabledState,
    pub review_class: ReviewClass,
    pub provenance: CapabilityProvenance,
    pub revocation_state: RevocationState,
    pub version_range: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRegistry {
    pub registry_version: String,
    pub capabilities: Vec<CapabilityRecord>,
}

impl CapabilityRegistry {
    pub fn capability_for(&self, capability_id: CapabilityId) -> Option<&CapabilityRecord> {
        self.capabilities
            .iter()
            .find(|entry| entry.capability_id == capability_id)
    }
}

#[derive(Debug, Default)]
pub struct CapabilityRegistryLoader;

impl CapabilityRegistryLoader {
    pub fn load_contract_value(value: &Value) -> FaLocalResult<CapabilityRegistry> {
        deserialize_contract_value(SchemaName::CapabilityRegistry, value)
    }

    pub fn admit_execution_request(
        registry: &CapabilityRegistry,
        policy: &PolicyArtifact,
        requester: &RequesterTrustEnvelope,
        request: &ExecutionRequest,
    ) -> Result<CapabilityRecord, DenialGuard> {
        let capability = registry
            .capability_for(request.requested_capability_id)
            .ok_or_else(|| {
                deny(
                    DenialReasonClass::CapabilityNotAdmitted,
                    DenialScope::Capability,
                    DenialBasis::Policy,
                    "unregistered capability",
                )
            })?;

        if capability.enabled_state == EnabledState::Disabled {
            return Err(deny(
                DenialReasonClass::DisabledByOperator,
                DenialScope::Capability,
                DenialBasis::Policy,
                "capability is disabled",
            ));
        }

        if capability.revocation_state == RevocationState::Revoked {
            return Err(deny(
                DenialReasonClass::CapabilityNotAdmitted,
                DenialScope::Capability,
                DenialBasis::Policy,
                "capability is revoked",
            ));
        }

        if capability.owner_service != SERVICE_ID {
            return Err(deny(
                DenialReasonClass::CapabilityNotAdmitted,
                DenialScope::Capability,
                DenialBasis::Policy,
                "capability owner does not match fa-local",
            ));
        }

        if !capability
            .allowed_requester_classes
            .contains(&requester.requester_class)
        {
            return Err(deny(
                DenialReasonClass::CapabilityNotAdmitted,
                DenialScope::Capability,
                DenialBasis::Policy,
                "requester class is not admitted for capability",
            ));
        }

        if capability.side_effect_class != request.requested_side_effect_class {
            return Err(deny(
                DenialReasonClass::PolicyDenied,
                DenialScope::Capability,
                DenialBasis::ContractAndPolicy,
                "policy/capability mismatch: requested side effect does not match capability",
            ));
        }

        let policy_rule = policy
            .capability_rule_for(capability.capability_id)
            .ok_or_else(|| {
                deny(
                    DenialReasonClass::PolicyDenied,
                    DenialScope::Capability,
                    DenialBasis::ContractAndPolicy,
                    "policy/capability mismatch: capability missing from policy",
                )
            })?;

        if !policy_rule.allowed {
            return Err(deny(
                DenialReasonClass::PolicyDenied,
                DenialScope::Capability,
                DenialBasis::Policy,
                "policy denies capability",
            ));
        }

        if !policy
            .scope
            .environment_modes
            .contains(&request.environment_mode)
            || !policy
                .environment_conditions
                .contains(&request.environment_mode)
        {
            return Err(deny(
                DenialReasonClass::PolicyDenied,
                DenialScope::Capability,
                DenialBasis::Policy,
                "policy does not admit this environment",
            ));
        }

        if !policy_rule
            .allowed_requester_classes
            .contains(&requester.requester_class)
        {
            return Err(deny(
                DenialReasonClass::PolicyDenied,
                DenialScope::Capability,
                DenialBasis::Policy,
                "policy/capability mismatch: requester class not allowed",
            ));
        }

        if !policy_rule
            .allowed_side_effect_classes
            .contains(&request.requested_side_effect_class)
        {
            return Err(deny(
                DenialReasonClass::PolicyDenied,
                DenialScope::Capability,
                DenialBasis::Policy,
                "policy/capability mismatch: side effect class not allowed",
            ));
        }

        Ok(capability.clone())
    }
}
