mod support;

use fa_local::integrations::neuronforge_local::{
    NeuronForgeDispatchOutcome, NeuronForgeForensicRedactionLevel,
    NeuronForgeReceiptValidationStatus, NeuronForgeTaskDispatchForensicEvent,
    ValidatedNeuronForgeTaskDispatchForensicEvent,
};

fn base_completed_event() -> NeuronForgeTaskDispatchForensicEvent {
    let value = support::load_fixture_json(
        "valid",
        "neuronforge-task-dispatch-forensic-event-completed-basic.json",
    );
    NeuronForgeTaskDispatchForensicEvent::load_contract_value(&value).unwrap()
}

fn base_dispatch_unavailable_event() -> NeuronForgeTaskDispatchForensicEvent {
    let value = support::load_fixture_json(
        "valid",
        "neuronforge-task-dispatch-forensic-event-dispatch-unavailable-basic.json",
    );
    NeuronForgeTaskDispatchForensicEvent::load_contract_value(&value).unwrap()
}

#[test]
fn valid_fixtures_load_and_validate() {
    let completed = base_completed_event();
    completed.validate().unwrap();
    assert_eq!(completed.outcome, NeuronForgeDispatchOutcome::Completed);
    assert_eq!(
        completed.receipt_validation_status,
        Some(NeuronForgeReceiptValidationStatus::Valid)
    );

    let unavailable = base_dispatch_unavailable_event();
    let validated = ValidatedNeuronForgeTaskDispatchForensicEvent::new(unavailable).unwrap();
    assert_eq!(
        validated.event.outcome,
        NeuronForgeDispatchOutcome::DispatchUnavailable
    );
    assert_eq!(validated.event.receipt_validation_status, None);
}

#[test]
fn completed_requires_valid_receipt_status() {
    let mut event = base_completed_event();
    event.receipt_validation_status = None;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: completed neuronforge task dispatch forensic event requires receipt_validation_status valid"
    );
}

#[test]
fn completed_rejects_a_degraded_receipt_status() {
    let mut event = base_completed_event();
    event.receipt_validation_status = Some(NeuronForgeReceiptValidationStatus::Degraded);

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: completed neuronforge task dispatch forensic event requires receipt_validation_status valid"
    );
}

#[test]
fn not_completed_requires_degraded_or_failed_receipt_status() {
    let mut event = base_completed_event();
    event.outcome = NeuronForgeDispatchOutcome::NotCompleted;
    event.receipt_validation_status = Some(NeuronForgeReceiptValidationStatus::Valid);

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: not_completed neuronforge task dispatch forensic event requires receipt_validation_status degraded or failed"
    );

    event.receipt_validation_status = Some(NeuronForgeReceiptValidationStatus::Degraded);
    event.validate().unwrap();

    event.receipt_validation_status = Some(NeuronForgeReceiptValidationStatus::Failed);
    event.validate().unwrap();
}

#[test]
fn dispatch_unavailable_must_not_include_a_receipt_status() {
    let mut event = base_dispatch_unavailable_event();
    event.receipt_validation_status = Some(NeuronForgeReceiptValidationStatus::Valid);

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: dispatch_unavailable neuronforge task dispatch forensic event must not include receipt_validation_status"
    );
}

#[test]
fn payload_minimized_must_remain_true() {
    let mut event = base_completed_event();
    event.payload_minimized = false;

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: neuronforge task dispatch forensic event payload_minimized must remain true for bounded forensics"
    );
}

#[test]
fn summary_must_be_bounded() {
    let mut event = base_completed_event();
    event.summary = String::new();

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: neuronforge task dispatch forensic event summary must be between 1 and 160 characters"
    );
}

#[test]
fn dispatch_id_must_be_bounded() {
    let mut event = base_completed_event();
    event.dispatch_id = String::new();

    let error = event.validate().unwrap_err();
    assert_eq!(
        error.to_string(),
        "contract invalid: neuronforge task dispatch forensic event dispatch_id must be between 1 and 200 characters"
    );
}

#[test]
fn redaction_level_round_trips() {
    let mut event = base_completed_event();
    event.redaction_level = NeuronForgeForensicRedactionLevel::None;
    event.validate().unwrap();
}
