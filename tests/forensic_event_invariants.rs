mod support;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use fa_local::domain::forensics::{
    ForensicEvent, ForensicEventType, RedactionLevel, ValidatedForensicEvent,
};
use fa_local::{
    ApprovalPosture, CorrelationId, DegradedSubtype, ExecutionPlanId, ExecutionState,
    ForensicEventId, RequestId, ReviewPackageId, RouteDecisionId,
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

fn base_denial_event() -> ForensicEvent {
    let value = support::load_fixture_json("valid", "forensic-event-denial-basic.json");
    ForensicEvent::load_contract_value(&value).unwrap()
}

fn base_review_event() -> ForensicEvent {
    let value = support::load_fixture_json("valid", "forensic-event-review-package-basic.json");
    ForensicEvent::load_contract_value(&value).unwrap()
}

fn base_execution_event() -> ForensicEvent {
    let value = support::load_fixture_json("valid", "forensic-event-execution-status-basic.json");
    ForensicEvent::load_contract_value(&value).unwrap()
}

#[test]
fn valid_forensic_event_fixtures_load_and_validate() {
    let denial = base_denial_event();
    denial.validate().unwrap();

    let review = base_review_event();
    review.validate().unwrap();

    let execution = base_execution_event();
    let validated = ValidatedForensicEvent::new(execution).unwrap();

    assert_eq!(
        validated.event.event_type,
        ForensicEventType::ExecutionStatusObserved
    );
}

#[test]
fn forensic_event_preserves_posture_state_distinction() {
    let review = base_review_event();
    assert_eq!(
        review.current_posture,
        ApprovalPosture::ExplicitOperatorApproval
    );
    assert_eq!(
        review.execution_state,
        ExecutionState::WaitingExplicitApproval
    );

    let execution = base_execution_event();
    assert_eq!(
        execution.current_posture,
        ApprovalPosture::PolicyPreapproved
    );
    assert_eq!(
        execution.execution_state,
        ExecutionState::CompletedWithConstraints
    );
}

#[test]
fn admitted_forensic_event_rejects_non_admitted_posture() {
    let mut event = base_execution_event();
    event.current_posture = ApprovalPosture::ReviewRequired;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: admitted forensic event states must use policy_preapproved or execute_allowed posture"
    );
}

#[test]
fn forensic_event_rejects_planner_or_workflow_narration() {
    let mut event = base_execution_event();
    event.summary = "planner decided the next step is a workflow reroute".to_owned();

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: forensic event summary must not narrate planner, workflow, or semantic interpretation"
    );
}

#[test]
fn a_forensic_event_may_mention_fallback_in_text_without_a_fallback_subtype() {
    // Regression guard for KI-FLO-20260918-003: an untrusted, interpolated
    // text field coincidentally containing the word "fallback" must never
    // fail validation on its own. The real invariant --
    // completed_with_constraints structurally requires an explicit fallback
    // degraded_subtype -- is covered separately below.
    let mut event = base_review_event();
    event.summary = "bounded review handoff includes a fallback path".to_owned();
    event.degraded_subtype = None;

    event
        .validate()
        .expect("mentioning fallback in text alone must not fail validation");
}

#[test]
fn forensic_event_constructor_rejects_non_minimized_payloads() {
    let error = ForensicEvent::new(
        ForensicEventId::from_uuid(
            Uuid::parse_str("fafafafa-fafa-4afa-8afa-fafafafafafa").unwrap(),
        ),
        CorrelationId::from_uuid(Uuid::parse_str("66666666-6666-4666-8666-666666666666").unwrap()),
        RequestId::from_uuid(Uuid::parse_str("55555555-5555-4555-8555-555555555555").unwrap()),
        ForensicEventType::DenialIssued,
        None,
        None,
        None,
        None,
        ts(2030, 1, 1, 0, 8, 30),
        ApprovalPosture::Denied,
        ExecutionState::Denied,
        None,
        "request denied due to missing trust basis".to_owned(),
        RedactionLevel::None,
        false,
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "contract invalid: forensic event payload_minimized must remain true for bounded forensics"
    );
}

#[test]
fn review_package_linkage_requires_route_and_plan_references() {
    let mut event = base_review_event();
    event.route_decision_id = None;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: forensic event review_package_id requires route_decision_id, execution_plan_id, and stable_plan_hash"
    );
}

#[test]
fn completed_with_constraints_requires_explicit_fallback_subtype() {
    let mut event = base_execution_event();
    event.degraded_subtype = Some(DegradedSubtype::DegradedInFlight);

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: completed_with_constraints forensic event must include an explicit fallback degraded_subtype"
    );
}

#[test]
fn review_package_event_can_declare_explicit_fallback_without_collapsing_state() {
    let event = ForensicEvent::new(
        ForensicEventId::from_uuid(
            Uuid::parse_str("f3f3f3f3-f3f3-4ff3-8ff3-f3f3f3f3f3f3").unwrap(),
        ),
        CorrelationId::from_uuid(Uuid::parse_str("66666666-6666-4666-8666-666666666666").unwrap()),
        RequestId::from_uuid(Uuid::parse_str("55555555-5555-4555-8555-555555555555").unwrap()),
        ForensicEventType::ReviewPackagePrepared,
        Some(RouteDecisionId::from_uuid(
            Uuid::parse_str("77777777-7777-4777-8777-777777777773").unwrap(),
        )),
        Some(ExecutionPlanId::from_uuid(
            Uuid::parse_str("99999999-9999-4999-8999-999999999999").unwrap(),
        )),
        Some("a8f7e75a4a4ea309803cf16fec43a4adb788ddcaa5589d91a9c00a41a8f56df7".to_owned()),
        Some(ReviewPackageId::from_uuid(
            Uuid::parse_str("abababab-abab-4bab-8bab-abababababab").unwrap(),
        )),
        ts(2030, 1, 1, 0, 9, 45),
        ApprovalPosture::ExplicitOperatorApproval,
        ExecutionState::WaitingExplicitApproval,
        Some(DegradedSubtype::DegradedFallbackLimited),
        "bounded review handoff includes a fallback path".to_owned(),
        RedactionLevel::SensitiveFieldsRedacted,
        true,
    )
    .unwrap();

    assert_eq!(
        event.current_posture,
        ApprovalPosture::ExplicitOperatorApproval
    );
    assert_eq!(
        event.execution_state,
        ExecutionState::WaitingExplicitApproval
    );
}
