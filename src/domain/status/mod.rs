use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::shared::{
    ApprovalPosture, CorrelationId, DegradedSubtype, ExecutionPlanId, ExecutionState, RequestId,
    SchemaName, TimestampUtc, deserialize_contract_value,
};
use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionStatus {
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub execution_plan_id: Option<ExecutionPlanId>,
    pub stable_plan_hash: Option<String>,
    pub current_posture: ApprovalPosture,
    pub state: ExecutionState,
    pub degraded_subtype: Option<DegradedSubtype>,
    pub started_at_utc: Option<TimestampUtc>,
    pub updated_at_utc: TimestampUtc,
    pub completed_at_utc: Option<TimestampUtc>,
    pub current_step: Option<String>,
    pub completion_summary: Option<String>,
    pub failure_summary: Option<String>,
    pub truthful_user_visible_summary: String,
}

impl ExecutionStatus {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: RequestId,
        correlation_id: CorrelationId,
        execution_plan_id: Option<ExecutionPlanId>,
        stable_plan_hash: Option<String>,
        current_posture: ApprovalPosture,
        state: ExecutionState,
        degraded_subtype: Option<DegradedSubtype>,
        started_at_utc: Option<TimestampUtc>,
        updated_at_utc: TimestampUtc,
        completed_at_utc: Option<TimestampUtc>,
        current_step: Option<String>,
        completion_summary: Option<String>,
        failure_summary: Option<String>,
        truthful_user_visible_summary: String,
    ) -> FaLocalResult<Self> {
        let status = Self {
            request_id,
            correlation_id,
            execution_plan_id,
            stable_plan_hash,
            current_posture,
            state,
            degraded_subtype,
            started_at_utc,
            updated_at_utc,
            completed_at_utc,
            current_step,
            completion_summary,
            failure_summary,
            truthful_user_visible_summary,
        };
        status.validate()?;
        Ok(status)
    }

    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        deserialize_contract_value(SchemaName::ExecutionStatus, value)
    }

    pub fn validate(&self) -> FaLocalResult<()> {
        validate_optional_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
        validate_optional_step(self.current_step.as_deref())?;
        validate_optional_summary(self.completion_summary.as_deref(), "completion_summary")?;
        validate_optional_summary(self.failure_summary.as_deref(), "failure_summary")?;
        validate_required_summary(
            &self.truthful_user_visible_summary,
            "truthful_user_visible_summary",
        )?;

        if let Some(stable_plan_hash) = self.stable_plan_hash.as_deref() {
            if !is_valid_hash(stable_plan_hash) {
                return Err(contract_invalid(
                    "execution status stable_plan_hash must be a 64-character lowercase hex digest",
                ));
            }
        }

        if let Some(started_at_utc) = self.started_at_utc {
            if self.updated_at_utc < started_at_utc {
                return Err(contract_invalid(
                    "execution status updated_at_utc must not precede started_at_utc",
                ));
            }
        }

        if let Some(completed_at_utc) = self.completed_at_utc {
            if self.updated_at_utc < completed_at_utc {
                return Err(contract_invalid(
                    "execution status updated_at_utc must not precede completed_at_utc",
                ));
            }
            if let Some(started_at_utc) = self.started_at_utc {
                if completed_at_utc < started_at_utc {
                    return Err(contract_invalid(
                        "execution status completed_at_utc must not precede started_at_utc",
                    ));
                }
            }
        }

        match self.state {
            ExecutionState::Denied => {
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::Denied],
                    "denied status must use denied posture",
                )?;
                require_no_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_none(
                    self.started_at_utc,
                    "denied status must not include started_at_utc",
                )?;
                require_none(
                    self.completed_at_utc,
                    "denied status must not include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "denied status must not include current_step",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "denied status must not include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "denied status must not include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "denied status must not include degraded_subtype",
                )?;
            }
            ExecutionState::ReviewRequired => {
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::ReviewRequired],
                    "review_required status must use review_required posture",
                )?;
                require_no_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_none(
                    self.started_at_utc,
                    "review_required status must not include started_at_utc",
                )?;
                require_none(
                    self.completed_at_utc,
                    "review_required status must not include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "review_required status must not include current_step",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "review_required status must not include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "review_required status must not include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "review_required status must not include degraded_subtype",
                )?;
            }
            ExecutionState::WaitingExplicitApproval => {
                require_posture(
                    self.current_posture,
                    &[ApprovalPosture::ExplicitOperatorApproval],
                    "waiting_explicit_approval status must use explicit_operator_approval posture",
                )?;
                require_no_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_none(
                    self.started_at_utc,
                    "waiting_explicit_approval status must not include started_at_utc",
                )?;
                require_none(
                    self.completed_at_utc,
                    "waiting_explicit_approval status must not include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "waiting_explicit_approval status must not include current_step",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "waiting_explicit_approval status must not include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "waiting_explicit_approval status must not include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "waiting_explicit_approval status must not include degraded_subtype",
                )?;
            }
            ExecutionState::AdmittedNotStarted => {
                require_admitted_posture(self.current_posture, "admitted_not_started")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_none(
                    self.started_at_utc,
                    "admitted_not_started status must not include started_at_utc",
                )?;
                require_none(
                    self.completed_at_utc,
                    "admitted_not_started status must not include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "admitted_not_started status must not include current_step",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "admitted_not_started status must not include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "admitted_not_started status must not include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "admitted_not_started status must not include degraded_subtype",
                )?;
            }
            ExecutionState::InProgress => {
                require_admitted_posture(self.current_posture, "in_progress")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_some(
                    self.started_at_utc,
                    "in_progress status must include started_at_utc",
                )?;
                require_none(
                    self.completed_at_utc,
                    "in_progress status must not include completed_at_utc",
                )?;
                require_some_ref(
                    self.current_step.as_deref(),
                    "in_progress status must include current_step",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "in_progress status must not include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "in_progress status must not include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "in_progress status must not include degraded_subtype",
                )?;
            }
            ExecutionState::Degraded => {
                require_admitted_posture(self.current_posture, "degraded")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                let degraded_subtype = require_some(
                    self.degraded_subtype,
                    "degraded status must include degraded_subtype",
                )?;
                require_none(
                    self.completed_at_utc,
                    "degraded status must not include completed_at_utc",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "degraded status must not include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "degraded status must not include failure_summary",
                )?;

                match degraded_subtype {
                    DegradedSubtype::DegradedPreStart
                    | DegradedSubtype::UnavailableDependencyBlock => {
                        require_none(
                            self.started_at_utc,
                            "pre-start degraded status must not include started_at_utc",
                        )?;
                        require_none_ref(
                            self.current_step.as_deref(),
                            "pre-start degraded status must not include current_step",
                        )?;
                    }
                    DegradedSubtype::DegradedInFlight
                    | DegradedSubtype::DegradedFallbackEquivalent
                    | DegradedSubtype::DegradedFallbackLimited
                    | DegradedSubtype::DegradedPartial => {
                        require_some(
                            self.started_at_utc,
                            "in-flight degraded status must include started_at_utc",
                        )?;
                    }
                }
            }
            ExecutionState::PartialSuccess => {
                require_admitted_posture(self.current_posture, "partial_success")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_some(
                    self.started_at_utc,
                    "partial_success status must include started_at_utc",
                )?;
                require_some(
                    self.completed_at_utc,
                    "partial_success status must include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "partial_success status must not include current_step",
                )?;
                require_some_ref(
                    self.completion_summary.as_deref(),
                    "partial_success status must include completion_summary",
                )?;
                require_some_ref(
                    self.failure_summary.as_deref(),
                    "partial_success status must include failure_summary",
                )?;
                if self.degraded_subtype != Some(DegradedSubtype::DegradedPartial) {
                    return Err(contract_invalid(
                        "partial_success status must use degraded_partial degraded_subtype",
                    ));
                }
            }
            ExecutionState::CompletedWithConstraints => {
                require_admitted_posture(self.current_posture, "completed_with_constraints")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_some(
                    self.started_at_utc,
                    "completed_with_constraints status must include started_at_utc",
                )?;
                require_some(
                    self.completed_at_utc,
                    "completed_with_constraints status must include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "completed_with_constraints status must not include current_step",
                )?;
                require_some_ref(
                    self.completion_summary.as_deref(),
                    "completed_with_constraints status must include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "completed_with_constraints status must not include failure_summary",
                )?;
                match self.degraded_subtype {
                    Some(DegradedSubtype::DegradedFallbackEquivalent)
                    | Some(DegradedSubtype::DegradedFallbackLimited) => {}
                    _ => {
                        return Err(contract_invalid(
                            "completed_with_constraints status must include an explicit fallback degraded_subtype",
                        ));
                    }
                }
            }
            ExecutionState::Completed => {
                require_admitted_posture(self.current_posture, "completed")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_some(
                    self.started_at_utc,
                    "completed status must include started_at_utc",
                )?;
                require_some(
                    self.completed_at_utc,
                    "completed status must include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "completed status must not include current_step",
                )?;
                require_some_ref(
                    self.completion_summary.as_deref(),
                    "completed status must include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "completed status must not include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "completed status must not include degraded_subtype",
                )?;
            }
            ExecutionState::Failed => {
                require_admitted_posture(self.current_posture, "failed")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_some(
                    self.started_at_utc,
                    "failed status must include started_at_utc",
                )?;
                require_some(
                    self.completed_at_utc,
                    "failed status must include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "failed status must not include current_step",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "failed status must not include completion_summary",
                )?;
                require_some_ref(
                    self.failure_summary.as_deref(),
                    "failed status must include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "failed status must not include degraded_subtype",
                )?;
            }
            ExecutionState::Canceled => {
                require_admitted_posture(self.current_posture, "canceled")?;
                require_plan_link(self.execution_plan_id, self.stable_plan_hash.as_deref())?;
                require_some(
                    self.completed_at_utc,
                    "canceled status must include completed_at_utc",
                )?;
                require_none_ref(
                    self.current_step.as_deref(),
                    "canceled status must not include current_step",
                )?;
                require_none_ref(
                    self.completion_summary.as_deref(),
                    "canceled status must not include completion_summary",
                )?;
                require_none_ref(
                    self.failure_summary.as_deref(),
                    "canceled status must not include failure_summary",
                )?;
                require_none(
                    self.degraded_subtype,
                    "canceled status must not include degraded_subtype",
                )?;
            }
        }

        Ok(())
    }

    pub fn validated(self) -> FaLocalResult<ValidatedExecutionStatus> {
        self.validate()?;
        Ok(ValidatedExecutionStatus { status: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExecutionStatus {
    pub status: ExecutionStatus,
}

impl ValidatedExecutionStatus {
    pub fn new(status: ExecutionStatus) -> FaLocalResult<Self> {
        status.validated()
    }
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}

fn validate_optional_plan_link(
    execution_plan_id: Option<ExecutionPlanId>,
    stable_plan_hash: Option<&str>,
) -> FaLocalResult<()> {
    match (execution_plan_id, stable_plan_hash) {
        (Some(_), Some(_)) | (None, None) => Ok(()),
        _ => Err(contract_invalid(
            "execution status must carry execution_plan_id and stable_plan_hash together",
        )),
    }
}

fn require_plan_link(
    execution_plan_id: Option<ExecutionPlanId>,
    stable_plan_hash: Option<&str>,
) -> FaLocalResult<()> {
    match (execution_plan_id, stable_plan_hash) {
        (Some(_), Some(_)) => Ok(()),
        _ => Err(contract_invalid(
            "execution status state requires execution_plan_id and stable_plan_hash",
        )),
    }
}

fn require_no_plan_link(
    execution_plan_id: Option<ExecutionPlanId>,
    stable_plan_hash: Option<&str>,
) -> FaLocalResult<()> {
    match (execution_plan_id, stable_plan_hash) {
        (None, None) => Ok(()),
        _ => Err(contract_invalid(
            "pre-execution status must not include execution_plan_id or stable_plan_hash",
        )),
    }
}

fn require_posture(
    current_posture: ApprovalPosture,
    allowed: &[ApprovalPosture],
    message: &str,
) -> FaLocalResult<()> {
    if allowed.contains(&current_posture) {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn require_admitted_posture(
    current_posture: ApprovalPosture,
    state_label: &str,
) -> FaLocalResult<()> {
    require_posture(
        current_posture,
        &[
            ApprovalPosture::PolicyPreapproved,
            ApprovalPosture::ExecuteAllowed,
        ],
        &format!("{state_label} status must use policy_preapproved or execute_allowed posture"),
    )
}

fn require_some<T: Copy>(value: Option<T>, message: &str) -> FaLocalResult<T> {
    value.ok_or_else(|| contract_invalid(message))
}

fn require_none<T>(value: Option<T>, message: &str) -> FaLocalResult<()> {
    if value.is_none() {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn require_some_ref<'a, T: ?Sized>(value: Option<&'a T>, message: &str) -> FaLocalResult<&'a T> {
    value.ok_or_else(|| contract_invalid(message))
}

fn require_none_ref<T: ?Sized>(value: Option<&T>, message: &str) -> FaLocalResult<()> {
    if value.is_none() {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn validate_optional_step(current_step: Option<&str>) -> FaLocalResult<()> {
    if let Some(current_step) = current_step {
        if !is_valid_step_id(current_step) {
            return Err(contract_invalid(
                "execution status current_step must match the bounded step id pattern",
            ));
        }
    }
    Ok(())
}

fn validate_required_summary(summary: &str, field_name: &str) -> FaLocalResult<()> {
    if summary.is_empty() || summary.len() > 160 {
        return Err(contract_invalid(format!(
            "execution status {field_name} must be between 1 and 160 characters",
        )));
    }
    Ok(())
}

fn validate_optional_summary(summary: Option<&str>, field_name: &str) -> FaLocalResult<()> {
    if let Some(summary) = summary {
        validate_required_summary(summary, field_name)?;
    }
    Ok(())
}

fn is_valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_valid_step_id(value: &str) -> bool {
    let len = value.len();
    if !(1..=48).contains(&len) {
        return false;
    }

    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };

    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }

    bytes.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    })
}
