use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::shared::{
    ApprovalPosture, CorrelationId, DegradedSubtype, ExecutionPlanId, ExecutionState,
    ForensicEventId, RequestId, ReviewPackageId, RouteDecisionId, SchemaName, TimestampUtc,
    deserialize_contract_value,
};
use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForensicEventType {
    DenialIssued,
    RouteDecisionResolved,
    ReviewPackagePrepared,
    ExecutionStatusObserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionLevel {
    None,
    SensitiveFieldsRedacted,
    LinkageOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForensicEvent {
    pub forensic_event_id: ForensicEventId,
    pub correlation_id: CorrelationId,
    pub request_id: RequestId,
    pub event_type: ForensicEventType,
    pub route_decision_id: Option<RouteDecisionId>,
    pub execution_plan_id: Option<ExecutionPlanId>,
    pub stable_plan_hash: Option<String>,
    pub review_package_id: Option<ReviewPackageId>,
    pub timestamp_utc: TimestampUtc,
    pub current_posture: ApprovalPosture,
    pub execution_state: ExecutionState,
    pub degraded_subtype: Option<DegradedSubtype>,
    pub summary: String,
    pub redaction_level: RedactionLevel,
    pub payload_minimized: bool,
}

impl ForensicEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        forensic_event_id: ForensicEventId,
        correlation_id: CorrelationId,
        request_id: RequestId,
        event_type: ForensicEventType,
        route_decision_id: Option<RouteDecisionId>,
        execution_plan_id: Option<ExecutionPlanId>,
        stable_plan_hash: Option<String>,
        review_package_id: Option<ReviewPackageId>,
        timestamp_utc: TimestampUtc,
        current_posture: ApprovalPosture,
        execution_state: ExecutionState,
        degraded_subtype: Option<DegradedSubtype>,
        summary: String,
        redaction_level: RedactionLevel,
        payload_minimized: bool,
    ) -> FaLocalResult<Self> {
        let event = Self {
            forensic_event_id,
            correlation_id,
            request_id,
            event_type,
            route_decision_id,
            execution_plan_id,
            stable_plan_hash,
            review_package_id,
            timestamp_utc,
            current_posture,
            execution_state,
            degraded_subtype,
            summary,
            redaction_level,
            payload_minimized,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        deserialize_contract_value(SchemaName::ForensicEvent, value)
    }

    pub fn validate(&self) -> FaLocalResult<()> {
        validate_required_summary(&self.summary)?;
        validate_optional_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
        validate_review_package_linkage(
            self.route_decision_id,
            self.execution_plan_id,
            self.stable_plan_hash.as_deref(),
            self.review_package_id,
        )?;
        validate_posture_state_pair(self.current_posture, self.execution_state)?;
        validate_degraded_subtype(self.event_type, self.execution_state, self.degraded_subtype)?;

        if !self.payload_minimized {
            return Err(contract_invalid(
                "forensic event payload_minimized must remain true for bounded forensics",
            ));
        }

        if let Some(stable_plan_hash) = self.stable_plan_hash.as_deref() {
            if !is_valid_hash(stable_plan_hash) {
                return Err(contract_invalid(
                    "forensic event stable_plan_hash must be a 64-character lowercase hex digest",
                ));
            }
        }

        if mentions_semantic_or_workflow_narration(&self.summary) {
            return Err(contract_invalid(
                "forensic event summary must not narrate planner, workflow, or semantic interpretation",
            ));
        }

        match self.event_type {
            ForensicEventType::DenialIssued => {
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::Denied],
                    "denial_issued forensic event must use denied posture",
                )?;
                require_state(
                    self.execution_state,
                    &[ExecutionState::Denied],
                    "denial_issued forensic event must use denied execution_state",
                )?;
                require_none(
                    self.route_decision_id,
                    "denial_issued forensic event must not include route_decision_id",
                )?;
                require_none(
                    self.execution_plan_id,
                    "denial_issued forensic event must not include execution_plan_id",
                )?;
                require_none_ref(
                    self.stable_plan_hash.as_deref(),
                    "denial_issued forensic event must not include stable_plan_hash",
                )?;
                require_none(
                    self.review_package_id,
                    "denial_issued forensic event must not include review_package_id",
                )?;
            }
            ForensicEventType::RouteDecisionResolved => {
                require_some(
                    self.route_decision_id,
                    "route_decision_resolved forensic event must include route_decision_id",
                )?;
                require_none(
                    self.execution_plan_id,
                    "route_decision_resolved forensic event must not include execution_plan_id",
                )?;
                require_none_ref(
                    self.stable_plan_hash.as_deref(),
                    "route_decision_resolved forensic event must not include stable_plan_hash",
                )?;
                require_none(
                    self.review_package_id,
                    "route_decision_resolved forensic event must not include review_package_id",
                )?;
                require_state(
                    self.execution_state,
                    &[
                        ExecutionState::Denied,
                        ExecutionState::ReviewRequired,
                        ExecutionState::WaitingExplicitApproval,
                        ExecutionState::AdmittedNotStarted,
                    ],
                    "route_decision_resolved forensic event must remain pre-execution",
                )?;
            }
            ForensicEventType::ReviewPackagePrepared => {
                require_some(
                    self.route_decision_id,
                    "review_package_prepared forensic event must include route_decision_id",
                )?;
                require_some(
                    self.execution_plan_id,
                    "review_package_prepared forensic event must include execution_plan_id",
                )?;
                require_some_ref(
                    self.stable_plan_hash.as_deref(),
                    "review_package_prepared forensic event must include stable_plan_hash",
                )?;
                require_some(
                    self.review_package_id,
                    "review_package_prepared forensic event must include review_package_id",
                )?;
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::ExplicitOperatorApproval],
                    "review_package_prepared forensic event must use explicit_operator_approval posture",
                )?;
                require_state(
                    self.execution_state,
                    &[ExecutionState::WaitingExplicitApproval],
                    "review_package_prepared forensic event must use waiting_explicit_approval execution_state",
                )?;
            }
            ForensicEventType::ExecutionStatusObserved => {
                require_some(
                    self.execution_plan_id,
                    "execution_status_observed forensic event must include execution_plan_id",
                )?;
                require_some_ref(
                    self.stable_plan_hash.as_deref(),
                    "execution_status_observed forensic event must include stable_plan_hash",
                )?;
                require_state(
                    self.execution_state,
                    &[
                        ExecutionState::AdmittedNotStarted,
                        ExecutionState::InProgress,
                        ExecutionState::Degraded,
                        ExecutionState::PartialSuccess,
                        ExecutionState::CompletedWithConstraints,
                        ExecutionState::Completed,
                        ExecutionState::Failed,
                        ExecutionState::Canceled,
                    ],
                    "execution_status_observed forensic event must use an admitted execution_state",
                )?;
                require_posture(
                    self.current_posture,
                    &[
                        ApprovalPosture::PolicyPreapproved,
                        ApprovalPosture::ExecuteAllowed,
                    ],
                    "execution_status_observed forensic event must use policy_preapproved or execute_allowed posture",
                )?;
            }
        }

        Ok(())
    }

    pub fn validated(self) -> FaLocalResult<ValidatedForensicEvent> {
        self.validate()?;
        Ok(ValidatedForensicEvent { event: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedForensicEvent {
    pub event: ForensicEvent,
}

impl ValidatedForensicEvent {
    pub fn new(event: ForensicEvent) -> FaLocalResult<Self> {
        event.validated()
    }
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}

fn validate_required_summary(summary: &str) -> FaLocalResult<()> {
    if summary.is_empty() || summary.len() > 160 {
        return Err(contract_invalid(
            "forensic event summary must be between 1 and 160 characters",
        ));
    }
    Ok(())
}

fn validate_optional_plan_link(
    execution_plan_id: Option<ExecutionPlanId>,
    stable_plan_hash: Option<&str>,
) -> FaLocalResult<()> {
    match (execution_plan_id, stable_plan_hash) {
        (Some(_), Some(_)) | (None, None) => Ok(()),
        _ => Err(contract_invalid(
            "forensic event execution_plan_id and stable_plan_hash must be present together or absent together",
        )),
    }
}

fn validate_review_package_linkage(
    route_decision_id: Option<RouteDecisionId>,
    execution_plan_id: Option<ExecutionPlanId>,
    stable_plan_hash: Option<&str>,
    review_package_id: Option<ReviewPackageId>,
) -> FaLocalResult<()> {
    if review_package_id.is_some()
        && (route_decision_id.is_none()
            || execution_plan_id.is_none()
            || stable_plan_hash.is_none())
    {
        return Err(contract_invalid(
            "forensic event review_package_id requires route_decision_id, execution_plan_id, and stable_plan_hash",
        ));
    }
    Ok(())
}

fn validate_posture_state_pair(
    current_posture: ApprovalPosture,
    execution_state: ExecutionState,
) -> FaLocalResult<()> {
    match execution_state {
        ExecutionState::Denied => require_posture(
            current_posture,
            &[ApprovalPosture::Denied],
            "denied forensic event state must use denied posture",
        ),
        ExecutionState::ReviewRequired => require_posture(
            current_posture,
            &[ApprovalPosture::ReviewRequired],
            "review_required forensic event state must use review_required posture",
        ),
        ExecutionState::WaitingExplicitApproval => require_posture(
            current_posture,
            &[ApprovalPosture::ExplicitOperatorApproval],
            "waiting_explicit_approval forensic event state must use explicit_operator_approval posture",
        ),
        ExecutionState::AdmittedNotStarted
        | ExecutionState::InProgress
        | ExecutionState::Degraded
        | ExecutionState::PartialSuccess
        | ExecutionState::CompletedWithConstraints
        | ExecutionState::Completed
        | ExecutionState::Failed
        | ExecutionState::Canceled => require_posture(
            current_posture,
            &[
                ApprovalPosture::PolicyPreapproved,
                ApprovalPosture::ExecuteAllowed,
            ],
            "admitted forensic event states must use policy_preapproved or execute_allowed posture",
        ),
    }
}

fn validate_degraded_subtype(
    event_type: ForensicEventType,
    execution_state: ExecutionState,
    degraded_subtype: Option<DegradedSubtype>,
) -> FaLocalResult<()> {
    match event_type {
        ForensicEventType::ReviewPackagePrepared => {
            if let Some(subtype) = degraded_subtype {
                if !matches!(
                    subtype,
                    DegradedSubtype::DegradedFallbackEquivalent
                        | DegradedSubtype::DegradedFallbackLimited
                ) {
                    return Err(contract_invalid(
                        "review_package_prepared forensic event may only declare explicit fallback degraded_subtype values",
                    ));
                }
            }
            Ok(())
        }
        _ => match execution_state {
            ExecutionState::Degraded => require_some(
                degraded_subtype,
                "degraded forensic event must include degraded_subtype",
            )
            .map(|_| ()),
            ExecutionState::PartialSuccess => require_some(
                degraded_subtype,
                "partial_success forensic event must include degraded_subtype",
            )
            .map(|_| ()),
            ExecutionState::CompletedWithConstraints => {
                let degraded_subtype = require_some(
                    degraded_subtype,
                    "completed_with_constraints forensic event must include an explicit fallback degraded_subtype",
                )?;
                if !is_explicit_fallback_subtype(Some(degraded_subtype)) {
                    return Err(contract_invalid(
                        "completed_with_constraints forensic event must include an explicit fallback degraded_subtype",
                    ));
                }
                Ok(())
            }
            _ => {
                if degraded_subtype.is_some() {
                    return Err(contract_invalid(
                        "forensic event execution_state must not include degraded_subtype",
                    ));
                }
                Ok(())
            }
        },
    }
}

fn require_posture(
    posture: ApprovalPosture,
    allowed: &[ApprovalPosture],
    message: &'static str,
) -> FaLocalResult<()> {
    if allowed.contains(&posture) {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn require_state(
    state: ExecutionState,
    allowed: &[ExecutionState],
    message: &'static str,
) -> FaLocalResult<()> {
    if allowed.contains(&state) {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn require_none<T>(value: Option<T>, message: &'static str) -> FaLocalResult<()> {
    if value.is_none() {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn require_none_ref<T: ?Sized>(value: Option<&T>, message: &'static str) -> FaLocalResult<()> {
    if value.is_none() {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn require_some<T>(value: Option<T>, message: &'static str) -> FaLocalResult<T> {
    value.ok_or_else(|| contract_invalid(message))
}

fn require_some_ref<'a, T: ?Sized>(
    value: Option<&'a T>,
    message: &'static str,
) -> FaLocalResult<&'a T> {
    value.ok_or_else(|| contract_invalid(message))
}

fn is_valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_explicit_fallback_subtype(value: Option<DegradedSubtype>) -> bool {
    matches!(
        value,
        Some(
            DegradedSubtype::DegradedFallbackEquivalent | DegradedSubtype::DegradedFallbackLimited
        )
    )
}

fn mentions_semantic_or_workflow_narration(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("planner")
        || lower.contains("workflow")
        || lower.contains("semantic")
        || lower.contains("next step")
}
