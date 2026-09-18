mod support;

use fa_local::integrations::cortex::{
    GnatDispatchForensicEvent, GnatForensicEventType, GnatForensicRedactionLevel,
    GnatNegotiationOutcome, GnatReceiptState, GnatShardOutcome, GnatWorkerType,
    ValidatedGnatDispatchForensicEvent,
};

fn base_negotiated_event() -> GnatDispatchForensicEvent {
    let value = support::load_fixture_json(
        "valid",
        "gnat-dispatch-forensic-event-negotiated-basic.json",
    );
    GnatDispatchForensicEvent::load_contract_value(&value).unwrap()
}

fn base_shard_dispatched_event() -> GnatDispatchForensicEvent {
    let value = support::load_fixture_json(
        "valid",
        "gnat-dispatch-forensic-event-shard-dispatched-basic.json",
    );
    GnatDispatchForensicEvent::load_contract_value(&value).unwrap()
}

#[test]
fn valid_fixtures_load_and_validate() {
    let negotiated = base_negotiated_event();
    negotiated.validate().unwrap();
    assert_eq!(
        negotiated.event_type,
        GnatForensicEventType::GnatDispatchNegotiated
    );
    assert_eq!(
        negotiated.negotiation_outcome,
        GnatNegotiationOutcome::ReadyForFaLocalDispatch
    );

    let shard_dispatched = base_shard_dispatched_event();
    let validated = ValidatedGnatDispatchForensicEvent::new(shard_dispatched).unwrap();
    assert_eq!(
        validated.event.event_type,
        GnatForensicEventType::GnatShardDispatched
    );
    assert_eq!(
        validated.event.worker_type,
        Some(GnatWorkerType::MarkdownSyntax)
    );
    assert_eq!(
        validated.event.shard_outcome,
        Some(GnatShardOutcome::Completed)
    );
    assert_eq!(
        validated.event.receipt_state,
        Some(GnatReceiptState::Complete)
    );
}

#[test]
fn negotiated_event_rejects_a_shard_id() {
    let mut event = base_negotiated_event();
    event.shard_id = Some("some-shard".to_owned());

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: gnat_dispatch_negotiated forensic event must not include shard_id"
    );
}

#[test]
fn shard_dispatched_event_requires_shard_id() {
    let mut event = base_shard_dispatched_event();
    event.shard_id = None;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: gnat_shard_dispatched forensic event must include shard_id"
    );
}

#[test]
fn shard_dispatched_event_requires_ready_negotiation_outcome() {
    let mut event = base_shard_dispatched_event();
    event.negotiation_outcome = GnatNegotiationOutcome::SerialFallbackPermitted;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: gnat_shard_dispatched forensic event requires negotiation_outcome ready_for_fa_local_dispatch"
    );
}

#[test]
fn a_completed_shard_requires_a_receipt_state() {
    let mut event = base_shard_dispatched_event();
    event.receipt_state = None;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: gnat_shard_dispatched forensic event must include receipt_state"
    );
}

#[test]
fn a_dispatch_unavailable_shard_must_not_include_a_receipt_state() {
    let mut event = base_shard_dispatched_event();
    event.shard_outcome = Some(GnatShardOutcome::DispatchUnavailable);
    // receipt_state left populated from the fixture -- must be rejected.

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: dispatch_unavailable gnat_shard_dispatched forensic event must not include receipt_state"
    );
}

#[test]
fn payload_minimized_must_remain_true() {
    let mut event = base_negotiated_event();
    event.payload_minimized = false;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: gnat dispatch forensic event payload_minimized must remain true for bounded forensics"
    );
}

#[test]
fn summary_must_be_bounded() {
    let mut event = base_negotiated_event();
    event.summary = String::new();

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: gnat dispatch forensic event summary must be between 1 and 160 characters"
    );
}

#[test]
fn redaction_level_round_trips() {
    let mut event = base_negotiated_event();
    event.redaction_level = GnatForensicRedactionLevel::None;
    event.validate().unwrap();
}
