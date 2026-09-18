//! Truthful, bounded forensic events for NeuronForge Local task-dispatch runs.
//!
//! `forensic-event.schema.json` (`src/domain/forensics/mod.rs`) is built
//! entirely around FA Local's own execution-request domain --
//! `route_decision_id`, `execution_plan_id`, `ApprovalPosture`,
//! `ExecutionState` -- none of which a NeuronForge-Local-initiated dispatch
//! has. This is a separate, parallel contract for that domain, matching how
//! `integrations::cortex::forensics::GnatDispatchForensicEvent` already
//! gets its own schema for the same reason.
//!
//! Unlike Gnat dispatch (one negotiation event plus one event per shard),
//! one NeuronForge Local dispatch run produces exactly one
//! [`NeuronForgeTaskDispatchForensicEvent`] -- there is no separate
//! negotiation phase to record.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::shared::{
    ForensicEventId, SchemaName, TimestampUtc, deserialize_contract_value,
};
use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeuronForgeDispatchOutcome {
    Completed,
    NotCompleted,
    DispatchUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeuronForgeReceiptValidationStatus {
    Valid,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeuronForgeForensicRedactionLevel {
    None,
    SensitiveFieldsRedacted,
    LinkageOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeuronForgeTaskDispatchForensicEvent {
    pub forensic_event_id: ForensicEventId,
    pub dispatch_id: String,
    pub request_id: String,
    pub task_id: String,
    pub outcome: NeuronForgeDispatchOutcome,
    pub receipt_validation_status: Option<NeuronForgeReceiptValidationStatus>,
    pub timestamp_utc: TimestampUtc,
    pub summary: String,
    pub redaction_level: NeuronForgeForensicRedactionLevel,
    pub payload_minimized: bool,
}

impl NeuronForgeTaskDispatchForensicEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        forensic_event_id: ForensicEventId,
        dispatch_id: String,
        request_id: String,
        task_id: String,
        outcome: NeuronForgeDispatchOutcome,
        receipt_validation_status: Option<NeuronForgeReceiptValidationStatus>,
        timestamp_utc: TimestampUtc,
        summary: String,
        redaction_level: NeuronForgeForensicRedactionLevel,
        payload_minimized: bool,
    ) -> FaLocalResult<Self> {
        let event = Self {
            forensic_event_id,
            dispatch_id,
            request_id,
            task_id,
            outcome,
            receipt_validation_status,
            timestamp_utc,
            summary,
            redaction_level,
            payload_minimized,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        deserialize_contract_value(SchemaName::NeuronForgeTaskDispatchForensicEvent, value)
    }

    pub fn validate(&self) -> FaLocalResult<()> {
        // Char count, not byte length: `summary`'s JSON Schema `maxLength`
        // is Unicode-codepoint-based per spec, and `bounded_summary()`
        // (the pipeline's own truncation helper) already truncates by char
        // count -- a byte-length check here rejects its own output for any
        // truncated text containing multi-byte characters (its trailing
        // ellipsis alone is 3 bytes), see `KI-FLO-20260918-005`.
        if self.summary.is_empty() || self.summary.chars().count() > 160 {
            return Err(contract_invalid(
                "neuronforge task dispatch forensic event summary must be between 1 and 160 characters",
            ));
        }
        if self.dispatch_id.is_empty() || self.dispatch_id.len() > 200 {
            return Err(contract_invalid(
                "neuronforge task dispatch forensic event dispatch_id must be between 1 and 200 characters",
            ));
        }
        if self.request_id.is_empty() || self.request_id.len() > 200 {
            return Err(contract_invalid(
                "neuronforge task dispatch forensic event request_id must be between 1 and 200 characters",
            ));
        }
        if self.task_id.is_empty() || self.task_id.len() > 200 {
            return Err(contract_invalid(
                "neuronforge task dispatch forensic event task_id must be between 1 and 200 characters",
            ));
        }
        if !self.payload_minimized {
            return Err(contract_invalid(
                "neuronforge task dispatch forensic event payload_minimized must remain true for bounded forensics",
            ));
        }

        match self.outcome {
            NeuronForgeDispatchOutcome::DispatchUnavailable => {
                if self.receipt_validation_status.is_some() {
                    return Err(contract_invalid(
                        "dispatch_unavailable neuronforge task dispatch forensic event must not include receipt_validation_status",
                    ));
                }
            }
            NeuronForgeDispatchOutcome::Completed => {
                if self.receipt_validation_status != Some(NeuronForgeReceiptValidationStatus::Valid)
                {
                    return Err(contract_invalid(
                        "completed neuronforge task dispatch forensic event requires receipt_validation_status valid",
                    ));
                }
            }
            NeuronForgeDispatchOutcome::NotCompleted => match self.receipt_validation_status {
                Some(
                    NeuronForgeReceiptValidationStatus::Degraded
                    | NeuronForgeReceiptValidationStatus::Failed,
                ) => {}
                _ => {
                    return Err(contract_invalid(
                        "not_completed neuronforge task dispatch forensic event requires receipt_validation_status degraded or failed",
                    ));
                }
            },
        }

        Ok(())
    }

    pub fn validated(self) -> FaLocalResult<ValidatedNeuronForgeTaskDispatchForensicEvent> {
        self.validate()?;
        Ok(ValidatedNeuronForgeTaskDispatchForensicEvent { event: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNeuronForgeTaskDispatchForensicEvent {
    pub event: NeuronForgeTaskDispatchForensicEvent,
}

impl ValidatedNeuronForgeTaskDispatchForensicEvent {
    pub fn new(event: NeuronForgeTaskDispatchForensicEvent) -> FaLocalResult<Self> {
        event.validated()
    }
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}
