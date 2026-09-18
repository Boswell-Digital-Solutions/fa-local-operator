use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::guards::DenialGuard;
use crate::domain::shared::{
    ApprovalPosture, CorrelationId, DegradedSubtype, ExecutionPlanId, ExecutionState,
    ForensicEventId, FrictionPayloadId, RequestId, ReviewPackageId, RouteDecisionId, SchemaName,
    TimestampUtc, deserialize_contract_value,
};
use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrictionKind {
    Denial,
    ReviewRequired,
    ExplicitApprovalRequired,
    ExecutionConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorAction {
    Stop,
    Review,
    ApproveOrDecline,
    Acknowledge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrictionPayload {
    pub friction_payload_id: FrictionPayloadId,
    pub correlation_id: CorrelationId,
    pub request_id: RequestId,
    pub friction_kind: FrictionKind,
    pub operator_action: OperatorAction,
    pub route_decision_id: Option<RouteDecisionId>,
    pub execution_plan_id: Option<ExecutionPlanId>,
    pub stable_plan_hash: Option<String>,
    pub review_package_id: Option<ReviewPackageId>,
    pub forensic_event_id: Option<ForensicEventId>,
    pub current_posture: ApprovalPosture,
    pub execution_state: ExecutionState,
    pub degraded_subtype: Option<DegradedSubtype>,
    pub denial_guards: Vec<DenialGuard>,
    pub operator_visible_summary: String,
    pub payload_minimized: bool,
    pub created_at_utc: TimestampUtc,
}

impl FrictionPayload {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        friction_payload_id: FrictionPayloadId,
        correlation_id: CorrelationId,
        request_id: RequestId,
        friction_kind: FrictionKind,
        operator_action: OperatorAction,
        route_decision_id: Option<RouteDecisionId>,
        execution_plan_id: Option<ExecutionPlanId>,
        stable_plan_hash: Option<String>,
        review_package_id: Option<ReviewPackageId>,
        forensic_event_id: Option<ForensicEventId>,
        current_posture: ApprovalPosture,
        execution_state: ExecutionState,
        degraded_subtype: Option<DegradedSubtype>,
        denial_guards: Vec<DenialGuard>,
        operator_visible_summary: String,
        payload_minimized: bool,
        created_at_utc: TimestampUtc,
    ) -> FaLocalResult<Self> {
        let payload = Self {
            friction_payload_id,
            correlation_id,
            request_id,
            friction_kind,
            operator_action,
            route_decision_id,
            execution_plan_id,
            stable_plan_hash,
            review_package_id,
            forensic_event_id,
            current_posture,
            execution_state,
            degraded_subtype,
            denial_guards,
            operator_visible_summary,
            payload_minimized,
            created_at_utc,
        };
        payload.validate()?;
        Ok(payload)
    }

    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        deserialize_contract_value(SchemaName::FrictionPayload, value)
    }

    pub fn validate(&self) -> FaLocalResult<()> {
        validate_required_summary(&self.operator_visible_summary)?;
        validate_optional_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
        validate_denial_guards(&self.denial_guards)?;
        validate_posture_state_pair(self.current_posture, self.execution_state)?;
        validate_degraded_subtype(self.execution_state, self.degraded_subtype)?;

        if !self.payload_minimized {
            return Err(contract_invalid(
                "friction payload payload_minimized must remain true for bounded friction",
            ));
        }

        if let Some(stable_plan_hash) = self.stable_plan_hash.as_deref() {
            if !is_valid_hash(stable_plan_hash) {
                return Err(contract_invalid(
                    "friction payload stable_plan_hash must be a 64-character lowercase hex digest",
                ));
            }
        }

        if mentions_semantic_or_workflow_narration(&self.operator_visible_summary) {
            return Err(contract_invalid(
                "friction payload summary must not narrate planner, workflow, or semantic interpretation",
            ));
        }

        match self.friction_kind {
            FrictionKind::Denial => {
                require_action(
                    self.operator_action,
                    &[OperatorAction::Stop],
                    "denial friction payload must use stop operator_action",
                )?;
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::Denied],
                    "denial friction payload must use denied posture",
                )?;
                require_state(
                    self.execution_state,
                    &[ExecutionState::Denied],
                    "denial friction payload must use denied execution_state",
                )?;
                require_non_empty_denial_guards(
                    &self.denial_guards,
                    "denial friction payload must include at least one denial guard",
                )?;
                require_none(
                    self.execution_plan_id,
                    "denial friction payload must not include execution_plan_id",
                )?;
                require_none_ref(
                    self.stable_plan_hash.as_deref(),
                    "denial friction payload must not include stable_plan_hash",
                )?;
                require_none(
                    self.review_package_id,
                    "denial friction payload must not include review_package_id",
                )?;
                require_none(
                    self.degraded_subtype,
                    "denial friction payload must not include degraded_subtype",
                )?;
            }
            FrictionKind::ReviewRequired => {
                require_action(
                    self.operator_action,
                    &[OperatorAction::Review],
                    "review_required friction payload must use review operator_action",
                )?;
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::ReviewRequired],
                    "review_required friction payload must use review_required posture",
                )?;
                require_state(
                    self.execution_state,
                    &[ExecutionState::ReviewRequired],
                    "review_required friction payload must use review_required execution_state",
                )?;
                require_some(
                    self.route_decision_id,
                    "review_required friction payload must include route_decision_id",
                )?;
                require_empty_denial_guards(
                    &self.denial_guards,
                    "review_required friction payload must not include denial guards",
                )?;
                require_none(
                    self.execution_plan_id,
                    "review_required friction payload must not include execution_plan_id",
                )?;
                require_none_ref(
                    self.stable_plan_hash.as_deref(),
                    "review_required friction payload must not include stable_plan_hash",
                )?;
                require_none(
                    self.review_package_id,
                    "review_required friction payload must not include review_package_id",
                )?;
                require_none(
                    self.degraded_subtype,
                    "review_required friction payload must not include degraded_subtype",
                )?;
            }
            FrictionKind::ExplicitApprovalRequired => {
                require_action(
                    self.operator_action,
                    &[OperatorAction::ApproveOrDecline],
                    "explicit_approval_required friction payload must use approve_or_decline operator_action",
                )?;
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::ExplicitOperatorApproval],
                    "explicit_approval_required friction payload must use explicit_operator_approval posture",
                )?;
                require_state(
                    self.execution_state,
                    &[ExecutionState::WaitingExplicitApproval],
                    "explicit_approval_required friction payload must use waiting_explicit_approval execution_state",
                )?;
                require_some(
                    self.route_decision_id,
                    "explicit_approval_required friction payload must include route_decision_id",
                )?;
                require_some(
                    self.execution_plan_id,
                    "explicit_approval_required friction payload must include execution_plan_id",
                )?;
                require_some_ref(
                    self.stable_plan_hash.as_deref(),
                    "explicit_approval_required friction payload must include stable_plan_hash",
                )?;
                require_some(
                    self.review_package_id,
                    "explicit_approval_required friction payload must include review_package_id",
                )?;
                require_empty_denial_guards(
                    &self.denial_guards,
                    "explicit_approval_required friction payload must not include denial guards",
                )?;
                require_none(
                    self.degraded_subtype,
                    "explicit_approval_required friction payload must not include degraded_subtype",
                )?;
            }
            FrictionKind::ExecutionConstraint => {
                require_action(
                    self.operator_action,
                    &[OperatorAction::Acknowledge],
                    "execution_constraint friction payload must use acknowledge operator_action",
                )?;
                require_posture(
                    self.current_posture,
                    &[
                        ApprovalPosture::PolicyPreapproved,
                        ApprovalPosture::ExecuteAllowed,
                    ],
                    "execution_constraint friction payload must use policy_preapproved or execute_allowed posture",
                )?;
                require_state(
                    self.execution_state,
                    &[
                        ExecutionState::Degraded,
                        ExecutionState::PartialSuccess,
                        ExecutionState::CompletedWithConstraints,
                        ExecutionState::Failed,
                        ExecutionState::Canceled,
                    ],
                    "execution_constraint friction payload must use a constrained execution_state",
                )?;
                require_some(
                    self.execution_plan_id,
                    "execution_constraint friction payload must include execution_plan_id",
                )?;
                require_some_ref(
                    self.stable_plan_hash.as_deref(),
                    "execution_constraint friction payload must include stable_plan_hash",
                )?;
                require_empty_denial_guards(
                    &self.denial_guards,
                    "execution_constraint friction payload must not include denial guards",
                )?;
                require_none(
                    self.review_package_id,
                    "execution_constraint friction payload must not include review_package_id",
                )?;
            }
        }

        Ok(())
    }

    pub fn validated(self) -> FaLocalResult<ValidatedFrictionPayload> {
        self.validate()?;
        Ok(ValidatedFrictionPayload { payload: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFrictionPayload {
    pub payload: FrictionPayload,
}

impl ValidatedFrictionPayload {
    pub fn new(payload: FrictionPayload) -> FaLocalResult<Self> {
        payload.validated()
    }
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}

fn validate_required_summary(summary: &str) -> FaLocalResult<()> {
    if summary.is_empty() || summary.len() > 160 {
        return Err(contract_invalid(
            "friction payload operator_visible_summary must be between 1 and 160 characters",
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
            "friction payload execution_plan_id and stable_plan_hash must be present together or absent together",
        )),
    }
}

fn validate_denial_guards(denial_guards: &[DenialGuard]) -> FaLocalResult<()> {
    if denial_guards.len() > 5 {
        return Err(contract_invalid(
            "friction payload denial_guards must contain at most 5 items",
        ));
    }

    for guard in denial_guards {
        if guard.summary.is_empty() || guard.summary.len() > 500 {
            return Err(contract_invalid(
                "friction payload denial guard summaries must be between 1 and 500 characters",
            ));
        }
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
            "denied friction payload state must use denied posture",
        ),
        ExecutionState::ReviewRequired => require_posture(
            current_posture,
            &[ApprovalPosture::ReviewRequired],
            "review_required friction payload state must use review_required posture",
        ),
        ExecutionState::WaitingExplicitApproval => require_posture(
            current_posture,
            &[ApprovalPosture::ExplicitOperatorApproval],
            "waiting_explicit_approval friction payload state must use explicit_operator_approval posture",
        ),
        ExecutionState::Degraded
        | ExecutionState::PartialSuccess
        | ExecutionState::CompletedWithConstraints
        | ExecutionState::Failed
        | ExecutionState::Canceled => require_posture(
            current_posture,
            &[
                ApprovalPosture::PolicyPreapproved,
                ApprovalPosture::ExecuteAllowed,
            ],
            "constrained friction payload states must use policy_preapproved or execute_allowed posture",
        ),
        ExecutionState::AdmittedNotStarted
        | ExecutionState::InProgress
        | ExecutionState::Completed => Err(contract_invalid(
            "friction payload execution_state must represent active friction, not friction-free execution",
        )),
    }
}

fn validate_degraded_subtype(
    execution_state: ExecutionState,
    degraded_subtype: Option<DegradedSubtype>,
) -> FaLocalResult<()> {
    match execution_state {
        ExecutionState::Degraded | ExecutionState::PartialSuccess => require_some(
            degraded_subtype,
            "constrained friction payload must include degraded_subtype for degraded or partial_success state",
        )
        .map(|_| ()),
        ExecutionState::CompletedWithConstraints => {
            let degraded_subtype = require_some(
                degraded_subtype,
                "completed_with_constraints friction payload must include an explicit fallback degraded_subtype",
            )?;
            if !is_explicit_fallback_subtype(Some(degraded_subtype)) {
                return Err(contract_invalid(
                    "completed_with_constraints friction payload must include an explicit fallback degraded_subtype",
                ));
            }
            Ok(())
        }
        _ => {
            if degraded_subtype.is_some() {
                return Err(contract_invalid(
                    "friction payload must not include degraded_subtype for this execution_state",
                ));
            }
            Ok(())
        }
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

fn require_action(
    action: OperatorAction,
    allowed: &[OperatorAction],
    message: &'static str,
) -> FaLocalResult<()> {
    if allowed.contains(&action) {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn require_non_empty_denial_guards(
    denial_guards: &[DenialGuard],
    message: &'static str,
) -> FaLocalResult<()> {
    if denial_guards.is_empty() {
        Err(contract_invalid(message))
    } else {
        Ok(())
    }
}

fn require_empty_denial_guards(
    denial_guards: &[DenialGuard],
    message: &'static str,
) -> FaLocalResult<()> {
    if denial_guards.is_empty() {
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
