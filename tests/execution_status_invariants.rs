mod support;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use fa_local::domain::status::{ExecutionStatus, ValidatedExecutionStatus};
use fa_local::{
    ApprovalPosture, CorrelationId, DegradedSubtype, ExecutionPlanId, ExecutionState, RequestId,
};

fn ts(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .unwrap()
}

fn base_in_progress_status() -> ExecutionStatus {
    let value = support::load_fixture_json("valid", "execution-status-in-progress-basic.json");
    ExecutionStatus::load_contract_value(&value).unwrap()
}

#[test]
fn valid_execution_status_fixture_passes_typed_invariants() {
    let status = base_in_progress_status();
    status.validate().unwrap();
    let validated = status.validated().unwrap();

    assert_eq!(validated.status.state, ExecutionState::InProgress);
}

#[test]
fn execution_status_state_remains_distinct_from_approval_posture() {
    let waiting = ExecutionStatus::new(
        RequestId::from_uuid(Uuid::parse_str("12121212-1212-4212-8212-121212121212").unwrap()),
        CorrelationId::from_uuid(Uuid::parse_str("34343434-3434-4434-8434-343434343434").unwrap()),
        None,
        None,
        ApprovalPosture::ExplicitOperatorApproval,
        ExecutionState::WaitingExplicitApproval,
        None,
        None,
        ts(2030, 1, 1, 0, 12, 0),
        None,
        None,
        None,
        None,
        "waiting for explicit operator approval".to_owned(),
    )
    .unwrap();

    assert_eq!(
        waiting.current_posture,
        ApprovalPosture::ExplicitOperatorApproval
    );
    assert_eq!(waiting.state, ExecutionState::WaitingExplicitApproval);

    let mut invalid = base_in_progress_status();
    invalid.current_posture = ApprovalPosture::Denied;
    let error = invalid.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: in_progress status must use policy_preapproved or execute_allowed posture"
    );
}

#[test]
fn degraded_status_requires_explicit_degraded_subtype() {
    let mut status = base_in_progress_status();
    status.state = ExecutionState::Degraded;
    status.current_step = Some("step_export_prepare".to_owned());
    status.degraded_subtype = None;

    let error = status.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: degraded status must include degraded_subtype"
    );
}

#[test]
fn completed_with_constraints_requires_explicit_subtype() {
    let mut status = base_in_progress_status();
    status.state = ExecutionState::CompletedWithConstraints;
    status.current_step = None;
    status.started_at_utc = Some(ts(2030, 1, 1, 0, 10, 5));
    status.updated_at_utc = ts(2030, 1, 1, 0, 10, 30);
    status.completed_at_utc = Some(ts(2030, 1, 1, 0, 10, 30));
    status.completion_summary = Some("completed with declared execution limits".to_owned());
    status.truthful_user_visible_summary = "execution completed with declared limits".to_owned();
    status.degraded_subtype = None;

    let error = status.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: completed_with_constraints status must include an explicit fallback degraded_subtype"
    );
}

#[test]
fn completed_state_cannot_fabricate_success_without_completion_summary() {
    let mut status = base_in_progress_status();
    status.state = ExecutionState::Completed;
    status.current_step = None;
    status.started_at_utc = Some(ts(2030, 1, 1, 0, 10, 5));
    status.updated_at_utc = ts(2030, 1, 1, 0, 10, 30);
    status.completed_at_utc = Some(ts(2030, 1, 1, 0, 10, 30));
    status.completion_summary = None;

    let error = status.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: completed status must include completion_summary"
    );
}

#[test]
fn partial_success_requires_truthful_failure_detail() {
    let mut status = base_in_progress_status();
    status.state = ExecutionState::PartialSuccess;
    status.current_step = None;
    status.started_at_utc = Some(ts(2030, 1, 1, 0, 10, 5));
    status.updated_at_utc = ts(2030, 1, 1, 0, 10, 30);
    status.completed_at_utc = Some(ts(2030, 1, 1, 0, 10, 30));
    status.completion_summary = Some("completed partially with bounded degradation".to_owned());
    status.failure_summary = None;
    status.degraded_subtype = Some(DegradedSubtype::DegradedPartial);
    status.truthful_user_visible_summary =
        "execution completed partially with bounded degradation".to_owned();

    let error = status.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: partial_success status must include failure_summary"
    );
}

#[test]
fn a_completed_status_may_mention_fallback_in_text_without_a_fallback_subtype() {
    // Regression guard for KI-FLO-20260918-003: an untrusted, interpolated
    // text field (e.g. a step id or adapter-supplied summary) coincidentally
    // containing the word "fallback" must never fail validation on its own.
    // The real invariant -- CompletedWithConstraints structurally requires
    // an explicit fallback degraded_subtype -- is covered separately by
    // completed_with_constraints_requires_explicit_subtype below; plain
    // Completed carries no such requirement and never did.
    let mut status = base_in_progress_status();
    status.state = ExecutionState::Completed;
    status.current_step = None;
    status.started_at_utc = Some(ts(2030, 1, 1, 0, 10, 5));
    status.updated_at_utc = ts(2030, 1, 1, 0, 10, 30);
    status.completed_at_utc = Some(ts(2030, 1, 1, 0, 10, 30));
    status.completion_summary = Some("completed after fallback substitution".to_owned());
    status.truthful_user_visible_summary =
        "execution completed after fallback substitution".to_owned();
    status.degraded_subtype = None;

    status
        .validate()
        .expect("mentioning fallback in text alone must not fail validation");
}

#[test]
fn completed_with_constraints_fixture_passes_typed_invariants() {
    let value = support::load_fixture_json(
        "valid",
        "execution-status-completed-with-constraints-basic.json",
    );
    let status = ExecutionStatus::load_contract_value(&value).unwrap();
    let validated = ValidatedExecutionStatus::new(status).unwrap();

    assert_eq!(
        validated.status.degraded_subtype,
        Some(DegradedSubtype::DegradedFallbackLimited)
    );
    assert_eq!(
        validated.status.state,
        ExecutionState::CompletedWithConstraints
    );
}

#[test]
fn pre_execution_states_cannot_carry_plan_linkage() {
    let status = ExecutionStatus::new(
        RequestId::from_uuid(Uuid::parse_str("56565656-5656-4565-8565-565656565656").unwrap()),
        CorrelationId::from_uuid(Uuid::parse_str("78787878-7878-4787-8787-787878787878").unwrap()),
        Some(ExecutionPlanId::from_uuid(
            Uuid::parse_str("99999999-9999-4999-8999-999999999999").unwrap(),
        )),
        Some("a8f7e75a4a4ea309803cf16fec43a4adb788ddcaa5589d91a9c00a41a8f56df7".to_owned()),
        ApprovalPosture::ReviewRequired,
        ExecutionState::ReviewRequired,
        None,
        None,
        ts(2030, 1, 1, 0, 8, 0),
        None,
        None,
        None,
        None,
        "request requires review".to_owned(),
    )
    .unwrap_err();

    assert_eq!(
        status.to_string(),
        "contract invalid: pre-execution status must not include execution_plan_id or stable_plan_hash"
    );
}
